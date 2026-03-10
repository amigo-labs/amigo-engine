use crate::inventory::{Inventory, ItemRegistry};
use crate::loot::ItemInstance;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Recipe definitions
// ---------------------------------------------------------------------------

/// Unique recipe identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecipeId(pub u32);

/// A single ingredient or output item.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeIngredient {
    pub item_id: u32,
    pub count: u32,
}

impl RecipeIngredient {
    pub fn new(item_id: u32, count: u32) -> Self {
        Self { item_id, count }
    }
}

/// Condition for a recipe to be unlockable/visible.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UnlockCondition {
    /// Always available.
    Always,
    /// Player must possess this item (not consumed).
    HasItem(u32),
    /// Player must be at least this level.
    PlayerLevel(u32),
    /// A quest/dialog flag must be set.
    QuestFlag(String),
}

/// A crafting recipe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: String,
    pub ingredients: Vec<RecipeIngredient>,
    pub results: Vec<RecipeIngredient>,
    /// Crafting time in seconds (0 = instant).
    pub crafting_time: f32,
    /// Required crafting station ID (None = craft anywhere).
    pub required_station: Option<u32>,
    /// Category for UI filtering.
    pub category: String,
    /// Unlock condition.
    pub unlock_condition: UnlockCondition,
}

impl Recipe {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id: RecipeId(id),
            name: name.into(),
            ingredients: Vec::new(),
            results: Vec::new(),
            crafting_time: 0.0,
            required_station: None,
            category: "General".to_string(),
            unlock_condition: UnlockCondition::Always,
        }
    }

    pub fn with_ingredient(mut self, item_id: u32, count: u32) -> Self {
        self.ingredients.push(RecipeIngredient::new(item_id, count));
        self
    }

    pub fn with_result(mut self, item_id: u32, count: u32) -> Self {
        self.results.push(RecipeIngredient::new(item_id, count));
        self
    }

    pub fn with_time(mut self, seconds: f32) -> Self {
        self.crafting_time = seconds;
        self
    }

    pub fn with_station(mut self, station_id: u32) -> Self {
        self.required_station = Some(station_id);
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    pub fn with_unlock(mut self, condition: UnlockCondition) -> Self {
        self.unlock_condition = condition;
        self
    }
}

// ---------------------------------------------------------------------------
// Recipe registry
// ---------------------------------------------------------------------------

/// Central registry for all crafting recipes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecipeRegistry {
    recipes: FxHashMap<RecipeId, Recipe>,
}

impl RecipeRegistry {
    pub fn new() -> Self {
        Self { recipes: FxHashMap::default() }
    }

    pub fn register(&mut self, recipe: Recipe) {
        self.recipes.insert(recipe.id, recipe);
    }

    pub fn get(&self, id: RecipeId) -> Option<&Recipe> {
        self.recipes.get(&id)
    }

    /// Get all recipes in a category.
    pub fn by_category(&self, category: &str) -> Vec<&Recipe> {
        self.recipes.values()
            .filter(|r| r.category == category)
            .collect()
    }

    /// Get all recipes that require a specific station.
    pub fn by_station(&self, station_id: u32) -> Vec<&Recipe> {
        self.recipes.values()
            .filter(|r| r.required_station == Some(station_id))
            .collect()
    }

    /// Get recipes craftable without a station.
    pub fn portable(&self) -> Vec<&Recipe> {
        self.recipes.values()
            .filter(|r| r.required_station.is_none())
            .collect()
    }

    /// Get all recipes.
    pub fn all(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }

    /// Count of registered recipes.
    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Crafting validation and execution
// ---------------------------------------------------------------------------

/// Error when crafting fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CraftError {
    RecipeNotFound,
    NotEnoughMaterials,
    WrongStation,
    RecipeLocked,
    InventoryFull,
}

/// Check if an inventory has enough materials for a recipe.
pub fn can_craft(recipe: &Recipe, inventory: &Inventory) -> bool {
    recipe.ingredients.iter().all(|ing| inventory.has(ing.item_id, ing.count))
}

/// Execute a crafting recipe: consume ingredients and produce results.
pub fn craft(
    recipe: &Recipe,
    inventory: &mut Inventory,
    item_registry: &ItemRegistry,
) -> Result<Vec<ItemInstance>, CraftError> {
    // Validate materials
    if !can_craft(recipe, inventory) {
        return Err(CraftError::NotEnoughMaterials);
    }

    // Check if we have room for results
    let free = inventory.free_slots();
    let needed = recipe.results.len(); // worst case: each result needs a slot
    if free < needed {
        return Err(CraftError::InventoryFull);
    }

    // Consume ingredients
    for ing in &recipe.ingredients {
        inventory.remove_by_id(ing.item_id, ing.count);
    }

    // Produce results
    let mut produced = Vec::new();
    for result in &recipe.results {
        let item = ItemInstance::with_stack(result.item_id, result.count);
        produced.push(item.clone());
        inventory.add(item, item_registry);
    }

    Ok(produced)
}

