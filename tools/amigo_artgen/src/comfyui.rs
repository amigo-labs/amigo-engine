//! ComfyUI HTTP client.
//!
//! Communicates with a local ComfyUI instance to queue image generation
//! prompts, poll for completion, and retrieve output images.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
    /// In a real implementation this sends POST /prompt with the workflow JSON.
    /// For now returns a placeholder — actual HTTP will use ureq when the
    /// dependency is added.
    pub fn queue_prompt(&self, prompt: &ComfyPrompt) -> Result<QueueResponse, ComfyError> {
        // Validate the prompt has at least one node
        if prompt.prompt.is_empty() {
            return Err(ComfyError::InvalidWorkflow("Empty workflow".into()));
        }

        // Placeholder: real implementation sends HTTP POST
        Ok(QueueResponse {
            prompt_id: format!("pending_{}", prompt.prompt.len()),
            number: 0,
        })
    }

    /// Check the status of a queued prompt.
    pub fn check_status(&self, prompt_id: &str) -> Result<PromptStatus, ComfyError> {
        if prompt_id.is_empty() {
            return Err(ComfyError::InvalidPromptId);
        }
        // Placeholder: real implementation polls GET /history/{prompt_id}
        Ok(PromptStatus::Unknown)
    }

    /// Get output images for a completed prompt.
    pub fn get_outputs(&self, prompt_id: &str) -> Result<Vec<OutputImage>, ComfyError> {
        if prompt_id.is_empty() {
            return Err(ComfyError::InvalidPromptId);
        }
        // Placeholder: real implementation fetches from /history
        Ok(Vec::new())
    }

    /// Download an output image to a local path.
    pub fn download_image(&self, image: &OutputImage, output_path: &str) -> Result<(), ComfyError> {
        let _url = format!(
            "{}/view?filename={}&subfolder={}&type={}",
            self.config.base_url(),
            image.filename,
            image.subfolder,
            image.image_type,
        );
        let _ = output_path;
        // Placeholder: real implementation downloads via GET /view
        Ok(())
    }

    /// Get the list of available models/checkpoints.
    pub fn list_models(&self) -> Result<Vec<String>, ComfyError> {
        // Placeholder: GET /object_info for CheckpointLoaderSimple
        Ok(Vec::new())
    }

    /// Get the system status (queue length, GPU info).
    pub fn system_stats(&self) -> Result<Value, ComfyError> {
        // Placeholder: GET /system_stats
        Ok(serde_json::json!({"status": "unknown"}))
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
    fn queue_valid_prompt() {
        let client = ComfyUiClient::new(ComfyUiConfig::default());
        let mut nodes = HashMap::new();
        nodes.insert("1".into(), serde_json::json!({"class_type": "KSampler"}));
        let prompt = ComfyPrompt {
            prompt: nodes,
            client_id: None,
        };
        let result = client.queue_prompt(&prompt);
        assert!(result.is_ok());
    }

    #[test]
    fn check_status_empty_id() {
        let client = ComfyUiClient::new(ComfyUiConfig::default());
        assert!(client.check_status("").is_err());
    }
}
