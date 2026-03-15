# Spec Implementation Status

> **Legend:** ✅ Implemented | 🔧 Partial | 🗓 Roadmap | ⛔ Intentionally not done

Last updated: 2026-03-15 (full audit against source code)

**Codebase summary:** 39,713 LOC across 14 crates, 6,517 LOC across 4 tools, 8 examples.

---

## Engine Spec ([01-engine-spec.md](01-engine-spec.md))

| # | Section | Status | Notes |
|---|---------|--------|-------|
| 1 | Vision & Philosophy | ✅ Implemented | Core principles reflected in codebase |
| 2 | Tech Stack | ✅ Implemented | All core deps in Cargo.toml |
| 3 | Architecture Overview | ✅ Implemented | 14 crates + 4 tools; spec lists `games/amigo_td/` but dir doesn't exist (game lives in separate repo) |
| 4 | Core Types, Math & ECS | ✅ Implemented | SparseSet, Fixed-Point, Change Tracking, BitSet (amigo_core, 18,360 LOC) |
| 5 | Rendering Pipeline | 🔧 Partial | Sprite batcher, atlas, camera, particles, lighting, post-processing (bloom, CRT, vignette, color grading, chromatic aberration), atmosphere (amigo_render, 4,223 LOC). **Missing:** 7-stage render pipeline (single pass instead), per-sprite shaders (flash, outline, dissolve, palette_swap, silhouette, wave) |
| 6 | Memory & Performance | 🔧 Partial | Pooling, scheduler, spatial hash. **Missing:** bumpalo arena allocator (no dependency) |
| 7 | API Design | ✅ Implemented | Game trait, EngineBuilder, GameContext, DrawContext, splash screen (amigo_engine, 1,720 LOC) |
| 8 | Command System & Networking | 🔧 Partial | Transport trait, LocalTransport, replay system with seek, CRC-32 checksum + DesyncDetector (amigo_net, 2,208 LOC). **Missing:** No laminar UDP transport — only LocalTransport. Network multiplayer is stub-level |
| 9 | Asset Pipeline | ✅ Implemented | Hot reload, handle system, `.pak` packing via `amigo pack` (amigo_assets, 1,030 LOC) |
| 10 | Tilemap System | ✅ Implemented | Auto-tiling, chunk caching, properties (amigo_tilemap, 774 LOC) |
| 11 | Pathfinding | ✅ Implemented | A*, flow fields (Dijkstra), waypoints (amigo_core/pathfinding.rs) |
| 12 | Animation System | ✅ Implemented | Aseprite integration, state machine (amigo_animation, 1,903 LOC) |
| 13 | Camera System | ✅ Implemented | Fixed, Follow (deadzone+lookahead+smoothing), FollowSmooth, ScreenLock, RoomTransition, BossArena, CinematicPan, EdgePan, FreePan + shake, zoom, bounds clamping, pixel_snap (amigo_render/camera.rs, 555 LOC) |
| 14 | Input System | ✅ Implemented | Keyboard, mouse, gamepad, action maps (amigo_input, 944 LOC) |
| 15 | Audio System | ✅ Implemented | kira wrapper, SFX with variants, AdaptiveMusicEngine (BarClock, LayerRule, MusicTransition, Stingers), volume channels (amigo_audio, 1,126 LOC) |
| 16 | Level Editor | ✅ Implemented | Tile painter, wave editor, collision editor, playtest, heatmap, visual scripting, auto-path, egui UI, wizard (amigo_editor, 5,103 LOC) |
| 17 | AI Agent Interface | ✅ Implemented | JSON-RPC server with 40+ methods, MCP bridge, screenshot, tick, headless (amigo_api, 1,291 LOC + amigo_mcp, 902 LOC) |
| 18 | Debug & Profiling | ✅ Implemented | FPS overlay, Tracy integration (feature-gated), F-keys (amigo_debug, 306 LOC) |
| 19 | Build & Distribution | ✅ Implemented | `amigo pack`, `amigo build`, `amigo release`, `amigo publish steam/itch`, `amigo export-level` (amigo_cli, 1,151 LOC) |
| 20 | Plugin System | ✅ Implemented | Plugin trait, feature flags |
| 21 | UI System | 🔧 Partial | Tier 1 Game HUD implemented (amigo_ui, 352 LOC): text, rect, sprite, progress bar. Tier 2 Editor Widgets use egui (amigo_editor/egui_ui.rs), not Pixel UI. Spec describes both tiers as Pixel UI — reality: editor uses egui overlay at native res |
| 22 | Error Handling & Logging | ✅ Implemented | thiserror + tracing |
| 23 | Configuration | ✅ Implemented | amigo.toml, RON, CLI flag overrides |
| 24 | Starter Template | ✅ Implemented | 10 project templates + 13 scene presets via CLI |
| 25 | Game-Specific Design | ⛔ Not done | Lives in game repo, not engine — by design |
| 26 | Implementation Phases | ✅ Implemented | Phase 0–5 largely complete |
| 27 | Key Decisions | ✅ Implemented | All resolved |

