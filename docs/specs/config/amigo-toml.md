---
status: draft
depends_on: []
last_updated: 2026-03-16
---

# Configuration: amigo.toml

## Purpose

Defines the engine configuration file `amigo.toml` and the three-layer configuration system used across the Amigo Engine.

## Public API

Configuration is loaded at engine startup and accessible via the engine builder and runtime:

```rust
Engine::build()
    .title("My Game")
    .virtual_resolution(480, 270)
    .build()
    .run(MyGame);
```

Overridable via CLI flags:

```bash
amigo run --fullscreen
```

Saved when player changes settings.

## Behavior

### Three Configuration Layers

| Layer | Format | File | Hot Reload | Purpose |
|-------|--------|------|------------|---------|
| Engine | TOML | `amigo.toml` | No (restart) | Window, rendering, audio hardware, dev settings |
| Input | RON | `input.ron` | Yes | Key/gamepad bindings, rebindable by player |
| Game Data | RON | `assets/data/*.ron` | Yes (dev mode) | Tower stats, wave configs, enemy definitions |

### Engine Config Example

```toml
# amigo.toml
[window]
title = "Pirate TD"
width = 1280
height = 720
fullscreen = false
vsync = true

[render]
virtual_width = 480
virtual_height = 270
scale_mode = "pixel_perfect"

[audio]
master_volume = 0.8
sfx_volume = 1.0
music_volume = 0.6

[dev]
hot_reload = true
debug_overlay = true
api_server = false
api_port = 9999
```

### Art Generation Config

```toml
[artgen]
server = "http://localhost:8188"
timeout = 120                        # seconds per generation
output_dir = "assets/generated"      # where results land
```

### Audio Generation Config

```toml
[audiogen]
acestep_server = "http://localhost:7860"    # ACE-Step Gradio API
audiogen_server = "http://localhost:7861"    # AudioGen API (if separate)
output_dir = "assets/audio/generated"
```

### GPU Scheduling Config

```toml
[gpu]
# Explicit mode: only one GPU consumer at a time
# artgen and audiogen check this lock before starting inference
lock_file = "/tmp/amigo_gpu.lock"
timeout = 300                          # seconds before lock is considered stale
```

### Logging Config

Logging is configured via environment variable, not `amigo.toml`:

```bash
AMIGO_LOG=debug amigo run              # all debug and above
AMIGO_LOG=amigo_render=trace amigo run # only renderer trace
```

## Internal Design

The TOML configuration is parsed at startup into strongly-typed Rust structs using `serde`. Values not present in the file use sensible defaults. CLI flags override file values.

The engine config is intentionally not hot-reloadable -- changes require a restart. This avoids complexity around reinitializing the window, GPU context, or audio hardware at runtime.

Input and game data configs use RON and are hot-reloadable via file watchers (see [assets/pipeline](../assets/pipeline.md)).

## Non-Goals

- GUI config editor (use text editor or in-game settings menu)
- Environment variable overrides for all settings (only logging uses env vars)
- Config file versioning or migration

## Open Questions

- Whether to support multiple config profiles (e.g., `amigo.dev.toml`, `amigo.release.toml`)
- User preferences storage path (platform-aware: AppData on Windows, ~/.local/share on Linux)
- Whether `[dev]` section should be stripped from release builds' config
