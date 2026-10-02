//! Shared ComfyUI HTTP client and lifecycle management.
//!
//! Extracted from `amigo_artgen` so that both artgen and audiogen can share
//! the same ComfyUI client, config, and output types.
//!
//! Communicates with a local ComfyUI instance to queue generation prompts,
//! poll for completion, and retrieve output images or audio.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Prompt / queue types
// ---------------------------------------------------------------------------

/// A ComfyUI workflow prompt ready to be queued.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComfyPrompt {
    /// The workflow graph as a JSON object (node_id -> node_config).
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

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Output image info from a completed prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputImage {
    pub filename: String,
    pub subfolder: String,
    pub image_type: String,
}

/// Output audio info from a completed prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputAudio {
    pub filename: String,
    pub subfolder: String,
    /// Audio format: "wav", "ogg", "mp3", etc.
    pub format: String,
}

/// ComfyUI output -- either an image or audio file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ComfyOutput {
    Image(OutputImage),
    Audio(OutputAudio),
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// ComfyUI client for queueing prompts and retrieving results.
///
/// All methods return `Result` -- actual HTTP calls require the engine
/// to be running with a live ComfyUI instance. The client is designed
/// to be used from the `amigo_mcp` server or CLI tools.
pub struct ComfyUiClient {
    pub config: ComfyUiConfig,
    /// Shared connection pool; every request reuses its keep-alive sockets.
    agent: ureq::Agent,
}

impl ComfyUiClient {
    /// Timeout for control-plane calls (queue, status, listings). Without
    /// one, a hung ComfyUI socket blocks forever — and `wait_for_completion`'s
    /// own timeout can never fire while a single request is stalled.
    const API_TIMEOUT: Duration = Duration::from_secs(15);
    /// Timeout for bulk downloads (generated images/audio can be large).
    const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
    /// Upper bound for one downloaded output. ureq's default body limit is
    /// 10 MB, which a few minutes of generated WAV audio already exceeds.
    const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;

    pub fn new(config: ComfyUiConfig) -> Self {
        Self {
            config,
            agent: agent(Self::API_TIMEOUT),
        }
    }

    /// `GET` a JSON document from an API endpoint.
    fn get_json(&self, path: &str) -> Result<Value, ComfyError> {
        self.agent
            .get(&self.url(path))
            .call()
            .map_err(ComfyError::from_ureq)?
            .body_mut()
            .read_json()
            .map_err(ComfyError::from_ureq)
    }

