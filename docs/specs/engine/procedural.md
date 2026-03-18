---
status: spec
crate: amigo_core
depends_on: ["engine/core", "engine/dynamic-tilemap"]
last_updated: 2026-03-18
---

# Procedural Generation

## Purpose

Provide a composable procedural generation toolkit for the Amigo Engine: noise
functions, terrain synthesis, biome assignment, dungeon layout, and tilemap
population.  The system must produce deterministic output for a given seed and be
fast enough to generate a 256x256 world on the main thread in under 50 ms.

## Existierende Bausteine

### Noise primitives (`crates/amigo_core/src/procgen.rs`)

| Symbol | Lines | Description |
|--------|-------|-------------|
| `permutation_table(seed)` | 8-31 | Fisher-Yates xorshift permutation, 512-entry table |
| `GRAD2` | 34-43 | 8 unit-circle gradient vectors for 2D Perlin |
| `fade()`, `lerp_f64()`, `grad2d()` | 45-56 | Interpolation helpers |
| `perlin2d(x, y, perm)` | 59-80 | Classic 2D Perlin noise, returns roughly [-1, 1] |
| `fbm2d(x, y, perm, octaves, lacunarity, persistence)` | 87-108 | Fractal Brownian Motion layered on `perlin2d` |
| `ridge2d(...)` | 111-133 | Ridged noise (abs-value creates mountain features) |
| `warp2d(x, y, perm, scale, strength)` | 136-140 | Domain warping via two offset Perlin samples |

### NoiseMap (`crates/amigo_core/src/procgen.rs`, lines 147-226)

- `NoiseMap { width, height, data: Vec<f64> }`
- `generate(w, h, seed, scale, octaves)` -- FBM-based
- `generate_ridged(...)` -- ridge-based
- `get(x, y)`, `normalize()`, `apply_curve(f)`, `range()`
- `grow_atlas()` not applicable here (font module)

### Biome system (`crates/amigo_core/src/procgen.rs`, lines 233-343)

- `BiomeDef { id, name, temperature_range, moisture_range, ground_tile, surface_tile, decoration_tiles }` with builder methods (`with_temperature`, `with_moisture`, `with_ground`, `with_surface`, `with_decoration`)
- `BiomeDef::contains(temp, moisture) -> bool`
- `BiomeMap { width, height, data: Vec<u32> }`
- `BiomeMap::from_noise(temperature, moisture, biomes)` -- Whittaker-style lookup

### WorldGenerator (`crates/amigo_core/src/procgen.rs`, lines 357-531)

- Builder: `new(seed, w, h)`, `with_biome()`, `with_sea_level()`, `with_terrain_scale()`
- `generate_heightmap() -> NoiseMap` (6-octave FBM, normalized)
- `generate_temperature_map()` (latitude gradient + Perlin noise)
- `generate_moisture_map()` (4-octave FBM, separate seed)
- `generate_biome_map() -> BiomeMap`
- `generate_tiles() -> Vec<u32>` (water below sea level, biome ground otherwise)
- `generate_collision() -> Vec<CollisionTile>` (Solid for water, Empty for land)
- `place_decorations(&self, tiles: &mut Vec<u32>)` (xorshift RNG, per-biome probabilities)

## Public API

### Existing (unchanged)

```rust
// Noise primitives
pub fn permutation_table(seed: u64) -> [u8; 512];
pub fn perlin2d(x: f64, y: f64, perm: &[u8; 512]) -> f64;
pub fn fbm2d(x, y, perm, octaves, lacunarity, persistence) -> f64;
pub fn ridge2d(x, y, perm, octaves, lacunarity, persistence) -> f64;
pub fn warp2d(x, y, perm, warp_scale, warp_strength) -> (f64, f64);

// Structured generation
pub struct NoiseMap { pub width: u32, pub height: u32, pub data: Vec<f64> }
pub struct BiomeDef { /* ... */ }
pub struct BiomeMap { /* ... */ }
pub struct WorldGenerator { /* ... */ }
```

