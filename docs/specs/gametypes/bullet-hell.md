---
status: done
crate: amigo_core
depends_on: ["gametypes/shmup", "engine/bullet-patterns"]
last_updated: 2026-06-09
---

# Bullet Hell (Danmaku)

## Purpose

Authoring layer for bullet-hell games: how to compose `PatternShape`s, `BulletEmitter`s, and `PatternSequence`s into dense, readable danmaku, how grazing rewards risk-taking near bullets, and how rank-driven density scaling keeps patterns fair across skill levels. The shmup fundamentals (hitboxes, bombs, extends, scoring) are covered by [gametypes/shmup](shmup.md); the raw pool/emitter machinery by [engine/bullet-patterns](../engine/bullet-patterns.md). This spec focuses on pattern *authoring*, grazing, and density.

Examples: Touhou Project (spell-card phases, grazing), DoDonPachi (rank-scaled density), Mushihimesama (1000+ bullet walls), Crimzon Clover (spiral + aimed pattern layering).

Implementation: `crates/amigo_core/src/bullet_pattern.rs` (pool, shapes, emitters, sequences) and `crates/amigo_core/src/shmup.rs` (grazing, rank).

## Public API

### Pattern Authoring

```rust
/// Shape of a bullet pattern spawn (declarative, RON-serializable).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PatternShape {
    /// Evenly distributed bullets in a circle.
    Radial { count: u32, speed: f32 },
    /// Spiral pattern that rotates over time.
    /// `rotation_speed` = radians added to the emitter rotation per firing.
    Spiral { count: u32, speed: f32, rotation_speed: f32 },
    /// Aimed at a target with spread. `spread_angle` is the total fan
    /// width in radians; count == 1 fires a single aimed bullet.
    Aimed { count: u32, speed: f32, spread_angle: f32 },
    /// Sine wave pattern: each bullet's angle is perturbed by
    /// `sin(base_angle * frequency) * amplitude`.
    Wave { count: u32, speed: f32, amplitude: f32, frequency: f32 },
    /// Random directions and speeds (deterministic XorShift64 RNG).
    Random { count: u32, min_speed: f32, max_speed: f32 },
}

/// Compute bullet velocities for a pattern shape.
/// `rotation`: current emitter rotation (accumulates for Spiral).
/// `target_angle`: angle toward the target (used by Aimed).
/// `rng_state`: XorShift64 state for Random patterns.
pub fn compute_pattern(
    shape: &PatternShape,
    rotation: f32,
    target_angle: f32,
    rng_state: &mut u64,
) -> Vec<(f32, f32)>;
```

### BulletEmitter

```rust
/// A bullet emitter that fires its pattern every `fire_interval` ticks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BulletEmitter {
    pub x: f32,
    pub y: f32,
    pub pattern: PatternShape,
    pub fire_interval: u32,     // Ticks between firings.
    pub timer: u32,
    pub rotation: f32,          // Accumulates rotation_speed for Spiral.
    pub bullet_lifetime: u32,   // Default 300 ticks.
    pub bullet_radius: f32,     // Default 2.0.
    pub bullet_damage: f32,     // Default 1.0.
    pub bullet_kind: u32,       // Render/effect tag, default 0.
    pub active: bool,
    pub target_x: f32,          // Target position for Aimed patterns.
    pub target_y: f32,
}

impl BulletEmitter {
    pub fn new(x: f32, y: f32, pattern: PatternShape, fire_interval: u32) -> Self;
    /// Builder: bullet lifetime/radius/damage/kind in one call.
    pub fn with_bullet(self, lifetime: u32, radius: f32, damage: f32, kind: u32) -> Self;
    /// Builder: seed the internal RNG (Random patterns).
    pub fn with_seed(self, seed: u64) -> Self;
    /// Aim point for Aimed patterns (update with player position each tick).
    pub fn set_target(&mut self, x: f32, y: f32);
    /// Tick the emitter; spawns into the pool when the interval elapses.
    /// Returns the number of bullets spawned (0 if pool is full or not firing).
    pub fn tick(&mut self, pool: &mut BulletPool) -> u32;
}
```

### Boss Phase Sequencing

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceLoop { Once, Loop }

/// A phase in a pattern sequence — a set of emitters plus a duration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternPhase {
    pub emitters: Vec<BulletEmitter>,
    pub duration: u32,          // Ticks.
}

impl PatternPhase {
    pub fn new(emitters: Vec<BulletEmitter>, duration: u32) -> Self;
}

/// A sequence of pattern phases (boss spell cards).
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
    /// Tick all emitters in the current phase. Returns true on phase change.
    pub fn tick(&mut self, pool: &mut BulletPool) -> bool;
    pub fn is_finished(&self) -> bool;
}
```

### Grazing

```rust
/// Tracks grazing state and rewards. Dedup uses (pool_index, bullet.kind)
/// tuples since BulletPool slots are recycled (see Internal Design).
#[derive(Clone, Debug)]
pub struct GrazingSystem {
    pub frame_graze_count: u32,   // New grazes this frame.
    pub total_graze: u64,         // Total grazes this life.
    pub graze_score: u64,         // Score bonus per graze.
    pub graze_meter: f32,         // Meter gained per graze.
}

