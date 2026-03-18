---
status: spec
crate: --
depends_on: ["engine/procedural", "engine/save-load"]
last_updated: 2026-03-18
---

# Roguelike

## Purpose

Template for roguelike and roguelite games with procedural dungeon generation, permadeath, run-based progression, and meta-progression between deaths. Target games: Hades, Dead Cells, Enter the Gungeon, Slay the Spire.

Provides the core systems that define the roguelike loop: start run -> explore procedural floors -> collect items -> escalate difficulty -> die -> unlock permanent upgrades -> start again.

## Public API

### RunConfig

```rust
/// Configuration for a roguelike run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    /// Permadeath mode.
    pub death_mode: DeathMode,
    /// Number of floors before the final boss.
    pub floor_count: u8,
    /// Starting seed. If None, a random seed is generated.
    pub seed: Option<u64>,
    /// Starting items granted at run start (unlocked via meta-progression).
    pub starting_items: Vec<String>,
    /// Base player stats before item modifiers.
    pub base_stats: PlayerStats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathMode {
    /// Full reset: all run progress lost on death.
    Hard,
    /// Partial reset: keep some currency or items on death.
    Soft { currency_retain_pct: u8 },
}
```

### RunManager

```rust
/// Manages the state of a single roguelike run.
#[derive(Clone, Debug)]
pub struct RunManager {
    pub seed: u64,
    pub current_floor: u8,
    pub config: RunConfig,
    pub stats: PlayerStats,
    pub inventory: Vec<Item>,
    pub run_stats: RunStats,
    pub rng: XorShift64,
    pub floor_cleared: bool,
    pub run_active: bool,
}

impl RunManager {
    /// Start a new run with the given configuration.
    pub fn new(config: RunConfig) -> Self;

    /// Advance to the next floor. Triggers dungeon generation.
    /// Returns the floor number entered, or None if the run is complete.
    pub fn advance_floor(&mut self) -> Option<u8>;

    /// Mark the current floor as cleared (boss or exit reached).
    pub fn clear_floor(&mut self);

    /// Record player death. Returns RunStats for the post-mortem screen.
    pub fn die(&mut self, cause: DeathCause) -> RunStats;

    /// Add an item to the player's inventory. Applies stat modifiers.
    pub fn add_item(&mut self, item: Item);

    /// Remove an item (dropped, consumed, or cursed-item removal).
    pub fn remove_item(&mut self, item_id: &str) -> Option<Item>;

    /// Get the current effective stats (base + all item modifiers).
    pub fn effective_stats(&self) -> PlayerStats;

    /// Peek at the next N random values without advancing the RNG
    /// (used for item preview in shops).
    pub fn peek_rng(&self, count: usize) -> Vec<u64>;

    /// Check if current floor is a boss floor.
    pub fn is_boss_floor(&self) -> bool;

    /// Get a seeded sub-RNG for dungeon generation on the current floor.
    pub fn floor_rng(&self) -> XorShift64;
}
```

### PlayerStats

```rust
/// Player stats that can be modified by items and meta-progression.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: I16F16,
    pub crit_chance: I16F16,     // 0.0 to 1.0
    pub crit_multiplier: I16F16, // default: 1.5
    pub luck: i32,               // affects item rarity rolls
}

impl PlayerStats {
    /// Apply a stat modifier (additive).
    pub fn apply_modifier(&mut self, modifier: &StatModifier);
    /// Remove a stat modifier.
    pub fn remove_modifier(&mut self, modifier: &StatModifier);
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatModifier {
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: I16F16,
    pub crit_chance: I16F16,
    pub crit_multiplier: I16F16,
    pub luck: i32,
}
```

### Item System

