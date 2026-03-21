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
    /// Image generation backend: "qwen-image", "flux2-klein", or "custom".
    pub backend: Option<String>,
    /// Art output mode: "pixel" (default) or "raster".
    pub art_mode: Option<String>,
    /// Custom ComfyUI endpoint URL (only used when backend = "custom").
    pub custom_endpoint: Option<String>,
    /// URL to a ComfyUI workflow JSON (only used when backend = "custom").
    pub custom_workflow_url: Option<String>,
}

impl ArtDefaults {
    /// Resolve the `ImageBackend` from config, falling back to default.
    pub fn resolve_backend(&self) -> crate::ImageBackend {
        self.backend
            .as_deref()
            .and_then(crate::ImageBackend::from_str)
            .unwrap_or_default()
    }

    /// Resolve the `ArtMode` from config, falling back to default.
    pub fn resolve_art_mode(&self) -> crate::ArtMode {
        self.art_mode
            .as_deref()
            .and_then(crate::ArtMode::from_str)
            .unwrap_or_default()
    }
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
        Err(e) => {
            tracing::warn!("Failed to parse amigo.toml: {}", e);
            return ArtDefaults::default();
        }
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
///
/// Returns `Ok(())` on success, or an error message if the file could not be written.
pub fn save_art_defaults(
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
    let art = table
        .entry("art")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let art_table = art
        .as_table_mut()
        .ok_or_else(|| "[art] is not a table".to_string())?;

    for (key, value) in updates {
        let toml_val = json_to_toml(value);
        art_table.insert(key.clone(), toml_val);
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
        let defaults = load_art_defaults(dir.path());
        assert!(defaults.default_sprite_size.is_none());
    }

    #[test]
    fn load_defaults_no_art_section() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[window]\ntitle = \"Test\"\n",
        )
        .unwrap();
        let defaults = load_art_defaults(dir.path());
        assert!(defaults.default_sprite_size.is_none());
    }

    #[test]
    fn load_defaults_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("amigo.toml"), "not valid { toml").unwrap();
        let defaults = load_art_defaults(dir.path());
        assert!(defaults.default_sprite_size.is_none());
    }

    #[test]
    fn save_and_load_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = std::fs::File::create(dir.path().join("amigo.toml")).unwrap();
        writeln!(f, "[window]\ntitle = \"Test\"").unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_sprite_size".into(), serde_json::json!(32));
        updates.insert("default_style".into(), serde_json::json!("caribbean"));
        save_art_defaults(dir.path(), &updates).unwrap();

        let defaults = load_art_defaults(dir.path());
        assert_eq!(defaults.default_sprite_size, Some(32));
        assert_eq!(defaults.default_style, Some("caribbean".into()));
    }

    #[test]
    fn save_merges_with_existing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[art]\ndefault_style = \"caribbean\"\n",
        )
        .unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_sprite_size".into(), serde_json::json!(32));
        save_art_defaults(dir.path(), &updates).unwrap();

        let defaults = load_art_defaults(dir.path());
        assert_eq!(defaults.default_style, Some("caribbean".into())); // preserved
        assert_eq!(defaults.default_sprite_size, Some(32)); // added
    }

    #[test]
    fn save_empty_updates_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let original = "[window]\ntitle = \"Test\"\n";
        std::fs::write(dir.path().join("amigo.toml"), original).unwrap();

        let updates = HashMap::new();
        save_art_defaults(dir.path(), &updates).unwrap();

        let content = std::fs::read_to_string(dir.path().join("amigo.toml")).unwrap();
        assert_eq!(content, original);
    }

    #[test]
    fn save_creates_art_section() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[window]\ntitle = \"Test\"\n",
        )
        .unwrap();

        let mut updates = HashMap::new();
        updates.insert("default_sprite_size".into(), serde_json::json!(32));
        save_art_defaults(dir.path(), &updates).unwrap();

        let content = std::fs::read_to_string(dir.path().join("amigo.toml")).unwrap();
        assert!(content.contains("[art]"));
    }
}
