# ADR-0001: Fixed-Point Math for the Deterministic Simulation Layer

- Status: accepted
- Date: 2026-06-09

## Context

The engine promises deterministic simulation: the same inputs must produce
bit-identical state on every machine. This is the foundation for rollback
netcode (`amigo_net`), replays, and state-rewind. IEEE-754 floating point is
not reliably deterministic across compilers, CPU targets, and math-library
versions (FMA contraction, x87 vs SSE, libm differences), so simulation state
cannot be stored in `f32`.

## Decision

All simulation-layer math uses Q16.16 fixed-point numbers via the `fixed`
crate: `amigo_core::math::Fix` is an alias for `fixed::types::I16F16`, and
positions/velocities in simulation space use `SimVec2 { x: Fix, y: Fix }`.

Rendering and presentation code (sprite positions, cameras, particles, UI)
uses `f32`/`RenderVec2`; values cross from simulation to render space through
explicit conversions. Operations that overflow easily are wrapped in
deterministic helpers (e.g. `SimVec2::length` and `distance_squared` work on
widened integers because `x*x` exceeds the I16F16 range above ~181).

## Consequences

- Simulation state hashes (CRC checksums in `amigo_net::checksum`) match
  across platforms, enabling desync detection and rollback.
- The numeric range is limited to ±32767 with 1/65536 precision; world
  coordinates must stay within that envelope or use chunk-local coordinates.
- Game code must be careful not to leak `f32` into simulation state; the
  convention is documented in CONTRIBUTING.md ("fixed-point for simulation,
  f32 for rendering").
- Math helpers (sqrt, trig) must be implemented or wrapped deterministically
  rather than calling libm directly.

## Status of the implementation (2026-10)

Deterministic helpers, all integer arithmetic:

- `math::sqrt_fix`, `SimVec2::length`, `normalize`, `distance_squared`, `dot`.
- `math::trig`: `sin_cos_fix`, `sin_fix`, `cos_fix`, `atan2_fix`, `acos_fix`,
  `asin_fix` (CORDIC on Q2.30), `exp2_fix`, and `SimVec2::from_angle`,
  `angle`, `rotate`.
- `EasingFn::apply_fix` for easing in simulation code.

`crates/amigo_core/tests/determinism.rs` pins golden hashes of these on every
CI platform.

Modules that follow this ADR: `rts` (formations), `navigation`, `spline`,
`math`, `physics`, `collision`, `broad_phase`, `raycast`, `bullet_pattern`,
the shmup hitbox and graze test, and the genre modules built on `SimVec2`.
Physics iterates bodies in `EntityId` order and sorts contact pairs and
spatial-hash results, so hash-map order no longer leaks into the simulation.
The GPU broad phase computes in `f32` with one unit of slack and confirms each
pair in fixed point, so it returns exactly what the CPU one does.

Modules that still compute simulation state in `f32` and are therefore not
safe for lockstep or cross-platform replays yet: `combat`, `ai` (steering),
`td_systems`, `platformer`, `idle` (f64 economy), the rest of `shmup`
(rank, scoring, player config) and `metroidvania` room bounds (`Rect`).
`tween::Tween` counts time in `f32` seconds and is presentation-only by design.
