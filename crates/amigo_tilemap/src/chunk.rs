//! Chunk-based tile streaming for large or infinite worlds.
//!
//! Each [`Chunk`] holds a fixed 32x32 grid of tiles and collision data.
//! [`ChunkMap`] indexes loaded chunks by [`ChunkCoord`], and
//! [`StreamingManager`] decides which chunks to load/unload based on camera
//! position.

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::CollisionType;

/// Re-export the collision type under the name used by the chunk API.
pub type CollisionTile = CollisionType;

/// Tiles per chunk edge. Each chunk is `CHUNK_SIZE x CHUNK_SIZE` tiles.
pub const CHUNK_SIZE: u32 = 32;

// ---------------------------------------------------------------------------
// ChunkCoord
// ---------------------------------------------------------------------------

/// Integer coordinate that identifies a chunk in the world grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkCoord {
    pub cx: i32,
    pub cy: i32,
}

impl ChunkCoord {
    pub const fn new(cx: i32, cy: i32) -> Self {
        Self { cx, cy }
    }
}

// ---------------------------------------------------------------------------
// Chunk
// ---------------------------------------------------------------------------

/// A fixed-size block of tiles and collision data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub coord: ChunkCoord,
    pub tiles: Vec<u32>,
    pub collision: Vec<CollisionTile>,
    #[serde(skip)]
    pub dirty: bool,
}

impl Chunk {
    /// Create a new empty chunk at the given coordinate.
    pub fn new(coord: ChunkCoord) -> Self {
        let len = (CHUNK_SIZE * CHUNK_SIZE) as usize;
        Self {
            coord,
            tiles: vec![0; len],
            collision: vec![CollisionTile::Empty; len],
            dirty: true,
        }
    }

    #[inline]
    fn index(local_x: u32, local_y: u32) -> usize {
        debug_assert!(local_x < CHUNK_SIZE && local_y < CHUNK_SIZE);
        (local_y * CHUNK_SIZE + local_x) as usize
    }

    pub fn get_tile(&self, local_x: u32, local_y: u32) -> u32 {
        self.tiles[Self::index(local_x, local_y)]
    }

    pub fn set_tile(&mut self, local_x: u32, local_y: u32, tile_id: u32) {
        let idx = Self::index(local_x, local_y);
        self.tiles[idx] = tile_id;
        self.dirty = true;
    }

    pub fn get_collision(&self, local_x: u32, local_y: u32) -> CollisionTile {
        self.collision[Self::index(local_x, local_y)]
    }

    pub fn set_collision(&mut self, local_x: u32, local_y: u32, col: CollisionTile) {
        let idx = Self::index(local_x, local_y);
        self.collision[idx] = col;
        self.dirty = true;
    }
}

// ---------------------------------------------------------------------------
// ChunkMap
// ---------------------------------------------------------------------------

/// Sparse collection of loaded chunks, indexed by [`ChunkCoord`].
pub struct ChunkMap {
    pub chunks: FxHashMap<ChunkCoord, Chunk>,
    pub tile_size: u32,
}

impl ChunkMap {
    pub fn new(tile_size: u32) -> Self {
        Self {
            chunks: FxHashMap::default(),
            tile_size,
        }
    }

    /// Insert (or replace) a chunk.
    pub fn load_chunk(&mut self, chunk: Chunk) {
        self.chunks.insert(chunk.coord, chunk);
    }

    /// Remove a chunk by coordinate, returning it if it was present.
    pub fn unload_chunk(&mut self, coord: ChunkCoord) -> Option<Chunk> {
        self.chunks.remove(&coord)
    }

    pub fn get_chunk(&self, coord: ChunkCoord) -> Option<&Chunk> {
        self.chunks.get(&coord)
    }

    pub fn get_chunk_mut(&mut self, coord: ChunkCoord) -> Option<&mut Chunk> {
        self.chunks.get_mut(&coord)
    }

    /// Convert world tile coordinates to a `(ChunkCoord, local_x, local_y)`.
    ///
    /// Works correctly for negative coordinates via Euclidean division.
    pub fn world_to_chunk(world_x: i32, world_y: i32) -> (ChunkCoord, u32, u32) {
        let cs = CHUNK_SIZE as i32;
        let cx = world_x.div_euclid(cs);
        let cy = world_y.div_euclid(cs);
        let lx = world_x.rem_euclid(cs) as u32;
        let ly = world_y.rem_euclid(cs) as u32;
        (ChunkCoord::new(cx, cy), lx, ly)
    }

