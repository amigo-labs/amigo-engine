//! Style definitions for themed art generation.
//!
//! Each style constrains AI generation for visual consistency: palette,
//! checkpoint, LoRA, prompt engineering, and post-processing flags.

use serde::{Deserialize, Serialize};

/// A style definition loaded from a `.style.ron` file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StyleDef {
    pub name: String,
    /// Stable Diffusion checkpoint
    pub checkpoint: String,
    /// Optional LoRA with strength
    pub lora: Option<(String, f32)>,
    /// Hex color palette (enforced in post-processing)
    pub palette: Vec<String>,
    /// Prepended to every prompt
    pub prompt_prefix: String,
    /// Default negative prompt
    pub negative_prompt: String,
    /// Default sprite size (width, height)
    pub default_size: (u32, u32),
    /// Diffusion steps
    pub steps: u32,
    /// CFG scale
    pub cfg_scale: f32,
    /// Post-processing configuration
    pub post_processing: PostProcessConfig,
    /// Reference images for img2img consistency
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

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            palette_clamp: true,
            remove_anti_aliasing: true,
            add_outline: true,
            outline_color: "#1a1a2e".into(),
            outline_mode: OutlineMode::Outer,
            cleanup_transparency: true,
            tile_edge_check: false,
        }
    }
}

impl StyleDef {
    /// Parse hex color string "#RRGGBB" to [u8; 3]
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

    /// Get the palette as RGB arrays
    pub fn palette_rgb(&self) -> Vec<[u8; 3]> {
        self.palette
            .iter()
            .filter_map(|h| Self::parse_hex_color(h))
            .collect()
    }

    /// Get outline color as RGBA
    pub fn outline_rgba(&self) -> [u8; 4] {
        let rgb = Self::parse_hex_color(&self.post_processing.outline_color).unwrap_or([0, 0, 0]);
        [rgb[0], rgb[1], rgb[2], 255]
    }

    /// Load a style from a RON file
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, StyleError> {
        let contents = std::fs::read_to_string(path).map_err(StyleError::Io)?;
        ron::from_str(&contents).map_err(|e| StyleError::Parse(e.to_string()))
    }

