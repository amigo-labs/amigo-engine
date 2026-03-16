---
status: draft
crate: amigo_tilemap
depends_on: ["engine/core"]
last_updated: 2026-03-16
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
