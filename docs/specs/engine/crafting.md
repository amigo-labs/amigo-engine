---
status: draft
crate: amigo_core
depends_on: ["engine/core", "engine/inventory"]
last_updated: 2026-03-18
---

# Crafting

## Purpose

Recipe-based crafting system for Sandbox games and optionally RPGs. Provides a data-driven recipe registry, material validation, crafting execution with inventory integration, station requirements, timed crafting jobs, recipe auto-discovery, and unlock conditions. Designed to be fully data-driven so that recipes can be defined as TOML/RON assets without code changes.

## Public API

Existing implementation in `crates/amigo_core/src/crafting.rs`.

### RecipeId & RecipeIngredient

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecipeId(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeIngredient {
    pub item_id: u32,
    pub count: u32,
}

impl RecipeIngredient {
    pub fn new(item_id: u32, count: u32) -> Self;
}
```

### UnlockCondition

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UnlockCondition {
    Always,
    HasItem(u32),
    PlayerLevel(u32),
    QuestFlag(String),
}
```

### Recipe

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: String,
    pub ingredients: Vec<RecipeIngredient>,
    pub results: Vec<RecipeIngredient>,
    pub crafting_time: f32,
    pub required_station: Option<u32>,
    pub category: String,
    pub unlock_condition: UnlockCondition,
}

impl Recipe {
    pub fn new(id: u32, name: impl Into<String>) -> Self;
    pub fn with_ingredient(mut self, item_id: u32, count: u32) -> Self;
    pub fn with_result(mut self, item_id: u32, count: u32) -> Self;
    pub fn with_time(mut self, seconds: f32) -> Self;
    pub fn with_station(mut self, station_id: u32) -> Self;
    pub fn with_category(mut self, category: impl Into<String>) -> Self;
    pub fn with_unlock(mut self, condition: UnlockCondition) -> Self;
}
```

### RecipeRegistry

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecipeRegistry {
    recipes: FxHashMap<RecipeId, Recipe>,
}

impl RecipeRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, recipe: Recipe);
    pub fn get(&self, id: RecipeId) -> Option<&Recipe>;
    pub fn by_category(&self, category: &str) -> Vec<&Recipe>;
    pub fn by_station(&self, station_id: u32) -> Vec<&Recipe>;
    pub fn portable(&self) -> Vec<&Recipe>;
    pub fn all(&self) -> impl Iterator<Item = &Recipe>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

### Crafting Functions

```rust
pub fn can_craft(recipe: &Recipe, inventory: &Inventory) -> bool;

pub fn craft(
    recipe: &Recipe,
    inventory: &mut Inventory,
    item_registry: &ItemRegistry,
) -> Result<Vec<ItemInstance>, CraftError>;

pub fn available_recipes<'a>(
    registry: &'a RecipeRegistry,
    inventory: &Inventory,
    station: Option<u32>,
) -> Vec<&'a Recipe>;

pub fn auto_discover(registry: &RecipeRegistry, inventory: &Inventory) -> Vec<RecipeId>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CraftError {
    RecipeNotFound,
    NotEnoughMaterials,
    WrongStation,
    RecipeLocked,
    InventoryFull,
}
```

### CraftingJob & CraftingState

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftingJob {
    pub recipe_id: RecipeId,
    pub progress: f32,
    pub duration: f32,
}

impl CraftingJob {
    pub fn new(recipe_id: RecipeId, duration: f32) -> Self;
    pub fn update(&mut self, dt: f32);
    pub fn is_complete(&self) -> bool;
    pub fn fraction(&self) -> f32;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CraftingState {
    pub unlocked_recipes: FxHashSet<RecipeId>,
    pub in_progress: Option<CraftingJob>,
    pub total_crafted: u32,
}

impl CraftingState {
    pub fn new() -> Self;
    pub fn unlock(&mut self, id: RecipeId);
    pub fn is_unlocked(&self, id: RecipeId) -> bool;
    pub fn start_crafting(&mut self, recipe: &Recipe) -> bool;
    pub fn update(&mut self, dt: f32) -> Option<RecipeId>;
    pub fn cancel(&mut self);
    pub fn discover_recipes(&mut self, registry: &RecipeRegistry, inventory: &Inventory) -> Vec<RecipeId>;
}
```

## Behavior

- **Validation:** `can_craft` checks that the [inventory](inventory.md) contains at least the required count of each ingredient. `available_recipes` additionally filters by station match (no-station recipes are always eligible).
- **Execution:** `craft()` validates materials, checks free slots for results, consumes ingredients via `Inventory::remove_by_id`, then produces result items via `Inventory::add`. On failure, the inventory is unchanged.
- **Timed crafting:** `CraftingState` supports a single active `CraftingJob`. Calling `update(dt)` advances the progress timer. When complete, it returns the `RecipeId` for the caller to execute via `craft()`. Only one job can be active at a time.
- **Auto-discovery:** `auto_discover` returns recipes where the player possesses at least one of every ingredient type, enabling a "discovered recipes" UI without explicit unlock actions.
- **Builder pattern:** `Recipe` uses a fluent builder (`with_ingredient`, `with_result`, `with_station`, etc.) for ergonomic construction.

## Internal Design

- `RecipeRegistry` uses `FxHashMap<RecipeId, Recipe>` for O(1) lookup by ID.
- Category and station filtering iterate all recipes (sufficient for typical recipe counts of 50-500).
- `CraftingState` is serializable for [save/load](save-load.md) integration.

## Non-Goals

- **Crafting UI.** Recipe list rendering, progress bars, and category tabs are game-layer responsibilities.
- **Multi-output crafting.** Each recipe has a single `results` list applied atomically; branching outputs (random results) are not supported.
- **Crafting queues.** Only one job at a time per `CraftingState`. Queue management is game-specific.

## Open Questions

- Should recipes support optional/alternative ingredients (e.g., any wood type)?
- Should `craft()` support batch crafting (craft N times in one call)?
- How should recipe data be loaded from asset files -- via a TOML loader in the asset pipeline or a custom format?
