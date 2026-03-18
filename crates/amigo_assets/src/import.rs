//! Import pipeline for external formats (RS-25).
//!
//! Converts Tiled (`.tmx`), LDTK (`.ldtk`), and MML music files
//! into engine-native `.map.toml` and audio patterns.

use crate::descriptors::{EntityPlacement, MapDescriptor, MapLayerDef, TriggerZone};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Import pipeline errors.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
    #[error("Invalid file: {0}")]
    InvalidFile(String),
}

// ---------------------------------------------------------------------------
// Tiled .tmx / .json import
// ---------------------------------------------------------------------------

/// Minimal representation of a Tiled JSON map (exported from Tiled as JSON).
///
/// We import the JSON export rather than raw XML to avoid pulling in an XML parser.
#[derive(Debug, Deserialize)]
struct TiledMap {
    width: u32,
    height: u32,
    tilewidth: u32,
    tileheight: u32,
    #[serde(default)]
    layers: Vec<TiledLayer>,
    #[serde(default)]
    tilesets: Vec<TiledTilesetRef>,
}

#[derive(Debug, Deserialize)]
struct TiledLayer {
    name: String,
    #[serde(rename = "type")]
    layer_type: String,
    #[serde(default)]
    visible: bool,
    #[serde(default)]
    data: Vec<u32>,
    #[serde(default)]
    objects: Vec<TiledObject>,
    #[serde(default)]
    #[allow(dead_code)]
    width: u32,
    #[serde(default)]
    #[allow(dead_code)]
    height: u32,
}

#[derive(Debug, Deserialize)]
struct TiledObject {
    #[serde(default)]
    name: String,
    #[serde(rename = "type", default)]
    obj_type: String,
    x: f64,
    y: f64,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
    #[serde(default)]
    properties: Vec<TiledProperty>,
}

#[derive(Debug, Deserialize)]
struct TiledProperty {
    name: String,
    #[serde(rename = "type", default)]
    #[allow(dead_code)]
    prop_type: String,
    value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct TiledTilesetRef {
    #[serde(default)]
    firstgid: u32,
    #[serde(default)]
    source: String,
    #[serde(default)]
    name: String,
}

/// Import a Tiled map (JSON export format) into a `MapDescriptor`.
pub fn import_tiled(path: &Path) -> Result<MapDescriptor, ImportError> {
    let contents = std::fs::read_to_string(path)?;
    let tiled: TiledMap = serde_json::from_str(&contents)?;

    let mut layers = Vec::new();
    let mut entities = Vec::new();
    let mut triggers = Vec::new();

    // Collect tileset names
    let tileset_names: Vec<String> = tiled
        .tilesets
        .iter()
        .map(|ts| {
            if !ts.name.is_empty() {
                ts.name.clone()
            } else {
                // Extract name from source path
                Path::new(&ts.source)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("tileset_{}", ts.firstgid))
            }
        })
        .collect();

