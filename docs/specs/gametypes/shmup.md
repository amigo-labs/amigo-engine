---
status: done
crate: --
depends_on: ["engine/bullet-patterns"]
last_updated: 2026-03-18
---

# Shoot'em Up (Shmup)

## Purpose

Template for shoot'em up and bullet hell games with precision hitboxes, dense bullet patterns, adaptive difficulty (rank system), and scoring mechanics. Target games: Touhou Project, Ikaruga, DoDonPachi, Mushihimesama.

Shmups demand sub-pixel collision precision, massive bullet counts (1000+ active), and a tight feedback loop between player skill and difficulty. This template layers game-specific systems on top of the engine's `BulletPool`, `PatternShape`, and `BulletEmitter`.

## Public API

### ShmupConfig

```rust
/// Global shmup configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShmupConfig {
    /// Scroll mode for the stage.
    pub scroll_mode: ScrollMode,
    /// Scroll speed in pixels/tick.
    pub scroll_speed: I16F16,
    /// Player movement speed in pixels/tick.
    pub player_speed: I16F16,
    /// Focused (slow) movement speed (while holding focus button).
    pub focus_speed: I16F16,
    /// Number of starting lives.
    pub starting_lives: u8,
    /// Maximum number of bombs the player can hold.
    pub max_bombs: u8,
    /// Starting bomb count.
    pub starting_bombs: u8,
    /// Arena bounds (playfield rectangle).
    pub arena: (f32, f32, f32, f32),  // x, y, width, height
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollMode {
    /// Classic vertical scroller (Touhou, DoDonPachi).
    Vertical,
    /// Horizontal scroller (Gradius, R-Type).
    Horizontal,
    /// Player-controlled direction (Geometry Wars).
    MultiDirectional,
    /// No scrolling, fixed arena (Asteroids-style).
    FixedScreen,
}
```

### ShmupHitbox

```rust
/// Precision hitbox for shmup entities. Player hitboxes are much smaller than sprites.
#[derive(Clone, Debug)]
pub struct ShmupHitbox {
    /// Circle hitbox radius for collision with bullets/enemies.
    /// Player: typically 1-3 pixels. Enemies: match their visual size.
    pub collision_radius: f32,
    /// Graze detection radius. Bullets within this radius but outside
    /// collision_radius trigger graze events. Player only.
    pub graze_radius: f32,
    /// Center offset from the entity's position (usually (0, 0)).
    pub offset_x: f32,
    pub offset_y: f32,
}

impl ShmupHitbox {
    /// Create a player hitbox with tiny collision and larger graze radius.
    pub fn player(collision: f32, graze: f32) -> Self;

    /// Create an enemy hitbox (no graze radius).
    pub fn enemy(radius: f32) -> Self;

    /// Check if a point (bullet position) is within the collision circle.
    pub fn hit_test(&self, self_x: f32, self_y: f32, point_x: f32, point_y: f32) -> bool;

    /// Check if a point is within the graze circle but outside collision.
    pub fn graze_test(&self, self_x: f32, self_y: f32, point_x: f32, point_y: f32) -> bool;
}
```

### GrazingSystem

```rust
/// Tracks grazing state and rewards.
#[derive(Clone, Debug)]
pub struct GrazingSystem {
    /// Number of bullets grazed this frame.
    pub frame_graze_count: u32,
    /// Total bullets grazed this life.
    pub total_graze: u64,
    /// Score bonus per graze tick.
    pub graze_score: u64,
    /// Meter (power/bomb gauge) gained per graze.
    pub graze_meter: f32,
    /// Set of bullet indices already counted as grazed this life
    /// (prevents double-counting the same bullet across frames).
    grazed_bullets: FxHashSet<usize>,
}

impl GrazingSystem {
    pub fn new(graze_score: u64, graze_meter: f32) -> Self;

    /// Process grazing for one frame. Tests all active bullets against the
    /// player's graze hitbox. Returns the number of new grazes this frame.
    pub fn tick(
        &mut self,
        player_x: f32,
        player_y: f32,
        hitbox: &ShmupHitbox,
        pool: &BulletPool,
    ) -> u32;

    /// Reset graze tracking (on death or new life).
    pub fn reset(&mut self);
}
```

### RankSystem

