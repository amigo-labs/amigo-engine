use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Level data — runtime representation of a loaded .amigo level
// ---------------------------------------------------------------------------

/// A tile layer loaded from a level file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileLayerData {
    pub name: String,
    pub tiles: Vec<u16>,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
}

impl TileLayerData {
    /// Tile at in-bounds coordinates, or 0 (empty) when `tiles` is shorter
    /// than `width * height` — only possible for a layer built by hand, since
    /// [`load_level_from_str`] rejects such files.
    fn tile(&self, x: u32, y: u32) -> u16 {
        let idx = u64::from(y) * u64::from(self.width) + u64::from(x);
        usize::try_from(idx)
            .ok()
            .and_then(|i| self.tiles.get(i))
            .copied()
            .unwrap_or(0)
    }
}

/// An entity placed in a level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityDef {
    pub entity_type: String,
    pub x: f32,
    pub y: f32,
    pub properties: HashMap<String, String>,
}

impl EntityDef {
    /// Get a property as a string.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    /// Get a property parsed as f32.
    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.properties.get(key).and_then(|s| s.parse().ok())
    }

    /// Get a property parsed as i32.
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.properties.get(key).and_then(|s| s.parse().ok())
    }

    /// Get a property parsed as bool.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.properties.get(key).and_then(|s| match s.as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        })
    }
}

/// A named path defined in a level (for AI patrol, camera rails, etc).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathDef {
    pub name: String,
    pub points: Vec<(f32, f32)>,
    pub closed: bool,
}

/// A zone/region defined in a level (for triggers, spawn areas, etc).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZoneDef {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub properties: HashMap<String, String>,
}

/// A fully loaded level ready for use.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoadedLevel {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub layers: Vec<TileLayerData>,
    pub entities: Vec<EntityDef>,
    pub paths: Vec<PathDef>,
    pub zones: Vec<ZoneDef>,
    pub metadata: HashMap<String, String>,
}

impl LoadedLevel {
    /// Find the first entity of a given type.
    pub fn find_entity(&self, entity_type: &str) -> Option<&EntityDef> {
        self.entities.iter().find(|e| e.entity_type == entity_type)
    }

    /// Find all entities of a given type.
    pub fn find_entities(&self, entity_type: &str) -> Vec<&EntityDef> {
        self.entities
            .iter()
            .filter(|e| e.entity_type == entity_type)
            .collect()
    }

    /// Find a path by name.
    pub fn find_path(&self, name: &str) -> Option<&PathDef> {
        self.paths.iter().find(|p| p.name == name)
    }

    /// Find a zone by name.
    pub fn find_zone(&self, name: &str) -> Option<&ZoneDef> {
        self.zones.iter().find(|z| z.name == name)
    }

    /// Check the level's dimensions and that every layer holds exactly
    /// `width * height` tiles.
    pub fn validate(&self) -> Result<(), String> {
        for layer in &self.layers {
            let expected = u64::from(layer.width) * u64::from(layer.height);
            if layer.tiles.len() as u64 != expected {
                return Err(format!(
                    "layer '{}' is {}x{} but has {} tiles (expected {expected})",
                    layer.name,
                    layer.width,
                    layer.height,
                    layer.tiles.len(),
                ));
            }
        }
        Ok(())
    }

    /// Get a layer by name.
    pub fn find_layer(&self, name: &str) -> Option<&TileLayerData> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Get tile at (x, y) from the first visible layer.
    pub fn tile_at(&self, x: u32, y: u32) -> u16 {
        for layer in &self.layers {
            if !layer.visible {
                continue;
            }
            if x < layer.width && y < layer.height {
                let tile = layer.tile(x, y);
                if tile != 0 {
                    return tile;
                }
            }
        }
        0
    }

    /// Get tile at (x, y) from a specific layer.
    pub fn tile_at_layer(&self, layer_name: &str, x: u32, y: u32) -> u16 {
        if let Some(layer) = self.find_layer(layer_name)
            && x < layer.width
            && y < layer.height
        {
            return layer.tile(x, y);
        }
        0
    }

