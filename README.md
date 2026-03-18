# amigo-engine

A 2D game engine built on fixed-point math, ECS, and wgpu. Designed for deterministic
simulation, pixel-art rendering, and fast iteration.

## Features

- **ECS** -- sparse-set entity-component system with fixed-point math (`amigo_core`)
- **Rendering** -- wgpu-based sprite batching, camera, particles, lighting, post-processing (`amigo_render`)
- **Physics** -- fixed-timestep simulation with deterministic fixed-point types
- **Audio** -- playback via kira (opt-in `audio` feature) (`amigo_audio`)
- **Animation** -- sprite animation state machine with Aseprite import (`amigo_animation`)
- **Networking** -- multiplayer transport layer (`amigo_net`)
- **Editor** -- built-in editor tooling (`amigo_editor`)
- **CLI** -- project scaffolding and build tool (`amigo_cli`)
- **UI** -- immediate-mode pixel UI for HUD and menus (`amigo_ui`)
- **Debug** -- FPS overlay, system profiling, visual debug toggles (`amigo_debug`)

## Prerequisites

- **Rust toolchain** -- install via [rustup](https://rustup.rs/):

    ```sh
    # Linux / macOS
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

    # Windows
    # Download and run rustup-init.exe from https://rustup.rs/
    ```

    After installation, make sure `cargo` is on your PATH (restart your terminal or run `source $HOME/.cargo/env`).

- **GPU drivers** -- a Vulkan, Metal, or DX12 capable GPU is required for the wgpu renderer.

## Quick start

```sh
# Install the CLI
cargo install --path tools/amigo_cli

# Create a new project
amigo new my_game

# Run it
cd my_game
cargo run
```

## Architecture

| Crate             | Description                                    |
| ----------------- | ---------------------------------------------- |
| `amigo_core`      | Fixed-point math, ECS, save system, scheduling |
| `amigo_render`    | wgpu renderer, camera, particles, lighting     |
| `amigo_input`     | Keyboard, mouse, and gamepad input             |
| `amigo_assets`    | Asset loading, Aseprite import, hot-reloading  |
| `amigo_tilemap`   | Tilemap data structures and utilities          |
| `amigo_animation` | Sprite animation state machine                 |
| `amigo_scene`     | Scene stack and transitions                    |
| `amigo_ui`        | Immediate-mode pixel UI                        |
| `amigo_net`       | Networking / multiplayer transport             |
| `amigo_debug`     | Debug overlay and system profiling             |
| `amigo_audio`     | Audio playback (feature-gated)                 |
| `amigo_editor`    | Editor integration                             |
| `amigo_api`       | Public API surface for plugins                 |
| `amigo_engine`    | Top-level crate that re-exports everything     |

### Tools

| Crate            | Description                 |
| ---------------- | --------------------------- |
| `amigo_cli`      | CLI for project scaffolding |
| `amigo_mcp`      | MCP integration tool        |
| `amigo_artgen`   | Art generation utilities    |
| `amigo_audiogen` | Audio generation utilities  |

## Documentation

| Guide                                                  | Description                        |
| ------------------------------------------------------ | ---------------------------------- |
| [Getting Started](docs/getting-started.md)             | First game in 15 minutes           |
| [Spec Overview](docs/specs/_index.md)                  | All engine modules                 |
| [Engine Tricks](docs/specs/engine/tricks.md)           | Optimization techniques reference  |

### Specifications

| Area | Specs |
| ---- | ----- |
| Engine | [Core](docs/specs/engine/core.md), [Rendering](docs/specs/engine/rendering.md), [Audio](docs/specs/engine/audio.md), [Input](docs/specs/engine/input.md), [Tilemap](docs/specs/engine/tilemap.md), [Pathfinding](docs/specs/engine/pathfinding.md), [Animation](docs/specs/engine/animation.md), [Camera](docs/specs/engine/camera.md), [UI](docs/specs/engine/ui.md), [Networking](docs/specs/engine/networking.md), [Memory](docs/specs/engine/memory-performance.md), [Plugins](docs/specs/engine/plugin-system.md), [Tricks](docs/specs/engine/tricks.md) |
| Assets | [Format](docs/specs/assets/format.md), [Pipeline](docs/specs/assets/pipeline.md), [Atlas](docs/specs/assets/atlas.md) |
| Tooling | [CLI](docs/specs/tooling/cli.md), [Editor](docs/specs/tooling/editor.md), [Debug](docs/specs/tooling/debug.md) |
| AI Pipelines | [Art Gen](docs/specs/ai-pipelines/artgen.md), [Audio Gen](docs/specs/ai-pipelines/audiogen.md), [Agent API](docs/specs/ai-pipelines/agent-api.md) |
| Game: TD | [Design](docs/specs/games/td/design.md), [UI](docs/specs/games/td/ui.md) |
| Config | [amigo.toml](docs/specs/config/amigo-toml.md), [Data Formats](docs/specs/config/data-formats.md) |

## Minimal example

```rust
use amigo_engine::prelude::*;

struct MyGame;

impl Game for MyGame {
    fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
        SceneAction::Continue
    }

    fn draw(&self, _ctx: &mut DrawContext) {}
}

fn main() {
    EngineBuilder::new()
        .run(MyGame);
}
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