```rust
/// Dynamic difficulty adjustment. Rank rises on skilled play, falls on death/bombing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RankConfig {
    /// Minimum rank value.
    pub min_rank: f32,           // default: 0.0
    /// Maximum rank value.
    pub max_rank: f32,           // default: 1.0
    /// Starting rank.
    pub initial_rank: f32,       // default: 0.3
    /// Rank increase per second of survival.
    pub survival_rate: f32,      // per tick, small value
    /// Rank increase per graze.
    pub graze_bonus: f32,
    /// Rank increase per enemy killed.
    pub kill_bonus: f32,
    /// Rank decrease on death.
    pub death_penalty: f32,
    /// Rank decrease on bomb use.
    pub bomb_penalty: f32,
    /// Rank increase on extend (extra life earned).
    pub extend_bonus: f32,
}

/// Runtime rank state.
#[derive(Clone, Debug)]
pub struct RankState {
    pub current_rank: f32,
    pub config: RankConfig,
}

impl RankState {
    pub fn new(config: RankConfig) -> Self;

    /// Tick survival time rank increase.
    pub fn tick_survival(&mut self);

    /// Apply a rank event.
    pub fn on_graze(&mut self);
    pub fn on_kill(&mut self);
    pub fn on_death(&mut self);
    pub fn on_bomb(&mut self);
    pub fn on_extend(&mut self);

    /// Get rank as a 0.0-1.0 normalized value for gameplay scaling.
    pub fn normalized(&self) -> f32;

    /// Get a bullet speed multiplier derived from current rank.
    /// At min rank: 0.8x. At max rank: 1.3x.
    pub fn speed_multiplier(&self) -> f32;

    /// Get a bullet density multiplier derived from current rank.
    /// At min rank: 0.7x. At max rank: 1.5x.
    pub fn density_multiplier(&self) -> f32;
}
```

### BombSystem

```rust
/// Bomb (panic button) configuration and state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BombConfig {
    /// Duration of bomb invincibility in frames.
    pub invincibility_frames: u16,   // default: 180 (3 seconds at 60fps)
    /// Duration of the screen-clear effect in frames.
    pub clear_duration: u8,          // default: 30
    /// Deathbomb window: frames after being hit where bomb input is still accepted.
    pub deathbomb_frames: u8,        // default: 6
    /// Damage dealt to on-screen enemies during bomb.
    pub bomb_damage: f32,
}

#[derive(Clone, Debug)]
pub struct BombState {
    /// Current bomb count.
    pub bombs: u8,
    /// Remaining invincibility frames (>0 means active).
    pub invincibility_timer: u16,
    /// Whether the bomb is currently clearing bullets.
    pub clearing: bool,
    pub clear_timer: u8,
    /// Deathbomb window timer. Set when hit, counts down.
    pub deathbomb_timer: u8,
}

impl BombState {
    pub fn new(starting_bombs: u8) -> Self;

    /// Attempt to use a bomb. Returns true if bomb was activated.
    pub fn try_bomb(&mut self, config: &BombConfig) -> bool;

    /// Called when the player is hit. Starts the deathbomb window.
    /// Returns true if the player actually dies (no deathbomb available).
    pub fn on_hit(&mut self, config: &BombConfig) -> bool;

    /// Tick bomb timers. Returns true if the clear effect is active
    /// (caller should despawn all enemy bullets).
    pub fn tick(&mut self) -> bool;

    /// Whether the player is currently invincible.
    pub fn is_invincible(&self) -> bool;

    /// Add bombs (from pickups).
    pub fn add_bombs(&mut self, count: u8, max: u8);
}
```

### ExtendSystem

```rust
/// Extra life system based on score thresholds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtendConfig {
    /// Score thresholds at which extra lives are awarded.
    /// E.g., [1_000_000, 5_000_000, 15_000_000] for 3 extends.
    pub score_thresholds: Vec<u64>,
    /// If true, after the last threshold, repeat the last interval forever.
    pub repeating: bool,
}

#[derive(Clone, Debug)]
pub struct ExtendState {
    pub config: ExtendConfig,
    /// Index of the next threshold to check.
    pub next_threshold_index: usize,
    /// Number of extends awarded so far.
    pub extends_awarded: u32,
}

impl ExtendState {
    pub fn new(config: ExtendConfig) -> Self;

    /// Check if the current score has crossed the next extend threshold.
    /// Returns the number of new extends earned (usually 0 or 1).
    pub fn check_score(&mut self, score: u64) -> u32;
}
```

### ShmupScoring

```rust
/// Score tracking with chain/combo mechanics.
#[derive(Clone, Debug)]
pub struct ShmupScoring {
    /// Current score.
    pub score: u64,
    /// Current chain (consecutive kills without being hit).
    pub chain: u32,
    /// Maximum chain achieved this run.
    pub max_chain: u32,
    /// Score multiplier from chain (grows with chain length).
    pub chain_multiplier: f32,
    /// Frames remaining in chain window. Resets on kill, depletes over time.
    pub chain_timer: u16,
    /// Frames for chain timeout.
    pub chain_timeout: u16,         // default: 120 (2 seconds)
}

impl ShmupScoring {
    pub fn new(chain_timeout: u16) -> Self;

    /// Register an enemy kill. Returns score earned (base_score * multiplier).
    pub fn on_kill(&mut self, base_score: u64) -> u64;

    /// Register a graze. Adds graze_score directly.
    pub fn on_graze(&mut self, graze_score: u64);

    /// Break the chain (on hit or timeout).
    pub fn break_chain(&mut self);

    /// Tick chain timer. Breaks chain if timeout.
    pub fn tick(&mut self);
}
```

