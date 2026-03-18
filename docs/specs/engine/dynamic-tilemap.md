---
status: done
crate: amigo_tilemap
depends_on: ["engine/core", "engine/tilemap"]
last_updated: 2026-03-18
---

# Dynamic Tilemap

## Purpose

Extends the static tilemap system with runtime tile mutation for Sandbox and God Sim games. Provides block place/destroy at runtime, tile-type property lookups (hardness, emission, drops), dirty-region tracking for efficient re-render and re-light, foreground/background layers (Terraria-style), tile-event hooks, and gravity simulation for falling tiles like sand and gravel.

## Public API

Existing implementation in `crates/amigo_tilemap/src/dynamic.rs`.

### Tile Properties

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileProperties {
    pub name: String,
    pub hardness: u32,
    pub solid: bool,
    pub opaque: bool,
    pub light_emission: u8,
    pub light_color: [u8; 3],
    pub gravity: bool,
    pub liquid_permeable: bool,
    pub drops: Vec<TileDrop>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileDrop {
    pub item_id: u32,
    pub count: u16,
    pub chance: f32,
}
```

### Tile Registry

```rust
pub struct TileRegistry {
    properties: Vec<TileProperties>,
}

impl TileRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, props: TileProperties) -> u32;
    pub fn get(&self, tile_id: u32) -> Option<&TileProperties>;
    pub fn count(&self) -> usize;
}
```

### Tile Events

```rust
#[derive(Clone, Debug)]
pub enum TileEvent {
    Placed { x: i32, y: i32, tile_id: u32 },
    Destroyed { x: i32, y: i32, old_tile_id: u32 },
    NeighborChanged { x: i32, y: i32 },
}
```

### Tile Layers

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileLayer {
    Background,
    Foreground,
}
```

### DynamicTileWorld

```rust
pub struct DynamicTileWorld {
    pub foreground: ChunkMap,
    pub background: ChunkMap,
    pub registry: TileRegistry,
    dirty_chunks: HashSet<(TileLayer, ChunkCoord)>,
    events: Vec<TileEvent>,
}

impl DynamicTileWorld {
    pub fn new(tile_size: u32) -> Self;
    pub fn place_tile(&mut self, layer: TileLayer, x: i32, y: i32, tile_id: u32) -> u32;
    pub fn destroy_tile(&mut self, layer: TileLayer, x: i32, y: i32) -> u32;
    pub fn get_tile(&self, layer: TileLayer, x: i32, y: i32) -> u32;
    pub fn get_properties(&self, layer: TileLayer, x: i32, y: i32) -> Option<&TileProperties>;
    pub fn is_solid(&self, x: i32, y: i32) -> bool;
    pub fn is_opaque(&self, x: i32, y: i32) -> bool;
    pub fn take_dirty_chunks(&mut self) -> Vec<(TileLayer, ChunkCoord)>;
    pub fn take_events(&mut self) -> Vec<TileEvent>;
    pub fn step_gravity(&mut self) -> u32;
}
```

## Behavior

- **Place/destroy** operations mark the affected chunk as dirty and emit `TileEvent::Placed` or `TileEvent::Destroyed` plus four `TileEvent::NeighborChanged` for orthogonal neighbors.
- **Dirty tracking** is per-chunk and per-layer. Consumers drain the dirty set via `take_dirty_chunks()` to trigger incremental re-render, collision grid update, and light recalculation.
- **Gravity step** processes all loaded chunks bottom-to-top within each chunk. A tile with `gravity: true` falls one cell per `step_gravity()` call if the cell below is air (tile ID 0).
- Tile ID 0 is always registered as "air" (non-solid, non-opaque, instant destroy).
- Destroying an air tile is a no-op and returns 0.
- `place_tile` returns the previous tile ID at that position.

## Internal Design

- Wraps two [`ChunkMap`](chunks.md) instances (foreground + background).
- Property lookup is O(1) via a dense `Vec<TileProperties>` indexed by tile ID.
- Dirty chunks use a `HashSet<(TileLayer, ChunkCoord)>` to deduplicate within a frame.
- Events are accumulated in a `Vec` and drained each frame.

## Non-Goals

- **Rendering.** The dynamic tilemap provides data; rendering is handled by [engine/rendering](rendering.md).
- **Procedural generation.** World generation is in `procgen.md`; this module handles mutation after generation.
- **Liquid simulation.** Liquid flow lives in [engine/liquids](liquids.md); this module only provides `liquid_permeable` as a property.

## Open Questions

- Should `on_tick` tile updates (e.g., grass growth, torch burnout) be a built-in tick system or purely event-driven via [engine/simulation](simulation.md)?
- Should tile IDs be `u32` or `u16` to save memory in chunk storage?
- How should multi-tile structures (doors, trees) be represented?
