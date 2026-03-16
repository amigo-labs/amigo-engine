---
status: "draft (placeholder -- full game mechanics spec needs to be written)"
crate: amigo_td
depends_on: ["engine/core", "engine/tilemap", "engine/pathfinding"]
last_updated: 2026-03-16
---

# Amigo TD -- Game Design (Placeholder)

**NOTE:** This is a placeholder spec. A full game mechanics specification covering tower types, enemy types, upgrade trees, wave design, balancing, and per-world mechanics needs to be written. The content below is extracted from the engine specification to preserve context.

## Purpose

Define the game mechanics, content, and balancing for Amigo TD -- the first game built on the Amigo Engine.

## Public API

Game-level logic -- no public API. Consumes engine systems (ECS, tilemap, pathfinding, audio, UI).

## Behavior

Tower Defense with 6 thematic worlds. Each world has unique pixel art tilesets, tower types, enemy types, and atmospheric effects. See [UI spec](ui.md) for the full user interface design.

## Internal Design

To be defined. Will use the engine's command system, ECS, and data-driven RON configuration for tower stats, wave configs, and enemy definitions.

## Non-Goals

- Engine-level systems (those belong in engine specs)
- Art/audio generation (see [artgen](../../ai-pipelines/artgen.md) and [audiogen](../../ai-pipelines/audiogen.md))

## Open Questions

- Exact tower types and stats per world
- Enemy types, abilities, and resistances per world
- Upgrade tree branching design
- Star rating thresholds per level
- Difficulty curve across 6 worlds
- Special mechanics per world (water tiles, teleporters, etc.)

---

## First Game: Tower Defense

Tower Defense with 6 thematic worlds: Pirates of the Caribbean, Lord of the Rings, Dune, Matrix, Game of Thrones, Stranger Things. Each world has unique pixel art tilesets, tower types, enemy types, and atmospheric effects.

### Target Genres

The engine supports all classic 2D pixel art genres: Tower Defense, Platformer/Jump'n'Run, RPG/Adventure, Shmup/Shoot'em Up, Beat'em Up, Puzzle, Fighting Games, Run'n'Gun, Metroidvania.

---

## Game-Specific Design

Asset pipeline decisions are maintained in separate spec files:

- **Art Pipeline**: See [artgen](../../ai-pipelines/artgen.md) (ComfyUI integration, post-processing, style definitions)
- **Audio Pipeline**: See [audiogen](../../ai-pipelines/audiogen.md) (ACE-Step music gen, AudioGen SFX, adaptive music system, stem-based vertical layering)

---

## Additional Engine Systems (Phase 2+)

The engine provides generic, reusable mechanics that work across many game types. These systems are **engine-level** -- data-driven, with the game defining the concrete content (which crops, which bullets, which puzzle pieces). The engine executes the logic.

### farming.rs -- Growth & Calendar System

**Engine-Level Mechanics:**

#### GrowthStage System
- `GrowthStage { id, duration_ticks, next_stage }` -- generic growth stages
- `GrowthDef { stages: Vec<GrowthStage>, requires_water, requires_light }` -- definition of a growing thing
- `GrowthInstance { def_id, current_stage, ticks_in_stage, watered, lit }` -- running instance
- `tick_growth()` -- advances all instances, returns events (StageChanged, Completed, Withered)

#### Calendar System
- `Calendar { day, season, year, ticks_per_day }` -- time tracking
- `Season` enum (Spring, Summer, Autumn, Winter) -- 28 days each
- `tick_calendar()` -- advances calendar, returns DayChanged/SeasonChanged events
- `TimeOfDay { hour, minute }` -- derived from tick position in day

#### TileGrid (interactive grid)
- `FarmTile { soil_state, moisture, fertility, content }` -- generic tile
- `SoilState` enum (Empty, Tilled, Planted, Watered)
- `FarmGrid { width, height, tiles }` -- grid with tile operations
- `till()`, `water()`, `plant()`, `harvest()` -- operations that produce events

**NOT in the engine:** Which plants exist, prices, NPC dialogues, shop system (Dialog + Inventory already exist)

### bullet_pattern.rs -- Bullet Spawner System

**Engine-Level Mechanics:**

#### BulletPool
- `Bullet { position, velocity, lifetime, damage, radius, active }` -- single projectile
- `BulletPool { bullets: Vec<Bullet>, active_count }` -- object pool for performance
- `spawn()`, `despawn()`, `tick()` -- pool management
- Collision check against Rect/Circle shapes (uses existing collision.rs)

