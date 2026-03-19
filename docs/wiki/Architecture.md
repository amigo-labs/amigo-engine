# Architecture

## Engine Crates

| Crate | Description |
|-------|-------------|
| `amigo_core` | Fixed-point math, ECS, save system, scheduling, game-type presets |
| `amigo_render` | wgpu renderer, camera, sprite batching, particles, lighting, post-processing |
| `amigo_input` | Keyboard, mouse, gamepad input, action mapping |
| `amigo_assets` | Asset loading, Aseprite import, hot-reloading, atlas packing |
| `amigo_tilemap` | Tilemap data structures, autotiling, collision layers |
| `amigo_animation` | Sprite animation state machine |
| `amigo_scene` | Scene stack and transitions |
| `amigo_ui` | Immediate-mode pixel UI for HUD and menus |
| `amigo_net` | Networking / multiplayer transport |
| `amigo_debug` | FPS overlay, system profiling, visual debug toggles |
| `amigo_audio` | Audio playback via kira (feature-gated) |
| `amigo_tidal_parser` | TidalCycles mini-notation parser and pattern evaluator |
| `amigo_audio_pipeline` | Audio-to-TidalCycles conversion pipeline (Demucs, Basic Pitch) |
| `amigo_editor` | Built-in level editor with Tidal Playground |
| `amigo_api` | Public API surface for plugins |
| `amigo_engine` | Top-level crate that re-exports everything |
| `amigo_steering` | Pathfinding, steering behaviors, flow fields |

## Tool Crates

| Crate | Description |
|-------|-------------|
| `amigo_cli` | CLI for scaffolding, setup, and audio pipeline |
| `amigo_mcp` | Claude MCP integration |
| `amigo_artgen` | Art generation via ComfyUI |
| `amigo_audiogen` | Audio generation via ACE-Step |

## Game-Type Presets

Die Engine liefert vorkonfigurierte Game-Type-Module in `amigo_core`:

| Preset | Features |
|--------|----------|
| Platformer | Jump buffer, coyote time, variable jump, wall-slide, dash |
| Shmup | Hitboxes, graze, bomb/deathbomb, score chain, rank system |
| Roguelike | Procgen, permadeath, loot, difficulty scaling |
| RTS | Unit selection, formations, resource management, building |
| Puzzle | Grid-based, match-3, sliding blocks |
| Metroidvania | Progression-locked abilities, backtracking |
| Visual Novel | Dialogue trees, choices, flag system |
| City Builder | Grid placement, zones, resource management |

## Workspace-Struktur

```
amigo-engine/
  crates/              # Engine library crates
  tools/               # CLI and external tool wrappers
  examples/            # Demo projects
  docs/
    specs/             # Module specifications
    wiki/              # GitHub Wiki source (auto-synced)
  .github/workflows/   # CI, Release, Wiki-Sync
```
