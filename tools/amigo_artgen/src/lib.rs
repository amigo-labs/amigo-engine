//! amigo_artgen — AI art generation pipeline for Amigo Engine.
//!
//! Provides a backend-agnostic image generation pipeline (Qwen-Image,
//! FLUX.2 Klein, or custom endpoint), pixel-art and raster-art
//! post-processing, and an MCP tool interface so Claude Code can
//! generate and refine sprite assets.
//!
//! ComfyUI is used as the inference orchestrator under the hood but is
//! fully managed by the engine — users only pick a model in `amigo.toml`.

pub mod comfyui;
pub mod config;
pub mod postprocess;
pub mod style;
pub mod tools;
pub mod workflows;

pub use style::{OutlineMode, PostProcessConfig, StyleDef, StyleError};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Backend + art mode enums
// ---------------------------------------------------------------------------

/// Which image generation model to use.
///
/// Each backend produces a different ComfyUI workflow graph under the hood.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ImageBackend {
    /// Qwen-Image 7B — best quality/size ratio, Apache 2.0. Default.
    #[default]
    QwenImage,
    /// FLUX.2 Klein 4B — compact, fast, large LoRA ecosystem.
    Flux2Klein,
    /// User-provided ComfyUI-compatible endpoint or workflow URL.
    Custom,
}

impl ImageBackend {
    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::QwenImage => "Qwen-Image (7B)",
            Self::Flux2Klein => "FLUX.2 Klein (4B)",
            Self::Custom => "Custom endpoint",
        }
    }

    /// Default checkpoint filename for this backend.
    pub fn default_checkpoint(&self) -> &'static str {
        match self {
            Self::QwenImage => "qwen-image-7b-Q4_K_M.gguf",
            Self::Flux2Klein => "flux2-klein-4b-fp8.safetensors",
            Self::Custom => "",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "qwen-image" => Some(Self::QwenImage),
            "flux2-klein" => Some(Self::Flux2Klein),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Whether to apply pixel-art post-processing or output raw raster art.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ArtMode {
    /// Full pixel-art pipeline: remove AA, palette clamp, outline, etc.
    #[default]
    Pixel,
    /// Raster output: only dimensions/transparency cleanup, no palette clamping.
    Raster,
}

impl ArtMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pixel" => Some(Self::Pixel),
            "raster" => Some(Self::Raster),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A request to generate art (pixel or raster).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtRequest {
    /// What to generate: "sprite", "tileset", "portrait", "background".
    pub asset_type: AssetType,
    /// Descriptive prompt for the art.
    pub prompt: String,
    /// Negative prompt (things to avoid).
    pub negative_prompt: String,
    /// Output resolution in pixels.
    pub width: u32,
    pub height: u32,
    /// The world/theme for style conditioning.
    pub world: String,
    /// Number of variants to generate.
    pub variants: u32,
    /// Post-processing steps to apply.
    pub postprocess: Vec<PostProcessStep>,
    /// Extra key-value options passed to the workflow.
    pub extra: HashMap<String, serde_json::Value>,
    /// Which generation backend to use.
    pub backend: ImageBackend,
    /// Pixel art or raster art output mode.
    pub art_mode: ArtMode,
}

impl Default for ArtRequest {
    fn default() -> Self {
        Self {
            asset_type: AssetType::Sprite,
            prompt: String::new(),
            negative_prompt: "blurry, 3d, realistic, anti-aliased".into(),
            width: 64,
            height: 64,
            world: "default".into(),
            variants: 1,
            postprocess: vec![
                PostProcessStep::RemoveAntiAliasing,
                PostProcessStep::PaletteClamp { max_colors: 32 },
            ],
            extra: HashMap::new(),
            backend: ImageBackend::default(),
            art_mode: ArtMode::default(),
        }
    }
}

/// The type of art asset to generate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    Sprite,
    Tileset,
    Portrait,
    Background,
    UiElement,
    Particle,
}

