# Getting Started

Build your first Amigo game in 15 minutes.

## Prerequisites

- **Rust toolchain** -- install via [rustup](https://rustup.rs/)
- **GPU drivers** -- Vulkan, Metal, or DX12 capable GPU required for the wgpu renderer

See [Installation](Installation) for detailed setup instructions.

## Create a Project

```sh
# Install the CLI
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.sh | sh

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

## Minimal Example

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

## Run It

```sh
cargo run
```

A window opens at 640x360 virtual resolution. The engine runs a fixed-timestep game loop at 60 ticks/second with interpolated rendering.

## Draw a Sprite

```rust
impl Game for MyGame {
    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_sprite("player", Vec2::new(100.0, 100.0));
    }
}
```

Place a `player.aseprite` or `player.png` in `assets/sprites/`. The asset pipeline auto-loads it with hot reload in dev mode.

## Configuration

Edit `amigo.toml` to adjust window size, audio, rendering, and dev settings.

## Next Steps

- [CLI Reference](CLI-Reference) -- all available commands
- [Architecture](Architecture) -- how the engine is structured
- [AI Setup](AI-Setup) -- optional AI asset pipelines
- [Audio Pipeline](Audio-Pipeline) -- convert audio to chiptune notation
- [Specifications](Specifications) -- detailed engine module docs