    /// World-space position from tile coordinates.
    pub fn tile_to_world(&self, tx: u32, ty: u32) -> (f32, f32) {
        let ts = self.tile_size as f32;
        (tx as f32 * ts + ts * 0.5, ty as f32 * ts + ts * 0.5)
    }

    /// Tile coordinates from world-space position.
    pub fn world_to_tile(&self, wx: f32, wy: f32) -> (u32, u32) {
        let ts = self.tile_size as f32;
        ((wx / ts).max(0.0) as u32, (wy / ts).max(0.0) as u32)
    }

    /// Check if tile coordinates are in bounds.
    pub fn in_bounds(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height
    }

    /// World-space dimensions.
    pub fn world_size(&self) -> (f32, f32) {
        (
            self.width as f32 * self.tile_size as f32,
            self.height as f32 * self.tile_size as f32,
        )
    }

    /// Generate a flat collision array (true = solid) from a named layer.
    /// Any tile != 0 is considered solid.
    pub fn collision_from_layer(&self, layer_name: &str) -> Vec<bool> {
        if let Some(layer) = self.find_layer(layer_name) {
            layer.tiles.iter().map(|&t| t != 0).collect()
        } else {
            vec![false; (self.width * self.height) as usize]
        }
    }

    /// Extract spawn points (entities of type "spawn" or a given type).
    pub fn spawn_points(&self, entity_type: &str) -> Vec<(f32, f32)> {
        self.entities
            .iter()
            .filter(|e| e.entity_type == entity_type)
            .map(|e| (e.x, e.y))
            .collect()
    }
}

/// Load a level from a RON string (for embedding or testing).
///
/// Besides parsing, checks that every layer's `tiles` holds exactly
/// `width * height` entries, so a malformed level file is reported here
/// instead of producing out-of-range lookups later.
pub fn load_level_from_str(ron_str: &str) -> Result<LoadedLevel, String> {
    let level: LoadedLevel = ron::from_str(ron_str).map_err(|e| e.to_string())?;
    level.validate()?;
    Ok(level)
}

