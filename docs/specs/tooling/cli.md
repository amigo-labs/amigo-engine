# CLI Tool

> Status: draft
> Crate: amigo_cli
> Depends on: [engine/core](../engine/core.md)
> Last updated: 2026-03-16

## Zweck

The `amigo` CLI is the primary developer-facing tool for creating, running, building, and distributing Amigo Engine games.

## Public API

### CLI Commands

```bash
amigo new my_game              # scaffold project
amigo run                      # dev build + run
amigo run --api                # with AI API server
amigo run --api --headless     # headless simulation
amigo pack                     # assets -> game.pak
amigo build --release          # optimized binary
amigo release --target windows,linux  # full pipeline
```

### Engine Startup (amigo_api)

```bash
# Start engine with API server enabled
amigo run --api                          # windowed + API on default socket
amigo run --api --port 9999              # windowed + API on TCP port
amigo run --api --headless               # no window, max speed simulation
amigo run --api --headless --level dune_01  # headless with specific level
```

## Verhalten

### `amigo new` -- Starter Template

Scaffolds a new project with the following structure:

```
amigo-engine/                   # github.com/amigo-labs/amigo-engine
+-- Cargo.toml                  # Workspace root
+-- crates/
|   +-- amigo_core/              # Math, types, Fixed-Point, SparseSet ECS
|   +-- amigo_render/            # wgpu renderer, sprite batcher, camera
|   +-- amigo_ui/                # Pixel-native UI system (Game HUD + Editor widgets)
|   +-- amigo_audio/             # kira wrapper, audio manager
|   +-- amigo_input/             # Keyboard, mouse, gamepad abstraction
|   +-- amigo_tilemap/           # Tilemap system, collision layers
|   +-- amigo_animation/         # Sprite animation, Aseprite integration
|   +-- amigo_assets/            # Asset loading, hot reload, atlas packing
|   +-- amigo_net/               # Networking, transport trait, commands
|   +-- amigo_scene/             # Scene/state machine
|   +-- amigo_editor/            # Level editor (feature flag: "editor")
|   +-- amigo_api/               # AI/IPC interface (feature flag: "api")
|   +-- amigo_debug/             # Debug overlay, Tracy integration
|   +-- amigo_engine/            # Ties everything together, public API
+-- tools/
|   +-- amigo_cli/               # CLI: pack, build, release, new project
|   +-- amigo_mcp/               # MCP server wrapping amigo_api for Claude Code
|   +-- amigo_artgen/            # MCP server for AI art generation (ComfyUI)
|   +-- amigo_audiogen/          # MCP server for AI audio generation (ACE-Step, AudioGen)
+-- games/
|   +-- amigo_td/                # Tower Defense game
+-- assets/
    +-- ...
```

### `amigo run` -- Development

Compiles in dev mode and runs the game. With `--api`, starts the JSON-RPC IPC server for AI agent control (see AI Agent Interface). With `--headless`, runs without a window at maximum simulation speed.

### `amigo pack` -- Asset Packing

Processes all assets for release distribution:
- Sprites are bin-packed into texture atlases (see [assets/atlas](../assets/atlas.md))
- Audio is compressed
- Data files are validated
- Everything is bundled into `game.pak` (memory-mappable)

### `amigo build --release` -- Optimized Build

Compiles with the release profile for maximum performance.

### `amigo release` -- Full Distribution Pipeline

Runs the full pipeline: pack assets, build release binary, bundle for distribution.

## Internes Design

### Release Profile

```toml
[profile.release]
lto = true
codegen-units = 1
strip = true
panic = "abort"
opt-level = 2
```

### Distribution

Windows + Linux binaries. `game.pak` for assets. Target: <70MB total. CI/CD via GitHub Actions.

### Feature Flags (Compile-Time)

```toml
[features]
default = ["audio", "input"]
audio = ["dep:kira"]
editor = []
api = []
networking = ["dep:laminar"]
gamepad = ["dep:gilrs"]
```

## Nicht-Ziele

- GUI-based project management (CLI only)
- Package registry / crate publishing
- macOS / iOS / Android / console builds (Phase 1)

## Offene Fragen

- Whether `amigo pack` should run as part of `amigo build --release` automatically
- Steam/itch.io integration details for `amigo release`
- Whether to support `amigo watch` for auto-rebuild on source changes
