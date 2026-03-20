//! Central format registry for RON-based asset definition files.
//!
//! Discovers and loads `.style.ron`, `.music.ron`, `.audio_style.ron`, and
//! `.sfx.ron` files from a directory tree, then provides lookup and
//! cross-reference validation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::warn;

// ---------------------------------------------------------------------------
// Error / Warning types
// ---------------------------------------------------------------------------

/// Errors produced during RON asset loading.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error in {file}: {message}")]
    Parse { file: PathBuf, message: String },
    #[error("Unknown extension: {0}")]
    UnknownExtension(String),
}

/// Non-fatal validation warning.
#[derive(Debug)]
pub struct FormatWarning {
    pub file: PathBuf,
    pub message: String,
}

// ---------------------------------------------------------------------------
// .style.ron — re-export-compatible mirror of artgen::StyleDef
// ---------------------------------------------------------------------------

/// Art generation style loaded from `.style.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StyleDef {
    pub name: String,
    pub checkpoint: String,
    pub lora: Option<(String, f32)>,
    pub palette: Vec<String>,
    pub prompt_prefix: String,
    pub negative_prompt: String,
    pub default_size: (u32, u32),
    pub steps: u32,
    pub cfg_scale: f32,
    pub post_processing: PostProcessConfig,
    #[serde(default)]
    pub reference_images: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostProcessConfig {
    pub palette_clamp: bool,
    pub remove_anti_aliasing: bool,
    pub add_outline: bool,
    pub outline_color: String,
    pub outline_mode: OutlineMode,
    pub cleanup_transparency: bool,
    pub tile_edge_check: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OutlineMode {
    Outer,
    Inner,
    Both,
}

impl StyleDef {
    pub fn load(path: &Path) -> Result<Self, FormatError> {
        let contents = std::fs::read_to_string(path)?;
        ron::from_str(&contents).map_err(|e| FormatError::Parse {
            file: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// Parse `"#RRGGBB"` to `[u8; 3]`.
    pub fn parse_hex_color(hex: &str) -> Option<[u8; 3]> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some([r, g, b])
    }
}

// ---------------------------------------------------------------------------
// .music.ron
// ---------------------------------------------------------------------------

/// Top-level music configuration loaded from `.music.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicConfig {
    pub bpm: u32,
    pub beats_per_bar: u32,
    pub sections: Vec<SectionDef>,
    #[serde(default)]
    pub transitions: HashMap<(String, String), MusicTransition>,
    #[serde(default)]
    pub stingers: Vec<StingerDef>,
}

/// A section within a music configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionDef {
    pub name: String,
    pub layers: Vec<LayerDef>,
}

/// A single audio layer inside a section.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerDef {
    pub name: String,
    pub file: String,
    #[serde(default = "default_base_volume")]
    pub base_volume: f32,
    #[serde(default)]
    pub rule: Option<LayerRule>,
}

fn default_base_volume() -> f32 {
    1.0
}

/// Rules that drive adaptive layer volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LayerRule {
    Lerp { param: String, from: f32, to: f32 },
    Threshold { param: String, above: f32, fade_seconds: f32 },
    Toggle { param: String, fade_seconds: f32 },
}

/// Transition type between music sections.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MusicTransition {
    CrossfadeOnBar { bars: u32 },
    FadeOutThenPlay { fade_bars: u32 },
    CutOnBar,
    StingerThen { stinger: String, then: Box<MusicTransition> },
    LayerSwap { bars_per_layer: u32 },
}

/// A one-shot musical cue.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StingerDef {
    pub name: String,
    pub file: String,
    #[serde(default)]
    pub quantize: StingerQuantize,
}

/// When a stinger fires relative to the beat grid.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum StingerQuantize {
    #[default]
    Beat,
    Bar,
    Immediate,
}