/// Load a level from a file path.
pub fn load_level_from_file(path: &std::path::Path) -> Result<LoadedLevel, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    load_level_from_str(&contents)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_level() -> LoadedLevel {
        LoadedLevel {
            name: "Test Level".to_string(),
            width: 4,
            height: 4,
            tile_size: 16,
            layers: vec![
                TileLayerData {
                    name: "ground".to_string(),
                    tiles: vec![1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1],
                    width: 4,
                    height: 4,
                    visible: true,
                },
                TileLayerData {
                    name: "collision".to_string(),
                    tiles: vec![1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1],
                    width: 4,
                    height: 4,
                    visible: false,
                },
            ],
            entities: vec![
                EntityDef {
                    entity_type: "PlayerSpawn".to_string(),
                    x: 24.0,
                    y: 24.0,
                    properties: HashMap::new(),
                },
                EntityDef {
                    entity_type: "Enemy".to_string(),
                    x: 40.0,
                    y: 40.0,
                    properties: {
                        let mut p = HashMap::new();
                        p.insert("hp".to_string(), "50".to_string());
                        p.insert("ai".to_string(), "patrol".to_string());
                        p
                    },
                },
                EntityDef {
                    entity_type: "Enemy".to_string(),
                    x: 56.0,
                    y: 24.0,
                    properties: HashMap::new(),
                },
            ],
            paths: vec![PathDef {
                name: "patrol_1".to_string(),
                points: vec![(24.0, 24.0), (40.0, 24.0), (40.0, 40.0)],
                closed: true,
            }],
            zones: vec![ZoneDef {
                name: "exit".to_string(),
                x: 48.0,
                y: 0.0,
                w: 16.0,
                h: 16.0,
                properties: {
                    let mut p = HashMap::new();
                    p.insert("target_scene".to_string(), "world_map".to_string());
                    p
                },
            }],
            metadata: HashMap::new(),
        }
    }

    // ── Entity queries ──────────────────────────────────────

    #[test]
    fn find_entity() {
        let level = sample_level();
        let spawn = level.find_entity("PlayerSpawn").unwrap();
        assert_eq!(spawn.x, 24.0);
        assert_eq!(spawn.y, 24.0);
    }

    #[test]
    fn find_entities() {
        let level = sample_level();
        let enemies = level.find_entities("Enemy");
        assert_eq!(enemies.len(), 2);
    }

    #[test]
    fn entity_properties() {
        let level = sample_level();
        let enemy = level.find_entities("Enemy")[0];
        assert_eq!(enemy.get("ai"), Some("patrol"));
        assert_eq!(enemy.get_f32("hp"), Some(50.0));
        assert_eq!(enemy.get_i32("hp"), Some(50));
    }

    // ── Paths and zones ─────────────────────────────────────

    #[test]
    fn find_path() {
        let level = sample_level();
        let path = level.find_path("patrol_1").unwrap();
        assert_eq!(path.points.len(), 3);
        assert!(path.closed);
    }

    #[test]
    fn find_zone() {
        let level = sample_level();
        let zone = level.find_zone("exit").unwrap();
        assert_eq!(zone.w, 16.0);
        assert_eq!(zone.properties.get("target_scene").unwrap(), "world_map");
    }

    // ── Tile access and coordinates ──────────────────────────

    #[test]
    fn tile_at() {
        let level = sample_level();
        assert_eq!(level.tile_at(0, 0), 1);
        assert_eq!(level.tile_at(1, 1), 0);
    }

    #[test]
    fn tile_at_layer() {
        let level = sample_level();
        assert_eq!(level.tile_at_layer("ground", 0, 0), 1);
        assert_eq!(level.tile_at_layer("ground", 1, 1), 0);
        assert_eq!(level.tile_at_layer("nonexistent", 0, 0), 0);
    }

    #[test]
    fn coordinate_conversion() {
        let level = sample_level();
        let (wx, wy) = level.tile_to_world(2, 3);
        assert_eq!(wx, 2.0 * 16.0 + 8.0);
        assert_eq!(wy, 3.0 * 16.0 + 8.0);

        let (tx, ty) = level.world_to_tile(wx, wy);
        assert_eq!(tx, 2);
        assert_eq!(ty, 3);
    }

    #[test]
    fn world_size() {
        let level = sample_level();
        let (w, h) = level.world_size();
        assert_eq!(w, 64.0);
        assert_eq!(h, 64.0);
    }

    // ── Collision, spawns, and serialization ────────────────

    #[test]
    fn collision_from_layer() {
        let level = sample_level();
        let collision = level.collision_from_layer("collision");
        assert_eq!(collision.len(), 16);
        assert!(collision[0]); // (0,0) = 1 = solid
        assert!(!collision[5]); // (1,1) = 0 = empty
    }

    #[test]
    fn spawn_points() {
        let level = sample_level();
        let spawns = level.spawn_points("Enemy");
        assert_eq!(spawns.len(), 2);
        assert_eq!(spawns[0], (40.0, 40.0));
    }

    #[test]
    fn in_bounds() {
        let level = sample_level();
        assert!(level.in_bounds(0, 0));
        assert!(level.in_bounds(3, 3));
        assert!(!level.in_bounds(4, 0));
    }

    #[test]
    fn serialization_roundtrip() {
        let level = sample_level();
        let ron = ron::ser::to_string_pretty(&level, ron::ser::PrettyConfig::default()).unwrap();
        let loaded: LoadedLevel = ron::from_str(&ron).unwrap();
        assert_eq!(loaded.name, "Test Level");
        assert_eq!(loaded.entities.len(), 3);
        assert_eq!(loaded.layers.len(), 2);
    }

    #[test]
    fn short_layer_data_is_rejected_on_load_and_never_panics() {
        let mut level = sample_level();
        level.layers[0].tiles.truncate(10); // 4x4 layer with 10 tiles
        let ron = ron::ser::to_string(&level).unwrap();
        let err = load_level_from_str(&ron).unwrap_err();
        assert!(err.contains("ground"), "{err}");

        // A layer built by hand bypasses the loader; lookups past the data
        // read as empty instead of indexing out of bounds.
        assert_eq!(level.tile_at_layer("ground", 3, 3), 0);
        assert_eq!(level.tile_at(3, 3), 0);
    }
}