### Previously reported gaps — now resolved

| Gap | Previous Status | Current Status | Notes |
|-----|----------------|----------------|-------|
| egui Editor UI | 🗓 Roadmap | ✅ Implemented | `amigo_render/egui_integration.rs` + `amigo_editor/egui_ui.rs` — egui runs behind `editor` feature flag, renders at native res on top of sprite pipeline |
| Headless Simulation | 🗓 Roadmap | ✅ Implemented | `amigo_engine/engine.rs:run_headless()` — full headless loop, API-driven, tick-on-demand |
| Screenshot API | 🗓 Roadmap | ✅ Implemented | `amigo_api/handler.rs` — `screenshot` + `screenshot.results` JSON-RPC methods with overlay support |
| `amigo pack` | 🗓 Roadmap | ✅ Implemented | `amigo_cli/main.rs:cmd_pack()` + `amigo_assets/pak.rs` — PakWriter with sprite atlas packing |
| Splashscreen | ✅ Implemented | ✅ Implemented | `amigo_engine/splash.rs`, disableable via `EngineBuilder::splash(false)` |

### Remaining gaps (Engine)

| Gap | Status | Notes |
|-----|--------|-------|
| Tier 2 Pixel UI widgets | 🗓 Roadmap | Spec describes text_input, slider, dropdown, color_picker, scrollable_list, tree_view in Pixel UI style. Reality: editor uses egui for these. No Pixel UI tier 2 exists |
| ~~Adaptive Music Engine (runtime)~~ | ✅ Implemented | `AdaptiveMusicEngine`, `BarClock`, `MusicSection`, `LayerRule`, `MusicTransition`, `Stinger` with quantization — all in amigo_audio/lib.rs |
| Per-sprite shaders | 🗓 Roadmap | Spec §5 describes flash, outline, dissolve, palette_swap, silhouette, wave shaders. Not implemented — single render pass only |
| 7-stage render pipeline | 🗓 Roadmap | Spec §5/A.6 describes Background → Tilemap → Entities → Particles → Lighting → PostProcess → UI stages. Code has single pass + post-processing |
| Bumpalo arena allocator | 🗓 Roadmap | Spec §6 lists bumpalo for per-frame temp data. No bumpalo dependency in codebase |
| UDP/laminar transport | 🗓 Roadmap | Spec §8 describes laminar UDP for multiplayer. Only LocalTransport exists. No laminar dependency |
| Isometric tilemap mode | 🔧 Partial | `GridMode::Isometric` enum variant exists in amigo_tilemap but rendering/conversion not tested |
| Chunk streaming tilemap | 🗓 Roadmap | Spec §10 describes `ChunkedTilemap` with load/unload by camera position. Not implemented |
| Skeleton animation (Phase 2) | 🗓 Roadmap | Spec §12 mentions skeletal animation for large bosses — not implemented |
| Spatial SFX | 🗓 Roadmap | Spec §15.1 mentions position-based volume falloff — not in amigo_audio |
| MusicTransition: StingerThen, LayerSwap | 🗓 Roadmap | 3 of 5 transition types implemented (CrossfadeOnBar, FadeOutThenPlay, CutOnBar). StingerThen and LayerSwap missing |
| Plugin::update() | 🗓 Roadmap | Spec shows Plugin with update() method. Code only has build() and init() |
| Debug F5-F8 keys | 🗓 Roadmap | F1-F4 implemented. F5 (entity_ids), F6 (tile_ids), F7 (audio_debug), F8 (network_debug) missing |
| Event streaming (WebSocket) | 🗓 Roadmap | `subscribe`/`poll_events` methods exist in API but events are polled, not streamed |
| ECS query API mismatch | ⚠️ Note | Spec shows `world.query::<T>()`. Code uses `join()`, `join2()`, `join3()`, `join4()` free functions with `.with()` filter |
| `games/amigo_td/` directory | ⛔ Not applicable | Spec §3 references this path but game lives in a separate repository |

