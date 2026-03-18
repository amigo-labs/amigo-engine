//! Stem splitting for adaptive music.
//!
//! Takes a full music track and splits it into individual instrument stems
//! (drums, bass, melody, vocals, etc.) using source separation models.
//! The stems are then used by the adaptive music engine for vertical layering.

use crate::processing::{AdaptiveMusicConfig, LayerConfig, LayerRule};
use crate::MusicSection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Stem separation model to use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StemModel {
    /// Demucs (4 stems: drums, bass, vocals, other)
    Demucs,
    /// Demucs with 6-stem fine-tuned model
    Demucs6,
    /// Custom model path
    Custom(String),
}

impl Default for StemModel {
    fn default() -> Self {
        Self::Demucs
    }
}

/// Configuration for stem splitting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StemSplitConfig {
    pub model: StemModel,
    /// Output directory for stems.
    pub output_dir: String,
    /// Audio format for stems.
    pub format: AudioFormat,
    /// Whether to normalize each stem individually.
    pub normalize_stems: bool,
}

impl Default for StemSplitConfig {
    fn default() -> Self {
        Self {
            model: StemModel::Demucs,
            output_dir: "assets/audio/stems".into(),
            format: AudioFormat::Ogg,
            normalize_stems: true,
        }
    }
}

/// Output audio format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Wav,
    Ogg,
}

/// Result of stem splitting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StemSplitResult {
    /// Map of stem name → file path.
    pub stems: HashMap<String, String>,
    /// Processing time in milliseconds.
    pub processing_time_ms: u64,
}

/// The standard stems produced by Demucs.
pub const DEMUCS_STEMS: &[&str] = &["drums", "bass", "vocals", "other"];

/// Extended stems for 6-stem model.
pub const DEMUCS6_STEMS: &[&str] = &["drums", "bass", "vocals", "guitar", "piano", "other"];

/// Split a track into stems.
///
/// Placeholder: actual implementation calls the Demucs model via subprocess
/// or a Python bridge.
pub fn split_stems(
    input_path: &str,
    config: &StemSplitConfig,
) -> Result<StemSplitResult, StemError> {
    if input_path.is_empty() {
        return Err(StemError::InvalidInput("Empty input path".into()));
    }

    let stem_names = match config.model {
        StemModel::Demucs => DEMUCS_STEMS,
        StemModel::Demucs6 => DEMUCS6_STEMS,
        StemModel::Custom(_) => DEMUCS_STEMS,
    };

    let ext = match config.format {
        AudioFormat::Wav => "wav",
        AudioFormat::Ogg => "ogg",
    };

    let base_name = std::path::Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("track");

    let mut stems = HashMap::new();
    for name in stem_names {
        let path = format!("{}/{}_{}.{}", config.output_dir, base_name, name, ext);
        stems.insert(name.to_string(), path);
    }

    // Placeholder: would run demucs here
    Ok(StemSplitResult {
        stems,
        processing_time_ms: 0,
    })
}

/// Generate an adaptive music config from stems and section info.
pub fn generate_adaptive_config(
    stems: &StemSplitResult,
    section: &MusicSection,
    bpm: f32,
) -> AdaptiveMusicConfig {
    let section_name = match section {
        MusicSection::Calm => "calm",
        MusicSection::Tense => "tense",
        MusicSection::Battle => "battle",
        MusicSection::Boss => "boss",
        MusicSection::Victory => "victory",
        MusicSection::Menu => "menu",
        MusicSection::Custom(s) => s,
    };

    let mut layers = Vec::new();

    for (name, path) in &stems.stems {
        let rule = match name.as_str() {
            "drums" => LayerRule::Threshold {
                param: "tension".into(),
                above: 0.3,
                fade_secs: 1.0,
            },
            "bass" => LayerRule::AlwaysOn,
            "vocals" => LayerRule::Threshold {
                param: "boss".into(),
                above: 0.5,
                fade_secs: 2.0,
            },
            "other" | "guitar" | "piano" => LayerRule::Lerp {
                param: "tension".into(),
                from: 0.2,
                to: 1.0,
            },
            _ => LayerRule::AlwaysOn,
        };

        let base_volume = match name.as_str() {
            "drums" => 0.8,
            "bass" => 0.7,
            "vocals" => 0.6,
            _ => 0.5,
        };

        layers.push(LayerConfig {
            name: name.clone(),
            stem_file: path.clone(),
            base_volume,
            rule,
        });
    }

    // Sort layers for deterministic order
    layers.sort_by(|a, b| a.name.cmp(&b.name));

    AdaptiveMusicConfig {
        section_name: section_name.to_string(),
        bpm,
        beats_per_bar: 4,
        layers,
    }
}

/// Stem splitting errors.
#[derive(Debug, thiserror::Error)]
pub enum StemError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Processing failed: {0}")]
    ProcessingFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Stem splitting ─────────────────────────────────────────

    #[test]
    fn split_stems_demucs() {
        let config = StemSplitConfig::default();
        let result = split_stems("track.wav", &config).unwrap();

        assert_eq!(result.stems.len(), 4);
        assert!(result.stems.contains_key("drums"));
        assert!(result.stems.contains_key("bass"));
        assert!(result.stems.contains_key("vocals"));
        assert!(result.stems.contains_key("other"));
    }

    #[test]
    fn split_stems_6_model() {
        let config = StemSplitConfig {
            model: StemModel::Demucs6,
            ..Default::default()
        };
        let result = split_stems("track.wav", &config).unwrap();
        assert_eq!(result.stems.len(), 6);
        assert!(result.stems.contains_key("guitar"));
        assert!(result.stems.contains_key("piano"));
    }

    #[test]
    fn split_stems_empty_input_fails() {
        let config = StemSplitConfig::default();
        assert!(split_stems("", &config).is_err());
    }

    // ── Adaptive config generation ──────────────────────────────

    #[test]
    fn generate_config_has_all_layers() {
        let config = StemSplitConfig::default();
        let result = split_stems("battle_theme.ogg", &config).unwrap();
        let adaptive = generate_adaptive_config(&result, &MusicSection::Battle, 140.0);

        assert_eq!(adaptive.section_name, "battle");
        assert_eq!(adaptive.bpm, 140.0);
        assert_eq!(adaptive.layers.len(), 4);
    }

    #[test]
    fn stem_paths_include_base_name() {
        let config = StemSplitConfig::default();
        let result = split_stems("caribbean_calm.ogg", &config).unwrap();
        assert!(result.stems["drums"].contains("caribbean_calm_drums"));
    }

    #[test]
    fn adaptive_config_drum_rule() {
        let config = StemSplitConfig::default();
        let result = split_stems("track.wav", &config).unwrap();
        let adaptive = generate_adaptive_config(&result, &MusicSection::Calm, 120.0);

        let drums = adaptive.layers.iter().find(|l| l.name == "drums").unwrap();
        match &drums.rule {
            LayerRule::Threshold { param, above, .. } => {
                assert_eq!(param, "tension");
                assert!(*above > 0.0);
            }
            _ => panic!("Expected Threshold rule for drums"),
        }
    }
}