## Behavior

### Hitbox Precision

The player's collision hitbox is a circle with radius 1-3 pixels, centered on the player sprite (often at a visible "core" or "orb"). The hitbox is rendered in debug mode and optionally shown when the focus button is held (common Touhou convention). All collision detection uses circle-vs-circle with squared-distance comparison (no sqrt) via `BulletPool::check_circle_hits`.

The graze radius is typically 10-20 pixels, creating a risk-reward zone. Bullets that enter the graze ring but miss the collision hitbox contribute to the graze counter.

### Grazing Mechanics

Each active bullet is tested against the graze radius every frame. To prevent counting the same bullet multiple times as it passes through the graze zone, `GrazingSystem` maintains a set of already-grazed bullet indices.

**Index Recycling Safety:** Since `BulletPool` recycles indices when bullets expire or are despawned, stale entries in `grazed_bullets` could prevent newly-spawned bullets from being counted. Solution: `grazed_bullets` stores `(usize, u32)` tuples — the pool index AND the bullet's generation counter. `BulletPool` increments a per-slot generation counter on each spawn. Graze lookup checks both index and generation, so recycled indices are treated as new bullets.

```rust
grazed_bullets: FxHashSet<(usize, u32)>,  // (pool_index, generation)
```

The grazed set is cleared on death (resetting graze tracking for the new life).

Graze rewards:
- Score: `graze_score` per new graze (feeds into extend thresholds).
- Meter: `graze_meter` accumulates toward a bomb or power gauge.

### Rank System (Dynamic Difficulty)

Rank is a floating-point value clamped between `min_rank` and `max_rank`. It starts at `initial_rank` (typically 0.3 = 30%) and adjusts based on player performance:

- **Increases**: survival time (constant drip), grazing (small per-graze), killing (small per-kill), earning extends (moderate).
- **Decreases**: death (large penalty), bomb use (moderate penalty).

Rank influences gameplay through multipliers:
- `speed_multiplier()`: bullet velocities are scaled. Linearly interpolated from 0.8x at min to 1.3x at max.
- `density_multiplier()`: bullet count per pattern is scaled. Radial(count: 16) at rank 0.5 fires `16 * density_multiplier(0.5)` bullets, rounded to nearest integer.

Emitters read these multipliers from `RankState` when firing. This creates a feedback loop: skilled players face harder patterns, while struggling players get relief.

### Bomb and Deathbomb

Pressing the bomb button (if `bombs > 0`) activates the bomb:
1. All enemy bullets on screen are despawned.
2. `bomb_damage` is dealt to all on-screen enemies.
3. Player becomes invincible for `invincibility_frames`.
4. Bomb count decrements by 1.
5. Rank decreases by `bomb_penalty`.

**Deathbomb**: when the player's collision hitbox overlaps a bullet, `on_hit()` is called. Instead of immediate death, a `deathbomb_timer` starts counting down from `deathbomb_frames` (typically 6 frames = 100ms). If the player presses bomb during this window, the bomb activates and the death is prevented. If the timer expires without a bomb, the player dies.

### Extend System

Score thresholds for extra lives are defined in `ExtendConfig`. As the player's score increases, `check_score()` compares against the next threshold. When crossed, an extend is awarded (life +1), the extend index advances, and rank increases by `extend_bonus`. If `repeating` is true, after the last defined threshold, the interval between the last two thresholds repeats indefinitely.

### Scroll Modes

- **Vertical**: the background scrolls downward at `scroll_speed`. Enemy spawn positions are defined relative to the top of the screen and scroll into view. The player is confined to the lower portion of the arena.
- **Horizontal**: background scrolls left. Enemies spawn from the right. Player is confined to the left half.
- **MultiDirectional**: no fixed scroll direction. The player can move freely and the camera follows. Enemies spawn from all edges.
- **FixedScreen**: no scrolling. The arena is a fixed rectangle. Wave-based enemy spawning from edges.

### Collision Processing Order

Each frame processes in this order:
1. Player input -> player movement (clamped to arena bounds).
2. Enemy movement and pattern firing (emitters tick into bullet pool).
3. Bullet pool tick (move all bullets, expire old ones, bounds-check).
4. Player-vs-bullet collision: check collision hitbox against all active bullets.
5. If hit: start deathbomb window or process death.
6. Graze detection: check graze hitbox against all active bullets.
7. Player-bullet-vs-enemy collision: player shots against enemy hitboxes.
8. Scoring and rank updates.