impl MusicConfig {
    pub fn load(path: &Path) -> Result<Self, FormatError> {
        let contents = std::fs::read_to_string(path)?;
        ron::from_str(&contents).map_err(|e| FormatError::Parse {
            file: path.to_path_buf(),
            message: e.to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// .audio_style.ron
// ---------------------------------------------------------------------------

/// Per-world audio style preset loaded from `.audio_style.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldAudioStyle {
    pub name: String,
    pub genre: String,
    #[serde(default)]
    pub genre_tags: Vec<String>,
    pub default_bpm: u32,
    #[serde(default)]
    pub sfx_style: String,
    #[serde(default)]
    pub key_instruments: Vec<String>,
}

impl WorldAudioStyle {
    pub fn load(path: &Path) -> Result<Self, FormatError> {
        let contents = std::fs::read_to_string(path)?;
        ron::from_str(&contents).map_err(|e| FormatError::Parse {
            file: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    pub fn load_all(dir: &Path) -> Vec<Self> {
        let mut styles = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path_has_compound_ext(&path, "audio_style.ron") {
                    match Self::load(&path) {
                        Ok(s) => styles.push(s),
                        Err(e) => warn!("Failed to load audio style {}: {e}", path.display()),
                    }
                }
            }
        }
        styles
    }
}

// ---------------------------------------------------------------------------
// .sfx.ron
// ---------------------------------------------------------------------------

/// SFX category tag.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SfxCategory {
    Gameplay,
    UI,
    Ambient,
    Impact,
    Explosion,
    Magic,
    Voice,
    Custom(String),
}

/// A single sound effect definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxDef {
    pub files: Vec<String>,
    #[serde(default = "default_volume")]
    pub volume: f32,
    #[serde(default)]
    pub pitch_variance: f32,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
    #[serde(default)]
    pub cooldown: Option<f32>,
    #[serde(default = "default_sfx_category")]
    pub category: SfxCategory,
}

fn default_volume() -> f32 {
    1.0
}

fn default_max_concurrent() -> u32 {
    4
}

fn default_sfx_category() -> SfxCategory {
    SfxCategory::Gameplay
}

/// A bundle of named SFX definitions loaded from `.sfx.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxBundle {
    pub sounds: HashMap<String, SfxDef>,
}

impl SfxBundle {
    pub fn load(path: &Path) -> Result<Self, FormatError> {
        let contents = std::fs::read_to_string(path)?;
        ron::from_str(&contents).map_err(|e| FormatError::Parse {
            file: path.to_path_buf(),
            message: e.to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// FormatRegistry
// ---------------------------------------------------------------------------

/// Unified loader and lookup for all RON-based asset definition formats.
pub struct FormatRegistry {
    styles: Vec<StyleDef>,
    music_configs: HashMap<String, MusicConfig>,
    audio_styles: Vec<WorldAudioStyle>,
    sfx_bundles: Vec<SfxBundle>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        Self {
            styles: Vec::new(),
            music_configs: HashMap::new(),
            audio_styles: Vec::new(),
            sfx_bundles: Vec::new(),
        }
    }

    /// Scan a directory tree recursively and load all recognised `.ron` files
    /// by their compound extension: `.style.ron`, `.music.ron`,
    /// `.audio_style.ron`, `.sfx.ron`.
    ///
    /// Unrecognised `.ron` files are silently skipped.
    pub fn load_directory(&mut self, root: &Path) -> Result<(), FormatError> {
        self.walk_dir(root)
    }

    // -- lookups --

    pub fn style(&self, name: &str) -> Option<&StyleDef> {
        self.styles.iter().find(|s| s.name == name)
    }

    pub fn styles(&self) -> &[StyleDef] {
        &self.styles
    }

    pub fn music_config(&self, name: &str) -> Option<&MusicConfig> {
        self.music_configs.get(name)
    }

    pub fn music_configs(&self) -> &HashMap<String, MusicConfig> {
        &self.music_configs
    }

    pub fn audio_style(&self, name: &str) -> Option<&WorldAudioStyle> {
        self.audio_styles.iter().find(|s| s.name == name)
    }

    pub fn audio_styles(&self) -> &[WorldAudioStyle] {
        &self.audio_styles
    }

    pub fn sfx_bundle(&self, index: usize) -> Option<&SfxBundle> {
        self.sfx_bundles.get(index)
    }

    pub fn sfx_bundles(&self) -> &[SfxBundle] {
        &self.sfx_bundles
    }

    // -- validation --

    /// Validate cross-references and file existence.
    ///
    /// Warnings are collected, not fatal -- allows partial asset sets during
    /// development.
    pub fn validate(&self, asset_root: &Path) -> Vec<FormatWarning> {
        let mut warnings = Vec::new();

        // Validate style palette hex colours.
        for style in &self.styles {
            for (i, hex) in style.palette.iter().enumerate() {
                if StyleDef::parse_hex_color(hex).is_none() {
                    warnings.push(FormatWarning {
                        file: PathBuf::from(format!("<style:{}>", style.name)),
                        message: format!("Invalid palette hex at index {i}: {hex:?}"),
                    });
                }
            }
        }

        // Validate music configs.
        for (cfg_name, cfg) in &self.music_configs {
            let file_label = PathBuf::from(format!("<music:{cfg_name}>"));

            // Check for duplicate section names.
            let mut seen_sections = std::collections::HashSet::new();
            for section in &cfg.sections {
                if !seen_sections.insert(&section.name) {
                    warnings.push(FormatWarning {
                        file: file_label.clone(),
                        message: format!("Duplicate section name: {:?}", section.name),
                    });
                }
                // Check that referenced audio files exist.
                for layer in &section.layers {
                    let full = asset_root.join(&layer.file);
                    if !full.exists() {
                        warnings.push(FormatWarning {
                            file: file_label.clone(),
                            message: format!(
                                "Layer {:?} references missing file: {}",
                                layer.name,
                                layer.file
                            ),
                        });
                    }
                }
            }

            // Check stinger file existence.
            for stinger in &cfg.stingers {
                let full = asset_root.join(&stinger.file);
                if !full.exists() {
                    warnings.push(FormatWarning {
                        file: file_label.clone(),
                        message: format!(
                            "Stinger {:?} references missing file: {}",
                            stinger.name,
                            stinger.file
                        ),
                    });
                }
            }

            // Validate StingerThen references.
            let stinger_names: std::collections::HashSet<&str> =
                cfg.stingers.iter().map(|s| s.name.as_str()).collect();
            for ((_from, _to), transition) in &cfg.transitions {
                check_stinger_refs(transition, &stinger_names, &file_label, &mut warnings);
            }
        }

        // Validate SFX bundles -- check file existence.
        for (bundle_idx, bundle) in self.sfx_bundles.iter().enumerate() {
            let file_label = PathBuf::from(format!("<sfx_bundle:{bundle_idx}>"));
            for (sound_name, sfx) in &bundle.sounds {
                for file in &sfx.files {
                    let full = asset_root.join(file);
                    if !full.exists() {
                        warnings.push(FormatWarning {
                            file: file_label.clone(),
                            message: format!(
                                "SFX {sound_name:?} references missing file: {file}"
                            ),
                        });
                    }
                }
            }
        }

        warnings
    }

    // -- internals --

    fn walk_dir(&mut self, dir: &Path) -> Result<(), FormatError> {
        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.walk_dir(&path)?;
            } else if path.is_file() {
                self.try_load_file(&path)?;
            }
        }
        Ok(())
    }

    fn try_load_file(&mut self, path: &Path) -> Result<(), FormatError> {
        if path_has_compound_ext(path, "style.ron") {
            let def = StyleDef::load(path)?;
            self.styles.push(def);
        } else if path_has_compound_ext(path, "music.ron") {
            let cfg = MusicConfig::load(path)?;
            // Derive key from the first section-less stem of the filename.
            let key = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".music.ron"))
                .unwrap_or("unknown")
                .to_string();
            self.music_configs.insert(key, cfg);
        } else if path_has_compound_ext(path, "audio_style.ron") {
            let style = WorldAudioStyle::load(path)?;
            self.audio_styles.push(style);
        } else if path_has_compound_ext(path, "sfx.ron") {
            let bundle = SfxBundle::load(path)?;
            self.sfx_bundles.push(bundle);
        }
        // Unrecognised .ron files silently skipped.
        Ok(())
    }
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a path ends with the given compound extension (e.g. `"music.ron"`).
fn path_has_compound_ext(path: &Path, ext: &str) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.ends_with(&format!(".{ext}")))
        .unwrap_or(false)
}

