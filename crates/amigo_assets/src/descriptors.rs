//! TOML asset descriptors — metadata layer between raw files and the engine.
//!
//! Each descriptor type corresponds to a `.xyz.toml` file sitting next to the
//! raw asset (e.g. `player.sprite.toml` beside `player.png`).

use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// .sprite.toml
// ---------------------------------------------------------------------------

/// Descriptor loaded from a `.sprite.toml` file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteDescriptor {
    /// Display name (defaults to filename stem).
    #[serde(default)]
    pub name: String,
    /// Source image path (relative to descriptor).
    pub image: String,
    /// Pixel dimensions of a single frame.
    pub frame_width: u32,
    pub frame_height: u32,
    /// Origin / pivot point (0.0–1.0, default center-bottom).
    #[serde(default = "default_origin")]
    pub origin: (f32, f32),
    /// Named animations.
    #[serde(default)]
    pub animations: Vec<AnimationDef>,
    /// Optional collision hitbox (relative to origin).
    #[serde(default)]
    pub hitbox: Option<HitboxDef>,
}

fn default_origin() -> (f32, f32) {
    (0.5, 1.0)
}

/// An animation defined in a sprite descriptor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimationDef {
    pub name: String,
    /// First frame index (0-based).
    pub start: u32,
    /// Number of frames.
    pub count: u32,
    /// Frames per second.
    #[serde(default = "default_fps")]
    pub fps: f32,
    /// Whether the animation loops.
    #[serde(default = "default_true")]
    pub looping: bool,
}

fn default_fps() -> f32 {
    10.0
}
fn default_true() -> bool {
    true
}

/// Axis-aligned hitbox definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HitboxDef {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

// ---------------------------------------------------------------------------
// .tileset.toml
// ---------------------------------------------------------------------------

/// Descriptor loaded from a `.tileset.toml` file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TilesetDescriptor {
    #[serde(default)]
    pub name: String,
    /// Source image path.
    pub image: String,
    /// Tile size in pixels.
    pub tile_width: u32,
    pub tile_height: u32,
    /// Number of columns in the tileset image.
    #[serde(default)]
    pub columns: u32,
    /// Spacing between tiles in pixels.
    #[serde(default)]
    pub spacing: u32,
    /// Margin around the tileset image.
    #[serde(default)]
    pub margin: u32,
    /// Auto-tile rules (e.g. Wang tiles, blob tileset).
    #[serde(default)]
    pub auto_tile_rules: Vec<AutoTileRule>,
    /// Per-tile properties.
    #[serde(default)]
    pub tile_properties: Vec<TileProperty>,
}

/// An auto-tile rule mapping neighbor bitmask to tile index.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoTileRule {
    /// 8-bit bitmask of neighbors (N, NE, E, SE, S, SW, W, NW).
    pub bitmask: u8,
    /// Tile index to use when this bitmask matches.
    pub tile_id: u32,
}

/// Per-tile custom property.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileProperty {
    pub tile_id: u32,
    pub key: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// .map.toml
// ---------------------------------------------------------------------------

/// Descriptor loaded from a `.map.toml` file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapDescriptor {
    #[serde(default)]
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    /// Tileset references (by name or path).
    #[serde(default)]
    pub tilesets: Vec<String>,
    /// Tile layers.
    #[serde(default)]
    pub layers: Vec<MapLayerDef>,
    /// Entity placements.
    #[serde(default)]
    pub entities: Vec<EntityPlacement>,
    /// Named trigger zones.
    #[serde(default)]
    pub triggers: Vec<TriggerZone>,
}

/// A tile layer in a map descriptor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapLayerDef {
    pub name: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Flat array of tile IDs (row-major).
    pub tiles: Vec<u32>,
}

/// An entity placed in the map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityPlacement {
    /// Entity type / prefab name.
    pub entity_type: String,
    pub x: f32,
    pub y: f32,
    /// Custom properties.
    #[serde(default)]
    pub properties: Vec<(String, String)>,
}

/// A named trigger zone in the map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriggerZone {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub on_enter: String,
    #[serde(default)]
    pub on_exit: String,
}

// ---------------------------------------------------------------------------
// .entity.toml
// ---------------------------------------------------------------------------

/// Descriptor loaded from a `.entity.toml` file (entity prefab).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityDescriptor {
    pub name: String,
    /// Sprite descriptor path (optional).
    #[serde(default)]
    pub sprite: Option<String>,
    /// Default components as key-value pairs.
    #[serde(default)]
    pub components: Vec<ComponentDef>,
    /// Tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A component definition in an entity prefab.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentDef {
    pub name: String,
    /// Serialized component data (TOML inline table or value).
    #[serde(default = "default_toml_table")]
    pub data: toml::Value,
}

fn default_toml_table() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

// ---------------------------------------------------------------------------
// Loader functions
// ---------------------------------------------------------------------------