    for layer in &tiled.layers {
        match layer.layer_type.as_str() {
            "tilelayer" => {
                layers.push(MapLayerDef {
                    name: layer.name.clone(),
                    visible: layer.visible,
                    tiles: layer.data.clone(),
                });
            }
            "objectgroup" => {
                for obj in &layer.objects {
                    let props: Vec<(String, String)> = obj
                        .properties
                        .iter()
                        .map(|p| {
                            let val = match &p.value {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            (p.name.clone(), val)
                        })
                        .collect();

                    if obj.obj_type == "trigger" || obj.name.starts_with("trigger_") {
                        triggers.push(TriggerZone {
                            name: obj.name.clone(),
                            x: obj.x as f32,
                            y: obj.y as f32,
                            width: obj.width as f32,
                            height: obj.height as f32,
                            on_enter: props
                                .iter()
                                .find(|(k, _)| k == "on_enter")
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default(),
                            on_exit: props
                                .iter()
                                .find(|(k, _)| k == "on_exit")
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default(),
                        });
                    } else {
                        entities.push(EntityPlacement {
                            entity_type: if obj.obj_type.is_empty() {
                                obj.name.clone()
                            } else {
                                obj.obj_type.clone()
                            },
                            x: obj.x as f32,
                            y: obj.y as f32,
                            properties: props,
                        });
                    }
                }
            }
            "imagelayer" => {
                return Err(ImportError::UnsupportedFeature(
                    "Tiled image layers are not supported; use tile layers instead".into(),
                ));
            }
            _ => {
                // Skip unknown layer types gracefully
            }
        }
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "imported_map".into());

    Ok(MapDescriptor {
        name,
        width: tiled.width,
        height: tiled.height,
        tile_width: tiled.tilewidth,
        tile_height: tiled.tileheight,
        tilesets: tileset_names,
        layers,
        entities,
        triggers,
    })
}

// ---------------------------------------------------------------------------
// LDTK import
// ---------------------------------------------------------------------------

/// Minimal LDTK project structure (single-file JSON).
#[derive(Debug, Deserialize)]
struct LdtkProject {
    #[serde(rename = "defaultGridSize")]
    default_grid_size: u32,
    #[serde(default)]
    levels: Vec<LdtkLevel>,
}

#[derive(Debug, Deserialize)]
struct LdtkLevel {
    identifier: String,
    #[serde(rename = "pxWid")]
    px_wid: u32,
    #[serde(rename = "pxHei")]
    px_hei: u32,
    #[serde(rename = "layerInstances", default)]
    layer_instances: Option<Vec<LdtkLayerInstance>>,
}

#[derive(Debug, Deserialize)]
struct LdtkLayerInstance {
    #[serde(rename = "__identifier")]
    identifier: String,
    #[serde(rename = "__type")]
    layer_type: String,
    #[serde(rename = "__gridSize")]
    grid_size: u32,
    #[serde(rename = "__cWid")]
    c_wid: u32,
    #[serde(rename = "__cHei")]
    c_hei: u32,
    #[serde(rename = "intGridCsv", default)]
    int_grid_csv: Vec<u32>,
    #[serde(rename = "autoLayerTiles", default)]
    auto_layer_tiles: Vec<LdtkAutoTile>,
    #[serde(rename = "entityInstances", default)]
    entity_instances: Vec<LdtkEntityInstance>,
    visible: bool,
}

#[derive(Debug, Deserialize)]
struct LdtkAutoTile {
    #[serde(rename = "t")]
    tile_id: u32,
    #[serde(default)]
    px: Vec<i32>,
}

#[derive(Debug, Deserialize)]
struct LdtkEntityInstance {
    #[serde(rename = "__identifier")]
    identifier: String,
    px: Vec<i32>,
    #[serde(rename = "fieldInstances", default)]
    field_instances: Vec<LdtkFieldInstance>,
}

#[derive(Debug, Deserialize)]
struct LdtkFieldInstance {
    #[serde(rename = "__identifier")]
    identifier: String,
    #[serde(rename = "__value")]
    value: serde_json::Value,
}

/// Import an LDTK project file and convert its first level to a `MapDescriptor`.
///
/// For multi-level projects, returns the first level. Use `import_ldtk_level`
/// to import a specific level by name.
pub fn import_ldtk(path: &Path) -> Result<Vec<MapDescriptor>, ImportError> {
    let contents = std::fs::read_to_string(path)?;
    let project: LdtkProject = serde_json::from_str(&contents)?;

    let mut maps = Vec::new();
    for level in &project.levels {
        maps.push(convert_ldtk_level(level, project.default_grid_size)?);
    }

    if maps.is_empty() {
        return Err(ImportError::InvalidFile("LDTK project has no levels".into()));
    }

    Ok(maps)
}

fn convert_ldtk_level(level: &LdtkLevel, default_grid: u32) -> Result<MapDescriptor, ImportError> {
    let layer_instances = level
        .layer_instances
        .as_ref()
        .ok_or_else(|| ImportError::InvalidFile("Level has no embedded layers".into()))?;

    let mut layers = Vec::new();
    let mut entities = Vec::new();
    let mut grid_size = default_grid;

    // LDTK layers are bottom-to-top; we reverse for our top-to-bottom convention
    for li in layer_instances.iter().rev() {
        grid_size = li.grid_size;
        match li.layer_type.as_str() {
            "IntGrid" | "AutoLayer" | "Tiles" => {
                // Use intGridCsv if available, otherwise reconstruct from auto tiles
                let tiles = if !li.int_grid_csv.is_empty() {
                    li.int_grid_csv.clone()
                } else {
                    // Reconstruct tile grid from auto layer tiles
                    let mut grid = vec![0u32; (li.c_wid * li.c_hei) as usize];
                    for tile in &li.auto_layer_tiles {
                        if tile.px.len() >= 2 {
                            let cx = tile.px[0] as u32 / li.grid_size;
                            let cy = tile.px[1] as u32 / li.grid_size;
                            let idx = (cy * li.c_wid + cx) as usize;
                            if idx < grid.len() {
                                grid[idx] = tile.tile_id + 1; // 0 = empty
                            }
                        }
                    }
                    grid
                };

                layers.push(MapLayerDef {
                    name: li.identifier.clone(),
                    visible: li.visible,
                    tiles,
                });
            }
            "Entities" => {
                for ei in &li.entity_instances {
                    let x = ei.px.first().copied().unwrap_or(0) as f32;
                    let y = ei.px.get(1).copied().unwrap_or(0) as f32;
                    let props: Vec<(String, String)> = ei
                        .field_instances
                        .iter()
                        .map(|fi| (fi.identifier.clone(), fi.value.to_string()))
                        .collect();

                    entities.push(EntityPlacement {
                        entity_type: ei.identifier.clone(),
                        x,
                        y,
                        properties: props,
                    });
                }
            }
            _ => {
                // Skip unknown layer types
            }
        }
    }

    let width = level.px_wid / grid_size;
    let height = level.px_hei / grid_size;

    Ok(MapDescriptor {
        name: level.identifier.clone(),
        width,
        height,
        tile_width: grid_size,
        tile_height: grid_size,
        tilesets: Vec::new(),
        layers,
        entities,
        triggers: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// MML (Music Macro Language) import
// ---------------------------------------------------------------------------

/// A parsed MML pattern converted to engine audio data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MmlPattern {
    /// Pattern name (from filename or header).
    pub name: String,
    /// BPM (from tempo command or default 120).
    pub bpm: u32,
    /// Individual channel data.
    pub channels: Vec<MmlChannel>,
}

/// A single MML channel with its note events.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MmlChannel {
    pub index: usize,
    pub notes: Vec<MmlNote>,
}

/// A note event in the MML stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MmlNote {
    /// MIDI note number (60 = C4).
    pub midi_note: u8,
    /// Duration in ticks (96 ticks per quarter note).
    pub duration_ticks: u32,
    /// Velocity (0–127).
    pub velocity: u8,
}

/// Import an MML file into audio patterns.
///
/// MML syntax supported:
/// - Notes: `c d e f g a b` (with `+`/`#` for sharp, `-` for flat)
/// - Octave: `o4` sets octave, `>` up, `<` down
/// - Length: `l8` sets default, number after note overrides (e.g. `c4`)
/// - Rest: `r`
/// - Tempo: `t120`
/// - Volume: `v15` (0–15, mapped to MIDI velocity)
/// - Channels separated by `;`
pub fn import_mml(path: &Path) -> Result<MmlPattern, ImportError> {
    let contents = std::fs::read_to_string(path)?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "pattern".into());

