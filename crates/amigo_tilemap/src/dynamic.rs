//! Dynamic tilemap extension for runtime tile mutation (Sandbox / God Sim).
//!
//! Builds on top of [`ChunkMap`] to add:
//! - Tile property registry (hardness, emission, drops, etc.)
//! - Dirty-region tracking for efficient re-render & re-light
//! - Background/foreground layer concept
//! - Tile-event hooks (on_place, on_destroy, on_neighbor_change)

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::chunk::{ChunkCoord, ChunkMap, CHUNK_SIZE};

// ---------------------------------------------------------------------------
// Tile properties
// ---------------------------------------------------------------------------

/// Physical and gameplay properties for a tile type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileProperties {
    /// Human-readable name.
    pub name: String,
    /// Mining/destroy time in sim ticks (0 = instant).
    pub hardness: u32,
    /// Whether the tile blocks movement.
    pub solid: bool,
    /// Whether the tile blocks light propagation.
    pub opaque: bool,
    /// Light emitted by this tile (0 = none, 255 = max).
    pub light_emission: u8,
    /// Light color if emissive (RGB).
    pub light_color: [u8; 3],
    /// Whether this tile falls under gravity (sand, gravel).
    pub gravity: bool,
    /// Whether liquids can pass through this tile.
    pub liquid_permeable: bool,
    /// Items dropped when destroyed.
    pub drops: Vec<TileDrop>,
}

impl Default for TileProperties {
    fn default() -> Self {
        Self {
            name: String::new(),
            hardness: 1,
            solid: true,
            opaque: true,
            light_emission: 0,
            light_color: [255, 255, 255],
            gravity: false,
            liquid_permeable: false,
            drops: Vec::new(),
        }
    }
}

/// An item drop when a tile is destroyed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileDrop {
    /// Item type identifier.
    pub item_id: u32,
    /// Number of items dropped.
    pub count: u16,
    /// Drop chance (0.0 - 1.0).
    pub chance: f32,
}

// ---------------------------------------------------------------------------
// Tile property registry
// ---------------------------------------------------------------------------

/// Central registry mapping tile IDs to their properties.
pub struct TileRegistry {
    properties: Vec<TileProperties>,
}

impl TileRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            properties: Vec::new(),
        };
        // ID 0 = empty/air
        registry.register(TileProperties {
            name: "air".into(),
            solid: false,
            opaque: false,
            hardness: 0,
            ..Default::default()
        });
        registry
    }

    /// Register a new tile type. Returns its ID.
    pub fn register(&mut self, props: TileProperties) -> u32 {
        let id = self.properties.len() as u32;
        self.properties.push(props);
        id
    }

    /// Look up properties by tile ID.
    pub fn get(&self, tile_id: u32) -> Option<&TileProperties> {
        self.properties.get(tile_id as usize)
    }

    /// Number of registered tile types.
    pub fn count(&self) -> usize {
        self.properties.len()
    }
}

impl Default for TileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tile change events
// ---------------------------------------------------------------------------

/// Events emitted when tiles are modified.
#[derive(Clone, Debug)]
pub enum TileEvent {
    /// A tile was placed at a position.
    Placed {
        x: i32,
        y: i32,
        tile_id: u32,
    },
    /// A tile was destroyed at a position.
    Destroyed {
        x: i32,
        y: i32,
        old_tile_id: u32,
    },
    /// A neighbor of this tile changed (for redstone-like propagation).
    NeighborChanged {
        x: i32,
        y: i32,
    },
}

// ---------------------------------------------------------------------------
// DynamicTileWorld
// ---------------------------------------------------------------------------

/// Layer designation for foreground/background distinction (Terraria-style).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileLayer {
    /// Background walls (behind entities, less collision interaction).
    Background,
    /// Foreground blocks (main gameplay layer, collision, lighting).
    Foreground,
}

/// Dynamic tile world with runtime mutation, dirty tracking, and events.
///
/// Wraps two [`ChunkMap`]s (foreground + background) and adds:
/// - Per-tile-type property lookups via [`TileRegistry`]
/// - Dirty chunk tracking for incremental re-render/re-light
/// - Event queue for tile changes
pub struct DynamicTileWorld {
    pub foreground: ChunkMap,
    pub background: ChunkMap,
    pub registry: TileRegistry,
    dirty_chunks: HashSet<(TileLayer, ChunkCoord)>,
    events: Vec<TileEvent>,
}

