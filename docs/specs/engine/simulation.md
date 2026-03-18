---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Simulation Tick

## Purpose

Overarching simulation framework for world updates that run independently of the render frame rate. Provides a fixed-rate tick system with speed control (pause, 1x, 2x, 5x, 10x), a priority-based system registry where each system can specify its own tick interval, and a spiral-of-death cap to prevent runaway simulation. Designed for God Sim and Sandbox games where liquid flow, agent AI, tile updates, and day-night cycles need deterministic, frame-rate-independent updates.

## Public API

Existing implementation in `crates/amigo_core/src/simulation.rs`.

### SimSpeed

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SimSpeed {
    Paused,
    Normal,
    Fast,
    VeryFast,
    Ultra,
    Custom(f32),
}

impl SimSpeed {
    pub fn multiplier(self) -> f32;
    pub fn next(self) -> Self;
}
```

### SimSystem Trait

```rust
pub trait SimSystem {
    fn priority(&self) -> u8;         // Lower runs first. Default: 100.
    fn tick_interval(&self) -> u32;   // Run every N sim-ticks. Default: 1.
    fn name(&self) -> &str;
    fn update(&mut self, ctx: &mut SimContext);
}

pub struct SimContext {
    pub tick: u64,
    pub dt: f64,
    pub speed: f32,
}
```

### SimulationRunner

```rust
pub struct SimulationRunner {
    pub ticks_per_second: u32,
    pub speed: SimSpeed,
    pub tick: u64,
    accumulator: f64,
    systems: Vec<Box<dyn SimSystem>>,
    dirty: bool,
    pub max_ticks_per_frame: u32,
}

impl SimulationRunner {
    pub fn new(ticks_per_second: u32) -> Self;
    pub fn add_system(&mut self, system: Box<dyn SimSystem>);
    pub fn set_speed(&mut self, speed: SimSpeed);
    pub fn toggle_speed(&mut self);
    pub fn advance(&mut self, real_dt: f64) -> u32;
    pub fn reset(&mut self);
    pub fn system_count(&self) -> usize;
    pub fn is_paused(&self) -> bool;
    pub fn alpha(&self) -> f32;
}
```

## Behavior

- **Fixed timestep:** The runner accumulates real-time delta and consumes it in fixed-size ticks of `1 / ticks_per_second` seconds. This ensures deterministic behavior regardless of frame rate.
- **Speed multiplier:** Real-time delta is multiplied by `SimSpeed::multiplier()` before accumulation. `Paused` (0.0) produces zero ticks. `Ultra` (10.0) runs 10x real-time.
- **Speed cycling:** `toggle_speed()` cycles through Paused -> Normal -> Fast -> VeryFast -> Ultra -> Paused. `Custom` resets to Normal.
- **Priority ordering:** Systems are sorted by `priority()` (lower first) on the first `advance()` after a system is added. Lower-priority systems run earlier each tick.
- **Tick intervals:** A system with `tick_interval() == 3` runs only on ticks divisible by 3. This allows expensive systems (e.g., [liquid simulation](liquids.md)) to run less frequently.
- **Spiral-of-death cap:** At most `max_ticks_per_frame` ticks execute per `advance()` call (default: 10). If the accumulator exceeds this cap, excess time is discarded to prevent the simulation from falling further behind.
- **Interpolation alpha:** `alpha()` returns the fractional progress between the last sim tick and the next, useful for smooth rendering interpolation.
- **Reset:** Clears the tick counter and accumulator without removing systems.

## Internal Design

- Systems are stored in a `Vec<Box<dyn SimSystem>>` and sorted lazily (only when `dirty` flag is set after `add_system`).
- Each tick creates a `SimContext` with the current tick number, fixed dt, and speed multiplier, then iterates all systems checking their tick interval.
- The accumulator is clamped after the tick loop to prevent unbounded growth.

## Non-Goals

- **Parallelism.** Systems run sequentially in priority order. Multi-threaded system execution is out of scope.
- **Deterministic replay.** While the fixed timestep enables determinism, input recording and replay infrastructure is not part of this module.
- **Rendering.** The simulation runner is purely logic-side; it does not interact with the render pipeline. Use `alpha()` for render-side interpolation.

## Open Questions

- Should systems declare read/write dependencies on world data to enable future parallel execution?
- Should the runner support removing or disabling individual systems at runtime?
- How should the simulation runner integrate with [save/load](save-load.md) -- should `tick` be persisted and restored?