    /// Read a tile at world tile coordinates. Returns `0` if the chunk is not
    /// loaded.
    pub fn get_tile(&self, world_x: i32, world_y: i32) -> u32 {
        let (coord, lx, ly) = Self::world_to_chunk(world_x, world_y);
        self.chunks.get(&coord).map_or(0, |c| c.get_tile(lx, ly))
    }

    /// Write a tile at world tile coordinates. No-op if the chunk is not
    /// loaded.
    pub fn set_tile(&mut self, world_x: i32, world_y: i32, tile_id: u32) {
        let (coord, lx, ly) = Self::world_to_chunk(world_x, world_y);
        if let Some(chunk) = self.chunks.get_mut(&coord) {
            chunk.set_tile(lx, ly, tile_id);
        }
    }

    /// Iterate over the coordinates of all currently loaded chunks.
    pub fn loaded_chunks(&self) -> impl Iterator<Item = &ChunkCoord> {
        self.chunks.keys()
    }
}

// ---------------------------------------------------------------------------
// StreamingManager
// ---------------------------------------------------------------------------

/// Describes which chunks should be loaded or unloaded after an update.
#[derive(Clone, Debug, Default)]
pub struct StreamingResult {
    pub chunks_to_load: Vec<ChunkCoord>,
    pub chunks_to_unload: Vec<ChunkCoord>,
}

/// Decides which chunks to load/unload based on camera position.
pub struct StreamingManager {
    pub load_radius: u32,
    pub unload_radius: u32,
}

impl StreamingManager {
    pub fn new(load_radius: u32, unload_radius: u32) -> Self {
        assert!(
            unload_radius >= load_radius,
            "unload_radius must be >= load_radius"
        );
        Self {
            load_radius,
            unload_radius,
        }
    }

