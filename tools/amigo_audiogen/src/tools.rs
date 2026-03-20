//! MCP tool definitions for amigo_audiogen.
//!
//! Each tool maps to an audio generation or processing operation.

use crate::config::{load_audio_defaults, save_audio_defaults};
use crate::{MusicResult, SfxResult, WorldAudioStyle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

fn default_bpm() -> u32 {
    120
}
fn default_duration() -> f32 {
    30.0
}
fn default_section() -> String {
    "calm".into()
}
fn default_true() -> bool {
    true
}

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

fn default_sfx_duration() -> f32 {
    2.0
}
fn default_variants() -> u32 {
    3
}

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateCoreMelodyParams {
    pub world: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default = "default_key")]
    pub key: String,
    #[serde(default = "default_bpm")]
    pub bpm: u32,
    #[serde(default = "default_duration")]
    pub duration_secs: f32,
}

fn default_key() -> String {
    "C minor".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateStemParams {
    pub stem_type: String,
    pub melody_ref: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default = "default_bpm")]
    pub bpm: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateVariationParams {
    pub input: String,
    #[serde(default = "default_variation_strength")]
    pub strength: f32,
    #[serde(default)]
    pub seed: Option<i64>,
}

fn default_variation_strength() -> f32 {
    0.3
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtendTrackParams {
    pub input: String,
    #[serde(default = "default_duration")]
    pub extend_secs: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemixParams {
    pub input: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default = "default_bpm")]
    pub bpm: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateAmbientParams {
    pub prompt: String,
    #[serde(default = "default_ambient_duration")]
    pub duration_secs: f32,
    #[serde(default = "default_true")]
    pub looping: bool,
}

fn default_ambient_duration() -> f32 {
    60.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopTrimParams {
    pub input: String,
    #[serde(default = "default_duration")]
    pub target_duration_secs: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizeParams {
    pub input: String,
    #[serde(default = "default_target_db")]
    pub target_db: f32,
}

fn default_target_db() -> f32 {
    -1.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvertParams {
    pub input: String,
    pub format: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewParams {
    pub input: String,
    #[serde(default = "default_preview_secs")]
    pub preview_secs: f32,
}

fn default_preview_secs() -> f32 {
    5.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetDefaultsParams {
    pub project_dir: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetDefaultsParams {
    pub project_dir: String,
    pub defaults: HashMap<String, serde_json::Value>,
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
            description: "Split an audio file into stems (drums, bass, vocals, other) using Demucs"
                .into(),
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
            description: "Post-process audio: trim silence, normalize, detect BPM, find loop point"
                .into(),
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
        ToolDef {
            name: "amigo_audiogen_generate_core_melody".into(),
            description: "Generate a core melody reference for clean-mode stem workflow".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "world": { "type": "string", "description": "World style for conditioning" },
                    "prompt": { "type": "string", "description": "Melody description" },
                    "key": { "type": "string", "description": "Musical key (e.g. C minor)" },
                    "bpm": { "type": "integer" },
                    "duration_secs": { "type": "number" }
                },
                "required": ["world"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_generate_stem".into(),
            description: "Generate an individual stem (bass, drums, harmony) conditioned on a melody reference".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "stem_type": { "type": "string", "description": "Stem type: bass, drums, harmony, etc." },
                    "melody_ref": { "type": "string", "description": "Path to the melody reference file" },
                    "prompt": { "type": "string" },
                    "bpm": { "type": "integer" }
                },
                "required": ["stem_type", "melody_ref"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_generate_variation".into(),
            description: "Generate a variation of an existing track".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to source audio" },
                    "strength": { "type": "number", "description": "Variation strength 0.0-1.0" },
                    "seed": { "type": "integer" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_extend_track".into(),
            description: "Extend a track by generating a continuation".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to source audio" },
                    "extend_secs": { "type": "number", "description": "Seconds to extend" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_remix".into(),
            description: "Remix a track with different parameters (genre, BPM)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to source audio" },
                    "genre": { "type": "string" },
                    "bpm": { "type": "integer" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_generate_ambient".into(),
            description: "Generate an ambient/atmosphere loop".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "Ambient description" },
                    "duration_secs": { "type": "number" },
                    "looping": { "type": "boolean" }
                },
                "required": ["prompt"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_loop_trim".into(),
            description: "Trim audio to an optimal loop point".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to audio file" },
                    "target_duration_secs": { "type": "number" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_normalize".into(),
            description: "Normalize audio to a target dB level".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to audio file" },
                    "target_db": { "type": "number", "description": "Target peak dB (default -1.0)" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_convert".into(),
            description: "Convert audio format (WAV→OGG, FLAC, etc.)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to audio file" },
                    "format": { "type": "string", "description": "Target format: ogg, wav, flac" }
                },
                "required": ["input", "format"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_preview".into(),
            description: "Generate a short preview clip of an audio file".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to audio file" },
                    "preview_secs": { "type": "number", "description": "Preview duration in seconds" }
                },
                "required": ["input"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_list_models".into(),
            description: "List available ACE-Step and AudioGen models".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_audiogen_queue_status".into(),
            description: "Check the generation queue status".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_audiogen_get_defaults".into(),
            description: "Get project audio generation defaults from amigo.toml".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Path to the project directory" }
                },
                "required": ["project_dir"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_set_defaults".into(),
            description: "Save audio generation defaults to amigo.toml [audio] section. \
                Merges with existing values. Use after asking the user for preferences.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Path to the project directory" },
                    "defaults": {
                        "type": "object",
                        "description": "Key-value pairs to merge into [audio] section",
                        "properties": {
                            "default_genre": { "type": "string", "description": "Default music genre" },
                            "default_bpm": { "type": "integer", "description": "Default BPM" },
                            "default_key": { "type": "string", "description": "Default musical key (e.g. 'C minor')" },
                            "sfx_duration": { "type": "number", "description": "Default SFX duration in seconds" },
                            "music_duration": { "type": "number", "description": "Default music duration in seconds" },
                            "sample_rate": { "type": "integer", "description": "Sample rate (e.g. 44100)" },
                            "output_format": { "type": "string", "description": "Output format (e.g. 'wav', 'ogg')" }
                        }
                    }
                },
                "required": ["project_dir", "defaults"]
            }),
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

pub fn dispatch_tool(
    name: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, ToolError> {
    dispatch_tool_with_defaults(name, params, None)
}

/// Like `dispatch_tool`, but accepts an explicit project directory for
/// resolving [audio] defaults from amigo.toml.
pub fn dispatch_tool_with_defaults(
    name: &str,
    params: serde_json::Value,
    project_dir: Option<&std::path::Path>,
) -> Result<serde_json::Value, ToolError> {
    match name {
        "amigo_audiogen_generate_music" => {
            let p: GenerateMusicParams = serde_json::from_value(params)?;
            let style = WorldAudioStyle::find(&p.world);
            let defaults = project_dir.map(load_audio_defaults);
            let mut missing: Vec<String> = Vec::new();

            // Resolution order: explicit param → amigo.toml → world style → hardcoded
            let _genre = if !p.genre.is_empty() {
                p.genre.clone()
            } else if let Some(g) = defaults.as_ref().and_then(|d| d.default_genre.clone()) {
                g
            } else {
                missing.push("genre".into());
                style.as_ref().map(|s| s.genre.clone()).unwrap_or_default()
            };
            let bpm = if p.bpm != default_bpm() {
                p.bpm
            } else if let Some(b) = defaults.as_ref().and_then(|d| d.default_bpm) {
                b
            } else {
                missing.push("bpm".into());
                style.as_ref().map(|s| s.default_bpm).unwrap_or(120)
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

            let result = MusicResult {
                full_track_path: format!("assets/generated/audio/{}.wav", base_name),
                stem_paths,
                detected_bpm: bpm as f32,
                generation_time_ms: 0,
            };

            let mut response = serde_json::to_value(result)?;
            if !missing.is_empty() {
                response["hints"] = serde_json::json!({
                    "defaults_missing": missing,
                    "suggestion": "Run amigo_audiogen_set_defaults to save project defaults"
                });
            }
            Ok(response)
        }
        "amigo_audiogen_generate_sfx" => {
            let p: GenerateSfxParams = serde_json::from_value(params)?;
            let defaults = project_dir.map(load_audio_defaults);
            let mut missing: Vec<String> = Vec::new();

            // Use amigo.toml sfx_duration as fallback if user provided the serde default
            let duration = if (p.duration_secs - default_sfx_duration()).abs() < f32::EPSILON {
                if let Some(d) = defaults.as_ref().and_then(|d| d.sfx_duration) {
                    d
                } else {
                    missing.push("sfx_duration".into());
                    p.duration_secs
                }
            } else {
                p.duration_secs
            };
            let _ = duration; // used when calling the actual AudioGen API

            let count = p.variants.min(10);
            let safe_name = sanitize(&p.prompt);

            // Placeholder: in production, calls AudioGen API
            let paths: Vec<String> = (0..count)
                .map(|i| format!("assets/generated/audio/sfx/{}_v{}.wav", safe_name, i + 1))
                .collect();
            let durations: Vec<f32> = (0..count).map(|_| p.duration_secs).collect();

            let result = SfxResult {
                output_paths: paths,
                durations,
                generation_time_ms: 0,
            };

            let mut response = serde_json::to_value(result)?;
            if !missing.is_empty() {
                response["hints"] = serde_json::json!({
                    "defaults_missing": missing,
                    "suggestion": "Run amigo_audiogen_set_defaults to save project defaults"
                });
            }
            Ok(response)
        }
        "amigo_audiogen_split_stems" => {
            let p: SplitStemsParams = serde_json::from_value(params)?;
            let dir = p
                .output_dir
                .unwrap_or_else(|| "assets/generated/audio/stems".into());
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
        "amigo_audiogen_server_status" => Ok(serde_json::to_value(ServerStatusResult {
            acestep_connected: false,
            audiogen_connected: false,
        })?),
        "amigo_audiogen_generate_core_melody" => {
            let p: GenerateCoreMelodyParams = serde_json::from_value(params)?;
            let style = WorldAudioStyle::find(&p.world);
            let bpm = if p.bpm == 0 {
                style.as_ref().map(|s| s.default_bpm).unwrap_or(120)
            } else {
                p.bpm
            };
            let base_name = format!("{}_melody_{}bpm_{}", p.world, bpm, p.key.replace(' ', "_"));
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/stems/{}.wav", base_name),
                "key": p.key,
                "bpm": bpm,
                "duration_secs": p.duration_secs,
            }))
        }
        "amigo_audiogen_generate_stem" => {
            let p: GenerateStemParams = serde_json::from_value(params)?;
            let bpm = if p.bpm == 0 { 120 } else { p.bpm };
            let stem_name = sanitize(&p.stem_type);
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/stems/{}_{}.wav",
                    std::path::Path::new(&p.melody_ref)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "melody".into()),
                    stem_name
                ),
                "stem_type": p.stem_type,
                "melody_ref": p.melody_ref,
                "bpm": bpm,
            }))
        }
        "amigo_audiogen_generate_variation" => {
            let p: GenerateVariationParams = serde_json::from_value(params)?;
            let base = std::path::Path::new(&p.input)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "track".into());
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/{}_var.wav", base),
                "source": p.input,
                "strength": p.strength,
            }))
        }
        "amigo_audiogen_extend_track" => {
            let p: ExtendTrackParams = serde_json::from_value(params)?;
            let base = std::path::Path::new(&p.input)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "track".into());
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/{}_extended.wav", base),
                "source": p.input,
                "extend_secs": p.extend_secs,
            }))
        }
        "amigo_audiogen_remix" => {
            let p: RemixParams = serde_json::from_value(params)?;
            let base = std::path::Path::new(&p.input)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "track".into());
            let bpm = if p.bpm == 0 { 120 } else { p.bpm };
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/{}_remix_{}bpm.wav", base, bpm),
                "source": p.input,
                "genre": p.genre,
                "bpm": bpm,
            }))
        }
        "amigo_audiogen_generate_ambient" => {
            let p: GenerateAmbientParams = serde_json::from_value(params)?;
            let safe_name = sanitize(&p.prompt);
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/ambient/{}.wav", safe_name),
                "duration_secs": p.duration_secs,
                "looping": p.looping,
            }))
        }
        "amigo_audiogen_loop_trim" => {
            let p: LoopTrimParams = serde_json::from_value(params)?;
            let base = std::path::Path::new(&p.input)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "track".into());
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/{}_looped.wav", base),
                "source": p.input,
                "target_duration_secs": p.target_duration_secs,
            }))
        }
        "amigo_audiogen_normalize" => {
            let p: NormalizeParams = serde_json::from_value(params)?;
            Ok(serde_json::json!({
                "output": p.input.clone(),
                "target_db": p.target_db,
            }))
        }
        "amigo_audiogen_convert" => {
            let p: ConvertParams = serde_json::from_value(params)?;
            let base = std::path::Path::new(&p.input)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "audio".into());
            let ext = match p.format.as_str() {
                "ogg" => "ogg",
                "flac" => "flac",
                "wav" => "wav",
                _ => "wav",
            };
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/{}.{}", base, ext),
                "source": p.input,
                "format": p.format,
            }))
        }
        "amigo_audiogen_preview" => {
            let p: PreviewParams = serde_json::from_value(params)?;
            let base = std::path::Path::new(&p.input)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "audio".into());
            Ok(serde_json::json!({
                "output": format!("assets/generated/audio/{}_preview.wav", base),
                "source": p.input,
                "preview_secs": p.preview_secs,
            }))
        }
        "amigo_audiogen_list_models" => Ok(serde_json::json!({
            "acestep_models": ["ace-step-v1"],
            "audiogen_models": ["audiogen-medium"],
            "demucs_models": ["demucs4", "demucs6"],
        })),
        "amigo_audiogen_queue_status" => Ok(serde_json::json!({
            "acestep_queue": 0,
            "audiogen_queue": 0,
            "total_pending": 0,
        })),
        "amigo_audiogen_get_defaults" => {
            let p: GetDefaultsParams = serde_json::from_value(params)?;
            let defaults = load_audio_defaults(std::path::Path::new(&p.project_dir));
            Ok(serde_json::to_value(defaults).unwrap_or_default())
        }
        "amigo_audiogen_set_defaults" => {
            let p: SetDefaultsParams = serde_json::from_value(params)?;
            let project_path = std::path::Path::new(&p.project_dir);
            if let Err(e) = save_audio_defaults(project_path, &p.defaults) {
                return Ok(serde_json::json!({ "saved": false, "error": e }));
            }
            Ok(serde_json::json!({ "saved": true, "path": "amigo.toml" }))
        }
        _ => Err(ToolError::UnknownTool(name.to_string())),
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
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

    // ── Tool listing ───────────────────────────────────────────

    #[test]
    fn list_tools_returns_20() {
        assert_eq!(list_tools().len(), 20);
    }

    // ── Music generation dispatch ─────────────────────────────────

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

    // ── SFX and stems dispatch ──────────────────────────────────

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

    // ── Query and status dispatch ──────────────────────────────

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

    // ── Clean-mode dispatch ─────────────────────────────────────

    #[test]
    fn dispatch_generate_core_melody() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_core_melody",
            serde_json::json!({ "world": "caribbean", "key": "A minor", "bpm": 130 }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["output"].as_str().unwrap().contains("caribbean"));
        assert!(v["output"].as_str().unwrap().contains("130bpm"));
    }

    #[test]
    fn dispatch_generate_stem() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_stem",
            serde_json::json!({
                "stem_type": "bass",
                "melody_ref": "assets/melody.wav",
                "bpm": 120
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["output"].as_str().unwrap().contains("bass"));
    }

    // ── Utility dispatch ────────────────────────────────────────

    #[test]
    fn dispatch_generate_variation() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_variation",
            serde_json::json!({ "input": "track.wav", "strength": 0.5 }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["output"].as_str().unwrap().contains("var"));
    }

    #[test]
    fn dispatch_generate_ambient() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_ambient",
            serde_json::json!({ "prompt": "ocean waves crashing" }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["output"].as_str().unwrap().contains("ambient"));
    }

    #[test]
    fn dispatch_convert() {
        let result = dispatch_tool(
            "amigo_audiogen_convert",
            serde_json::json!({ "input": "track.wav", "format": "ogg" }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["output"].as_str().unwrap().ends_with(".ogg"));
    }

    #[test]
    fn dispatch_list_models() {
        let result = dispatch_tool("amigo_audiogen_list_models", serde_json::json!({}));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(!v["acestep_models"].as_array().unwrap().is_empty());
    }

    #[test]
    fn dispatch_queue_status() {
        let result = dispatch_tool("amigo_audiogen_queue_status", serde_json::json!({}));
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["total_pending"], 0);
    }

    #[test]
    fn dispatch_get_defaults_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = dispatch_tool(
            "amigo_audiogen_get_defaults",
            serde_json::json!({ "project_dir": dir.path().to_str().unwrap() }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["default_bpm"].is_null());
    }

    #[test]
    fn dispatch_set_and_get_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[window]\ntitle = \"Test\"\n",
        )
        .unwrap();

        let result = dispatch_tool(
            "amigo_audiogen_set_defaults",
            serde_json::json!({
                "project_dir": dir.path().to_str().unwrap(),
                "defaults": { "default_bpm": 140, "default_genre": "chiptune" }
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["saved"], true);
        assert_eq!(v["path"], "amigo.toml");

        // Verify they were actually saved
        let get_result = dispatch_tool(
            "amigo_audiogen_get_defaults",
            serde_json::json!({ "project_dir": dir.path().to_str().unwrap() }),
        )
        .unwrap();
        assert_eq!(get_result["default_bpm"], 140);
        assert_eq!(get_result["default_genre"], "chiptune");
    }

    #[test]
    fn dispatch_generate_music_defaults_missing_hint() {
        // No amigo.toml -> falls back to style/hardcoded -> should have defaults_missing
        let result = dispatch_tool(
            "amigo_audiogen_generate_music",
            serde_json::json!({ "world": "caribbean" }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        let hints = &v["hints"];
        assert!(hints["defaults_missing"].is_array());
        assert!(hints["suggestion"]
            .as_str()
            .unwrap()
            .contains("amigo_audiogen_set_defaults"));
    }

    #[test]
    fn dispatch_generate_music_no_hint_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("amigo.toml"),
            "[audio]\ndefault_bpm = 140\ndefault_genre = \"chiptune\"\n",
        )
        .unwrap();

        let result = dispatch_tool_with_defaults(
            "amigo_audiogen_generate_music",
            serde_json::json!({ "world": "caribbean" }),
            Some(dir.path()),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        // All defaults are provided, so no hints
        assert!(v.get("hints").is_none());
    }
}
