---
number: "0006"
title: Cross-Platform Determinism
status: done
date: 2026-03-20
---

# ADR-0006: Cross-Platform Determinism

## Status

proposed

## Context

The engine uses fixed-point arithmetic (`Fix = I16F16` from the `fixed` crate) for all simulation math (`crates/amigo_core/src/math.rs`, line 5). `SimVec2` uses `Fix` for positions and velocities (line 9-13), and all game logic operates on these types. This is the correct foundation for determinism.

However, the `sqrt_fix` function (`math.rs`, lines 142-156) breaks cross-platform determinism:

```rust
pub fn sqrt_fix(x: Fix) -> Fix {
    if x <= Fix::ZERO {
        return Fix::ZERO;
    }
    let approx: f32 = x.to_num::<f32>().sqrt();
    let mut g = Fix::from_num(approx.max(f32::MIN_POSITIVE));
    for _ in 0..8 {
        let g_next = (g + x / g) / Fix::from_num(2_i32);
        if g_next == g {
            break;
        }
        g = g_next;
    }
    g
}
```

Line 146 converts the fixed-point value to `f32` and calls `f32::sqrt()` to seed the Newton-Raphson iteration. The comment on line 140-141 claims "IEEE 754 sqrt is correctly rounded -- deterministic," but this is only true within a single platform. Different platforms can produce different `f32::sqrt()` results due to:

1. **x87 vs. SSE.** On x86, the x87 FPU uses 80-bit extended precision internally, which can produce a differently-rounded `f32` result than SSE's native 32-bit `sqrtss`. Rust defaults to SSE on x86-64 but x87 may be used on 32-bit x86.
2. **ARM NEON vs. VFP.** Different ARM implementations may produce different rounding for `fsqrt`.
3. **Wasm.** WebAssembly specifies IEEE 754 semantics but NaN bit patterns differ, and `sqrt` of denormals may vary.

If the initial seed `g` differs by even 1 ULP, the Newton-Raphson iteration may converge to a different fixed-point result, because `Fix::from_num(approx)` rounds differently. The subsequent 8 iterations are pure fixed-point and deterministic, but they amplify the initial seed difference.

This function is called by `SimVec2::length()` (line 122) and `SimVec2::normalize()` (line 128), which are used pervasively in game logic: pathfinding, collision detection, movement normalization.

The `StateHasher` in `crates/amigo_net/src/checksum.rs` hashes `Fix` values via `to_bits()` (line 287-288 in the test), so any divergence in `sqrt_fix` will cause desync detection to fire in multiplayer. The determinism tests in `checksum.rs` (lines 252-301, `run_deterministic_sim`) do not exercise `sqrt_fix` -- they only use addition, subtraction, and comparison, so they pass even though `sqrt_fix` is broken.

## Decision

Replace the `f32`-seeded `sqrt_fix` with a **pure integer arithmetic** implementation. No feature flag -- this is a correctness fix.

The replacement uses a binary search / bit-by-bit algorithm operating entirely on the `i32` bit representation of `Fix` (Q16.16):

1. Work on the raw `i32` bits via `x.to_bits()`.
2. Use a standard integer square root algorithm (shift-and-subtract) adapted for Q16.16: the input is shifted left by 16 bits to account for the fractional part, then a 48-bit integer square root is computed, yielding a 24-bit result that maps back to Q16.16.
3. No floating-point operations anywhere.

Additionally, audit all other simulation math for hidden `f32` usage:
- `Health::fraction()` (`world.rs`, line 36) uses `f32` -- this is render-only, acceptable.
- `SimVec2::to_render()` (line 38) converts to `f32` -- render-only, acceptable.
- No other simulation-path functions use `f32`.

### Alternatives Considered

1. **Force SSE2 on all x86 targets via `-C target-feature=+sse2` and accept Wasm differences.** Rejected because it does not solve ARM or Wasm determinism, and compiler flags are fragile (not enforced by the type system).

2. **Use a lookup table for sqrt approximation.** A 256-entry table for the initial estimate, then Newton-Raphson. Faster than binary search but still requires careful table construction to guarantee identical results. Rejected in favor of the simpler bit-by-bit algorithm that needs no lookup table.

## Migration Path

1. **Implement pure-integer `sqrt_fix_int`** -- Add a new function in `math.rs` that computes the square root of a Q16.16 value using only integer arithmetic. The algorithm: treat the Q16.16 value as a 32-bit integer, left-shift by 16 to get a 48-bit value, compute the integer square root yielding a 24-bit result, and reinterpret as Q16.16. Verify: property test that for all representable positive Q16.16 values, `sqrt_fix_int(x) * sqrt_fix_int(x)` is within 1 ULP of `x`.

2. **Replace `sqrt_fix` with `sqrt_fix_int`** -- Swap the implementation in `math.rs` (line 142), keeping the old version behind `#[cfg(test)]` for comparison. Update `SimVec2::length()` and `normalize()` to use the new function (they already call `sqrt_fix`, so this is a one-line change). Verify: run all existing tests (`cargo test -p amigo_core`), plus the determinism tests in `amigo_net` (`cargo test -p amigo_net`).

3. **Add cross-platform verification test** -- Create a test that computes `sqrt_fix_int` for a set of 1000 fixed input values and asserts the results match a hardcoded table of expected outputs. This table is the "golden reference." Verify: test passes on x86-64, aarch64 (via cross-compilation or CI), and wasm32 (via `wasm-pack test`).

4. (rough) Audit `amigo_core` for any other `f32`/`f64` usage in simulation paths.
5. (rough) Add a CI job that cross-compiles the determinism test to wasm32 and aarch64.

## Abort Criteria

N/A -- this is a correctness fix. Cross-platform determinism is a hard requirement for rollback netcode (AP-07). If the pure-integer sqrt is too slow (>2x regression in `sqrt_fix` microbenchmark), optimize with a lookup table seed, but the fix must ship.

## Consequences

### Positive
- Bit-identical simulation results across x86-64, aarch64, and wasm32.
- Enables reliable rollback netcode (AP-07) and replay verification.
- Removes a subtle, hard-to-diagnose desync source.

### Negative / Trade-offs
- Pure-integer sqrt may be slower than the f32-seeded version (the f32 sqrt is a single hardware instruction; the integer version is ~24 iterations of shift-and-subtract). Benchmarking will quantify the impact.
- The migration requires verifying that the new implementation produces results close enough to the old one that existing game behavior does not visibly change (positions, distances, etc.).

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
