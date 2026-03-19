//! Read/write art generation defaults from amigo.toml [art] section.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Project-level art generation defaults stored in amigo.toml [art].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtDefaults {
    pub default_sprite_size: Option<u32>,
    pub default_style: Option<String>,
    pub default_palette: Option<String>,
    pub color_depth: Option<u32>,
    pub tileset_tile_size: Option<u32>,
    pub background_style: Option<String>,
    pub add_outline: Option<bool>,
    pub outline_color: Option<String>,
}

/// Load [art] defaults from amigo.toml in the given project directory.
pub fn load_art_defaults(project_dir: &Path) -> ArtDefaults {
    let path = project_dir.join("amigo.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return ArtDefaults::default(),
    };
    let table: toml::Value = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return ArtDefaults::default(),
    };
    match table.get("art") {
        Some(art) => {
            let art_str = toml::to_string(art).unwrap_or_default();
            toml::from_str(&art_str).unwrap_or_default()
        }
        None => ArtDefaults::default(),
    }
}

/// Merge updates into the [art] section of amigo.toml.
pub fn save_art_defaults(project_dir: &Path, updates: &HashMap<String, serde_json::Value>) {
    let path = project_dir.join("amigo.toml");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: toml::Value = toml::from_str(&content).unwrap_or(toml::Value::Table(Default::default()));

    let table = doc.as_table_mut().expect("root must be table");
    let art = table
        .entry("art")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let art_table = art.as_table_mut().expect("[art] must be table");

    for (key, value) in updates {
        let toml_val = json_to_toml(value);
        art_table.insert(key.clone(), toml_val);
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
        let defaults = load_art_defaults(dir.path());
        assert!(defaults.default_sprite_size.is_none());
    }

    #[test]
    fn save_and_load_defaults() {
        let dir = tempfile::tempdir().unwrap();
        // Write a minimal amigo.toml
        let mut f = std::fs::File::create(dir.path().join("amigo.toml")).unwrap();
        writeln!(f, "[window]\ntitle = \"Test\"").unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_sprite_size".into(), serde_json::json!(32));
        updates.insert("default_style".into(), serde_json::json!("caribbean"));
        save_art_defaults(dir.path(), &updates);

        let defaults = load_art_defaults(dir.path());
        assert_eq!(defaults.default_sprite_size, Some(32));
        assert_eq!(defaults.default_style, Some("caribbean".into()));
    }
}