### Features in code but NOT in spec

| Feature | Location | Notes |
|---------|----------|-------|
| `amigo scene` CLI command | amigo_cli | Add scenes to projects with 13 presets — not documented in spec |
| `amigo publish steam/itch` | amigo_cli | Steam (steamcmd) and itch.io (butler) upload — not documented in spec |
| `amigo export-level` | amigo_cli | Export .amigo levels to JSON — not documented in spec |
| `amigo editor` CLI command | amigo_cli | Launch editor from CLI — not documented in spec |
| `amigo info` CLI command | amigo_cli | Show project info — not documented in spec |
| 18 genre modules | amigo_core | platformer, roguelike, fighting, farming, bullet_pattern, puzzle, combat, loot, inventory, turn_combat, dialog, crafting, procgen, economy, ai, navigation, status_effect, projectile — spec §24 only covers 4 |
| TD systems | amigo_core | td_systems.rs, tower.rs, waves.rs, enemy.rs — TD-specific systems not detailed in engine spec |
| Game presets | amigo_core/game_preset.rs | Scene presets and project templates — not in spec |
| Level loader | amigo_core/level_loader.rs | Level loading system — not in spec |
| Save system | amigo_core/save.rs | Save/load with slot management — described briefly in spec A.7, but actual implementation not documented |
| Collision events | amigo_core/collision_events.rs | Typed collision events — not in spec |
| Wizard UI | amigo_editor/wizard.rs | Project creation wizard — not in spec |
| Atmosphere system | amigo_render/atmosphere.rs | Mood/atmosphere interpolation — mentioned in tricks-patterns.md but not in engine spec |
| Camera EdgePan + FreePan | amigo_render/camera.rs | Two extra camera modes not in spec |
| Particle ForceFields | amigo_render/particles.rs | Wind, Attractor, Repulsor, Vortex, Drag, Turbulence — not in spec |
| Chromatic Aberration post-effect | amigo_render/post_process.rs | Exists but not in spec (spec lists bloom, color grade, vignette, CRT, rain) |
| Font rendering | amigo_render/font.rs | Font system via fontdue — not detailed in spec |
| egui integration | amigo_render/egui_integration.rs | Full egui-wgpu rendering layer — not in spec (spec says "no egui") |
| Distribution config | amigo_cli | Steam app ID, depot, branch + itch.io game/channel config — not in spec |

---

## Asset Pipeline Spec ([02-asset-pipeline-spec.md](02-asset-pipeline-spec.md))

