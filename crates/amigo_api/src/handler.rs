use crate::{RpcRequest, RpcResponse, METHOD_NOT_FOUND};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Snapshot of engine state exposed to AI agents.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub tick: u64,
    pub fps: f32,
    pub entity_count: usize,
    pub scene: String,
    pub paused: bool,
    pub custom: HashMap<String, Value>,
}

/// Mailbox for commands from the API to the engine's main loop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiCommand {
    pub action: String,
    pub params: Value,
}

/// Shared state between the API server thread and the engine main loop.
pub struct ApiSharedState {
    pub snapshot: EngineSnapshot,
    pub pending_commands: Vec<ApiCommand>,
    pub log_buffer: Vec<String>,
}

impl ApiSharedState {
    pub fn new() -> Self {
        Self {
            snapshot: EngineSnapshot::default(),
            pending_commands: Vec::new(),
            log_buffer: Vec::new(),
        }
    }
}

impl Default for ApiSharedState {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe handle to the shared state.
pub type SharedState = Arc<Mutex<ApiSharedState>>;

/// Create a new shared state handle.
pub fn new_shared_state() -> SharedState {
    Arc::new(Mutex::new(ApiSharedState::new()))
}

/// Route an RPC request to the appropriate handler.
pub fn handle_request(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match req.method.as_str() {
        "engine.status" => handle_status(req, state),
        "engine.pause" => handle_pause(req, state),
        "engine.unpause" => handle_unpause(req, state),
        "engine.step" => handle_step(req, state),
        "engine.command" => handle_command(req, state),
        "engine.get_log" => handle_get_log(req, state),
        "engine.set_property" => handle_set_property(req, state),
        "engine.get_property" => handle_get_property(req, state),
        _ => RpcResponse::error(
            req.id,
            METHOD_NOT_FOUND,
            format!("Method not found: {}", req.method),
        ),
    }
}

fn handle_status(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let state = state.lock().unwrap();
    RpcResponse::success(req.id, json!({
        "tick": state.snapshot.tick,
        "fps": state.snapshot.fps,
        "entity_count": state.snapshot.entity_count,
        "scene": state.snapshot.scene,
        "paused": state.snapshot.paused,
    }))
}

fn handle_pause(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let mut state = state.lock().unwrap();
    state.pending_commands.push(ApiCommand {
        action: "pause".into(),
        params: Value::Null,
    });
    RpcResponse::success(req.id, json!({"ok": true}))
}

fn handle_unpause(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let mut state = state.lock().unwrap();
    state.pending_commands.push(ApiCommand {
        action: "unpause".into(),
        params: Value::Null,
    });
    RpcResponse::success(req.id, json!({"ok": true}))
}

fn handle_step(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let ticks = req.params.get("ticks").and_then(|v| v.as_u64()).unwrap_or(1);
    let mut state = state.lock().unwrap();
    state.pending_commands.push(ApiCommand {
        action: "step".into(),
        params: json!({"ticks": ticks}),
    });
    RpcResponse::success(req.id, json!({"ok": true, "ticks": ticks}))
}

fn handle_command(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let action = req.params.get("action").and_then(|v| v.as_str());
    let params = req.params.get("params").cloned().unwrap_or(Value::Null);
    match action {
        Some(action) => {
            let mut state = state.lock().unwrap();
            state.pending_commands.push(ApiCommand {
                action: action.to_string(),
                params,
            });
            RpcResponse::success(req.id, json!({"ok": true}))
        }
        None => RpcResponse::error(
            req.id,
            crate::INVALID_PARAMS,
            "Missing 'action' in params",
        ),
    }
}

fn handle_get_log(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let limit = req.params.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let state = state.lock().unwrap();
    let start = state.log_buffer.len().saturating_sub(limit);
    let lines: Vec<_> = state.log_buffer[start..].to_vec();
    RpcResponse::success(req.id, json!({"lines": lines}))
}

fn handle_set_property(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let key = req.params.get("key").and_then(|v| v.as_str());
    let value = req.params.get("value");
    match (key, value) {
        (Some(key), Some(value)) => {
            let mut state = state.lock().unwrap();
            state.snapshot.custom.insert(key.to_string(), value.clone());
            RpcResponse::success(req.id, json!({"ok": true}))
        }
        _ => RpcResponse::error(
            req.id,
            crate::INVALID_PARAMS,
            "Missing 'key' or 'value' in params",
        ),
    }
}

fn handle_get_property(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let key = req.params.get("key").and_then(|v| v.as_str());
    match key {
        Some(key) => {
            let state = state.lock().unwrap();
            let value = state.snapshot.custom.get(key).cloned().unwrap_or(Value::Null);
            RpcResponse::success(req.id, json!({"key": key, "value": value}))
        }
        None => RpcResponse::error(req.id, crate::INVALID_PARAMS, "Missing 'key' in params"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: &str, params: Value) -> RpcRequest {
        RpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(1),
            method: method.into(),
            params,
        }
    }

    #[test]
    fn status_returns_snapshot() {
        let state = new_shared_state();
        {
            let mut s = state.lock().unwrap();
            s.snapshot.tick = 42;
            s.snapshot.fps = 60.0;
            s.snapshot.entity_count = 100;
        }
        let resp = handle_request(&make_request("engine.status", Value::Null), &state);
        let result = resp.result.unwrap();
        assert_eq!(result["tick"], 42);
        assert_eq!(result["entity_count"], 100);
    }

    #[test]
    fn pause_queues_command() {
        let state = new_shared_state();
        handle_request(&make_request("engine.pause", Value::Null), &state);
        let s = state.lock().unwrap();
        assert_eq!(s.pending_commands.len(), 1);
        assert_eq!(s.pending_commands[0].action, "pause");
    }

    #[test]
    fn command_with_params() {
        let state = new_shared_state();
        let req = make_request("engine.command", json!({"action": "spawn", "params": {"x": 10}}));
        let resp = handle_request(&req, &state);
        assert!(resp.error.is_none());
        let s = state.lock().unwrap();
        assert_eq!(s.pending_commands[0].action, "spawn");
    }

    #[test]
    fn unknown_method_returns_error() {
        let state = new_shared_state();
        let resp = handle_request(&make_request("nonexistent", Value::Null), &state);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn set_and_get_property() {
        let state = new_shared_state();
        handle_request(
            &make_request("engine.set_property", json!({"key": "difficulty", "value": 3})),
            &state,
        );
        let resp = handle_request(
            &make_request("engine.get_property", json!({"key": "difficulty"})),
            &state,
        );
        let result = resp.result.unwrap();
        assert_eq!(result["value"], 3);
    }
}
