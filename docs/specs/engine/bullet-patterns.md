---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/particles"]
last_updated: 2026-03-18
---

# Bullet Patterns

## Purpose

Provides an object-pooled bullet system, declarative pattern shapes, timed emitters, and multi-phase pattern sequencing for bullet-hell gameplay, tower projectiles, and boss attacks.

Existing implementation in `crates/amigo_core/src/bullet_pattern.rs` (812 lines).

## Public API

### Bullet

```rust
/// A single bullet in the pool.
#[derive(Clone, Debug)]
pub struct Bullet {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub lifetime: u32,
    pub max_lifetime: u32,
    pub radius: f32,
    pub damage: f32,
    pub active: bool,
    /// User-defined type tag for rendering/effects.
    pub kind: u32,
    /// Generation counter — incremented each time this slot is reused.
    /// Used by shmup grazing system to distinguish recycled bullets.
    pub generation: u32,
}
```

### BulletEvent

```rust
/// Events produced by the bullet system.
#[derive(Clone, Debug, PartialEq)]
pub enum BulletEvent {
    /// Bullet expired (lifetime ran out).
    Expired { index: usize },
    /// Bullet left the arena bounds.
    OutOfBounds { index: usize },
}
```

### BulletPool

```rust
/// Object pool for bullets. Pre-allocates capacity to avoid per-frame allocations.
pub struct BulletPool {
    pub bullets: Vec<Bullet>,
    pub active_count: usize,
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_w: f32,
    pub bounds_h: f32,
}

impl BulletPool {
    pub fn new(capacity: usize) -> Self;
    pub fn with_bounds(mut self, x: f32, y: f32, w: f32, h: f32) -> Self;
    pub fn spawn(&mut self, x: f32, y: f32, vx: f32, vy: f32,
                 lifetime: u32, radius: f32, damage: f32, kind: u32) -> Option<usize>;
    pub fn despawn(&mut self, index: usize);
    pub fn tick(&mut self) -> Vec<BulletEvent>;
    pub fn check_circle_hits(&self, cx: f32, cy: f32, radius: f32) -> Vec<usize>;
    pub fn clear(&mut self);
    pub fn active_iter(&self) -> impl Iterator<Item = (usize, &Bullet)>;
}
```

### PatternShape

```rust
/// Shape of a bullet pattern spawn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PatternShape {
    /// Evenly distributed bullets in a circle.
    Radial { count: u32, speed: f32 },
    /// Spiral pattern that rotates over time.
    Spiral { count: u32, speed: f32, rotation_speed: f32 },
    /// Aimed at a target with spread.
    Aimed { count: u32, speed: f32, spread_angle: f32 },
    /// Sine wave pattern.
    Wave { count: u32, speed: f32, amplitude: f32, frequency: f32 },
    /// Random directions.
    Random { count: u32, min_speed: f32, max_speed: f32 },
}
```

### compute_pattern

```rust
/// Compute bullet velocities for a pattern shape.
/// `rotation` is the current emitter rotation in radians.
/// `target_angle` is the angle toward the target (for Aimed).
/// `rng_state` is for Random patterns (XorShift64).
pub fn compute_pattern(
    shape: &PatternShape,
    rotation: f32,
    target_angle: f32,
    rng_state: &mut u64,
) -> Vec<(f32, f32)>;
```

### BulletEmitter

```rust
/// A bullet emitter that fires patterns at a fixed rate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BulletEmitter {
    pub x: f32,
    pub y: f32,
    pub pattern: PatternShape,
    pub fire_interval: u32,      // ticks between firings
    pub timer: u32,
    pub rotation: f32,           // current rotation (accumulates for Spiral)
    pub bullet_lifetime: u32,
    pub bullet_radius: f32,
    pub bullet_damage: f32,
    pub bullet_kind: u32,
    pub active: bool,
    pub target_x: f32,
    pub target_y: f32,
    rng_state: u64,
}

impl BulletEmitter {
    pub fn new(x: f32, y: f32, pattern: PatternShape, fire_interval: u32) -> Self;
    pub fn with_bullet(self, lifetime: u32, radius: f32, damage: f32, kind: u32) -> Self;
    pub fn with_seed(self, seed: u64) -> Self;
    pub fn set_target(&mut self, x: f32, y: f32);
    /// Tick the emitter. If it fires, spawns bullets into the pool.
    /// Returns the number of bullets spawned.
    pub fn tick(&mut self, pool: &mut BulletPool) -> u32;
}
```

### PatternSequence (Multi-Phase Patterns)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceLoop {
    Once,
    Loop,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternPhase {
    pub emitters: Vec<BulletEmitter>,
    pub duration: u32,  // duration in ticks
}

impl PatternPhase {
    pub fn new(emitters: Vec<BulletEmitter>, duration: u32) -> Self;
}

