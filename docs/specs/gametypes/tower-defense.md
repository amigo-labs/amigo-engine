---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/pathfinding"]
last_updated: 2026-06-09
---

# Tower Defense

## Purpose

Template for tower defense games: place towers on a grid, enemies march along a waypoint path in timed waves, towers auto-target and fire projectiles, kills award bounty, leaks cost lives. Covers targeting strategies, tiered upgrades, placement validation, wave choreography, and a complete pre-orchestrated game tick.

Examples: Kingdom Rush (tiered tower upgrades, wave announcements), Bloons TD (targeting strategy selection per tower), Defense Grid (path-progress "First" targeting), Dungeon Warfare (grid placement on/off the path).

Implementation: `crates/amigo_core/src/tower.rs`, `td_systems.rs`, `waves.rs`. Collaborators from other modules: `EnemyManager`/`EnemyDef` (`enemy.rs`), `ProjectileManager` (`projectile.rs`), `TdGameState` (`game_state.rs`), `WaypointPath` (`pathfinding.rs`).

## Public API

### Targeting

```rust
/// How a tower selects its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetingStrategy {
    Nearest,      // Closest enemy.
    First,        // Enemy furthest along the path (closest to exit).
    Strongest,    // Highest current HP.
    Weakest,      // Lowest current HP.
    MostDamaged,  // Lowest HP percentage.
    Random,       // Deterministic pseudo-random pick (seeded).
}

/// Candidate for targeting (built per tower per fire attempt).
pub struct TargetCandidate {
    pub entity: EntityId,
    pub position: RenderVec2,
    pub distance: f32,
    pub health: i32,
    pub max_health: i32,
    pub path_progress: f32,
}

/// Select the best target from candidates using the given strategy.
/// `seed` drives the deterministic Random strategy.
pub fn select_target(
    candidates: &[TargetCandidate],
    strategy: TargetingStrategy,
    seed: u64,
) -> Option<EntityId>;
```

### Tower Definitions

```rust
/// Attack type for a tower.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TowerAttackType {
    SingleTarget,             // Single target projectile.
    Splash { radius: f32 },   // Area of effect around impact.
    Beam,                     // Continuous beam (instant-speed projectile).
    Aura { radius: f32 },     // Slow/debuff aura, no projectile.
}

/// Tower tier/upgrade level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerTier {
    pub damage: i32,
    pub range: f32,
    pub attack_speed: f32,    // Attacks per second.
    pub cost: u32,
    pub attack_type: TowerAttackType,
    pub sprite_name: String,
}

/// A tower definition with upgrade tiers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerDef {
    pub id: u32,
    pub name: String,
    pub tiers: Vec<TowerTier>,
    pub targeting: TargetingStrategy,
}

impl TowerDef {
    pub fn max_tier(&self) -> usize;
    /// Tier at the given level, clamped to max tier.
    pub fn tier(&self, level: usize) -> &TowerTier;
}
```

### TowerInstance

```rust
/// Runtime state for a placed tower.
#[derive(Clone, Debug)]
pub struct TowerInstance {
    pub def_id: u32,
    pub position: RenderVec2,
    pub current_tier: usize,
    pub targeting: TargetingStrategy,
    pub attack_cooldown: f32,       // 1.0 / attack_speed.
    pub cooldown_timer: f32,
    pub current_target: Option<EntityId>,
    pub total_damage_dealt: u64,    // Lifetime stats for UI.
    pub total_kills: u32,
    pub enabled: bool,
}

impl TowerInstance {
    pub fn new(def_id: u32, position: RenderVec2, tier: &TowerTier,
               targeting: TargetingStrategy) -> Self;
    /// Update cooldown. Returns true if the tower is ready to fire.
    pub fn update(&mut self, dt: f32) -> bool;
    /// Fire the tower (reset cooldown).
    pub fn fire(&mut self);
    /// Upgrade to next tier. Returns the cost, or None if max tier.
    pub fn upgrade(&mut self, def: &TowerDef) -> Option<u32>;
    pub fn can_upgrade(&self, def: &TowerDef) -> bool;
    /// Sell value: 70% of the total invested cost across purchased tiers.
    pub fn sell_value(&self, def: &TowerDef) -> u32;
}
```

