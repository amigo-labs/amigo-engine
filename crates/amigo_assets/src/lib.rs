pub mod aseprite;
pub mod asset_manager;
pub mod descriptors;
pub mod handle;
pub mod hot_reload;
pub mod pak;

pub use aseprite::{load_aseprite, AsepriteData};
pub use asset_manager::{AssetManager, SpriteData};
pub use handle::{AssetHandle, AssetState, HandleAllocator};
pub use hot_reload::HotReloader;
pub use descriptors::{
    EntityDescriptor, MapDescriptor, SpriteDescriptor, TilesetDescriptor,
};
pub use pak::{AssetKind, PakEntry, PakReader, PakWriter};

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
}
