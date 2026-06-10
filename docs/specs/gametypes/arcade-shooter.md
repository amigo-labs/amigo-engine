---
status: done
crate: amigo_core
depends_on: ["gametypes/shmup"]
last_updated: 2026-06-09
---

# Arcade Shooter

## Purpose

Template for classic arcade-style shooters: lives and score-based extends, chained high-score play, screen-clearing bombs, and hand-authored enemy waves. Where [gametypes/shmup](shmup.md) covers the full bullet-hell stack (precision hitboxes, grazing, rank), this spec covers the *arcade loop*: wave in, kill chain up, bank score, earn extends, lose a life, repeat — in the vein of Galaga, Raiden, and Geometry Wars.

Examples: Galaga (fixed waves, lives, extends at score thresholds), Raiden (vertical scroller, bombs, chain bonuses), Geometry Wars (multi-directional arena, score multiplier chains), Sky Force (wave-scripted stages, scoring medals).

Implementation: `crates/amigo_core/src/shmup.rs` (config, lives/bombs, extends, scoring) and `crates/amigo_core/src/waves.rs` (wave definitions and spawner — shared with tower defense).

## Public API

### Stage Configuration

```rust
/// Scroll direction mode for a stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollMode {
    Vertical,          // Classic vertical scroller (Raiden).
    Horizontal,        // Gradius, R-Type.
    MultiDirectional,  // Player-controlled direction (Geometry Wars).
    FixedScreen,       // No scrolling, fixed arena (Asteroids, Galaga).
}

/// Global shmup/arcade-shooter configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShmupConfig {
    pub scroll_mode: ScrollMode,        // Default Vertical.
    pub scroll_speed: f32,              // Pixels/tick, default 1.0.
    pub player_speed: f32,              // Default 4.0.
    pub focus_speed: f32,               // Slow-move speed, default 1.5.
    pub starting_lives: u8,             // Default 3.
    pub max_bombs: u8,                  // Default 3.
    pub starting_bombs: u8,             // Default 3.
    pub arena: (f32, f32, f32, f32),    // Playfield rect, default 384x448.
}
```

### Waves

```rust
/// A group of enemies to spawn within a wave.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnGroup {
    pub enemy_type: u32,
    pub count: u32,
    pub spawn_interval: f32,    // Seconds between spawns within the group.
    pub spawn_point: usize,     // Index into WaveSpawner::spawn_points.
}

/// A single wave definition (builder API: with_group / with_delay /
/// with_announcement; total_enemies() for HUD).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveDef {
    pub groups: Vec<SpawnGroup>,
    pub start_delay: f32,
    pub announcement: Option<String>,   // "WAVE 3" banner text.
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavePhase { Waiting, Spawning, Active, Complete, Victory }

/// Drives wave progression and emits SpawnEvents.
pub struct WaveSpawner {
    pub waves: Vec<WaveDef>,
    pub spawn_points: Vec<RenderVec2>,
    pub current_wave: usize,
    pub phase: WavePhase,
    pub enemies_alive: u32,
    pub total_kills: u32,
}

impl WaveSpawner {
    pub fn new(waves: Vec<WaveDef>, spawn_points: Vec<RenderVec2>) -> Self;
    pub fn set_auto_advance(&mut self, auto: bool);
    pub fn start_next_wave(&mut self);
    /// Returns SpawnEvent { enemy_type, position, wave_index, group_index }.
    pub fn update(&mut self, dt: f32) -> Vec<SpawnEvent>;
    pub fn on_enemy_killed(&mut self);
    pub fn wave_number(&self) -> usize;
    pub fn total_waves(&self) -> usize;
    pub fn current_announcement(&self) -> Option<&str>;
    pub fn reset(&mut self);
}
```

### Scoring and Chains

```rust
/// Score tracking with chain/combo mechanics.
#[derive(Clone, Debug)]
pub struct ShmupScoring {
    pub score: u64,
    pub chain: u32,             // Consecutive kills without being hit.
    pub max_chain: u32,         // Best chain this run (for results screen).
    pub chain_multiplier: f32,  // 1.0 + chain * 0.1, capped at 10x.
    pub chain_timer: u16,       // Frames left before chain breaks.
    pub chain_timeout: u16,
}

impl ShmupScoring {
    pub fn new(chain_timeout: u16) -> Self;
    /// Register a kill. Returns score earned (base_score * multiplier).
    pub fn on_kill(&mut self, base_score: u64) -> u64;
    /// Add flat score (graze bonus, pickups) — no multiplier.
    pub fn on_graze(&mut self, graze_score: u64);
    /// Break the chain (on hit or timeout).
    pub fn break_chain(&mut self);
    /// Tick chain timer; breaks chain on timeout.
    pub fn tick(&mut self);
}
```

### Extends (Extra Lives)

```rust
/// Extra life system based on score thresholds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtendConfig {
    /// E.g. [1_000_000, 5_000_000, 15_000_000] for 3 extends.
    pub score_thresholds: Vec<u64>,
    /// After the last threshold, repeat the last interval forever.
    pub repeating: bool,
}

#[derive(Clone, Debug)]
pub struct ExtendState {
    pub config: ExtendConfig,
    pub next_threshold_index: usize,
    pub extends_awarded: u32,
}

impl ExtendState {
    pub fn new(config: ExtendConfig) -> Self;
    /// Check crossed thresholds. Returns number of new extends earned
    /// (can be >1 if a single bonus crosses multiple thresholds).
    pub fn check_score(&mut self, score: u64) -> u32;
}
```

