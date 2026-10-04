//! amigo_audiogen MCP server.
//!
//! Speaks MCP protocol on stdio, dispatching tool calls to the audio pipeline.

use amigo_audiogen::tools;
use amigo_comfyui::ComfyUiLifecycle;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[expect(
        dead_code,
        reason = "JSON-RPC envelope field; deserialized for validation, not read"
    )]
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

    // Parse --server flag for a custom ComfyUI endpoint. `--acestep` is an
    // accepted alias (older configs used it); ACE-Step runs as ComfyUI
    // custom nodes, so both point at the same server.
    let server_url = args
        .iter()
        .position(|a| a == "--server" || a == "--acestep")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("http://localhost:8188");

    // Export ComfyUI URL so tools::create_comfyui_client() can pick it up
    // SAFETY: this runs at the top of main, before the server spawns any
    // thread, so nothing can read the environment concurrently.
    unsafe { std::env::set_var("AMIGO_COMFYUI_URL", server_url) };

    eprintln!(
        "amigo-audiogen MCP server starting (ComfyUI: {})...",
        server_url
    );

    // Started on the first tool that needs ComfyUI if nothing answers at
    // the address; stopped when the server exits.
    let mut comfyui = ComfyUiLifecycle::new(tools::comfyui_config());
    // Project defaults (`[audio]` in amigo.toml) come from the directory
    // the server runs in; they were never passed to the tools before.
    let project_dir = std::env::current_dir().unwrap_or_default();

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

        let response = handle_request(&request, &mut comfyui, &project_dir);
        let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
        let _ = stdout.flush();
    }
}

fn handle_request(
    req: &JsonRpcRequest,
    comfyui: &mut ComfyUiLifecycle,
    project_dir: &std::path::Path,
) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "amigo-audiogen",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            })),
            error: None,
        },
        "initialized" | "notifications/initialized" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(serde_json::json!({})),
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

            let outcome = if tools::needs_comfyui(tool_name) {
                comfyui.ensure_running().map_err(|e| {
                    tools::ToolError::Backend(format!("ComfyUI is not available: {e}"))
                })
            } else {
                Ok(())
            }
            .and_then(|()| {
                tools::dispatch_tool_with_defaults(tool_name, tool_args, Some(project_dir))
            });

            match outcome {
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
                // A tool that ran and failed is reported to the model as an
                // `isError` result, per MCP; failures used to come back as
                // successful results with an "error" field.
                Err(e) if !e.is_protocol_error() => JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: req.id.clone(),
                    result: Some(serde_json::json!({
                        "content": [{ "type": "text", "text": e.to_string() }],
                        "isError": true
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
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id.clone(),
            result: Some(serde_json::json!({})),
            error: None,
        },
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