#[derive(Clone, Debug)]
pub struct PatternSequence {
    pub phases: Vec<PatternPhase>,
    pub current_phase: usize,
    pub phase_timer: u32,
    pub loop_mode: SequenceLoop,
    pub finished: bool,
}

impl PatternSequence {
    pub fn new(phases: Vec<PatternPhase>, loop_mode: SequenceLoop) -> Self;
    /// Tick the sequence. Fires active emitters into the pool.
    /// Returns true if the phase changed.
    pub fn tick(&mut self, pool: &mut BulletPool) -> bool;
    pub fn is_finished(&self) -> bool;
}
```

## Behavior

### Object Pool Lifecycle

`BulletPool` pre-allocates a fixed number of `Bullet` slots at construction. Spawning scans for the first inactive slot (linear search), increments its `generation` counter, and activates it. Despawning marks a slot inactive (generation is NOT incremented on despawn — only on spawn). The generation counter allows external systems (e.g. shmup grazing) to distinguish newly spawned bullets from previous occupants of the same slot. This avoids per-frame heap allocations during gameplay.

On each `tick()`:

1. Each active bullet's position is updated by adding its velocity (`x += vx`, `y += vy`).
2. Lifetime is incremented; if it reaches `max_lifetime`, the bullet is deactivated and an `Expired` event is emitted.
3. Bounds checking deactivates bullets that exit the configured arena rectangle, emitting `OutOfBounds`.

### Collision Detection

`check_circle_hits(cx, cy, radius)` performs circle-vs-circle intersection tests against all active bullets. It uses squared-distance comparison (no `sqrt`) for performance. Returns the indices of all hitting bullets; the caller is responsible for calling `despawn()` on them.

### Pattern Computation

`compute_pattern()` is a pure function that returns a list of `(vx, vy)` velocity vectors based on the shape:

- **Radial**: Distributes `count` bullets evenly around a full circle (TAU / count step), offset by the emitter's current `rotation`.
- **Spiral**: Same as Radial, but the emitter accumulates `rotation_speed` after each firing, causing the pattern to rotate over time.
- **Aimed**: Centers the spread on `target_angle`. For a single bullet, fires directly at the target. For multiple bullets, distributes them evenly across `spread_angle`.
- **Wave**: Like Radial, but each bullet's angle is offset by a sine function of its base angle, producing wave-like distortion.
- **Random**: Uses a XorShift64 PRNG to generate random angles and speeds within `[min_speed, max_speed]`.

### Emitter Firing

`BulletEmitter::tick()` increments an internal timer. When `timer >= fire_interval`, it resets the timer and calls `compute_pattern()` to get velocities, then spawns bullets into the pool. For `Spiral` patterns, the emitter advances its `rotation` by `rotation_speed` after each firing.

### Pattern Sequencing

`PatternSequence` manages boss-fight-style multi-phase patterns. Each `PatternPhase` contains a set of emitters and a duration in ticks. On each `tick()`, the sequence fires all emitters in the current phase, then checks whether the phase duration has elapsed. On phase transition:

- `SequenceLoop::Once` stops after the last phase (sets `finished = true`).
- `SequenceLoop::Loop` wraps back to phase 0.

## Internal Design

### XorShift64 PRNG

Random patterns use a simple XorShift64 RNG (shifts: 13, 7, 17) that produces `f32` values in `[0.0, 1.0)` by masking 24 bits and dividing by `2^24`. The RNG state is stored per-emitter, allowing deterministic replay when seeded via `with_seed()`.

### Memory Layout

All bullets live in a contiguous `Vec<Bullet>`. The `active` flag determines whether a slot is in use. This provides cache-friendly iteration for the tick loop. The pool tracks `active_count` for quick capacity checks but does not use it for iteration (the loop checks each slot).

### Serialization

`PatternShape`, `BulletEmitter`, `PatternPhase`, and `SequenceLoop` all derive `Serialize`/`Deserialize`, enabling pattern definitions to be stored in RON/JSON data files. `BulletPool` and `PatternSequence` are runtime-only (not serialized).

## Non-Goals

- GPU-accelerated bullet simulation (runs on CPU only).
- Homing or curved bullet trajectories (bullets follow straight-line velocity).
- Bullet-to-bullet collision (only bullet-to-circle is supported).
- Visual rendering (the pool only tracks positions; rendering is handled by the sprite system).
- Networking synchronization (determinism is left to the caller to seed the PRNG).

## Open Questions

- Whether to add a `BulletKind` enum with typed variants rather than a `u32` tag.
- Whether the pool should dynamically grow or remain fixed-capacity.
- Whether to support acceleration / curved trajectories as a first-class pattern type.

## Referenzen

- [engine/core](core.md) -- Fixed-point types and ECS integration
- [engine/particles](particles.md) -- Visual effects for muzzle flashes and bullet trails
- [engine/simulation](simulation.md) -- Fixed timestep for deterministic spawning
- [gametypes/shmup](../gametypes/shmup.md) -- Primary consumer of bullet patterns
