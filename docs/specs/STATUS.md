# Spec Implementation Status

> **Legend:** ✅ Implemented | 🗓 Roadmap | ❓ Unclear | ⛔ Intentionally not done

Last updated: 2026-03-15

---

## Engine Spec ([01-engine-spec.md](01-engine-spec.md))

| # | Section | Status | Notes |
|---|---------|--------|-------|
| 1 | Vision & Philosophy | ✅ Implemented | Core principles reflected in codebase |
| 2 | Tech Stack | ✅ Implemented | All core deps in Cargo.toml |
| 3 | Architecture Overview | ✅ Implemented | 14 crates + 4 tools |
| 4 | Core Types, Math & ECS | ✅ Implemented | SparseSet, Fixed-Point, Change Tracking |
| 5 | Rendering Pipeline | ✅ Implemented | 7-stage batcher (amigo_render, 3695 LOC) |
| 6 | Memory & Performance | ✅ Implemented | Pooling, arena, budgets |
| 7 | API Design | ✅ Implemented | Game trait, EngineBuilder, GameContext, DrawContext |
| 8 | Command System & Networking | ✅ Implemented | amigo_net (2169 LOC), lockstep, replay |
| 9 | Asset Pipeline | 🗓 Roadmap | Hot reload works; `.pak` packing not implemented (`amigo pack` missing) |
| 10 | Tilemap System | ✅ Implemented | amigo_tilemap (762 LOC), auto-tiling |
| 11 | Pathfinding | ✅ Implemented | A*, flow fields (Dijkstra), waypoints |
| 12 | Animation System | ✅ Implemented | Aseprite integration (amigo_animation, 1935 LOC) |
| 13 | Camera System | ✅ Implemented | Follow, shake, clamp |
| 14 | Input System | ✅ Implemented | Keyboard, mouse, gamepad, action maps (amigo_input, 931 LOC) |
| 15 | Audio System | ✅ Implemented | kira wrapper (amigo_audio, 1112 LOC) |
| 16 | Level Editor | ✅ Implemented | Tile painter, wave editor, collision editor, playtest, heatmap, visual scripting (amigo_editor, 4829 LOC) |
| 17 | AI Agent Interface | ✅ Implemented | amigo_api JSON-RPC (1198 LOC) + amigo_mcp bridge (902 LOC) |
| 18 | Debug & Profiling | ✅ Implemented | FPS overlay, Tracy integration (feature-gated), F-keys |
| 19 | Build & Distribution | 🗓 Roadmap | CLI scaffolding done (949 LOC); `amigo pack` command missing |
| 20 | Plugin System | ✅ Implemented | Plugin trait, events, resources |
| 21 | UI System | ❓ Unclear | amigo_ui minimal (361 LOC); egui not integrated in game loop |
| 22 | Error Handling & Logging | ✅ Implemented | thiserror + tracing |
| 23 | Configuration | ✅ Implemented | amigo.toml, RON |
| 24 | Starter Template | ✅ Implemented | 10 templates via CLI |
| 25 | Game-Specific Design | ⛔ Not done | Lives in game repo, not engine — by design |
| 26 | Implementation Phases | ✅ Implemented | Phase 0–1 complete |
| 27 | Key Decisions | ✅ Implemented | All resolved |

### Open gaps (Engine)

| Gap | Status | Notes |
|-----|--------|-------|
| egui Editor UI | 🗓 Roadmap | Editor code exists, but egui rendering pipeline not in engine loop |
| Headless Simulation | 🗓 Roadmap | Spec describes `amigo_tick(count)` for AI fast-forward — not implemented |
| Screenshot API | 🗓 Roadmap | `amigo_screenshot()` MCP tool described but not built |
| Splashscreen | ✅ Implemented | Default "Powered by Amigo Engine", disableable via `EngineBuilder::splash(false)` |

---

## Asset Pipeline Spec ([02-asset-pipeline-spec.md](02-asset-pipeline-spec.md))

| # | Section | Status | Notes |
|---|---------|--------|-------|
| 1 | Overview | ✅ Implemented | amigo_artgen (2468 LOC) + amigo_audiogen (1762 LOC) |
| 2 | Art Architecture | ✅ Implemented | ComfyUI workflow builder + HTTP client + post-processing |
| 3 | Connection | ✅ Implemented | Local/Remote/Cloud modes |
| 4 | Style Definitions | ✅ Implemented | Per-world style TOML files |
| 5 | Sprite Generation | ✅ Implemented | MCP tools for sprite/tileset/animation generation |
| 6 | Tileset Generation | ✅ Implemented | Auto-tiling compatible output |
| 7 | Post-Processing | ✅ Implemented | Color quantization, outline, palette enforcement |
| 8 | Audio Architecture | ✅ Implemented | ACE-Step + AudioGen integration |
| 9 | Music Generation | ✅ Implemented | ACE-Step for music tracks and stems |
| 10 | SFX Generation | ✅ Implemented | AudioGen for sound effects |
| 11 | Audio MCP Tools | ✅ Implemented | audiogen MCP server |
| 12 | Adaptive Music | 🗓 Roadmap | Spec only — not wired into engine loop |

---

## Asset Format Spec ([03-asset-format-spec.md](03-asset-format-spec.md))

| # | Section | Status | Notes |
|---|---------|--------|-------|
| 1 | Overview | 🗓 Roadmap | v0.2.0-draft — design phase |
| 2 | Project Structure | 🗓 Roadmap | Convention defined, not enforced by tooling |
| 3 | Sprite Format | 🗓 Roadmap | TOML descriptor spec complete, tooling not built |
| 4 | Tileset Format | 🗓 Roadmap | Spec complete, tooling not built |
| 5 | Tilemap Format | 🗓 Roadmap | Spec complete, tooling not built |
| 6 | Entity Format | 🗓 Roadmap | Spec complete, tooling not built |
| 7 | Palette Format | 🗓 Roadmap | Spec complete, tooling not built |
| 8 | Audio Format | 🗓 Roadmap | Pattern language spec complete, runtime not built |
| 9 | Build System | 🗓 Roadmap | `amigo build` not implemented |
| 10 | Import Pipeline | 🗓 Roadmap | Aseprite import works; Tiled/LDTK/ROM import not built |
| 11 | Export Pipeline | 🗓 Roadmap | Not implemented |
| 12 | Runtime Formats | 🗓 Roadmap | .ait (indexed tile) and .pak not implemented |
| 13 | Migration & Versioning | 🗓 Roadmap | Spec only |
| 14 | CLI Integration | 🗓 Roadmap | `amigo asset` subcommands not implemented |
