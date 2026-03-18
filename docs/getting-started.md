# Getting Started

Build your first Amigo game in 15 minutes.

## Prerequisites

- **Rust toolchain** -- install via [rustup](https://rustup.rs/)
- **GPU drivers** -- Vulkan, Metal, or DX12 capable GPU required for the wgpu renderer

## Create a project

```sh
# Install the CLI
cargo install --path tools/amigo_cli

# Scaffold a new game
amigo new my_game
cd my_game
```

This generates a workspace with the following structure:

```
my_game/
  Cargo.toml          # workspace root
  amigo.toml          # engine configuration
  game/
    Cargo.toml
    src/
      main.rs         # entry point
      game.rs         # Game trait impl
  assets/
    sprites/
    tilesets/
    audio/
```

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

## Run it

```sh
cargo run
```

A window opens at 640x360 virtual resolution. The engine runs a fixed-timestep game loop at 60 ticks/second with interpolated rendering.

## Draw a sprite

```rust
impl Game for MyGame {
    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_sprite("player", Vec2::new(100.0, 100.0));
    }
}
```

Place a `player.aseprite` or `player.png` in `assets/sprites/`. The asset pipeline auto-loads it with hot reload in dev mode.

## Configuration

Edit `amigo.toml` to adjust window size, audio, rendering, and dev settings. See the [config spec](specs/config/amigo-toml.md) for all options.

## Next steps

- [Engine Core](specs/engine/core.md) -- ECS, fixed-point math, game loop
- [Rendering](specs/engine/rendering.md) -- sprite batching, layers, effects
- [Input](specs/engine/input.md) -- keyboard, mouse, gamepad
- [Audio](specs/engine/audio.md) -- SFX, adaptive music, ambient layers
- [Tilemap](specs/engine/tilemap.md) -- tile grids, chunk streaming
- [Full Spec Overview](specs/_index.md) -- all engine modules
