# Amigo Engine – Complete Specification

## Architecture, Pipelines & Tools – v2.0

**Unified spec document. Contains: Engine Core, Audio Pipeline, Art Pipeline, AI Interfaces, Editor, Starter Template.**

### Table of Contents

| # | Section | Topic |
|---|---------|-------|
| 1 | Vision & Philosophy | Why this engine exists |
| 2 | Tech Stack | Rust, wgpu, kira, egui, dependencies |
| 3 | Architecture Overview | Workspace, crate structure, layers |
| 4 | Core Types, Math & ECS | SparseSet, fixed-point, change tracking |
| 5 | Rendering Pipeline | 7 fixed stages, shaders, post-processing |
| 6 | Memory & Performance | Budgets, pooling, profiling |
| 7 | API Design | Game trait, engine builder |
| 8 | Command System & Networking | Serializable commands, lockstep |
| 9 | Asset Pipeline | Dev (loose) vs Release (packed), hot reload |
| 10 | Tilemap System | Orthogonal, isometric, chunks, auto-tiling |
| 11 | Pathfinding | A*, waypoints, flow fields |
| 12 | Animation System | Aseprite, frame-based |
| 13 | Camera System | Follow, clamp, shake |
| 14 | Input System | Keyboard, gamepad, rebinding |
| 15 | Audio System | SFX, adaptive music engine, ambient |
| 15b | Audio Generation Pipeline | ACE-Step, AudioGen, stems, MCP tools |
| 16 | Level Editor | Tile painter, entity placement, undo/redo |
| 16b | Art Studio | Managed ComfyUI, workflow templates, egui panel |
| 16c | Art Generation Pipeline | amigo_artgen MCP, style definitions, post-processing |
| 17 | AI Agent Interface | amigo_api (JSON-RPC) + amigo_mcp |
| 18 | Debug & Profiling | Overlays, Tracy, F-keys |
| 19 | Build & Distribution | amigo CLI, pack, release |
| 20 | Plugin System | Feature flags, Plugin trait |
| 21 | UI System | Dual layer: Pixel UI (game) + egui (editor) |
| 22 | Error Handling & Logging | thiserror, tracing, graceful fallbacks |
| 23 | Configuration | amigo.toml, input.ron, data/*.ron |
| 24 | Starter Template | `amigo new`, examples/starter |
| 25 | Game-Specific Design | References to game specs |
| 26 | Implementation Phases | Phase 0-2, 4-8 timeline (content in game repo) |
| 27 | Key Decisions Summary | All decisions in one table |
| A | Appendix | Detailed design rationale |

---

## 1. Vision & Philosophy

Amigo is a **Modern Pixel Art Game Engine** built in Pure Rust. It treats pixel art as an aesthetic choice, not a technical limitation. No artificial color limits, no forced palette constraints, no layer caps. The engine enables the kind of games that define modern pixel art: Celeste's particle effects and screen shake, Dead Cells' skeletal animation, Hyper Light Drifter's dynamic lighting – all built on a pixel grid with the full power of modern GPUs.

### Core Principles

- **Pixel art is aesthetic, not limitation.** Unlimited colors, alpha transparency, blend modes, shader effects. The pixel grid is the only constraint.
- **One language, one toolchain.** Pure Rust. Game logic in Rust, data in RON/TOML (hot-reloadable). The Rust compiler is the feedback loop. No scripting layer.
- **Opinionated defaults, escape hatches when needed.** `draw_sprite("player", pos)` works out of the box. Typed handles exist for performance-critical paths.
- **Multiplayer-ready from day 1.** Client-Server architecture even in singleplayer. Deterministic simulation via Fixed-Point arithmetic. Serializable game state and command-based input.
- **AI-native development.** First-class IPC interface for AI agents (Claude Code). The engine can be observed, controlled, and debugged programmatically. Screenshot-based visual feedback, headless simulation, and a persistent command API allow AI to build levels, balance gameplay, run playtests, and debug issues autonomously.

### First Game

Tower Defense with 6 thematic worlds: Pirates of the Caribbean, Lord of the Rings, Dune, Matrix, Game of Thrones, Stranger Things. Each world has unique pixel art tilesets, tower types, enemy types, and atmospheric effects.

### Target Genres

The engine supports all classic 2D pixel art genres: Tower Defense, Platformer/Jump'n'Run, RPG/Adventure, Shmup/Shoot'em Up, Beat'em Up, Puzzle, Fighting Games, Run'n'Gun, Metroidvania.

---

## 2. Tech Stack

### Language & Toolchain

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust (latest stable) | Performance, safety, compiler feedback for AI dev |
| Build | Cargo | Standard Rust toolchain |
| Data Format | RON (primary), TOML (config) | Rust-native, readable, hot-reloadable |
| Linker (dev) | mold | Fast incremental builds (1-3s) |

### Core Dependencies (Crates)

| Crate | Purpose |
|-------|---------|
| `wgpu` | GPU rendering (Vulkan/DX12/Metal/WebGPU) |
| `winit` | Window creation, event loop, input |
| `gilrs` | Gamepad input |
| `kira` | Audio (playback, mixing, spatial, crossfade) |
| `fixed` (I16F16) | Fixed-point arithmetic for deterministic simulation |
| `thiserror` | Ergonomic custom error types |
| `tracing` | Structured logging + Tracy integration |
| `serde` + `serde_ron` | Serialization (state, commands, assets, saves) |
| `notify` | Filesystem watcher for hot reload |
| `bumpalo` | Arena allocator for per-frame temp data |
| `tracy-client` | Performance profiling |
| `asefile` | Aseprite file parsing |
| `fontdue` | TTF font rasterization for Pixel UI |
| `image` | Image loading/processing |
| `laminar` | UDP networking with reliability layer |
| `rustc-hash` | Fast deterministic hashing (FxHashMap) |
| `egui` + `egui-wgpu` | Editor UI at native resolution (behind `editor` feature flag) |

### Target Platforms (Phase 1)

| Platform | Backend | Priority |
|----------|---------|----------|
| Windows | DX12 via wgpu | Primary |
| Linux | Vulkan via wgpu | Primary |
| macOS | Metal via wgpu | Later |
| Web/WASM | WebGPU via wgpu | Later |

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                     Game Code (Rust)                     │
│          Scenes, Systems, Game Logic, AI                 │
├─────────────────────────────────────────────────────────┤
│                  Data Files (RON/TOML)                   │
│        Tower stats, wave configs, level data             │
├──────────┬──────────────────────────────────────────────┤
│ Editor   │              Engine Core                      │
│ (own UI, │  ┌─────────┬──────────┬──────┬────────────┐  │
│  feature │  │Renderer │ Audio    │ Net  │ AI API     │  │
│  flag)   │  │(Batcher)│ (kira)   │(UDP) │ (IPC)      │  │
│          │  ├─────────┼──────────┼──────┼────────────┤  │
│          │  │Tilemap  │ Input    │Colli-│ Pixel UI   │  │
│          │  │System   │ System   │sion  │ System     │  │
│          │  ├─────────┼──────────┼──────┼────────────┤  │
│          │  │Camera   │Animation │Assets│ Commands   │  │
│          │  │System   │System    │      │ & State    │  │
│          │  ├─────────┴──────────┴──────┴────────────┤  │
│          │  │  ECS (SparseSet) + Core Types & Math    │  │
│          │  │  (Fixed-Point, SimVec2, Rect, Color)    │  │
│          │  └─────────────────────────────────────────┘  │
├──────────┴──────────────────────────────────────────────┤
│                    wgpu / winit / gilrs                   │
└─────────────────────────────────────────────────────────┘
```

### Module Structure (Cargo Workspace)

```
amigo-engine/                   # github.com/amigo-labs/amigo-engine
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── amigo_core/              # Math, types, Fixed-Point, SparseSet ECS
│   ├── amigo_render/            # wgpu renderer, sprite batcher, camera
│   ├── amigo_ui/                # Pixel-native UI system (Game HUD + Editor widgets)
│   ├── amigo_audio/             # kira wrapper, audio manager
│   ├── amigo_input/             # Keyboard, mouse, gamepad abstraction
│   ├── amigo_tilemap/           # Tilemap system, collision layers
│   ├── amigo_animation/         # Sprite animation, Aseprite integration
│   ├── amigo_assets/            # Asset loading, hot reload, atlas packing
│   ├── amigo_net/               # Networking, transport trait, commands
│   ├── amigo_scene/             # Scene/state machine
│   ├── amigo_editor/            # Level editor (feature flag: "editor")
│   ├── amigo_api/               # AI/IPC interface (feature flag: "api")
│   ├── amigo_debug/             # Debug overlay, Tracy integration
│   └── amigo_engine/            # Ties everything together, public API
├── tools/
│   ├── amigo_cli/               # CLI: pack, build, release, new project
│   ├── amigo_mcp/               # MCP server wrapping amigo_api for Claude Code
│   ├── amigo_artgen/            # MCP server for AI art generation (ComfyUI)
│   └── amigo_audiogen/          # MCP server for AI audio generation (ACE-Step, AudioGen)
├── examples/
│   └── starter/                 # Starter template (reference + scaffold base)
└── assets/
    └── ...
```

### Client-Server Architecture

```
Singleplayer:
  Client ──fn calls──→ Server    (same process, zero overhead)

Multiplayer:
  Client A ──UDP──→ Server ──UDP──→ Client B
                          ──UDP──→ Client C
```

All player input becomes serializable Commands. The server validates and applies them. This separation enables multiplayer, replays, save/load, and AI control through the same interface.

---

## 4. Core Types, Math & ECS

### Fixed-Point Arithmetic (Simulation)

All simulation uses Q16.16 fixed-point (`I16F16` from the `fixed` crate). Float (`f32`) is used only for rendering.

| Domain | Number Type | Deterministic | Used For |
|--------|-------------|---------------|----------|
| Simulation | `Fix` (I16F16) | Yes | Positions, velocities, health, damage, range, timers, cooldowns, pathfinding |
| Rendering | `f32` | No (irrelevant) | Screen positions, particles, camera, screen shake, UI |
| Data Files | `f32` literals | N/A | Converted to `Fix` on load |

### Key Types

```rust
pub type Fix = I16F16;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimVec2 { pub x: Fix, pub y: Fix }

#[derive(Clone, Copy)]
pub struct RenderVec2 { pub x: f32, pub y: f32 }

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId { index: u32, generation: u32 }
```

### Lightweight ECS (SparseSet + Change Tracking)

The engine uses a custom Lightweight ECS. Not Bevy-style with macros and schedulers – normal Rust code, explicit control flow, but with the flexibility of component composition.

**Storage: SparseSet per Component type.** Each component type gets a SparseSet – a dense array (cache-friendly iteration) with a sparse lookup array (O(1) access by EntityId). No HashMap, no pointer chasing.

```rust
pub struct SparseSet<T> {
    sparse: Vec<u32>,           // EntityId.index → dense index
    dense_ids: Vec<EntityId>,   // dense → EntityId
    dense_data: Vec<T>,         // dense → component data (cache-friendly)
    changed: BitSet,            // tracks mutations this tick
    added: BitSet,              // tracks insertions this tick
    removed: BitSet,            // tracks removals this tick
}
```

**Change Tracking** is automatic. Calling `get_mut()` marks the entity as changed. Systems can query only changed/added/removed entities:

```rust
// All enemies whose health changed this tick
for (id, health) in world.query_changed::<&Health, With<EnemyData>>() { ... }

// All entities that got a StatusEffect this tick
for (id, status) in world.query_added::<&StatusEffect>() { ... }
```

**World struct** holds all component storages:

```rust
pub struct World {
    entities: GenerationalArena,
    positions: SparseSet<Position>,
    velocities: SparseSet<Velocity>,
    healths: SparseSet<Health>,
    // ... one SparseSet per component type
}

impl World {
    pub fn spawn(&mut self) -> EntityId;
    pub fn despawn(&mut self, id: EntityId);
    pub fn add<T>(&mut self, id: EntityId, component: T);
    pub fn get<T>(&self, id: EntityId) -> Option<&T>;
    pub fn get_mut<T>(&mut self, id: EntityId) -> Option<&mut T>; // marks changed
    pub fn remove<T>(&mut self, id: EntityId);
    pub fn query<T: QueryParam>(&self) -> QueryIter<T>;
    pub fn query_changed<T: QueryParam>(&self) -> QueryIter<T>;
    pub fn query_added<T: QueryParam>(&self) -> QueryIter<T>;
    pub fn flush(&mut self); // end-of-tick: process despawns, clear tracking
}
```

**Game code is normal Rust** – no macro magic, no dependency injection:

```rust
fn shooting_system(world: &mut World, audio: &mut AudioManager) {
    let towers: Vec<_> = world.query::<(&Position, &TowerData)>().collect();
    for (id, pos, tower) in &towers {
        for (eid, epos, mut health) in world.query::<(&Position, &mut Health), With<EnemyData>>() {
            if pos.distance(epos) < tower.range {
                health.current -= tower.damage;
            }
        }
    }
}

// Called explicitly, in order you control:
fn update(world: &mut World, ctx: &mut GameContext) {
    movement_system(world);
    shooting_system(world, &mut ctx.audio);
    collision_system(world);
    cleanup_dead_entities(world);
    world.flush();
}
```

### State-Scoped Entity Cleanup (from Bevy, improved)

Entities can be tagged with a game state. When the state changes, tagged entities are automatically despawned:

```rust
world.add(enemy, StateScoped(GameState::Playing));
// When transitioning to GameOver → all StateScoped(Playing) entities auto-despawned
```

### Tick Scheduler

Systems that don't need to run every tick can be scheduled at intervals:

```rust
scheduler.every(10, |world| pathfinding_system(world));  // every 10 ticks
scheduler.every(3, |world| tower_targeting(world));       // every 3 ticks
scheduler.every(60, |world| cleanup_system(world));       // once per second
```

### Determinism Rules

1. Fixed timestep (60 ticks/sec)
2. Fixed-point arithmetic (Q16.16) for all simulation
3. Seeded RNG (`StdRng`) as part of GameState
4. No `HashMap` iteration in simulation (use `BTreeMap` / `IndexMap`)
5. No `f32` in simulation logic
6. All state changes through validated Commands

---

## 5. Rendering Pipeline

- **Backend:** wgpu (Vulkan/DX12/Metal/WebGPU)
- **Sprite Batcher:** Collect all sprites per frame, sort by texture atlas, one draw call per atlas. Target: 5-10 draw calls for a full TD scene.
- **Virtual Resolution:** 640×360 (16:9). Pixel-perfect integer scaling to window/screen size.

**Integer Scaling Table:**

| Display | Scale Factor | Result | Perfect? |
|---------|-------------|--------|----------|
| 1080p (1920×1080) | 3× | 1920×1080 | ✓ Exact fit |
| 1440p (2560×1440) | 4× | 2560×1440 | ✓ Exact fit |
| 4K (3840×2160) | 6× | 3840×2160 | ✓ Exact fit |
| Ultrawide 2560×1080 | 3× vert → 1920×1080 | 640px horizontal spare | Extended viewport |
| Ultrawide 3440×1440 | 4× vert → 2560×1440 | 880px horizontal spare | Extended viewport |

**Ultrawide Handling:** On displays wider than 16:9, the engine extends the horizontal viewport (showing more of the world, up to a configurable max like 853×360 for 21:9). The game world is always larger than the viewport – ultrawide players see more, but gain no gameplay advantage. Vertical resolution stays fixed at 360px.

**Viewport Config (RON):**

```ron
// amigo.toml
[render]
virtual_width = 640
virtual_height = 360
scaling = "integer"              // "integer" or "letterbox"
ultrawide_extend = true          // extend horizontal view on wide displays
ultrawide_max_width = 853        // max virtual width (21:9 equivalent)
```
- **No artificial limits:** Unlimited colors, alpha, blend modes, shaders.

### Layer Model (SNES-inspired)

| Layer | Z-Order | Content |
|-------|---------|---------|
| Background | 0 | Sky, distant scenery (parallax) |
| Terrain | 1 | Tilemap ground layer |
| Decoration (back) | 2 | Behind-entity decorations |
| Entities | 3 | Towers, enemies, projectiles |
| Decoration (front) | 4 | In-front decorations |
| Effects | 5 | Particles, explosions |
| UI | 6 | HUD, menus |
| Debug | 7 | Debug overlay (dev only) |

Each layer has independent scroll factor for parallax.

### Tilemap Rendering

Chunk-based (16×16 tiles) with render texture caching. Only dirty chunks re-rendered. Chunks outside camera frustum culled.

### Modern Effects (optional)

Dynamic lighting (normal maps, point lights), particles (pixel-sized), post-processing (bloom, chromatic aberration, CRT filter), screen shake, hitstop, custom WGSL shaders.

---

## 6. Memory & Performance

### Data-Oriented Design (SoA)

Entity data stored as Structure of Arrays for cache efficiency.

### Allocator Strategy

| Allocator | Use Case |
|-----------|----------|
| Arena (`bumpalo`) | Per-frame temporary data. Reset at frame end. |
| Object Pool | Entities (enemies, projectiles, particles). Pre-allocated. |
| Standard heap | Long-lived data (assets, tilemap, config). Load-time only. |

### Fixed Timestep Game Loop

```
while running:
    accumulate time
    while accumulator >= TICK_DURATION (1/60s):
        gather input → Commands
        transport.send(commands)
        all_commands = transport.receive()
        server.update(game_state, all_commands)
        accumulator -= TICK_DURATION
    render(game_state, interpolation_alpha)
```

### Collision Detection

- **Tile-based:** O(1) lookup per tile
- **AABB:** Entity-vs-entity
- **Spatial Hash:** Broad-phase, grid cells, O(n) instead of O(n²)
- **Trigger Zones:** Non-physical areas that fire events

### Threading

Main thread: game loop + simulation + rendering. Separate threads: audio (kira), asset IO, network, AI API server.

### Profiling

Tracy integration from day 1. In-game debug overlay (own Pixel UI): FPS, entity count, draw calls, memory.

---

## 7. API Design

### Minimal Example

```rust
use amigo::prelude::*;

struct MyGame;

impl Game for MyGame {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        if ctx.input.pressed(Key::Escape) {
            return SceneAction::Quit;
        }
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_sprite("player", vec2(100.0, 50.0));
    }
}

fn main() {
    Engine::build()
        .title("My Game")
        .virtual_resolution(640, 360)
        .build()
        .run(MyGame);
}
```

### Design Principles

- **One Context object** (`GameContext` for update, `DrawContext` for rendering). No ECS macro magic, no dependency injection.
- **No lifetime parameters** in user-facing API. Entities referenced by copy-type `EntityId`.
- **Builder pattern** for setup, short function calls in game loop.
- **String-based** for prototyping (`draw_sprite("name", pos)`), **typed handles** for performance (`draw_sprite_handle(HANDLE, pos)`).
- **`_ex` variant** with closure for extended options: `draw_sprite_ex("x", pos, |s| s.flip_x().tint(RED))`.
- **Events as structs** in a queue (no callbacks, no lifetime issues).
- **Scene/State Machine:** `Push` (overlay), `Pop` (back), `Replace` (transition), `Quit` (exit).
- **Dev mode:** fuzzy matching for asset names. `"playe_walk"` → `"Did you mean 'player_walk'?"`.
- **Release mode:** fallback sprite (magenta rect) for missing assets, never crash.

---

## 8. Command System & Networking

### Commands

All player input becomes serializable commands. No direct state mutation.

```rust
#[derive(Clone, Serialize, Deserialize)]
pub enum GameCommand {
    PlaceTower { pos: IVec2, tower_type: TowerTypeId },
    SellTower { tower_id: EntityId },
    UpgradeTower { tower_id: EntityId, path: UpgradePath },
    StartWave,
    Pause,
    Unpause,
    // ...
}
```

### Transport Trait

```rust
pub trait Transport {
    fn send(&mut self, commands: &[GameCommand]);
    fn receive(&mut self) -> Vec<(PlayerId, Vec<GameCommand>)>;
}

// LocalTransport: singleplayer (zero overhead)
// NetworkTransport: multiplayer (UDP via laminar)
```

### GameState (Fully Serializable)

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct GameState {
    pub tick: u64,
    pub rng: SerializableRng,
    pub gold: i32,
    pub lives: i32,
    pub wave: WaveState,
    pub towers: EntityPool<Tower>,
    pub enemies: EntityPool<Enemy>,
    pub projectiles: EntityPool<Projectile>,
    pub tilemap: TileMap,
}
```

### Multiplayer (Phase 2+)

- **Co-op (2-4 players):** Shared map, lockstep protocol
- **Competitive:** Own maps, send waves to opponent
- **Spectator:** Receive-only

### Replay System

Commands logged with tick numbers. Replay = feed commands into fresh GameState.

---

## 9. Asset Pipeline

### Philosophy

Dev: loose files, hot reload, Aseprite native. Release: packed into `game.pak`.

### Supported Formats

| Asset Type | Dev Format | Tool |
|------------|------------|------|
| Sprites | `.aseprite` (native), `.png` | Aseprite |
| Tilemaps | `.amigo` (engine format) | Amigo Editor |
| Audio SFX | `.wav`, `.ogg` | Audacity/sfxr |
| Audio Music | `.ogg` | Any DAW |
| Data | `.ron`, `.toml` | VS Code |
| Shaders | `.wgsl` | VS Code |

### Aseprite Integration

Engine reads `.aseprite` directly via `asefile`. Tags → named animations, layers → composited, slices → 9-patch UI.

```rust
ctx.draw_sprite_animated("player", "walk_right", pos);
```

### Asset Loading Strategy

**Synchronous loading at startup** – all assets for the current world are loaded into memory before gameplay begins, like a cartridge. No async handle-checking, no "is it loaded yet?" callbacks. `ctx.assets.sprite("player")` always returns immediately. Background async loading only during level/world transitions (loading screen).

### Atlas Pipeline (Dev vs Release)

**Dev mode:** Each Aseprite file / PNG is loaded as an individual texture. More draw calls (20-30 instead of 5), but irrelevant for pixel art performance. Hot reload is trivial – file changes, texture is replaced instantly.

**Release mode:** `amigo pack` (CLI tool) runs bin-packing to combine all sprites into texture atlases. One atlas = one texture = one draw call. The packing logic lives in the CLI tool, not the engine runtime.

The engine has two loaders behind a common `SpriteHandle` – game code is identical in both modes:

```rust
// Game code doesn't know or care about Dev vs Release
ctx.draw_sprite("player", pos);
// Dev: SpriteHandle → individual texture → draw
// Release: SpriteHandle → atlas index + UV rect → draw
```

```rust
// String-based (prototyping)
ctx.draw_sprite("pirates/captain", pos);

// Typed handles (performance, compile-time safe, build-script generated)
ctx.draw_sprite_handle(assets::sprites::CAPTAIN, pos);
```

### Hot Reload (Dev Mode)

File watcher on assets directory. Sprites, configs, levels, audio, shaders all hot-reloadable.

### Asset Packing (Release)

`amigo pack`: sprites → texture atlases, audio → compressed, data → validated, all → `game.pak` (memory-mappable).

---

## 10. Tilemap System

First-class engine feature. Multiple layers, auto-tiling (bitmask-based), animated tiles, collision layer with solid/one-way/slope/trigger types.

### Grid Modes

```rust
pub enum GridMode {
    Orthogonal { tile_width: u32, tile_height: u32 },
    Isometric { tile_width: u32, tile_height: u32 },
}
```

Both modes share the same API -- the engine handles coordinate conversion internally. Isometric rendering sorts tiles back-to-front automatically.

### Chunk Streaming

For large worlds, the tilemap is divided into chunks that load/unload based on camera position. For small levels (TD, Platformer): entire map in memory, no streaming needed. Chunk streaming is opt-in.

### Tilemap API

```rust
let is_solid = tilemap.is_solid(x, y);
tilemap.set(layer, x, y, TileId(42));
tilemap.set_terrain(x, y, TerrainType::Water); // auto-selects variant
```

---

## 11. Pathfinding

Engine-level pathfinding for any genre.

- **A* on Tile Grid:** Configurable 4-way or 8-way movement, max search budget
- **Predefined Waypoint Paths (TD):** Editor-defined waypoints, no dynamic pathfinding
- **Flow Fields (optional):** For horde modes / RTS. Grid-sized field, O(1) per entity lookup. Opt-in.

---

## 12. Animation System

Sprite animations from Aseprite tags. Frame-based with per-frame duration (fixed-point). Looping/one-shot/ping-pong modes. State machine for animation transitions. Phase 2: skeletal animation for large bosses.

---

## 13. Camera System

Pre-built patterns: Fixed, Follow (with deadzone + lookahead), FollowSmooth, ScreenLock (Zelda), RoomTransition (Metroidvania), BossArena, CinematicPan.

Effects: shake (configurable decay), zoom (with easing).

Parallax: each tile layer has independent scroll factor.

---

## 14. Input System

Abstract action mapping (RON-defined). Keyboard, mouse, gamepad (gilrs). API: `pressed()`, `released()`, `held()`, `axis()`, `mouse_pos()`, `mouse_world_pos()`.

---

## 15. Audio System

Wrapper around `kira`. Three subsystems: SFX playback, Adaptive Music Engine, and Ambient layers.

### 15.1 SFX Playback

Per-sound cooldowns, concurrency limits, pitch variance, sound variants. Spatial SFX with distance-based volume falloff.

```rust
res.audio.play_sfx("cannon_fire"); // picks random variant
```

### 15.2 Adaptive Music Engine

Vertical layering (multiple stems synchronized, volume driven by game parameters) + horizontal re-sequencing (bar-synced section transitions) + stingers (one-shot cues quantized to beat/bar).

```rust
pub struct AdaptiveMusicEngine {
    active_section: Option<MusicSection>,
    pending_transition: Option<(String, MusicTransition)>,
    bar_clock: BarClock,
    params: MusicParameters,
    stinger_queue: Vec<StingerRequest>,
}
```

### 15.3 Ambient Layer

Looping environmental audio per world. Crossfades when atmosphere changes.

### 15.4 Volume Channels

Master -> Music / SFX / Ambient. All configurable and saved.

---

## 15b. Audio Generation Pipeline

See `docs/plans/02-asset-pipeline-spec.md` for the complete amigo_audiogen MCP server specification: ACE-Step music generation, AudioGen SFX, adaptive music stems, MCP tools.

---

## 16. Integrated Level Editor

In-engine tool, uses own Pixel UI system, enabled via `--features editor`. Zero overhead in release builds. Toggle with `Tab` between Play and Edit mode.

- **Phase 1:** Tile painter, entity placement, path editor, undo/redo, `.amigo` format
- **Phase 2:** Edit-while-playing, live preview
- **Phase 3:** AI-assisted features (auto-pathing, wave balancing, auto-decoration, heatmaps)

---

## 16b. Art Studio

Managed ComfyUI integration with egui panel for in-editor art generation. See `docs/plans/02-asset-pipeline-spec.md`.

---

## 16c. Art Generation Pipeline

See `docs/plans/02-asset-pipeline-spec.md` for the complete amigo_artgen MCP server specification: ComfyUI workflows, style definitions, post-processing pipeline.

---

## 17. AI Agent Interface (amigo_api + amigo_mcp)

### Architecture (Two Layers)

```
Claude Code <-> MCP (stdio) <-> amigo_mcp <-> JSON-RPC (TCP) <-> amigo_api
```

`amigo_api` is the engine's raw IPC interface (JSON-RPC 2.0). `amigo_mcp` translates MCP protocol to JSON-RPC.

### MCP Tools

**Observation:** screenshot, get_state, list_entities, inspect_entity, perf
**Simulation:** place_tower, sell_tower, upgrade_tower, start_wave, tick (headless), set_speed, pause/unpause, spawn
**Editor:** new_level, paint_tile, fill_rect, place_entity, add_path, auto_decorate, save/load, undo/redo
**Audio:** play, play_music, crossfade, set_volume
**Save/Load/Replay:** save, load, replay_record, replay_play
**Debug:** dump_state, tile_collision, step, state_crc

### Headless Mode

```bash
amigo run --api --headless --level caribbean_01
```

Simulation runs at max CPU speed. 3-minute game in <1 second.

### Event Streaming

Subscribe to events: enemy_killed, wave_complete, tower_fired, game_over.

See `docs/ai-integration.md` for the complete AI integration guide.

---

## 18. Debug & Profiling

- **Debug Overlay (F1):** FPS, entity count, draw calls, memory
- **Visual Debug:** F2 Grid, F3 Collision, F4 Paths, F5 Zones, F6 Perf, F7 Entities, F8 Network
- **Dev Mode:** Hot reload, state snapshots, tick stepping, speed control
- **Tracy:** Integration planned for detailed profiling

---

## 19. Build & Distribution

```bash
amigo new my_game              # scaffold project
amigo run                      # dev build + run
amigo run --api                # with AI API server
amigo run --api --headless     # headless simulation
amigo pack                     # assets -> game.pak
amigo build --release          # optimized binary
amigo release --target windows,linux  # full pipeline
```

Release profile: LTO, single codegen unit, stripped, abort on panic.

---

## 20. Plugin System

Feature flags (compile-time) + Plugin trait (runtime lifecycle).

```rust
pub trait Plugin {
    fn build(&self, ctx: &mut PluginContext);
    fn init(&self, ctx: &mut GameContext) {}
}

Engine::build()
    .add_plugin(AudioPlugin)
    .add_plugin(InputPlugin)
    .build()
    .run(MyGame);
```

---

## 21. UI System (Pixel-Native, Two Tiers)

### Tier 1: Game HUD (always available)

```rust
ui.pixel_text("Gold: 350", pos, Color::GOLD);
if ui.sprite_button("btn_archer", pos) { /* select tower */ }
ui.progress_bar(rect, health / max_health, Color::RED);
```

### Tier 2: Editor Widgets (behind `editor` feature flag)

Text inputs, sliders, dropdowns, color pickers, scrollable containers, tree views.

---

## 22. Error Handling & Logging

- **Engine init:** `Result<T, EngineError>` -- fatal if fails
- **Game loop:** No `Result` in hot path, graceful fallbacks
- **Asset loading:** Dev mode: fuzzy-match suggestion. Release mode: fallback sprite
- **Logging:** `tracing` crate, env-configurable (`AMIGO_LOG=debug`)

---

## 23. Configuration (Three Layers)

| Layer | Format | File | Hot Reload | Purpose |
|-------|--------|------|------------|---------|
| Engine | TOML | `amigo.toml` | No | Window, rendering, audio, dev settings |
| Input | RON | `input.ron` | Yes | Key/gamepad bindings |
| Game Data | RON | `assets/data/*.ron` | Yes (dev) | Tower stats, wave configs, etc. |

---

## 24. Additional Engine Systems (Phase 2+)

Genre-specific engine modules (data-driven, game defines content):

- **`farming.rs`** -- Growth stages, calendar system, farm tile grid
- **`bullet_pattern.rs`** -- Bullet pool, pattern shapes (radial, spiral, aimed, wave), boss sequencer
- **`puzzle.rs`** -- Generic grid, pattern matching, move system, block spawning
- **`platformer.rs`** -- Jump buffering, coyote time, variable jump, wall interactions, dash, moving platforms

---

## 25. Game-Specific Design

Asset pipeline decisions are maintained in `docs/plans/02-asset-pipeline-spec.md`.

---

## 26. Implementation Phases

| Phase | Focus | Duration |
|-------|-------|----------|
| 1 | Engine Foundation (window, renderer, input, tilemap, ECS, game loop) | 4-6 weeks |
| 2 | Game Systems (commands, animations, collision, pathfinding, audio) | 4-6 weeks |
| 4 | Editor (tile painter, entity placement, path editor, undo/redo) | 3-5 weeks |
| 5 | AI API + Asset Pipelines (IPC, MCP, artgen, audiogen) | 3-4 weeks |
| 6 | Multiplayer (transport, lockstep, UDP, lobby, replay) | 3-5 weeks |
| 7 | AI Editor Features (auto-pathing, balancing, heatmaps) | 3-5 weeks |
| 8 | Release (CLI, typed handles, CI/CD, distribution) | 2-3 weeks |

---

## 27. Key Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Pure Rust | Safety, compiler feedback, AI-dev friendly |
| Rendering | wgpu | Cross-platform GPU (Vulkan/DX12/Metal/WebGPU) |
| ECS | SparseSet + Change Tracking | Cache-friendly, no macro magic |
| Arithmetic | Fixed-Point Q16.16 | Deterministic simulation |
| Audio | kira | Tweening, spatial, streaming, crossfade |
| UI | Own Pixel UI (two tiers) | Consistent pixel art aesthetic |
| Sprites | Aseprite native | No manual export, tag-based animations |
| Tilemap | Orthogonal + Isometric | First-class, auto-tiling, chunk streaming |
| Networking | Client-Server (lockstep UDP) | Multiplayer-ready, enables replays |
| AI Interface | amigo_api + amigo_mcp | MCP for Claude Code, JSON-RPC for scripts |
| Art Pipeline | amigo_artgen + ComfyUI | External backend, post-processing in Rust |
| Audio Pipeline | amigo_audiogen + ACE-Step | Local GPU, royalty-free |
| Splashscreen | Default "Powered by Amigo Engine" | Opt-out via `EngineBuilder::splash(false)` |

---

*This specification is a living document. It will evolve as implementation progresses and new requirements emerge.*