```rust
/// Rarity tiers for items. Higher rarity = rarer drops, stronger effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

/// An item definition loaded from RON data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rarity: Rarity,
    pub sprite: String,
    /// Stat modifiers applied while the item is held.
    pub modifiers: StatModifier,
    /// Tags used for synergy matching (e.g., "fire", "poison", "speed").
    pub tags: Vec<String>,
    /// Whether this item is cursed (has negative effects alongside positive).
    pub cursed: bool,
    /// If true, the item is consumed on use rather than held passively.
    pub consumable: bool,
}

/// A concrete item instance in the player's inventory.
#[derive(Clone, Debug)]
pub struct Item {
    pub def: ItemDef,
    /// Unique instance ID for removal tracking.
    pub instance_id: u32,
    /// Stacks remaining (for consumables).
    pub stacks: u8,
}

/// Synergy bonus activated when the player holds items with matching tags.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SynergyDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Tags required. All must be present in the player's item tags.
    pub required_tags: Vec<String>,
    /// Minimum number of items with matching tags needed.
    pub min_items: u8,
    /// Bonus stat modifiers when synergy is active.
    pub bonus: StatModifier,
}

/// Manages item drops, rarity rolls, and synergy checking.
pub struct ItemSystem {
    items: Vec<ItemDef>,
    synergies: Vec<SynergyDef>,
    next_instance_id: u32,
}

impl ItemSystem {
    /// Load item and synergy definitions from RON data.
    pub fn from_ron(items_path: &str, synergies_path: &str, assets: &AssetManager) -> Self;

    /// Roll a random item drop at the given rarity distribution.
    /// `luck` affects the chance of higher rarities.
    pub fn roll_drop(&self, rng: &mut XorShift64, luck: i32) -> ItemDef;

    /// Roll N items for a shop or treasure room.
    pub fn roll_shop(&self, rng: &mut XorShift64, count: usize, luck: i32) -> Vec<ItemDef>;

    /// Check which synergies are active given the player's current items.
    pub fn active_synergies(&self, inventory: &[Item]) -> Vec<&SynergyDef>;

    /// Instantiate an item definition into a concrete item with a unique ID.
    pub fn instantiate(&mut self, def: ItemDef) -> Item;
}
```

### Dungeon Generator

```rust
/// Configuration for procedural dungeon generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonConfig {
    /// Minimum number of rooms per floor.
    pub min_rooms: u8,
    /// Maximum number of rooms per floor.
    pub max_rooms: u8,
    /// Room size range (tiles).
    pub room_min_size: (u16, u16),
    pub room_max_size: (u16, u16),
    /// Corridor width in tiles.
    pub corridor_width: u8,
    /// Chance (0.0-1.0) of a room being a special room (shop, treasure, etc.).
    pub special_room_chance: f32,
}

/// A generated room within a dungeon floor.
#[derive(Clone, Debug)]
pub struct DungeonRoom {
    pub id: u16,
    pub rect: (i32, i32, u16, u16),   // x, y, width, height in tiles
    pub room_type: RoomType,
    pub connections: Vec<u16>,          // IDs of connected rooms
    pub cleared: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomType {
    Start,
    Normal,
    Elite,
    Shop,
    Treasure,
    Boss,
    Secret,
}

/// Result of dungeon generation.
#[derive(Clone, Debug)]
pub struct DungeonFloor {
    pub rooms: Vec<DungeonRoom>,
    pub corridors: Vec<(u16, u16)>,    // pairs of connected room IDs
    pub tilemap_width: u32,
    pub tilemap_height: u32,
    pub tile_data: Vec<u8>,            // tilemap indices
    pub start_room_id: u16,
    pub boss_room_id: u16,
}

pub struct DungeonGenerator;

impl DungeonGenerator {
    /// Generate a dungeon floor from the given seed and config.
    /// The floor number affects difficulty scaling (more rooms, more elites).
    pub fn generate(
        seed: u64,
        floor: u8,
        config: &DungeonConfig,
    ) -> DungeonFloor;
}
```

### Floor Escalation

