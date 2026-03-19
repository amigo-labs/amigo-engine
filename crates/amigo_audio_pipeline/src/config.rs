use std::collections::HashMap;
use std::path::Path;

/// Full pipeline configuration, typically loaded from pipeline.yaml or pipeline.toml.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineConfig {
    #[serde(default = "default_pipeline_name")]
    pub name: String,
    #[serde(default)]
    pub separation: SeparationConfig,
    #[serde(default)]
    pub transcription: TranscriptionConfig,
    #[serde(default)]
    pub conversion: ConversionConfig,
    #[serde(default)]
    pub postprocessing: PostprocessConfig,
}

fn default_pipeline_name() -> String {
    "default".into()
}

impl Default for PipelineConfig {
    fn default() -> Self {
        toml::from_str(DEFAULT_CONFIG).expect("default config should parse")
    }
}

impl PipelineConfig {
    /// Load config from a TOML file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

/// Source separation (Demucs) configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SeparationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_true")]
    pub skip_if_mono: bool,
    #[serde(default = "default_stem_count")]
    pub stem_count: u32,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default)]
    pub stem_mapping: HashMap<String, String>,
}

impl Default for SeparationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model: "htdemucs".into(),
            skip_if_mono: true,
            stem_count: 4,
            sample_rate: 44100,
            output_format: "wav".into(),
            stem_mapping: [
                ("vocals".into(), "melody".into()),
                ("bass".into(), "bass".into()),
                ("drums".into(), "percussion".into()),
                ("other".into(), "harmony".into()),
            ]
            .into(),
        }
    }
}

/// Audio-to-MIDI transcription (Basic Pitch) configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptionConfig {
    #[serde(default = "default_onset_threshold")]
    pub onset_threshold: f64,
    #[serde(default = "default_frame_threshold")]
    pub frame_threshold: f64,
    #[serde(default = "default_min_note_length_ms")]
    pub min_note_length_ms: u32,
    #[serde(default = "default_min_frequency")]
    pub min_frequency_hz: f64,
    #[serde(default = "default_max_frequency")]
    pub max_frequency_hz: f64,
    #[serde(default)]
    pub midi_tempo_bpm: Option<f64>,
    #[serde(default)]
    pub pitch_bend: bool,
    #[serde(default = "default_quantize_grid")]
    pub quantize_grid: u32,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            onset_threshold: 0.5,
            frame_threshold: 0.3,
            min_note_length_ms: 50,
            min_frequency_hz: 27.5,
            max_frequency_hz: 4186.0,
            midi_tempo_bpm: None,
            pitch_bend: false,
            quantize_grid: 16,
        }
    }
}

/// MIDI-to-TidalCycles conversion configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversionConfig {
    #[serde(default = "default_resolution")]
    pub resolution: u32,
    #[serde(default = "default_true")]
    pub include_legato: bool,
    #[serde(default = "default_true")]
    pub include_amplitude: bool,
    #[serde(default = "default_true")]
    pub consolidate: bool,
    #[serde(default = "default_output_fmt")]
    pub output_format: String,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            resolution: 16,
            include_legato: true,
            include_amplitude: true,
            consolidate: true,
            output_format: "amigo_tidal".into(),
        }
    }
}

/// Post-processing configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostprocessConfig {
    #[serde(default = "default_true")]
    pub remove_ghost_notes: bool,
    #[serde(default = "default_ghost_threshold")]
    pub ghost_note_threshold: f64,
    #[serde(default = "default_true")]
    pub normalize_velocity: bool,
    #[serde(default = "default_true")]
    pub merge_short_rests: bool,
    #[serde(default = "default_min_rest_length")]
    pub min_rest_length_ms: u32,
}

impl Default for PostprocessConfig {
    fn default() -> Self {
        Self {
            remove_ghost_notes: true,
            ghost_note_threshold: 0.1,
            normalize_velocity: true,
            merge_short_rests: true,
            min_rest_length_ms: 30,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

// Default value helpers for serde.
fn default_true() -> bool { true }
fn default_model() -> String { "htdemucs".into() }
fn default_stem_count() -> u32 { 4 }
fn default_sample_rate() -> u32 { 44100 }
fn default_output_format() -> String { "wav".into() }
fn default_onset_threshold() -> f64 { 0.5 }
fn default_frame_threshold() -> f64 { 0.3 }
fn default_min_note_length_ms() -> u32 { 50 }
fn default_min_frequency() -> f64 { 27.5 }
fn default_max_frequency() -> f64 { 4186.0 }
fn default_quantize_grid() -> u32 { 16 }
fn default_resolution() -> u32 { 16 }
fn default_output_fmt() -> String { "amigo_tidal".into() }
fn default_ghost_threshold() -> f64 { 0.1 }
fn default_min_rest_length() -> u32 { 30 }

/// Embedded default config.
const DEFAULT_CONFIG: &str = r#"
name = "default"

[separation]
enabled = true
model = "htdemucs"
skip_if_mono = true
stem_count = 4
sample_rate = 44100
output_format = "wav"

[separation.stem_mapping]
vocals = "melody"
bass = "bass"
drums = "percussion"
other = "harmony"

[transcription]
onset_threshold = 0.5
frame_threshold = 0.3
min_note_length_ms = 50
min_frequency_hz = 27.5
max_frequency_hz = 4186.0
pitch_bend = false
quantize_grid = 16

[conversion]
resolution = 16
include_legato = true
include_amplitude = true
consolidate = true
output_format = "amigo_tidal"

[postprocessing]
remove_ghost_notes = true
ghost_note_threshold = 0.1
normalize_velocity = true
merge_short_rests = true
min_rest_length_ms = 30
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let config = PipelineConfig::default();
        assert_eq!(config.name, "default");
        assert!(config.separation.enabled);
        assert_eq!(config.separation.model, "htdemucs");
        assert_eq!(config.transcription.quantize_grid, 16);
        assert_eq!(config.conversion.resolution, 16);
    }

    #[test]
    fn partial_config_uses_defaults() {
        let partial = r#"
name = "custom"

[separation]
model = "htdemucs_ft"
"#;
        let config: PipelineConfig = toml::from_str(partial).unwrap();
        assert_eq!(config.name, "custom");
        assert_eq!(config.separation.model, "htdemucs_ft");
        // Other fields use defaults.
        assert!(config.separation.skip_if_mono);
    }
}
