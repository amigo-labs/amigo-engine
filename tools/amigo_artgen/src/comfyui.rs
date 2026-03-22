//! ComfyUI HTTP client and lifecycle management.
//!
//! Communicates with a local ComfyUI instance to queue image generation
//! prompts, poll for completion, and retrieve output images.
//!
//! `ComfyUiLifecycle` manages ComfyUI as a child process — auto-starting
//! it when needed and shutting it down cleanly on drop.

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
            .map_err(ComfyError::Io)?;

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
            .map_err(ComfyError::Io)?;

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
            .map_err(ComfyError::Io)?;

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
            .map_err(ComfyError::Io)?;

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
            .map_err(ComfyError::Io)?;
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
    #[error("ComfyUI not found — run `amigo setup artgen` first")]
    NotInstalled,
    #[error("ComfyUI failed to start: {0}")]
    StartFailed(String),
}

// ---------------------------------------------------------------------------
// Lifecycle management
// ---------------------------------------------------------------------------

/// Manages a ComfyUI process as a child of the artgen server.
///
/// On `ensure_running()`, checks if the port is already reachable.
/// If not, starts ComfyUI as a subprocess. On `Drop`, shuts it down.
pub struct ComfyUiLifecycle {
    process: Option<std::process::Child>,
    config: ComfyUiConfig,
}

impl ComfyUiLifecycle {
    pub fn new(config: ComfyUiConfig) -> Self {
        Self {
            process: None,
            config,
        }
    }

    /// Check if ComfyUI is reachable at the configured host:port.
    pub fn is_running(&self) -> bool {
        let url = format!("{}/system_stats", self.config.base_url());
        ureq::get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .call()
            .is_ok()
    }

    /// Ensure ComfyUI is running. If the port is already reachable
    /// (e.g. user started it manually), this is a no-op. Otherwise,
    /// starts ComfyUI as a child process.
    pub fn ensure_running(&mut self) -> Result<(), ComfyError> {
        if self.is_running() {
            tracing::info!("ComfyUI already running at {}", self.config.base_url());
            return Ok(());
        }

        // Already started by us but not yet responding — wait a bit
        if self.process.is_some() {
            return self.wait_for_startup();
        }

        tracing::info!("Starting ComfyUI on port {}...", self.config.port);

        // Try to find comfyui in PATH or common locations
        let comfyui_cmd = self.find_comfyui_binary()?;

        let child = std::process::Command::new(&comfyui_cmd)
            .args([
                "--listen",
                &self.config.host,
                "--port",
                &self.config.port.to_string(),
                "--preview-method",
                "none",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ComfyError::StartFailed(format!("{comfyui_cmd}: {e}")))?;

        self.process = Some(child);
        self.wait_for_startup()
    }

    /// Shut down the managed ComfyUI process (if we started it).
    pub fn shutdown(&mut self) {
        if let Some(mut child) = self.process.take() {
            tracing::info!("Shutting down ComfyUI...");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Returns the config (for creating a `ComfyUiClient`).
    pub fn config(&self) -> &ComfyUiConfig {
        &self.config
    }

    // -- private --

    fn find_comfyui_binary(&self) -> Result<String, ComfyError> {
        // Check common locations
        let candidates = [
            "comfyui",
            "python -m comfy",
            // ~/.amigo/venv/bin/python -m comfyui
        ];

        for cmd in &candidates {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if let Ok(output) = std::process::Command::new(parts[0])
                .args(&["--version"])
                .output()
            {
                if output.status.success() {
                    return Ok(cmd.to_string());
                }
            }
        }

        // Check amigo venv
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        let venv_python = format!("{home}/.amigo/venv/bin/python");
        if std::path::Path::new(&venv_python).exists() {
            return Ok(format!("{venv_python} -m comfyui"));
        }

        Err(ComfyError::NotInstalled)
    }

    fn wait_for_startup(&self) -> Result<(), ComfyError> {
        let max_wait = std::time::Duration::from_secs(30);
        let poll_interval = std::time::Duration::from_millis(500);
        let start = std::time::Instant::now();

        while start.elapsed() < max_wait {
            if self.is_running() {
                tracing::info!("ComfyUI is ready at {}", self.config.base_url());
                return Ok(());
            }
            std::thread::sleep(poll_interval);
        }

        Err(ComfyError::StartFailed(format!(
            "ComfyUI did not become ready within {}s",
            max_wait.as_secs()
        )))
    }
}

impl Drop for ComfyUiLifecycle {
    fn drop(&mut self) {
        self.shutdown();
    }
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

    // ── Lifecycle ──────────────────────────────────────────────

    #[test]
    fn lifecycle_new_has_no_process() {
        let lc = ComfyUiLifecycle::new(ComfyUiConfig::default());
        assert!(lc.process.is_none());
    }

    #[test]
    fn lifecycle_is_running_returns_false_without_server() {
        let lc = ComfyUiLifecycle::new(ComfyUiConfig {
            host: "127.0.0.1".into(),
            port: 59999, // unlikely to be in use
        });
        assert!(!lc.is_running());
    }

    #[test]
    fn lifecycle_shutdown_is_safe_when_not_started() {
        let mut lc = ComfyUiLifecycle::new(ComfyUiConfig::default());
        lc.shutdown(); // should not panic
        assert!(lc.process.is_none());
    }
}
