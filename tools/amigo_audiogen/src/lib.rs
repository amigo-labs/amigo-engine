//! amigo_audiogen — AI audio generation pipeline for Amigo Engine.
//!
//! Dual-mode audio generation:
//! - **ACE-Step**: Full music tracks with lyrics/melody conditioning, then stem
//!   splitting for adaptive music layers.
//! - **AudioGen**: Short SFX clips (impacts, UI sounds, ambient loops).
//!
//! Both backends run locally on GPU. This crate provides the client libraries,
//! audio processing utilities, and MCP tool interface.

pub mod acestep;
pub mod audiogen;
pub mod processing;
pub mod stems;
pub mod tools;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
                genre_tags: vec!["folk".into(), "sea shanty".into(), "accordion".into(), "fiddle".into()],
                default_bpm: 130,
                sfx_style: "wooden, ocean, cannon, ".into(),
                key_instruments: vec!["accordion".into(), "fiddle".into(), "drums".into(), "bass".into()],
            },
            WorldAudioStyle {
                name: "lotr".into(),
                genre: "orchestral fantasy".into(),
                genre_tags: vec!["orchestral".into(), "epic".into(), "choir".into(), "strings".into()],
                default_bpm: 100,
                sfx_style: "medieval, metallic, magical, ".into(),
                key_instruments: vec!["strings".into(), "brass".into(), "choir".into(), "drums".into()],
            },
            WorldAudioStyle {
                name: "dune".into(),
                genre: "ambient electronic".into(),
                genre_tags: vec!["ambient".into(), "electronic".into(), "drone".into(), "world music".into()],
                default_bpm: 90,
                sfx_style: "desert, sand, wind, mechanical, ".into(),
                key_instruments: vec!["synth pad".into(), "percussion".into(), "bass drone".into(), "vocal".into()],
            },
            WorldAudioStyle {
                name: "matrix".into(),
                genre: "synthwave".into(),
                genre_tags: vec!["synthwave".into(), "industrial".into(), "electronic".into(), "dark".into()],
                default_bpm: 140,
                sfx_style: "digital, glitch, electric, cyberpunk, ".into(),
                key_instruments: vec!["synth lead".into(), "synth bass".into(), "drums".into(), "arpeggios".into()],
            },
            WorldAudioStyle {
                name: "got".into(),
                genre: "medieval orchestral".into(),
                genre_tags: vec!["medieval".into(), "dark orchestral".into(), "cello".into(), "war drums".into()],
                default_bpm: 85,
                sfx_style: "sword, fire, stone, dark, ".into(),
                key_instruments: vec!["cello".into(), "war drums".into(), "brass".into(), "strings".into()],
            },
            WorldAudioStyle {
                name: "stranger_things".into(),
                genre: "80s synth".into(),
                genre_tags: vec!["80s synth".into(), "retro".into(), "analog".into(), "horror".into()],
                default_bpm: 110,
                sfx_style: "retro, analog, eerie, electric, ".into(),
                key_instruments: vec!["analog synth".into(), "drums machine".into(), "bass synth".into(), "pad".into()],
            },
        ]
    }

    pub fn find(name: &str) -> Option<WorldAudioStyle> {
        Self::builtin_styles().into_iter().find(|s| s.name == name)
    }
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
        assert!(WorldAudioStyle::find("matrix").is_some());
        assert!(WorldAudioStyle::find("nonexistent").is_none());
    }

    #[test]
    fn caribbean_is_shanty() {
        let style = WorldAudioStyle::find("caribbean").unwrap();
        assert!(style.genre.contains("shanty"));
        assert_eq!(style.default_bpm, 130);
    }
}
