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
deterministic helpers (e.g. `SimVec2::length` pre-scales large vectors because
`x*x` exceeds the I16F16 range above ~181).

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
