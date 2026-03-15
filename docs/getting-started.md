# Getting Started with Amigo Engine

Amigo Engine is a Rust 2D pixel art game engine designed for building retro-style games with modern ergonomics.

## Prerequisites

### Rust

Amigo Engine requires [Rust](https://www.rust-lang.org/) (latest stable). Install via [rustup](https://rustup.rs/):

**Windows:**

```sh
winget install Rustlang.Rustup
```

**macOS:**

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Linux (Ubuntu/Debian):**

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### System Dependencies (Linux)

On Linux you need development libraries for graphics, audio, and input:

```sh
# Ubuntu / Debian
sudo apt-get install -y \
  libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev \
  libx11-dev libxi-dev libxrandr-dev libxcursor-dev libxinerama-dev \
  pkg-config

# Fedora
sudo dnf install -y \
  alsa-lib-devel systemd-devel wayland-devel libxkbcommon-devel \
  libX11-devel libXi-devel libXrandr-devel libXcursor-devel libXinerama-devel

# Arch
sudo pacman -S --needed \
  alsa-lib systemd-libs wayland libxkbcommon \
  libx11 libxi libxrandr libxcursor libxinerama pkg-config
```

Windows and macOS have no additional system dependencies.

## Installation

Install the Amigo CLI tool:

```sh
cargo install --path tools/amigo_cli
```

## Create a New Project

```sh
amigo new my_game
```

Available templates: `platformer`, `topdown-rpg`, `turn-based-rpg`, `roguelike`, `tower-defense`, `bullet-hell`, `puzzle`, `farming-sim`, `fighting`, `visual-novel`.

To use a specific template:

```sh
amigo new my_game --template platformer
```

## Run Your Game

```sh
cd my_game && cargo run
```

## Core Concepts

### The Game Trait

Every game implements the `Game` trait with three lifecycle methods:

- **`init()`** -- called once at startup.
- **`update()`** -- called every tick with a `GameContext` for game logic.
- **`draw()`** -- called every frame with a `DrawContext` for rendering.

### Engine Setup

Use `EngineBuilder` to configure and launch your game. The engine runs a fixed timestep of 60 ticks per second.

### Contexts

- **`GameContext`** -- provided during `update()`. Gives access to input, ECS world, timing, and scene management.
- **`DrawContext`** -- provided during `draw()`. Gives access to sprite drawing, text rendering, and camera control.

## Minimal Example

```rust
use amigo_engine::prelude::*;

struct MyGame;

impl Game for MyGame {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        if ctx.input.pressed(KeyCode::Escape) {
            return SceneAction::Quit;
        }
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_text("Hello, Amigo!", 10.0, 10.0, Color::WHITE);
    }
}

fn main() {
    Engine::build()
        .title("My First Game")
        .virtual_resolution(480, 270)
        .build()
        .run(MyGame);
}
```

## A More Complete Example

This example shows a player that moves with the arrow keys and draws a sprite on screen.

```rust
use amigo_engine::prelude::*;

struct MyGame {
    player_pos: Vec2,
    speed: f32,
}

impl MyGame {
    fn new() -> Self {
        Self {
            player_pos: Vec2::new(240.0, 135.0),
            speed: 120.0,
        }
    }
}

impl Game for MyGame {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        if ctx.input.pressed(KeyCode::Escape) {
            return SceneAction::Quit;
        }

        let mut direction = Vec2::ZERO;
        if ctx.input.held(KeyCode::Left)  { direction.x -= 1.0; }
        if ctx.input.held(KeyCode::Right) { direction.x += 1.0; }
        if ctx.input.held(KeyCode::Up)    { direction.y -= 1.0; }
        if ctx.input.held(KeyCode::Down)  { direction.y += 1.0; }

        if direction != Vec2::ZERO {
            direction = direction.normalize();
        }

        self.player_pos += direction * self.speed * ctx.delta_time();

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_sprite("player", self.player_pos);
        ctx.draw_text("Arrow keys to move", 10.0, 10.0, Color::WHITE);
    }
}

fn main() {
    Engine::build()
        .title("Player Movement")
        .virtual_resolution(480, 270)
        .build()
        .run(MyGame::new());
}
```

## Assets

Load and draw assets by string name for convenience:

```rust
ctx.draw_sprite("player", pos);
```

For performance-critical paths, use typed handles instead:

```rust
ctx.draw_sprite_handle(HANDLE, pos);
```

## ECS

Amigo Engine includes a lightweight Entity Component System:

- **`World`** -- the ECS container that holds all entities and components.
- **`EntityId`** -- a unique identifier for each entity.
- **`SparseSet`** -- the underlying storage for component data.

## Scene Stack

Manage game flow with scene actions returned from `update()`:

- **`SceneAction::Push`** -- push a new scene onto the stack.
- **`SceneAction::Pop`** -- pop the current scene.
- **`SceneAction::Replace`** -- replace the current scene.
- **`SceneAction::Quit`** -- exit the game.

## Input

```rust
// Keyboard
ctx.input.pressed(KeyCode::Space)   // true on the frame the key is first pressed
ctx.input.held(KeyCode::Space)      // true every frame the key is held
ctx.input.released(KeyCode::Space)  // true on the frame the key is released

// Mouse
let world_pos = ctx.input.mouse_world_pos();
```

## Camera

```rust
ctx.camera.follow(target);                  // smoothly follow a target position
ctx.camera.shake(intensity, duration);      // screen shake effect
```

## Configuration

Project settings live in `amigo.toml` at the root of your project.

## Hot Reload

In dev mode, assets are hot-reloaded automatically. Change a sprite or configuration file and see the result immediately without restarting your game.

## Debug Overlay

Press the following keys during development to toggle debug overlays:

| Key | Overlay |
|-----|---------|
| F1  | General debug info |
| F2  | Performance stats |
| F3  | Collision boxes |
| F4  | ECS inspector |
