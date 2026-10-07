pub mod aseprite;
pub mod asset_manager;
pub mod atlas_manifest;
pub mod descriptors;
pub mod formats;
pub mod handle;
pub mod hot_reload;
pub mod import;
pub mod modding;
pub mod pak;
pub mod registry;

#[cfg(feature = "asset_streaming")]
pub mod streaming;

pub use aseprite::{AsepriteData, load_aseprite};
pub use asset_manager::{
    AssetManager, PAK_ANIMATIONS, PAK_ATLAS_PREFIX, PackedAtlases, SheetData, SpriteData,
    is_packed_sheet, is_sprite_file, load_sprite_file, load_sprite_file_with_frames, pack_atlases,
};
pub use atlas_manifest::{
    AtlasError, AtlasFrame, AtlasManifest, AtlasSprite, LoadedAtlas, SpriteFrame, is_atlas_manifest,
};
pub use descriptors::{EntityDescriptor, MapDescriptor, SpriteDescriptor, TilesetDescriptor};
pub use handle::{AssetHandle, AssetState, HandleAllocator};
pub use hot_reload::HotReloader;
pub use pak::{AssetKind, PakEntry, PakReader, PakWriter};
pub use registry::{
    FormatError as RegistryError, FormatRegistry, FormatWarning, LayerDef, LayerRule, MusicConfig,
    MusicTransition, PostProcessConfig, SectionDef, SfxBundle, SfxCategory, SfxDef, StingerDef,
    StingerQuantize, StyleDef, WorldAudioStyle,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("Asset not found: {path}")]
    NotFound { path: String },

    #[error("Asset not found: {path} (did you mean '{suggestion}'?)")]
    NotFoundWithSuggestion { path: String, suggestion: String },

    #[error("Failed to load asset: {path}: {reason}")]
    LoadFailed { path: String, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error(transparent)]
    Atlas(#[from] AtlasError),
}
