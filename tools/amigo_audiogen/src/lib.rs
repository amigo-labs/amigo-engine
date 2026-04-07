//! amigo_audiogen — AI audio generation pipeline for Amigo Engine.
//!
//! Triple-mode audio generation via ComfyUI:
//! - **ACE-Step**: Full music tracks with lyrics/melody conditioning, then stem
//!   splitting for adaptive music layers.
//! - **Stable Audio Open**: Short SFX clips (impacts, UI sounds, ambient loops).
//! - **Qwen3-TTS 1.7B**: Text-to-speech with voice cloning and emotion control.
//!
//! All backends run through a single ComfyUI instance. This crate provides
//! workflow builders, audio processing utilities, and MCP tool interface.

pub mod config;

pub mod clean_mode;
pub mod processing;
pub mod stems;
pub mod style_registry;
pub mod tools;
pub mod voice_registry;
pub mod workflows;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A request to generate music via ACE-Step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicRequest {
    /// The world/theme for style conditioning.
    pub world: String,
    /// Musical genre override (if empty, uses world default).
    pub genre: String,
    /// Target BPM.
    pub bpm: u32,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Optional lyrics for vocal conditioning.
    pub lyrics: Option<String>,
    /// Which section this is (calm, tense, battle, boss, victory).
    pub section: MusicSection,
    /// Whether to split into stems after generation.
    pub split_stems: bool,
    /// Extra parameters.
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for MusicRequest {
    fn default() -> Self {
        Self {
            world: "default".into(),
            genre: String::new(),
            bpm: 120,
            duration_secs: 30.0,
            lyrics: None,
            section: MusicSection::Calm,
            split_stems: true,
            extra: HashMap::new(),
        }
    }
}

/// Music section types for adaptive music.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MusicSection {
    Calm,
    Tense,
    Battle,
    Boss,
    Victory,
    Menu,
    Custom(String),
}

/// A request to generate SFX via AudioGen.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxRequest {
    /// Descriptive prompt for the sound effect.
    pub prompt: String,
    /// Duration in seconds (max ~10s for AudioGen).
    pub duration_secs: f32,
    /// Number of variants to generate.
    pub variants: u32,
    /// Whether to trim silence from start/end.
    pub trim_silence: bool,
    /// Whether to normalize volume.
    pub normalize: bool,
    /// Optional category for organization.
    pub category: SfxCategory,
}

impl Default for SfxRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            duration_secs: 2.0,
            variants: 3,
            trim_silence: true,
            normalize: true,
            category: SfxCategory::Gameplay,
        }
    }
}

/// SFX categories for organization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SfxCategory {
    Gameplay,
    UI,
    Ambient,
    Impact,
    Explosion,
    Magic,
    Voice,
    Custom(String),
}

/// Result of a music generation job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicResult {
    /// Path to the full mixed track.
    pub full_track_path: String,
    /// Paths to individual stems (if split_stems was true).
    pub stem_paths: HashMap<String, String>,
    /// Detected BPM of the output.
    pub detected_bpm: f32,
    /// Generation time in milliseconds.
    pub generation_time_ms: u64,
}

/// Result of an SFX generation job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxResult {
    /// Paths to generated audio files.
    pub output_paths: Vec<String>,
    /// Duration of each variant in seconds.
    pub durations: Vec<f32>,
    /// Generation time in milliseconds.
    pub generation_time_ms: u64,
}

// ---------------------------------------------------------------------------
// World audio style definitions
// ---------------------------------------------------------------------------

/// Audio style configuration per world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldAudioStyle {
    pub name: String,
    /// Primary music genre.
    pub genre: String,
    /// Genre descriptors for ACE-Step conditioning.
    pub genre_tags: Vec<String>,
    /// Default BPM for this world.
    pub default_bpm: u32,
    /// SFX style description prefix.
    pub sfx_style: String,
    /// Instruments to emphasize in stems.
    pub key_instruments: Vec<String>,
}

