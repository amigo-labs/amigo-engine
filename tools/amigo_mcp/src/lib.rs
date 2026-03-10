//! amigo_mcp — MCP server for Claude Code integration with Amigo Engine.
//!
//! This is a thin bridge that speaks MCP protocol on stdio and translates
//! tool calls to JSON-RPC requests sent to the running amigo_api server.
//! The engine doesn't need to know about MCP, and MCP updates don't require
//! engine changes.
//!
//! Architecture:
//! ```text
//! Claude Code ←→ MCP (stdio) ←→ amigo_mcp ←→ JSON-RPC (TCP) ←→ amigo_api
//! ```

pub mod protocol;
pub mod tools;
pub mod transport;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// MCP Protocol types (subset needed for tool serving)
// ---------------------------------------------------------------------------

/// MCP JSON-RPC request (MCP uses JSON-RPC 2.0 internally).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// MCP JSON-RPC response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

impl McpResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, msg: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: msg.into(),
            }),
        }
    }
}

/// MCP server configuration.
#[derive(Clone, Debug)]
pub struct McpServerConfig {
    /// Amigo Engine API host.
    pub api_host: String,
    /// Amigo Engine API port.
    pub api_port: u16,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            api_host: "127.0.0.1".into(),
            api_port: 9999,
        }
    }
}
