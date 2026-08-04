# Specifications

> Links point at the repository rather than at relative paths: the published wiki
> is a separate repo with no `docs/` tree, so `../../docs/specs/...` links were
> dead there. `docs/specs/index.md` in the repo is the authoritative index and
> lists every spec; this page is a curated entry point.

All engine modules are documented as specs under `docs/specs/`. Each spec has a status (`done`, `draft`, `spec`) and lists its dependencies.

## Engine

| Spec | Description |
|------|-------------|
| [Core](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/core.md) | ECS, fixed-point math, save system |
| [Rendering](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/rendering.md) | wgpu renderer, sprites, camera |
| [Audio](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/audio.md) | Kira-based audio system |
| [Input](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/input.md) | Keyboard, mouse, gamepad |
| [Tilemap](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/tilemap.md) | Tilemap structures, autotiling |
| [Pathfinding](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/pathfinding.md) | A*, flow fields |
| [Animation](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/animation.md) | Sprite animation state machine |
| [Camera](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/camera.md) | Camera system, scrolling |
| [UI](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/ui.md) | Immediate-mode pixel UI |
| [Networking](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/networking.md) | Multiplayer transport |
| [Memory](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/memory-performance.md) | Performance optimization |
| [Plugins](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/plugin-system.md) | Plugin system |

## Engine (extended)

| Spec | Description |
|------|-------------|
| [Fog of War](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/fog-of-war.md) | Visibility system |
| [Steering](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/steering.md) | Steering behaviors |
| [Spline](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/spline.md) | Spline curves |
| [Tween](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/tween.md) | Tween animations |
| [Positional Audio](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/positional-audio.md) | 2D positional audio |
| [Bullet Patterns](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/bullet-patterns.md) | Danmaku system |
| [Procedural](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/procedural.md) | Procedural generation |
| [Dialogue](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/dialogue.md) | Dialogue system |
| [Localization](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/localization.md) | i18n |
| [Timeline](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/timeline.md) | Cutscene timeline |
| [Behavior Trees](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/behavior-tree.md) | AI behavior trees |
| [Minimap](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/minimap.md) | Minimap widget |
| [State Rewind](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/state-rewind.md) | Rewind mechanic |
| [Achievements](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/achievements.md) | Achievement system |
| [Physics](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/physics.md) | Physics simulation |
| [Font Rendering](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/font-rendering.md) | Pixel font rendering |
| [GPU Instancing](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/gpu-instancing.md) | Batch rendering |
| [Modding](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/modding.md) | Mod support |
| [Accessibility](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/engine/accessibility.md) | Accessibility features |

## Game Types

| Spec | Description |
|------|-------------|
| [Platformer](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/platformer.md) | Jump buffer, coyote time, wall-slide |
| [Roguelike](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/roguelike.md) | Procgen, permadeath, loot |
| [Shmup](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/shmup.md) | Hitboxes, graze, rank |
| [RTS](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/rts.md) | Units, formations, resources |
| [Metroidvania](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/metroidvania.md) | Abilities, backtracking |
| [Visual Novel](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/visual-novel.md) | Dialogue, choices |
| [Puzzle](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/puzzle.md) | Grid-based, match-3 |
| [City Builder](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/gametypes/city-builder.md) | Zones, resources |

## Assets & Tooling

| Spec | Description |
|------|-------------|
| [Asset Format](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/assets/format.md) | .amigo file format |
| [Asset Pipeline](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/assets/pipeline.md) | Import/export |
| [Atlas](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/assets/atlas.md) | Texture atlas packing |
| [CLI](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/tooling/cli.md) | amigo CLI |
| [Setup](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/tooling/setup.md) | Python toolchain setup |
| [Editor](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/tooling/editor.md) | Level editor |
| [Debug](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/tooling/debug.md) | Debug overlay |

## AI Pipelines

| Spec | Description |
|------|-------------|
| [Art Gen](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/ai-pipelines/artgen.md) | ComfyUI sprite generation |
| [Audio Gen](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/ai-pipelines/audiogen.md) | ACE-Step music generation |
| [Tidal Pipeline](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/ai-pipelines/tidal-pipeline.md) | Audio-to-TidalCycles |
| [Agent API](https://github.com/amigo-labs/amigo-engine/blob/main/docs/specs/ai-pipelines/agent-api.md) | Claude MCP integration |
