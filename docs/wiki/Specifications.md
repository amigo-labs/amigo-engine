# Specifications

Alle Engine-Module sind als Specs dokumentiert unter `docs/specs/`. Jede Spec hat einen Status (`done`, `draft`, `spec`) und zeigt Abhaengigkeiten.

## Engine

| Spec | Beschreibung |
|------|-------------|
| [Core](../../docs/specs/engine/core.md) | ECS, Fixed-Point Math, Save System |
| [Rendering](../../docs/specs/engine/rendering.md) | wgpu Renderer, Sprites, Camera |
| [Audio](../../docs/specs/engine/audio.md) | Kira-basiertes Audio-System |
| [Input](../../docs/specs/engine/input.md) | Keyboard, Mouse, Gamepad |
| [Tilemap](../../docs/specs/engine/tilemap.md) | Tilemap-Strukturen, Autotiling |
| [Pathfinding](../../docs/specs/engine/pathfinding.md) | A*, Flow Fields |
| [Animation](../../docs/specs/engine/animation.md) | Sprite Animation State Machine |
| [Camera](../../docs/specs/engine/camera.md) | Kamera-System, Scrolling |
| [UI](../../docs/specs/engine/ui.md) | Immediate-Mode Pixel UI |
| [Networking](../../docs/specs/engine/networking.md) | Multiplayer Transport |
| [Memory](../../docs/specs/engine/memory-performance.md) | Performance-Optimierung |
| [Plugins](../../docs/specs/engine/plugin-system.md) | Plugin-System |

## Engine (erweitert)

| Spec | Beschreibung |
|------|-------------|
| [Fog of War](../../docs/specs/engine/fog-of-war.md) | Sichtbarkeits-System |
| [Steering](../../docs/specs/engine/steering.md) | Steering Behaviors |
| [Spline](../../docs/specs/engine/spline.md) | Spline-Kurven |
| [Tween](../../docs/specs/engine/tween.md) | Tween-Animationen |
| [Positional Audio](../../docs/specs/engine/positional-audio.md) | 2D Positional Audio |
| [Bullet Patterns](../../docs/specs/engine/bullet-patterns.md) | Danmaku-System |
| [Procedural](../../docs/specs/engine/procedural.md) | Procedural Generation |
| [Dialogue](../../docs/specs/engine/dialogue.md) | Dialogsystem |
| [Localization](../../docs/specs/engine/localization.md) | i18n |
| [Timeline](../../docs/specs/engine/timeline.md) | Cutscene-Timeline |
| [Behavior Trees](../../docs/specs/engine/behavior-tree.md) | AI Behavior Trees |
| [Minimap](../../docs/specs/engine/minimap.md) | Minimap-Widget |
| [State Rewind](../../docs/specs/engine/state-rewind.md) | Rewind-Mechanik |
| [Achievements](../../docs/specs/engine/achievements.md) | Achievement-System |
| [Physics](../../docs/specs/engine/physics.md) | Physik-Simulation |
| [Font Rendering](../../docs/specs/engine/font-rendering.md) | Pixel-Font Rendering |
| [GPU Instancing](../../docs/specs/engine/gpu-instancing.md) | Batch-Rendering |
| [Modding](../../docs/specs/engine/modding.md) | Mod-Support |
| [Accessibility](../../docs/specs/engine/accessibility.md) | Barrierefreiheit |

## Game Types

| Spec | Beschreibung |
|------|-------------|
| [Platformer](../../docs/specs/gametypes/platformer.md) | Jump Buffer, Coyote Time, Wall-Slide |
| [Roguelike](../../docs/specs/gametypes/roguelike.md) | Procgen, Permadeath, Loot |
| [Shmup](../../docs/specs/gametypes/shmup.md) | Hitboxes, Graze, Rank |
| [RTS](../../docs/specs/gametypes/rts.md) | Units, Formations, Resources |
| [Metroidvania](../../docs/specs/gametypes/metroidvania.md) | Abilities, Backtracking |
| [Visual Novel](../../docs/specs/gametypes/visual-novel.md) | Dialogue, Choices |
| [Puzzle](../../docs/specs/gametypes/puzzle.md) | Grid-based, Match-3 |
| [City Builder](../../docs/specs/gametypes/city-builder.md) | Zones, Resources |

## Assets & Tooling

| Spec | Beschreibung |
|------|-------------|
| [Asset Format](../../docs/specs/assets/format.md) | .amigo Dateiformat |
| [Asset Pipeline](../../docs/specs/assets/pipeline.md) | Import/Export |
| [Atlas](../../docs/specs/assets/atlas.md) | Texture Atlas Packing |
| [CLI](../../docs/specs/tooling/cli.md) | amigo CLI |
| [Setup](../../docs/specs/tooling/setup.md) | Python-Toolchain Setup |
| [Editor](../../docs/specs/tooling/editor.md) | Level-Editor |
| [Debug](../../docs/specs/tooling/debug.md) | Debug-Overlay |

## AI Pipelines

| Spec | Beschreibung |
|------|-------------|
| [Art Gen](../../docs/specs/ai-pipelines/artgen.md) | ComfyUI Sprite-Generierung |
| [Audio Gen](../../docs/specs/ai-pipelines/audiogen.md) | ACE-Step Musik-Generierung |
| [Tidal Pipeline](../../docs/specs/ai-pipelines/tidal-pipeline.md) | Audio-to-TidalCycles |
| [Agent API](../../docs/specs/ai-pipelines/agent-api.md) | Claude MCP Integration |
