pub mod asset_manager;
pub mod aseprite;
pub mod hot_reload;
pub mod handle;
pub mod pak;

pub use asset_manager::{AssetManager, SpriteData};
pub use aseprite::{load_aseprite, AsepriteData};
pub use hot_reload::HotReloader;
pub use handle::{AssetHandle, AssetState, HandleAllocator};
pub use pak::{PakReader, PakWriter, PakEntry, AssetKind};

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