impl GrazingSystem {
    pub fn new(graze_score: u64, graze_meter: f32) -> Self;
    /// Test all active bullets against the player's graze hitbox
    /// (ShmupHitbox::graze_test). Returns new grazes this frame.
    pub fn tick(&mut self, player_x: f32, player_y: f32,
                hitbox: &ShmupHitbox, pool: &BulletPool) -> u32;
    /// Reset graze tracking (on death or new life).
    pub fn reset(&mut self);
}
```

### Density Scaling (Rank)

```rust
impl RankState {  // see gametypes/shmup for the full rank API
    /// Bullet speed multiplier: 0.8x at min rank to 1.3x at max rank.
    pub fn speed_multiplier(&self) -> f32;
    /// Bullet density multiplier: 0.7x at min rank to 1.5x at max rank.
    pub fn density_multiplier(&self) -> f32;
}
```

## Behavior

- **Authoring loop**: a pattern is `PatternShape` (geometry) + `BulletEmitter` (timing, bullet properties) + optionally `PatternPhase`/`PatternSequence` (choreography). Composite danmaku is built by running several emitters at once in a phase — e.g. a slow `Spiral { count: 6, rotation_speed: 0.07 }` curtain layered with a fast `Aimed { count: 3, spread_angle: 0.4 }` punisher that tracks the player via `set_target`.
- **Spiral rotation** accumulates on the *emitter* (`rotation += rotation_speed` per firing), so `compute_pattern` itself stays stateless; two emitters with opposite `rotation_speed` signs produce the classic interleaved double-spiral.
- **Determinism**: `Random` patterns use an XorShift64 state seeded via `with_seed`; given the same seed and tick sequence, bullet layouts replay exactly.
- **Grazing**: each tick, every active bullet inside the player's graze annulus (outside `collision_radius`, inside `graze_radius`) is counted once per life. New grazes feed `ShmupScoring::on_graze`, `RankState::on_graze`, and bomb/power meter via `graze_meter`.
- **Density**: games apply `RankState::density_multiplier()` when *constructing* emitters (scale `PatternShape` counts, or shorten `fire_interval`), and `speed_multiplier()` to pattern speeds. Patterns are rebuilt or rescaled on phase transitions, not mid-phase, to keep visual coherence.
- **Phase transitions**: `PatternSequence::tick` returns `true` on a phase change — the natural hook for clearing leftover bullets (`BulletPool::clear`), screen flash, and boss HP-bar segment updates. `SequenceLoop::Once` sequences report `is_finished()` for spell-card timeout logic.

## Internal Design

- `BulletPool` is a fixed-capacity object pool (no allocations after construction). `spawn` linear-scans for an inactive slot and returns `None` when full — emitters silently drop bullets rather than grow the pool, which caps worst-case cost. Size pools for the densest phase (4096+ for bullet hell).
- Bullets auto-despawn on `max_lifetime` expiry or when leaving the pool's bounds rectangle (`with_bounds`); both emit `BulletEvent`s for visual cleanup.
- `Bullet` has no generation counter; `GrazingSystem` dedups grazes with `(pool_index, bullet.kind)` keys, using `kind` as a generation stand-in. Recycled slots with the same `kind` are therefore *not* re-grazed within one life — acceptable for scoring, see Future Work.
- Collision against the player uses `BulletPool::check_circle_hits` (squared-distance, no sqrt) or `ShmupHitbox::hit_test` per bullet; both are O(active bullets) with no broad-phase, which is fine up to a few thousand bullets per frame.

## Future Work

- Per-slot generation counter on `Bullet` so graze dedup is exact under index recycling (the current `kind`-based key is an approximation).
- Curved/accelerating bullets: velocities are constant after spawn; homing, gravity, or angular-velocity bullets require per-bullet behavior data not yet in the pool.
- Data-driven pattern scripts (RON-loaded `PatternSequence` libraries) and a pattern preview tool.
- Built-in density scaling inside `BulletEmitter` (rank-aware count/interval) instead of game-side emitter rebuilding.

## Open Questions

- Should graze dedup be per-bullet-lifetime (cleared on bullet despawn) instead of per-player-life?
- Should `PatternPhase` support emitter position animation (moving emitters along splines) natively?
- Is a spatial hash for `check_circle_hits` worthwhile above ~4000 active bullets, or does the linear scan stay cache-friendly enough?

## Referenzen

- [gametypes/shmup](shmup.md) — `ShmupHitbox`, `RankState`, bombs, extends, chain scoring
- [engine/bullet-patterns](../engine/bullet-patterns.md) — `BulletPool`, `Bullet`, `BulletEvent` details
- [engine/particles](../engine/particles.md) — Graze sparkles, bullet despawn bursts
- [engine/timeline](../engine/timeline.md) — Stage choreography around pattern sequences
- Touhou Project — Spell-card phase structure, grazing rewards
- DoDonPachi — Rank-driven speed/density escalation
- Mushihimesama — Pool sizing for extreme bullet counts