impl WorldAudioStyle {
    pub fn builtin_styles() -> Vec<WorldAudioStyle> {
        vec![
            WorldAudioStyle {
                name: "caribbean".into(),
                genre: "pirate shanty".into(),
                genre_tags: vec![
                    "folk".into(),
                    "sea shanty".into(),
                    "accordion".into(),
                    "fiddle".into(),
                ],
                default_bpm: 130,
                sfx_style: "wooden, ocean, cannon, ".into(),
                key_instruments: vec![
                    "accordion".into(),
                    "fiddle".into(),
                    "drums".into(),
                    "bass".into(),
                ],
            },
            WorldAudioStyle {
                name: "lotr".into(),
                genre: "orchestral fantasy".into(),
                genre_tags: vec![
                    "orchestral".into(),
                    "epic".into(),
                    "choir".into(),
                    "strings".into(),
                ],
                default_bpm: 100,
                sfx_style: "medieval, metallic, magical, ".into(),
                key_instruments: vec![
                    "strings".into(),
                    "brass".into(),
                    "choir".into(),
                    "drums".into(),
                ],
            },
            WorldAudioStyle {
                name: "dune".into(),
                genre: "ambient electronic".into(),
                genre_tags: vec![
                    "ambient".into(),
                    "electronic".into(),
                    "drone".into(),
                    "world music".into(),
                ],
                default_bpm: 90,
                sfx_style: "desert, sand, wind, mechanical, ".into(),
                key_instruments: vec![
                    "synth pad".into(),
                    "percussion".into(),
                    "bass drone".into(),
                    "vocal".into(),
                ],
            },
            WorldAudioStyle {
                name: "matrix".into(),
                genre: "synthwave".into(),
                genre_tags: vec![
                    "synthwave".into(),
                    "industrial".into(),
                    "electronic".into(),
                    "dark".into(),
                ],
                default_bpm: 140,
                sfx_style: "digital, glitch, electric, cyberpunk, ".into(),
                key_instruments: vec![
                    "synth lead".into(),
                    "synth bass".into(),
                    "drums".into(),
                    "arpeggios".into(),
                ],
            },
            WorldAudioStyle {
                name: "got".into(),
                genre: "medieval orchestral".into(),
                genre_tags: vec![
                    "medieval".into(),
                    "dark orchestral".into(),
                    "cello".into(),
                    "war drums".into(),
                ],
                default_bpm: 85,
                sfx_style: "sword, fire, stone, dark, ".into(),
                key_instruments: vec![
                    "cello".into(),
                    "war drums".into(),
                    "brass".into(),
                    "strings".into(),
                ],
            },
            WorldAudioStyle {
                name: "stranger_things".into(),
                genre: "80s synth".into(),
                genre_tags: vec![
                    "80s synth".into(),
                    "retro".into(),
                    "analog".into(),
                    "horror".into(),
                ],
                default_bpm: 110,
                sfx_style: "retro, analog, eerie, electric, ".into(),
                key_instruments: vec![
                    "analog synth".into(),
                    "drums machine".into(),
                    "bass synth".into(),
                    "pad".into(),
                ],
            },
        ]
    }

    /// Find a style by name. Custom styles (from project registry) take
    /// precedence over builtins.
    pub fn find(name: &str, project_dir: Option<&Path>) -> Option<WorldAudioStyle> {
        // Check custom styles first
        if let Some(dir) = project_dir {
            let registry = style_registry::StyleRegistry::load(
                &style_registry::StyleRegistry::default_dir(dir),
            );
            if let Some(s) = registry.get(name) {
                return Some(s.clone());
            }
        }
        Self::builtin_styles().into_iter().find(|s| s.name == name)
    }

    /// Return all styles (builtins + custom). Custom styles override builtins
    /// with the same name.
    pub fn all_styles(project_dir: Option<&Path>) -> Vec<WorldAudioStyle> {
        let mut by_name: HashMap<String, WorldAudioStyle> = HashMap::new();
        for s in Self::builtin_styles() {
            by_name.insert(s.name.clone(), s);
        }
        if let Some(dir) = project_dir {
            let registry = style_registry::StyleRegistry::load(
                &style_registry::StyleRegistry::default_dir(dir),
            );
            for (_, s) in registry.styles {
                by_name.insert(s.name.clone(), s);
            }
        }
        let mut styles: Vec<_> = by_name.into_values().collect();
        styles.sort_by(|a, b| a.name.cmp(&b.name));
        styles
    }
}

// ---------------------------------------------------------------------------
// Audio backend selection
// ---------------------------------------------------------------------------

/// Audio backend selection (replaces the implicit Gradio clients).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AudioBackend {
    /// ACE-Step via ComfyUI for music generation.
    AceStep,
    /// Stable Audio Open via ComfyUI for SFX generation.
    StableAudio,
    /// Qwen3-TTS 1.7B via ComfyUI for speech synthesis.
    Qwen3Tts,
}