    parse_mml(&name, &contents)
}

/// Parse MML string into a pattern.
pub fn parse_mml(name: &str, mml: &str) -> Result<MmlPattern, ImportError> {
    let mut bpm = 120u32;
    let mut channels = Vec::new();

    // Strip comments (lines starting with # or //)
    let clean: String = mml
        .lines()
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.starts_with('#') && !trimmed.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Split channels by ';'
    let channel_strs: Vec<&str> = clean.split(';').collect();

    for (idx, ch_str) in channel_strs.iter().enumerate() {
        let trimmed = ch_str.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (notes, detected_bpm) = parse_mml_channel(trimmed)?;
        if detected_bpm > 0 {
            bpm = detected_bpm;
        }

        channels.push(MmlChannel {
            index: idx,
            notes,
        });
    }

    Ok(MmlPattern {
        name: name.to_string(),
        bpm,
        channels,
    })
}

fn parse_mml_channel(input: &str) -> Result<(Vec<MmlNote>, u32), ImportError> {
    let mut notes = Vec::new();
    let mut octave: i32 = 4;
    let mut default_length: u32 = 4; // quarter note
    let mut velocity: u8 = 100;
    let mut bpm: u32 = 0;
    let ticks_per_quarter = 96u32;

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            'c' | 'd' | 'e' | 'f' | 'g' | 'a' | 'b' => {
                let base_note = match ch {
                    'c' => 0,
                    'd' => 2,
                    'e' => 4,
                    'f' => 5,
                    'g' => 7,
                    'a' => 9,
                    'b' => 11,
                    _ => unreachable!(),
                };
                i += 1;

                // Check for sharp/flat
                let mut semitone_offset: i32 = 0;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '#') {
                    semitone_offset = 1;
                    i += 1;
                } else if i < chars.len() && chars[i] == '-' {
                    semitone_offset = -1;
                    i += 1;
                }

                // Check for length number
                let length = parse_number(&chars, &mut i).unwrap_or(default_length);

                let midi = ((octave + 1) * 12 + base_note + semitone_offset) as u8;
                let duration = ticks_per_quarter * 4 / length;

                notes.push(MmlNote {
                    midi_note: midi,
                    duration_ticks: duration,
                    velocity,
                });
            }
            'r' => {
                // Rest
                i += 1;
                let length = parse_number(&chars, &mut i).unwrap_or(default_length);
                let duration = ticks_per_quarter * 4 / length;
                notes.push(MmlNote {
                    midi_note: 0,
                    duration_ticks: duration,
                    velocity: 0,
                });
            }
            'o' => {
                i += 1;
                if let Some(n) = parse_number(&chars, &mut i) {
                    octave = n as i32;
                }
            }
            '>' => {
                octave += 1;
                i += 1;
            }
            '<' => {
                octave -= 1;
                i += 1;
            }
            'l' => {
                i += 1;
                if let Some(n) = parse_number(&chars, &mut i) {
                    default_length = n;
                }
            }
            'v' => {
                i += 1;
                if let Some(n) = parse_number(&chars, &mut i) {
                    // MML volume 0-15 → MIDI velocity 0-127
                    velocity = ((n.min(15) as f32 / 15.0) * 127.0) as u8;
                }
            }
            't' => {
                i += 1;
                if let Some(n) = parse_number(&chars, &mut i) {
                    bpm = n;
                }
            }
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            _ => {
                // Skip unknown characters
                i += 1;
            }
        }
    }

    Ok((notes, bpm))
}

