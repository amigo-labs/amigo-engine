//! ACE-Step music generation client.
//!
//! Communicates with a local ACE-Step Gradio server to generate music tracks
//! with lyrics/melody conditioning. Output can be split into stems for the
//! adaptive music engine.

use crate::{MusicRequest, MusicResult, MusicSection, WorldAudioStyle};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read as _;

/// ACE-Step server configuration.
#[derive(Clone, Debug)]
pub struct AceStepConfig {
    pub host: String,
    pub port: u16,
}

impl Default for AceStepConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 7860,
        }
    }
}

impl AceStepConfig {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Parameters sent to ACE-Step's Gradio API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AceStepParams {
    pub prompt: String,
    pub lyrics: String,
    pub duration: f32,
    pub steps: u32,
    pub cfg_scale: f32,
    pub seed: i64,
}

/// ACE-Step client.
pub struct AceStepClient {
    pub config: AceStepConfig,
}

impl AceStepClient {
    pub fn new(config: AceStepConfig) -> Self {
        Self { config }
    }

    /// Build generation parameters from a MusicRequest.
    pub fn build_params(&self, request: &MusicRequest) -> AceStepParams {
        let style = WorldAudioStyle::find(&request.world);
        let genre = if request.genre.is_empty() {
            style
                .as_ref()
                .map(|s| s.genre.as_str())
                .unwrap_or("instrumental")
        } else {
            &request.genre
        };

        let genre_tags = style
            .as_ref()
            .map(|s| s.genre_tags.join(", "))
            .unwrap_or_default();

        let section_mood = match &request.section {
            MusicSection::Calm => "calm, peaceful, relaxed",
            MusicSection::Tense => "tense, suspenseful, building",
            MusicSection::Battle => "intense, aggressive, driving",
            MusicSection::Boss => "epic, powerful, climactic",
            MusicSection::Victory => "triumphant, celebratory, joyful",
            MusicSection::Menu => "ambient, gentle, atmospheric",
            MusicSection::Custom(s) => s,
        };

        let prompt = format!(
            "{} music, {} BPM, {}, {}",
            genre, request.bpm, section_mood, genre_tags
        );

        let lyrics = request.lyrics.clone().unwrap_or_default();

        let steps = request
            .extra
            .get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as u32;

        let cfg_scale = request
            .extra
            .get("cfg_scale")
            .and_then(|v| v.as_f64())
            .unwrap_or(5.0) as f32;

        let seed = request
            .extra
            .get("seed")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);

        AceStepParams {
            prompt,
            lyrics,
            duration: request.duration_secs,
            steps,
            cfg_scale,
            seed,
        }
    }

    /// Generate music via ACE-Step's Gradio `/api/predict` endpoint.
    pub fn generate(&self, request: &MusicRequest) -> Result<MusicResult, AceStepError> {
        let params = self.build_params(request);
        let start = std::time::Instant::now();

        let body = serde_json::json!({
            "data": [
                params.prompt,
                params.lyrics,
                params.duration,
                params.steps,
                params.cfg_scale,
                params.seed,
            ]
        });

        let resp: serde_json::Value =
            ureq::post(&format!("{}/api/predict", self.config.base_url()))
                .send_json(body)
                .map_err(|e| AceStepError::Http(e.to_string()))?
                .into_json()
                .map_err(|e| AceStepError::Io(e))?;

        let data = resp["data"]
            .as_array()
            .ok_or_else(|| AceStepError::GenerationFailed("Missing data in response".into()))?;

        // First element is typically the output audio file path
        let audio_path = data
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if audio_path.is_empty() {
            return Err(AceStepError::GenerationFailed(
                "No audio path in response".into(),
            ));
        }

        let generation_time_ms = start.elapsed().as_millis() as u64;

        Ok(MusicResult {
            full_track_path: audio_path,
            stem_paths: HashMap::new(),
            detected_bpm: request.bpm as f32,
            generation_time_ms,
        })
    }

    /// Download a generated audio file from the Gradio server to a local path.
    pub fn download(&self, remote_path: &str, local_path: &str) -> Result<(), AceStepError> {
        let url = format!("{}/file={}", self.config.base_url(), remote_path);
        let mut bytes = Vec::new();
        ureq::get(&url)
            .call()
            .map_err(|e| AceStepError::Http(e.to_string()))?
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(AceStepError::Io)?;
        std::fs::write(local_path, &bytes)?;
        Ok(())
    }

    /// Check if the ACE-Step server is running via `/api/status`.
    pub fn health_check(&self) -> Result<bool, AceStepError> {
        match ureq::get(&format!("{}/api/status", self.config.base_url())).call() {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// ACE-Step errors.
#[derive(Debug, thiserror::Error)]
pub enum AceStepError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Generation failed: {0}")]
    GenerationFailed(String),
    #[error("Server not available")]
    ServerUnavailable,
    #[error("Timeout")]
    Timeout,
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
    fn build_params_default() {
        let client = AceStepClient::new(AceStepConfig::default());
        let req = MusicRequest::default();
        let params = client.build_params(&req);

        assert!(params.prompt.contains("120 BPM"));
        assert!(params.prompt.contains("calm"));
        assert_eq!(params.duration, 30.0);
    }

    #[test]
    fn build_params_with_world_style() {
        let client = AceStepClient::new(AceStepConfig::default());
        let req = MusicRequest {
            world: "caribbean".into(),
            section: MusicSection::Battle,
            bpm: 140,
            ..Default::default()
        };
        let params = client.build_params(&req);

        assert!(params.prompt.contains("shanty"));
        assert!(params.prompt.contains("140 BPM"));
        assert!(params.prompt.contains("intense"));
    }

    #[test]
    fn build_params_boss_section() {
        let client = AceStepClient::new(AceStepConfig::default());
        let req = MusicRequest {
            section: MusicSection::Boss,
            ..Default::default()
        };
        let params = client.build_params(&req);
        assert!(params.prompt.contains("epic"));
    }

    #[test]
    fn config_url() {
        let cfg = AceStepConfig::default();
        assert_eq!(cfg.base_url(), "http://127.0.0.1:7860");
    }
}
