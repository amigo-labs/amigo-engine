use serde::Deserialize;

/// Player stats loaded from RON.
#[derive(Debug, Deserialize)]
pub struct PlayerStats {
    pub speed: f32,
    pub jump_force: f32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            speed: 80.0,
            jump_force: 200.0,
        }
    }
}

/// Loads a RON file from disk, returning the default on failure.
pub fn load_ron_or_default<T: serde::de::DeserializeOwned + Default>(path: &str) -> T {
    match std::fs::read_to_string(path) {
        Ok(contents) => ron::from_str(&contents).unwrap_or_default(),
        Err(_) => T::default(),
    }
}
