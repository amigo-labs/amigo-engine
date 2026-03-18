---
status: draft
last_updated: 2026-03-16
---

# Amigo Engine -- Conventions

## Purpose

Cross-cutting conventions that apply to all modules.

## Rust Patterns

### Error Handling

Error handling uses three layers:

**Engine init** (wgpu, window, audio): `Result<T, EngineError>`. Fatal if fails, clear message to user.

**Game loop** (update/draw): No `Result` in hot path. Asset errors are graceful (fallback sprite, log warning). Panics only for real programming bugs.

**Asset loading**: `Result<T, AssetError>`. Dev mode: warning with fuzzy-match suggestion (`"playe" -> did you mean "player"?`). Release mode: fallback magenta rect, silent log.

```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("GPU initialization failed: {0}")]
    GpuInit(#[from] wgpu::Error),
    #[error("Asset not found: {path} (did you mean '{suggestion}'?)")]
    AssetNotFound { path: String, suggestion: Option<String> },
    #[error("Asset parse error: {path}: {reason}")]
    AssetParse { path: String, reason: String },
}
```

### Logging

`tracing` crate with structured logging. Configurable via environment variable:

```bash
AMIGO_LOG=debug amigo run              # all debug and above
AMIGO_LOG=amigo_render=trace amigo run # only renderer trace
```

Integrates with Tracy for performance profiling.

## Spec Conventions

- Each spec uses the template from this document
- Status flow: draft -> ready -> implementing -> stable
- A spec is "ready" when all dependencies are at least "ready"
- The Public API section is the contract: implement exactly that, nothing more

## Naming

- Crates: snake_case with amigo_ prefix
- Traits: PascalCase, descriptive (AudioMixer, TileRenderer)
- Config files: kebab-case.toml / kebab-case.ron

## Design Decisions (Appendix)

### A.1 ECS Serialization

SparseSet serializes only `dense_ids` + `dense_data` vectors. The `sparse` index array is rebuilt on deserialization. Change tracking bitsets are not serialized (irrelevant after load).

### A.2 Hybrid World Storage

Core engine components (`Position`, `Velocity`, `Health`, `SpriteComp`) are statically typed fields on `World` for zero-overhead access. Game-specific components (`TowerData`, `Poisoned`, `LootTable`, etc.) use dynamic storage via `HashMap<TypeId, Box<dyn AnyStorage>>`. Game code uses `world.get_dynamic::<T>(id)` for dynamic components.

### A.3 Command -> ECS Translation

`GameCommand` variants are the network/replay-safe high-level API. The server's `execute_command()` function translates each command into ECS operations (spawn, add components, despawn). Commands travel over network and are logged for replays. ECS operations are never serialized or sent.

### A.4 Plugin Borrow Resolution

Plugins receive `(&mut World, &mut Resources)` as separate parameters. `World` holds ECS data, `Resources` holds engine systems (AudioManager, InputState, AssetManager, TimeInfo, EventQueues). No borrow conflict between the two.

### A.5 Event Double-Buffer

Two `Vec<T>` per event type. Current tick writes to the write-buffer, all systems read from the read-buffer (previous tick's events). At tick end: clear read-buffer, swap. Events live exactly one tick for reading. One tick delay (~16ms at 60fps) -- not perceptible.

### A.6 Render Pipeline Stages

Fixed order, configurable per stage: Background (parallax) -> Tilemap (cached chunks) -> Entities (sprite batcher, per-sprite shader) -> Particles (additive blend) -> Lighting (ambient + point lights + normal maps) -> Post-Processing Stack (bloom, chromatic aberration, color grading, vignette, custom) -> UI (no post-processing). Post-processing stack is a `Vec<PostEffect>` configurable per world via RON.

### A.7 Save System

Engine provides: slot management (configurable count), autosave (rotating N slots at configurable interval), quicksave/quickload, platform-aware paths (AppData on Windows, ~/.local/share on Linux), LZ4 compression, CRC corruption check, `SlotInfo` metadata (timestamp, play time, label) without loading full save. Game provides: the `SaveData` struct content.
