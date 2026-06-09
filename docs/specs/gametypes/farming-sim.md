---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/crafting", "engine/save-load"]
last_updated: 2026-06-09
---

# Farming Sim

## Purpose

Farming and life-sim games built around a calendar: till soil, plant seasonal crops, water them daily, harvest, craft goods at stations, and manage money. Time advances in ticks through days, seasons, and years; crops grow in stages and wither without water.

Examples: Stardew Valley (seasonal crops, watering, crafting stations), Harvest Moon (day/season calendar), Animal Crossing (real-time daily loop), Graveyard Keeper (timed crafting + economy).

Backed by `amigo_core::farming`, `amigo_core::crafting`, `amigo_core::economy`, and `amigo_core::scheduler`.

## Public API

### Calendar (`amigo_core::farming`)

```rust
pub enum Season { Spring, Summer, Autumn, Winter }
impl Season { pub fn next(self) -> Self; pub fn index(self) -> u32; }

/// Tracks day/season/year from simulation ticks.
pub struct Calendar {
    pub ticks_per_day: u32, pub days_per_season: u32,
    pub day: u32, pub season: Season, pub year: u32,
}
impl Calendar {
    pub fn new(ticks_per_day: u32, days_per_season: u32) -> Self;
    pub fn day_progress(&self) -> f32;  // 0.0..1.0 time of day
    pub fn hour(&self) -> u32;          // 0..23, derived from day_progress
    pub fn total_days(&self) -> u32;
    /// Advance one tick; emits DayChanged / SeasonChanged / YearChanged.
    pub fn tick(&mut self) -> Vec<CalendarEvent>;
}
pub enum CalendarEvent {
    DayChanged { day: u32, season: Season, year: u32 },
    SeasonChanged { season: Season, year: u32 },
    YearChanged { year: u32 },
}
```

### Crop Growth (`amigo_core::farming`)

```rust
/// A growth stage: duration in ticks, optional water requirement.
pub struct GrowthStage { pub duration: u32, pub needs_water: bool }
// Builder: GrowthStage::new(duration).with_water()

/// Definition of anything that grows (crop, tree, animal).
pub struct GrowthDef {
    pub id: u32, pub name: String,
    pub stages: Vec<GrowthStage>,
    pub allowed_seasons: Vec<Season>, // empty = all seasons
}
// Builder: GrowthDef::new(id, name, stages).with_seasons(vec![...]); total_duration()

/// A live, planted instance.
pub struct GrowthInstance {
    pub id: u32, pub def_id: u32,
    pub current_stage: u32, pub ticks_in_stage: u32,
    pub watered: bool, pub withered: bool,
    pub wither_threshold: u32, pub dry_ticks: u32, // ticks without water before withering (0 = never)
}
impl GrowthInstance {
    pub fn new(id: u32, def_id: u32) -> Self;
    pub fn with_wither_threshold(self, ticks: u32) -> Self;
    pub fn water(&mut self);
    pub fn is_complete(&self, def: &GrowthDef) -> bool;
}

/// Tick all instances: enforces season windows and water requirements,
/// emits StageAdvanced / Completed / Withered events.
pub fn tick_growth(instances: &mut [GrowthInstance], defs: &[GrowthDef],
                   current_season: Season) -> Vec<GrowthEvent>;
```

### Farm Grid (`amigo_core::farming`)

```rust
pub enum SoilState { Empty, Tilled, Planted { growth_id: u32 } }
pub struct FarmTile { pub soil: SoilState, pub moisture: f32, pub fertility: f32 }

/// Grid of interactable soil tiles with moisture evaporation.
pub struct FarmGrid { pub width: u32, pub height: u32, pub moisture_decay: f32 }
impl FarmGrid {
    pub fn new(width: u32, height: u32) -> Self;
    pub fn till(&mut self, x: u32, y: u32) -> Option<FarmEvent>;     // Empty -> Tilled
    pub fn water(&mut self, x: u32, y: u32, amount: f32) -> Option<FarmEvent>;
    pub fn plant(&mut self, x: u32, y: u32, growth_id: u32) -> Option<FarmEvent>; // Tilled -> Planted
    pub fn harvest(&mut self, x: u32, y: u32) -> Option<FarmEvent>;  // Planted -> Tilled
    pub fn tick_moisture(&mut self);                                 // evaporation
    pub fn get(&self, x: u32, y: u32) -> Option<&FarmTile>;
}
```

### Crafting (`amigo_core::crafting`)

