---
status: done
crate: amigo_core
depends_on: ["engine/procedural", "engine/crafting", "engine/inventory", "engine/chunks"]
last_updated: 2026-06-09
---

# Sandbox Survival

## Purpose

Open-world survival sandboxes: a procedurally generated world (noise-based terrain, biomes, optionally WFC-built structures and dungeons), mining/gathering resources into a stacking inventory, crafting tools at stations, and free-form building. The loop is: explore, gather, craft better tools, expand, repeat.

Examples: Terraria (noise terrain + biomes, mining, crafting stations), Minecraft (biome maps, gather-craft-build), Valheim (exploration, crafting progression), Don't Starve (survival crafting, auto-discovered recipes).

Backed by `amigo_core::procgen`, `amigo_core::crafting`, `amigo_core::inventory`, and `amigo_core::resources`.

## Public API

### Noise Primitives (`amigo_core::procgen`)

```rust
/// Seeded permutation table feeding all gradient noise.
pub fn permutation_table(seed: u64) -> [u8; 512];

pub fn perlin2d(x: f64, y: f64, perm: &[u8; 512]) -> f64;            // ~[-1, 1]
pub fn simplex2d(x: f64, y: f64, perm: &[u8; 512]) -> f64;           // ~40% faster than Perlin
pub fn fbm2d(x, y, perm, octaves: u32, lacunarity: f64, persistence: f64) -> f64;
pub fn simplex_fbm2d(x, y, perm, octaves, lacunarity, persistence) -> f64;
pub fn ridge2d(x, y, perm, octaves, lacunarity, persistence) -> f64; // mountains
pub fn warp2d(x, y, perm, warp_scale: f64, warp_strength: f64) -> (f64, f64); // domain warping

/// 2D grid of noise values with post-processing helpers.
pub struct NoiseMap { pub width: u32, pub height: u32, pub data: Vec<f64> }
impl NoiseMap {
    pub fn generate(width: u32, height: u32, seed: u64, scale: f64, octaves: u32) -> Self; // FBM
    pub fn generate_ridged(...) -> Self;
    pub fn generate_simplex(...) -> Self;
    pub fn get(&self, x: u32, y: u32) -> f64;
    pub fn normalize(&mut self);                       // remap to 0..1
    pub fn apply_curve(&mut self, f: impl Fn(f64) -> f64);
}
```

### Biomes and World Generation (`amigo_core::procgen`)

```rust
/// Biome occupies a temperature x moisture rectangle and defines its tiles.
pub struct BiomeDef {
    pub id: u32, pub name: String,
    pub temperature_range: (f64, f64), pub moisture_range: (f64, f64),
    pub ground_tile: u32, pub surface_tile: Option<u32>,
    pub decoration_tiles: Vec<(u32, f32)>, // (tile, spawn chance)
}
// Builder: BiomeDef::new(id, name).with_temperature(..).with_moisture(..)
//          .with_ground(..).with_surface(..).with_decoration(..)

/// Whittaker-style biome assignment from two noise maps.
pub struct BiomeMap { pub fn from_noise(temperature: &NoiseMap, moisture: &NoiseMap,
                                        biomes: &[BiomeDef]) -> Self;
                      pub fn get_biome(&self, x: u32, y: u32) -> u32; }

/// One-stop overworld generator: heightmap + latitude-based temperature
/// + moisture -> biome map -> tile and collision arrays.
pub struct WorldGenerator {
    pub seed: u64, pub width: u32, pub height: u32,
    pub biomes: Vec<BiomeDef>, pub sea_level: f64,
    pub terrain_scale: f64, pub temperature_scale: f64, pub moisture_scale: f64,
}
impl WorldGenerator {
    pub fn new(seed: u64, width: u32, height: u32) -> Self; // builder: with_biome/with_sea_level/with_terrain_scale
    pub fn generate_heightmap(&self) -> NoiseMap;
    pub fn generate_temperature_map(&self) -> NoiseMap; // hot equator, cold poles
    pub fn generate_moisture_map(&self) -> NoiseMap;
    pub fn generate_biome_map(&self) -> BiomeMap;
    pub fn generate_tiles(&self) -> Vec<u32>;           // water below sea_level, else biome ground tile
    pub fn generate_collision(&self) -> Vec<CollisionTile>; // water = Solid, land = Empty
    pub fn place_decorations(&self, tiles: &mut [u32]); // trees/rocks per biome chance
}
```

### Structures: WFC and Dungeons (`amigo_core::procgen`)

```rust
/// Wave Function Collapse for ruins/villages/cave decoration.
pub struct WfcRuleset { pub tile_count: u32,
                        pub adjacency: Vec<[Vec<u32>; 4]>, // allowed neighbors per direction
                        pub weights: Vec<f32> }
pub struct WfcSolver { /* ... */ }
impl WfcSolver {
    pub fn new(width: u32, height: u32, rules: &WfcRuleset, seed: u64) -> Self;
    pub fn pin(&mut self, x: u32, y: u32, tile: u32);      // fix tiles (entrances) pre-solve
    pub fn step(&mut self) -> Result<bool, WfcError>;       // incremental (loading screens)
    pub fn solve(&mut self) -> Result<Vec<u32>, WfcError>;  // Err(Contradiction { x, y })
}

/// Room-and-corridor dungeons (caves/mines): rejection-sampled rooms,
/// Prim's MST connectivity, loop_chance extra edges, L-shaped corridors.
pub struct DungeonConfig { pub width: u32, pub height: u32, pub seed: u64,
                           pub min_room_size: u32, pub max_room_size: u32, pub max_rooms: u32,
                           pub corridor_width: u32, pub room_padding: u32, pub loop_chance: f32 }
pub fn generate_dungeon(config: &DungeonConfig) -> DungeonResult;
pub struct DungeonResult { pub tiles: Vec<u32>, // 0=wall, 1=floor, 2=corridor, 3=door
                           pub rooms: Vec<DungeonRoom>, pub start_room: usize, pub end_room: usize, /* ... */ }
pub fn dungeon_tiles_to_collision(tiles: &[u32]) -> Vec<u8>; // wall -> Solid
```

