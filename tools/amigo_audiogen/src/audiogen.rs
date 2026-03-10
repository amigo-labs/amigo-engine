//! AudioGen SFX generation client.
//!
//! Generates short sound effects from text descriptions using Facebook's
//! AudioGen model running locally via a Gradio API.

use crate::{SfxRequest, SfxResult, SfxCategory};
use serde::{Deserialize, Serialize};

/// AudioGen server configuration.
#[derive(Clone, Debug)]
pub struct AudioGenConfig {
    pub host: String,
    pub port: u16,
}

impl Default for AudioGenConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 7861,
        }
    }
}

impl AudioGenConfig {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Parameters sent to AudioGen's API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioGenParams {
    pub prompt: String,
    pub duration: f32,
    pub num_samples: u32,
    pub temperature: f32,
    pub top_k: u32,
}

/// AudioGen client.
pub struct AudioGenClient {
    pub config: AudioGenConfig,
}

impl AudioGenClient {
    pub fn new(config: AudioGenConfig) -> Self {
        Self { config }
    }

    /// Build generation parameters from an SfxRequest.
    pub fn build_params(&self, request: &SfxRequest) -> AudioGenParams {
        let category_prefix = match &request.category {
            SfxCategory::Gameplay => "game sound effect, ",
            SfxCategory::UI => "user interface click sound, subtle, ",
            SfxCategory::Ambient => "ambient environmental sound, looping, ",
            SfxCategory::Impact => "impact sound, punchy, ",
            SfxCategory::Explosion => "explosion sound, powerful, ",
            SfxCategory::Magic => "magical sound effect, sparkle, ",
            SfxCategory::Voice => "vocal sound, ",
            SfxCategory::Custom(_) => "",
        };

        AudioGenParams {
            prompt: format!("{}{}", category_prefix, request.prompt),
            duration: request.duration_secs.min(10.0), // AudioGen max ~10s
            num_samples: request.variants,
            temperature: 1.0,
            top_k: 250,
        }
    }

    /// Generate SFX. Returns output file paths.
    ///
    /// Placeholder: actual implementation calls the Gradio predict API.
    pub fn generate(&self, request: &SfxRequest) -> Result<SfxResult, AudioGenError> {
        let _params = self.build_params(request);

        // Placeholder: POST to /api/predict
        Ok(SfxResult {
            output_paths: Vec::new(),
            durations: Vec::new(),
            generation_time_ms: 0,
        })
    }

    /// Check if the AudioGen server is running.
    pub fn health_check(&self) -> Result<bool, AudioGenError> {
        // Placeholder: GET /api/status
        Ok(false)
    }
}

/// AudioGen errors.
#[derive(Debug, thiserror::Error)]
pub enum AudioGenError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Generation failed: {0}")]
    GenerationFailed(String),
    #[error("Server not available")]
    ServerUnavailable,
    #[error("Duration too long (max 10s)")]
    DurationTooLong,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_params_gameplay() {
        let client = AudioGenClient::new(AudioGenConfig::default());
        let req = SfxRequest {
            prompt: "cannon firing".into(),
            category: SfxCategory::Gameplay,
            ..Default::default()
        };
        let params = client.build_params(&req);
        assert!(params.prompt.contains("game sound effect"));
        assert!(params.prompt.contains("cannon firing"));
    }

    #[test]
    fn build_params_clamps_duration() {
        let client = AudioGenClient::new(AudioGenConfig::default());
        let req = SfxRequest {
            duration_secs: 30.0, // too long
            ..Default::default()
        };
        let params = client.build_params(&req);
        assert_eq!(params.duration, 10.0);
    }

    #[test]
    fn config_url() {
        let cfg = AudioGenConfig::default();
        assert_eq!(cfg.base_url(), "http://127.0.0.1:7861");
    }

    #[test]
    fn ui_category_prefix() {
        let client = AudioGenClient::new(AudioGenConfig::default());
        let req = SfxRequest {
            prompt: "button click".into(),
            category: SfxCategory::UI,
            ..Default::default()
        };
        let params = client.build_params(&req);
        assert!(params.prompt.contains("user interface"));
    }
}
