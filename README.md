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

| Crate             | Description                                      |
|-------------------|--------------------------------------------------|
| `amigo_core`      | Fixed-point math, ECS, save system, scheduling   |
| `amigo_render`    | wgpu renderer, camera, particles, lighting       |
| `amigo_input`     | Keyboard, mouse, and gamepad input               |
| `amigo_assets`    | Asset loading, Aseprite import, hot-reloading    |
| `amigo_tilemap`   | Tilemap data structures and utilities            |
| `amigo_animation` | Sprite animation state machine                   |
| `amigo_scene`     | Scene stack and transitions                      |
| `amigo_ui`        | Immediate-mode pixel UI                          |
| `amigo_net`       | Networking / multiplayer transport               |
| `amigo_debug`     | Debug overlay and system profiling               |
| `amigo_audio`     | Audio playback (feature-gated)                   |
| `amigo_editor`    | Editor integration                               |
| `amigo_api`       | Public API surface for plugins                   |
| `amigo_engine`    | Top-level crate that re-exports everything       |

### Tools

| Crate            | Description                    |
|------------------|--------------------------------|
| `amigo_cli`      | CLI for project scaffolding    |
| `amigo_mcp`      | MCP integration tool           |
| `amigo_artgen`   | Art generation utilities       |
| `amigo_audiogen` | Audio generation utilities     |

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