```rust
/// Difficulty scaling parameters per floor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FloorEscalation {
    /// Multiplier to enemy HP per floor (compounding). E.g., 1.15 = +15% per floor.
    pub hp_multiplier: f32,
    /// Multiplier to enemy damage per floor.
    pub damage_multiplier: f32,
    /// Additional enemy count per room per floor.
    pub extra_enemies_per_floor: u8,
    /// Floors on which boss encounters occur (e.g., [5, 10, 15]).
    pub boss_floors: Vec<u8>,
    /// Floor at which elite enemies start appearing.
    pub elite_start_floor: u8,
    /// Chance of an elite spawn per room, increasing per floor.
    pub elite_chance_per_floor: f32,
}

impl FloorEscalation {
    /// Get the effective HP multiplier for a given floor number.
    pub fn hp_at_floor(&self, floor: u8) -> f32;
    /// Get the effective damage multiplier for a given floor number.
    pub fn damage_at_floor(&self, floor: u8) -> f32;
    /// Check if the given floor is a boss floor.
    pub fn is_boss_floor(&self, floor: u8) -> bool;
    /// Get the elite spawn chance for the given floor.
    pub fn elite_chance_at_floor(&self, floor: u8) -> f32;
}
```

### Meta-Progression

```rust
/// Permanent unlock state that survives death. Persisted via SaveManager.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MetaProgression {
    /// Permanent currencies accumulated across runs.
    pub currencies: FxHashMap<String, u64>,
    /// IDs of permanently unlocked items (added to the drop pool).
    pub unlocked_items: Vec<String>,
    /// IDs of permanently unlocked abilities.
    pub unlocked_abilities: Vec<String>,
    /// Permanent stat bonuses purchased at the hub.
    pub permanent_upgrades: Vec<String>,
    /// Total number of runs attempted.
    pub total_runs: u32,
    /// Total number of runs that reached the final boss.
    pub total_completions: u32,
    /// Best run stats for the records screen.
    pub best_run: Option<RunStats>,
}

/// Manages loading, saving, and modifying meta-progression.
pub struct MetaManager {
    progression: MetaProgression,
    save_manager: SaveManager,
}

impl MetaManager {
    /// Load meta-progression from the save system.
    pub fn load(save_manager: SaveManager) -> Self;

    /// Persist current meta-progression to disk.
    pub fn save(&self) -> Result<(), SaveError>;

    /// Add currency earned from a run (respects DeathMode retention).
    pub fn add_currency(&mut self, currency: &str, amount: u64);

    /// Attempt to purchase an unlock. Returns false if insufficient currency.
    pub fn purchase_unlock(&mut self, unlock_id: &str, cost: u64, currency: &str) -> bool;

    /// Check if an item is unlocked (and thus eligible for drop rolls).
    pub fn is_unlocked(&self, item_id: &str) -> bool;

    /// Record the end of a run into the meta-progression stats.
    pub fn record_run(&mut self, stats: &RunStats);
}
```

### RunStats

```rust
/// Statistics for a completed (or ended) run. Displayed on the post-mortem screen.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunStats {
    pub seed: u64,
    pub floor_reached: u8,
    pub death_cause: Option<DeathCause>,
    pub items_collected: Vec<String>,
    pub enemies_killed: u32,
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub time_played_secs: f64,
    pub currency_earned: FxHashMap<String, u64>,
    pub synergies_activated: Vec<String>,
    pub completed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeathCause {
    Enemy { enemy_id: String },
    Boss { boss_id: String },
    Trap { trap_type: String },
    CursedItem { item_id: String },
    Environmental,
}
```

## Behavior

### Run Lifecycle

