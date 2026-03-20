//! Read/write audio generation defaults from amigo.toml [audio] section.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Project-level audio generation defaults stored in amigo.toml [audio].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioGenDefaults {
    pub default_genre: Option<String>,
    pub default_bpm: Option<u32>,
    pub default_key: Option<String>,
    pub sfx_duration: Option<f32>,
    pub music_duration: Option<f32>,
    pub sample_rate: Option<u32>,
    pub output_format: Option<String>,
}

/// Load [audio] defaults from amigo.toml in the given project directory.
pub fn load_audio_defaults(project_dir: &Path) -> AudioGenDefaults {
    let path = project_dir.join("amigo.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return AudioGenDefaults::default(),
    };
    let table: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Failed to parse amigo.toml: {}", e);
            return AudioGenDefaults::default();
        }
    };
    match table.get("audio") {
        Some(section) => {
            let s = toml::to_string(section).unwrap_or_default();
            toml::from_str(&s).unwrap_or_default()
        }
        None => AudioGenDefaults::default(),
    }
}

/// Merge updates into the [audio] section of amigo.toml.
///
/// Returns `Ok(())` on success, or an error message if the file could not be written.
pub fn save_audio_defaults(
    project_dir: &Path,
    updates: &HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    if updates.is_empty() {
        return Ok(());
    }

    let path = project_dir.join("amigo.toml");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: toml::Value =
        toml::from_str(&content).unwrap_or(toml::Value::Table(Default::default()));

    let table = doc
        .as_table_mut()
        .ok_or_else(|| "amigo.toml root is not a table".to_string())?;
    let section = table
        .entry("audio")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let section_table = section
        .as_table_mut()
        .ok_or_else(|| "[audio] is not a table".to_string())?;

    for (key, value) in updates {
        let toml_val = json_to_toml(value);
        section_table.insert(key.clone(), toml_val);
    }

    let output = toml::to_string_pretty(&doc).unwrap_or_default();
    std::fs::write(&path, output).map_err(|e| format!("Failed to write amigo.toml: {}", e))
}

fn json_to_toml(v: &serde_json::Value) -> toml::Value {
    match v {
        serde_json::Value::Bool(b) => toml::Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => toml::Value::String(s.clone()),
        _ => toml::Value::String(v.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_defaults_empty() {
        let dir = tempfile::tempdir().unwrap();
        let defaults = load_audio_defaults(dir.path());
        assert!(defaults.default_bpm.is_none());
    }

    #[test]
    fn load_defaults_no_audio_section() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[window]\ntitle = \"Test\"\n",
        )
        .unwrap();
        let defaults = load_audio_defaults(dir.path());
        assert!(defaults.default_bpm.is_none());
    }

    #[test]
    fn load_defaults_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("amigo.toml"), "not valid { toml").unwrap();
        let defaults = load_audio_defaults(dir.path());
        assert!(defaults.default_bpm.is_none());
    }

    #[test]
    fn save_and_load_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("amigo.toml")).unwrap();
        writeln!(f, "[window]\ntitle = \"Test\"").unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_bpm".into(), serde_json::json!(140));
        updates.insert("default_genre".into(), serde_json::json!("chiptune"));
        save_audio_defaults(dir.path(), &updates).unwrap();

        let defaults = load_audio_defaults(dir.path());
        assert_eq!(defaults.default_bpm, Some(140));
        assert_eq!(defaults.default_genre, Some("chiptune".into()));
    }

    #[test]
    fn save_merges_with_existing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[audio]\ndefault_bpm = 100\n",
        )
        .unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_genre".into(), serde_json::json!("chiptune"));
        save_audio_defaults(dir.path(), &updates).unwrap();

        let defaults = load_audio_defaults(dir.path());
        assert_eq!(defaults.default_bpm, Some(100)); // preserved
        assert_eq!(defaults.default_genre, Some("chiptune".into())); // added
    }

    #[test]
    fn save_empty_updates_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let original = "[window]\ntitle = \"Test\"\n";
        std::fs::write(dir.path().join("amigo.toml"), original).unwrap();

        let updates = HashMap::new();
        save_audio_defaults(dir.path(), &updates).unwrap();

        let content = std::fs::read_to_string(dir.path().join("amigo.toml")).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn save_creates_audio_section() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[window]\ntitle = \"Test\"\n",
        )
        .unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_bpm".into(), serde_json::json!(120));
        save_audio_defaults(dir.path(), &updates).unwrap();

        let content = std::fs::read_to_string(dir.path().join("amigo.toml")).unwrap();
        assert!(content.contains("[audio]"));
    }
}
