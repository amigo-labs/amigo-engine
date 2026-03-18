---
status: done
crate: amigo_tilemap
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Tilemap System

## Purpose

First-class engine feature for tile-based worlds. Provides multiple layers, auto-tiling (bitmask-based), animated tiles, collision layer with solid/one-way/slope/trigger types, orthogonal and isometric grid modes, and chunk streaming for large worlds.

## Public API

### Grid Modes

```rust
pub enum GridMode {
    Orthogonal { tile_width: u32, tile_height: u32 },   // Standard (TD, Platformer)
    Isometric { tile_width: u32, tile_height: u32 },     // Diamond (Diablo, Sacred)
}
```

Both modes share the same API -- the engine handles coordinate conversion internally:

```rust
// Same API regardless of grid mode
let is_solid = tilemap.is_solid(x, y);
tilemap.set(layer, x, y, TileId(42));

// Mouse-to-tile conversion handles both modes
let tile_pos = tilemap.screen_to_tile(ctx.input.mouse_world_pos());
```

Isometric rendering sorts tiles back-to-front automatically for correct overlap.

### Tilemap API

```rust
// Reading
let is_solid = tilemap.is_solid(x, y);

// Writing
tilemap.set(layer, x, y, TileId(42));

// Auto-tiling
tilemap.set_terrain(x, y, TerrainType::Water);
// Engine auto-selects correct variant based on neighbors
```

## Internal Design

### Chunk Streaming

For large worlds (Diablo-style), the tilemap is divided into chunks that load/unload based on camera position:

```rust
pub struct ChunkedTilemap {
    pub chunk_size: u32,              // tiles per chunk (e.g., 32x32)
    pub active_radius: u32,           // chunks around camera to keep loaded
    loaded_chunks: HashMap<(i32, i32), TileChunk>,
}

// Engine auto-loads/unloads chunks as camera moves
// Only chunks within active_radius are in memory and rendered
// Chunks outside are serialized to disk and freed
```

For small levels (TD, Platformer): entire map in memory, no streaming needed. Chunk streaming is opt-in.

---

## Extensions (Sandbox/God Sim)

> Added per gap analysis (`05-sandbox-godsim-gaps.md`). These features are implemented in `crates/amigo_tilemap/src/dynamic.rs`.

### Tile Properties System

Each tile type has data-driven properties instead of hardcoded behavior. Properties are registered in a central `TileRegistry` and looked up by tile ID at runtime.

```rust
// crates/amigo_tilemap/src/dynamic.rs

pub struct TileProperties {
    pub name: String,
    pub hardness: u32,           // Mining/destroy time in sim ticks (0 = instant)
    pub solid: bool,             // Blocks movement
    pub opaque: bool,            // Blocks light propagation
    pub light_emission: u8,      // 0 = none, 255 = max
    pub light_color: [u8; 3],    // RGB
    pub gravity: bool,           // Falls under gravity (sand, gravel)
    pub liquid_permeable: bool,  // Liquids can pass through
    pub drops: Vec<TileDrop>,    // Items dropped when destroyed
}

pub struct TileRegistry {
    properties: Vec<TileProperties>,
}

impl TileRegistry {
    pub fn register(&mut self, props: TileProperties) -> u32;
    pub fn get(&self, tile_id: u32) -> Option<&TileProperties>;
    pub fn count(&self) -> usize;
}
```

### Background/Foreground Layer Concept

The `DynamicTileWorld` maintains two separate `ChunkMap` layers (Terraria-style):

- **Foreground:** Main gameplay layer -- collision, lighting, interaction.
- **Background:** Behind entities -- decorative walls, less collision interaction.

```rust
// crates/amigo_tilemap/src/dynamic.rs

pub enum TileLayer {
    Background,
    Foreground,
}

pub struct DynamicTileWorld {
    pub foreground: ChunkMap,
    pub background: ChunkMap,
    pub registry: TileRegistry,
    dirty_chunks: HashSet<(TileLayer, ChunkCoord)>,
    events: Vec<TileEvent>,
}

impl DynamicTileWorld {
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

### Hook-based Tile Events

When tiles are placed or destroyed, the system emits events including `NeighborChanged` for all four cardinal neighbors. This enables Redstone-style propagation logic.

```rust
// crates/amigo_tilemap/src/dynamic.rs

pub enum TileEvent {
    Placed { x: i32, y: i32, tile_id: u32 },
    Destroyed { x: i32, y: i32, old_tile_id: u32 },
    NeighborChanged { x: i32, y: i32 },
}
```

On every `place_tile` or `destroy_tile` call, four `NeighborChanged` events are emitted for `(x, y-1)`, `(x, y+1)`, `(x-1, y)`, `(x+1, y)`. Game code can drain events via `take_events()` each frame and react accordingly.

### Cross-reference

For the full runtime tile mutation API (chunk-based storage, dirty tracking, gravity simulation), see the planned `engine/dynamic-tilemap.md` spec. The implementation lives in `crates/amigo_tilemap/src/dynamic.rs` and `crates/amigo_tilemap/src/chunk.rs`.
