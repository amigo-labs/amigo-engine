//! Voice profile registry -- load/save voice profiles from RON files.
//!
//! Voice profiles are stored under `assets/voices/` in the project directory:
//! ```text
//! assets/voices/
//! +-- voices.ron          <- Registry of all profiles
//! +-- old_wizard.wav      <- Reference audio
//! +-- narrator_de.wav
//! ```

use crate::VoiceProfile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The on-disk registry of voice profiles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VoiceRegistry {
    pub voices: HashMap<String, VoiceProfile>,
}

impl VoiceRegistry {
    /// Load the voice registry from a `voices.ron` file.
    ///
    /// Returns an empty registry if the file does not exist.
    pub fn load(voices_dir: &Path) -> Self {
        let path = voices_dir.join("voices.ron");
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

    /// Save the voice registry to a `voices.ron` file.
    pub fn save(&self, voices_dir: &Path) -> Result<(), VoiceRegistryError> {
        std::fs::create_dir_all(voices_dir).map_err(VoiceRegistryError::Io)?;

        let path = voices_dir.join("voices.ron");
        let pretty = ron::ser::PrettyConfig::default();
        let contents = ron::ser::to_string_pretty(self, pretty)
            .map_err(|e| VoiceRegistryError::Serialize(e.to_string()))?;

        std::fs::write(&path, contents).map_err(VoiceRegistryError::Io)?;
        tracing::info!("Saved voice registry to {}", path.display());
        Ok(())
    }

    /// Get a voice profile by name.
    pub fn get(&self, name: &str) -> Option<&VoiceProfile> {
        self.voices.get(name)
    }

    /// Insert or update a voice profile.
    pub fn insert(&mut self, profile: VoiceProfile) {
        self.voices.insert(profile.name.clone(), profile);
    }

    /// Remove a voice profile by name. Returns the removed profile if it existed.
    pub fn remove(&mut self, name: &str) -> Option<VoiceProfile> {
        self.voices.remove(name)
    }

    /// List all voice profile names.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.voices.keys().cloned().collect();
        names.sort();
        names
    }

    /// Return the default voices directory for a project.
    pub fn default_dir(project_dir: &Path) -> PathBuf {
        project_dir.join("assets").join("voices")
    }
}

/// Errors from voice registry operations.
#[derive(Debug, thiserror::Error)]
pub enum VoiceRegistryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(String),
    #[error("Voice not found: {0}")]
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
        let reg = VoiceRegistry::default();
        assert!(reg.voices.is_empty());
        assert!(reg.names().is_empty());
    }

    #[test]
    fn insert_and_get() {
        let mut reg = VoiceRegistry::default();
        reg.insert(VoiceProfile {
            name: "test_voice".into(),
            reference_audio: "test.wav".into(),
            default_language: "de-DE".into(),
            default_delivery: None,
            description: Some("A test voice".into()),
        });

        assert_eq!(reg.names(), vec!["test_voice"]);
        let v = reg.get("test_voice").unwrap();
        assert_eq!(v.default_language, "de-DE");
    }

    #[test]
    fn remove_voice() {
        let mut reg = VoiceRegistry::default();
        reg.insert(VoiceProfile {
            name: "removeme".into(),
            reference_audio: "r.wav".into(),
            default_language: "en-US".into(),
            default_delivery: None,
            description: None,
        });

        assert!(reg.remove("removeme").is_some());
        assert!(reg.get("removeme").is_none());
        assert!(reg.remove("removeme").is_none());
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let reg = VoiceRegistry::load(Path::new("/tmp/nonexistent_voice_dir_12345"));
        assert!(reg.voices.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("amigo_voice_test_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);

        let mut reg = VoiceRegistry::default();
        reg.insert(VoiceProfile {
            name: "wizard".into(),
            reference_audio: "wizard.wav".into(),
            default_language: "de-DE".into(),
            default_delivery: Some("speak slowly with gravitas".into()),
            description: Some("Old wizard".into()),
        });

        reg.save(&dir).unwrap();

        let loaded = VoiceRegistry::load(&dir);
        assert_eq!(loaded.voices.len(), 1);
        let w = loaded.get("wizard").unwrap();
        assert_eq!(w.default_language, "de-DE");
        assert_eq!(
            w.default_delivery.as_deref(),
            Some("speak slowly with gravitas")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
