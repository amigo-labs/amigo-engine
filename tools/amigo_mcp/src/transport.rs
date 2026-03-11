//! Transport layer: stdio ←→ TCP bridge.
//!
//! Reads MCP messages from stdin, sends JSON-RPC to the engine API server,
//! and writes MCP responses to stdout.

use crate::{McpRequest, McpResponse, McpServerConfig};
use crate::protocol::handle_mcp_request;
use std::io::{self, BufRead, Write};

/// Run the MCP server on stdio, forwarding tool calls to the engine API.
///
/// This is the main entry point for the `amigo mcp-server` binary.
pub fn run_stdio_server(config: &McpServerConfig) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let api_addr = format!("{}:{}", config.api_host, config.api_port);

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<McpRequest>(&line) {
            Ok(req) => {
                let addr = api_addr.clone();
                handle_mcp_request(&req, &|method, params| {
                    forward_to_api(&addr, method, params)
                })
            }
            Err(e) => McpResponse::error(None, -32700, format!("Parse error: {}", e)),
        };

        let mut json = serde_json::to_string(&response).unwrap_or_default();
        json.push('\n');
        out.write_all(json.as_bytes())?;
        out.flush()?;
    }

    Ok(())
}

/// Forward a JSON-RPC call to the engine API server via TCP.
fn forward_to_api(
    addr: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let mut stream = TcpStream::connect(addr).map_err(|e| {
        format!(
            "Cannot connect to engine API at {}: {}. Is the engine running with --api?",
            addr, e
        )
    })?;

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|e| format!("Set timeout: {}", e))?;

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });

    let mut req_json = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    req_json.push('\n');

    stream
        .write_all(req_json.as_bytes())
        .map_err(|e| format!("Write: {}", e))?;

    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| format!("Read: {}", e))?;

    let resp: serde_json::Value =
        serde_json::from_str(&response_line).map_err(|e| format!("Parse response: {}", e))?;

    if let Some(error) = resp.get("error") {
        let msg = error
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        return Err(msg.to_string());
    }

    Ok(resp
        .get("result")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_to_api_fails_gracefully() {
        // No server running → should return error, not panic
        let result = forward_to_api("127.0.0.1:1", "engine.status", serde_json::json!({}));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Cannot connect"));
    }
}
