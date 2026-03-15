use crate::{RpcRequest, RpcResponse, INVALID_PARAMS, METHOD_NOT_FOUND};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Shared state types
// ---------------------------------------------------------------------------

/// Snapshot of engine state exposed to AI agents.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EngineSnapshot {
    pub tick: u64,
    pub fps: f32,
    pub frame_time_ms: f32,
    pub entity_count: usize,
    pub draw_calls: u32,
    pub scene: String,
    pub paused: bool,
    pub speed_multiplier: f32,
    pub custom: HashMap<String, Value>,
}

/// An entity visible through the API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityInfo {
    pub id: u64,
    pub entity_type: String,
    pub pos: [f32; 2],
    pub health: Option<f32>,
    pub components: HashMap<String, Value>,
}

/// Screenshot request queued for the render thread.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScreenshotRequest {
    pub path: String,
    pub overlays: Vec<String>,
    pub area: Option<[f32; 4]>,
    pub mode: String,
    pub heatmap_type: Option<String>,
}

/// A game event streamed to subscribed clients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameEvent {
    pub event: String,
    pub data: Value,
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
    pub entities: Vec<EntityInfo>,
    pub screenshot_queue: Vec<ScreenshotRequest>,
    pub screenshot_results: Vec<Value>,
    pub event_buffer: Vec<GameEvent>,
    pub subscriptions: Vec<String>,
}

impl ApiSharedState {
    pub fn new() -> Self {
        Self {
            snapshot: EngineSnapshot {
                speed_multiplier: 1.0,
                ..Default::default()
            },
            pending_commands: Vec::new(),
            log_buffer: Vec::new(),
            entities: Vec::new(),
            screenshot_queue: Vec::new(),
            screenshot_results: Vec::new(),
            event_buffer: Vec::new(),
            subscriptions: Vec::new(),
        }
    }

    /// Push a game event (called from engine main loop).
    pub fn push_event(&mut self, event: &str, data: Value) {
        self.event_buffer.push(GameEvent {
            event: event.to_string(),
            data,
        });
        // Cap buffer to prevent unbounded growth
        if self.event_buffer.len() > 1000 {
            self.event_buffer.drain(..500);
        }
    }

    /// Update the entity list (called from engine main loop each tick).
    pub fn update_entities(&mut self, entities: Vec<EntityInfo>) {
        self.entities = entities;
    }

    /// Drain pending commands (called from engine main loop).
    pub fn drain_commands(&mut self) -> Vec<ApiCommand> {
        std::mem::take(&mut self.pending_commands)
    }

