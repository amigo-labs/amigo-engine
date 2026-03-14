//! amigo_mcp — MCP server binary for Claude Code integration.
//!
//! Bridges MCP protocol on stdio to the running Amigo Engine API via TCP.
//!
//! Usage:
//!   amigo mcp-server [--host HOST] [--port PORT]

use amigo_mcp::{McpServerConfig, transport};
use std::io;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let host = args
        .iter()
        .position(|a| a == "--host")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("127.0.0.1");

    let port = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(9999u16);

    let config = McpServerConfig {
        api_host: host.to_string(),
        api_port: port,
    };

    eprintln!(
        "amigo-mcp server starting (engine API at {}:{})...",
        config.api_host, config.api_port
    );

    transport::run_stdio_server(&config)
}
