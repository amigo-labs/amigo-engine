//! Custom audio style registry -- load/save user-defined styles from RON files.
//!
//! Custom styles are stored under `assets/audio/` in the project directory:
//! ```text
//! assets/audio/
//! +-- styles.ron          <- Registry of custom styles
//! ```

use crate::WorldAudioStyle;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The on-disk registry of custom audio styles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StyleRegistry {
    pub styles: HashMap<String, WorldAudioStyle>,
}

impl StyleRegistry {
    /// Load the style registry from a `styles.ron` file.
    ///
    /// Returns an empty registry if the file does not exist.
    pub fn load(styles_dir: &Path) -> Self {
        let path = styles_dir.join("styles.ron");
        if !path.exists() {
            return Self::default();
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read {}: {}", path.display(), e);
                return Self::default();
            }
        };

        match ron::from_str(&contents) {
            Ok(registry) => registry,
            Err(e) => {
                tracing::warn!("Failed to parse {}: {}", path.display(), e);
                Self::default()
            }
        }
    }

    /// Save the style registry to a `styles.ron` file.
    pub fn save(&self, styles_dir: &Path) -> Result<(), StyleRegistryError> {
        std::fs::create_dir_all(styles_dir).map_err(StyleRegistryError::Io)?;

        let path = styles_dir.join("styles.ron");
        let pretty = ron::ser::PrettyConfig::default();
        let contents = ron::ser::to_string_pretty(self, pretty)
            .map_err(|e| StyleRegistryError::Serialize(e.to_string()))?;

        std::fs::write(&path, contents).map_err(StyleRegistryError::Io)?;
        tracing::info!("Saved style registry to {}", path.display());
        Ok(())
    }

    /// Get a custom style by name.
    pub fn get(&self, name: &str) -> Option<&WorldAudioStyle> {
        self.styles.get(name)
    }

    /// Insert or update a custom style.
    pub fn insert(&mut self, style: WorldAudioStyle) {
        self.styles.insert(style.name.clone(), style);
    }

    /// Remove a custom style by name. Returns the removed style if it existed.
    pub fn remove(&mut self, name: &str) -> Option<WorldAudioStyle> {
        self.styles.remove(name)
    }

    /// List all custom style names.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.styles.keys().cloned().collect();
        names.sort();
        names
    }

    /// Return the default styles directory for a project.
    pub fn default_dir(project_dir: &Path) -> PathBuf {
        project_dir.join("assets").join("audio")
    }
}

/// Errors from style registry operations.
#[derive(Debug, thiserror::Error)]
pub enum StyleRegistryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(String),
    #[error("Style not found: {0}")]
    NotFound(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry() {
        let reg = StyleRegistry::default();
        assert!(reg.styles.is_empty());
        assert!(reg.names().is_empty());
    }

    #[test]
    fn insert_and_get() {
        let mut reg = StyleRegistry::default();
        reg.insert(WorldAudioStyle {
            name: "cyberpunk".into(),
            genre: "cyberpunk electronic".into(),
            genre_tags: vec!["synth".into(), "industrial".into(), "dark".into()],
            default_bpm: 135,
            sfx_style: "digital, neon, electric, ".into(),
            key_instruments: vec![
                "synth lead".into(),
                "distorted bass".into(),
                "drum machine".into(),
            ],
        });

        assert_eq!(reg.names(), vec!["cyberpunk"]);
        let s = reg.get("cyberpunk").unwrap();
        assert_eq!(s.default_bpm, 135);
        assert_eq!(s.genre, "cyberpunk electronic");
    }

    #[test]
    fn remove_style() {
        let mut reg = StyleRegistry::default();
        reg.insert(WorldAudioStyle {
            name: "removeme".into(),
            genre: "test".into(),
            genre_tags: vec![],
            default_bpm: 120,
            sfx_style: String::new(),
            key_instruments: vec![],
        });

        assert!(reg.remove("removeme").is_some());
        assert!(reg.get("removeme").is_none());
        assert!(reg.remove("removeme").is_none());
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let reg = StyleRegistry::load(Path::new("/tmp/nonexistent_style_dir_12345"));
        assert!(reg.styles.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("amigo_style_test_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);

        let mut reg = StyleRegistry::default();
        reg.insert(WorldAudioStyle {
            name: "anime_ost".into(),
            genre: "anime orchestral".into(),
            genre_tags: vec![
                "orchestral".into(),
                "emotional".into(),
                "piano".into(),
                "strings".into(),
            ],
            default_bpm: 95,
            sfx_style: "anime, dramatic, ".into(),
            key_instruments: vec![
                "piano".into(),
                "strings".into(),
                "choir".into(),
                "taiko".into(),
            ],
        });

        reg.save(&dir).unwrap();

        let loaded = StyleRegistry::load(&dir);
        assert_eq!(loaded.styles.len(), 1);
        let s = loaded.get("anime_ost").unwrap();
        assert_eq!(s.genre, "anime orchestral");
        assert_eq!(s.default_bpm, 95);
        assert_eq!(s.genre_tags.len(), 4);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
