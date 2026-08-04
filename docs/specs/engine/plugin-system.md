---
status: partial
crate: amigo_engine
depends_on: ["engine/core"]
last_updated: 2026-08-03
---

# Plugin System

> **Status: the trait exists, but it can register less than this spec describes.**
> There is no `amigo_plugin` crate: `Plugin` and `PluginContext` live in
> `crates/amigo_engine/src/engine.rs`. `PluginContext` offers exactly two
> capabilities — `register_event::<T>()` and `insert_resource::<T>()` — so a plugin
> cannot register a system, a draw pass or an input handler. The signatures and
> feature flags below are the design, not the code; see the corrections inline.

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

Actual: `build` receives a `PluginContext`, not the builder, and there is an extra
`init` hook that runs once the window and renderer exist. Passing the builder was
dropped because a plugin holding `&mut EngineBuilder` could reconfigure anything,
including things already consumed; `PluginContext` is a narrow registration
surface instead — currently a *very* narrow one.

```rust
pub trait Plugin: 'static {
    fn build(&self, ctx: &mut PluginContext);
    fn init(&self, _ctx: &mut GameContext) {}
    fn update(&mut self, _ctx: &mut GameContext) {}
}

// Usage:
Engine::build()
    .add_plugin(MyPlugin)
    .build()
    .run(MyGame);
```

There are no `AudioPlugin`, `InputPlugin` or `EditorPlugin` types: audio, input and
the editor are feature flags the engine wires directly, not plugins. The editor
exposes `EditorRuntime` (`crates/amigo_editor/src/plugin.rs`).