/// Post-processing steps applied after generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PostProcessStep {
    /// Remove anti-aliasing by snapping semi-transparent pixels.
    RemoveAntiAliasing,
    /// Reduce to N colors via median-cut quantization.
    PaletteClamp { max_colors: u32 },
    /// Add a 1px outline around non-transparent regions.
    AddOutline { color: [u8; 4] },
    /// Scale down by integer factor (for generating at higher res then downscaling).
    Downscale { factor: u32 },
    /// Force exact pixel dimensions (crop/pad).
    ForceDimensions { width: u32, height: u32 },
    /// Apply a reference palette from a .png or .pal file.
    ApplyPalette { palette_path: String },
    /// Snap semi-transparent pixels to fully opaque or fully transparent.
    CleanupTransparency,
    /// Check tile edge compatibility (informational, does not mutate).
    TileEdgeCheck,
}

/// Result of an art generation job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtResult {
    /// Paths to generated image files.
    pub output_paths: Vec<String>,
    /// The ComfyUI prompt ID for reference.
    pub prompt_id: String,
    /// Generation time in milliseconds.
    pub generation_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Style definitions per world
// ---------------------------------------------------------------------------

/// Style configuration for a themed world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldStyle {
    pub name: String,
    pub lora: Option<String>,
    pub style_prompt_prefix: String,
    pub palette_path: Option<String>,
    pub max_colors: u32,
    pub outline_color: Option<[u8; 4]>,
}

impl WorldStyle {
    /// Built-in styles for the 6 TD worlds.
    pub fn builtin_styles() -> Vec<WorldStyle> {
        vec![
            WorldStyle {
                name: "caribbean".into(),
                lora: None,
                style_prompt_prefix: "pixel art, 16-bit, tropical pirate theme, warm colors, "
                    .into(),
                palette_path: None,
                max_colors: 32,
                outline_color: Some([20, 12, 8, 255]),
            },
            WorldStyle {
                name: "lotr".into(),
                lora: None,
                style_prompt_prefix: "pixel art, 16-bit, high fantasy medieval, earth tones, "
                    .into(),
                palette_path: None,
                max_colors: 32,
                outline_color: Some([15, 15, 10, 255]),
            },
            WorldStyle {
                name: "dune".into(),
                lora: None,
                style_prompt_prefix: "pixel art, 16-bit, desert sci-fi, orange and bronze, ".into(),
                palette_path: None,
                max_colors: 24,
                outline_color: Some([40, 25, 10, 255]),
            },
            WorldStyle {
                name: "matrix".into(),
                lora: None,
                style_prompt_prefix: "pixel art, 16-bit, cyberpunk dark, green on black, neon, "
                    .into(),
                palette_path: None,
                max_colors: 16,
                outline_color: Some([0, 30, 0, 255]),
            },
            WorldStyle {
                name: "got".into(),
                lora: None,
                style_prompt_prefix: "pixel art, 16-bit, dark medieval, grim, muted colors, "
                    .into(),
                palette_path: None,
                max_colors: 32,
                outline_color: Some([10, 10, 15, 255]),
            },
            WorldStyle {
                name: "stranger_things".into(),
                lora: None,
                style_prompt_prefix: "pixel art, 16-bit, 1980s retro, neon, synthwave colors, "
                    .into(),
                palette_path: None,
                max_colors: 24,
                outline_color: Some([30, 10, 40, 255]),
            },
        ]
    }

    pub fn find(name: &str) -> Option<WorldStyle> {
        Self::builtin_styles().into_iter().find(|s| s.name == name)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_art_request() {
        let req = ArtRequest::default();
        assert_eq!(req.width, 64);
        assert_eq!(req.variants, 1);
        assert_eq!(req.postprocess.len(), 2);
    }

    #[test]
    fn world_styles_lookup() {
        assert!(WorldStyle::find("caribbean").is_some());
        assert!(WorldStyle::find("matrix").is_some());
        assert!(WorldStyle::find("nonexistent").is_none());
    }

    #[test]
    fn all_six_worlds() {
        let styles = WorldStyle::builtin_styles();
        assert_eq!(styles.len(), 6);
    }
}
