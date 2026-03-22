//! amigo_artgen MCP server.
//!
//! Speaks MCP protocol on stdio, dispatching tool calls to the artgen pipeline.
//! ComfyUI is managed automatically — started on first generation request
//! and shut down when the server exits.

use amigo_artgen::comfyui::{ComfyUiConfig, ComfyUiLifecycle};
use amigo_artgen::config::load_art_defaults;
use amigo_artgen::tools;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Parse --server flag for custom ComfyUI endpoint
    let server_url = args
        .iter()
        .position(|a| a == "--server")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("http://localhost:8188");

    // Parse ComfyUI config from URL
    let comfy_config = parse_comfy_url(server_url);

    // Load project defaults to determine backend
    let project_dir = std::env::current_dir().unwrap_or_default();
    let defaults = load_art_defaults(&project_dir);
    let backend = defaults.resolve_backend();
    let art_mode = defaults.resolve_art_mode();

    eprintln!(
        "amigo-artgen MCP server starting (backend: {}, mode: {:?})",
        backend.display_name(),
        art_mode
    );

    // ComfyUI lifecycle — will auto-start on first ensure_running() call
    // and auto-shutdown on drop.
    let _lifecycle = ComfyUiLifecycle::new(comfy_config);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {e}"),
                    }),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let response = handle_request(&request);
        let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
        let _ = stdout.flush();
    }
}

fn handle_request(req: &JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "amigo-artgen",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            })),
            error: None,
        },
        "tools/list" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(serde_json::json!({
                "tools": tools::list_tools()
            })),
            error: None,
        },
        "tools/call" => {
            let tool_name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let tool_args = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            match tools::dispatch_tool(tool_name, tool_args) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: req.id.clone(),
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                        }]
                    })),
                    error: None,
                },
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: req.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32603,
                        message: e.to_string(),
                    }),
                },
            }
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
            }),
        },
    }
}

/// Parse a URL like "http://localhost:8188" into a ComfyUiConfig.
fn parse_comfy_url(url: &str) -> ComfyUiConfig {
    let stripped = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let (host, port) = if let Some((h, p)) = stripped.split_once(':') {
        (h.to_string(), p.parse().unwrap_or(8188))
    } else {
        (stripped.to_string(), 8188)
    };

    ComfyUiConfig { host, port }
}
