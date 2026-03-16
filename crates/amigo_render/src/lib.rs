#![warn(missing_docs)]

pub mod atlas;
pub mod atmosphere;
pub mod camera;
pub mod font;
pub mod lighting;
pub mod particles;
pub mod post_process;
pub mod renderer;
pub mod sprite_batcher;
pub mod texture;
pub mod vertex;

#[cfg(feature = "editor")]
pub mod egui_integration;

pub use atmosphere::{AtmosphereManager, AtmospherePreset};
pub use camera::{Camera, CameraMode, Easing};
pub use font::{FontAtlas, FontId, FontManager, GlyphInfo};
pub use lighting::{AmbientLight, LightingState, PointLight};
pub use particles::{BlendMode, EmitterConfig, EmitterShape, ParticleEmitter, ParticleSystem};
pub use post_process::{PostEffect, PostProcessPipeline, PostProcessUniforms};
pub use renderer::Renderer;
pub use sprite_batcher::{SpriteBatcher, SpriteInstance};
pub use texture::{Texture, TextureId};
pub use vertex::Vertex;

// ---------------------------------------------------------------------------
// Art style configuration
// ---------------------------------------------------------------------------

/// The overall art style of the game, affecting texture filtering and camera behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArtStyle {
    /// Classic pixel art: nearest-neighbor filtering, pixel-snapped camera.
    #[default]
    PixelArt,
    /// High-resolution hand-drawn / vector art (Cuphead, Hollow Knight):
    /// bilinear filtering, smooth sub-pixel camera.
    RasterArt,
    /// Mixed mode: default nearest-neighbor, but individual textures can
    /// opt into bilinear filtering. Camera uses smooth positioning.
    Hybrid,
}

impl ArtStyle {
    /// The default sampler mode for this art style.
    pub fn default_sampler_mode(self) -> SamplerMode {
        match self {
            ArtStyle::PixelArt => SamplerMode::Nearest,
            ArtStyle::RasterArt => SamplerMode::Linear,
            // Hybrid defaults to nearest; per-texture overrides expected.
            ArtStyle::Hybrid => SamplerMode::Nearest,
        }
    }

    /// Whether the camera should snap positions to integer pixels.
    pub fn pixel_snap(self) -> bool {
        matches!(self, ArtStyle::PixelArt)
    }

    /// Parse from a config string.
    pub fn from_str_config(s: &str) -> Self {
        match s {
            "raster_art" | "raster" => ArtStyle::RasterArt,
            "hybrid" | "mixed" => ArtStyle::Hybrid,
            _ => ArtStyle::PixelArt,
        }
    }
}

/// Texture sampling / filtering mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SamplerMode {
    /// Nearest-neighbor — sharp pixels, no blending between texels.
    #[default]
    Nearest,
    /// Bilinear — smooth interpolation between texels.
    Linear,
}

impl SamplerMode {
    /// Convert to the corresponding wgpu filter mode.
    pub fn to_wgpu(self) -> wgpu::FilterMode {
        match self {
            SamplerMode::Nearest => wgpu::FilterMode::Nearest,
            SamplerMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}