    /// Load all styles from a directory
    pub fn load_all(dir: &std::path::Path) -> Vec<Self> {
        let mut styles = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "ron").unwrap_or(false) {
                    if let Ok(style) = Self::load_from_file(&path) {
                        styles.push(style);
                    }
                }
            }
        }
        styles
    }

    /// Builtin default styles (fallback when no RON files exist)
    pub fn builtin_defaults() -> Vec<Self> {
        vec![
            Self::caribbean(),
            Self::lotr(),
            Self::dune(),
            Self::matrix(),
            Self::got(),
            Self::stranger_things(),
        ]
    }

    pub fn find(name: &str) -> Option<Self> {
        Self::builtin_defaults()
            .into_iter()
            .find(|s| s.name == name)
    }

    fn caribbean() -> Self {
        Self {
            name: "caribbean".into(),
            checkpoint: "pixel_art_xl_v1.safetensors".into(),
            lora: Some(("pixel_art_16bit.safetensors".into(), 0.7)),
            palette: vec![
                "#1a1a2e", "#e8c170", "#8b5e3c", "#3b7dd8", "#4caf50", "#f5f5dc", "#c0392b",
                "#f39c12", "#2c3e50", "#ecf0f1",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            prompt_prefix: "pixel art, 16-bit style, tropical pirate theme,".into(),
            negative_prompt: "realistic, 3d, smooth, anti-aliased, gradient, blurry, modern, photo"
                .into(),
            default_size: (32, 32),
            steps: 20,
            cfg_scale: 7.0,
            post_processing: PostProcessConfig {
                outline_color: "#1a1a2e".into(),
                ..PostProcessConfig::default()
            },
            reference_images: vec![
                "styles/ref/caribbean_tower.png".into(),
                "styles/ref/caribbean_enemy.png".into(),
                "styles/ref/caribbean_tiles.png".into(),
            ],
        }
    }

    fn lotr() -> Self {
        Self {
            name: "lotr".into(),
            checkpoint: "pixel_art_xl_v1.safetensors".into(),
            lora: Some(("pixel_art_16bit.safetensors".into(), 0.7)),
            palette: vec![
                "#2d1b00", "#5c3a1e", "#8b6914", "#4a7023", "#2e4600", "#808080", "#c0c0c0",
                "#f0e68c", "#1a1a2e", "#d4c4a8",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            prompt_prefix: "pixel art, 16-bit style, high fantasy medieval,".into(),
            negative_prompt: "realistic, 3d, smooth, anti-aliased, gradient, blurry, modern, photo"
                .into(),
            default_size: (32, 32),
            steps: 20,
            cfg_scale: 7.0,
            post_processing: PostProcessConfig {
                outline_color: "#1a1a2e".into(),
                ..PostProcessConfig::default()
            },
            reference_images: vec![],
        }
    }

    fn dune() -> Self {
        Self {
            name: "dune".into(),
            checkpoint: "pixel_art_xl_v1.safetensors".into(),
            lora: Some(("pixel_art_16bit.safetensors".into(), 0.6)),
            palette: vec![
                "#2a1a0a", "#c2956a", "#e8c170", "#f0d890", "#4a3520", "#6b4226", "#d4a06a",
                "#1a1a2e", "#8b7355", "#f5deb3",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            prompt_prefix: "pixel art, 16-bit style, desert sci-fi, sandworm,".into(),
            negative_prompt: "realistic, 3d, smooth, anti-aliased, gradient, blurry, modern, photo"
                .into(),
            default_size: (32, 32),
            steps: 20,
            cfg_scale: 7.0,
            post_processing: PostProcessConfig {
                outline_color: "#2a1a0a".into(),
                ..PostProcessConfig::default()
            },
            reference_images: vec![],
        }
    }

    fn matrix() -> Self {
        Self {
            name: "matrix".into(),
            checkpoint: "pixel_art_xl_v1.safetensors".into(),
            lora: None,
            palette: vec![
                "#000000", "#001a00", "#003300", "#006600", "#00cc00", "#00ff00", "#0a0a0a",
                "#1a1a1a", "#2a2a2a", "#00ff41",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            prompt_prefix: "pixel art, 16-bit style, cyberpunk dark, green on black, neon,".into(),
            negative_prompt:
                "realistic, 3d, smooth, anti-aliased, gradient, blurry, colorful, bright".into(),
            default_size: (32, 32),
            steps: 20,
            cfg_scale: 7.0,
            post_processing: PostProcessConfig {
                outline_color: "#001a00".into(),
                ..PostProcessConfig::default()
            },
            reference_images: vec![],
        }
    }

    fn got() -> Self {
        Self {
            name: "got".into(),
            checkpoint: "pixel_art_xl_v1.safetensors".into(),
            lora: Some(("pixel_art_16bit.safetensors".into(), 0.7)),
            palette: vec![
                "#1a1a2e", "#2c2c3e", "#4a4a5e", "#8b8b8b", "#c0c0c0", "#8b0000", "#b22222",
                "#f5f5dc", "#2f4f4f", "#696969",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            prompt_prefix: "pixel art, 16-bit style, dark medieval, grim, muted colors,".into(),
            negative_prompt:
                "realistic, 3d, smooth, anti-aliased, gradient, blurry, bright, colorful".into(),
            default_size: (32, 32),
            steps: 20,
            cfg_scale: 7.0,
            post_processing: PostProcessConfig {
                outline_color: "#1a1a2e".into(),
                ..PostProcessConfig::default()
            },
            reference_images: vec![],
        }
    }

    fn stranger_things() -> Self {
        Self {
            name: "stranger_things".into(),
            checkpoint: "pixel_art_xl_v1.safetensors".into(),
            lora: Some(("pixel_art_16bit.safetensors".into(), 0.65)),
            palette: vec![
                "#1a0a2e", "#2e0854", "#6a0dad", "#ff1493", "#ff6ec7", "#00ffff", "#ff4500",
                "#ffd700", "#1a1a2e", "#e0e0e0",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            prompt_prefix: "pixel art, 16-bit style, 1980s retro, neon, synthwave,".into(),
            negative_prompt:
                "realistic, 3d, smooth, anti-aliased, gradient, blurry, modern, minimal".into(),
            default_size: (32, 32),
            steps: 20,
            cfg_scale: 7.0,
            post_processing: PostProcessConfig {
                outline_color: "#1a0a2e".into(),
                ..PostProcessConfig::default()
            },
            reference_images: vec![],
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StyleError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Hex color parsing ──────────────────────────────────────

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(StyleDef::parse_hex_color("#ff0000"), Some([255, 0, 0]));
        assert_eq!(StyleDef::parse_hex_color("#00ff00"), Some([0, 255, 0]));
        assert_eq!(StyleDef::parse_hex_color("#0000ff"), Some([0, 0, 255]));
        assert_eq!(StyleDef::parse_hex_color("#1a1a2e"), Some([26, 26, 46]));
    }

    #[test]
    fn parse_hex_color_no_hash() {
        assert_eq!(StyleDef::parse_hex_color("ff0000"), Some([255, 0, 0]));
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert_eq!(StyleDef::parse_hex_color("#fff"), None);
        assert_eq!(StyleDef::parse_hex_color(""), None);
        assert_eq!(StyleDef::parse_hex_color("#zzzzzz"), None);
    }

    // ── Palette and style lookup ──────────────────────────────────

    #[test]
    fn palette_rgb_converts() {
        let style = StyleDef::find("caribbean").unwrap();
        let rgb = style.palette_rgb();
        assert_eq!(rgb.len(), 10);
        assert_eq!(rgb[0], [26, 26, 46]); // #1a1a2e
    }

    #[test]
    fn builtin_defaults_has_six() {
        let defaults = StyleDef::builtin_defaults();
        assert_eq!(defaults.len(), 6);
    }

    #[test]
    fn find_existing_style() {
        assert!(StyleDef::find("caribbean").is_some());
        assert!(StyleDef::find("lotr").is_some());
        assert!(StyleDef::find("dune").is_some());
        assert!(StyleDef::find("matrix").is_some());
        assert!(StyleDef::find("got").is_some());
        assert!(StyleDef::find("stranger_things").is_some());
    }

    #[test]
    fn find_nonexistent_style() {
        assert!(StyleDef::find("nonexistent").is_none());
    }

    #[test]
    fn outline_rgba_from_style() {
        let style = StyleDef::find("caribbean").unwrap();
        assert_eq!(style.outline_rgba(), [26, 26, 46, 255]);
    }

    // ── PostProcessConfig defaults ────────────────────────────────

    #[test]
    fn default_post_process_config() {
        let config = PostProcessConfig::default();
        assert!(config.palette_clamp);
        assert!(config.remove_anti_aliasing);
        assert!(config.add_outline);
        assert!(config.cleanup_transparency);
        assert!(!config.tile_edge_check);
    }
}
