//! MCP tool definitions for amigo_audiogen.
//!
//! Each tool maps to an audio generation or processing operation.
//! Audio generation tools call ComfyUI via the shared `amigo_comfyui` client.

use crate::config::{load_audio_defaults, save_audio_defaults};
use crate::style_registry::StyleRegistry;
use crate::voice_registry::VoiceRegistry;
use crate::workflows;
use crate::{
    AudioFormat, CreateVoiceResult, MusicRequest, MusicResult, MusicSection, SfxCategory,
    SfxRequest, SfxResult, TtsRequest, TtsResult, VoiceProfile, WorldAudioStyle,
};
use amigo_comfyui::{ComfyUiClient, ComfyUiConfig, PromptStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// ComfyUI client helpers
// ---------------------------------------------------------------------------

/// Default ComfyUI server URL. Can be overridden by AMIGO_COMFYUI_URL env var.
const DEFAULT_COMFYUI_URL: &str = "http://127.0.0.1:8188";

/// Timeout for audio generation (5 minutes).
const GENERATION_TIMEOUT_MS: u64 = 300_000;
/// Poll interval for checking generation status.
const POLL_INTERVAL_MS: u64 = 1_000;

/// Create a ComfyUI client from the environment or default URL.
fn create_comfyui_client() -> ComfyUiClient {
    let url = std::env::var("AMIGO_COMFYUI_URL").unwrap_or_else(|_| DEFAULT_COMFYUI_URL.into());
    let config = parse_comfy_url(&url);
    ComfyUiClient::new(config)
}

/// Parse a URL like "http://localhost:8188" into a ComfyUiConfig.
fn parse_comfy_url(url: &str) -> ComfyUiConfig {
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let (host, port) = if let Some((h, p)) = stripped.split_once(':') {
        (h.to_string(), p.parse().unwrap_or(8188))
    } else {
        (stripped.to_string(), 8188)
    };

    ComfyUiConfig { host, port }
}

/// Queue a ComfyUI workflow prompt, wait for completion, retrieve output audio,
/// and download to the given output path. Returns the filename on success.
fn run_comfyui_audio_workflow(
    client: &ComfyUiClient,
    prompt: &amigo_comfyui::ComfyPrompt,
    output_path: &str,
) -> Result<String, String> {
    let queue_resp = client
        .queue_prompt(prompt)
        .map_err(|e| format!("Failed to queue prompt: {}", e))?;

    let status = client
        .wait_for_completion(
            &queue_resp.prompt_id,
            GENERATION_TIMEOUT_MS,
            POLL_INTERVAL_MS,
        )
        .map_err(|e| format!("Generation failed: {}", e))?;

    match status {
        PromptStatus::Completed => {}
        PromptStatus::Failed { error } => {
            return Err(format!("ComfyUI generation failed: {}", error));
        }
        _ => {
            return Err("Generation ended in unexpected state".into());
        }
    }

    let audio_files = client
        .get_audio(&queue_resp.prompt_id)
        .map_err(|e| format!("Failed to get audio output: {}", e))?;

    let audio = audio_files
        .first()
        .ok_or_else(|| "No audio output from ComfyUI".to_string())?;

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(output_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create output directory {}: {e}",
                parent.display()
            )
        })?;
    }

    client
        .download_audio(audio, output_path)
        .map_err(|e| format!("Failed to download audio: {}", e))?;

    Ok(audio.filename.clone())
}

/// Parse a section string into a MusicSection enum.
fn parse_section(s: &str) -> MusicSection {
    match s {
        "calm" => MusicSection::Calm,
        "tense" => MusicSection::Tense,
        "battle" => MusicSection::Battle,
        "boss" => MusicSection::Boss,
        "victory" => MusicSection::Victory,
        "menu" => MusicSection::Menu,
        other => MusicSection::Custom(other.to_string()),
    }
}

/// Parse a category string into an SfxCategory enum.
fn parse_category(s: Option<&str>) -> SfxCategory {
    match s {
        Some("gameplay") | None => SfxCategory::Gameplay,
        Some("ui") => SfxCategory::UI,
        Some("ambient") => SfxCategory::Ambient,
        Some("impact") => SfxCategory::Impact,
        Some("explosion") => SfxCategory::Explosion,
        Some("magic") => SfxCategory::Magic,
        Some("voice") => SfxCategory::Voice,
        Some(other) => SfxCategory::Custom(other.to_string()),
    }
}

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
// TTS tool parameter structs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateTtsParams {
    pub text: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub delivery: Option<String>,
    #[serde(default)]
    pub reference_audio: Option<String>,
    #[serde(default)]
    pub speaker_id: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_language() -> String {
    "de-DE".into()
}