    /// Determine which chunks need loading/unloading given the current camera
    /// pixel position.
    pub fn update(
        &self,
        camera_x: f32,
        camera_y: f32,
        tile_size: u32,
        chunk_map: &mut ChunkMap,
    ) -> StreamingResult {
        let chunk_pixel = (CHUNK_SIZE * tile_size) as f32;
        // Which chunk the camera is currently in.
        let cam_cx = (camera_x / chunk_pixel).floor() as i32;
        let cam_cy = (camera_y / chunk_pixel).floor() as i32;

        let mut result = StreamingResult::default();

        // --- chunks to load ---
        let r = self.load_radius as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let coord = ChunkCoord::new(cam_cx + dx, cam_cy + dy);
                if !chunk_map.chunks.contains_key(&coord) {
                    result.chunks_to_load.push(coord);
                }
            }
        }

        // --- chunks to unload ---
        let ur = self.unload_radius as i32;
        let to_unload: Vec<ChunkCoord> = chunk_map
            .chunks
            .keys()
            .filter(|c| {
                let dx = (c.cx - cam_cx).abs();
                let dy = (c.cy - cam_cy).abs();
                dx > ur || dy > ur
            })
            .copied()
            .collect();

        result.chunks_to_unload = to_unload;

        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Coordinate Conversion ────────────────────────────────────

    #[test]
    fn world_to_chunk_positive() {
        let (coord, lx, ly) = ChunkMap::world_to_chunk(33, 65);
        assert_eq!(coord, ChunkCoord::new(1, 2));
        assert_eq!(lx, 1);
        assert_eq!(ly, 1);
    }

    #[test]
    fn world_to_chunk_origin() {
        let (coord, lx, ly) = ChunkMap::world_to_chunk(0, 0);
        assert_eq!(coord, ChunkCoord::new(0, 0));
        assert_eq!(lx, 0);
        assert_eq!(ly, 0);
    }

    #[test]
    fn world_to_chunk_negative() {
        // -1 should map to chunk -1, local 31
        let (coord, lx, ly) = ChunkMap::world_to_chunk(-1, -1);
        assert_eq!(coord, ChunkCoord::new(-1, -1));
        assert_eq!(lx, 31);
        assert_eq!(ly, 31);
    }

    #[test]
    fn world_to_chunk_boundary() {
        // Exactly on chunk boundary (32 -> chunk 1, local 0)
        let (coord, lx, ly) = ChunkMap::world_to_chunk(32, 64);
        assert_eq!(coord, ChunkCoord::new(1, 2));
        assert_eq!(lx, 0);
        assert_eq!(ly, 0);
    }

    // ── Cross-Chunk Access ───────────────────────────────────────

    #[test]
    fn set_get_tiles_across_chunks() {
        let mut map = ChunkMap::new(16);
        map.load_chunk(Chunk::new(ChunkCoord::new(0, 0)));
        map.load_chunk(Chunk::new(ChunkCoord::new(1, 0)));

        // Set in chunk (0,0)
        map.set_tile(31, 0, 42);
        // Set in chunk (1,0)
        map.set_tile(32, 0, 99);

        assert_eq!(map.get_tile(31, 0), 42);
        assert_eq!(map.get_tile(32, 0), 99);

        // Unloaded chunk returns 0
        assert_eq!(map.get_tile(100, 100), 0);
    }

    #[test]
    fn set_get_tiles_negative_coords() {
        let mut map = ChunkMap::new(16);
        map.load_chunk(Chunk::new(ChunkCoord::new(-1, -1)));

        map.set_tile(-1, -1, 7);
        assert_eq!(map.get_tile(-1, -1), 7);
    }

    // ── Streaming Manager ────────────────────────────────────────

    #[test]
    fn streaming_manager_load_set() {
        let sm = StreamingManager::new(1, 2);
        let mut map = ChunkMap::new(16);

        let result = sm.update(0.0, 0.0, 16, &mut map);

        // load_radius=1 → 3x3 = 9 chunks around (0,0)
        assert_eq!(result.chunks_to_load.len(), 9);
        assert!(result.chunks_to_unload.is_empty());

        // All requested coords should be within [-1, 1]
        for c in &result.chunks_to_load {
            assert!(c.cx >= -1 && c.cx <= 1);
            assert!(c.cy >= -1 && c.cy <= 1);
        }
    }

    #[test]
    fn streaming_manager_unload_set() {
        let sm = StreamingManager::new(1, 2);
        let mut map = ChunkMap::new(16);

        // Load a chunk far away
        map.load_chunk(Chunk::new(ChunkCoord::new(10, 10)));
        // And one nearby
        map.load_chunk(Chunk::new(ChunkCoord::new(0, 0)));

        let result = sm.update(0.0, 0.0, 16, &mut map);

        // (10,10) is outside unload_radius=2 → should be unloaded
        assert!(result.chunks_to_unload.contains(&ChunkCoord::new(10, 10)));
        // (0,0) is inside → should NOT be unloaded
        assert!(!result.chunks_to_unload.contains(&ChunkCoord::new(0, 0)));
    }

    #[test]
    fn streaming_no_duplicate_loads() {
        let sm = StreamingManager::new(1, 2);
        let mut map = ChunkMap::new(16);
        map.load_chunk(Chunk::new(ChunkCoord::new(0, 0)));

        let result = sm.update(0.0, 0.0, 16, &mut map);

        // (0,0) is already loaded → should NOT appear in to_load
        assert!(!result.chunks_to_load.contains(&ChunkCoord::new(0, 0)));
        // But all other 8 neighbours should be requested
        assert_eq!(result.chunks_to_load.len(), 8);
    }

    // ── Serialization ────────────────────────────────────────────

    #[test]
    fn chunk_serialization_roundtrip() {
        let mut chunk = Chunk::new(ChunkCoord::new(3, -7));
        chunk.set_tile(5, 10, 42);
        chunk.set_collision(5, 10, CollisionTile::Solid);

        let json = serde_json::to_string(&chunk).expect("serialize");
        let deserialized: Chunk = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.coord, chunk.coord);
        assert_eq!(deserialized.get_tile(5, 10), 42);
        assert_eq!(deserialized.get_collision(5, 10), CollisionTile::Solid);
        // dirty is skipped during serialization, defaults to false
        assert!(!deserialized.dirty);
    }
}
