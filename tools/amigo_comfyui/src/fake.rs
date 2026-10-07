//! A stand-in ComfyUI HTTP server for tests.
//!
//! Real generation needs a GPU and model weights, so tests talk to this
//! instead: it implements the handful of endpoints the client uses
//! (`/prompt`, `/history`, `/view`, `/upload/image`, `/system_stats`,
//! `/object_info`), records what it was sent, and answers every prompt with
//! a configurable number of output images. Enabled by the `fake-server`
//! feature (and always in this crate's own tests).

use crate::ComfyUiConfig;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Bytes served for every output image. Not a decodable PNG: tests check
/// that exactly these bytes reach disk.
pub const FAKE_IMAGE: &[u8] = b"\x89PNG\r\n\x1a\nfake comfyui output";

#[derive(Default)]
struct State {
    prompts: Vec<Value>,
    uploads: Vec<String>,
    fail_with: Option<String>,
    outputs: usize,
    image: Option<Vec<u8>>,
    audio: Option<Vec<u8>>,
}

/// A running fake server; stops when dropped.
pub struct FakeComfyUi {
    port: u16,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeComfyUi {
    /// Start on a free local port. Every prompt yields one output image.
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake ComfyUI");
        let port = listener.local_addr().expect("local addr").port();
        let state = Arc::new(Mutex::new(State {
            outputs: 1,
            ..State::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));

        let thread = {
            let state = Arc::clone(&state);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = stream {
                        let _ = handle(stream, &state);
                    }
                }
            })
        };

        Self {
            port,
            state,
            stop,
            thread: Some(thread),
        }
    }

    /// Client configuration pointing at this server.
    pub fn config(&self) -> ComfyUiConfig {
        ComfyUiConfig {
            host: "127.0.0.1".into(),
            port: self.port,
        }
    }

    /// Workflow graphs received on `POST /prompt`, in order.
    pub fn prompts(&self) -> Vec<Value> {
        self.state.lock().unwrap().prompts.clone()
    }

    /// File names received on `POST /upload/image`, in order.
    pub fn uploads(&self) -> Vec<String> {
        self.state.lock().unwrap().uploads.clone()
    }

    /// Make every following prompt fail with `message`.
    pub fn fail_prompts_with(&self, message: &str) {
        self.state.lock().unwrap().fail_with = Some(message.to_string());
    }

    /// Serve `bytes` for every output image instead of [`FAKE_IMAGE`], for
    /// tests that decode what ComfyUI returned.
    pub fn set_output_image(&self, bytes: Vec<u8>) {
        self.state.lock().unwrap().image = Some(bytes);
    }

    /// Make every prompt also output one audio file (`<id>_<i>.wav`, under
    /// the `audio` key like ComfyUI's `SaveAudio`), served as `bytes`.
    pub fn set_output_audio(&self, bytes: Vec<u8>) {
        self.state.lock().unwrap().audio = Some(bytes);
    }

    /// How many images each prompt outputs (0 is a run with no output).
    pub fn set_outputs_per_prompt(&self, count: usize) {
        self.state.lock().unwrap().outputs = count;
    }
}

impl Drop for FakeComfyUi {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Unblock `accept` so the thread sees the flag.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle(stream: TcpStream, state: &Mutex<State>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;

    let path = target.split('?').next().unwrap_or_default();
    let (status, content_type, response): (u16, &str, Vec<u8>) = match (method.as_str(), path) {
        ("POST", "/prompt") => {
            let prompt: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let mut s = state.lock().unwrap();
            s.prompts.push(prompt);
            let n = s.prompts.len();
            json_response(json!({ "prompt_id": format!("p{n}"), "number": n }))
        }
        ("GET", p) if p.starts_with("/history/") => {
            let id = &p["/history/".len()..];
            let s = state.lock().unwrap();
            let entry = match &s.fail_with {
                Some(message) => json!({
                    "outputs": {},
                    "status": {
                        "status_str": "error",
                        "messages": [["execution_error", { "exception_message": message }]]
                    }
                }),
                None if s.outputs == 0 => json!({
                    "outputs": { "9": { "images": [] } },
                    "status": { "status_str": "success" }
                }),
                None => {
                    let images: Vec<Value> = (0..s.outputs)
                        .map(|i| {
                            json!({ "filename": format!("{id}_{i}.png"), "subfolder": "", "type": "output" })
                        })
                        .collect();
                    let mut outputs = json!({ "9": { "images": images } });
                    if s.audio.is_some() {
                        let audio: Vec<Value> = (0..s.outputs)
                            .map(|i| {
                                json!({ "filename": format!("{id}_{i}.wav"), "subfolder": "", "type": "output" })
                            })
                            .collect();
                        outputs["10"] = json!({ "audio": audio });
                    }
                    json!({ "outputs": outputs })
                }
            };
            json_response(json!({ id: entry }))
        }
        ("GET", "/view") => {
            let s = state.lock().unwrap();
            match &s.audio {
                Some(audio) if target.contains(".wav") => (200, "audio/wav", audio.clone()),
                _ => (
                    200,
                    "image/png",
                    s.image.clone().unwrap_or_else(|| FAKE_IMAGE.to_vec()),
                ),
            }
        }
        ("POST", "/upload/image") => {
            let text = String::from_utf8_lossy(&body);
            let name = text
                .split("filename=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or("upload.png")
                .to_string();
            state.lock().unwrap().uploads.push(name.clone());
            json_response(json!({ "name": name, "subfolder": "", "type": "input" }))
        }
        ("GET", "/system_stats") => json_response(json!({
            "system": { "os": "fake" },
            "devices": [{ "name": "Fake GPU", "vram_total": 8_589_934_592u64, "vram_free": 4_294_967_296u64 }]
        })),
        ("GET", "/object_info/LoraLoader") => json_response(json!({
            "LoraLoader": { "input": { "required": { "lora_name": [["pixel-art.safetensors"]] } } }
        })),
        ("GET", "/object_info/CheckpointLoaderSimple") => json_response(json!({
            "CheckpointLoaderSimple": { "input": { "required": { "ckpt_name": [["qwen-image.gguf"]] } } }
        })),
        _ => (404, "text/plain", b"not found".to_vec()),
    };

    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.len()
    )?;
    stream.write_all(&response)?;
    stream.flush()
}

fn json_response(value: Value) -> (u16, &'static str, Vec<u8>) {
    (200, "application/json", value.to_string().into_bytes())
}