| # | Section | Status | Notes |
|---|---------|--------|-------|
| 1 | Overview | ✅ Implemented | amigo_artgen (2,608 LOC) + amigo_audiogen (1,856 LOC) |
| 2 | Art Architecture | 🔧 Partial | Workflow builder builds ComfyUI node graphs programmatically. HTTP client methods are **stubs** (no actual ComfyUI calls). Post-processing fully implemented |
| 3 | Connection | 🔧 Partial | Local mode scaffolded (127.0.0.1:8188). Remote/Cloud modes not functional (no HTTP client) |
| 4 | Style Definitions | 🔧 Partial | `StyleDef` struct with all spec'd fields. 6 builtin world styles hardcoded. RON file loading scaffolded but **no `.style.ron` files exist on disk** |
| 5 | Art MCP Tools | 🔧 Partial | All 12 tools registered (generate_sprite, generate_tileset, generate_spritesheet, variation, inpaint, palette_swap, upscale, post_process, list_styles, list_checkpoints, list_loras, server_status). All **return placeholder data** — no actual generation |
| 6 | Post-Processing Pipeline | ✅ Implemented | palette_clamp, remove_aa, add_outline (inner/outer/both), cleanup_transparency, downscale, tile_edge_check, force_dimensions — all in Rust (postprocess.rs) |
| 7 | Workflow Builder | 🔧 Partial | Programmatic workflow building for txt2img, img2img, inpaint, upscale. JSON template files exist on disk but are **not loaded by code**. No `custom/` directory |
| 8 | Audio Architecture | 🔧 Partial | ACE-Step + AudioGen client structs exist. HTTP calls are **stubs** (no actual AI generation). Prompt building fully implemented |
| 9 | AI Models | 🔧 Partial | Client code scaffolded for ACE-Step 1.5 + AudioGen. No actual model communication |
| 10 | Stem Strategy | 🔧 Partial | Quick Mode scaffolded (generate + Demucs split). **Clean Mode missing** (no per-stem conditioned generation, no generate_core_melody, no generate_stem) |
| 11 | Audio MCP Tools | 🔧 Partial | 6 of 19 spec'd tools: generate_music, generate_sfx, split_stems, process, list_styles, server_status. **Missing 13 tools:** generate_core_melody, generate_stem, generate_variation, extend_track, remix, generate_ambient, loop_trim, normalize, convert, preview (+ name mismatches: spec `generate_track` → code `generate_music`) |
| 12 | Adaptive Music System (engine-side) | ✅ Implemented | Full system in amigo_audio: AdaptiveMusicEngine, BarClock, LayerRule (Lerp/Threshold/Toggle), MusicTransition (CrossfadeOnBar/FadeOutThenPlay/CutOnBar/StingerThen/LayerSwap), Stinger with beat/bar quantization |
| 13 | Sound Effects Pipeline | ✅ Implemented | SFX categories (8 types), variation system, pitch variance (amigo_audio) |
| 14 | World Audio Styles | 🔧 Partial | 6 builtin world audio styles hardcoded. No `.audio_style.ron` files on disk |
| 15 | Audio Post-Processing | 🔧 Partial | `apply_loop_crossfade()`, normalization logic exist. Full pipeline (BPM detection, bar snap, spectral validation) not implemented |
| 16 | GPU Scheduling | 🔧 Partial | Lock file concept referenced in spec but no actual implementation |
| 17 | Workspace Structure | 🔧 Partial | Matches spec layout for code. Missing: `styles/` directory with RON files, `scripts/audiogen_server.py` |

### Gaps (Asset Pipeline)

| Gap | Status | Notes |
|-----|--------|-------|
| ~~Adaptive Music runtime~~ | ✅ Implemented | Full system exists in amigo_audio: BarClock, vertical layering, horizontal transitions, stingers |
| RON-based music config loading | 🗓 Roadmap | Spec §12 shows `.music.ron` and `.sequence.ron` config files — engine has the runtime structs but no RON loader |
| ComfyUI HTTP integration | 🗓 Roadmap | Client methods exist as stubs. No actual HTTP calls to ComfyUI (no reqwest/ureq dependency) |
| ACE-Step / AudioGen HTTP integration | 🗓 Roadmap | Client structs scaffolded, all generate() calls return empty placeholders |
| 13 missing audiogen MCP tools | 🗓 Roadmap | generate_core_melody, generate_stem, generate_variation, extend_track, remix, generate_ambient, loop_trim, normalize, convert, preview, and more |
| Style RON files on disk | 🗓 Roadmap | Code loads from RON but no `.style.ron` or `.audio_style.ron` files exist — only hardcoded builtins |
| Clean Mode stem workflow | 🗓 Roadmap | Per-stem conditioned generation (core melody → stems) not scaffolded |
| Post-processing pipeline order | ⚠️ Inconsistency | Spec order: Downscale → Palette Clamp → AA Removal → Transparency → Outline → Tile Edge. Code order: Transparency → AA Removal → Palette Clamp → Outline |

### Key insight: artgen + audiogen are architectural scaffolds

Both MCP servers have correct architecture (tools registered, workflow building, post-processing, style system) but **no actual AI backend communication**. All ComfyUI and ACE-Step/AudioGen calls are stubs. The post-processing pipeline in amigo_artgen is the only fully functional component. This means the tools will work correctly once HTTP clients are connected, but currently produce no real output.

---

## Asset Format Spec ([03-asset-format-spec.md](03-asset-format-spec.md))

