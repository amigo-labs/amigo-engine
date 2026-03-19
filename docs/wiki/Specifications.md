# Specifications

All engine modules are documented as specs under `docs/specs/`. Each spec has a status (`done`, `draft`, `spec`) and lists its dependencies.

## Engine

| Spec | Description |
|------|-------------|
| [Core](../../docs/specs/engine/core.md) | ECS, fixed-point math, save system |
| [Rendering](../../docs/specs/engine/rendering.md) | wgpu renderer, sprites, camera |
| [Audio](../../docs/specs/engine/audio.md) | Kira-based audio system |
| [Input](../../docs/specs/engine/input.md) | Keyboard, mouse, gamepad |
| [Tilemap](../../docs/specs/engine/tilemap.md) | Tilemap structures, autotiling |
| [Pathfinding](../../docs/specs/engine/pathfinding.md) | A*, flow fields |
| [Animation](../../docs/specs/engine/animation.md) | Sprite animation state machine |
| [Camera](../../docs/specs/engine/camera.md) | Camera system, scrolling |
| [UI](../../docs/specs/engine/ui.md) | Immediate-mode pixel UI |
| [Networking](../../docs/specs/engine/networking.md) | Multiplayer transport |
| [Memory](../../docs/specs/engine/memory-performance.md) | Performance optimization |
| [Plugins](../../docs/specs/engine/plugin-system.md) | Plugin system |

## Engine (extended)

| Spec | Description |
|------|-------------|
| [Fog of War](../../docs/specs/engine/fog-of-war.md) | Visibility system |
| [Steering](../../docs/specs/engine/steering.md) | Steering behaviors |
| [Spline](../../docs/specs/engine/spline.md) | Spline curves |
| [Tween](../../docs/specs/engine/tween.md) | Tween animations |
| [Positional Audio](../../docs/specs/engine/positional-audio.md) | 2D positional audio |
| [Bullet Patterns](../../docs/specs/engine/bullet-patterns.md) | Danmaku system |
| [Procedural](../../docs/specs/engine/procedural.md) | Procedural generation |
| [Dialogue](../../docs/specs/engine/dialogue.md) | Dialogue system |
| [Localization](../../docs/specs/engine/localization.md) | i18n |
| [Timeline](../../docs/specs/engine/timeline.md) | Cutscene timeline |
| [Behavior Trees](../../docs/specs/engine/behavior-tree.md) | AI behavior trees |
| [Minimap](../../docs/specs/engine/minimap.md) | Minimap widget |
| [State Rewind](../../docs/specs/engine/state-rewind.md) | Rewind mechanic |
| [Achievements](../../docs/specs/engine/achievements.md) | Achievement system |
| [Physics](../../docs/specs/engine/physics.md) | Physics simulation |
| [Font Rendering](../../docs/specs/engine/font-rendering.md) | Pixel font rendering |
| [GPU Instancing](../../docs/specs/engine/gpu-instancing.md) | Batch rendering |
| [Modding](../../docs/specs/engine/modding.md) | Mod support |
| [Accessibility](../../docs/specs/engine/accessibility.md) | Accessibility features |

## Game Types

| Spec | Description |
|------|-------------|
| [Platformer](../../docs/specs/gametypes/platformer.md) | Jump buffer, coyote time, wall-slide |
| [Roguelike](../../docs/specs/gametypes/roguelike.md) | Procgen, permadeath, loot |
| [Shmup](../../docs/specs/gametypes/shmup.md) | Hitboxes, graze, rank |
| [RTS](../../docs/specs/gametypes/rts.md) | Units, formations, resources |
| [Metroidvania](../../docs/specs/gametypes/metroidvania.md) | Abilities, backtracking |
| [Visual Novel](../../docs/specs/gametypes/visual-novel.md) | Dialogue, choices |
| [Puzzle](../../docs/specs/gametypes/puzzle.md) | Grid-based, match-3 |
| [City Builder](../../docs/specs/gametypes/city-builder.md) | Zones, resources |

## Assets & Tooling

| Spec | Description |
|------|-------------|
| [Asset Format](../../docs/specs/assets/format.md) | .amigo file format |
| [Asset Pipeline](../../docs/specs/assets/pipeline.md) | Import/export |
| [Atlas](../../docs/specs/assets/atlas.md) | Texture atlas packing |
| [CLI](../../docs/specs/tooling/cli.md) | amigo CLI |
| [Setup](../../docs/specs/tooling/setup.md) | Python toolchain setup |
| [Editor](../../docs/specs/tooling/editor.md) | Level editor |
| [Debug](../../docs/specs/tooling/debug.md) | Debug overlay |

## AI Pipelines

| Spec | Description |
|------|-------------|
| [Art Gen](../../docs/specs/ai-pipelines/artgen.md) | ComfyUI sprite generation |
| [Audio Gen](../../docs/specs/ai-pipelines/audiogen.md) | ACE-Step music generation |
| [Tidal Pipeline](../../docs/specs/ai-pipelines/tidal-pipeline.md) | Audio-to-TidalCycles |
| [Agent API](../../docs/specs/ai-pipelines/agent-api.md) | Claude MCP integration |
