---
status: done
crate: amigo_engine
depends_on: ["engine/core"]
last_updated: 2026-10-07
---

# Plugin System

> **Status: done.** `Plugin` and `PluginContext` live in
> `crates/amigo_engine/src/engine.rs`; there is no separate `amigo_plugin` crate.
> A plugin registers events, resources and systems, and it has hooks before and
> after every tick's `Game::update` and after every frame's `Game::draw`. Those
> hooks are how it handles input and draws. Where the original design and the
> code differ, the sections below say so ("Designed" / "Actual").

## Purpose

Compile-time plugin architecture for composing engine features. No dynamic plugin loading, no runtime discovery. Compile-time decides what's included, Plugin Trait provides clean initialization order and update lifecycle.

## Public API

### Feature Flags (Compile-Time)

Designed:

```toml
[features]
default = ["audio", "input"]
audio = ["dep:kira"]
editor = []
api = []
networking = ["dep:laminar"]
gamepad = ["dep:gilrs"]
```

Actual, in `crates/amigo_engine/Cargo.toml`. There is no `networking` or `gamepad`
flag: `amigo_net` is a mandatory dependency (with its own `rollback_net` flag) and
`gilrs` is unconditional in `amigo_input`.

```toml
[features]
default = ["audio", "input"]
audio = ["dep:amigo_audio"]
editor = ["dep:amigo_editor", "amigo_render/editor", "amigo_editor/egui"]
api = ["dep:amigo_api", "dep:ctrlc", "dep:ron"]
tracy = ["amigo_debug/tracy"]
input = []
async_tasks = ["amigo_core/async_tasks"]
```

### Plugin Trait (Structure)

Each `amigo_*` crate exposes a Plugin with a clean lifecycle:

Designed:

```rust
pub trait Plugin {
    fn build(&self, engine: &mut EngineBuilder);
    fn update(&mut self, ctx: &mut GameContext);
}
```

Actual:
- `build` receives a `PluginContext`, not the builder. A plugin holding
  `&mut EngineBuilder` could reconfigure anything, including things already
  consumed, so `PluginContext` is a narrow registration surface instead.
- Besides `update` there are `init` (once the window and renderer exist),
  `pre_update` and `draw`.

```rust
pub trait Plugin: 'static {
    fn build(&self, ctx: &mut PluginContext);
    /// Once, after the window and renderer exist.
    fn init(&self, _ctx: &mut GameContext) {}
    /// Every tick after input is read, before `Game::update`: input handling.
    fn pre_update(&mut self, _ctx: &mut GameContext) {}
    /// Every tick after `Game::update`, before the ECS and event flush.
    fn update(&mut self, _ctx: &mut GameContext) {}
    /// Every frame after `Game::draw`, in the same `DrawContext`: a draw pass.
    fn draw(&self, _ctx: &mut DrawContext) {}
}

impl PluginContext {
    pub fn register_event<T: 'static>(&mut self);
    pub fn insert_resource<T: 'static>(&mut self, resource: T);
    /// Run `system` every tick at `stage`.
    pub fn add_system(&mut self, stage: SystemStage, system: impl FnMut(&mut GameContext) + 'static);
}

pub enum SystemStage { PreUpdate, PostUpdate }

// Usage:
Engine::build()
    .add_plugin(MyPlugin)
    .build()
    .run(MyGame);
```

Within a stage, the plugins' hooks run first, then the registered systems, each
in the order they were added. Plugins and systems run in the windowed and in the
headless loop alike. `draw` only runs in the windowed loop.

There are no `AudioPlugin`, `InputPlugin` or `EditorPlugin` types: audio, input and
the editor are feature flags the engine wires directly, not plugins. The editor
exposes `EditorRuntime` (`crates/amigo_editor/src/plugin.rs`).