fn parse_number(chars: &[char], i: &mut usize) -> Option<u32> {
    let start = *i;
    while *i < chars.len() && chars[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i > start {
        let s: String = chars[start..*i].iter().collect();
        s.parse().ok()
    } else {
        None
    }
}

/// Serialize a `MapDescriptor` to TOML and write to a file.
pub fn write_map_toml(map: &MapDescriptor, path: &Path) -> Result<(), ImportError> {
    let toml_str =
        toml::to_string_pretty(map).map_err(|e| ImportError::InvalidFile(e.to_string()))?;
    std::fs::write(path, toml_str)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tiled import ────────────────────────────────────────────

    #[test]
    fn parse_tiled_json() {
        let json = r#"{
            "width": 10, "height": 8,
            "tilewidth": 16, "tileheight": 16,
            "layers": [
                {
                    "name": "ground",
                    "type": "tilelayer",
                    "visible": true,
                    "data": [1,2,3,4,5,6,7,8,9,10],
                    "width": 10, "height": 1
                },
                {
                    "name": "objects",
                    "type": "objectgroup",
                    "visible": true,
                    "objects": [
                        {
                            "name": "player_spawn",
                            "type": "spawn",
                            "x": 32.0, "y": 48.0,
                            "width": 0.0, "height": 0.0,
                            "properties": []
                        },
                        {
                            "name": "trigger_door",
                            "type": "trigger",
                            "x": 64.0, "y": 0.0,
                            "width": 16.0, "height": 16.0,
                            "properties": [
                                {"name": "on_enter", "type": "string", "value": "open_door"}
                            ]
                        }
                    ]
                }
            ],
            "tilesets": [
                {"firstgid": 1, "source": "terrain.tsx", "name": "terrain"}
            ]
        }"#;

        let tmp = std::env::temp_dir().join("test_tiled.json");
        std::fs::write(&tmp, json).unwrap();

        let map = import_tiled(&tmp).unwrap();
        assert_eq!(map.width, 10);
        assert_eq!(map.height, 8);
        assert_eq!(map.tile_width, 16);
        assert_eq!(map.layers.len(), 1);
        assert_eq!(map.layers[0].name, "ground");
        assert_eq!(map.layers[0].tiles.len(), 10);
        assert_eq!(map.entities.len(), 1);
        assert_eq!(map.entities[0].entity_type, "spawn");
        assert_eq!(map.triggers.len(), 1);
        assert_eq!(map.triggers[0].name, "trigger_door");
        assert_eq!(map.triggers[0].on_enter, "open_door");
        assert_eq!(map.tilesets, vec!["terrain"]);

        let _ = std::fs::remove_file(&tmp);
    }

    // ── LDTK import ─────────────────────────────────────────────

    #[test]
    fn parse_ldtk_json() {
        let json = r#"{
            "defaultGridSize": 16,
            "levels": [
                {
                    "identifier": "Level_0",
                    "pxWid": 160,
                    "pxHei": 128,
                    "layerInstances": [
                        {
                            "__identifier": "Tiles",
                            "__type": "IntGrid",
                            "__gridSize": 16,
                            "__cWid": 10,
                            "__cHei": 8,
                            "intGridCsv": [1,0,1,0,1,0,1,0,1,0],
                            "autoLayerTiles": [],
                            "entityInstances": [],
                            "visible": true
                        },
                        {
                            "__identifier": "Entities",
                            "__type": "Entities",
                            "__gridSize": 16,
                            "__cWid": 10,
                            "__cHei": 8,
                            "intGridCsv": [],
                            "autoLayerTiles": [],
                            "entityInstances": [
                                {
                                    "__identifier": "Player",
                                    "px": [32, 48],
                                    "fieldInstances": [
                                        {"__identifier": "hp", "__value": 100}
                                    ]
                                }
                            ],
                            "visible": true
                        }
                    ]
                }
            ]
        }"#;

        let tmp = std::env::temp_dir().join("test_ldtk.ldtk");
        std::fs::write(&tmp, json).unwrap();

        let maps = import_ldtk(&tmp).unwrap();
        assert_eq!(maps.len(), 1);
        let map = &maps[0];
        assert_eq!(map.name, "Level_0");
        assert_eq!(map.width, 10);
        assert_eq!(map.height, 8);
        assert_eq!(map.layers.len(), 1);
        assert_eq!(map.layers[0].name, "Tiles");
        assert_eq!(map.entities.len(), 1);
        assert_eq!(map.entities[0].entity_type, "Player");
        assert_eq!(map.entities[0].x, 32.0);

        let _ = std::fs::remove_file(&tmp);
    }

    // ── MML import ──────────────────────────────────────────────

    #[test]
    fn parse_mml_basic() {
        let mml = "t140 o4 l8 c d e f g a b > c";
        let pattern = parse_mml("test", mml).unwrap();
        assert_eq!(pattern.bpm, 140);
        assert_eq!(pattern.channels.len(), 1);
        assert_eq!(pattern.channels[0].notes.len(), 8);
        // C4 = MIDI 60
        assert_eq!(pattern.channels[0].notes[0].midi_note, 60);
        // C5 = MIDI 72
        assert_eq!(pattern.channels[0].notes[7].midi_note, 72);
    }

    #[test]
    fn parse_mml_sharps_and_flats() {
        let mml = "o4 c+ d- e";
        let pattern = parse_mml("test", mml).unwrap();
        let notes = &pattern.channels[0].notes;
        assert_eq!(notes[0].midi_note, 61); // C#4
        assert_eq!(notes[1].midi_note, 61); // Db4 = C#4
        assert_eq!(notes[2].midi_note, 64); // E4
    }

    #[test]
    fn parse_mml_multi_channel() {
        let mml = "o4 c d e; o5 g a b";
        let pattern = parse_mml("test", mml).unwrap();
        assert_eq!(pattern.channels.len(), 2);
        assert_eq!(pattern.channels[0].notes.len(), 3);
        assert_eq!(pattern.channels[1].notes.len(), 3);
    }

    #[test]
    fn parse_mml_rest() {
        let mml = "o4 c r d";
        let pattern = parse_mml("test", mml).unwrap();
        assert_eq!(pattern.channels[0].notes.len(), 3);
        assert_eq!(pattern.channels[0].notes[1].midi_note, 0); // rest
        assert_eq!(pattern.channels[0].notes[1].velocity, 0);
    }

    #[test]
    fn parse_mml_volume() {
        let mml = "v8 c v15 d";
        let pattern = parse_mml("test", mml).unwrap();
        let notes = &pattern.channels[0].notes;
        // v8 → ~67
        assert!(notes[0].velocity > 60 && notes[0].velocity < 75);
        // v15 → 127
        assert_eq!(notes[1].velocity, 127);
    }

    #[test]
    fn parse_mml_note_lengths() {
        let mml = "l4 c8 d c";
        let pattern = parse_mml("test", mml).unwrap();
        let notes = &pattern.channels[0].notes;
        // c8 = eighth note = 48 ticks
        assert_eq!(notes[0].duration_ticks, 48);
        // d (default l4) = quarter = 96 ticks
        assert_eq!(notes[1].duration_ticks, 96);
        // c (default l4) = quarter = 96 ticks
        assert_eq!(notes[2].duration_ticks, 96);
    }

    // ── Error handling ──────────────────────────────────────────

    #[test]
    fn tiled_unsupported_image_layer() {
        let json = r#"{
            "width": 10, "height": 8,
            "tilewidth": 16, "tileheight": 16,
            "layers": [
                {
                    "name": "bg",
                    "type": "imagelayer",
                    "visible": true,
                    "objects": []
                }
            ],
            "tilesets": []
        }"#;

        let tmp = std::env::temp_dir().join("test_tiled_img.json");
        std::fs::write(&tmp, json).unwrap();

        let result = import_tiled(&tmp);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("image layers"));

        let _ = std::fs::remove_file(&tmp);
    }
}