### PlacementGrid

```rust
/// Grid for tower placement validation. Cells are buildable or blocked
/// (path, obstacle); at most one tower per cell.
pub struct PlacementGrid {
    pub width: u32,
    pub height: u32,
    pub tile_size: f32,
}

impl PlacementGrid {
    pub fn new(width: u32, height: u32, tile_size: f32) -> Self;
    pub fn set_blocked(&mut self, x: u32, y: u32);
    pub fn set_buildable(&mut self, x: u32, y: u32);
    /// In bounds, buildable, and unoccupied.
    pub fn can_place(&self, x: u32, y: u32) -> bool;
    /// Place a tower on a cell. Returns false if invalid.
    pub fn place(&mut self, x: u32, y: u32, tower: EntityId) -> bool;
    pub fn remove(&mut self, x: u32, y: u32);
    pub fn world_to_grid(&self, pos: RenderVec2) -> (u32, u32);
    /// Grid cell to world center position.
    pub fn grid_to_world(&self, x: u32, y: u32) -> RenderVec2;
}
```

### Waves

```rust
/// A group of enemies to spawn within a wave.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnGroup {
    pub enemy_type: u32,        // Game-defined enemy id.
    pub count: u32,
    pub spawn_interval: f32,    // Seconds between spawns.
    pub spawn_point: usize,     // Index into WaveSpawner::spawn_points.
}

/// A single wave definition (builder API).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaveDef {
    pub groups: Vec<SpawnGroup>,
    pub start_delay: f32,                 // Delay before wave starts (default 3.0).
    pub announcement: Option<String>,
}

impl WaveDef {
    pub fn new() -> Self;
    pub fn with_group(self, enemy_type: u32, count: u32, interval: f32, spawn_point: usize) -> Self;
    pub fn with_delay(self, delay: f32) -> Self;
    pub fn with_announcement(self, text: impl Into<String>) -> Self;
    pub fn total_enemies(&self) -> u32;
}

/// Event emitted when the spawner wants to create an enemy.
pub struct SpawnEvent {
    pub enemy_type: u32,
    pub position: RenderVec2,
    pub wave_index: usize,
    pub group_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WavePhase {
    Waiting,    // Pre-wave delay.
    Spawning,   // Actively emitting SpawnEvents.
    Active,     // All spawned, waiting for kills.
    Complete,   // Wave cleared.
    Victory,    // All waves finished.
}

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
    /// Auto-advance to the next wave when all enemies are dead (default true).
    pub fn set_auto_advance(&mut self, auto: bool);
    pub fn start_next_wave(&mut self);
    /// Update the spawner. Returns spawn events for this tick.
    pub fn update(&mut self, dt: f32) -> Vec<SpawnEvent>;
    /// Call when an enemy dies (drives Active -> Complete transition).
    pub fn on_enemy_killed(&mut self);
    pub fn wave_number(&self) -> usize;        // 1-indexed for display.
    pub fn total_waves(&self) -> usize;
    pub fn current_announcement(&self) -> Option<&str>;
    pub fn reset(&mut self);
}
```

### Systems (td_systems.rs)

```rust
/// Scan enemies in range and fire projectiles from ready towers.
pub fn tower_fire_system(
    towers: &mut [(EntityId, TowerInstance)], tower_defs: &[TowerDef],
    enemies: &EnemyManager, projectiles: &mut ProjectileManager,
    dt: f32, tick: u64);

/// Apply projectile hits to enemies; credits damage/kills to owning towers.
pub fn apply_hits_system(hits: &[ProjectileHit], enemies: &mut EnemyManager,
                         towers: &mut [(EntityId, TowerInstance)]);

/// Process wave spawn events into enemy instances. Returns spawned ids.
pub fn spawn_enemies_system(events: &[SpawnEvent], enemy_defs: &[EnemyDef],
                            enemies: &mut EnemyManager) -> Vec<EntityId>;

/// Move enemies along the path, process status effects.
pub fn update_enemies_system(enemies: &mut EnemyManager, path: &WaypointPath,
                             dt: f32) -> EnemyUpdateResult;

pub struct EnemyUpdateResult { pub leaked: Vec<EntityId> }

/// Award bounties for kills, apply leak damage for leakers.
pub fn process_dead_enemies(dead: &[DeadEnemy], game_state: &mut TdGameState);

/// Run one complete tower defense game tick. The main game loop body.
pub fn td_tick(
    game_state: &mut TdGameState, towers: &mut Vec<(EntityId, TowerInstance)>,
    tower_defs: &[TowerDef], enemy_defs: &[EnemyDef], enemies: &mut EnemyManager,
    projectiles: &mut ProjectileManager, path: &WaypointPath, dt: f32);
```