// ---------------------------------------------------------------------------
// TTS types
// ---------------------------------------------------------------------------

/// A request to generate speech via Qwen3-TTS.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsRequest {
    /// The text to speak.
    pub text: String,
    /// Language (BCP-47). Default: "de-DE".
    pub language: String,
    /// Delivery instruction for emotion/style.
    /// e.g. "speak with anger", "whisper softly", "excited and fast".
    pub delivery: Option<String>,
    /// Path to reference audio for voice cloning (10s is enough).
    /// If None, the model's default voice is used.
    pub reference_audio: Option<String>,
    /// Speaker name for consistent assignment (e.g. "narrator", "npc_guard").
    pub speaker_id: Option<String>,
    /// Output format.
    pub format: AudioFormat,
}

impl Default for TtsRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            language: "de-DE".into(),
            delivery: None,
            reference_audio: None,
            speaker_id: None,
            format: AudioFormat::default(),
        }
    }
}

/// Audio output format.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum AudioFormat {
    #[default]
    Wav,
    Ogg,
}

/// TTS generation result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsResult {
    /// Path to the generated audio file.
    pub output_path: String,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Generation time in milliseconds.
    pub generation_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Voice creation and management
// ---------------------------------------------------------------------------

/// A saved voice profile for TTS voice cloning.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceProfile {
    /// Unique name (e.g. "old_wizard", "young_princess", "narrator_de").
    pub name: String,
    /// Path to reference audio (WAV, 10-30s recommended).
    pub reference_audio: String,
    /// Default language for this voice.
    pub default_language: String,
    /// Default delivery instruction (e.g. "speak slowly with gravitas").
    pub default_delivery: Option<String>,
    /// Description of the voice (for MCP tool display).
    pub description: Option<String>,
}

/// Request to create a voice profile.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateVoiceRequest {
    /// Name for the new voice.
    pub name: String,
    /// Reference audio: either a path to an existing file or "record"
    /// for microphone recording.
    pub reference_audio: String,
    /// Language. Default: "de-DE".
    pub language: String,
    /// Default delivery for this voice.
    pub default_delivery: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Optional test text -- spoken after creation to validate the voice.
    pub test_text: Option<String>,
}

impl Default for CreateVoiceRequest {
    fn default() -> Self {
        Self {
            name: String::new(),
            reference_audio: String::new(),
            language: "de-DE".into(),
            default_delivery: None,
            description: None,
            test_text: None,
        }
    }
}

/// Result of voice profile creation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateVoiceResult {
    /// The saved profile.
    pub profile: VoiceProfile,
    /// Path to test audio file (if test_text was set).
    pub test_audio: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_music_request() {
        let req = MusicRequest::default();
        assert_eq!(req.bpm, 120);
        assert_eq!(req.duration_secs, 30.0);
        assert!(req.split_stems);
    }

    #[test]
    fn default_sfx_request() {
        let req = SfxRequest::default();
        assert_eq!(req.variants, 3);
        assert!(req.trim_silence);
    }

    #[test]
    fn world_audio_styles() {
        let styles = WorldAudioStyle::builtin_styles();
        assert_eq!(styles.len(), 6);
        assert!(WorldAudioStyle::find("matrix", None).is_some());
        assert!(WorldAudioStyle::find("nonexistent", None).is_none());
    }

    #[test]
    fn caribbean_is_shanty() {
        let style = WorldAudioStyle::find("caribbean", None).unwrap();
        assert!(style.genre.contains("shanty"));
        assert_eq!(style.default_bpm, 130);
    }

    #[test]
    fn default_tts_request() {
        let req = TtsRequest::default();
        assert_eq!(req.language, "de-DE");
        assert!(matches!(req.format, AudioFormat::Wav));
        assert!(req.text.is_empty());
    }

    #[test]
    fn default_create_voice_request() {
        let req = CreateVoiceRequest::default();
        assert_eq!(req.language, "de-DE");
        assert!(req.name.is_empty());
    }

    #[test]
    fn audio_backend_variants() {
        let be = AudioBackend::AceStep;
        assert!(matches!(be, AudioBackend::AceStep));
        let be = AudioBackend::StableAudio;
        assert!(matches!(be, AudioBackend::StableAudio));
        let be = AudioBackend::Qwen3Tts;
        assert!(matches!(be, AudioBackend::Qwen3Tts));
    }
}