```rust
/// Recipe: ingredients -> results, with timing, station, category, unlock condition.
pub struct Recipe {
    pub id: RecipeId, pub name: String,
    pub ingredients: Vec<RecipeIngredient>,  // { item_id, count }
    pub results: Vec<RecipeIngredient>,
    pub crafting_time: f32,                  // seconds, 0 = instant
    pub required_station: Option<u32>,       // None = craft anywhere
    pub category: String,
    pub unlock_condition: UnlockCondition,   // Always | HasItem | PlayerLevel | QuestFlag
}
// Builder: Recipe::new(id, name).with_ingredient(..).with_result(..)
//          .with_time(..).with_station(..).with_category(..).with_unlock(..)

pub struct RecipeRegistry { /* register/get/by_category/by_station/portable/all */ }

pub fn can_craft(recipe: &Recipe, inventory: &Inventory) -> bool;
pub fn craft(recipe: &Recipe, inventory: &mut Inventory, item_registry: &ItemRegistry)
    -> Result<Vec<ItemInstance>, CraftError>; // NotEnoughMaterials | InventoryFull | ...
pub fn available_recipes<'a>(registry: &'a RecipeRegistry, inventory: &Inventory,
                             station: Option<u32>) -> Vec<&'a Recipe>;

/// Persistent crafting state: unlocked recipes, one timed job in progress.
pub struct CraftingState { pub unlocked_recipes: FxHashSet<RecipeId>,
                           pub in_progress: Option<CraftingJob>, pub total_crafted: u32 }
impl CraftingState {
    pub fn start_crafting(&mut self, recipe: &Recipe) -> bool;
    pub fn update(&mut self, dt: f32) -> Option<RecipeId>; // Some when job completes
    pub fn discover_recipes(&mut self, registry: &RecipeRegistry, inventory: &Inventory) -> Vec<RecipeId>;
}
```

### Money (`amigo_core::economy`)

```rust
/// Gold/lives/score wallet with full transaction history.
pub struct Economy { pub gold: i32, pub score: u64, /* lives fields unused here */ }
impl Economy {
    pub fn new(starting_gold: i32, starting_lives: i32) -> Self;
    pub fn add_gold(&mut self, amount: i32, kind: TransactionKind) -> i32;   // selling crops
    pub fn try_spend(&mut self, cost: i32, kind: TransactionKind) -> bool;   // buying seeds
    pub fn can_afford(&self, cost: i32) -> bool;
    pub fn history(&self) -> &[Transaction];  // ledger UI
}
// TransactionKind::Custom { tag } covers shop purchases/sales; ItemDef.value gives base prices.
```

### Tick Scheduling (`amigo_core::scheduler`)

```rust
/// Tick-based scheduler: register intervals, poll per tick, dispatch externally.
/// Used to run subsystems at different cadences (moisture decay every tick,
/// growth every N ticks, shop restock once per in-game day).
pub struct TickScheduler { /* entries keyed by CallbackId */ }
impl TickScheduler {
    pub fn every(&mut self, interval: u64, id: CallbackId); // panics if interval == 0
    pub fn should_run(&mut self, id: CallbackId, current_tick: u64) -> bool;
    pub fn remove(&mut self, id: CallbackId) -> bool;
}
```

## Behavior

- **Daily loop**: Each sim tick, the game calls `Calendar::tick()` and uses `TickScheduler::should_run()` to dispatch subsystems. On `DayChanged`, crops are advanced (`tick_growth`), tiles dry out (`tick_moisture`), and the player wakes. On `SeasonChanged`, out-of-season crops simply stop growing (the game may choose to wither them).
- **Crop pipeline**: `FarmGrid::till` → `plant(x, y, growth_id)` links a tile to a `GrowthInstance`. Watering calls both `FarmGrid::water` (moisture visual) and `GrowthInstance::water` (growth gate). When `tick_growth` emits `Completed`, the crop is harvestable; `FarmGrid::harvest` returns the tile to `Tilled` and the game adds produce via `Inventory::add` and sells it through `Economy::add_gold`.
- **Crafting**: At a station (`required_station` matches the station the player stands at, via trigger zone), `available_recipes()` populates the menu. Timed recipes run through `CraftingState::start_crafting` + `update(dt)`; ingredients are consumed and results produced by `craft()`. `discover_recipes()` auto-unlocks recipes once the player has touched each ingredient type.
- **Persistence**: `Calendar`, `GrowthInstance`s, `FarmGrid`, `CraftingState`, and `Economy` are all `Serialize`/`Deserialize` and stored in the save slot.

## Future Work

- **NPC daily schedules**: `amigo_core::scheduler` is a tick-interval scheduler, not an NPC routine system. Stardew-style "NPC is at the shop 9:00-17:00" schedules must be built in game code from `Calendar::hour()` plus `engine/agents`/`engine/pathfinding`.
- No weather system (rain auto-watering), no fertilizer effects (`FarmTile.fertility` is stored but unused by `tick_growth`), no animal husbandry, no relationship/gifting system (see `amigo_core::dialog` flags and `AgentMemory.relationships` as building blocks).
- No dynamic pricing — `Economy` has no supply/demand model; prices come from `ItemDef.value`.

## Referenzen

- [engine/crafting](../engine/crafting.md) — crafting UI/UX conventions
- [engine/save-load](../engine/save-load.md) — save-game persistence of calendar and farm state
- [engine/simulation](../engine/simulation.md) — fixed-tick runner driving `Calendar::tick()`
- [engine/agents](../engine/agents.md) — building block for villager AI
- Stardew Valley — seasons, watering, crafting stations, shipping economy
- Harvest Moon — stage-based crop growth and withering
