---
status: done
crate: amigo_plugin
depends_on: ["engine/core"]
last_updated: 2026-03-16
---

# Plugin System

## Purpose

Compile-time plugin architecture for composing engine features. No dynamic plugin loading, no runtime discovery. Compile-time decides what's included, Plugin Trait provides clean initialization order and update lifecycle.

## Public API

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

### Plugin Trait (Structure)

Each `amigo_*` crate exposes a Plugin with a clean lifecycle:

```rust
pub trait Plugin {
    fn build(&self, engine: &mut EngineBuilder);
    fn update(&mut self, ctx: &mut GameContext);
}

// Usage:
Engine::build()
    .add_plugin(AudioPlugin)
    .add_plugin(InputPlugin)
    #[cfg(feature = "editor")]
    .add_plugin(EditorPlugin)
    .build()
    .run(MyGame);
```
