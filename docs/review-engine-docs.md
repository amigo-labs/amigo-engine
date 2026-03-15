# Review: Amigo Engine — Docs, Specs, Architecture & Splashscreen

## Context

Comprehensive review of the Amigo Engine covering: spec completeness, developer experience, AI integration, service communication, and licensing strategy. Result: improve documentation + add default splashscreen.

## Decisions
- **License:** MIT OR Apache-2.0 remains
- **Splashscreen:** Default "Powered by Amigo Engine", can be disabled via `EngineBuilder`

---

## 1. Spec Implementation Status

### Implemented (with actual code, ~34,000 LOC Rust)

| Crate | LOC | Status | Notes |
|-------|-----|--------|-------|
| `amigo_core` | 15,795 | **Solid** | ECS (SparseSet + Change Tracking), Fixed-Point Math, Pathfinding, Collision, Physics, Save System, Scheduler, Commands, 10+ Genre Modules (TD, Platformer, Roguelike, Fighting, Farming, Puzzle, Bullet-Hell...) |
| `amigo_render` | 3,695 | **Solid** | wgpu Renderer, Sprite Batcher, Camera, Particles, Lighting, Post-Processing, Atmosphere, Font Rendering |
| `amigo_animation` | 1,935 | **Solid** | Sprite Animation State Machine, Aseprite Integration |
| `amigo_net` | 2,169 | **Solid** | Protocol, Client/Server, Replay, Lobby, Stats, Checksum |
| `amigo_editor` | 4,829 | **Solid** | Tile Painter, Wave Editor, Collision Editor, Playtest, Heatmap, Visual Scripting, Auto-Path, Wizard |
| `amigo_input` | 931 | **OK** | Keyboard, Mouse, Gamepad, Action Maps |
| `amigo_audio` | 1,112 | **OK** | kira Wrapper, Audio Manager |
| `amigo_api` | 1,198 | **OK** | JSON-RPC 2.0 Server + Handler |
| `amigo_assets` | 647 | **OK** | Asset Manager, Hot Reload, Aseprite, Handles |
| `amigo_tilemap` | 762 | **OK** | TileLayer, Auto-Tiling |
| `amigo_scene` | 361 | **OK** | Scene Stack, Transitions |
| `amigo_ui` | 361 | **Minimal** | Immediate-Mode Pixel UI (basics present) |
| `amigo_debug` | 274 | **Minimal** | FPS Overlay, Toggle System |
| `amigo_engine` | 1,002 | **Solid** | Game Loop, EngineBuilder, Plugin System, GameContext, DrawContext |

### Tools (separate binaries)

| Tool | LOC | Status |
|------|-----|--------|
| `amigo_cli` | 949 | **OK** -- Project scaffolding, 10 templates |
| `amigo_mcp` | 902 | **OK** -- MCP bridge to amigo_api |
| `amigo_artgen` | 2,468 | **OK** -- ComfyUI integration, post-processing |
| `amigo_audiogen` | 1,762 | **OK** -- ACE-Step + AudioGen integration |

### Spec vs. Implementation -- Gaps

| Spec Feature | Status | Notes |
|-------------|--------|-------|
| Sections 10-27 of Spec-v2 | **Reference only** | The unified spec contained sections 10-27 only as references. Now completed. |
| Tracy Profiling | **Not connected** | `amigo_debug` has no Tracy integration, only FPS overlay |
| Spatial Hash / Flow Fields | **Not found** | Pathfinding yes, but Spatial Hash broad-phase and Flow Fields are missing from the code |
| egui Editor UI | **Not integrated** | Editor code exists, but egui rendering pipeline is missing from the engine loop |
| Asset Packing (`game.pak`) | **Not implemented** | CLI has scaffolding, but no `amigo pack` command |
| Headless Simulation | **Not implemented** | Spec describes headless tick-forward for AI, code is missing |
| Screenshot API | **Not implemented** | `amigo_screenshot` MCP tool described, but not built |
| Splashscreen | **Implemented** | Default "Powered by Amigo Engine", can be disabled |

---

## 2. Developer Experience -- Can a developer build a new game?

### What works well
- **Quick Start is clear:** `cargo install --path tools/amigo_cli && amigo new my_game && cargo run`
- **10 Genre Templates** in the CLI (platformer, topdown-rpg, roguelike, tower-defense, etc.)
- **Minimal Example** in README and lib.rs doc comment
- **Game Trait** is simple: `init()`, `update()`, `draw()` -- no macro magic
- **Starter Template** (`examples/starter/`) with player logic, asset loading, states
- **Prelude** exports all important types

### What is missing / improved
1. **Getting Started Guide** -- `docs/getting-started.md` (NEW)
2. **API Reference** -- generate and host via `cargo doc --workspace --no-deps`
3. **AI Integration Guide** -- `docs/ai-integration.md` (NEW)
4. **Architecture Diagram** -- `docs/architecture.md` (NEW)

---

## 3. AI Integration -- Are prerequisites documented?

### What is documented
- **Two-Layer Architecture:** `amigo_api` (JSON-RPC IPC) + `amigo_mcp` (MCP Bridge)
- **MCP Tools** fully specified: Screenshot, State, Entities, Perf, Simulation Control, Editor
- **Art Generation Pipeline:** ComfyUI integration with style definitions, post-processing
- **Audio Generation Pipeline:** ACE-Step + AudioGen with MCP tools
- **AI Integration Guide:** `docs/ai-integration.md` (NEW)

### What is still missing (implementation)
1. **Headless Mode** -- `amigo_tick(count)` for fast-forward
2. **Screenshot API** -- `amigo_screenshot()` MCP tool
3. **Hardware requirements for AI** -- GPU for ComfyUI, ACE-Step

---

## 4. Service Communication

See `docs/architecture.md` for the complete Mermaid diagram.

### Communication Protocols

| Connection | Protocol | Format | Direction |
|-----------|----------|--------|-----------|
| Claude Code <-> MCP Servers | MCP (stdio) | JSON-RPC 2.0 | Bidirectional |
| amigo_mcp <-> amigo_api | TCP Socket | JSON-RPC 2.0 | Bidirectional |
| amigo_artgen <-> ComfyUI | HTTP REST | JSON + Binary | Request/Response |
| amigo_audiogen <-> ACE-Step | HTTP REST | JSON + WAV | Request/Response |
| Engine <-> Assets | Filesystem | PNG/ASE/WAV/RON | Watch + Reload |
| Multiplayer Clients <-> Server | UDP (laminar) | Serialized Commands | Lockstep |

---

## 5. License & Splashscreen

### License: MIT OR Apache-2.0 (remains)

### Splashscreen: Default "Powered by Amigo Engine"

The splashscreen is built into the engine as **default behavior** -- not license-enforced, but active by default. Developers can disable it via `EngineBuilder::splash(false)`.

**Technical Implementation:**
- Module: `crates/amigo_engine/src/splash.rs`
- Shows "Powered by Amigo Engine" for 2 seconds at startup
- Text is rendered as pixel font (no external asset needed)
- `EngineBuilder::splash(bool)` to enable/disable
- `EngineConfig.splash.enabled` in `amigo.toml`
- Default: `true`
