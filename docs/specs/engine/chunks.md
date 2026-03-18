---
status: done
crate: amigo_tilemap
depends_on: ["engine/core", "engine/tilemap"]
last_updated: 2026-03-18
---

# Chunk Streaming

## Purpose

Provides a chunk-based tile storage and streaming system for large or infinite worlds. Chunks are fixed-size grids of tiles and collision data loaded/unloaded dynamically based on camera proximity, enabling Sandbox and God Sim worlds that do not fit entirely in memory.

## Public API

Existing implementation in `crates/amigo_tilemap/src/chunk.rs`.

### Constants

```rust
pub const CHUNK_SIZE: u32 = 32;
```

### ChunkCoord

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub cx: i32,
    pub cy: i32,
}

impl ChunkCoord {
    pub const fn new(cx: i32, cy: i32) -> Self;
}
```

### Chunk

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub coord: ChunkCoord,
    pub tiles: Vec<u32>,
    pub collision: Vec<CollisionTile>,
    #[serde(skip)]
    pub dirty: bool,
}

impl Chunk {
    pub fn new(coord: ChunkCoord) -> Self;
    pub fn get_tile(&self, local_x: u32, local_y: u32) -> u32;
    pub fn set_tile(&mut self, local_x: u32, local_y: u32, tile_id: u32);
    pub fn get_collision(&self, local_x: u32, local_y: u32) -> CollisionTile;
    pub fn set_collision(&mut self, local_x: u32, local_y: u32, col: CollisionTile);
}
```

### ChunkMap

```rust
pub struct ChunkMap {
    pub chunks: FxHashMap<ChunkCoord, Chunk>,
    pub tile_size: u32,
}

impl ChunkMap {
    pub fn new(tile_size: u32) -> Self;
    pub fn load_chunk(&mut self, chunk: Chunk);
    pub fn unload_chunk(&mut self, coord: ChunkCoord) -> Option<Chunk>;
    pub fn get_chunk(&self, coord: ChunkCoord) -> Option<&Chunk>;
    pub fn get_chunk_mut(&mut self, coord: ChunkCoord) -> Option<&mut Chunk>;
    pub fn world_to_chunk(world_x: i32, world_y: i32) -> (ChunkCoord, u32, u32);
    pub fn get_tile(&self, world_x: i32, world_y: i32) -> u32;
    pub fn set_tile(&mut self, world_x: i32, world_y: i32, tile_id: u32);
    pub fn loaded_chunks(&self) -> impl Iterator<Item = &ChunkCoord>;
}
```

### StreamingManager

```rust
#[derive(Clone, Debug, Default)]
pub struct StreamingResult {
    pub chunks_to_load: Vec<ChunkCoord>,
    pub chunks_to_unload: Vec<ChunkCoord>,
}

pub struct StreamingManager {
    pub load_radius: u32,
    pub unload_radius: u32,
}

impl StreamingManager {
    pub fn new(load_radius: u32, unload_radius: u32) -> Self;
    pub fn update(
        &self,
        camera_x: f32,
        camera_y: f32,
        tile_size: u32,
        chunk_map: &mut ChunkMap,
    ) -> StreamingResult;
}
```

## Behavior

- Each chunk is a fixed 32x32 grid (`CHUNK_SIZE`). Tile and collision data are stored as flat `Vec`s indexed by `(local_y * CHUNK_SIZE + local_x)`.
- **World-to-chunk conversion** uses Euclidean division so negative world coordinates map correctly (e.g., world x=-1 maps to chunk cx=-1, local_x=31).
- **Unloaded chunks** return tile ID 0 for reads; writes to unloaded chunks are no-ops.
- **StreamingManager** computes a square load region of `(2 * load_radius + 1)^2` chunks centered on the camera's chunk. Chunks outside `unload_radius` are marked for unloading. `unload_radius >= load_radius` is enforced at construction.
- Already-loaded chunks are never re-requested. Already-unloaded chunks are never re-unloaded.
- Chunks carry a `dirty` flag (skipped during serialization) set whenever a tile or collision cell is modified.
- Chunk serialization round-trips cleanly via serde (JSON, bincode, etc.).

## Internal Design

- `ChunkMap` uses `FxHashMap<ChunkCoord, Chunk>` for O(1) lookup by coordinate.
- Streaming decisions are purely spatial (Manhattan distance from camera chunk).
- The `StreamingManager` does not perform I/O itself; it returns `StreamingResult` with lists the caller processes (first-time chunks trigger procgen, returning chunks trigger disk load).

## Non-Goals

- **Async I/O.** The streaming manager identifies what to load/unload; the actual file I/O and background threading are the caller's responsibility.
- **Compression.** Chunk serialization uses serde; RLE or other compression is layered on top by [engine/save-load](save-load.md).
- **Simulation radius.** Deciding which chunks receive simulation ticks is [engine/simulation](simulation.md)'s responsibility.

## Open Questions

- Should `CHUNK_SIZE` be compile-time configurable (const generic) or remain a module constant?
- Is a hysteresis band (`unload_radius > load_radius`) sufficient, or should unloading also be time-delayed?
- Should chunk data support multiple tile layers inline (foreground + background) rather than requiring two `ChunkMap` instances in `DynamicTileWorld`?