/// Find recipes that can be crafted right now with current inventory.
pub fn available_recipes<'a>(
    registry: &'a RecipeRegistry,
    inventory: &Inventory,
    station: Option<u32>,
) -> Vec<&'a Recipe> {
    registry.all()
        .filter(|r| {
            // Station check
            match (r.required_station, station) {
                (Some(required), Some(current)) => required == current,
                (Some(_), None) => false,
                (None, _) => true,
            }
        })
        .filter(|r| can_craft(r, inventory))
        .collect()
}

/// Discover recipes whose ingredients the player has (even if not enough quantity).
pub fn auto_discover(registry: &RecipeRegistry, inventory: &Inventory) -> Vec<RecipeId> {
    registry.all()
        .filter(|r| {
            // Player has at least 1 of each ingredient type
            r.ingredients.iter().all(|ing| inventory.count(ing.item_id) > 0)
        })
        .map(|r| r.id)
        .collect()
}

// ---------------------------------------------------------------------------
// Crafting state (runtime, saveable)
// ---------------------------------------------------------------------------

/// A crafting job in progress.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftingJob {
    pub recipe_id: RecipeId,
    pub progress: f32,
    pub duration: f32,
}

impl CraftingJob {
    pub fn new(recipe_id: RecipeId, duration: f32) -> Self {
        Self {
            recipe_id,
            progress: 0.0,
            duration,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.progress = (self.progress + dt).min(self.duration);
    }

    pub fn is_complete(&self) -> bool {
        self.progress >= self.duration
    }

    pub fn fraction(&self) -> f32 {
        if self.duration <= 0.0 { 1.0 } else { self.progress / self.duration }
    }
}

/// Persistent crafting state (unlocked recipes, active job).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CraftingState {
    pub unlocked_recipes: FxHashSet<RecipeId>,
    pub in_progress: Option<CraftingJob>,
    /// Total items crafted (for stats/achievements).
    pub total_crafted: u32,
}

impl CraftingState {
    pub fn new() -> Self {
        Self {
            unlocked_recipes: FxHashSet::default(),
            in_progress: None,
            total_crafted: 0,
        }
    }

    pub fn unlock(&mut self, id: RecipeId) {
        self.unlocked_recipes.insert(id);
    }

    pub fn is_unlocked(&self, id: RecipeId) -> bool {
        self.unlocked_recipes.contains(&id)
    }

    /// Start a crafting job.
    pub fn start_crafting(&mut self, recipe: &Recipe) -> bool {
        if self.in_progress.is_some() {
            return false;
        }
        self.in_progress = Some(CraftingJob::new(recipe.id, recipe.crafting_time));
        true
    }

    /// Update the active crafting job. Returns recipe ID if just completed.
    pub fn update(&mut self, dt: f32) -> Option<RecipeId> {
        if let Some(job) = &mut self.in_progress {
            job.update(dt);
            if job.is_complete() {
                let id = job.recipe_id;
                self.in_progress = None;
                self.total_crafted += 1;
                return Some(id);
            }
        }
        None
    }

    /// Cancel the current crafting job.
    pub fn cancel(&mut self) {
        self.in_progress = None;
    }

