pub mod asset_manager;
pub mod hot_reload;

pub use asset_manager::{AssetManager, SpriteData};
pub use hot_reload::HotReloader;

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