    /// Drain screenshot requests (called from render thread).
    pub fn drain_screenshot_requests(&mut self) -> Vec<ScreenshotRequest> {
        std::mem::take(&mut self.screenshot_queue)
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

// ---------------------------------------------------------------------------
// Helper: queue a command and return ok
// ---------------------------------------------------------------------------

fn queue_cmd(req: &RpcRequest, state: &SharedState, action: &str, params: Value) -> RpcResponse {
    let mut s = state.lock().unwrap();
    s.pending_commands.push(ApiCommand {
        action: action.to_string(),
        params,
    });
    RpcResponse::success(req.id, json!({"ok": true}))
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing '{}'", key))
}

fn require_f64(params: &Value, key: &str) -> Result<f64, String> {
    params
        .get(key)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| format!("Missing '{}'", key))
}

fn require_i64(params: &Value, key: &str) -> Result<i64, String> {
    params
        .get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("Missing '{}'", key))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Route an RPC request to the appropriate handler.
pub fn handle_request(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match req.method.as_str() {
        // ── Observation ──
        "engine.status" | "get_state" => handle_status(req, state),
        "perf" => handle_perf(req, state),
        "screenshot" | "amigo_screenshot" => handle_screenshot(req, state),
        "screenshot.results" => handle_screenshot_results(req, state),
        "list_entities" => handle_list_entities(req, state),
        "inspect_entity" => handle_inspect_entity(req, state),

        // ── Engine control ──
        "engine.pause" | "pause" => queue_cmd(req, state, "pause", Value::Null),
        "engine.unpause" | "unpause" => queue_cmd(req, state, "unpause", Value::Null),
        "engine.step" | "debug.step" => handle_step(req, state),
        "engine.command" => handle_command(req, state),
        "set_speed" => handle_set_speed(req, state),
        "tick" | "amigo_tick" => handle_tick(req, state),

        // ── Simulation ──
        "place_tower" => handle_place_tower(req, state),
        "sell_tower" => handle_sell_tower(req, state),
        "upgrade_tower" => handle_upgrade_tower(req, state),
        "start_wave" => queue_cmd(req, state, "start_wave", Value::Null),
        "spawn" => handle_spawn(req, state),

        // ── Editor ──
        "editor.new_level" => handle_editor_new_level(req, state),
        "editor.paint_tile" => handle_editor_paint_tile(req, state),
        "editor.fill_rect" => handle_editor_fill_rect(req, state),
        "editor.place_entity" => handle_editor_place_entity(req, state),
        "editor.add_path" => handle_editor_add_path(req, state),
        "editor.move_path_point" => handle_editor_move_path_point(req, state),
        "editor.auto_decorate" => handle_editor_auto_decorate(req, state),
        "editor.save" => handle_editor_save(req, state),
        "editor.load" => handle_editor_load(req, state),
        "editor.undo" => queue_cmd(req, state, "editor.undo", Value::Null),
        "editor.redo" => queue_cmd(req, state, "editor.redo", Value::Null),

        // ── Audio ──
        "audio.play" => handle_audio_play(req, state),
        "audio.play_music" => handle_audio_play_music(req, state),
        "audio.crossfade" => handle_audio_crossfade(req, state),
        "audio.set_volume" => handle_audio_set_volume(req, state),
        "audio.list" => queue_cmd(req, state, "audio.list", Value::Null),

        // ── Save / Load / Replay ──
        "save" => handle_save(req, state),
        "load" => handle_load(req, state),
        "replay.record_start" => queue_cmd(req, state, "replay.record_start", Value::Null),
        "replay.record_stop" => handle_replay_record_stop(req, state),
        "replay.play" => handle_replay_play(req, state),

        // ── Debug ──
        "debug.dump_state" => handle_debug_dump_state(req, state),
        "debug.tile_collision" => handle_debug_tile_collision(req, state),
        "debug.state_crc" => queue_cmd(req, state, "debug.state_crc", Value::Null),

        // ── Events ──
        "subscribe" => handle_subscribe(req, state),
        "poll_events" => handle_poll_events(req, state),

        // ── Properties / Log (existing) ──
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

// ---------------------------------------------------------------------------
// Observation handlers
// ---------------------------------------------------------------------------

fn handle_status(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let s = state.lock().unwrap();
    RpcResponse::success(
        req.id,
        json!({
            "tick": s.snapshot.tick,
            "fps": s.snapshot.fps,
            "entity_count": s.snapshot.entity_count,
            "scene": s.snapshot.scene,
            "paused": s.snapshot.paused,
            "speed": s.snapshot.speed_multiplier,
        }),
    )
}

fn handle_perf(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let s = state.lock().unwrap();
    RpcResponse::success(
        req.id,
        json!({
            "fps": s.snapshot.fps,
            "frame_time_ms": s.snapshot.frame_time_ms,
            "entities": s.snapshot.entity_count,
            "draw_calls": s.snapshot.draw_calls,
        }),
    )
}

fn handle_screenshot(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let path = req
        .params
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("/tmp/screenshot.png");
    let overlays: Vec<String> = req
        .params
        .get("overlays")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let area: Option<[f32; 4]> = req
        .params
        .get("area")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let mode = req
        .params
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("normal")
        .to_string();
    let heatmap_type = req
        .params
        .get("heatmap_type")
        .and_then(|v| v.as_str())
        .map(String::from);

    let mut s = state.lock().unwrap();
    s.screenshot_queue.push(ScreenshotRequest {
        path: path.to_string(),
        overlays,
        area,
        mode,
        heatmap_type,
    });
    RpcResponse::success(req.id, json!({"queued": true, "path": path}))
}

fn handle_screenshot_results(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let mut s = state.lock().unwrap();
    let results = std::mem::take(&mut s.screenshot_results);
    RpcResponse::success(req.id, json!({"results": results}))
}

fn handle_list_entities(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let s = state.lock().unwrap();
    let filter = req.params.get("filter").and_then(|v| v.as_str());
    let near: Option<[f32; 2]> = req
        .params
        .get("near")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let radius = req
        .params
        .get("radius")
        .and_then(|v| v.as_f64())
        .unwrap_or(f64::MAX);

    let entities: Vec<Value> = s
        .entities
        .iter()
        .filter(|e| {
            if let Some(f) = filter {
                if !e.entity_type.contains(f) {
                    return false;
                }
            }
            if let Some(center) = near {
                let dx = (e.pos[0] - center[0]) as f64;
                let dy = (e.pos[1] - center[1]) as f64;
                if (dx * dx + dy * dy).sqrt() > radius {
                    return false;
                }
            }
            true
        })
        .map(|e| {
            json!({
                "id": e.id,
                "type": e.entity_type,
                "pos": e.pos,
                "health": e.health,
            })
        })
        .collect();

    RpcResponse::success(req.id, json!(entities))
}

fn handle_inspect_entity(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let id = match req.params.get("id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => return RpcResponse::error(req.id, INVALID_PARAMS, "Missing 'id'"),
    };
    let s = state.lock().unwrap();
    match s.entities.iter().find(|e| e.id == id) {
        Some(e) => RpcResponse::success(req.id, serde_json::to_value(e).unwrap()),
        None => RpcResponse::error(req.id, INVALID_PARAMS, format!("Entity {} not found", id)),
    }
}

// ---------------------------------------------------------------------------
// Engine control handlers
// ---------------------------------------------------------------------------

fn handle_step(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let ticks = req
        .params
        .get("ticks")
        .or(req.params.get("count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    queue_cmd(req, state, "step", json!({"ticks": ticks}))
}

fn handle_set_speed(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match req.params.get("multiplier").and_then(|v| v.as_f64()) {
        Some(m) => queue_cmd(req, state, "set_speed", json!({"multiplier": m})),
        None => RpcResponse::error(req.id, INVALID_PARAMS, "Missing 'multiplier'"),
    }
}

fn handle_tick(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let count = req
        .params
        .get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    queue_cmd(req, state, "tick", json!({"count": count}))
}

fn handle_command(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match req.params.get("action").and_then(|v| v.as_str()) {
        Some(action) => {
            let params = req.params.get("params").cloned().unwrap_or(Value::Null);
            queue_cmd(req, state, action, params)
        }
        None => RpcResponse::error(req.id, INVALID_PARAMS, "Missing 'action' in params"),
    }
}

// ---------------------------------------------------------------------------
// Simulation handlers
// ---------------------------------------------------------------------------

fn handle_place_tower(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (
        require_i64(p, "x"),
        require_i64(p, "y"),
        require_str(p, "tower_type"),
    ) {
        (Ok(x), Ok(y), Ok(tt)) => queue_cmd(
            req,
            state,
            "place_tower",
            json!({"x": x, "y": y, "tower_type": tt}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: x, y, tower_type"),
    }
}

fn handle_sell_tower(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match req.params.get("tower_id").and_then(|v| v.as_u64()) {
        Some(id) => queue_cmd(req, state, "sell_tower", json!({"tower_id": id})),
        None => RpcResponse::error(req.id, INVALID_PARAMS, "Missing 'tower_id'"),
    }
}

fn handle_upgrade_tower(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (
        p.get("tower_id").and_then(|v| v.as_u64()),
        require_str(p, "path"),
    ) {
        (Some(id), Ok(path)) => queue_cmd(
            req,
            state,
            "upgrade_tower",
            json!({"tower_id": id, "path": path}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: tower_id, path"),
    }
}

fn handle_spawn(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    let entity_type = p.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let subtype = p.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
    let pos = p.get("pos").cloned().unwrap_or(json!([0, 0]));
    queue_cmd(
        req,
        state,
        "spawn",
        json!({"type": entity_type, "subtype": subtype, "pos": pos}),
    )
}

// ---------------------------------------------------------------------------
// Editor handlers
// ---------------------------------------------------------------------------

fn handle_editor_new_level(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    let world = p.get("world").and_then(|v| v.as_str()).unwrap_or("default");
    let width = p.get("width").and_then(|v| v.as_u64()).unwrap_or(30);
    let height = p.get("height").and_then(|v| v.as_u64()).unwrap_or(20);
    queue_cmd(
        req,
        state,
        "editor.new_level",
        json!({"world": world, "width": width, "height": height}),
    )
}

fn handle_editor_paint_tile(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (
        require_str(p, "layer"),
        require_i64(p, "x"),
        require_i64(p, "y"),
        require_i64(p, "tile"),
    ) {
        (Ok(layer), Ok(x), Ok(y), Ok(tile)) => queue_cmd(
            req,
            state,
            "editor.paint_tile",
            json!({"layer": layer, "x": x, "y": y, "tile": tile}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: layer, x, y, tile"),
    }
}

fn handle_editor_fill_rect(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (
        require_str(p, "layer"),
        require_i64(p, "x"),
        require_i64(p, "y"),
        require_i64(p, "w"),
        require_i64(p, "h"),
        require_i64(p, "tile"),
    ) {
        (Ok(layer), Ok(x), Ok(y), Ok(w), Ok(h), Ok(tile)) => queue_cmd(
            req,
            state,
            "editor.fill_rect",
            json!({"layer": layer, "x": x, "y": y, "w": w, "h": h, "tile": tile}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: layer, x, y, w, h, tile"),
    }
}

fn handle_editor_place_entity(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (
        require_str(p, "type"),
        require_f64(p, "x"),
        require_f64(p, "y"),
    ) {
        (Ok(t), Ok(x), Ok(y)) => queue_cmd(
            req,
            state,
            "editor.place_entity",
            json!({"type": t, "x": x, "y": y}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: type, x, y"),
    }
}

fn handle_editor_add_path(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match req.params.get("points") {
        Some(points) => queue_cmd(req, state, "editor.add_path", json!({"points": points})),
        None => RpcResponse::error(req.id, INVALID_PARAMS, "Missing 'points'"),
    }
}

fn handle_editor_move_path_point(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (
        p.get("path").and_then(|v| v.as_u64()),
        p.get("point").and_then(|v| v.as_u64()),
        p.get("new_pos"),
    ) {
        (Some(path), Some(point), Some(new_pos)) => queue_cmd(
            req,
            state,
            "editor.move_path_point",
            json!({"path": path, "point": point, "new_pos": new_pos}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: path, point, new_pos"),
    }
}

fn handle_editor_auto_decorate(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let world = req
        .params
        .get("world")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    queue_cmd(req, state, "editor.auto_decorate", json!({"world": world}))
}

fn handle_editor_save(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match require_str(&req.params, "path") {
        Ok(path) => queue_cmd(req, state, "editor.save", json!({"path": path})),
        Err(e) => RpcResponse::error(req.id, INVALID_PARAMS, e),
    }
}

fn handle_editor_load(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match require_str(&req.params, "path") {
        Ok(path) => queue_cmd(req, state, "editor.load", json!({"path": path})),
        Err(e) => RpcResponse::error(req.id, INVALID_PARAMS, e),
    }
}

// ---------------------------------------------------------------------------
// Audio handlers
// ---------------------------------------------------------------------------

fn handle_audio_play(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match require_str(&req.params, "name") {
        Ok(name) => queue_cmd(req, state, "audio.play", json!({"name": name})),
        Err(e) => RpcResponse::error(req.id, INVALID_PARAMS, e),
    }
}

fn handle_audio_play_music(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match require_str(&req.params, "name") {
        Ok(name) => queue_cmd(req, state, "audio.play_music", json!({"name": name})),
        Err(e) => RpcResponse::error(req.id, INVALID_PARAMS, e),
    }
}

fn handle_audio_crossfade(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (require_str(p, "name"), require_f64(p, "duration")) {
        (Ok(name), Ok(dur)) => queue_cmd(
            req,
            state,
            "audio.crossfade",
            json!({"name": name, "duration": dur}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: name, duration"),
    }
}

fn handle_audio_set_volume(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (require_str(p, "channel"), require_f64(p, "volume")) {
        (Ok(ch), Ok(vol)) => queue_cmd(
            req,
            state,
            "audio.set_volume",
            json!({"channel": ch, "volume": vol}),
        ),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: channel, volume"),
    }
}

// ---------------------------------------------------------------------------
// Save / Load / Replay handlers
// ---------------------------------------------------------------------------

fn handle_save(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let path = req
        .params
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("saves/quicksave.ron");
    let slot = req.params.get("slot").and_then(|v| v.as_str());
    queue_cmd(req, state, "save", json!({"path": path, "slot": slot}))
}

fn handle_load(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let path = req.params.get("path").and_then(|v| v.as_str());
    let slot = req.params.get("slot").and_then(|v| v.as_str());
    queue_cmd(req, state, "load", json!({"path": path, "slot": slot}))
}

fn handle_replay_record_stop(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match require_str(&req.params, "path") {
        Ok(path) => queue_cmd(req, state, "replay.record_stop", json!({"path": path})),
        Err(e) => RpcResponse::error(req.id, INVALID_PARAMS, e),
    }
}

fn handle_replay_play(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match require_str(&req.params, "path") {
        Ok(path) => {
            let from_tick = req.params.get("from_tick").and_then(|v| v.as_u64());
            queue_cmd(
                req,
                state,
                "replay.play",
                json!({"path": path, "from_tick": from_tick}),
            )
        }
        Err(e) => RpcResponse::error(req.id, INVALID_PARAMS, e),
    }
}

// ---------------------------------------------------------------------------
// Debug handlers
// ---------------------------------------------------------------------------

fn handle_debug_dump_state(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let path = req
        .params
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("/tmp/state.ron");
    queue_cmd(req, state, "debug.dump_state", json!({"path": path}))
}

fn handle_debug_tile_collision(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let p = &req.params;
    match (require_i64(p, "x"), require_i64(p, "y")) {
        (Ok(x), Ok(y)) => queue_cmd(req, state, "debug.tile_collision", json!({"x": x, "y": y})),
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Required: x, y"),
    }
}

// ---------------------------------------------------------------------------
// Event streaming
// ---------------------------------------------------------------------------

fn handle_subscribe(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let events: Vec<String> = req
        .params
        .get("events")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let mut s = state.lock().unwrap();
    for event in &events {
        if !s.subscriptions.contains(event) {
            s.subscriptions.push(event.clone());
        }
    }
    RpcResponse::success(req.id, json!({"subscribed": events}))
}

fn handle_poll_events(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let limit = req
        .params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;
    let mut s = state.lock().unwrap();
    let count = s.event_buffer.len().min(limit);
    let drained: Vec<GameEvent> = s.event_buffer.drain(..count).collect();
    let events: Vec<Value> = drained
        .into_iter()
        .filter(|e| s.subscriptions.is_empty() || s.subscriptions.contains(&e.event))
        .map(|e| json!({"event": e.event, "data": e.data}))
        .collect();
    RpcResponse::success(req.id, json!({"events": events}))
}

// ---------------------------------------------------------------------------
// Properties / Log (existing)
// ---------------------------------------------------------------------------

fn handle_get_log(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let limit = req
        .params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;
    let s = state.lock().unwrap();
    let start = s.log_buffer.len().saturating_sub(limit);
    let lines: Vec<_> = s.log_buffer[start..].to_vec();
    RpcResponse::success(req.id, json!({"lines": lines}))
}

fn handle_set_property(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    let key = req.params.get("key").and_then(|v| v.as_str());
    let value = req.params.get("value");
    match (key, value) {
        (Some(key), Some(value)) => {
            let mut s = state.lock().unwrap();
            s.snapshot.custom.insert(key.to_string(), value.clone());
            RpcResponse::success(req.id, json!({"ok": true}))
        }
        _ => RpcResponse::error(req.id, INVALID_PARAMS, "Missing 'key' or 'value'"),
    }
}

fn handle_get_property(req: &RpcRequest, state: &SharedState) -> RpcResponse {
    match req.params.get("key").and_then(|v| v.as_str()) {
        Some(key) => {
            let s = state.lock().unwrap();
            let value = s.snapshot.custom.get(key).cloned().unwrap_or(Value::Null);
            RpcResponse::success(req.id, json!({"key": key, "value": value}))
        }
        None => RpcResponse::error(req.id, INVALID_PARAMS, "Missing 'key'"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        let req = make_request(
            "engine.command",
            json!({"action": "spawn", "params": {"x": 10}}),
        );
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
            &make_request(
                "engine.set_property",
                json!({"key": "difficulty", "value": 3}),
            ),
            &state,
        );
        let resp = handle_request(
            &make_request("engine.get_property", json!({"key": "difficulty"})),
            &state,
        );
        let result = resp.result.unwrap();
        assert_eq!(result["value"], 3);
    }

    #[test]
    fn perf_returns_metrics() {
        let state = new_shared_state();
        {
            let mut s = state.lock().unwrap();
            s.snapshot.fps = 60.0;
            s.snapshot.frame_time_ms = 2.1;
            s.snapshot.draw_calls = 8;
        }
        let resp = handle_request(&make_request("perf", Value::Null), &state);
        let r = resp.result.unwrap();
        assert_eq!(r["draw_calls"], 8);
    }

    #[test]
    fn screenshot_queues_request() {
        let state = new_shared_state();
        let resp = handle_request(
            &make_request(
                "screenshot",
                json!({"path": "/tmp/test.png", "overlays": ["grid"]}),
            ),
            &state,
        );
        assert!(resp.error.is_none());
        let s = state.lock().unwrap();
        assert_eq!(s.screenshot_queue.len(), 1);
        assert_eq!(s.screenshot_queue[0].path, "/tmp/test.png");
        assert_eq!(s.screenshot_queue[0].overlays, vec!["grid"]);
    }

    #[test]
    fn list_entities_with_filter() {
        let state = new_shared_state();
        {
            let mut s = state.lock().unwrap();
            s.entities = vec![
                EntityInfo {
                    id: 1,
                    entity_type: "enemy_orc".into(),
                    pos: [5.0, 5.0],
                    health: Some(100.0),
                    components: HashMap::new(),
                },
                EntityInfo {
                    id: 2,
                    entity_type: "tower_archer".into(),
                    pos: [10.0, 10.0],
                    health: None,
                    components: HashMap::new(),
                },
            ];
        }
        let resp = handle_request(
            &make_request("list_entities", json!({"filter": "enemy"})),
            &state,
        );
        let result = resp.result.unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "enemy_orc");
    }

    #[test]
    fn inspect_entity_found() {
        let state = new_shared_state();
        {
            let mut s = state.lock().unwrap();
            s.entities = vec![EntityInfo {
                id: 42,
                entity_type: "boss".into(),
                pos: [0.0, 0.0],
                health: Some(500.0),
                components: HashMap::new(),
            }];
        }
        let resp = handle_request(&make_request("inspect_entity", json!({"id": 42})), &state);
        assert!(resp.error.is_none());
        let r = resp.result.unwrap();
        assert_eq!(r["entity_type"], "boss");
    }

    #[test]
    fn place_tower_queues() {
        let state = new_shared_state();
        let resp = handle_request(
            &make_request(
                "place_tower",
                json!({"x": 5, "y": 3, "tower_type": "archer"}),
            ),
            &state,
        );
        assert!(resp.error.is_none());
        let s = state.lock().unwrap();
        assert_eq!(s.pending_commands[0].action, "place_tower");
    }

    #[test]
    fn place_tower_missing_params() {
        let state = new_shared_state();
        let resp = handle_request(&make_request("place_tower", json!({"x": 5})), &state);
        assert!(resp.error.is_some());
    }

    #[test]
    fn editor_paint_tile_queues() {
        let state = new_shared_state();
        let resp = handle_request(
            &make_request(
                "editor.paint_tile",
                json!({"layer": "terrain", "x": 3, "y": 4, "tile": 42}),
            ),
            &state,
        );
        assert!(resp.error.is_none());
        let s = state.lock().unwrap();
        assert_eq!(s.pending_commands[0].action, "editor.paint_tile");
    }

    #[test]
    fn subscribe_and_poll() {
        let state = new_shared_state();
        handle_request(
            &make_request(
                "subscribe",
                json!({"events": ["enemy_killed", "wave_complete"]}),
            ),
            &state,
        );

        // Push events from "engine side"
        {
            let mut s = state.lock().unwrap();
            s.push_event("enemy_killed", json!({"id": 17}));
            s.push_event("tower_fired", json!({"tower": 1}));
            s.push_event("wave_complete", json!({"wave": 3}));
        }

        let resp = handle_request(&make_request("poll_events", json!({})), &state);
        let r = resp.result.unwrap();
        let events = r["events"].as_array().unwrap();
        // tower_fired should be filtered out
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["event"], "enemy_killed");
        assert_eq!(events[1]["event"], "wave_complete");
    }

    #[test]
    fn drain_commands_clears() {
        let state = new_shared_state();
        handle_request(&make_request("engine.pause", Value::Null), &state);
        handle_request(&make_request("start_wave", Value::Null), &state);

        let mut s = state.lock().unwrap();
        let cmds = s.drain_commands();
        assert_eq!(cmds.len(), 2);
        assert!(s.pending_commands.is_empty());
    }

    #[test]
    fn audio_crossfade_params() {
        let state = new_shared_state();
        let resp = handle_request(
            &make_request(
                "audio.crossfade",
                json!({"name": "battle", "duration": 2.0}),
            ),
            &state,
        );
        assert!(resp.error.is_none());
        let s = state.lock().unwrap();
        assert_eq!(s.pending_commands[0].action, "audio.crossfade");
    }

    #[test]
    fn save_load_roundtrip() {
        let state = new_shared_state();
        handle_request(&make_request("save", json!({"path": "test.ron"})), &state);
        handle_request(&make_request("load", json!({"path": "test.ron"})), &state);
        let s = state.lock().unwrap();
        assert_eq!(s.pending_commands[0].action, "save");
        assert_eq!(s.pending_commands[1].action, "load");
    }

    #[test]
    fn get_state_alias() {
        let state = new_shared_state();
        let resp = handle_request(&make_request("get_state", Value::Null), &state);
        assert!(resp.error.is_none());
        assert!(resp.result.unwrap().get("tick").is_some());
    }
}
