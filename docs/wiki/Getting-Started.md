# Getting Started

Build your first Amigo game in 15 minutes.

## Prerequisites

- **Rust toolchain** -- install via [rustup](https://rustup.rs/)
- **GPU drivers** -- Vulkan, Metal, or DX12 capable GPU required for the wgpu renderer
- **VS Code** + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) -- provides IntelliSense (autocomplete, go-to-definition, inline errors, hover docs) for the engine API

See [Installation](Installation) for detailed setup instructions.

## Create a Project

```sh
# Install the CLI
curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-engine/main/install.sh | sh

# Scaffold a new game (`amigo list-templates` shows the other templates)
amigo new my_game --template platformer
cd my_game

# Open in VS Code
code .
```

When VS Code opens, rust-analyzer will index the project automatically. You get full autocomplete for all engine types (`GameContext`, `DrawContext`, `SimVec2`, `CollisionShape`, etc.) and can jump to definitions with `F12`.

This generates a single crate (not a workspace) with the following structure:

```
my_game/
  Cargo.toml
  amigo.toml          # engine configuration
  src/
    main.rs           # entry point
    scenes/
      mod.rs
      title_menu.rs   # title screen
      gameplay.rs     # playable skeleton for the chosen template
      pause_menu.rs   # pause overlay
  assets/
    sprites/
    tilesets/
    levels/
    audio/
    fonts/
  .vscode/
    tasks.json        # `amigo dev` as the default build task
```

`amigo new --template <name>` picks what `gameplay.rs` contains: a platformer has
gravity and jumping, a tower defense has a build cursor and waves, and so on. Run
`amigo list-templates` for the full list.

## Minimal Example

What a `Game` looks like at its smallest — this is the shape the generated scenes
follow:

```rust
use amigo_engine::prelude::*;

struct MyGame;

impl Game for MyGame {
    fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_rect(Rect::new(16.0, 16.0, 32.0, 32.0), Color::WHITE);
    }
}

fn main() {
    Engine::build()
        .title("My Game")
        .virtual_resolution(480, 270)
        .build()
        .run(MyGame);
}
```

## Run It

```sh
amigo run
```

Or press `Ctrl+Shift+B` in VS Code if you have the Amigo task configured.

A window opens at the virtual resolution your template chose (480x270 unless you
set another, which is also the engine's default). The engine runs a fixed-timestep
game loop at 60 ticks/second with interpolated rendering. Inside `update`,
`ctx.time.dt` is always one tick (1/60 s).

### Record and replay a session

```sh
amigo run --record replays/bug.json   # play; the file is written on exit
amigo run --replay replays/bug.json   # the same session again, tick for tick
```

A replay stores the input of every tick and the random seed. Draw random numbers
from `ctx.rng`, not from an RNG of your own, so a replay draws the same ones.
Implement `Game::state_hash` and a replay also reports the first tick where the
game went another way. The `engine/replays` spec, linked from
[Specifications](Specifications), has the details.

### Play with two

```sh
amigo run --host 7777                  # on the first machine
amigo run --join 192.168.1.5:7777      # on the second
```

Both machines run the same game in lockstep: each tick runs once both
players' input is there. Write the gameplay against `ctx.players()` and
`ctx.player_actions(p)` instead of `ctx.input`, and the same code runs alone
(one player) and with two. See `examples/lockstep_demo`.

## Draw a Sprite

```rust
impl Game for MyGame {
    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_sprite("player", RenderVec2 { x: 100.0, y: 100.0 });
    }
}
```

Render positions are `RenderVec2` (f32). Simulation state uses `SimVec2` with
fixed-point `Fix` components instead — see [ADR-0001](https://github.com/amigo-labs/amigo-engine/blob/main/docs/adrs/0001-fixed-point-simulation.md)
for why the two are separate. There is no `Vec2` in the prelude.

Place a `player.png` or `player.aseprite` in `assets/sprites/`; subfolders
become part of the name (`assets/sprites/enemies/bat.png` is `"enemies/bat"`).
The asset pipeline auto-loads it with hot reload in dev mode.

An Aseprite file's tags become animations named `"<sprite>/<tag>"`. Play one
with an `AnimPlayer`:

```rust
// fields: hero: AnimPlayer = AnimPlayer::new("player/idle")
// in update:
self.hero.play("player/walk", PlayMode::Loop);
self.hero.advance(ctx.assets.animations());
// in draw:
ctx.draw_animated("player", &self.hero, RenderVec2::new(100.0, 80.0));
```

`examples/animation_demo` shows idle, walk and a one-shot jump.

## Add a HUD

Widgets go through the immediate-mode UI in `update`, and are drawn in a
screen-space pass after post-processing — so a HUD stays put while the world
scrolls:

```rust
fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
    ctx.ui.pixel_text("Score: 0", 4.0, 4.0, Color::WHITE);
    ctx.ui.progress_bar(Rect::new(4.0, 14.0, 60.0, 6.0), 0.75, Color::GREEN);
    SceneAction::Continue
}
```

## Add Another Scene

A scene is just another `Game`. Returning `SceneAction::Push` from `update` stacks
it on top; `Pop` returns to what was underneath, with its state intact:

```rust
if ctx.input.pressed(KeyCode::Escape) {
    return SceneAction::Push(Box::new(|| Box::new(PauseMenu) as Box<dyn Game>));
}
```

## Configuration

Edit `amigo.toml` to adjust window size, audio, rendering, and dev settings.

## Next Steps

- [CLI Reference](CLI-Reference) -- all available commands
- [Architecture](Architecture) -- how the engine is structured
- [AI Setup](AI-Setup) -- optional AI asset pipelines
- [Audio Pipeline](Audio-Pipeline) -- convert audio to chiptune notation
- [Specifications](Specifications) -- detailed engine module docs