/// Load a descriptor from a TOML file.
pub fn load_sprite_descriptor(path: &Path) -> Result<SpriteDescriptor, DescriptorError> {
    let contents = std::fs::read_to_string(path)?;
    let mut desc: SpriteDescriptor = toml::from_str(&contents)?;
    if desc.name.is_empty() {
        desc.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .trim_end_matches(".sprite")
            .to_string();
    }
    Ok(desc)
}

pub fn load_tileset_descriptor(path: &Path) -> Result<TilesetDescriptor, DescriptorError> {
    let contents = std::fs::read_to_string(path)?;
    let mut desc: TilesetDescriptor = toml::from_str(&contents)?;
    if desc.name.is_empty() {
        desc.name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .trim_end_matches(".tileset")
            .to_string();
    }
    Ok(desc)
}

pub fn load_map_descriptor(path: &Path) -> Result<MapDescriptor, DescriptorError> {
    let contents = std::fs::read_to_string(path)?;
    let desc: MapDescriptor = toml::from_str(&contents)?;
    Ok(desc)
}

pub fn load_entity_descriptor(path: &Path) -> Result<EntityDescriptor, DescriptorError> {
    let contents = std::fs::read_to_string(path)?;
    let desc: EntityDescriptor = toml::from_str(&contents)?;
    Ok(desc)
}

/// Errors from loading descriptors.
#[derive(Debug, thiserror::Error)]
pub enum DescriptorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sprite Descriptor Parsing ──────────────────────────────

    #[test]
    fn parse_sprite_descriptor() {
        let toml_str = r#"
            image = "player.png"
            frame_width = 32
            frame_height = 32

            [[animations]]
            name = "idle"
            start = 0
            count = 4
            fps = 8.0

            [[animations]]
            name = "run"
            start = 4
            count = 6
            fps = 12.0
            looping = true

            [hitbox]
            x = -8.0
            y = -16.0
            width = 16.0
            height = 32.0
        "#;

        let desc: SpriteDescriptor = toml::from_str(toml_str).unwrap();
        assert_eq!(desc.image, "player.png");
        assert_eq!(desc.frame_width, 32);
        assert_eq!(desc.animations.len(), 2);
        assert_eq!(desc.animations[0].name, "idle");
        assert_eq!(desc.animations[1].fps, 12.0);
        assert!(desc.hitbox.is_some());
        assert_eq!(desc.origin, (0.5, 1.0)); // default
    }

    // ── Tileset Descriptor Parsing ─────────────────────────────

    #[test]
    fn parse_tileset_descriptor() {
        let toml_str = r#"
            name = "grass"
            image = "grass_tileset.png"
            tile_width = 16
            tile_height = 16
            columns = 10
            spacing = 1

            [[auto_tile_rules]]
            bitmask = 0xFF
            tile_id = 0

            [[tile_properties]]
            tile_id = 5
            key = "collision"
            value = "solid"
        "#;

        let desc: TilesetDescriptor = toml::from_str(toml_str).unwrap();
        assert_eq!(desc.tile_width, 16);
        assert_eq!(desc.columns, 10);
        assert_eq!(desc.auto_tile_rules.len(), 1);
        assert_eq!(desc.tile_properties.len(), 1);
    }

    // ── Map Descriptor Parsing ───────────────────────────────────

    #[test]
    fn parse_map_descriptor() {
        let toml_str = r#"
            name = "level1"
            width = 40
            height = 23
            tile_width = 16
            tile_height = 16
            tilesets = ["grass", "stone"]

            [[layers]]
            name = "ground"
            tiles = [0, 1, 2, 3]

            [[entities]]
            entity_type = "player_spawn"
            x = 100.0
            y = 200.0

            [[triggers]]
            name = "exit_zone"
            x = 500.0
            y = 0.0
            width = 32.0
            height = 368.0
            on_enter = "next_level"
        "#;

        let desc: MapDescriptor = toml::from_str(toml_str).unwrap();
        assert_eq!(desc.width, 40);
        assert_eq!(desc.layers.len(), 1);
        assert_eq!(desc.entities.len(), 1);
        assert_eq!(desc.triggers.len(), 1);
        assert_eq!(desc.triggers[0].on_enter, "next_level");
    }

    // ── Entity Descriptor Parsing ─────────────────────────────

    #[test]
    fn parse_entity_descriptor() {
        let toml_str = r#"
            name = "skeleton_warrior"
            sprite = "skeleton.sprite.toml"
            tags = ["enemy", "undead", "melee"]

            [[components]]
            name = "health"
            data = { max = 50, current = 50 }

            [[components]]
            name = "combat"
            data = { damage = 10, range = 1.5 }
        "#;

        let desc: EntityDescriptor = toml::from_str(toml_str).unwrap();
        assert_eq!(desc.name, "skeleton_warrior");
        assert_eq!(desc.tags.len(), 3);
        assert_eq!(desc.components.len(), 2);
    }
}