impl DynamicTileWorld {
    pub fn new(tile_size: u32) -> Self {
        Self {
            foreground: ChunkMap::new(tile_size),
            background: ChunkMap::new(tile_size),
            registry: TileRegistry::new(),
            dirty_chunks: HashSet::new(),
            events: Vec::new(),
        }
    }

    /// Place a tile. Marks the chunk dirty and emits a Placed event.
    /// Returns the previous tile ID at that position.
    pub fn place_tile(&mut self, layer: TileLayer, x: i32, y: i32, tile_id: u32) -> u32 {
        let map = match layer {
            TileLayer::Foreground => &mut self.foreground,
            TileLayer::Background => &mut self.background,
        };

        let old = map.get_tile(x, y);
        map.set_tile(x, y, tile_id);

        let (coord, _, _) = ChunkMap::world_to_chunk(x, y);
        self.dirty_chunks.insert((layer, coord));

        self.events.push(TileEvent::Placed { x, y, tile_id });

        // Emit neighbor change events for adjacent tiles.
        for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
            self.events.push(TileEvent::NeighborChanged {
                x: x + dx,
                y: y + dy,
            });
        }

        old
    }

    /// Destroy a tile (replace with air/0). Returns the old tile ID.
    pub fn destroy_tile(&mut self, layer: TileLayer, x: i32, y: i32) -> u32 {
        let map = match layer {
            TileLayer::Foreground => &mut self.foreground,
            TileLayer::Background => &mut self.background,
        };

        let old = map.get_tile(x, y);
        if old == 0 {
            return 0; // Already air
        }

        map.set_tile(x, y, 0);

        let (coord, _, _) = ChunkMap::world_to_chunk(x, y);
        self.dirty_chunks.insert((layer, coord));

        self.events.push(TileEvent::Destroyed {
            x,
            y,
            old_tile_id: old,
        });

        for (dx, dy) in &[(0, -1), (0, 1), (-1, 0), (1, 0)] {
            self.events.push(TileEvent::NeighborChanged {
                x: x + dx,
                y: y + dy,
            });
        }

        old
    }

    /// Get a tile from the specified layer.
    pub fn get_tile(&self, layer: TileLayer, x: i32, y: i32) -> u32 {
        match layer {
            TileLayer::Foreground => self.foreground.get_tile(x, y),
            TileLayer::Background => self.background.get_tile(x, y),
        }
    }

    /// Get properties for a tile at a position.
    pub fn get_properties(&self, layer: TileLayer, x: i32, y: i32) -> Option<&TileProperties> {
        let id = self.get_tile(layer, x, y);
        self.registry.get(id)
    }

    /// Is a position solid (foreground)?
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        let id = self.foreground.get_tile(x, y);
        self.registry.get(id).map_or(false, |p| p.solid)
    }

    /// Is a position opaque (blocks light)?
    pub fn is_opaque(&self, x: i32, y: i32) -> bool {
        let id = self.foreground.get_tile(x, y);
        self.registry.get(id).map_or(false, |p| p.opaque)
    }

    /// Drain and return all dirty chunks since last call.
    pub fn take_dirty_chunks(&mut self) -> Vec<(TileLayer, ChunkCoord)> {
        self.dirty_chunks.drain().collect()
    }

    /// Drain and return all tile events since last call.
    pub fn take_events(&mut self) -> Vec<TileEvent> {
        std::mem::take(&mut self.events)
    }

    /// Process gravity tiles (sand, gravel falling down).
    /// Returns the number of tiles that moved.
    pub fn step_gravity(&mut self) -> u32 {
        let mut moved = 0u32;
        let coords: Vec<ChunkCoord> = self.foreground.loaded_chunks().copied().collect();

        for coord in coords {
            // Process bottom-to-top within each chunk so tiles fall correctly.
            for ly in (0..CHUNK_SIZE).rev() {
                for lx in 0..CHUNK_SIZE {
                    let wx = coord.cx * CHUNK_SIZE as i32 + lx as i32;
                    let wy = coord.cy * CHUNK_SIZE as i32 + ly as i32;

                    let tile_id = self.foreground.get_tile(wx, wy);
                    if tile_id == 0 {
                        continue;
                    }

                    let has_gravity = self
                        .registry
                        .get(tile_id)
                        .map_or(false, |p| p.gravity);

                    if !has_gravity {
                        continue;
                    }

                    // Check tile below.
                    let below = self.foreground.get_tile(wx, wy + 1);
                    if below == 0 {
                        self.foreground.set_tile(wx, wy, 0);
                        self.foreground.set_tile(wx, wy + 1, tile_id);

                        let (c1, _, _) = ChunkMap::world_to_chunk(wx, wy);
                        let (c2, _, _) = ChunkMap::world_to_chunk(wx, wy + 1);
                        self.dirty_chunks.insert((TileLayer::Foreground, c1));
                        self.dirty_chunks.insert((TileLayer::Foreground, c2));

                        moved += 1;
                    }
                }
            }
        }

        moved
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::Chunk;

    fn setup_world() -> DynamicTileWorld {
        let mut world = DynamicTileWorld::new(16);

        // Register some tile types.
        let _stone = world.registry.register(TileProperties {
            name: "stone".into(),
            hardness: 10,
            ..Default::default()
        }); // ID 1

        let _sand = world.registry.register(TileProperties {
            name: "sand".into(),
            hardness: 2,
            gravity: true,
            ..Default::default()
        }); // ID 2

        let _torch = world.registry.register(TileProperties {
            name: "torch".into(),
            solid: false,
            opaque: false,
            light_emission: 200,
            light_color: [255, 200, 100],
            ..Default::default()
        }); // ID 3

        // Load a chunk at origin.
        world
            .foreground
            .load_chunk(Chunk::new(ChunkCoord::new(0, 0)));
        world
            .background
            .load_chunk(Chunk::new(ChunkCoord::new(0, 0)));

        world
    }

    #[test]
    fn place_and_get() {
        let mut world = setup_world();
        world.place_tile(TileLayer::Foreground, 5, 5, 1);
        assert_eq!(world.get_tile(TileLayer::Foreground, 5, 5), 1);
        assert!(world.is_solid(5, 5));
    }

    #[test]
    fn destroy_emits_event() {
        let mut world = setup_world();
        world.place_tile(TileLayer::Foreground, 3, 3, 1);
        world.take_events(); // Clear place events.

        let old = world.destroy_tile(TileLayer::Foreground, 3, 3);
        assert_eq!(old, 1);
        assert_eq!(world.get_tile(TileLayer::Foreground, 3, 3), 0);

        let events = world.take_events();
        assert!(events.iter().any(|e| matches!(e, TileEvent::Destroyed { x: 3, y: 3, .. })));
    }

    #[test]
    fn dirty_tracking() {
        let mut world = setup_world();
        world.take_dirty_chunks(); // Clear initial.

        world.place_tile(TileLayer::Foreground, 1, 1, 2);
        let dirty = world.take_dirty_chunks();
        assert!(!dirty.is_empty());

        // After taking, should be empty.
        let dirty2 = world.take_dirty_chunks();
        assert!(dirty2.is_empty());
    }

    #[test]
    fn background_layer() {
        let mut world = setup_world();
        world.place_tile(TileLayer::Background, 5, 5, 1);
        assert_eq!(world.get_tile(TileLayer::Background, 5, 5), 1);
        // Foreground is still air.
        assert_eq!(world.get_tile(TileLayer::Foreground, 5, 5), 0);
    }

    #[test]
    fn gravity_step() {
        let mut world = setup_world();
        // Place sand at y=5, air below at y=6.
        world.foreground.set_tile(10, 5, 2);
        world.take_dirty_chunks();

        let moved = world.step_gravity();
        assert_eq!(moved, 1);
        assert_eq!(world.foreground.get_tile(10, 5), 0); // Sand fell.
        assert_eq!(world.foreground.get_tile(10, 6), 2); // Sand is now here.
    }

    #[test]
    fn tile_properties_lookup() {
        let world = setup_world();
        let props = world.registry.get(3).unwrap();
        assert_eq!(props.name, "torch");
        assert!(!props.solid);
        assert_eq!(props.light_emission, 200);
    }

    #[test]
    fn neighbor_events() {
        let mut world = setup_world();
        world.take_events();

        world.place_tile(TileLayer::Foreground, 10, 10, 1);
        let events = world.take_events();

        // Should have 1 Placed + 4 NeighborChanged.
        let neighbor_count = events
            .iter()
            .filter(|e| matches!(e, TileEvent::NeighborChanged { .. }))
            .count();
        assert_eq!(neighbor_count, 4);
    }
}
