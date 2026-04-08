//! MCP protocol handling — initialize, list tools, call tool.

use crate::tools::tool_definitions;
use crate::{McpRequest, McpResponse};
use serde_json::json;

/// Handle an incoming MCP request.
pub fn handle_mcp_request(
    req: &McpRequest,
    api_call: &dyn Fn(&str, serde_json::Value) -> Result<serde_json::Value, String>,
) -> McpResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req),
        "initialized" => McpResponse::success(req.id.clone(), json!({})),
        "tools/list" => handle_tools_list(req),
        "tools/call" => handle_tools_call(req, api_call),
        "ping" => McpResponse::success(req.id.clone(), json!({})),
        _ => McpResponse::error(
            req.id.clone(),
            -32601,
            format!("Method not found: {}", req.method),
        ),
    }
}

fn handle_initialize(req: &McpRequest) -> McpResponse {
    McpResponse::success(
        req.id.clone(),
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "amigo",
                "version": "0.1.0"
            }
        }),
    )
}

fn handle_tools_list(req: &McpRequest) -> McpResponse {
    McpResponse::success(
        req.id.clone(),
        json!({
            "tools": tool_definitions()
        }),
    )
}

fn handle_tools_call(
    req: &McpRequest,
    api_call: &dyn Fn(&str, serde_json::Value) -> Result<serde_json::Value, String>,
) -> McpResponse {
    let tool_name = req
        .params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let arguments = req.params.get("arguments").cloned().unwrap_or(json!({}));

    // Map MCP tool name → JSON-RPC method
    let rpc_method = match tool_name {
        "amigo_screenshot" => "screenshot",
        "amigo_get_state" => "get_state",
        "amigo_list_entities" => "list_entities",
        "amigo_inspect_entity" => "inspect_entity",
        "amigo_perf" => "perf",
        "amigo_place_tower" => "place_tower",
        "amigo_sell_tower" => "sell_tower",
        "amigo_upgrade_tower" => "upgrade_tower",
        "amigo_start_wave" => "start_wave",
        "amigo_tick" => "tick",
        "amigo_set_speed" => "set_speed",
        "amigo_pause" => "pause",
        "amigo_unpause" => "unpause",
        "amigo_spawn" => "spawn",
        "amigo_editor_new_level" => "editor.new_level",
        "amigo_editor_paint_tile" => "editor.paint_tile",
        "amigo_editor_fill_rect" => "editor.fill_rect",
        "amigo_editor_place_entity" => "editor.place_entity",
        "amigo_editor_add_path" => "editor.add_path",
        "amigo_editor_move_path_point" => "editor.move_path_point",
        "amigo_editor_auto_decorate" => "editor.auto_decorate",
        "amigo_editor_save" => "editor.save",
        "amigo_editor_load" => "editor.load",
        "amigo_editor_undo" => "editor.undo",
        "amigo_editor_redo" => "editor.redo",
        "amigo_audio_play" => "audio.play",
        "amigo_audio_play_music" => "audio.play_music",
        "amigo_audio_crossfade" => "audio.crossfade",
        "amigo_audio_set_volume" => "audio.set_volume",
        "amigo_save" => "save",
        "amigo_load" => "load",
        "amigo_replay_record_start" => "replay.record_start",
        "amigo_replay_record_stop" => "replay.record_stop",
        "amigo_replay_play" => "replay.play",
        "amigo_debug_dump_state" => "debug.dump_state",
        "amigo_debug_tile_collision" => "debug.tile_collision",
        "amigo_debug_step" => "debug.step",
        "amigo_debug_state_crc" => "debug.state_crc",
        "amigo_tilemap_get_tile" => "tilemap.get_tile",
        "amigo_tilemap_get_region" => "tilemap.get_region",
        "amigo_tilemap_collision_at" => "tilemap.collision_at",
        "amigo_tilemap_dimensions" => "tilemap.dimensions",
        "amigo_camera_get" => "camera.get",
        "amigo_camera_set" => "camera.set",
        "amigo_camera_shake" => "camera.shake",
        "amigo_camera_follow" => "camera.follow",
        "amigo_lighting_add" => "lighting.add",
        "amigo_lighting_remove" => "lighting.remove",
        "amigo_lighting_list" => "lighting.list",
        "amigo_particles_spawn" => "particles.spawn",
        "amigo_particles_stop" => "particles.stop",
        "amigo_inventory_list" => "inventory.list",
        "amigo_inventory_add" => "inventory.add",
        "amigo_inventory_remove" => "inventory.remove",
        "amigo_crafting_list_recipes" => "crafting.list_recipes",
        "amigo_crafting_craft" => "crafting.craft",
        "amigo_dialogue_start" => "dialogue.start",
        "amigo_dialogue_choose" => "dialogue.choose",
        "amigo_dialogue_get_state" => "dialogue.get_state",
        "amigo_dialogue_set_flag" => "dialogue.set_flag",
        "amigo_pack" => "pack",
        "amigo_dev_save_snapshot" => "dev.save_snapshot",
        "amigo_dev_snapshot_status" => "dev.snapshot_status",
        "amigo_dev_restore_snapshot" => "dev.restore_snapshot",
        "amigo_preview_level" => "preview.level",
        "amigo_preview_palette" => "preview.palette",
        "amigo_diff_levels" => "diff.levels",
        "amigo_metrics_snapshot" => "metrics.snapshot",
        "amigo_metrics_clear" => "metrics.clear",
        _ => {
            return McpResponse::success(
                req.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Unknown tool: {}", tool_name)
                    }],
                    "isError": true
                }),
            );
        }
    };

    match api_call(rpc_method, arguments) {
        Ok(result) => McpResponse::success(
            req.id.clone(),
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                }]
            }),
        ),
        Err(e) => McpResponse::success(
            req.id.clone(),
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("Error: {}", e)
                }],
                "isError": true
            }),
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_api_call(
        _method: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(json!({"tick": 100, "fps": 60}))
    }

    fn make_req(method: &str, params: serde_json::Value) -> McpRequest {
        McpRequest {
            jsonrpc: "2.0".into(),
            id: Some(json!(1)),
            method: method.into(),
            params,
        }
    }

    // ── Initialize and tool listing ──────────────────────────────

    #[test]
    fn initialize_returns_capabilities() {
        let resp = handle_mcp_request(&make_req("initialize", json!({})), &mock_api_call);
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn tools_list_returns_array() {
        let resp = handle_mcp_request(&make_req("tools/list", json!({})), &mock_api_call);
        let result = resp.result.unwrap();
        assert!(result["tools"].is_array());
        let tools = result["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
    }

    // ── Tool call routing ───────────────────────────────────────

    #[test]
    fn tools_call_routes_to_api() {
        let resp = handle_mcp_request(
            &make_req(
                "tools/call",
                json!({"name": "amigo_get_state", "arguments": {}}),
            ),
            &mock_api_call,
        );
        let result = resp.result.unwrap();
        let content = &result["content"][0];
        assert_eq!(content["type"], "text");
        assert!(content["text"].as_str().unwrap().contains("tick"));
    }

    #[test]
    fn unknown_tool_returns_error() {
        let resp = handle_mcp_request(
            &make_req(
                "tools/call",
                json!({"name": "nonexistent", "arguments": {}}),
            ),
            &mock_api_call,
        );
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }

    // ── Error handling ──────────────────────────────────────────

    #[test]
    fn unknown_method_returns_rpc_error() {
        let resp = handle_mcp_request(&make_req("bogus/method", json!({})), &mock_api_call);
        assert!(resp.error.is_some());
    }

    // ── Newly-routed tools ─────────────────────────────────────

    #[test]
    fn preview_level_routes_correctly() {
        let resp = handle_mcp_request(
            &make_req(
                "tools/call",
                json!({"name": "amigo_preview_level", "arguments": {"config": {}}}),
            ),
            &mock_api_call,
        );
        let result = resp.result.unwrap();
        assert_ne!(result.get("isError"), Some(&json!(true)));
    }

    #[test]
    fn metrics_snapshot_routes_correctly() {
        let resp = handle_mcp_request(
            &make_req(
                "tools/call",
                json!({"name": "amigo_metrics_snapshot", "arguments": {}}),
            ),
            &mock_api_call,
        );
        let result = resp.result.unwrap();
        assert_ne!(result.get("isError"), Some(&json!(true)));
    }

    #[test]
    fn metrics_clear_routes_correctly() {
        let resp = handle_mcp_request(
            &make_req(
                "tools/call",
                json!({"name": "amigo_metrics_clear", "arguments": {}}),
            ),
            &mock_api_call,
        );
        let result = resp.result.unwrap();
        assert_ne!(result.get("isError"), Some(&json!(true)));
    }

    #[test]
    fn diff_levels_routes_correctly() {
        let resp = handle_mcp_request(
            &make_req(
                "tools/call",
                json!({"name": "amigo_diff_levels", "arguments": {"a": "x", "b": "y"}}),
            ),
            &mock_api_call,
        );
        let result = resp.result.unwrap();
        assert_ne!(result.get("isError"), Some(&json!(true)));
    }

    #[test]
    fn preview_palette_routes_correctly() {
        let resp = handle_mcp_request(
            &make_req(
                "tools/call",
                json!({"name": "amigo_preview_palette", "arguments": {"primary": "#000"}}),
            ),
            &mock_api_call,
        );
        let result = resp.result.unwrap();
        assert_ne!(result.get("isError"), Some(&json!(true)));
    }
}