## Internal Design

### Bullet Pool Scaling

The shmup template configures `BulletPool` with large capacities (4096-8192 for enemy bullets, 256-512 for player bullets). Two separate pools are used: one for enemy bullets (harms player) and one for player bullets (harms enemies). This avoids the need for faction tags on individual bullets.

### Rank-Scaled Patterns

Emitters do not directly reference `RankState`. Instead, the spawning code reads `rank.speed_multiplier()` and `rank.density_multiplier()` and adjusts the emitter's parameters before firing. This keeps the bullet pattern system rank-agnostic and reusable outside shmups.

```
let speed_mult = rank.speed_multiplier();
let density_mult = rank.density_multiplier();
emitter.pattern = match &base_pattern {
    PatternShape::Radial { count, speed } => PatternShape::Radial {
        count: (*count as f32 * density_mult) as u32,
        speed: speed * speed_mult,
    },
    // ... similar for other shapes
};
```

### Enemy Movement Patterns

Enemies follow scripted movement paths, not AI navigation. Each enemy has an `EnemyPath` component:

```rust
#[derive(Clone, Debug)]
pub enum EnemyPath {
    /// Move in a straight line at constant speed (basic fodder).
    Linear { velocity: RenderVec2 },
    /// Follow a predefined spline path (from engine/spline).
    Spline { path: CatmullRom, speed: f32, progress: f32 },
    /// Sinusoidal wave pattern (horizontal + vertical oscillation).
    Wave { base_velocity: RenderVec2, amplitude: f32, frequency: f32, phase: f32 },
    /// Enter screen, stop at a position, fire patterns, then exit.
    StopAndShoot { enter_pos: RenderVec2, stop_pos: RenderVec2, exit_pos: RenderVec2,
                   enter_speed: f32, exit_speed: f32, stop_duration: u32 },
    /// Boss: phase-based movement with position targets per phase.
    BossPhase { positions: Vec<RenderVec2>, current_phase: usize },
}
```

Enemy paths are defined in stage data (RON) and interpreted by the enemy movement system each frame. Enemies use `f32` positions (same as bullets) since they are visual entities, not gameplay-deterministic.

### Fixed-Point Collision

Player position and movement use `SimVec2` (I16F16 fixed-point) for deterministic replay. Bullet and enemy positions are `f32` for throughput (bulk iteration over thousands of entities). Conversion happens once per frame at the collision boundary:

```rust
// Conversion happens ONCE per frame, not per-bullet:
let player_f32_x = player_pos.x.to_f32();
let player_f32_y = player_pos.y.to_f32();
// Then all hit_test() and graze_test() calls use these f32 values.
```

Precision loss at typical arena sizes (480x270) is negligible — I16F16 has ~0.00002 precision, f32 has ~0.00006 at these magnitudes. Both are sub-pixel.

### Particle Integration

Bullet despawn events (from bombs, screen clear, or lifetime expiry) trigger particle bursts from the CPU particle system. Graze events trigger small sparkle particles at the graze point. These are purely visual and do not affect gameplay.

## Non-Goals

- **3D shmup mechanics.** This template is strictly 2D. Depth layers or pseudo-3D bullet patterns are not supported.
- **Color-polarity systems.** Ikaruga-style color switching (absorb same-color bullets) is game-specific logic not included in the template.
- **Replay recording.** Deterministic replay requires input recording and the state rewind system, which is separate from this template.
- **Online leaderboards.** Score submission and retrieval require a network backend not provided by the engine.
- **Stage scripting.** Enemy wave timing and stage choreography require a timeline/scripting system. This template provides the mechanical systems, not the stage director.

## Open Questions

- Should the rank system support per-boss rank snapshots (reset rank to a fixed value for boss fights)?
- Should the graze set use a bitfield instead of a hash set for better cache performance at high bullet counts?
- Should there be a "practice mode" that disables rank scaling for pattern learning?
- How should the focus button interact with player shot patterns (focused narrow shot vs. wide shot)?
- Should extends also refill bombs, or are bombs and lives tracked independently?

## Referenzen

- [engine/bullet-patterns](../engine/bullet-patterns.md) -- BulletPool, PatternShape, BulletEmitter, PatternSequence
- [engine/physics](../engine/physics.md) -- Circle collision for hitbox tests
- [engine/particles](../engine/particles.md) -- Visual effects for bullet despawn, graze sparkles
- [engine/animation](../engine/animation.md) -- Player ship animation, enemy animation
- [engine/camera](../engine/camera.md) -- ScreenLock for fixed-arena shmups
- Touhou Project -- Grazing, rank system, deathbomb, extend thresholds
- DoDonPachi -- Dense bullet patterns, chain scoring, rank escalation
- Ikaruga -- Polarity mechanics (reference for future extension)
