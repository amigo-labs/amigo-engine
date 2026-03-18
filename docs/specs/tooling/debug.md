---
status: done
crate: amigo_debug
depends_on: ["engine/core"]
last_updated: 2026-03-16
---

# Debug & Profiling

## Purpose

Provides in-game debug overlays, visual debug tools, profiling integration, and dev mode features for rapid iteration during development.

## Public API

### Debug Toggle Keys

| Key | Function |
|-----|----------|
| F1 | In-Game Debug Overlay (toggle) |
| F2 | Grid |
| F3 | Collision boxes |
| F4 | Pathfinding |
| F5 | Spawn/build zones |
| F6 | Performance |
| F7 | Entity list |
| F8 | Network stats |

## Behavior

### In-Game Debug Overlay (Pixel UI)

Toggle with `F1`. Shows: FPS, entity count, draw calls, memory.

The overlay renders using the engine's own Pixel UI system, ensuring it is pixel-perfect and consistent with the game's visual style.

### Visual Debug (F-Keys)

Each F-key toggles a specific debug visualization layer:

- **F2: Grid** -- Shows the tile grid overlay on the game world
- **F3: Collision boxes** -- Renders AABB collision boxes for all entities
- **F4: Pathfinding** -- Visualizes active pathfinding routes, waypoints, and flow fields
- **F5: Spawn/build zones** -- Highlights valid spawn points and buildable areas
- **F6: Performance** -- Detailed performance metrics (frame time breakdown, draw call details)
- **F7: Entity list** -- Scrollable list of all active entities with their components
- **F8: Network stats** -- Latency, packet loss, command queue depth (multiplayer)

### Dev Mode Features

- **Hot reload** (assets + data) -- File watcher on assets directory. Sprites, configs, levels, audio, shaders all hot-reloadable.
- **State snapshot to file** -- Dump full game state for offline analysis
- **Tick stepping** (one tick at a time) -- Advance simulation frame by frame for precise debugging
- **Speed control** (0.5x, 1x, 2x, 4x) -- Slow down or fast-forward simulation

### Debug API (via AI Agent Interface)

Debug tools are also accessible through the JSON-RPC API for AI-driven debugging:

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

**MCP Tools (exposed to Claude Code):**

- `amigo_debug_dump_state(path)`
- `amigo_debug_tile_collision(x, y)`
- `amigo_debug_step()` -- advance one tick
- `amigo_debug_state_crc()` -- checksum for desync detection

### Profiling

Tracy integration from day 1. `tracing` crate with structured logging. Configurable via environment variable:

```bash
AMIGO_LOG=debug amigo run              # all debug and above
AMIGO_LOG=amigo_render=trace amigo run # only renderer trace
```

Integrates with Tracy for performance profiling. In-game debug overlay logs FPS, frame time, draw calls, entity count.

## Internal Design

The debug overlay renders on Layer 7 (highest Z-order), above all game content and UI. It uses the same sprite batcher and Pixel UI system as the rest of the engine.

Debug visualizations are rendered as transparent overlays on top of the game world using dedicated draw calls that are excluded from the sprite batcher's normal sorting.

Profiling spans are inserted via the `tracing` crate macros, which Tracy picks up for visualization in its external profiler UI.

## Non-Goals

- External debugger tool (all debugging is in-engine or via Tracy)
- Breakpoint-style debugging (use Rust's native debugger for that)
- Production telemetry or crash reporting

## Open Questions

- Whether to add a debug console (text input for commands) in addition to F-key toggles
- Memory profiling granularity (per-system vs per-component)
- Whether debug overlay should be available in release builds behind a secret key combination