| # | Section | Status | Notes |
|---|---------|--------|-------|
| 1 | Overview | 🗓 Roadmap | v0.2.0-draft — design phase |
| 2 | Project Structure | 🔧 Partial | `Amigo.toml` manifest exists and is parsed by `amigo_cli`, but fields differ from spec (simpler schema) |
| 3 | Runtime Formats (.ait, WebP, OGG, FLAC, pattern bytecode) | 🗓 Roadmap | Spec complete, none implemented. Engine uses PNG + WAV directly |
| 4 | Sprite Format (.sprite.toml) | 🗓 Roadmap | TOML descriptor spec complete, no tooling. Engine reads .aseprite directly |
| 5 | Tileset Format (.tileset.toml) | 🗓 Roadmap | Spec complete, no tooling |
| 6 | Map Format (.map.toml) | 🗓 Roadmap | Spec complete. Engine uses .amigo (RON) format instead |
| 7 | Entity Format (.entity.toml) | 🗓 Roadmap | Spec complete, no tooling |
| 8 | Palette Format (.palette.toml) | 🗓 Roadmap | Spec complete, no tooling |
| 9 | Audio Pattern Language | 🗓 Roadmap | Mini-notation spec + formal grammar complete, no parser or runtime synthesizer |
| 10 | Instrument Bank (.bank.toml) | 🗓 Roadmap | Spec complete, no tooling |
| 11 | Build System (`amigo build`) | 🔧 Partial | `amigo build` exists but only validates project structure. Full asset build pipeline (image → .ait, WAV → OGG, pattern compile) not implemented |
| 12 | Import Pipeline | 🔧 Partial | Aseprite import works (amigo_assets/aseprite.rs). Tiled, LDTK, MML, VGM, ROM imports not built |
| 13 | Export Pipeline | 🗓 Roadmap | `amigo export-level` converts .amigo → JSON. Full Tiled/LDTK/Aseprite/MML export not built |
| 14 | .amigo-pak Binary Format | 🔧 Partial | PakWriter exists (amigo_assets/pak.rs) with magic `AMIGOPAK`, asset kinds (Sprite/Audio/Data/Level/Font/AtlasImage/AtlasManifest). Differs from spec: no type tags (0x01-0x07), no LZ4 block compression, no flags, no SHA256 manifest. Functional but simpler format |
| 15 | CLI Integration | 🔧 Partial | `amigo build`, `amigo pack` exist but don't implement full spec pipeline. `amigo import/export` subcommands not implemented |
| 16 | Migration & Versioning | 🗓 Roadmap | Spec only |
| 17 | Future Extensions (§14) | 🗓 Roadmap | World format, dialogue format, shader format, PICO-8/Godot import, tier 2 ROM support — all future |

### Summary

The Asset Format spec is a comprehensive design document for the v0.2.0 asset pipeline. Currently:
- **Implemented:** Aseprite import, basic .pak packing, project manifest parsing, level export
- **Not implemented:** TOML asset descriptors (.sprite.toml, .tileset.toml, etc.), runtime formats (.ait, WebP conversion, OGG/FLAC encoding), pattern language parser, full build pipeline, Tiled/LDTK/ROM importers, round-trip export
- **Gap:** The engine currently loads assets directly (PNG, .aseprite, WAV) without the intermediate TOML descriptor layer. The spec envisions a fundamentally different pipeline that would require significant new tooling

---

## Spec Completeness Audit

### Gaps in spec definitions (incomplete or vague)

