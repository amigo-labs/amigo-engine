//! MCP tool definitions for amigo_audiogen.
//!
//! Each tool maps to an audio generation or processing operation.

use crate::{MusicRequest, MusicResult, MusicSection, SfxRequest, SfxResult, SfxCategory, WorldAudioStyle};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Tool parameter structs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateMusicParams {
    pub world: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default = "default_bpm")]
    pub bpm: u32,
    #[serde(default = "default_duration")]
    pub duration_secs: f32,
    #[serde(default)]
    pub lyrics: Option<String>,
    #[serde(default = "default_section")]
    pub section: String,
    #[serde(default = "default_true")]
    pub split_stems: bool,
}

fn default_bpm() -> u32 { 120 }
fn default_duration() -> f32 { 30.0 }
fn default_section() -> String { "calm".into() }
fn default_true() -> bool { true }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateSfxParams {
    pub prompt: String,
    #[serde(default = "default_sfx_duration")]
    pub duration_secs: f32,
    #[serde(default = "default_variants")]
    pub variants: u32,
    #[serde(default = "default_true")]
    pub trim_silence: bool,
    #[serde(default = "default_true")]
    pub normalize: bool,
    #[serde(default)]
    pub category: Option<String>,
}

fn default_sfx_duration() -> f32 { 2.0 }
fn default_variants() -> u32 { 3 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplitStemsParams {
    pub input: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub output_dir: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessAudioParams {
    pub input: String,
    #[serde(default)]
    pub trim_silence: bool,
    #[serde(default)]
    pub normalize: bool,
    #[serde(default)]
    pub find_loop: bool,
    #[serde(default)]
    pub detect_bpm: bool,
}

// ---------------------------------------------------------------------------
// Tool result structs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StemResult {
    pub stems: Vec<String>,
    pub adaptive_config: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessResult {
    pub output: String,
    pub bpm: Option<f32>,
    pub loop_point: Option<f32>,
    pub duration_secs: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StylesResult {
    pub styles: Vec<StyleInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StyleInfo {
    pub name: String,
    pub genre: String,
    pub default_bpm: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerStatusResult {
    pub acestep_connected: bool,
    pub audiogen_connected: bool,
}

// ---------------------------------------------------------------------------
// Tool registry for MCP
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

pub fn list_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "amigo_audiogen_generate_music".into(),
            description: "Generate a music track using ACE-Step AI".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "world": { "type": "string", "description": "World style (caribbean, lotr, dune, matrix, got, stranger_things)" },
                    "genre": { "type": "string", "description": "Genre override" },
                    "bpm": { "type": "integer", "description": "Target BPM" },
                    "duration_secs": { "type": "number", "description": "Track duration in seconds" },
                    "lyrics": { "type": "string", "description": "Optional lyrics for vocal conditioning" },
                    "section": { "type": "string", "description": "Music section: calm, tense, battle, boss, victory, menu" },
                    "split_stems": { "type": "boolean", "description": "Split into stems for adaptive music" }
                },
                "required": ["world"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_generate_sfx".into(),
            description: "Generate sound effects using AudioGen AI".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Description of the sound effect" },
                    "duration_secs": { "type": "number", "description": "Duration in seconds (max 10)" },
                    "variants": { "type": "integer", "description": "Number of variants to generate" },
                    "trim_silence": { "type": "boolean" },
                    "normalize": { "type": "boolean" },
                    "category": { "type": "string", "description": "Category: gameplay, ui, ambient, impact, explosion, magic, voice" }
                },
                "required": ["prompt"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_split_stems".into(),
            description: "Split an audio file into stems (drums, bass, vocals, other) using Demucs".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to the audio file" },
                    "model": { "type": "string", "description": "Demucs model: demucs4, demucs6" },
                    "output_dir": { "type": "string" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_process".into(),
            description: "Post-process audio: trim silence, normalize, detect BPM, find loop point".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to audio file" },
                    "trim_silence": { "type": "boolean" },
                    "normalize": { "type": "boolean" },
                    "find_loop": { "type": "boolean" },
                    "detect_bpm": { "type": "boolean" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_list_styles".into(),
            description: "List available world audio styles".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_audiogen_server_status".into(),
            description: "Check ACE-Step and AudioGen server status".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Unknown tool: {0}")]
    UnknownTool(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(#[from] serde_json::Error),
}

pub fn dispatch_tool(name: &str, params: serde_json::Value) -> Result<serde_json::Value, ToolError> {
    match name {
        "amigo_audiogen_generate_music" => {
            let p: GenerateMusicParams = serde_json::from_value(params)?;
            let style = WorldAudioStyle::find(&p.world);
            let _genre = if p.genre.is_empty() {
                style.as_ref().map(|s| s.genre.clone()).unwrap_or_default()
            } else {
                p.genre.clone()
            };
            let bpm = if p.bpm == 0 {
                style.as_ref().map(|s| s.default_bpm).unwrap_or(120)
            } else {
                p.bpm
            };

            // Placeholder: in production, calls ACE-Step API
            let base_name = format!("{}_{}_{}bpm", p.world, p.section, bpm);
            let mut stem_paths = std::collections::HashMap::new();
            if p.split_stems {
                for stem in &["drums", "bass", "vocals", "other"] {
                    stem_paths.insert(
                        stem.to_string(),
                        format!("assets/generated/audio/stems/{}_{}.wav", base_name, stem),
                    );
                }
            }

            Ok(serde_json::to_value(MusicResult {
                full_track_path: format!("assets/generated/audio/{}.wav", base_name),
                stem_paths,
                detected_bpm: bpm as f32,
                generation_time_ms: 0,
            })?)
        }
        "amigo_audiogen_generate_sfx" => {
            let p: GenerateSfxParams = serde_json::from_value(params)?;
            let count = p.variants.min(10);
            let safe_name = sanitize(&p.prompt);

            // Placeholder: in production, calls AudioGen API
            let paths: Vec<String> = (0..count)
                .map(|i| format!("assets/generated/audio/sfx/{}_v{}.wav", safe_name, i + 1))
                .collect();
            let durations: Vec<f32> = (0..count).map(|_| p.duration_secs).collect();

            Ok(serde_json::to_value(SfxResult {
                output_paths: paths,
                durations,
                generation_time_ms: 0,
            })?)
        }
        "amigo_audiogen_split_stems" => {
            let p: SplitStemsParams = serde_json::from_value(params)?;
            let dir = p.output_dir.unwrap_or_else(|| "assets/generated/audio/stems".into());
            let stem_name = std::path::Path::new(&p.input)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "track".into());

            let stems: Vec<String> = ["drums", "bass", "vocals", "other"]
                .iter()
                .map(|s| format!("{}/{}_{}.wav", dir, stem_name, s))
                .collect();

            Ok(serde_json::to_value(StemResult {
                stems,
                adaptive_config: Some(format!("{}/{}_adaptive.ron", dir, stem_name)),
            })?)
        }
        "amigo_audiogen_process" => {
            let p: ProcessAudioParams = serde_json::from_value(params)?;

            // Demonstrate processing capabilities (no real file I/O)
            let mut bpm = None;
            let mut loop_point = None;

            if p.detect_bpm {
                // Would detect from actual audio
                bpm = Some(120.0);
            }
            if p.find_loop {
                // Would find from actual audio
                loop_point = Some(0.0);
            }

            Ok(serde_json::to_value(ProcessResult {
                output: p.input,
                bpm,
                loop_point,
                duration_secs: 0.0,
            })?)
        }
        "amigo_audiogen_list_styles" => {
            let styles: Vec<StyleInfo> = WorldAudioStyle::builtin_styles()
                .into_iter()
                .map(|s| StyleInfo {
                    name: s.name,
                    genre: s.genre,
                    default_bpm: s.default_bpm,
                })
                .collect();
            Ok(serde_json::to_value(StylesResult { styles })?)
        }
        "amigo_audiogen_server_status" => {
            Ok(serde_json::to_value(ServerStatusResult {
                acestep_connected: false,
                audiogen_connected: false,
            })?)
        }
        _ => Err(ToolError::UnknownTool(name.to_string())),
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>()
        .chars()
        .take(40)
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_tools_returns_6() {
        assert_eq!(list_tools().len(), 6);
    }

    #[test]
    fn dispatch_generate_music() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_music",
            serde_json::json!({ "world": "caribbean" }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["full_track_path"].as_str().unwrap().contains("caribbean"));
    }

    #[test]
    fn dispatch_generate_music_with_style() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_music",
            serde_json::json!({
                "world": "matrix",
                "section": "battle",
                "bpm": 160
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["full_track_path"].as_str().unwrap().contains("160bpm"));
        assert!(v["stem_paths"].as_object().unwrap().len() == 4);
    }

    #[test]
    fn dispatch_generate_sfx() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_sfx",
            serde_json::json!({
                "prompt": "sword slash impact",
                "variants": 2
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["output_paths"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn dispatch_split_stems() {
        let result = dispatch_tool(
            "amigo_audiogen_split_stems",
            serde_json::json!({ "input": "music/battle.wav" }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["stems"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn dispatch_list_styles() {
        let result = dispatch_tool("amigo_audiogen_list_styles", serde_json::json!({}));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["styles"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn dispatch_unknown() {
        let result = dispatch_tool("nonexistent", serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_server_status() {
        let result = dispatch_tool("amigo_audiogen_server_status", serde_json::json!({}));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["acestep_connected"], false);
    }
}