### Proposed: Simplex Noise (2D/3D)

```rust
/// 2D Simplex noise.  ~40% faster than Perlin for the same quality.
pub fn simplex2d(x: f64, y: f64, perm: &[u8; 512]) -> f64;

/// 3D Simplex noise (for animated textures, volumetric effects).
pub fn simplex3d(x: f64, y: f64, z: f64, perm: &[u8; 512]) -> f64;

/// FBM layered on simplex2d.
pub fn simplex_fbm2d(x: f64, y: f64, perm: &[u8; 512],
                      octaves: u32, lacunarity: f64, persistence: f64) -> f64;

/// FBM layered on simplex3d.
pub fn simplex_fbm3d(x: f64, y: f64, z: f64, perm: &[u8; 512],
                      octaves: u32, lacunarity: f64, persistence: f64) -> f64;
```

### Proposed: Wave Function Collapse

```rust
/// Tileset rules for WFC: which tiles can be adjacent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WfcRuleset {
    pub tile_count: u32,
    /// Allowed neighbors per tile per direction (Up, Right, Down, Left).
    pub adjacency: Vec<[Vec<u32>; 4]>,
    /// Relative weights per tile (higher = more frequent).
    pub weights: Vec<f32>,
}

/// WFC solver state.
pub struct WfcSolver {
    width: u32,
    height: u32,
    cells: Vec<WfcCell>,
    rng_seed: u64,
}

impl WfcSolver {
    pub fn new(width: u32, height: u32, rules: &WfcRuleset, seed: u64) -> Self;
    /// Run to completion.  Returns Ok(tile grid) or Err if contradiction.
    pub fn solve(&mut self) -> Result<Vec<u32>, WfcError>;
    /// Step one cell (lowest entropy).  Returns false when done.
    pub fn step(&mut self) -> Result<bool, WfcError>;
    /// Pin a tile at (x,y) before solving (e.g. entrance/exit).
    pub fn pin(&mut self, x: u32, y: u32, tile: u32);
}

#[derive(Debug)]
pub enum WfcError {
    Contradiction { x: u32, y: u32 },
}
```

### Room-and-Corridor Dungeon Generator

Used by [gametypes/roguelike](../gametypes/roguelike.md). The `DungeonConfig` here is the engine primitive; the roguelike gametype wraps it with additional fields (special room chances, boss floors).

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DungeonConfig {
    pub width: u32,
    pub height: u32,
    pub seed: u64,
    pub min_room_size: u32,     // default 5
    pub max_room_size: u32,     // default 15
    pub max_rooms: u32,         // default 30
    pub corridor_width: u32,    // default 1
    pub room_padding: u32,      // min gap between rooms, default 2
    /// Percentage of non-MST edges to re-add for loops (0.0-1.0). Default: 0.15.
    pub loop_chance: f32,
}

/// Result of dungeon generation. This is the engine's canonical type.
/// [gametypes/roguelike] `DungeonFloor` wraps this with gameplay-layer fields
/// (room types, start/boss room IDs, corridor pairs).
pub struct DungeonResult {
    /// Tile grid: 0 = wall, 1 = floor, 2 = corridor, 3 = door.
    pub tiles: Vec<u32>,
    pub width: u32,
    pub height: u32,
    pub rooms: Vec<DungeonRoom>,
    /// Index of the starting room (most central).
    pub start_room: usize,
    /// Index of the end room (farthest from start by graph distance).
    pub end_room: usize,
}

#[derive(Clone, Debug)]
pub struct DungeonRoom {
    pub x: u32, pub y: u32,
    pub width: u32, pub height: u32,
    pub center: (u32, u32),
    pub connections: Vec<usize>,
}

pub fn generate_dungeon(config: &DungeonConfig) -> DungeonResult;