### Bombs (Panic Button)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BombConfig {
    pub invincibility_frames: u16,  // Default 180.
    pub clear_duration: u8,         // Screen-clear effect, default 30.
    pub deathbomb_frames: u8,       // Post-hit bomb grace window, default 6.
    pub bomb_damage: f32,           // Damage to on-screen enemies, default 100.
}

#[derive(Clone, Debug)]
pub struct BombState {
    pub bombs: u8,
    pub invincibility_timer: u16,
    pub clearing: bool,
    pub clear_timer: u8,
    pub deathbomb_timer: u8,
}

impl BombState {
    pub fn new(starting_bombs: u8) -> Self;
    /// Use a bomb if available. Grants invincibility + screen clear.
    pub fn try_bomb(&mut self, config: &BombConfig) -> bool;
    /// On player hit: opens the deathbomb window if bombs remain.
    /// Returns true if the player actually dies.
    pub fn on_hit(&mut self, config: &BombConfig) -> bool;
    /// Tick timers. Returns true while the clear effect is active
    /// (caller despawns all enemy bullets).
    pub fn tick(&mut self) -> bool;
    pub fn is_invincible(&self) -> bool;
    /// Add bombs from pickups, clamped to max.
    pub fn add_bombs(&mut self, count: u8, max: u8);
}
```

## Behavior

- **Game loop**: per tick — `WaveSpawner::update` emits `SpawnEvent`s, the game spawns enemies at the indexed spawn points; on each enemy kill it calls `spawner.on_enemy_killed()`, `scoring.on_kill(base_score)`, and `extends.check_score(scoring.score)`; each earned extend increments lives (and typically plays the 1UP jingle).
- **Wave pacing**: waves auto-advance through `Waiting → Spawning → Active → Complete`; `Victory` after the last wave is the stage-clear hook (bonus tally: `max_chain`, remaining lives/bombs). Disabling `auto_advance` lets the game insert shops or boss intros between waves.
- **Chains**: every kill refreshes `chain_timer`; the multiplier grows linearly (`1.0 + chain * 0.1`, cap 10x). Getting hit calls `break_chain()` — risk/reward is keeping the chain alive across wave gaps within `chain_timeout` frames.
- **Lives and bombs**: a hit while not invincible calls `BombState::on_hit`. With bombs in stock the deathbomb window opens (`deathbomb_frames`); pressing bomb in time rescues the player via `try_bomb` (consumes the bomb, grants invincibility, cancels the death). Otherwise a life is lost, the chain breaks, and bombs typically refill to `starting_bombs`.
- **Extends**: `check_score` is monotonic over thresholds; with `repeating: true` the interval between the last two thresholds repeats forever (every-N-points extends, Galaga style).

## Internal Design

- All systems here are plain data + methods with no ECS or engine dependencies, so the score/extend/bomb loop is unit-testable headlessly (see tests in `shmup.rs`).
- `WaveSpawner` is shared verbatim with the tower-defense template ([gametypes/tower-defense](tower-defense.md)); only the consumer of `SpawnEvent` differs (arena enemies vs. path followers).
- `ExtendState::check_score` loops, so one large score award (boss bonus) can grant multiple extends in a single call.
- `ShmupScoring` multiplier and chain rules are fixed constants (0.1 growth, 10x cap); games wanting different curves wrap or fork the type.

## Future Work

- **Power-up system**: there is no power-up/pickup type in code yet (weapon levels, spread/laser swaps, magnet pickups). Currently games hand-roll pickups and call `BombState::add_bombs` / `ShmupScoring::on_graze` for bomb and score items. A `PowerUpDef`/`PowerUpState` pair is the main missing piece of this template.
- **High-score table persistence**: `ShmupScoring` tracks the run score only; name-entry tables and persistence should go through [engine/save-load](../engine/save-load.md).
- **Player weapon patterns**: player shot authoring (rate, spread, options) is not modeled; reuse `BulletEmitter` from [engine/bullet-patterns](../engine/bullet-patterns.md) with a player-owned pool.
- Configurable chain multiplier curve and per-enemy chain values.

## Open Questions

- Should extends and bomb refills be unified into a generic threshold-reward system?
- Should `WaveDef` gain formation data (entry splines per group) for Galaga-style choreographed entries?
- Per-wave score bonuses (no-miss, speed-kill) — template or game code?

## Referenzen

- [gametypes/shmup](shmup.md) — Hitboxes, grazing, rank; this spec's parent template
- [gametypes/tower-defense](tower-defense.md) — Shared `WaveSpawner`/`WaveDef`
- [engine/bullet-patterns](../engine/bullet-patterns.md) — Enemy and player bullet pools
- [engine/save-load](../engine/save-load.md) — High-score persistence
- Galaga — Fixed waves, threshold extends
- Raiden — Bombs, lives, chain bonuses
- Geometry Wars — Multi-directional arena, multiplier chains