1. **Run Start**: `RunManager::new()` initializes a fresh run. A seed is either provided or generated from system entropy. The seed is split into per-floor sub-seeds via `seed ^ floor_number` so that floor generation is independent.
2. **Floor Entry**: `advance_floor()` increments `current_floor` and returns the new floor number. The caller uses `floor_rng()` to generate the dungeon via `DungeonGenerator::generate()`.
3. **Exploration**: The player clears rooms. Enemy stats are scaled by `FloorEscalation`. Items are rolled by `ItemSystem` using the run's RNG.
4. **Floor Clear**: `clear_floor()` marks the floor as complete, allowing `advance_floor()` to proceed.
5. **Death**: `die()` records the death cause, finalizes `RunStats`, and marks `run_active = false`. Meta-currencies are calculated and handed to `MetaManager`.
6. **Completion**: If the player defeats the final boss, the run is marked `completed` in `RunStats`.

### Seed Determinism

The run seed generates a `XorShift64` PRNG. All randomness within a run (dungeon layout, item drops, enemy spawns) derives from this single seed, making runs fully reproducible. Each floor uses `seed ^ (floor as u64)` to derive its sub-seed, ensuring that floor N is always the same regardless of how many items were rolled on previous floors.

The RNG state is part of `RunManager` and must be included in save state for mid-run saves (soft permadeath mode).

### Item Rarity Rolls

`ItemSystem::roll_drop()` uses weighted random selection. Base weights: Common=50, Uncommon=30, Rare=15, Epic=4, Legendary=1. The `luck` stat shifts weight from Common toward higher tiers: each point of luck transfers 1 weight from Common to Uncommon, 0.5 to Rare, etc. The final weights are normalized and a uniform roll selects the tier. Within a tier, a uniform roll picks the specific item.

### Synergy Detection

`active_synergies()` iterates all synergy definitions. For each synergy, it counts how many items in the inventory have tags matching `required_tags`. If the count meets `min_items`, the synergy is active. Active synergies contribute their `bonus` `StatModifier` to the effective stats calculation.

Synergies are recalculated whenever an item is added or removed. The cost is O(synergies * inventory_size), acceptable for typical inventory sizes (<30 items).

### Dungeon Generation

`DungeonGenerator::generate()` uses the Room-and-Corridor algorithm:

1. Generate `N` random room rectangles (within size bounds) placed without overlap using rejection sampling.
2. Build a Delaunay triangulation of room centers, then compute the minimum spanning tree for guaranteed connectivity.
3. Re-add a percentage of non-MST edges for loops (configurable per floor for variety).
4. Carve corridors between connected rooms using L-shaped paths.
5. Assign room types: start room is the one closest to center, boss room is furthest from start. Special rooms (shop, treasure) are assigned randomly with `special_room_chance`.

The resulting `DungeonFloor` contains a tilemap (`tile_data`) and a graph of rooms for gameplay logic.

**Tile Mapping to Tilemap CollisionLayer:** `DungeonFloor.tile_data` uses semantic indices that map to `CollisionType`:

| tile_data value | Semantic | CollisionType |
|----------------|----------|---------------|
| 0 | Wall | `Solid` |
| 1 | Floor | `Empty` |
| 2 | Corridor | `Empty` |
| 3 | Door (closed) | `Solid` → `Empty` on interaction |
| 4 | Door (open) | `Empty` |
| 5 | Trap | `Trigger { id }` |
| 6 | Pit | `Empty` (visual hazard, damage via TriggerZone) |

`DungeonFloor::to_collision_layer()` converts `tile_data` to a `CollisionLayer` for the tilemap system. The visual tileset (which sprite per tile) is resolved separately by the renderer using tile_data + world theme.

### Floor Escalation

Enemy stats on floor N are: `base_stat * multiplier^(N-1)`. Boss floors (defined in `FloorEscalation.boss_floors`) guarantee a boss room. Elite enemies begin appearing after `elite_start_floor`, with spawn chance growing linearly by `elite_chance_per_floor` per floor.

## Internal Design

### RNG Architecture

All randomness flows from a single `XorShift64` seed. The RNG uses the same XorShift64 implementation as the bullet pattern system (shifts: 13, 7, 17).

