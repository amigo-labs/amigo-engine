# Contributing to Amigo Engine

Thanks for your interest in contributing! This document covers the conventions
and workflow for getting changes merged.

## Getting started

```sh
git clone https://github.com/amigo-labs/amigo-engine.git
cd amigo-engine
cargo build --workspace
cargo test --workspace
```

Rust **stable** (latest) is the minimum supported toolchain.

## Project layout

```
crates/
  amigo_core/       # ECS, math, physics, pathfinding, genre modules
  amigo_render/     # wgpu renderer, camera, particles, lighting
  amigo_engine/     # Top-level crate (game loop, config, plugin system)
  amigo_editor/     # Built-in level editor
  amigo_input/      # Keyboard, mouse, gamepad
  amigo_assets/     # Asset loading, hot reload
  amigo_tilemap/    # Tilemap structures
  amigo_animation/  # Sprite animation state machine
  amigo_scene/      # Scene stack and transitions
  amigo_ui/         # Immediate-mode pixel UI
  amigo_net/        # Multiplayer transport
  amigo_debug/      # Debug overlay, profiling
  amigo_audio/      # Audio (kira, feature-gated)
  amigo_api/        # JSON-RPC API server
tools/
  amigo_cli/        # Project scaffolding CLI
  amigo_mcp/        # MCP bridge
  amigo_artgen/     # ComfyUI art generation
  amigo_audiogen/   # Audio generation
examples/
  starter/          # Starter template project
  particles/        # Particle system demo
  tilemap_demo/     # Tilemap rendering + camera scrolling
  audio_demo/       # Audio playback, crossfade, volume
  ecs_demo/         # ECS spawn/despawn, bouncing balls
  input_demo/       # Keyboard, mouse, gamepad visualization
  animation_demo/   # Sprite animation state machine
  pathfinding_demo/ # A* pathfinding + flow fields
docs/               # Specs, guides, architecture docs
```

## Feature flags

| Flag      | Crate          | Purpose                                |
|-----------|----------------|----------------------------------------|
| `audio`   | amigo_engine   | Enable audio playback (kira)           |
| `editor`  | amigo_engine   | Enable egui editor overlay             |
| `api`     | amigo_engine   | Enable JSON-RPC API + headless mode    |
| `tracy`   | amigo_engine   | Enable Tracy profiler integration      |
| `td`      | amigo_core     | Enable tower-defense genre modules     |

Default features: `audio`, `input`.

## Development workflow

1. **Fork & branch** — create a feature branch from `main`.
2. **Make changes** — keep commits focused; one logical change per commit.
3. **Check** — run `cargo check --workspace` and `cargo test --workspace`.
4. **Clippy** — run `cargo clippy --workspace` and fix any warnings.
5. **Format** — run `cargo fmt --all` before committing.
6. **PR** — open a pull request against `main` with a clear description.

### Running with feature flags

```sh
# Editor mode
cargo run --features editor

# With API server
cargo run --features api

# Tracy profiling
cargo run --features tracy

# All features
cargo run --features "editor,api,tracy"
```

## Code conventions

### General

- **Edition 2021**, stable Rust.
- Prefer simple, direct code. Avoid premature abstraction.
- Use `tracing` for logging (`info!`, `warn!`, `error!`), not `println!`.
- Public items should have doc comments. Internal helpers don't need them.

### ECS & simulation

- All game-logic math uses **fixed-point** (`Fix` / `SimVec2`). Never use `f32`
  in deterministic simulation code.
- `f32` is only for rendering (`RenderVec2`), particles, camera, and UI.
- Entity IDs are `EntityId` (generational). Never store raw indices.

### Rendering

- Sprites go through the `SpriteBatcher`. Don't issue raw wgpu draw calls.
- The renderer uses a virtual resolution (pixel-art scale). Screen resolution
  is handled by the camera.

### Feature gating

- New optional subsystems should be behind a feature flag.
- Use `#[cfg(feature = "...")]` at the module level, not scattered through code.
- Ensure the crate compiles with and without the feature.

## Adding a new crate

1. Create the crate under `crates/` (or `tools/` for binaries).
2. Add it to the workspace `members` list in the root `Cargo.toml`.
3. Add shared dependencies to `[workspace.dependencies]`.
4. Re-export from `amigo_engine` if it's a core engine crate.
5. Add commonly used types to the `prelude` module.

## Testing

```sh
# All tests
cargo test --workspace

# Specific crate
cargo test -p amigo_core

# Specific test
cargo test -p amigo_core -- pathfinding::tests
```

Tests live in `#[cfg(test)] mod tests` blocks within each module. Integration
tests can go in `tests/` directories within each crate.

## API Documentation

Build and browse the API reference locally:

```sh
# Build all docs and open in browser
cargo doc --workspace --no-deps --open

# With all features (incl. editor, api, tracy)
cargo doc --workspace --no-deps --all-features --open
```

CI automatically builds docs on every PR and fails on doc warnings.

## Commit messages

Use short, imperative-mood summaries:

```
Add flow field Dijkstra pathfinding
Fix sprite batch sorting for transparency
Update egui to 0.31
```

Prefix with `feat:`, `fix:`, `chore:`, `docs:` when it helps clarity, but
it's not required.

## License

By contributing, you agree that your contributions will be licensed under the
same dual license as the project: MIT OR Apache-2.0.