#### Pattern System
- `PatternShape` enum:
  - `Radial { count, speed }` -- evenly distributed in a circle
  - `Spiral { count, speed, rotation_speed }` -- rotating spiral
  - `Aimed { count, spread_angle, speed }` -- aimed at target with spread
  - `Wave { count, amplitude, frequency, speed }` -- sine wave
  - `Random { count, min_speed, max_speed }` -- random directions
- `BulletEmitter { position, pattern, fire_rate, timer, rotation }` -- fires patterns
- `tick_emitters()` -- advances all emitters, spawns bullets in pool

#### Boss Pattern Sequencer
- `PhasePattern { emitters: Vec<BulletEmitter>, duration_ticks }` -- one phase
- `PatternSequence { phases, current_phase, loop_mode }` -- sequence of phases
- `LoopMode` enum (Once, Loop, PingPong)

**NOT in the engine:** Concrete boss patterns, bullet sprites, scoring, graze reward logic

### puzzle.rs -- Grid Puzzle System

**Engine-Level Mechanics:**

#### PuzzleGrid
- `Cell<T: Copy + Eq>` -- generic cell (T = game-defined enum for colors/types)
- `PuzzleGrid<T> { width, height, cells }` -- 2D grid
- `get()`, `set()`, `swap()` -- basic operations
- `apply_gravity(direction)` -- cells fall in one direction (for Match-3, Tetris)
- `is_empty()`, `find_all(predicate)` -- queries

#### Pattern Matching
- `MatchResult { cells: Vec<(u32, u32)>, pattern_type }` -- found match
- `find_horizontal_matches(min_length)` -- find rows
- `find_vertical_matches(min_length)` -- find columns
- `find_connected(x, y, predicate)` -- flood-fill for matching cells (Puzzle Bobble)
- `clear_matches()` -- remove found matches, returns ClearEvent

#### Move System
- `PuzzleMove` enum (Swap, Insert, Rotate, Slide) -- generic moves
- `MoveHistory { moves: Vec<PuzzleMove>, cursor }` -- undo/redo stack
- `apply_move()`, `undo()`, `redo()` -- move management
- `validate_move()` -- checks if a move is legal (callback-based)

#### Block Spawning (for Tetris-style games)
- `BlockShape { cells: Vec<(i32, i32)> }` -- relative cell positions
- `rotate_cw()`, `rotate_ccw()` -- rotation
- `can_place(grid, x, y)`, `place(grid, x, y)` -- placement
- `BlockBag { shapes, remaining }` -- fair randomization (7-bag system)

**NOT in the engine:** Concrete puzzle rules (how many colors, points per match, level design)

### platformer.rs -- Platformer Controller System

**Engine-Level Mechanics:**

#### PlatformerController
- Input buffering: `JumpBuffer { buffered, buffer_ticks }` -- jump input stored for N ticks
- `CoyoteTime { grounded_ticks, coyote_ticks }` -- can still jump after leaving edge
- `VariableJump { jump_velocity, cut_multiplier, holding }` -- short press = small jump
- `WallInteraction { wall_slide_speed, wall_jump_velocity, wall_jump_direction }` -- wall slide/jump
- `DashState { can_dash, dash_speed, dash_duration, dash_cooldown, dashing }` -- dash/dodge
- `PlatformerState` -- combines everything, `tick()` returns desired velocity

#### Ground Detection
- `GroundCheck { is_grounded, was_grounded, ground_normal, on_slope }` -- ground detection
- `check_ground(position, shape, tilemap_query)` -- checks ground below feet
- Slope handling: adjust speed on slopes

#### Moving Platforms
- `PlatformPath { waypoints: Vec<RenderVec2>, speed, mode }` -- path definition
- `PathMode` enum (Loop, PingPong, Once)
- `MovingPlatform { path, current_segment, t, riders }` -- platform instance
- `tick_platforms()` -- moves platforms, shifts riders along

#### One-Way & Through Platforms
- `PlatformType` enum (Solid, OneWay, Through) -- already partly in tilemap
- `should_collide(player_velocity, platform_type)` -- collision logic

**NOT in the engine:** Character sprites, level design, enemy-specific AI, power-ups

### Implementation Order
1. `platformer.rs` -- builds on existing physics.rs + collision.rs
2. `farming.rs` -- independent, uses scheduler concept
3. `bullet_pattern.rs` -- uses existing collision.rs
4. `puzzle.rs` -- completely independent

### Conventions
- Builder pattern with `.with_*()` for all config structs
- Serde Serialize/Deserialize where possible (no RenderVec2 in serialized structs)
- Internal XorShift RNG where randomization is needed
- Fix (I16F16) only where determinism is needed, otherwise f32
- Events as return values instead of callbacks
- Tests for every module