/// Tile semantics mapping (shared convention with [gametypes/roguelike]):
/// 0 = wall (→ CollisionType::Solid)
/// 1 = floor (→ CollisionType::Empty)
/// 2 = corridor (→ CollisionType::Empty)
/// 3 = door (→ CollisionType::Solid initially, Empty when opened)
pub fn tiles_to_collision_layer(tiles: &[u32], width: u32, height: u32) -> CollisionLayer;
```

### Proposed: 3D Noise Variants

```rust
/// 3D Perlin noise.
pub fn perlin3d(x: f64, y: f64, z: f64, perm: &[u8; 512]) -> f64;

/// 3D ridged noise.
pub fn ridge3d(x: f64, y: f64, z: f64, perm: &[u8; 512],
               octaves: u32, lacunarity: f64, persistence: f64) -> f64;

/// 3D domain warping.
pub fn warp3d(x: f64, y: f64, z: f64, perm: &[u8; 512],
              warp_scale: f64, warp_strength: f64) -> (f64, f64, f64);
```

## Behavior

### Simplex Noise

Use the standard simplex lattice with skew factor `F2 = 0.5 * (sqrt(3) - 1)` for
2D and the corresponding 3D skew.  Gradient set: 12 edges of a cube for 3D,
3 axes + diagonals for 2D.  Output range approximately [-1, 1], same as Perlin.
The 3D variant enables time-varying noise (pass elapsed time as the z coordinate)
for animated terrain or cloud effects.

### Wave Function Collapse

1. Initialize every cell with all tile possibilities.
2. Each step: pick the cell with lowest Shannon entropy (random tiebreak using
   xorshift from seed).
3. Collapse that cell to one tile (weighted random).
4. Propagate constraints to neighbors using an arc-consistency queue.
5. If any cell reaches zero possibilities, return `WfcError::Contradiction`.
6. `pin()` pre-collapses cells, useful for placing entrances, exits, or
   connecting to existing map sections.

The solver is iterative (`step()`) so it can be spread across frames for large
maps, or run to completion with `solve()`.

### Room-and-Corridor Dungeon

1. Place rooms via rejection sampling: random position/size, reject if overlapping
   (including padding) with existing rooms.
2. Build a minimum spanning tree (Prim's) on room centers to guarantee
   connectivity.
3. Optionally add ~15% extra edges for loops (configurable).
4. Carve L-shaped corridors between connected rooms.
5. Place door tiles at room-corridor boundaries.
6. Select start room (most central) and end room (graph-distance farthest from
   start).

### NoiseMap Integration

Add a new constructor to `NoiseMap`:

```rust
impl NoiseMap {
    pub fn generate_simplex(w: u32, h: u32, seed: u64, scale: f64, octaves: u32) -> Self;
}
```

## Internal Design

- All RNG is xorshift-based seeded from `u64` for determinism and no-std compat.
- WFC cell state stored as a `BitVec` (up to 256 tile types) or `Vec<bool>` for
  simplicity.  Propagation uses a `VecDeque` work queue.
- Dungeon generator outputs indices compatible with `amigo_tilemap` tile IDs.
  The caller maps abstract IDs (wall/floor/corridor/door) to actual atlas tiles.
- Simplex noise uses the public-domain reference implementation adapted to Rust.
  No patent concerns (the Perlin simplex patent expired 2022).

## Non-Goals

- Real-time infinite terrain streaming (handled by the chunk system, not procgen).
- 3D voxel generation.
- Machine-learning-based generation.
- Cellular automata (can be added later as a separate module).

## Open Questions

1. Should WFC support hex grids, or only rectangular?
2. Should the dungeon generator support non-rectangular room shapes (L-shaped,
   circular)?
3. Is `BitVec` worth the dependency, or is `Vec<bool>` sufficient for tile counts
   under 256?
4. Should simplex noise replace Perlin as the default in `NoiseMap::generate()`?

## Referenzen

- Existing implementation: `crates/amigo_core/src/procgen.rs` (688 lines)
- Simplex noise reference: Stefan Gustavson, "Simplex noise demystified" (2005)
- WFC reference: Maxim Gumin, github.com/mxgmn/WaveFunctionCollapse
- Dungeon generation: Bob Nystrom, "Rooms and Mazes" (journal.stuffwithstuff.com)
