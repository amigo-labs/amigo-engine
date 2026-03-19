//! MCP tool definitions for the Amigo Engine.
//!
//! Each tool maps 1:1 to a JSON-RPC method in amigo_api.
//! Tool definitions follow the MCP tool schema.

use serde_json::{json, Value};

/// All tool definitions exposed to Claude Code.
pub fn tool_definitions() -> Vec<Value> {
    vec![
        // ── Observation ──
        tool(
            "amigo_screenshot",
            "Capture a screenshot of the current frame, optionally with debug overlays.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Output file path for the screenshot"},
                    "overlays": {"type": "array", "items": {"type": "string"}, "description": "Debug overlays: collision, grid, paths, tower_ranges, entity_ids"},
                    "area": {"type": "array", "items": {"type": "number"}, "description": "Crop area [x, y, w, h]"},
                    "mode": {"type": "string", "description": "Capture mode: normal, heatmap"},
                    "heatmap_type": {"type": "string", "description": "Heatmap type: enemy_deaths, damage_taken"}
                }
            }),
        ),
        tool(
            "amigo_get_state",
            "Get the current game state: tick, gold, lives, wave, entity counts.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_list_entities",
            "List entities with optional filter by type or proximity.",
            json!({
                "type": "object",
                "properties": {
                    "filter": {"type": "string", "description": "Filter by entity type substring"},
                    "near": {"type": "array", "items": {"type": "number"}, "description": "Center point [x, y]"},
                    "radius": {"type": "number", "description": "Search radius around near point"}
                }
            }),
        ),
        tool(
            "amigo_inspect_entity",
            "Get full component dump for a single entity.",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Entity ID"}
                },
                "required": ["id"]
            }),
        ),
        tool(
            "amigo_perf",
            "Get performance metrics: FPS, frame time, entity count, draw calls.",
            json!({"type": "object", "properties": {}}),
        ),
        // ── Simulation ──
        tool(
            "amigo_place_tower",
            "Place a tower at tile coordinates.",
            json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "tower_type": {"type": "string"}
                },
                "required": ["x", "y", "tower_type"]
            }),
        ),
        tool(
            "amigo_sell_tower",
            "Sell a placed tower.",
            json!({
                "type": "object",
                "properties": {
                    "tower_id": {"type": "integer"}
                },
                "required": ["tower_id"]
            }),
        ),
        tool(
            "amigo_upgrade_tower",
            "Upgrade a tower along a path.",
            json!({
                "type": "object",
                "properties": {
                    "tower_id": {"type": "integer"},
                    "path": {"type": "string", "description": "Upgrade path: damage, range, speed"}
                },
                "required": ["tower_id", "path"]
            }),
        ),
        tool(
            "amigo_start_wave",
            "Start the next enemy wave.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_tick",
            "Advance simulation by N ticks (useful for headless mode).",
            json!({
                "type": "object",
                "properties": {
                    "count": {"type": "integer", "description": "Number of ticks to advance"}
                }
            }),
        ),
        tool(
            "amigo_set_speed",
            "Set simulation speed multiplier.",
            json!({
                "type": "object",
                "properties": {
                    "multiplier": {"type": "number"}
                },
                "required": ["multiplier"]
            }),
        ),
        tool(
            "amigo_pause",
            "Pause the simulation.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_unpause",
            "Unpause the simulation.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_spawn",
            "Spawn a debug entity.",
            json!({
                "type": "object",
                "properties": {
                    "type": {"type": "string"},
                    "subtype": {"type": "string"},
                    "pos": {"type": "array", "items": {"type": "number"}}
                },
                "required": ["type"]
            }),
        ),
        // ── Editor ──
        tool(
            "amigo_editor_new_level",
            "Create a new empty level.",
            json!({
                "type": "object",
                "properties": {
                    "world": {"type": "string"},
                    "width": {"type": "integer"},
                    "height": {"type": "integer"}
                }
            }),
        ),
        tool(
            "amigo_editor_paint_tile",
            "Paint a single tile.",
            json!({
                "type": "object",
                "properties": {
                    "layer": {"type": "string"},
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "tile": {"type": "integer"}
                },
                "required": ["layer", "x", "y", "tile"]
            }),
        ),
        tool(
            "amigo_editor_fill_rect",
            "Fill a rectangular area with a tile.",
            json!({
                "type": "object",
                "properties": {
                    "layer": {"type": "string"},
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "w": {"type": "integer"},
                    "h": {"type": "integer"},
                    "tile": {"type": "integer"}
                },
                "required": ["layer", "x", "y", "w", "h", "tile"]
            }),
        ),
        tool(
            "amigo_editor_place_entity",
            "Place an entity in the level.",
            json!({
                "type": "object",
                "properties": {
                    "type": {"type": "string"},
                    "x": {"type": "number"},
                    "y": {"type": "number"}
                },
                "required": ["type", "x", "y"]
            }),
        ),
        tool(
            "amigo_editor_add_path",
            "Add an enemy path defined by waypoints.",
            json!({
                "type": "object",
                "properties": {
                    "points": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}}
                },
                "required": ["points"]
            }),
        ),
        tool(
            "amigo_editor_move_path_point",
            "Move a specific point on a path.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "integer"},
                    "point": {"type": "integer"},
                    "new_pos": {"type": "array", "items": {"type": "number"}}
                },
                "required": ["path", "point", "new_pos"]
            }),
        ),
        tool(
            "amigo_editor_auto_decorate",
            "Auto-decorate non-gameplay tiles with themed decoration.",
            json!({
                "type": "object",
                "properties": {
                    "world": {"type": "string"}
                }
            }),
        ),
        tool(
            "amigo_editor_save",
            "Save the current level to a .amigo file.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        ),
        tool(
            "amigo_editor_load",
            "Load a level from a .amigo file.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        ),
        tool(
            "amigo_editor_undo",
            "Undo the last editor action.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_editor_redo",
            "Redo the last undone editor action.",
            json!({"type": "object", "properties": {}}),
        ),
        // ── Audio ──
        tool(
            "amigo_audio_play",
            "Play a sound effect.",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                },
                "required": ["name"]
            }),
        ),
        tool(
            "amigo_audio_play_music",
            "Play a music track.",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                },
                "required": ["name"]
            }),
        ),
        tool(
            "amigo_audio_crossfade",
            "Crossfade to a different music track.",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "duration": {"type": "number"}
                },
                "required": ["name", "duration"]
            }),
        ),
        tool(
            "amigo_audio_set_volume",
            "Set volume for a channel (music, sfx, ambient).",
            json!({
                "type": "object",
                "properties": {
                    "channel": {"type": "string"},
                    "volume": {"type": "number"}
                },
                "required": ["channel", "volume"]
            }),
        ),
        // ── Save/Load/Replay ──
        tool(
            "amigo_save",
            "Save the game state.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "slot": {"type": "string"}
                }
            }),
        ),
        tool(
            "amigo_load",
            "Load a saved game state.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "slot": {"type": "string"}
                }
            }),
        ),
        tool(
            "amigo_replay_record_start",
            "Start recording a replay.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_replay_record_stop",
            "Stop recording and save the replay.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        ),
        tool(
            "amigo_replay_play",
            "Play back a recorded replay.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "from_tick": {"type": "integer"}
                },
                "required": ["path"]
            }),
        ),
        // ── Debug ──
        tool(
            "amigo_debug_dump_state",
            "Dump full game state to a file.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        ),
        tool(
            "amigo_debug_tile_collision",
            "Check tile collision type at a position.",
            json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"}
                },
                "required": ["x", "y"]
            }),
        ),
        tool(
            "amigo_debug_step",
            "Advance simulation by exactly one tick.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_debug_state_crc",
            "Get CRC checksum of current state for desync detection.",
            json!({"type": "object", "properties": {}}),
        ),
        // ── Tilemap Query ──
        tool(
            "amigo_tilemap_get_tile",
            "Read tile ID and collision type at a position.",
            json!({
                "type": "object",
                "properties": {
                    "layer": {"type": "string", "description": "Tilemap layer: terrain, decoration, etc."},
                    "x": {"type": "integer"},
                    "y": {"type": "integer"}
                },
                "required": ["layer", "x", "y"]
            }),
        ),
        tool(
            "amigo_tilemap_get_region",
            "Read a rectangular region of tile IDs.",
            json!({
                "type": "object",
                "properties": {
                    "layer": {"type": "string", "description": "Tilemap layer: terrain, decoration, etc."},
                    "x": {"type": "integer"},
                    "y": {"type": "integer"},
                    "w": {"type": "integer"},
                    "h": {"type": "integer"}
                },
                "required": ["layer", "x", "y", "w", "h"]
            }),
        ),
        tool(
            "amigo_tilemap_collision_at",
            "Read the CollisionType at a tile position.",
            json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"},
                    "y": {"type": "integer"}
                },
                "required": ["x", "y"]
            }),
        ),
        tool(
            "amigo_tilemap_dimensions",
            "Get tilemap dimensions: width, height, tile_size.",
            json!({"type": "object", "properties": {}}),
        ),
        // ── Camera ──
        tool(
            "amigo_camera_get",
            "Get current camera state: position, zoom, mode, bounds.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_camera_set",
            "Set camera position and optional zoom level.",
            json!({
                "type": "object",
                "properties": {
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "zoom": {"type": "number", "description": "Optional zoom level"}
                },
                "required": ["x", "y"]
            }),
        ),
        tool(
            "amigo_camera_shake",
            "Trigger a camera shake effect.",
            json!({
                "type": "object",
                "properties": {
                    "intensity": {"type": "number"},
                    "duration": {"type": "number", "description": "Duration in seconds"}
                },
                "required": ["intensity", "duration"]
            }),
        ),
        tool(
            "amigo_camera_follow",
            "Set camera to follow a specific entity.",
            json!({
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer"}
                },
                "required": ["entity_id"]
            }),
        ),
        // ── Lighting ──
        tool(
            "amigo_lighting_add",
            "Add a light source at a position.",
            json!({
                "type": "object",
                "properties": {
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "radius": {"type": "number"},
                    "color": {"type": "array", "items": {"type": "integer"}, "description": "RGB color [r, g, b]"},
                    "intensity": {"type": "number"}
                },
                "required": ["x", "y", "radius", "color", "intensity"]
            }),
        ),
        tool(
            "amigo_lighting_remove",
            "Remove a light source by ID.",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer"}
                },
                "required": ["id"]
            }),
        ),
        tool(
            "amigo_lighting_list",
            "List all active light sources.",
            json!({"type": "object", "properties": {}}),
        ),
        // ── Particles ──
        tool(
            "amigo_particles_spawn",
            "Spawn a particle emitter at a position.",
            json!({
                "type": "object",
                "properties": {
                    "emitter_type": {"type": "string", "description": "Emitter type: fire, smoke, sparkle, blood, etc."},
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "params": {"type": "object", "description": "Optional emitter parameters (color, rate, lifetime, etc.)"}
                },
                "required": ["emitter_type", "x", "y"]
            }),
        ),
        tool(
            "amigo_particles_stop",
            "Stop a particle emitter by ID.",
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer"}
                },
                "required": ["id"]
            }),
        ),
        // ── Inventory/Crafting ──
        tool(
            "amigo_inventory_list",
            "List inventory contents for an entity.",
            json!({
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer", "description": "Entity to query, defaults to player"}
                }
            }),
        ),
        tool(
            "amigo_inventory_add",
            "Add items to an entity's inventory.",
            json!({
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer"},
                    "item": {"type": "string"},
                    "count": {"type": "integer"}
                },
                "required": ["entity_id", "item", "count"]
            }),
        ),
        tool(
            "amigo_inventory_remove",
            "Remove items from an entity's inventory.",
            json!({
                "type": "object",
                "properties": {
                    "entity_id": {"type": "integer"},
                    "item": {"type": "string"},
                    "count": {"type": "integer"}
                },
                "required": ["entity_id", "item", "count"]
            }),
        ),
        tool(
            "amigo_crafting_list_recipes",
            "List all available crafting recipes.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_crafting_craft",
            "Execute a crafting recipe.",
            json!({
                "type": "object",
                "properties": {
                    "recipe_id": {"type": "string"}
                },
                "required": ["recipe_id"]
            }),
        ),
        // ── Dialogue ──
        tool(
            "amigo_dialogue_start",
            "Start a dialogue tree.",
            json!({
                "type": "object",
                "properties": {
                    "tree_id": {"type": "string"}
                },
                "required": ["tree_id"]
            }),
        ),
        tool(
            "amigo_dialogue_choose",
            "Select a dialogue choice by index.",
            json!({
                "type": "object",
                "properties": {
                    "choice_index": {"type": "integer"}
                },
                "required": ["choice_index"]
            }),
        ),
        tool(
            "amigo_dialogue_get_state",
            "Get current dialogue node, speaker, text, choices, and flags.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_dialogue_set_flag",
            "Set a dialogue flag value.",
            json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "value": {"type": "integer"}
                },
                "required": ["name", "value"]
            }),
        ),
        // ── Asset Pipeline ──
        tool(
            "amigo_pack",
            "Run atlas packing on generated art assets. Blocks until packing completes and returns the updated manifest.",
            json!({
                "type": "object",
                "properties": {
                    "force": {"type": "boolean", "description": "Force re-pack even if no changes detected"}
                }
            }),
        ),
        // ── Dev workflow ──
        tool(
            "amigo_dev_save_snapshot",
            "Save a dev snapshot of the engine state before recompile.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_dev_snapshot_status",
            "Check if a dev snapshot exists and its metadata.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "amigo_dev_restore_snapshot",
            "Restore the engine state from the last dev snapshot.",
            json!({"type": "object", "properties": {}}),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_is_not_empty() {
        let tools = tool_definitions();
        assert!(!tools.is_empty());
    }

    #[test]
    fn all_tools_have_required_fields() {
        for tool in tool_definitions() {
            assert!(tool.get("name").is_some(), "Tool missing name");
            assert!(
                tool.get("description").is_some(),
                "Tool missing description"
            );
            assert!(
                tool.get("inputSchema").is_some(),
                "Tool missing inputSchema"
            );
        }
    }

    #[test]
    fn screenshot_tool_exists() {
        let found = tool_definitions()
            .iter()
            .any(|t| t["name"] == "amigo_screenshot");
        assert!(found);
    }
}