| Spec | Section | Issue |
|------|---------|-------|
| 01 | §12 Animation | Very brief (4 lines). No detail on AnimationStateMachine API, transition rules, blending, or frame event callbacks. Actual implementation (1,903 LOC) is far richer than spec |
| 01 | §13 Camera | Brief list of patterns. Missing: API examples, configuration parameters, easing functions. Implementation has Follow/Shake/Clamp but spec lists 7 patterns |
| 01 | §14 Input | Single paragraph. Missing: ActionMap RON format, rebinding API, input priority, touch support plans |
| 01 | §18 Debug | Brief. Missing: exact F-key mappings, memory overlay details, Tracy configuration |
| 01 | §20 Plugin | Generic trait shown. Missing: lifecycle order, resource registration, plugin dependencies, how plugins interact with ECS |
| 01 | §24 Genre Modules | Only covers 4 of 18 implemented modules (platformer, farming, bullet_pattern, puzzle). Missing: combat, loot, inventory, turn_combat, dialog, crafting, procgen, economy, ai, navigation, status_effect, projectile, roguelike, fighting |
| 01 | — | Missing sections: Atmosphere system, save/load system (only appendix A.7), collision events, game presets/templates, level loader, scene transitions |
| 02 | §12 Adaptive Music | Engine-side runtime (AdaptiveMusicEngine) IS implemented. Remaining gap: RON config loading for music definitions not built |
| 03 | §9 Pattern Language | Grammar defined, but no detail on: synthesizer architecture (oscillators, envelopes, filters), sample playback engine, pattern evaluation algorithm, MIDI-like event generation |
| 03 | §11 Build System | Pipeline steps listed but no detail on: incremental builds, dependency tracking, cache invalidation, parallel processing |

### Inconsistencies between specs

| Issue | Details |
|-------|---------|
| UI decision conflict | Spec §21 describes two-tier Pixel UI. Key Decisions table §27 says "No egui". Reality: editor uses egui. Spec needs to acknowledge egui for editor |
| LOC counts outdated | All LOC figures in STATUS.md were stale. Updated in this revision |
| Module structure mismatch | Spec §3 shows `games/amigo_td/` directory — doesn't exist, game is in separate repo |
| Asset Pipeline §9 vs §19 | Engine spec §9 says `.pak` packing is CLI tool. §19 says `amigo pack` missing. Both outdated — `amigo pack` is implemented |
| Phase tracking | §26 says "Phase 0–1 complete" but phases 2–5 are also largely done (editor, API, CLI, headless, distribution) |
| Spec version | 03-asset-format-spec.md references "Amigo RomKit Spec v0.1" in depends-on — this spec doesn't exist in the repo |

---

## LOC Summary (source of truth)

### Engine Crates

| Crate | LOC | Primary Responsibility |
|-------|-----|----------------------|
| amigo_core | 18,360 | Math, ECS, physics, collision, pathfinding, 18 genre modules, game state, events, save, scheduler |
| amigo_editor | 5,103 | Level editor: tile painter, wave editor, collision editor, playtest, heatmap, visual scripting, wizard, egui UI |
| amigo_render | 4,223 | wgpu renderer, sprite batcher, atlas, camera, particles, lighting, post-processing, atmosphere, font, egui integration |
| amigo_net | 2,208 | Networking, lockstep, replay, command serialization, transport trait |
| amigo_animation | 1,903 | Sprite animation state machine, Aseprite integration, frame events |
| amigo_engine | 1,720 | Engine builder, game loop, config, context, splash screen, headless mode |
| amigo_api | 1,291 | JSON-RPC IPC server (40+ methods), screenshot, tick, entity inspection |
| amigo_audio | 1,126 | kira wrapper, SFX with variants, music playback, volume channels |
| amigo_assets | 1,030 | Asset manager, Aseprite parser, hot reload, handle system, .pak writer |
| amigo_input | 944 | Keyboard, mouse, gamepad (gilrs), action maps |
| amigo_tilemap | 774 | Tilemap data structures, auto-tiling, tile properties |
| amigo_scene | 373 | Scene stack, state machine, transitions |
| amigo_ui | 352 | Pixel UI context (Tier 1: text, rect, sprite, progress bar) |
| amigo_debug | 306 | FPS overlay, Tracy integration, debug toggles |
| **Total** | **39,713** | |

### Tools

| Tool | LOC | Purpose |
|------|-----|---------|
| amigo_artgen | 2,608 | MCP server for AI art generation (ComfyUI) |
| amigo_audiogen | 1,856 | MCP server for AI audio generation (ACE-Step, AudioGen) |
| amigo_cli | 1,151 | CLI: new, scene, build, run, pack, release, publish, editor, export-level, info |
| amigo_mcp | 902 | MCP bridge wrapping amigo_api JSON-RPC |
| **Total** | **6,517** | |

### Examples (8)

starter, particles, tilemap_demo, audio_demo, ecs_demo, input_demo, animation_demo, pathfinding_demo