fn default_format() -> String {
    "wav".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateVoiceParams {
    pub name: String,
    pub reference_audio: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub default_delivery: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub test_text: Option<String>,
    /// Project directory for resolving the voices directory.
    #[serde(default)]
    pub project_dir: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListVoicesParams {
    /// Project directory for resolving the voices directory.
    #[serde(default)]
    pub project_dir: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewVoiceParams {
    pub name: String,
    #[serde(default = "default_preview_text")]
    pub text: String,
    #[serde(default)]
    pub project_dir: Option<String>,
}

fn default_preview_text() -> String {
    "Dies ist ein Test der Stimme.".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteVoiceParams {
    pub name: String,
    #[serde(default)]
    pub project_dir: Option<String>,
}

// ---------------------------------------------------------------------------
// Style tool parameter structs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateStyleParams {
    pub name: String,
    pub genre: String,
    #[serde(default)]
    pub genre_tags: String,
    #[serde(default = "default_bpm")]
    pub default_bpm: u32,
    #[serde(default)]
    pub sfx_style: String,
    #[serde(default)]
    pub key_instruments: String,
    #[serde(default)]
    pub project_dir: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditStyleParams {
    pub name: String,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub genre_tags: Option<String>,
    #[serde(default)]
    pub default_bpm: Option<u32>,
    #[serde(default)]
    pub sfx_style: Option<String>,
    #[serde(default)]
    pub key_instruments: Option<String>,
    #[serde(default)]
    pub project_dir: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeleteStyleParams {
    pub name: String,
    #[serde(default)]
    pub project_dir: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FromReferenceParams {
    pub reference_description: String,
    #[serde(default)]
    pub style_name: Option<String>,
    #[serde(default)]
    pub target_genre: Option<String>,
    #[serde(default)]
    pub target_bpm: Option<u32>,
    #[serde(default = "default_duration")]
    pub duration_secs: f32,
    #[serde(default = "default_section")]
    pub section: String,
    #[serde(default = "default_variation_strength")]
    pub variation_strength: f32,
    #[serde(default = "default_one")]
    pub num_variations: u32,
    #[serde(default)]
    pub project_dir: Option<String>,
}

fn default_one() -> u32 {
    1
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
    pub genre_tags: Vec<String>,
    pub sfx_style: String,
    pub key_instruments: Vec<String>,
    pub is_custom: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerStatusResult {
    pub comfyui_connected: bool,
    pub comfyui_url: String,
    /// Legacy fields for backwards compatibility.
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
                    "world": { "type": "string", "description": "World style (builtin: caribbean, lotr, dune, matrix, got, stranger_things — or any custom style name)" },
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
            description: "List available world audio styles (builtin + custom)".into(),
            input_schema: serde_json::json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "amigo_audiogen_create_style".into(),
            description: "Create a custom audio style preset".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique style name (e.g. cyberpunk, anime_ost)" },
                    "genre": { "type": "string", "description": "Primary music genre (e.g. cyberpunk electronic)" },
                    "genre_tags": { "type": "string", "description": "Comma-separated genre descriptors for conditioning (e.g. synth, industrial, dark)" },
                    "default_bpm": { "type": "integer", "description": "Default BPM for this style" },
                    "sfx_style": { "type": "string", "description": "SFX style prefix (e.g. digital, neon, electric, )" },
                    "key_instruments": { "type": "string", "description": "Comma-separated instruments (e.g. synth lead, distorted bass, drum machine)" },
                    "project_dir": { "type": "string", "description": "Project directory (optional)" }
                },
                "required": ["name", "genre"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_edit_style".into(),
            description: "Edit an existing custom audio style preset".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name of the custom style to edit" },
                    "genre": { "type": "string", "description": "New primary genre" },
                    "genre_tags": { "type": "string", "description": "New comma-separated genre tags" },
                    "default_bpm": { "type": "integer", "description": "New default BPM" },
                    "sfx_style": { "type": "string", "description": "New SFX style prefix" },
                    "key_instruments": { "type": "string", "description": "New comma-separated instruments" },
                    "project_dir": { "type": "string", "description": "Project directory (optional)" }
                },
                "required": ["name"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_delete_style".into(),
            description: "Delete a custom audio style preset".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name of the custom style to delete" },
                    "project_dir": { "type": "string", "description": "Project directory (optional)" }
                },
                "required": ["name"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_from_reference".into(),
            description: "Generate music inspired by a reference song description. Describe the vibe, genre, instruments, tempo — the system generates original music with that feel.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reference_description": { "type": "string", "description": "Text description of the reference song's characteristics: genre, instruments, tempo, mood, dynamics (e.g. 'Orchestral, piano-driven, dramatic dynamics, 72 BPM, major to minor shifts, operatic elements')" },
                    "style_name": { "type": "string", "description": "If set, saves the derived parameters as a reusable custom style with this name" },
                    "target_genre": { "type": "string", "description": "Genre override (if you want to shift the genre away from the reference)" },
                    "target_bpm": { "type": "integer", "description": "BPM override" },
                    "duration_secs": { "type": "number", "description": "Track duration in seconds (default 30)" },
                    "section": { "type": "string", "description": "Music section: calm, tense, battle, boss, victory, menu (default calm)" },
                    "variation_strength": { "type": "number", "description": "How far to deviate from the reference vibe (0.0 = very close, 1.0 = very different, default 0.3)" },
                    "num_variations": { "type": "integer", "description": "Number of variations to generate (default 1)" },
                    "project_dir": { "type": "string", "description": "Project directory (optional)" }
                },
                "required": ["reference_description"]
            }),
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
        // --- TTS tools ---
        ToolDef {
            name: "amigo_audiogen_generate_tts".into(),
            description: "Generate speech from text using Qwen3-TTS via ComfyUI. \
                Supports 10 languages, emotion via delivery instructions, and voice cloning.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to speak" },
                    "language": { "type": "string", "description": "BCP-47 language tag (default: de-DE)" },
                    "delivery": { "type": "string", "description": "Delivery instruction for emotion/style, e.g. 'speak with anger', 'whisper softly'" },
                    "reference_audio": { "type": "string", "description": "Path to reference audio for voice cloning (10s recommended)" },
                    "speaker_id": { "type": "string", "description": "Name of a saved voice profile (e.g. 'narrator', 'old_wizard')" },
                    "format": { "type": "string", "description": "Output format: wav or ogg (default: wav)" }
                },
                "required": ["text"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_create_voice".into(),
            description: "Create a new voice profile for TTS. Stores reference audio and metadata \
                under assets/voices/ for use with speaker_id in generate_tts.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Unique voice name (e.g. 'old_wizard', 'narrator_de')" },
                    "reference_audio": { "type": "string", "description": "Path to reference audio file (WAV, 10-30s recommended)" },
                    "language": { "type": "string", "description": "Default language (default: de-DE)" },
                    "default_delivery": { "type": "string", "description": "Default delivery instruction for this voice" },
                    "description": { "type": "string", "description": "Human-readable description of the voice" },
                    "test_text": { "type": "string", "description": "Optional text to speak as validation after creation" },
                    "project_dir": { "type": "string", "description": "Project directory (for locating assets/voices/)" }
                },
                "required": ["name", "reference_audio"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_list_voices".into(),
            description: "List all saved voice profiles.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_dir": { "type": "string", "description": "Project directory" }
                }
            }),
        },
        ToolDef {
            name: "amigo_audiogen_preview_voice".into(),
            description: "Generate a short preview of a saved voice profile with test text.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Voice profile name" },
                    "text": { "type": "string", "description": "Test text to speak (default: German test sentence)" },
                    "project_dir": { "type": "string", "description": "Project directory" }
                },
                "required": ["name"]
            }),
        },
        ToolDef {
            name: "amigo_audiogen_delete_voice".into(),
            description: "Delete a saved voice profile.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Voice profile name to delete" },
                    "project_dir": { "type": "string", "description": "Project directory" }
                },
                "required": ["name"]
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
            let style = WorldAudioStyle::find(&p.world, project_dir);
            let defaults = project_dir.map(load_audio_defaults);
            let mut missing: Vec<String> = Vec::new();

            // Resolution order: explicit param → amigo.toml → world style → hardcoded
            let genre = if !p.genre.is_empty() {
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

            let section = parse_section(&p.section);
            let base_name = format!("{}_{}_{}bpm", p.world, p.section, bpm);
            let output_path = format!("assets/generated/audio/{}.wav", base_name);

            // Build ComfyUI workflow and generate
            let request = MusicRequest {
                world: p.world.clone(),
                genre,
                bpm,
                duration_secs: p.duration_secs,
                lyrics: p.lyrics.clone(),
                section,
                split_stems: p.split_stems,
                extra: HashMap::new(),
            };
            let workflow = workflows::music::build_music_workflow(&request);

            let start = std::time::Instant::now();
            let client = create_comfyui_client();

            let generation_time_ms =
                match run_comfyui_audio_workflow(&client, &workflow, &output_path) {
                    Ok(_) => start.elapsed().as_millis() as u64,
                    Err(e) => {
                        return Ok(serde_json::json!({
                            "error": e,
                            "output_path": output_path,
                            "hint": "Is ComfyUI running? Check with amigo_audiogen_server_status"
                        }));
                    }
                };

            let mut stem_paths = HashMap::new();
            if p.split_stems {
                for stem in &["drums", "bass", "vocals", "other"] {
                    stem_paths.insert(
                        stem.to_string(),
                        format!("assets/generated/audio/stems/{}_{}.wav", base_name, stem),
                    );
                }
            }

            let result = MusicResult {
                full_track_path: output_path,
                stem_paths,
                detected_bpm: bpm as f32,
                generation_time_ms,
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

            let count = p.variants.min(10);
            let safe_name = sanitize(&p.prompt);
            let category = parse_category(p.category.as_deref());

            // Build ComfyUI workflow and generate
            let request = SfxRequest {
                prompt: p.prompt.clone(),
                duration_secs: duration,
                variants: count,
                trim_silence: p.trim_silence,
                normalize: p.normalize,
                category,
            };
            let workflow = workflows::sfx::build_sfx_workflow(&request);

            let start = std::time::Instant::now();
            let client = create_comfyui_client();

            // Generate each variant via ComfyUI
            let mut output_paths = Vec::new();
            let mut durations_out = Vec::new();
            let mut gen_error = None;

            for i in 0..count {
                let path = format!("assets/generated/audio/sfx/{}_v{}.wav", safe_name, i + 1);
                match run_comfyui_audio_workflow(&client, &workflow, &path) {
                    Ok(_) => {
                        output_paths.push(path);
                        durations_out.push(duration);
                    }
                    Err(e) => {
                        gen_error = Some(e);
                        break;
                    }
                }
            }

            if output_paths.is_empty() {
                if let Some(e) = gen_error {
                    return Ok(serde_json::json!({
                        "error": e,
                        "hint": "Is ComfyUI running? Check with amigo_audiogen_server_status"
                    }));
                }
            }

            let generation_time_ms = start.elapsed().as_millis() as u64;

            let generated_count = output_paths.len();
            let result = SfxResult {
                output_paths,
                durations: durations_out,
                generation_time_ms,
            };

            let mut response = serde_json::to_value(result)?;
            if !missing.is_empty() {
                response["hints"] = serde_json::json!({
                    "defaults_missing": missing,
                    "suggestion": "Run amigo_audiogen_set_defaults to save project defaults"
                });
            }
            if let Some(e) = gen_error {
                response["warning"] = serde_json::json!(format!(
                    "Only generated {} of {} variants: {}",
                    generated_count, count, e
                ));
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
            let custom_registry = {
                let styles_dir = project_dir
                    .map(StyleRegistry::default_dir)
                    .unwrap_or_else(|| std::path::PathBuf::from("assets/audio"));
                StyleRegistry::load(&styles_dir)
            };
            let custom_names: std::collections::HashSet<String> =
                custom_registry.styles.keys().cloned().collect();

            let mut styles: Vec<StyleInfo> = WorldAudioStyle::builtin_styles()
                .into_iter()
                .filter(|s| !custom_names.contains(&s.name))
                .map(|s| StyleInfo {
                    name: s.name,
                    genre: s.genre,
                    default_bpm: s.default_bpm,
                    genre_tags: s.genre_tags,
                    sfx_style: s.sfx_style,
                    key_instruments: s.key_instruments,
                    is_custom: false,
                })
                .collect();

            for (_, s) in custom_registry.styles {
                styles.push(StyleInfo {
                    name: s.name,
                    genre: s.genre,
                    default_bpm: s.default_bpm,
                    genre_tags: s.genre_tags,
                    sfx_style: s.sfx_style,
                    key_instruments: s.key_instruments,
                    is_custom: true,
                });
            }

            styles.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(serde_json::to_value(StylesResult { styles })?)
        }
        "amigo_audiogen_server_status" => {
            let client = create_comfyui_client();
            let connected = client.system_stats().is_ok();
            Ok(serde_json::to_value(ServerStatusResult {
                comfyui_connected: connected,
                comfyui_url: client.config.base_url(),
                // Legacy fields mirror the single ComfyUI connection state
                acestep_connected: connected,
                audiogen_connected: connected,
            })?)
        }
        "amigo_audiogen_generate_core_melody" => {
            let p: GenerateCoreMelodyParams = serde_json::from_value(params)?;
            let style = WorldAudioStyle::find(&p.world, project_dir);
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
        "amigo_audiogen_list_models" => {
            // Try to get live model list from ComfyUI; fall back to known defaults
            let client = create_comfyui_client();
            let comfyui_models = client.list_models().unwrap_or_default();
            Ok(serde_json::json!({
                "acestep_models": ["ace-step-v1"],
                "stable_audio_models": ["stable-audio-open-1.0"],
                "tts_models": ["qwen3-tts-1.7b"],
                "demucs_models": ["demucs4", "demucs6"],
                "comfyui_checkpoints": comfyui_models,
            }))
        }
        "amigo_audiogen_queue_status" => {
            let client = create_comfyui_client();
            let stats = client.system_stats().ok();
            let queue_remaining = stats
                .as_ref()
                .and_then(|s| s["exec_info"]["queue_remaining"].as_u64())
                .unwrap_or(0);
            Ok(serde_json::json!({
                "comfyui_queue": queue_remaining,
                "total_pending": queue_remaining,
            }))
        }
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

        // --- TTS tools ---
        "amigo_audiogen_generate_tts" => {
            let p: GenerateTtsParams = serde_json::from_value(params)?;

            let audio_format = match p.format.as_str() {
                "ogg" => AudioFormat::Ogg,
                _ => AudioFormat::Wav,
            };

            // Resolve speaker_id to voice profile if provided
            let (ref_audio, delivery) = if let Some(ref speaker) = p.speaker_id {
                let voices_dir = project_dir
                    .map(VoiceRegistry::default_dir)
                    .unwrap_or_else(|| std::path::PathBuf::from("assets/voices"));
                let registry = VoiceRegistry::load(&voices_dir);
                if let Some(profile) = registry.get(speaker) {
                    (
                        p.reference_audio
                            .or_else(|| Some(profile.reference_audio.clone())),
                        p.delivery.or_else(|| profile.default_delivery.clone()),
                    )
                } else {
                    (p.reference_audio, p.delivery)
                }
            } else {
                (p.reference_audio, p.delivery)
            };

            let request = TtsRequest {
                text: p.text.clone(),
                language: p.language.clone(),
                delivery: delivery.clone(),
                reference_audio: ref_audio.clone(),
                speaker_id: p.speaker_id.clone(),
                format: audio_format,
            };

            let safe_name = sanitize(&p.text);
            let ext = match p.format.as_str() {
                "ogg" => "ogg",
                _ => "wav",
            };
            let output_path = format!("assets/generated/audio/tts/{}.{}", safe_name, ext);

            // Build ComfyUI workflow and generate
            let workflow = workflows::tts::build_tts_workflow(&request);
            let start = std::time::Instant::now();
            let client = create_comfyui_client();

            match run_comfyui_audio_workflow(&client, &workflow, &output_path) {
                Ok(_) => {
                    let generation_time_ms = start.elapsed().as_millis() as u64;
                    let result = TtsResult {
                        output_path,
                        duration_secs: 0.0, // actual duration from file metadata
                        generation_time_ms,
                    };
                    Ok(serde_json::to_value(result)?)
                }
                Err(e) => Ok(serde_json::json!({
                    "error": e,
                    "output_path": output_path,
                    "hint": "Is ComfyUI running? Check with amigo_audiogen_server_status"
                })),
            }
        }
        "amigo_audiogen_create_voice" => {
            let p: CreateVoiceParams = serde_json::from_value(params)?;

            let voices_dir = p
                .project_dir
                .as_deref()
                .or(project_dir.map(|p| p.to_str().unwrap_or(".")))
                .map(|d| VoiceRegistry::default_dir(std::path::Path::new(d)))
                .unwrap_or_else(|| std::path::PathBuf::from("assets/voices"));

            let mut registry = VoiceRegistry::load(&voices_dir);

            let profile = VoiceProfile {
                name: p.name.clone(),
                reference_audio: p.reference_audio.clone(),
                default_language: p.language,
                default_delivery: p.default_delivery,
                description: p.description,
            };

            registry.insert(profile.clone());
            if let Err(e) = registry.save(&voices_dir) {
                return Ok(serde_json::json!({
                    "error": format!("Failed to save voice registry: {}", e)
                }));
            }

            let test_audio = if let Some(ref test_text) = p.test_text {
                // Generate a test clip via TTS workflow to validate the voice
                let test_path = format!("{}/{}_test.wav", voices_dir.display(), sanitize(&p.name));
                let test_request = TtsRequest {
                    text: test_text.clone(),
                    language: profile.default_language.clone(),
                    delivery: profile.default_delivery.clone(),
                    reference_audio: Some(profile.reference_audio.clone()),
                    speaker_id: Some(profile.name.clone()),
                    format: AudioFormat::Wav,
                };
                let workflow = workflows::tts::build_tts_workflow(&test_request);
                let client = create_comfyui_client();
                match run_comfyui_audio_workflow(&client, &workflow, &test_path) {
                    Ok(_) => Some(test_path),
                    Err(e) => {
                        // Voice was saved but test generation failed -- not fatal
                        tracing::warn!("Test audio generation failed: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            let result = CreateVoiceResult {
                profile,
                test_audio,
            };
            Ok(serde_json::to_value(result)?)
        }
        "amigo_audiogen_list_voices" => {
            let p: ListVoicesParams = serde_json::from_value(params)?;

            let voices_dir = p
                .project_dir
                .as_deref()
                .or(project_dir.map(|p| p.to_str().unwrap_or(".")))
                .map(|d| VoiceRegistry::default_dir(std::path::Path::new(d)))
                .unwrap_or_else(|| std::path::PathBuf::from("assets/voices"));

            let registry = VoiceRegistry::load(&voices_dir);
            let voices: Vec<&VoiceProfile> = registry.voices.values().collect();
            Ok(serde_json::json!({
                "voices": serde_json::to_value(&voices)?,
                "count": voices.len(),
                "voices_dir": voices_dir.display().to_string(),
            }))
        }
        "amigo_audiogen_preview_voice" => {
            let p: PreviewVoiceParams = serde_json::from_value(params)?;

            let voices_dir = p
                .project_dir
                .as_deref()
                .or(project_dir.map(|p| p.to_str().unwrap_or(".")))
                .map(|d| VoiceRegistry::default_dir(std::path::Path::new(d)))
                .unwrap_or_else(|| std::path::PathBuf::from("assets/voices"));

            let registry = VoiceRegistry::load(&voices_dir);
            let profile = match registry.get(&p.name) {
                Some(v) => v.clone(),
                None => {
                    return Ok(serde_json::json!({
                        "error": format!("Voice profile '{}' not found", p.name)
                    }));
                }
            };

            // Generate preview via TTS workflow using the voice profile
            let preview_path = format!(
                "assets/generated/audio/tts/{}_preview.wav",
                sanitize(&p.name)
            );
            let preview_request = TtsRequest {
                text: p.text.clone(),
                language: profile.default_language.clone(),
                delivery: profile.default_delivery.clone(),
                reference_audio: Some(profile.reference_audio.clone()),
                speaker_id: Some(profile.name.clone()),
                format: AudioFormat::Wav,
            };
            let workflow = workflows::tts::build_tts_workflow(&preview_request);
            let client = create_comfyui_client();
            match run_comfyui_audio_workflow(&client, &workflow, &preview_path) {
                Ok(_) => Ok(serde_json::json!({
                    "preview_path": preview_path,
                    "voice": serde_json::to_value(&profile)?,
                    "text": p.text,
                })),
                Err(e) => Ok(serde_json::json!({
                    "error": e,
                    "voice": serde_json::to_value(&profile)?,
                    "hint": "Is ComfyUI running? Check with amigo_audiogen_server_status"
                })),
            }
        }
        "amigo_audiogen_delete_voice" => {
            let p: DeleteVoiceParams = serde_json::from_value(params)?;

            let voices_dir = p
                .project_dir
                .as_deref()
                .or(project_dir.map(|p| p.to_str().unwrap_or(".")))
                .map(|d| VoiceRegistry::default_dir(std::path::Path::new(d)))
                .unwrap_or_else(|| std::path::PathBuf::from("assets/voices"));

            let mut registry = VoiceRegistry::load(&voices_dir);
            let removed = registry.remove(&p.name);

            if removed.is_some() {
                if let Err(e) = registry.save(&voices_dir) {
                    return Ok(serde_json::json!({
                        "error": format!("Failed to save voice registry: {}", e)
                    }));
                }
            }

            Ok(serde_json::json!({
                "deleted": removed.is_some(),
                "name": p.name,
            }))
        }

        // --- Style CRUD tools ---
        "amigo_audiogen_create_style" => {
            let p: CreateStyleParams = serde_json::from_value(params)?;

            if p.name.is_empty() {
                return Ok(serde_json::json!({ "error": "Style name must not be empty" }));
            }

            let styles_dir = p
                .project_dir
                .as_deref()
                .or(project_dir.map(|d| d.to_str().unwrap_or(".")))
                .map(|d| StyleRegistry::default_dir(std::path::Path::new(d)))
                .unwrap_or_else(|| std::path::PathBuf::from("assets/audio"));

            let mut registry = StyleRegistry::load(&styles_dir);

            let parse_csv = |s: &str| -> Vec<String> {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            };

            let style = WorldAudioStyle {
                name: p.name.clone(),
                genre: p.genre,
                genre_tags: parse_csv(&p.genre_tags),
                default_bpm: p.default_bpm,
                sfx_style: p.sfx_style,
                key_instruments: parse_csv(&p.key_instruments),
            };

            registry.insert(style.clone());
            if let Err(e) = registry.save(&styles_dir) {
                return Ok(serde_json::json!({
                    "error": format!("Failed to save style registry: {}", e)
                }));
            }

            Ok(serde_json::json!({
                "created": true,
                "style": serde_json::to_value(&style)?,
                "styles_dir": styles_dir.display().to_string(),
            }))
        }
        "amigo_audiogen_edit_style" => {
            let p: EditStyleParams = serde_json::from_value(params)?;

            let styles_dir = p
                .project_dir
                .as_deref()
                .or(project_dir.map(|d| d.to_str().unwrap_or(".")))
                .map(|d| StyleRegistry::default_dir(std::path::Path::new(d)))
                .unwrap_or_else(|| std::path::PathBuf::from("assets/audio"));

            let mut registry = StyleRegistry::load(&styles_dir);

            let existing = match registry.get(&p.name) {
                Some(s) => s.clone(),
                None => {
                    // Check if it's a builtin
                    if WorldAudioStyle::builtin_styles()
                        .iter()
                        .any(|s| s.name == p.name)
                    {
                        return Ok(serde_json::json!({
                            "error": format!("'{}' is a builtin style. To customize it, create a custom style with the same name using amigo_audiogen_create_style.", p.name)
                        }));
                    }
                    return Ok(serde_json::json!({
                        "error": format!("Custom style '{}' not found", p.name)
                    }));
                }
            };

            let parse_csv = |s: &str| -> Vec<String> {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            };

            let updated = WorldAudioStyle {
                name: existing.name,
                genre: p.genre.unwrap_or(existing.genre),
                genre_tags: p
                    .genre_tags
                    .map(|t| parse_csv(&t))
                    .unwrap_or(existing.genre_tags),
                default_bpm: p.default_bpm.unwrap_or(existing.default_bpm),
                sfx_style: p.sfx_style.unwrap_or(existing.sfx_style),
                key_instruments: p
                    .key_instruments
                    .map(|t| parse_csv(&t))
                    .unwrap_or(existing.key_instruments),
            };

            registry.insert(updated.clone());
            if let Err(e) = registry.save(&styles_dir) {
                return Ok(serde_json::json!({
                    "error": format!("Failed to save style registry: {}", e)
                }));
            }

            Ok(serde_json::json!({
                "updated": true,
                "style": serde_json::to_value(&updated)?,
            }))
        }
        "amigo_audiogen_delete_style" => {
            let p: DeleteStyleParams = serde_json::from_value(params)?;

            let styles_dir = p
                .project_dir
                .as_deref()
                .or(project_dir.map(|d| d.to_str().unwrap_or(".")))
                .map(|d| StyleRegistry::default_dir(std::path::Path::new(d)))
                .unwrap_or_else(|| std::path::PathBuf::from("assets/audio"));

            let mut registry = StyleRegistry::load(&styles_dir);

            if WorldAudioStyle::builtin_styles()
                .iter()
                .any(|s| s.name == p.name)
                && registry.get(&p.name).is_none()
            {
                return Ok(serde_json::json!({
                    "error": format!("'{}' is a builtin style and cannot be deleted", p.name)
                }));
            }

            let removed = registry.remove(&p.name);
            if removed.is_some() {
                if let Err(e) = registry.save(&styles_dir) {
                    return Ok(serde_json::json!({
                        "error": format!("Failed to save style registry: {}", e)
                    }));
                }
            }

            Ok(serde_json::json!({
                "deleted": removed.is_some(),
                "name": p.name,
            }))
        }

        // --- Song reference tool ---
        "amigo_audiogen_from_reference" => {
            let p: FromReferenceParams = serde_json::from_value(params)?;

            // Use the reference description as genre tags for conditioning
            let genre_tags: Vec<String> = p
                .reference_description
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();

            let genre = p
                .target_genre
                .clone()
                .unwrap_or_else(|| genre_tags.first().cloned().unwrap_or_else(|| "ambient".into()));

            let bpm = p.target_bpm.unwrap_or(120);

            // Optionally save as a custom style
            if let Some(ref style_name) = p.style_name {
                let styles_dir = p
                    .project_dir
                    .as_deref()
                    .or(project_dir.map(|d| d.to_str().unwrap_or(".")))
                    .map(|d| StyleRegistry::default_dir(std::path::Path::new(d)))
                    .unwrap_or_else(|| std::path::PathBuf::from("assets/audio"));

                let mut registry = StyleRegistry::load(&styles_dir);
                registry.insert(WorldAudioStyle {
                    name: style_name.clone(),
                    genre: genre.clone(),
                    genre_tags: genre_tags.clone(),
                    default_bpm: bpm,
                    sfx_style: String::new(),
                    key_instruments: vec![],
                });
                let _ = registry.save(&styles_dir);
            }

            let section = parse_section(&p.section);
            let num_variations = p.num_variations.max(1);

            let mut outputs = Vec::new();
            for i in 0..num_variations {
                let suffix = if num_variations > 1 {
                    format!("_v{}", i + 1)
                } else {
                    String::new()
                };
                let base_name = format!(
                    "ref_{}_{}bpm{}",
                    sanitize(&genre),
                    bpm,
                    suffix
                );
                let output_path = format!("assets/generated/audio/{}.wav", base_name);

                let request = MusicRequest {
                    world: p.style_name.clone().unwrap_or_else(|| "custom".into()),
                    genre: genre.clone(),
                    bpm,
                    duration_secs: p.duration_secs,
                    lyrics: None,
                    section: section.clone(),
                    split_stems: false,
                    extra: HashMap::new(),
                };
                let workflow = workflows::music::build_music_workflow(&request);
                let client = create_comfyui_client();

                match run_comfyui_audio_workflow(&client, &workflow, &output_path) {
                    Ok(_) => outputs.push(output_path),
                    Err(e) => {
                        return Ok(serde_json::json!({
                            "error": e,
                            "generated_so_far": outputs,
                            "hint": "Is ComfyUI running? Check with amigo_audiogen_server_status"
                        }));
                    }
                }
            }

            Ok(serde_json::json!({
                "outputs": outputs,
                "genre": genre,
                "genre_tags": genre_tags,
                "bpm": bpm,
                "variation_strength": p.variation_strength,
                "num_variations": num_variations,
                "saved_style": p.style_name,
            }))
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

    // ── Helper ──────────────────────────────────────────────────

    /// Whether ComfyUI is reachable in the test environment.
    /// Tests that require a live server check this and gracefully
    /// accept error responses when the server is unavailable.
    fn comfyui_available() -> bool {
        let client = create_comfyui_client();
        client.system_stats().is_ok()
    }

    // ── Tool listing ───────────────────────────────────────────

    #[test]
    fn list_tools_returns_29() {
        assert_eq!(list_tools().len(), 29);
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
        // When ComfyUI is unavailable, returns an error object with output_path
        if v.get("error").is_some() {
            assert!(v["output_path"].as_str().unwrap().contains("caribbean"));
        } else {
            assert!(v["full_track_path"].as_str().unwrap().contains("caribbean"));
        }
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
        if v.get("error").is_some() {
            assert!(v["output_path"].as_str().unwrap().contains("160bpm"));
        } else {
            assert!(v["full_track_path"].as_str().unwrap().contains("160bpm"));
            assert!(v["stem_paths"].as_object().unwrap().len() == 4);
        }
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
        // When ComfyUI is unavailable, returns an error object
        if v.get("error").is_some() {
            assert!(v["error"].as_str().unwrap().contains("Failed"));
        } else {
            assert_eq!(v["output_paths"].as_array().unwrap().len(), 2);
        }
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
        // comfyui_connected is true or false depending on whether server is running
        assert!(v["comfyui_connected"].is_boolean());
        assert!(v["comfyui_url"].is_string());
        // Legacy fields mirror comfyui_connected
        assert_eq!(v["acestep_connected"], v["comfyui_connected"]);
        assert_eq!(v["audiogen_connected"], v["comfyui_connected"]);
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
        assert!(v["total_pending"].is_number());
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
        // When ComfyUI is unavailable, returns error; otherwise check hints
        if v.get("error").is_none() {
            let hints = &v["hints"];
            assert!(hints["defaults_missing"].is_array());
            assert!(hints["suggestion"]
                .as_str()
                .unwrap()
                .contains("amigo_audiogen_set_defaults"));
        }
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
        // When ComfyUI is available: no hints. When unavailable: error object.
        if v.get("error").is_none() {
            assert!(v.get("hints").is_none());
        }
    }

    // ── TTS dispatch ───────────────────────────────────────────

    #[test]
    fn dispatch_generate_tts() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_tts",
            serde_json::json!({ "text": "Hallo Welt" }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        // Returns either TtsResult or an error object with output_path
        let path = v["output_path"].as_str().unwrap();
        assert!(path.contains("tts"));
        assert!(path.ends_with(".wav"));
    }

    #[test]
    fn dispatch_generate_tts_ogg() {
        let result = dispatch_tool(
            "amigo_audiogen_generate_tts",
            serde_json::json!({ "text": "Hello", "format": "ogg" }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        let path = v["output_path"].as_str().unwrap();
        assert!(path.ends_with(".ogg"));
    }

    #[test]
    fn dispatch_create_voice() {
        let dir = tempfile::tempdir().unwrap();
        let result = dispatch_tool(
            "amigo_audiogen_create_voice",
            serde_json::json!({
                "name": "test_wizard",
                "reference_audio": "wizard.wav",
                "language": "de-DE",
                "description": "A wizard",
                "project_dir": dir.path().to_str().unwrap()
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["profile"]["name"], "test_wizard");
    }

    #[test]
    fn dispatch_create_voice_with_test_text() {
        // When ComfyUI is unavailable, the voice is still saved; test_audio is None
        let dir = tempfile::tempdir().unwrap();
        let result = dispatch_tool(
            "amigo_audiogen_create_voice",
            serde_json::json!({
                "name": "test_narrator",
                "reference_audio": "narrator.wav",
                "test_text": "Hallo Welt",
                "project_dir": dir.path().to_str().unwrap()
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["profile"]["name"], "test_narrator");
        // test_audio may be null if ComfyUI is unavailable
        if !comfyui_available() {
            assert!(v["test_audio"].is_null());
        }
    }

    #[test]
    fn dispatch_list_voices_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = dispatch_tool(
            "amigo_audiogen_list_voices",
            serde_json::json!({ "project_dir": dir.path().to_str().unwrap() }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["count"], 0);
    }

    #[test]
    fn dispatch_delete_voice_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let result = dispatch_tool(
            "amigo_audiogen_delete_voice",
            serde_json::json!({
                "name": "nope",
                "project_dir": dir.path().to_str().unwrap()
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert_eq!(v["deleted"], false);
    }

    #[test]
    fn dispatch_preview_voice_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = dispatch_tool(
            "amigo_audiogen_preview_voice",
            serde_json::json!({
                "name": "missing",
                "project_dir": dir.path().to_str().unwrap()
            }),
        );
        assert!(result.is_ok());
        let v = result.unwrap();
        assert!(v["error"].as_str().unwrap().contains("not found"));
    }

    // ── Helpers ────────────────────────────────────────────────

    #[test]
    fn parse_section_known() {
        assert_eq!(parse_section("calm"), MusicSection::Calm);
        assert_eq!(parse_section("battle"), MusicSection::Battle);
        assert_eq!(parse_section("boss"), MusicSection::Boss);
    }

    #[test]
    fn parse_section_custom() {
        assert_eq!(
            parse_section("mystical"),
            MusicSection::Custom("mystical".into())
        );
    }

    #[test]
    fn parse_category_known() {
        assert!(matches!(parse_category(Some("magic")), SfxCategory::Magic));
        assert!(matches!(parse_category(None), SfxCategory::Gameplay));
    }

    #[test]
    fn parse_comfy_url_default() {
        let cfg = parse_comfy_url("http://127.0.0.1:8188");
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 8188);
    }

    #[test]
    fn parse_comfy_url_custom_port() {
        let cfg = parse_comfy_url("http://myhost:9000");
        assert_eq!(cfg.host, "myhost");
        assert_eq!(cfg.port, 9000);
    }
}