## Behavior

- **Tick order** (`td_tick`): game state update (phase/defeat check, early-out when paused/over) → wave spawner update → enemy movement → tower targeting/firing → projectile update and hit detection → damage application → dead-enemy cleanup with bounty/leak processing → projectile cleanup. `dt` is scaled by `TdGameState::speed_multiplier` for fast-forward.
- **Targeting**: each ready tower builds a `TargetCandidate` list of alive enemies within the current tier's `range` (path progress = `segment + progress` from the enemy's path follower) and calls `select_target`. The `Random` strategy is seeded with `tick + tower_id.index()` so replays stay deterministic.
- **Attack types**: `SingleTarget` spawns a `Physical` projectile, `Splash` a `Fire` projectile with `AoeShape::Circle` on hit, `Beam` an instant-speed (`9999.0`) `Lightning` projectile. `Aura` towers spawn nothing — effect application is left to a game-provided aura system.
- **Waves**: `WaveSpawner` runs the phase machine `Waiting → Spawning → Active → Complete → (next wave | Victory)`. Each `SpawnGroup` keeps its own interval timer; groups within a wave spawn in parallel. With `auto_advance` off, the game calls `start_next_wave()` manually (call-next-wave-early button).
- **Economy & lives**: kills route through `TdGameState::on_enemy_killed` (gold bounty + score), leaks through `on_enemy_leaked` (lives cost). `TowerInstance::sell_value` returns 70% of the summed tier costs.

## Internal Design

- All TD math runs in `f32`/`RenderVec2` — determinism is achieved by seeded xorshift selection and fixed `dt`, not fixed-point. Enemy path movement uses fixed-point internally via `WaypointPath`/path follower.
- `PlacementGrid` stores two flat row-major `Vec`s (`cells: Vec<bool>`, `occupied: Vec<Option<EntityId>>`). Path cells are marked blocked at level load.
- Towers are plain data in a `Vec<(EntityId, TowerInstance)>` rather than ECS components; systems are free functions over slices, so the whole pipeline is unit-testable headlessly (see tests in `td_systems.rs`).
- `tower_fire_system` looks up `TowerDef` by linear scan over `def_id` — fine for the typical <32 tower types.

## Future Work

- Aura attack type currently spawns nothing; a dedicated `aura_system` applying slows/debuffs to enemies in radius is not yet implemented.
- Beam attacks are modeled as instant projectiles rather than true continuous beams (no sustained damage ticks or beam rendering data).
- No targeting-strategy UI cycling helper or per-tower strategy override persistence.
- No interest/income-per-wave economy hooks (flat bounty only).

## Open Questions

- Should `TargetCandidate` lists be built via a broad-phase grid instead of an O(towers × enemies) scan for very large maps?
- Should `WaveDef` support nested sub-waves or scripted boss waves (cf. timeline system)?
- Should sell refund (70%) be configurable per `TowerDef`?

## Referenzen

- [engine/pathfinding](../engine/pathfinding.md) — `WaypointPath` and path followers for enemy movement
- [engine/core](../engine/core.md) — `EntityId`, math types
- [gametypes/rts](rts.md) — Shares economy and unit-management patterns
- Kingdom Rush — Tiered upgrades, wave announcements
- Bloons TD 6 — Targeting strategies, sell refunds
- Defense Grid: The Awakening — Path-progress targeting
