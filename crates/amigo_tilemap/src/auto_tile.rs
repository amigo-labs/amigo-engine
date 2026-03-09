use serde::{Deserialize, Serialize};

// 4-directional neighbor bitmask constants.
pub const NEIGHBOR_N: u8 = 1;
pub const NEIGHBOR_E: u8 = 2;
pub const NEIGHBOR_S: u8 = 4;
pub const NEIGHBOR_W: u8 = 8;

// Diagonal neighbor bitmask constants.
pub const NEIGHBOR_NE: u8 = 16;
pub const NEIGHBOR_SE: u8 = 32;
pub const NEIGHBOR_SW: u8 = 64;
pub const NEIGHBOR_NW: u8 = 128;

/// A single auto-tile rule mapping a neighbor bitmask to a tile ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoTileRule {
    /// The terrain type this rule applies to.
    pub terrain_type: u32,
    /// Bitmask encoding which neighbors share the same terrain.
    /// 4-bit (N/E/S/W) or 8-bit (including diagonals).
    pub bitmask: u8,
    /// The tile to use when this bitmask is matched.
    pub tile_id: u32,
}

/// A named set of auto-tile rules for a specific terrain type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoTileSet {
    /// The terrain type this tileset resolves.
    pub terrain_type: u32,
    /// Human-readable name for this tileset.
    pub name: String,
    /// Ordered list of rules; the first matching rule wins.
    pub rules: Vec<AutoTileRule>,
    /// Fallback tile ID if no rule matches the neighbor configuration.
    pub default_tile: u32,
}

impl AutoTileSet {
    /// Find the appropriate tile for the given neighbor bitmask.
    /// Returns the tile ID from the first matching rule, or `default_tile` if none match.
    pub fn resolve(&self, neighbors: u8) -> u32 {
        for rule in &self.rules {
            if rule.bitmask == neighbors {
                return rule.tile_id;
            }
        }
        self.default_tile
    }
}

/// Resolver that holds multiple `AutoTileSet`s and provides lookup and
/// neighbor-computation utilities.
pub struct AutoTileResolver {
    pub tilesets: Vec<AutoTileSet>,
}

impl AutoTileResolver {
    /// Create an empty resolver.
    pub fn new() -> Self {
        Self {
            tilesets: Vec::new(),
        }
    }

    /// Register an auto-tile set.
    pub fn add_tileset(&mut self, tileset: AutoTileSet) {
        self.tilesets.push(tileset);
    }

    /// Resolve the tile for a given terrain type and neighbor bitmask.
    /// Returns `None` if no tileset is registered for the terrain type.
    pub fn resolve_tile(&self, terrain_type: u32, neighbors: u8) -> Option<u32> {
        self.tilesets
            .iter()
            .find(|ts| ts.terrain_type == terrain_type)
            .map(|ts| ts.resolve(neighbors))
    }

    /// Compute a 4-directional neighbor bitmask for the cell at (`x`, `y`).
    ///
    /// Bit layout: 0 = North, 1 = East, 2 = South, 3 = West.
    /// A bit is set when the neighboring cell contains `terrain_type`.
    pub fn compute_neighbors_4(
        terrain_map: &[Vec<u32>],
        x: usize,
        y: usize,
        terrain_type: u32,
    ) -> u8 {
        let height = terrain_map.len();
        if height == 0 {
            return 0;
        }

        let mut mask: u8 = 0;

        // North (y - 1)
        if y > 0 {
            if let Some(&val) = terrain_map[y - 1].get(x) {
                if val == terrain_type {
                    mask |= NEIGHBOR_N;
                }
            }
        }

        // East (x + 1)
        if let Some(&val) = terrain_map[y].get(x + 1) {
            if val == terrain_type {
                mask |= NEIGHBOR_E;
            }
        }

        // South (y + 1)
        if y + 1 < height {
            if let Some(&val) = terrain_map[y + 1].get(x) {
                if val == terrain_type {
                    mask |= NEIGHBOR_S;
                }
            }
        }

        // West (x - 1)
        if x > 0 {
            if let Some(&val) = terrain_map[y].get(x - 1) {
                if val == terrain_type {
                    mask |= NEIGHBOR_W;
                }
            }
        }

        mask
    }

    /// Compute an 8-directional neighbor bitmask for the cell at (`x`, `y`).
    ///
    /// Bit layout: 0 = N, 1 = E, 2 = S, 3 = W, 4 = NE, 5 = SE, 6 = SW, 7 = NW.
    /// A bit is set when the neighboring cell contains `terrain_type`.
    pub fn compute_neighbors_8(
        terrain_map: &[Vec<u32>],
        x: usize,
        y: usize,
        terrain_type: u32,
    ) -> u8 {
        // Start with the 4-directional bits.
        let mut mask = Self::compute_neighbors_4(terrain_map, x, y, terrain_type);

        let height = terrain_map.len();
        if height == 0 {
            return mask;
        }

        // NE (x + 1, y - 1)
        if y > 0 {
            if let Some(&val) = terrain_map[y - 1].get(x + 1) {
                if val == terrain_type {
                    mask |= NEIGHBOR_NE;
                }
            }
        }

        // SE (x + 1, y + 1)
        if y + 1 < height {
            if let Some(&val) = terrain_map[y + 1].get(x + 1) {
                if val == terrain_type {
                    mask |= NEIGHBOR_SE;
                }
            }
        }

        // SW (x - 1, y + 1)
        if y + 1 < height && x > 0 {
            if let Some(&val) = terrain_map[y + 1].get(x - 1) {
                if val == terrain_type {
                    mask |= NEIGHBOR_SW;
                }
            }
        }

        // NW (x - 1, y - 1)
        if y > 0 && x > 0 {
            if let Some(&val) = terrain_map[y - 1].get(x - 1) {
                if val == terrain_type {
                    mask |= NEIGHBOR_NW;
                }
            }
        }

        mask
    }
}

impl Default for AutoTileResolver {
    fn default() -> Self {
        Self::new()
    }
}