**Fork Semantics:** Sub-RNGs for specific purposes (floor gen, item rolls, enemy spawns) are created by seeding a new `XorShift64` with a derived seed — NOT by splitting the parent's state:

```rust
impl XorShift64 {
    /// Create a child RNG with a deterministic seed derived from the parent.
    /// Uses the parent's current state XORed with a domain tag to ensure
    /// different sub-systems get different sequences.
    pub fn fork(&self, domain: u64) -> XorShift64 {
        XorShift64::new(self.state ^ domain)
    }
}

// Usage:
let floor_rng = run_rng.fork(floor_number as u64);           // floor generation
let item_rng = run_rng.fork(0xITEM_0000 | floor as u64);     // item drops
let spawn_rng = run_rng.fork(0xSPAWN_000 | floor as u64);    // enemy spawns
```

Forking does NOT advance the parent RNG's state. This means floor N's generation is always deterministic regardless of how many items were rolled on floor N-1. The `domain` tag prevents collisions between sub-systems.

### Save Integration

Meta-progression is saved via `SaveManager` in a dedicated slot (slot ID = `max_slots + autosave_slots + 1`). Mid-run state (for soft permadeath) is saved as a quicksave. On run completion or death, the quicksave is deleted to enforce permadeath.

### AI Integration

Enemy AI uses a unified architecture with complexity tiers:

| Enemy Type | AI System | Details |
|-----------|-----------|---------|
| **Normal** | Utility AI (from [engine/agents](../engine/agents.md)) | 2-3 needs: aggression (chase player), self-preservation (flee at low HP), patrol (wander in room). Simple scoring. |
| **Elite** | Utility AI + extended needs | Additional needs: "use special attack at range", "dodge projectiles", "retreat and heal". Higher-weight scoring functions. |
| **Boss** | FSM (from [engine/agents](../engine/agents.md) StateMachine) | Phase-based: each HP threshold triggers a state transition. States: `Idle`, `Attack(pattern_id)`, `Enrage`, `Summon`, `Vulnerable`. Transitions defined in boss RON data. |

All enemy types use A* pathfinding (from [engine/pathfinding](../engine/pathfinding.md)) for navigation within dungeon rooms. Pathfinding is re-requested when the player moves >3 tiles from the enemy's last-known player position (stored in `Memory` from agents module).

There is no mixing of architectures within a single enemy — an enemy is either Utility AI OR FSM, never both.

## Non-Goals

- **Turn-based combat.** This template assumes real-time action combat. Turn-based roguelikes (Slay the Spire, traditional roguelikes) would need a separate turn system.
- **Overworld map.** A node-based map screen (Slay the Spire style) is not included; floors are entered sequentially.
- **Crafting.** Item combination or crafting is not part of this template. Use the engine crafting system separately if needed.
- **Multiplayer co-op.** Run state is single-player only.
- **Narrative branching.** Story integration (Hades-style) requires the dialogue system and is not covered here.

## Open Questions

- Should there be a "daily challenge" mode with a fixed seed derived from the date?
- How should mid-run saving work in hard permadeath mode -- disallow it, or delete the save on load (save-and-quit pattern)?
- Should the dungeon generator support alternative algorithms (BSP, cellular automata) as config options?
- Should cursed items be removable at shops, or are they permanent for the run?
- How deep should the meta-progression tree go before it trivializes the game?

## Referenzen

- [engine/procedural](../engine/procedural.md) -- Dungeon generation algorithms
- [engine/save-load](../engine/save-load.md) -- Meta-progression persistence, mid-run saves
- [engine/agents](../engine/agents.md) -- Utility AI for enemy behavior, FSM for bosses
- [engine/achievements](../engine/achievements.md) -- Run milestones and unlock triggers
- Hades -- Meta-progression, real-time roguelite loop, death as narrative
- Dead Cells -- Roguelite with escalating difficulty, permanent unlocks
- Enter the Gungeon -- Item synergies, bullet-hell-meets-roguelike
