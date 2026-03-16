# AI Agent Interface (amigo_api + amigo_mcp)

> Status: draft
> Crate: amigo_api, amigo_mcp
> Depends on: [engine/core](../engine/core.md)
> Last updated: 2026-03-16

## Zweck

Amigo is designed to be developed *with* AI agents, not just *by* humans. The AI API provides a persistent IPC interface that allows Claude Code (or any AI agent) to observe, control, and debug the running engine as a first-class development workflow.

## Public API

See [MCP Tools](#mcp-tools-exposed-to-claude-code) for the full tool interface and [Command Categories](#command-categories) for the underlying JSON-RPC protocol.

## Verhalten

See [Example Workflows](#example-claude-code-workflow-via-mcp) for concrete usage patterns including level building, balancing, and debugging.

## Internes Design

See [Architecture (Two Layers)](#architecture-two-layers) for the separation between amigo_api (JSON-RPC) and amigo_mcp (MCP wrapper).

## Nicht-Ziele

- Art/audio asset generation (see [artgen](artgen.md) and [audiogen](audiogen.md))
- Remote access over the internet (localhost only by default)
- Replacing manual development workflows

## Offene Fragen

- Authentication for non-localhost access
- Rate limiting for event streaming
- MCP protocol version compatibility guarantees

---

## Architecture (Two Layers)

```
+-----------------------------------------------------------+
|                 Claude Code (Terminal)                      |
|                                                            |
|  Uses MCP tools natively:                                  |
|    amigo_screenshot(path, overlays)                        |
|    amigo_place_tower(x, y, type)                           |
|    amigo_editor_paint_tile(layer, x, y, tile)              |
|    amigo_get_state()                                       |
|                                                            |
+------------------------------------------------------------+
|               MCP Protocol (stdio)                          |
+------------------------------------------------------------+
|                                                            |
|            amigo_mcp (MCP Server Binary)                    |
|                                                            |
|  - Separate process, started by Claude Code                |
|  - Translates MCP tool calls -> JSON-RPC                   |
|  - Translates JSON-RPC responses -> MCP results            |
|  - Forwards screenshots as MCP image resources             |
|  - Streams engine events as MCP notifications              |
|                                                            |
+------------------------------------------------------------+
|          JSON-RPC over Unix Socket / TCP                    |
+------------------------------------------------------------+
|                                                            |
|               Amigo Engine Process                          |
|                                                            |
|  +------------------------------------------------------+  |
|  |              amigo_api (IPC Server)                   |  |
|  |                                                       |  |
|  |  - Listens on socket/port                             |  |
|  |  - Receives JSON-RPC commands                         |  |
|  |  - Validates and queues as engine commands             |  |
|  |  - Returns results / state / screenshots              |  |
|  |  - Streams events (enemy killed, wave done)           |  |
|  +------------------------------------------------------+  |
|                                                            |
+------------------------------------------------------------+
```

**Why two layers?**

`amigo_api` is the engine's raw IPC interface -- JSON-RPC over socket. Any tool can use it: scripts, other editors, testing frameworks, CI/CD pipelines.

`amigo_mcp` is a thin wrapper that speaks MCP protocol on stdio and translates to/from JSON-RPC. This is what Claude Code connects to. Separation means: the engine doesn't need to know about MCP, and MCP updates don't require engine changes.

## MCP Server Configuration

Claude Code auto-discovers the MCP servers via config:

```json
// ~/.claude/claude_code_config.json
{
  "mcpServers": {
    "amigo": {
      "command": "amigo",
      "args": ["mcp-server", "--port", "9999"]
    },
    "amigo-artgen": {
      "command": "amigo-artgen",
      "args": ["--server", "http://localhost:8188"]
    },
    "amigo-audiogen": {
      "command": "amigo-audiogen",
      "args": ["--acestep", "http://localhost:7860"]
    }
  }
}
```

Three MCP servers side by side: `amigo` for engine control, `amigo-artgen` for art (see [artgen](artgen.md)), `amigo-audiogen` for audio (see [audiogen](audiogen.md)).

## MCP Tools (exposed to Claude Code)

Claude Code sees these as native tools it can call directly:

**Observation:**
- `amigo_screenshot(path, overlays?, area?)` -- captures frame, returns image
- `amigo_get_state()` -- returns tick, gold, lives, wave, entity counts
- `amigo_list_entities(filter?, near?, radius?)` -- entity list with details
- `amigo_inspect_entity(id)` -- full entity component dump
- `amigo_perf()` -- FPS, frame time, draw calls, entity count

**Simulation:**
- `amigo_place_tower(x, y, tower_type)` -- entity_id, gold_remaining
- `amigo_sell_tower(tower_id)`
- `amigo_upgrade_tower(tower_id, path)`
- `amigo_start_wave()`
- `amigo_tick(count)` -- state after N ticks (headless fast-forward)
- `amigo_set_speed(multiplier)`
- `amigo_pause()` / `amigo_unpause()`
- `amigo_spawn(type, subtype, pos)` -- debug entity spawning

**Editor:**
- `amigo_editor_new_level(world, width, height)`
- `amigo_editor_paint_tile(layer, x, y, tile)`
- `amigo_editor_fill_rect(layer, x, y, w, h, tile)`
- `amigo_editor_place_entity(type, x, y)`
- `amigo_editor_add_path(points)`
- `amigo_editor_move_path_point(path, point, new_pos)`
- `amigo_editor_auto_decorate(world)`
- `amigo_editor_save(path)` / `amigo_editor_load(path)`
- `amigo_editor_undo()` / `amigo_editor_redo()`

**Audio:**
- `amigo_audio_play(name)` / `amigo_audio_play_music(name)`
- `amigo_audio_crossfade(name, duration)`
- `amigo_audio_set_volume(channel, volume)`

**Save/Load/Replay:**
- `amigo_save(slot)` / `amigo_load(slot)`
- `amigo_replay_record_start()` / `amigo_replay_record_stop(path)`
- `amigo_replay_play(path, from_tick?)`

**Debug:**
- `amigo_debug_dump_state(path)`
- `amigo_debug_tile_collision(x, y)`
- `amigo_debug_step()` -- advance one tick
- `amigo_debug_state_crc()` -- checksum for desync detection

## Engine Startup (amigo_api)

```bash
# Start engine with API server enabled
amigo run --api                          # windowed + API on default socket
amigo run --api --port 9999              # windowed + API on TCP port
amigo run --api --headless               # no window, max speed simulation
amigo run --api --headless --level dune_01  # headless with specific level
```

## Underlying Protocol: JSON-RPC 2.0 over Unix Socket or TCP

The MCP tools above map 1:1 to JSON-RPC commands below. The MCP server (`amigo_mcp`) translates between them. Direct JSON-RPC access is available for non-MCP clients (scripts, CI/CD, custom tools).

Request/response pattern with optional event streaming.

## Command Categories

### Screenshots & Observation

```jsonc
// Capture current frame
{"method": "screenshot", "params": {"path": "/tmp/frame.png"}}
// -> {"result": {"path": "/tmp/frame.png", "width": 480, "height": 270}}

// Capture with overlays
{"method": "screenshot", "params": {
    "path": "/tmp/debug.png",
    "overlays": ["collision", "grid", "paths", "tower_ranges", "entity_ids"]
}}

// Capture enemy density heatmap
{"method": "screenshot", "params": {
    "path": "/tmp/heatmap.png",
    "mode": "heatmap",
    "heatmap_type": "enemy_deaths"
}}

// Get current game state as structured data
{"method": "get_state"}
// -> {"result": {"tick": 4200, "gold": 350, "lives": 17, "wave": 5, ...}}

// List entities with optional filter
{"method": "list_entities", "params": {"filter": "enemy"}}
// -> {"result": [{"id": 42, "type": "orc", "pos": [12.5, 8.3], "health": 75}, ...]}

// Inspect single entity
{"method": "inspect_entity", "params": {"id": 42}}
// -> {"result": {"id": 42, "type": "orc", "pos": [12.5, 8.3], "health": 75, "speed": 2.0, "state": "walking", "path_progress": 0.45}}

// Performance metrics
{"method": "perf"}
// -> {"result": {"fps": 60, "frame_time_ms": 2.3, "entities": 247, "draw_calls": 8}}
```

### Simulation Control

```jsonc
// Place a tower
{"method": "place_tower", "params": {"x": 5, "y": 3, "tower_type": "archer"}}
// -> {"result": {"entity_id": 42, "gold_remaining": 450}}

// Sell a tower
{"method": "sell_tower", "params": {"tower_id": 42}}

// Upgrade a tower
{"method": "upgrade_tower", "params": {"tower_id": 42, "path": "damage"}}

// Start next wave
{"method": "start_wave"}

// Advance simulation by N ticks (useful for headless mode)
{"method": "tick", "params": {"count": 60}}
// -> {"result": {"tick": 4260, "events": [...]}}

// Set simulation speed
{"method": "set_speed", "params": {"multiplier": 4.0}}

// Pause / Unpause
{"method": "pause"}
{"method": "unpause"}

// Spawn entity (debug/testing)
{"method": "spawn", "params": {"type": "enemy", "subtype": "orc", "pos": [0, 10]}}
```

### Level Editor (Remote)

```jsonc
// Create new level
{"method": "editor.new_level", "params": {"world": "caribbean", "width": 30, "height": 20}}

// Paint tiles
{"method": "editor.paint_tile", "params": {"layer": "terrain", "x": 5, "y": 3, "tile": 42}}

// Fill rectangle
{"method": "editor.fill_rect", "params": {"layer": "terrain", "x": 0, "y": 0, "w": 10, "h": 5, "tile": 1}}

// Place entity marker
{"method": "editor.place_entity", "params": {"type": "spawn_point", "x": 0, "y": 10}}

// Define path
{"method": "editor.add_path", "params": {"points": [[0,10], [5,10], [5,5], [15,5], [15,15], [29,15]]}}

// Modify path point
{"method": "editor.move_path_point", "params": {"path": 0, "point": 2, "new_pos": [7, 7]}}

// Auto-decorate (fill non-gameplay tiles with themed decoration)
{"method": "editor.auto_decorate", "params": {"world": "caribbean"}}

// Save level
{"method": "editor.save", "params": {"path": "levels/caribbean/level_02.amigo"}}

// Load level
{"method": "editor.load", "params": {"path": "levels/caribbean/level_01.amigo"}}

// Undo / Redo
{"method": "editor.undo"}
{"method": "editor.redo"}
```

### Audio Control

```jsonc
// Play sound effect
{"method": "audio.play", "params": {"name": "sfx/cannon_fire"}}

// Play music
{"method": "audio.play_music", "params": {"name": "music/caribbean_theme"}}

// Crossfade to different music
{"method": "audio.crossfade", "params": {"name": "music/dune_theme", "duration": 2.0}}

// Set volume
{"method": "audio.set_volume", "params": {"channel": "music", "volume": 0.5}}

// List all audio assets
{"method": "audio.list"}
```

### Save / Load / Replay

```jsonc
// Save game state
{"method": "save", "params": {"path": "saves/my_save.ron"}}

// Load game state
{"method": "load", "params": {"path": "saves/my_save.ron"}}

// Start recording replay
{"method": "replay.record_start"}

// Stop recording and save
{"method": "replay.record_stop", "params": {"path": "replays/test_run.ron"}}

// Play replay
{"method": "replay.play", "params": {"path": "replays/test_run.ron"}}

// Play replay from specific tick
{"method": "replay.play", "params": {"path": "replays/test_run.ron", "from_tick": 500}}
```

### Debug

```jsonc
// Dump full game state to file
{"method": "debug.dump_state", "params": {"path": "/tmp/state.ron"}}

// Check tile collision at position
{"method": "debug.tile_collision", "params": {"x": 7, "y": 4}}
// -> {"result": {"type": "Solid"}}

// Step simulation one tick at a time
{"method": "debug.step"}

// Get CRC of current state (for desync detection)
{"method": "debug.state_crc"}
// -> {"result": {"tick": 4200, "crc": "a3f7b2c1"}}
```

## Event Streaming

The API server can stream events to the connected client:

```jsonc
// Subscribe to events
{"method": "subscribe", "params": {"events": ["enemy_killed", "wave_complete", "tower_fired", "game_over"]}}

// Events streamed as notifications:
{"event": "enemy_killed", "data": {"id": 17, "type": "orc", "pos": [12, 8], "bounty": 25, "killed_by": 42}}
{"event": "wave_complete", "data": {"wave": 3, "enemies_killed": 45, "lives_remaining": 18}}
{"event": "tower_fired", "data": {"tower_id": 42, "target_id": 17, "projectile_id": 89}}
{"event": "game_over", "data": {"result": "victory", "waves_cleared": 10, "score": 12500}}
```

## Headless Mode

For AI playtesting at maximum speed (no rendering, no window):

```bash
amigo run --api --headless --level caribbean_01
```

In headless mode, simulation runs as fast as the CPU allows. A 3-minute game can be simulated in <1 second. Claude Code can run hundreds of variations to find optimal balancing.

## Example: Claude Code Workflow (via MCP)

**Building a level:**
```
Claude Code calls MCP tools natively:

1. amigo_editor_new_level(world="dune", width=40, height=25)
2. amigo_editor_fill_rect(layer="terrain", x=0, y=0, w=40, h=25, tile="sand")
3. amigo_editor_add_path(points=[[0,12],[10,12],[10,5],[20,5],[20,20],[39,20]])
4. amigo_editor_auto_decorate(world="dune")
5. amigo_screenshot(path="/tmp/level_draft.png", overlays=["paths","grid"])
   -> Claude SEES the image, analyzes layout
6. "Path needs more curves"
7. amigo_editor_move_path_point(path=0, point=2, new_pos=[12,7])
8. amigo_screenshot(path="/tmp/level_v2.png")
   -> "Better. Now testing playability..."
9. amigo_editor_save(path="levels/dune/level_02.amigo")
```

**Balancing:**
```
1. amigo_editor_load(path="levels/dune/level_02.amigo")
2. amigo_tick(count=10800)  // simulate 3 minutes at max speed
3. amigo_get_state()
   -> "Too easy, 19/20 lives remaining"
4. Claude edits waves/dune_02.ron (adds more enemies to wave 4)
5. amigo_load(slot=0)  // reload level
6. amigo_tick(count=10800)
7. amigo_get_state()
   -> "Better, 12/20 lives"
8. Repeat with different tower placements...
```

**Debugging:**
```
1. User reports: "Enemies get stuck at tile 7,4"
2. amigo_load(slot="buggy")
3. amigo_list_entities(filter="enemy", near=[7,4], radius=2)
   -> Entity 23: pos=(7.02, 4.00), vel=(0,0), state=STUCK
4. amigo_debug_tile_collision(x=7, y=4)
   -> "Solid" (should be Empty!)
5. amigo_screenshot(path="/tmp/bug.png", overlays=["collision"], area=[5,2,10,7])
   -> Claude sees the collision overlay, identifies the misplaced tile
6. Claude fixes the code, verifies with another screenshot
```

## Security

The API server only binds to localhost by default. No remote access unless explicitly configured. In headless mode, no window exists so no visual information leaks. The API validates all commands before executing.