    /// Discover and unlock recipes based on inventory contents.
    pub fn discover_recipes(&mut self, registry: &RecipeRegistry, inventory: &Inventory) -> Vec<RecipeId> {
        let discovered = auto_discover(registry, inventory);
        let mut newly_unlocked = Vec::new();
        for id in discovered {
            if self.unlocked_recipes.insert(id) {
                newly_unlocked.push(id);
            }
        }
        newly_unlocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemRegistry;
    use crate::loot::{ItemDef, ItemType, Rarity};

    fn test_registry() -> (RecipeRegistry, ItemRegistry) {
        let mut items = ItemRegistry::new();
        items.register(ItemDef { id: 1, name: "Wood".into(), item_type: ItemType::Material, rarity: Rarity::Common, max_stack: 99, icon_name: "wood".into(), value: 1 });
        items.register(ItemDef { id: 2, name: "Stone".into(), item_type: ItemType::Material, rarity: Rarity::Common, max_stack: 99, icon_name: "stone".into(), value: 1 });
        items.register(ItemDef { id: 3, name: "Wooden Sword".into(), item_type: ItemType::Weapon, rarity: Rarity::Common, max_stack: 1, icon_name: "sword_wood".into(), value: 10 });
        items.register(ItemDef { id: 4, name: "Stone Axe".into(), item_type: ItemType::Weapon, rarity: Rarity::Common, max_stack: 1, icon_name: "axe_stone".into(), value: 15 });
        items.register(ItemDef { id: 5, name: "Iron Ingot".into(), item_type: ItemType::Material, rarity: Rarity::Uncommon, max_stack: 50, icon_name: "iron".into(), value: 5 });

        let mut recipes = RecipeRegistry::new();
        recipes.register(
            Recipe::new(1, "Wooden Sword")
                .with_ingredient(1, 5)
                .with_result(3, 1)
                .with_category("Weapons")
        );
        recipes.register(
            Recipe::new(2, "Stone Axe")
                .with_ingredient(1, 3)
                .with_ingredient(2, 2)
                .with_result(4, 1)
                .with_category("Weapons")
                .with_station(1) // requires workbench
        );
        recipes.register(
            Recipe::new(3, "Smelt Iron")
                .with_ingredient(2, 3)
                .with_result(5, 1)
                .with_time(5.0)
                .with_station(2) // requires furnace
                .with_category("Smelting")
        );

        (recipes, items)
    }

    #[test]
    fn can_craft_checks_materials() {
        let (recipes, items) = test_registry();
        let mut inv = Inventory::new(20);
        let recipe = recipes.get(RecipeId(1)).unwrap();

        assert!(!can_craft(recipe, &inv)); // no wood

        inv.add(ItemInstance::with_stack(1, 3), &items);
        assert!(!can_craft(recipe, &inv)); // not enough

        inv.add(ItemInstance::with_stack(1, 2), &items);
        assert!(can_craft(recipe, &inv)); // exactly 5
    }

    #[test]
    fn craft_consumes_and_produces() {
        let (recipes, items) = test_registry();
        let mut inv = Inventory::new(20);
        inv.add(ItemInstance::with_stack(1, 10), &items);

        let recipe = recipes.get(RecipeId(1)).unwrap();
        let result = craft(recipe, &mut inv, &items);
        assert!(result.is_ok());

        assert_eq!(inv.count(1), 5); // 10 - 5 wood
        assert_eq!(inv.count(3), 1); // got 1 wooden sword
    }

    #[test]
    fn craft_fails_without_materials() {
        let (recipes, items) = test_registry();
        let mut inv = Inventory::new(20);
        inv.add(ItemInstance::with_stack(1, 2), &items);

        let recipe = recipes.get(RecipeId(1)).unwrap();
        let result = craft(recipe, &mut inv, &items);
        assert!(matches!(result, Err(CraftError::NotEnoughMaterials)));
        assert_eq!(inv.count(1), 2); // unchanged
    }

    #[test]
    fn available_recipes_filters_by_station() {
        let (recipes, items) = test_registry();
        let mut inv = Inventory::new(20);
        inv.add(ItemInstance::with_stack(1, 99), &items);
        inv.add(ItemInstance::with_stack(2, 99), &items);

        // No station
        let avail = available_recipes(&recipes, &inv, None);
        assert_eq!(avail.len(), 1); // only wooden sword (no station needed)

        // At workbench (station 1)
        let avail = available_recipes(&recipes, &inv, Some(1));
        assert_eq!(avail.len(), 2); // wooden sword + stone axe
    }

    #[test]
    fn auto_discover_finds_recipes() {
        let (recipes, items) = test_registry();
        let mut inv = Inventory::new(20);

        // No items → no discoveries
        let found = auto_discover(&recipes, &inv);
        assert!(found.is_empty());

        // Add wood → discovers wooden sword recipe
        inv.add(ItemInstance::with_stack(1, 1), &items);
        let found = auto_discover(&recipes, &inv);
        assert!(found.contains(&RecipeId(1)));

        // Add stone → also discovers stone axe + smelt iron
        inv.add(ItemInstance::with_stack(2, 1), &items);
        let found = auto_discover(&recipes, &inv);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn crafting_job_timer() {
        let mut job = CraftingJob::new(RecipeId(3), 5.0);
        assert!(!job.is_complete());
        assert!((job.fraction() - 0.0).abs() < 0.01);

        job.update(2.5);
        assert!(!job.is_complete());
        assert!((job.fraction() - 0.5).abs() < 0.01);

        job.update(3.0);
        assert!(job.is_complete());
        assert!((job.fraction() - 1.0).abs() < 0.01);
    }

    #[test]
    fn crafting_state_workflow() {
        let (recipes, items) = test_registry();
        let mut state = CraftingState::new();
        let mut inv = Inventory::new(20);
        inv.add(ItemInstance::with_stack(1, 1), &items);
        inv.add(ItemInstance::with_stack(2, 1), &items);

        // Discover
        let newly = state.discover_recipes(&recipes, &inv);
        assert!(!newly.is_empty());
        assert!(state.is_unlocked(RecipeId(1)));

        // Start timed crafting
        let recipe = recipes.get(RecipeId(3)).unwrap();
        assert!(state.start_crafting(recipe));
        assert!(state.in_progress.is_some());

        // Can't start another
        assert!(!state.start_crafting(recipe));

        // Update
        assert!(state.update(3.0).is_none());
        assert!(state.update(3.0).is_some()); // completed
        assert_eq!(state.total_crafted, 1);
    }

    #[test]
    fn recipe_categories() {
        let (recipes, _) = test_registry();
        let weapons = recipes.by_category("Weapons");
        assert_eq!(weapons.len(), 2);

        let smelting = recipes.by_category("Smelting");
        assert_eq!(smelting.len(), 1);
    }
}
