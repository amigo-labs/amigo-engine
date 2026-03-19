//! ComfyUI HTTP client.
//!
//! Communicates with a local ComfyUI instance to queue image generation
//! prompts, poll for completion, and retrieve output images.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read as _;

/// ComfyUI server connection configuration.
#[derive(Clone, Debug)]
pub struct ComfyUiConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ComfyUiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8188,
        }
    }
}

impl ComfyUiConfig {
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// A ComfyUI workflow prompt ready to be queued.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComfyPrompt {
    /// The workflow graph as a JSON object (node_id → node_config).
    pub prompt: HashMap<String, Value>,
    /// Optional client ID for tracking.
    pub client_id: Option<String>,
}

/// Response from queuing a prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueResponse {
    pub prompt_id: String,
    pub number: u64,
}

/// Status of a queued prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptStatus {
    Queued,
    Running,
    Completed,
    Failed { error: String },
    Unknown,
}

/// Output image info from a completed prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputImage {
    pub filename: String,
    pub subfolder: String,
    pub image_type: String,
}

/// ComfyUI client for queueing prompts and retrieving results.
///
/// All methods return `Result` — actual HTTP calls require the engine
/// to be running with a live ComfyUI instance. The client is designed
/// to be used from the `amigo_mcp` server or CLI tools.
pub struct ComfyUiClient {
    pub config: ComfyUiConfig,
}

impl ComfyUiClient {
    pub fn new(config: ComfyUiConfig) -> Self {
        Self { config }
    }

