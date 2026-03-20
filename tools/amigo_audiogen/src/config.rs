//! Read/write audio generation defaults from amigo.toml [audio_defaults] section.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Project-level audio generation defaults stored in amigo.toml [audio_defaults].
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

/// Load [audio_defaults] from amigo.toml in the given project directory.
pub fn load_audio_defaults(project_dir: &Path) -> AudioGenDefaults {
    let path = project_dir.join("amigo.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return AudioGenDefaults::default(),
    };
    let table: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return AudioGenDefaults::default(),
    };
    match table.get("audio_defaults") {
        Some(section) => {
            let s = toml::to_string(section).unwrap_or_default();
            toml::from_str(&s).unwrap_or_default()
        }
        None => AudioGenDefaults::default(),
    }
}

/// Merge updates into the [audio_defaults] section of amigo.toml.
pub fn save_audio_defaults(project_dir: &Path, updates: &HashMap<String, serde_json::Value>) {
    let path = project_dir.join("amigo.toml");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: toml::Value =
        toml::from_str(&content).unwrap_or(toml::Value::Table(Default::default()));

    let table = doc.as_table_mut().expect("root must be table");
    let section = table
        .entry("audio_defaults")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let section_table = section
        .as_table_mut()
        .expect("[audio_defaults] must be table");

    for (key, value) in updates {
        let toml_val = json_to_toml(value);
        section_table.insert(key.clone(), toml_val);
    }

    let output = toml::to_string_pretty(&doc).unwrap_or_default();
    let _ = std::fs::write(&path, output);
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
    fn save_and_load_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("amigo.toml")).unwrap();
        writeln!(f, "[window]\ntitle = \"Test\"").unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_bpm".into(), serde_json::json!(140));
        updates.insert("default_genre".into(), serde_json::json!("chiptune"));
        save_audio_defaults(dir.path(), &updates);

        let defaults = load_audio_defaults(dir.path());
        assert_eq!(defaults.default_bpm, Some(140));
        assert_eq!(defaults.default_genre, Some("chiptune".into()));
    }
}