### Gathering and Inventory (`amigo_core::inventory`, `amigo_core::loot`)

```rust
/// Item templates (ItemType::Material for ores/wood, max_stack for stacking).
pub struct ItemRegistry { pub fn register(&mut self, def: ItemDef); pub fn get(&self, id: u32) -> Option<&ItemDef>; }

/// Slot inventory with automatic stack merging — mining feeds straight into add().
pub struct Inventory { pub capacity: usize }
impl Inventory {
    pub fn add(&mut self, item: ItemInstance, registry: &ItemRegistry) -> Option<ItemInstance>; // leftover if full
    pub fn remove_by_id(&mut self, def_id: u32, count: u32) -> u32; // consume materials for building
    pub fn count(&self, def_id: u32) -> u32;
    pub fn has(&self, def_id: u32, count: u32) -> bool;
    pub fn free_slots(&self) -> usize;
}
/// Tool/armor slots; Equipment::total_modifier("mining_speed") etc.
pub struct Equipment { /* equip/unequip/get/total_modifier */ }
```

### Crafting Progression (`amigo_core::crafting`)

```rust
// Recipe / RecipeRegistry / craft() / can_craft() as in the farming-sim spec.
// Survival-relevant pieces:
pub fn available_recipes<'a>(registry: &'a RecipeRegistry, inventory: &Inventory,
                             station: Option<u32>) -> Vec<&'a Recipe>; // workbench/furnace gating
pub fn auto_discover(registry: &RecipeRegistry, inventory: &Inventory) -> Vec<RecipeId>;
impl CraftingState {
    /// Unlocks recipes the moment the player holds >=1 of every ingredient
    /// (Don't Starve style discovery). Returns newly unlocked ids.
    pub fn discover_recipes(&mut self, registry: &RecipeRegistry, inventory: &Inventory) -> Vec<RecipeId>;
}
```

### Game-Wide State (`amigo_core::resources`)

```rust
/// Type-keyed singleton container — one instance per type. Stores world seed,
/// player survival stats, generated maps, etc. without engine changes.
pub struct Resources { /* HashMap<TypeId, Box<dyn Any>> */ }
impl Resources {
    pub fn insert<T: 'static>(&mut self, resource: T);
    pub fn get<T: 'static>(&self) -> Option<&T>;
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T>;
    pub fn get_or_insert_with<T: 'static>(&mut self, f: impl FnOnce() -> T) -> &mut T;
    pub fn get_or_default<T: 'static + Default>(&mut self) -> &mut T;
    pub fn remove<T: 'static>(&mut self) -> Option<T>;
    pub fn contains<T: 'static>(&self) -> bool;
}
```

## Behavior

- **World gen**: At new-game, `WorldGenerator` (seeded) produces tiles, collision, and decorations; the results map onto the tilemap/chunk systems. Underground levels use `generate_dungeon` per depth (seed = world seed ^ depth); hand-authored structure flavor comes from `WfcSolver` with entrance tiles pinned. Everything derives from one seed, so worlds are reproducible.
- **Mining/building**: Breaking a tile converts it to an `ItemInstance` (`ItemType::Material`) and `Inventory::add`s it; placing a block does the reverse via `remove_by_id`. Tile mutation itself goes through the dynamic tilemap system.
- **Crafting progression**: Picking up a new material triggers `CraftingState::discover_recipes`, expanding the craft menu. Stations (workbench, furnace) gate higher tiers through `Recipe.required_station` + `available_recipes`. Crafted tools equip into `Equipment` and their `ItemModifier`s (e.g. `mining_speed`) scale interaction rates.
- **Survival stats**: Hunger/health/temperature are game-defined structs stored in `Resources` and ticked by game systems — the engine deliberately does not prescribe them.

## Future Work

- No chunk streaming inside procgen — `WorldGenerator` produces whole maps; infinite worlds need per-chunk generation driven by `engine/chunks` (seeding noise by chunk coordinates works since noise is stateless).
- No built-in survival needs (hunger/thirst/temperature) module for the player; `amigo_core::agents::Needs` exists but targets NPCs.
- No cave-in/lighting/fluid simulation here (see `engine/lighting`, `engine/liquids`).
- No 3D/heightmap-to-mesh; all generators are 2D grids.

## Referenzen

- [engine/procedural](../engine/procedural.md) — engine-level procedural generation spec
- [engine/chunks](../engine/chunks.md) — streaming large generated worlds
- [engine/dynamic-tilemap](../engine/dynamic-tilemap.md) — runtime tile mutation for mining/building
- [engine/crafting](../engine/crafting.md), [engine/inventory](../engine/inventory.md) — UI conventions
- Terraria — biome bands, mining loop, station-gated crafting
- Minecraft — temperature/moisture biome model
- Don't Starve — ingredient-based recipe discovery