    /// Build the URL for an API endpoint.
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url(), path)
    }

    /// Queue a workflow prompt. Returns the prompt ID.
    ///
    /// Sends `POST /prompt` with the workflow JSON to ComfyUI.
    pub fn queue_prompt(&self, prompt: &ComfyPrompt) -> Result<QueueResponse, ComfyError> {
        if prompt.prompt.is_empty() {
            return Err(ComfyError::InvalidWorkflow("Empty workflow".into()));
        }

        let body = serde_json::to_value(prompt).map_err(ComfyError::Json)?;
        let resp: Value = ureq::post(&self.url("/prompt"))
            .send_json(body)
            .map_err(|e| ComfyError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| ComfyError::Io(e))?;

        let prompt_id = resp["prompt_id"]
            .as_str()
            .ok_or_else(|| ComfyError::Http("Missing prompt_id in response".into()))?
            .to_string();
        let number = resp["number"].as_u64().unwrap_or(0);

        Ok(QueueResponse { prompt_id, number })
    }

    /// Check the status of a queued prompt via `GET /history/{prompt_id}`.
    pub fn check_status(&self, prompt_id: &str) -> Result<PromptStatus, ComfyError> {
        if prompt_id.is_empty() {
            return Err(ComfyError::InvalidPromptId);
        }

        let resp: Value = ureq::get(&self.url(&format!("/history/{}", prompt_id)))
            .call()
            .map_err(|e| ComfyError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| ComfyError::Io(e))?;

        let entry = &resp[prompt_id];
        if entry.is_null() {
            return Ok(PromptStatus::Queued);
        }

        if let Some(outputs) = entry["outputs"].as_object() {
            if !outputs.is_empty() {
                return Ok(PromptStatus::Completed);
            }
        }

        if let Some(err) = entry["status"]["status_str"].as_str() {
            if err == "error" {
                let msg = entry["status"]["messages"]
                    .as_str()
                    .unwrap_or("unknown error");
                return Ok(PromptStatus::Failed {
                    error: msg.to_string(),
                });
            }
        }

        Ok(PromptStatus::Running)
    }

    /// Get output images for a completed prompt from `/history/{prompt_id}`.
    pub fn get_outputs(&self, prompt_id: &str) -> Result<Vec<OutputImage>, ComfyError> {
        if prompt_id.is_empty() {
            return Err(ComfyError::InvalidPromptId);
        }

        let resp: Value = ureq::get(&self.url(&format!("/history/{}", prompt_id)))
            .call()
            .map_err(|e| ComfyError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| ComfyError::Io(e))?;

        let mut images = Vec::new();
        if let Some(outputs) = resp[prompt_id]["outputs"].as_object() {
            for (_node_id, node_output) in outputs {
                if let Some(imgs) = node_output["images"].as_array() {
                    for img in imgs {
                        images.push(OutputImage {
                            filename: img["filename"].as_str().unwrap_or_default().to_string(),
                            subfolder: img["subfolder"].as_str().unwrap_or_default().to_string(),
                            image_type: img["type"].as_str().unwrap_or("output").to_string(),
                        });
                    }
                }
            }
        }

        Ok(images)
    }

    /// Download an output image to a local path via `GET /view`.
    pub fn download_image(&self, image: &OutputImage, output_path: &str) -> Result<(), ComfyError> {
        let url = format!(
            "{}/view?filename={}&subfolder={}&type={}",
            self.config.base_url(),
            image.filename,
            image.subfolder,
            image.image_type,
        );

        let mut bytes = Vec::new();
        ureq::get(&url)
            .call()
            .map_err(|e| ComfyError::Http(e.to_string()))?
            .into_reader()
            .read_to_end(&mut bytes)
            .map_err(ComfyError::Io)?;

        std::fs::write(output_path, &bytes)?;
        Ok(())
    }

    /// Get the list of available models/checkpoints via `GET /object_info`.
    pub fn list_models(&self) -> Result<Vec<String>, ComfyError> {
        let resp: Value = ureq::get(&self.url("/object_info/CheckpointLoaderSimple"))
            .call()
            .map_err(|e| ComfyError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| ComfyError::Io(e))?;

        let mut models = Vec::new();
        if let Some(names) =
            resp["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"].as_array()
        {
            if let Some(first) = names.first() {
                if let Some(arr) = first.as_array() {
                    for item in arr {
                        if let Some(name) = item.as_str() {
                            models.push(name.to_string());
                        }
                    }
                }
            }
        }

        Ok(models)
    }

    /// Get the system status (queue length, GPU info) via `GET /system_stats`.
    pub fn system_stats(&self) -> Result<Value, ComfyError> {
        let resp: Value = ureq::get(&self.url("/system_stats"))
            .call()
            .map_err(|e| ComfyError::Http(e.to_string()))?
            .into_json()
            .map_err(|e| ComfyError::Io(e))?;
        Ok(resp)
    }

    /// Poll for prompt completion with a timeout.
    /// Returns the final status once completed, failed, or timed out.
    pub fn wait_for_completion(
        &self,
        prompt_id: &str,
        timeout_ms: u64,
        poll_interval_ms: u64,
    ) -> Result<PromptStatus, ComfyError> {
        let start = std::time::Instant::now();
        loop {
            let status = self.check_status(prompt_id)?;
            match &status {
                PromptStatus::Completed | PromptStatus::Failed { .. } => return Ok(status),
                _ => {}
            }
            if start.elapsed().as_millis() as u64 >= timeout_ms {
                return Err(ComfyError::Timeout);
            }
            std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
        }
    }
}

/// Errors from ComfyUI operations.
#[derive(Debug, thiserror::Error)]
pub enum ComfyError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("Invalid workflow: {0}")]
    InvalidWorkflow(String),
    #[error("Invalid prompt ID")]
    InvalidPromptId,
    #[error("Prompt failed: {0}")]
    PromptFailed(String),
    #[error("Timeout waiting for result")]
    Timeout,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_url() {
        let cfg = ComfyUiConfig::default();
        assert_eq!(cfg.base_url(), "http://127.0.0.1:8188");
    }

    #[test]
    fn client_url_building() {
        let client = ComfyUiClient::new(ComfyUiConfig::default());
        assert_eq!(client.url("/prompt"), "http://127.0.0.1:8188/prompt");
    }

    #[test]
    fn queue_empty_prompt_fails() {
        let client = ComfyUiClient::new(ComfyUiConfig::default());
        let prompt = ComfyPrompt {
            prompt: HashMap::new(),
            client_id: None,
        };
        assert!(client.queue_prompt(&prompt).is_err());
    }

    #[test]
    fn check_status_empty_id() {
        let client = ComfyUiClient::new(ComfyUiConfig::default());
        assert!(client.check_status("").is_err());
    }
}
