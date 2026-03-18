//! AudioGen SFX generation client.
//!
//! Generates short sound effects from text descriptions using Facebook's
//! AudioGen model running locally via a Gradio API.

use crate::{SfxCategory, SfxRequest, SfxResult};
use serde::{Deserialize, Serialize};
use std::io::Read as _;

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

    /// Generate SFX via AudioGen's Gradio `/api/predict` endpoint.
    pub fn generate(&self, request: &SfxRequest) -> Result<SfxResult, AudioGenError> {
        if request.duration_secs > 10.0 {
            return Err(AudioGenError::DurationTooLong);
        }

        let params = self.build_params(request);
        let start = std::time::Instant::now();

        let body = serde_json::json!({
            "data": [
                params.prompt,
                params.duration,
                params.num_samples,
                params.temperature,
                params.top_k,
            ]
        });

        let resp: serde_json::Value =
            ureq::post(&format!("{}/api/predict", self.config.base_url()))
                .send_json(body)
                .map_err(|e| AudioGenError::Http(e.to_string()))?
                .into_json()
                .map_err(|e| AudioGenError::Io(e))?;

        let data = resp["data"]
            .as_array()
            .ok_or_else(|| AudioGenError::GenerationFailed("Missing data in response".into()))?;

        let mut output_paths = Vec::new();
        let mut durations = Vec::new();

        for item in data {
            if let Some(path) = item.as_str() {
                output_paths.push(path.to_string());
                durations.push(params.duration);
            }
        }

        let generation_time_ms = start.elapsed().as_millis() as u64;

        Ok(SfxResult {
            output_paths,
            durations,
            generation_time_ms,
        })
    }

    /// Download a generated audio file from the Gradio server to a local path.
    pub fn download(&self, remote_path: &str, local_path: &str) -> Result<(), AudioGenError> {
        let url = format!("{}/file={}", self.config.base_url(), remote_path);
        let mut bytes = Vec::new();
        ureq::get(&url)
            .call()
            .map_err(|e| AudioGenError::Http(e.to_string()))?
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(AudioGenError::Io)?;
        std::fs::write(local_path, &bytes)?;
        Ok(())
    }

    /// Check if the AudioGen server is running via `/api/status`.
    pub fn health_check(&self) -> Result<bool, AudioGenError> {
        match ureq::get(&format!("{}/api/status", self.config.base_url())).call() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
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