/// Recursively check `StingerThen` transitions for references to undefined
/// stinger names.
fn check_stinger_refs(
    transition: &MusicTransition,
    stinger_names: &std::collections::HashSet<&str>,
    file_label: &Path,
    warnings: &mut Vec<FormatWarning>,
) {
    if let MusicTransition::StingerThen { stinger, then } = transition {
        if !stinger_names.contains(stinger.as_str()) {
            warnings.push(FormatWarning {
                file: file_label.to_path_buf(),
                message: format!(
                    "Transition references undefined stinger: {stinger:?}"
                ),
            });
        }
        check_stinger_refs(then, stinger_names, file_label, warnings);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn style_ron_roundtrip() {
        let ron_text = r#"StyleDef(
    name: "test",
    checkpoint: "test.safetensors",
    lora: None,
    palette: ["#ff0000", "#00ff00"],
    prompt_prefix: "pixel art,",
    negative_prompt: "blurry",
    default_size: (32, 32),
    steps: 20,
    cfg_scale: 7.0,
    post_processing: PostProcessConfig(
        palette_clamp: true,
        remove_anti_aliasing: false,
        add_outline: true,
        outline_color: "#000000",
        outline_mode: Outer,
        cleanup_transparency: true,
        tile_edge_check: false,
    ),
    reference_images: [],
)"#;
        let def: StyleDef = ron::from_str(ron_text).unwrap();
        assert_eq!(def.name, "test");
        assert_eq!(def.palette.len(), 2);
    }

    #[test]
    fn music_ron_roundtrip() {
        let ron_text = r#"MusicConfig(
    bpm: 130,
    beats_per_bar: 4,
    sections: [
        SectionDef(
            name: "calm",
            layers: [
                LayerDef(name: "bass", file: "music/calm_bass.ogg", base_volume: 0.8, rule: None),
            ],
        ),
    ],
    transitions: {
        ("calm", "battle"): CrossfadeOnBar(bars: 2),
    },
    stingers: [
        StingerDef(name: "victory", file: "music/victory.ogg", quantize: Bar),
    ],
)"#;
        let cfg: MusicConfig = ron::from_str(ron_text).unwrap();
        assert_eq!(cfg.bpm, 130);
        assert_eq!(cfg.sections.len(), 1);
        assert_eq!(cfg.sections[0].layers.len(), 1);
        assert_eq!(cfg.stingers.len(), 1);
        assert!(cfg.transitions.contains_key(&("calm".into(), "battle".into())));
    }

    #[test]
    fn audio_style_ron_roundtrip() {
        let ron_text = r#"WorldAudioStyle(
    name: "caribbean",
    genre: "pirate shanty",
    genre_tags: ["folk", "sea shanty"],
    default_bpm: 130,
    sfx_style: "wooden, ocean,",
    key_instruments: ["accordion", "fiddle"],
)"#;
        let style: WorldAudioStyle = ron::from_str(ron_text).unwrap();
        assert_eq!(style.name, "caribbean");
        assert_eq!(style.genre_tags.len(), 2);
    }

    #[test]
    fn sfx_ron_roundtrip() {
        let ron_text = r#"SfxBundle(
    sounds: {
        "sword_hit": SfxDef(
            files: ["sfx/sword_01.ogg", "sfx/sword_02.ogg"],
            volume: 0.9,
            pitch_variance: 0.15,
            max_concurrent: 3,
            cooldown: Some(0.05),
            category: Impact,
        ),
        "arrow_fire": SfxDef(
            files: ["sfx/arrow_01.ogg"],
            volume: 0.7,
            pitch_variance: 0.1,
            max_concurrent: 5,
            cooldown: None,
            category: Gameplay,
        ),
    },
)"#;
        let bundle: SfxBundle = ron::from_str(ron_text).unwrap();
        assert_eq!(bundle.sounds.len(), 2);
        let sword = bundle.sounds.get("sword_hit").unwrap();
        assert_eq!(sword.files.len(), 2);
        assert_eq!(sword.category, SfxCategory::Impact);
    }

    #[test]
    fn load_directory_finds_all_types() {
        let tmp = std::env::temp_dir().join("amigo_registry_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("styles")).unwrap();
        fs::create_dir_all(tmp.join("music")).unwrap();
        fs::create_dir_all(tmp.join("audio")).unwrap();
        fs::create_dir_all(tmp.join("sfx")).unwrap();

        fs::write(
            tmp.join("styles/test.style.ron"),
            r#"StyleDef(
    name: "test",
    checkpoint: "test.safetensors",
    lora: None,
    palette: ["#ff0000"],
    prompt_prefix: "",
    negative_prompt: "",
    default_size: (32, 32),
    steps: 20,
    cfg_scale: 7.0,
    post_processing: PostProcessConfig(
        palette_clamp: true,
        remove_anti_aliasing: false,
        add_outline: false,
        outline_color: "#000000",
        outline_mode: Outer,
        cleanup_transparency: true,
        tile_edge_check: false,
    ),
    reference_images: [],
)"#,
        )
        .unwrap();

        fs::write(
            tmp.join("music/test.music.ron"),
            r#"MusicConfig(
    bpm: 120,
    beats_per_bar: 4,
    sections: [],
    transitions: {},
    stingers: [],
)"#,
        )
        .unwrap();

        fs::write(
            tmp.join("audio/test.audio_style.ron"),
            r#"WorldAudioStyle(
    name: "test_audio",
    genre: "ambient",
    genre_tags: [],
    default_bpm: 90,
    sfx_style: "",
    key_instruments: [],
)"#,
        )
        .unwrap();

        fs::write(
            tmp.join("sfx/combat.sfx.ron"),
            r#"SfxBundle(
    sounds: {
        "hit": SfxDef(
            files: ["sfx/hit.ogg"],
            volume: 1.0,
            pitch_variance: 0.0,
            max_concurrent: 2,
            cooldown: None,
            category: Gameplay,
        ),
    },
)"#,
        )
        .unwrap();

        // Also add an unrecognised .ron that should be silently skipped.
        fs::write(tmp.join("other.ron"), r#"(foo: "bar")"#).unwrap();

        let mut reg = FormatRegistry::new();
        reg.load_directory(&tmp).unwrap();

        assert_eq!(reg.styles().len(), 1);
        assert!(reg.style("test").is_some());
        assert_eq!(reg.music_configs().len(), 1);
        assert!(reg.music_config("test").is_some());
        assert_eq!(reg.audio_styles().len(), 1);
        assert!(reg.audio_style("test_audio").is_some());
        assert_eq!(reg.sfx_bundles().len(), 1);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn validate_catches_bad_palette_hex() {
        let mut reg = FormatRegistry::new();
        reg.styles.push(StyleDef {
            name: "bad".into(),
            checkpoint: String::new(),
            lora: None,
            palette: vec!["#ff0000".into(), "not-hex".into()],
            prompt_prefix: String::new(),
            negative_prompt: String::new(),
            default_size: (32, 32),
            steps: 20,
            cfg_scale: 7.0,
            post_processing: PostProcessConfig {
                palette_clamp: true,
                remove_anti_aliasing: false,
                add_outline: false,
                outline_color: "#000000".into(),
                outline_mode: OutlineMode::Outer,
                cleanup_transparency: false,
                tile_edge_check: false,
            },
            reference_images: vec![],
        });
        let warnings = reg.validate(Path::new("/nonexistent"));
        assert!(warnings.iter().any(|w| w.message.contains("not-hex")));
    }

    #[test]
    fn validate_catches_duplicate_sections() {
        let mut reg = FormatRegistry::new();
        reg.music_configs.insert(
            "dup".into(),
            MusicConfig {
                bpm: 120,
                beats_per_bar: 4,
                sections: vec![
                    SectionDef { name: "a".into(), layers: vec![] },
                    SectionDef { name: "a".into(), layers: vec![] },
                ],
                transitions: HashMap::new(),
                stingers: vec![],
            },
        );
        let warnings = reg.validate(Path::new("/nonexistent"));
        assert!(warnings.iter().any(|w| w.message.contains("Duplicate section")));
    }

    #[test]
    fn validate_catches_missing_stinger_ref() {
        let mut transitions = HashMap::new();
        transitions.insert(
            ("a".into(), "b".into()),
            MusicTransition::StingerThen {
                stinger: "nonexistent".into(),
                then: Box::new(MusicTransition::CutOnBar),
            },
        );
        let mut reg = FormatRegistry::new();
        reg.music_configs.insert(
            "ref".into(),
            MusicConfig {
                bpm: 120,
                beats_per_bar: 4,
                sections: vec![],
                transitions,
                stingers: vec![],
            },
        );
        let warnings = reg.validate(Path::new("/nonexistent"));
        assert!(warnings.iter().any(|w| w.message.contains("undefined stinger")));
    }

    #[test]
    fn format_registry_default() {
        let reg = FormatRegistry::default();
        assert!(reg.styles().is_empty());
        assert!(reg.music_configs().is_empty());
        assert!(reg.audio_styles().is_empty());
        assert!(reg.sfx_bundles().is_empty());
    }
}