    /// Download a file from `GET /view` and write it to `output_path`.
    fn download(&self, query: &[(&str, &str)], output_path: &str) -> Result<(), ComfyError> {
        let mut request = self
            .agent
            .get(&self.url("/view"))
            .config()
            .timeout_global(Some(Self::DOWNLOAD_TIMEOUT))
            .build();
        for &(key, value) in query {
            request = request.query(key, value);
        }
        let bytes = request
            .call()
            .map_err(ComfyError::from_ureq)?
            .into_body()
            .into_with_config()
            .limit(Self::MAX_DOWNLOAD_BYTES)
            .read_to_vec()
            .map_err(ComfyError::from_ureq)?;

        std::fs::write(output_path, &bytes)?;
        Ok(())
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
        let resp: Value = self
            .agent
            .post(&self.url("/prompt"))
            .send_json(body)
            .map_err(ComfyError::from_ureq)?
            .body_mut()
            .read_json()
            .map_err(ComfyError::from_ureq)?;

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

        let resp = self.get_json(&format!("/history/{prompt_id}"))?;

        let entry = &resp[prompt_id];
        if entry.is_null() {
            return Ok(PromptStatus::Queued);
        }

        if let Some(outputs) = entry["outputs"].as_object()
            && !outputs.is_empty()
        {
            return Ok(PromptStatus::Completed);
        }

        if let Some(err) = entry["status"]["status_str"].as_str()
            && err == "error"
        {
            // `messages` is an array of [event_name, payload] pairs;
            // pull out anything human-readable rather than always
            // reporting "unknown error".
            let msg = entry["status"]["messages"]
                .as_array()
                .map(|msgs| {
                    msgs.iter()
                        .filter_map(|m| {
                            let name = m.get(0)?.as_str()?;
                            let detail = m
                                .get(1)
                                .and_then(|p| p.get("exception_message").and_then(|v| v.as_str()))
                                .unwrap_or("");
                            Some(if detail.is_empty() {
                                name.to_string()
                            } else {
                                format!("{name}: {detail}")
                            })
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .filter(|joined| !joined.is_empty())
                .unwrap_or_else(|| "unknown error".to_string());
            return Ok(PromptStatus::Failed { error: msg });
        }

        Ok(PromptStatus::Running)
    }

    /// Get output images for a completed prompt from `/history/{prompt_id}`.
    pub fn get_images(&self, prompt_id: &str) -> Result<Vec<OutputImage>, ComfyError> {
        if prompt_id.is_empty() {
            return Err(ComfyError::InvalidPromptId);
        }

        let resp = self.get_json(&format!("/history/{prompt_id}"))?;

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

    /// Get output audio files for a completed prompt from `/history/{prompt_id}`.
    pub fn get_audio(&self, prompt_id: &str) -> Result<Vec<OutputAudio>, ComfyError> {
        if prompt_id.is_empty() {
            return Err(ComfyError::InvalidPromptId);
        }

        let resp = self.get_json(&format!("/history/{prompt_id}"))?;

        let mut audio_files = Vec::new();
        if let Some(outputs) = resp[prompt_id]["outputs"].as_object() {
            for (_node_id, node_output) in outputs {
                // ComfyUI audio nodes output under "audio" key
                if let Some(audios) = node_output["audio"].as_array() {
                    for a in audios {
                        audio_files.push(OutputAudio {
                            filename: a["filename"].as_str().unwrap_or_default().to_string(),
                            subfolder: a["subfolder"].as_str().unwrap_or_default().to_string(),
                            format: a["format"]
                                .as_str()
                                .or_else(|| {
                                    // Infer format from filename extension
                                    a["filename"].as_str().and_then(|f| f.rsplit('.').next())
                                })
                                .unwrap_or("wav")
                                .to_string(),
                        });
                    }
                }
            }
        }

        Ok(audio_files)
    }

    /// Get all outputs (images + audio) for a completed prompt.
    pub fn get_outputs(&self, prompt_id: &str) -> Result<Vec<ComfyOutput>, ComfyError> {
        let mut outputs = Vec::new();

        for img in self.get_images(prompt_id)? {
            outputs.push(ComfyOutput::Image(img));
        }
        for audio in self.get_audio(prompt_id)? {
            outputs.push(ComfyOutput::Audio(audio));
        }

        Ok(outputs)
    }

    /// Download an output image to a local path via `GET /view`.
    pub fn download_image(&self, image: &OutputImage, output_path: &str) -> Result<(), ComfyError> {
        self.download(
            &[
                ("filename", &image.filename),
                ("subfolder", &image.subfolder),
                ("type", &image.image_type),
            ],
            output_path,
        )
    }

    /// Download an output audio file to a local path via `GET /view`.
    pub fn download_audio(&self, audio: &OutputAudio, output_path: &str) -> Result<(), ComfyError> {
        self.download(
            &[
                ("filename", &audio.filename),
                ("subfolder", &audio.subfolder),
                ("type", "output"),
            ],
            output_path,
        )
    }

    /// Get the list of available models/checkpoints via `GET /object_info`.
    pub fn list_models(&self) -> Result<Vec<String>, ComfyError> {
        let resp = self.get_json("/object_info/CheckpointLoaderSimple")?;

        let mut models = Vec::new();
        if let Some(names) =
            resp["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"].as_array()
            && let Some(first) = names.first()
            && let Some(arr) = first.as_array()
        {
            for item in arr {
                if let Some(name) = item.as_str() {
                    models.push(name.to_string());
                }
            }
        }

        Ok(models)
    }

    /// Get the system status (queue length, GPU info) via `GET /system_stats`.
    pub fn system_stats(&self) -> Result<Value, ComfyError> {
        let resp = self.get_json("/system_stats")?;
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

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

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
    #[error("ComfyUI not found -- run `amigo setup artgen` first")]
    NotInstalled,
    #[error("ComfyUI failed to start: {0}")]
    StartFailed(String),
}

impl ComfyError {
    /// Keep I/O and JSON failures in their own variants, as ureq 2's
    /// `into_json` did; everything else is a transport or status error.
    fn from_ureq(err: ureq::Error) -> Self {
        match err {
            ureq::Error::Io(e) => Self::Io(e),
            ureq::Error::Json(e) => Self::Json(e),
            other => Self::Http(other.to_string()),
        }
    }
}

/// An agent for talking to a local ComfyUI instance.
///
/// ureq 3 picks up `HTTP(S)_PROXY` from the environment by default; ComfyUI
/// runs on localhost, so routing it through a corporate proxy would only
/// break it. This keeps ureq 2's behavior of connecting directly.
fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .proxy(None)
        .build()
        .into()
}

// ---------------------------------------------------------------------------
// Lifecycle management
// ---------------------------------------------------------------------------

/// Manages a ComfyUI process as a child of the server.
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
        agent(Duration::from_secs(2)).get(&url).call().is_ok()
    }

    /// Ensure ComfyUI is running. If the port is already reachable
    /// (e.g. user started it manually), this is a no-op. Otherwise,
    /// starts ComfyUI as a child process.
    pub fn ensure_running(&mut self) -> Result<(), ComfyError> {
        if self.is_running() {
            tracing::info!("ComfyUI already running at {}", self.config.base_url());
            return Ok(());
        }

        // Already started by us but not yet responding -- wait a bit
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
                .args(&parts[1..])
                .arg("--version")
                .output()
                && output.status.success()
            {
                return Ok(cmd.to_string());
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

    // -- HTTP round trips against a one-shot local server --

    /// Serve exactly one HTTP response on a random local port. The join
    /// handle yields the request line the client sent.
    fn serve_once(
        status: &'static str,
        body: &'static [u8],
    ) -> (ComfyUiConfig, std::thread::JoinHandle<String>) {
        use std::io::{BufRead as _, BufReader, Write as _};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap() == 0 || header == "\r\n" {
                    break;
                }
            }
            let mut stream = reader.into_inner();
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
            request_line
        });
        let config = ComfyUiConfig {
            host: "127.0.0.1".into(),
            port,
        };
        (config, server)
    }

    #[test]
    fn check_status_reads_history() {
        let (config, server) = serve_once("200 OK", br#"{"abc":{"outputs":{"9":{"images":[]}}}}"#);
        let client = ComfyUiClient::new(config);
        assert_eq!(client.check_status("abc").unwrap(), PromptStatus::Completed);
        assert!(server.join().unwrap().starts_with("GET /history/abc "));
    }

    #[test]
    fn error_status_is_an_http_error() {
        let (config, server) = serve_once("500 Internal Server Error", b"boom");
        let client = ComfyUiClient::new(config);
        let err = client.check_status("abc").unwrap_err();
        assert!(matches!(err, ComfyError::Http(_)), "{err:?}");
        server.join().unwrap();
    }

    #[test]
    fn malformed_json_is_a_json_error() {
        let (config, server) = serve_once("200 OK", b"not json");
        let client = ComfyUiClient::new(config);
        let err = client.system_stats().unwrap_err();
        assert!(matches!(err, ComfyError::Json(_)), "{err:?}");
        server.join().unwrap();
    }

    #[test]
    fn download_image_encodes_query_and_writes_file() {
        let (config, server) = serve_once("200 OK", b"PNGDATA");
        let client = ComfyUiClient::new(config);
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.png");
        let image = OutputImage {
            filename: "a b&c.png".into(),
            subfolder: "x/y".into(),
            image_type: "output".into(),
        };
        client
            .download_image(&image, out.to_str().unwrap())
            .unwrap();

        assert_eq!(std::fs::read(&out).unwrap(), b"PNGDATA");
        let request_line = server.join().unwrap();
        assert!(
            request_line
                .starts_with("GET /view?filename=a%20b%26c.png&subfolder=x%2Fy&type=output "),
            "{request_line}"
        );
    }

    // -- Lifecycle --

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

    #[test]
    fn output_audio_debug() {
        let audio = OutputAudio {
            filename: "test.wav".into(),
            subfolder: "".into(),
            format: "wav".into(),
        };
        assert_eq!(audio.format, "wav");
    }

    #[test]
    fn comfy_output_variants() {
        let img = ComfyOutput::Image(OutputImage {
            filename: "img.png".into(),
            subfolder: "".into(),
            image_type: "output".into(),
        });
        let aud = ComfyOutput::Audio(OutputAudio {
            filename: "clip.wav".into(),
            subfolder: "".into(),
            format: "wav".into(),
        });
        assert!(matches!(img, ComfyOutput::Image(_)));
        assert!(matches!(aud, ComfyOutput::Audio(_)));
    }
}
