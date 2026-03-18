---
status: spec
crate: --
depends_on: ["engine/state-rewind"]
last_updated: 2026-03-18
---

# Puzzle Game

## Purpose

Turn-based or step-based puzzle games where the simulation advances only on player action. Every move is recorded as a reversible command, enabling unlimited undo/redo. The core loop is: think, act, observe result, undo if needed. Grid-based worlds with constraint validation and win condition detection.

Examples: Baba Is You (rule manipulation on a grid), The Witness (line-drawing puzzles with environmental clues), Sokoban (box pushing with minimal moves), Tetris (falling piece placement under time pressure).

## Public API

### PuzzleCommand Trait

```rust
/// A reversible command representing a single atomic game action.
/// Implements the Command Pattern — every move can be undone.
pub trait PuzzleCommand: Send + Sync {
    /// Execute the command, mutating the puzzle state. Returns Ok if valid, Err if rejected.
    fn execute(&mut self, state: &mut PuzzleState) -> Result<(), MoveError>;
    /// Reverse the command, restoring the previous state exactly.
    fn undo(&mut self, state: &mut PuzzleState);
    /// Human-readable description for debugging / replay display.
    fn description(&self) -> &str;
}

#[derive(Clone, Debug)]
pub enum MoveError {
    /// Move blocked by wall or obstacle.
    Blocked,
    /// Move violates a constraint (e.g. pushing two boxes at once in Sokoban).
    ConstraintViolation(String),
    /// No valid action for the given input.
    InvalidAction,
}
```

### Built-in Commands

```rust
/// Move an entity one tile in a direction.
pub struct MoveCommand {
    pub entity: EntityId,
    pub direction: GridDir,
    /// Entities that were pushed as a chain reaction (filled during execute).
    pushed: Vec<(EntityId, SimVec2)>,
}

/// Place or remove a tile/object at a grid position.
pub struct PlaceCommand {
    pub position: GridPos,
    pub tile: TileId,
    /// Previous tile at this position (filled during execute, used for undo).
    previous: Option<TileId>,
}

/// Composite command — multiple sub-commands that execute/undo atomically.
/// On execute: runs sub-commands in order. If any fails, undoes all previously
/// executed sub-commands in reverse order and returns the error.
/// On undo: undoes all sub-commands in reverse order.
pub struct CompositeCommand {
    pub commands: Vec<Box<dyn PuzzleCommand>>,
    /// How many sub-commands were successfully executed (for partial rollback).
    executed_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GridDir {
    Up, Down, Left, Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub fn neighbor(self, dir: GridDir) -> GridPos;
    pub fn manhattan_distance(self, other: GridPos) -> u32;
}
```

### UndoStack

```rust
/// Unlimited undo/redo stack using the Command Pattern.
pub struct UndoStack {
    /// Executed commands (past). Top of stack = most recent.
    history: Vec<Box<dyn PuzzleCommand>>,
    /// Undone commands (future). Cleared when a new command is executed.
    redo_stack: Vec<Box<dyn PuzzleCommand>>,
    /// Maximum history depth (0 = unlimited).
    pub max_depth: usize,
}

impl UndoStack {
    pub fn new() -> Self;
    pub fn with_max_depth(max_depth: usize) -> Self;
    /// Execute a command and push it onto the history stack.
    /// Clears the redo stack (branching invalidates future).
    pub fn execute(
        &mut self,
        command: Box<dyn PuzzleCommand>,
        state: &mut PuzzleState,
    ) -> Result<(), MoveError>;
    /// Undo the most recent command. Returns false if history is empty.
    pub fn undo(&mut self, state: &mut PuzzleState) -> bool;
    /// Redo the most recently undone command. Returns false if redo stack is empty.
    pub fn redo(&mut self, state: &mut PuzzleState) -> bool;
    /// Undo all commands back to the initial state.
    pub fn undo_all(&mut self, state: &mut PuzzleState);
    /// Number of moves in history.
    pub fn move_count(&self) -> usize;
    /// Whether undo is available.
    pub fn can_undo(&self) -> bool;
    /// Whether redo is available.
    pub fn can_redo(&self) -> bool;
    /// Clear all history (e.g. on level restart).
    pub fn clear(&mut self);
}
```

### TurnTick

```rust
/// Controls the turn-based simulation tick.
/// The world only advances when the player commits an action.
#[derive(Clone, Debug)]
pub struct TurnTick {
    /// Current turn number (incremented on each player action).
    pub turn: u32,
    /// Whether a turn is currently being processed (animation phase).
    pub animating: bool,
    /// Duration of the animation phase per turn (in render frames).
    pub animation_frames: u32,
    /// Remaining animation frames for the current turn.
    remaining_frames: u32,
}

impl TurnTick {
    pub fn new(animation_frames: u32) -> Self;
    /// Submit a player action, advancing the turn. Returns false if still animating.
    pub fn submit_action(&mut self) -> bool;
    /// Tick the animation timer. Returns true when animation is complete
    /// and the system is ready for the next action.
    pub fn tick_animation(&mut self) -> bool;
    /// Whether the system is ready to accept a new player action.
    pub fn ready_for_input(&self) -> bool;
    /// Reset to turn 0 (level restart).
    pub fn reset(&mut self);
}
```

### PuzzleState

```rust
/// The full puzzle state for a single level. Contains all mutable game data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PuzzleState {
    /// Grid dimensions.
    pub width: u32,
    pub height: u32,
    /// Tile data — collision layer and object layer.
    pub tiles: Vec<TileId>,
    /// Movable entities on the grid (player, boxes, etc.).
    pub entities: Vec<PuzzleEntity>,
    /// Named flags for level-specific logic (switches, toggles).
    pub flags: FxHashMap<String, bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PuzzleEntity {
    pub id: EntityId,
    pub pos: GridPos,
    pub entity_type: PuzzleEntityType,
    /// Whether this entity can be pushed.
    pub pushable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PuzzleEntityType {
    Player,
    Box,
    Key,
    /// Game-specific entity identified by index.
    Custom(u16),
}

impl PuzzleState {
    pub fn tile_at(&self, pos: GridPos) -> Option<TileId>;
    pub fn set_tile(&mut self, pos: GridPos, tile: TileId);
    pub fn entity_at(&self, pos: GridPos) -> Option<&PuzzleEntity>;
    pub fn entity_at_mut(&mut self, pos: GridPos) -> Option<&mut PuzzleEntity>;
    pub fn move_entity(&mut self, id: EntityId, to: GridPos);
    /// Check if a grid position is within bounds and not blocked.
    pub fn is_walkable(&self, pos: GridPos) -> bool;
}
```

### ConstraintValidator

```rust
/// Pre-move validation. Checks whether a proposed action is legal
/// before it is executed, preventing invalid state.
pub struct ConstraintValidator {
    constraints: Vec<Box<dyn Constraint>>,
}

/// A single constraint rule.
pub trait Constraint: Send + Sync {
    /// Check if the proposed command is valid in the current state.
    /// Returns Ok(()) if valid, Err with reason if not.
    fn validate(
        &self,
        command: &dyn PuzzleCommand,
        state: &PuzzleState,
    ) -> Result<(), MoveError>;
}

/// Built-in constraint: entities cannot move into Solid tiles.
pub struct SolidTileConstraint;

/// Built-in constraint: only one entity per tile (no stacking).
pub struct NoOverlapConstraint;

/// Built-in constraint: push chains have a maximum length.
pub struct MaxPushChainConstraint {
    pub max_chain: usize,
}

impl ConstraintValidator {
    pub fn new() -> Self;
    pub fn add_constraint(&mut self, constraint: Box<dyn Constraint>);
    /// Validate a command against all constraints. Returns first failure.
    pub fn validate(
        &self,
        command: &dyn PuzzleCommand,
        state: &PuzzleState,
    ) -> Result<(), MoveError>;
}
```

### WinCondition

```rust
/// Configurable completion detection — checked after each turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WinCondition {
    /// All entities of type A are on tiles of type B (Sokoban: all boxes on targets).
    AllOnTarget {
        entity_type: PuzzleEntityType,
        target_tile: TileId,
    },
    /// All specified flags are true.
    AllFlagsSet(Vec<String>),
    /// A specific entity reaches a specific position (reach the exit).
    EntityAtPosition {
        entity_type: PuzzleEntityType,
        target: GridPos,
    },
    /// No entities of a given type remain (e.g. clear all gems).
    NoneRemaining(PuzzleEntityType),
    /// Custom: evaluated by a callback index (for game-specific conditions).
    Custom(u16),
    /// Multiple conditions, all must be satisfied.
    All(Vec<WinCondition>),
    /// Multiple conditions, any one suffices.
    Any(Vec<WinCondition>),
}

/// Result of checking the win condition after a turn.
#[derive(Clone, Debug)]
pub enum PuzzleResult {
    /// Puzzle not yet solved.
    InProgress,
    /// Puzzle solved. Includes move count for scoring.
    Solved { moves: u32 },
    /// Puzzle is in an unwinnable state (optional detection).
    Deadlocked,
}

impl WinCondition {
    /// Evaluate the win condition against the current puzzle state.
    pub fn check(&self, state: &PuzzleState) -> PuzzleResult;
}
```

### HintSystem

```rust
/// Optional hint system with pre-computed solution steps.
pub struct HintSystem {
    /// Full solution as a sequence of commands (loaded from level data).
    solution: Vec<GridDir>,
    /// How many hints have been revealed so far.
    revealed_count: usize,
    /// Whether hints are available for this level.
    pub available: bool,
}

impl HintSystem {
    pub fn new(solution: Vec<GridDir>) -> Self;
    pub fn empty() -> Self;
    /// Reveal the next hint step. Returns the direction, or None if all revealed.
    pub fn reveal_next(&mut self) -> Option<GridDir>;
    /// Number of hints revealed so far.
    pub fn revealed(&self) -> usize;
    /// Total number of solution steps.
    pub fn total_steps(&self) -> usize;
    /// Reset revealed count (e.g. after undo-all).
    pub fn reset(&mut self);
}
```

### LevelLoader

```rust
/// Loads puzzle levels from RON definitions.
pub struct LevelLoader;

/// RON-serializable level definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelDef {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Flat tile array (row-major).
    pub tiles: Vec<TileId>,
    /// Entity placements.
    pub entities: Vec<EntityPlacement>,
    /// Win condition for this level.
    pub win_condition: WinCondition,
    /// Optional pre-computed solution for hints.
    pub solution: Option<Vec<GridDir>>,
    /// Optional par score (target move count).
    pub par_moves: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityPlacement {
    pub pos: GridPos,
    pub entity_type: PuzzleEntityType,
    pub pushable: bool,
}

impl LevelLoader {
    /// Load a level from a RON file path.
    pub fn load(path: &str) -> Result<LevelDef, LoadError>;
    /// Convert a LevelDef into a playable PuzzleState.
    pub fn instantiate(def: &LevelDef) -> (PuzzleState, WinCondition, HintSystem);
}

/// Tracks which levels have been completed and their best scores.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LevelProgress {
    /// Level name → best move count.
    pub completed: FxHashMap<String, u32>,
    /// Currently selected level pack / world.
    pub current_pack: String,
}

impl LevelProgress {
    pub fn mark_complete(&mut self, level: &str, moves: u32);
    pub fn best_score(&self, level: &str) -> Option<u32>;
    pub fn is_complete(&self, level: &str) -> bool;
    pub fn completion_count(&self) -> usize;
}
```

## Behavior

- **Turn Loop**: The game runs in a turn-based loop controlled by `TurnTick`. On player input (directional press, action button), a `PuzzleCommand` is constructed. `ConstraintValidator::validate()` checks legality. If valid, `UndoStack::execute()` runs the command and increments `TurnTick::turn`. An animation phase follows (entities slide to new positions via `Tween`), during which input is blocked. After animation, `WinCondition::check()` evaluates the state.
- **Undo/Redo**: Pressing undo calls `UndoStack::undo()`, which invokes the top command's `undo()` method, restoring the exact previous state. The undone command moves to the redo stack. Executing a new command after undo clears the redo stack (no branching timeline). `undo_all()` rewinds to the initial state.
- **Push Chains**: In Sokoban-style puzzles, `MoveCommand` detects pushable entities in the move direction and chains them. During `execute()`, the `pushed` field records all displaced entities and their original positions. During `undo()`, all pushed entities are restored.
- **Constraint System**: `ConstraintValidator` runs all registered `Constraint`s before execution. `SolidTileConstraint` checks the `CollisionLayer` of target tiles. `NoOverlapConstraint` prevents two entities from occupying the same cell. Games can add custom constraints (e.g. "ice tiles slide until hitting a wall").
- **Win Detection**: After each turn, `WinCondition::check()` evaluates the current `PuzzleState`. `AllOnTarget` counts matching entity-tile pairs. `EntityAtPosition` checks player location. `Deadlocked` detection is optional and game-specific (e.g. box pushed into corner with no target). On `Solved`, `LevelProgress::mark_complete()` records the move count.
- **Animation**: Entity movement between grid positions is purely visual. `Tween<RenderVec2>` interpolates the render position from the old tile to the new tile over `animation_frames`. The simulation state (`PuzzleState`) is updated instantly on command execution — the `Tween` only affects rendering.
- **Level Loading**: `LevelLoader::load()` reads a RON file and produces a `LevelDef`. `LevelLoader::instantiate()` converts it into a `PuzzleState` + `WinCondition` + `HintSystem`. Level progress is saved via `SaveManager` (using `LevelProgress` as the serializable payload).
- **Camera**: Uses `Camera::ScreenLock` mode — the camera is fixed to show the entire puzzle grid. For larger puzzles, the camera may use a fixed zoom level calculated from grid dimensions to fit the screen.

## Internal Design

- `PuzzleState` stores tile data as a flat `Vec<TileId>` (row-major, `width * height`). Entity positions are `GridPos` (integer coordinates). This avoids fixed-point math entirely for puzzle logic — `SimVec2` and physics are not used.
- **Grid ↔ ECS Mapping**: `PuzzleEntity` in `PuzzleState` is the authoritative game state (used by commands, undo, win detection). Each `PuzzleEntity` has a corresponding ECS entity for rendering. The mapping is maintained via a `FxHashMap<EntityId, Entity>` (PuzzleEntityId → ECS Entity). When a command moves a `PuzzleEntity` to a new `GridPos`, the corresponding ECS entity's `RenderVec2` position is updated via Tween for animation. The ECS entity is purely visual — it has no gameplay authority.
- **Animation Frame Count**: `TurnTick::animation_frames` counts **render frames** (not simulation ticks, since simulation is turn-based). At 60fps, `animation_frames = 12` gives a 200ms slide animation. The animation Tween runs in real-time (per render frame), while the turn counter only advances on player input.
- `UndoStack` stores trait objects (`Box<dyn PuzzleCommand>`). Each command carries its own undo data (captured during `execute()`), so undo requires no snapshots or diff computation. This is cheaper than the frame-level `StateRewind` system for turn-based games.
- `TurnTick` replaces the engine's fixed-timestep simulation loop. Physics `tick()` and steering `update()` are not called. Only the animation `Tween` system runs per frame.
- `ConstraintValidator` is a simple linear scan — puzzle games have few constraints (typically 2-5), so no optimization is needed.
- `LevelDef` RON files are human-editable. Tile indices map to the `Tilemap` collision layer types (`Solid`, `OneWay`, `Trigger`).

## Non-Goals

- **Real-time physics.** Puzzle state transitions are discrete. No `RigidBody`, no continuous collision detection. Grid-only.
- **Procedural puzzle generation.** Levels are hand-crafted and loaded from RON. Procedural generation of solvable puzzles is an unsolved research problem and out of scope.
- **Multiplayer puzzle solving.** Single-player turn-based only.
- **Automatic deadlock detection for all puzzle types.** Deadlock detection is game-specific (Sokoban has known algorithms, but general puzzles do not). Games can implement `WinCondition::Custom` for this.
- **Visual puzzle types (jigsaw, match-3).** This spec covers grid-based logic puzzles. Visual/physics puzzles require different systems.

## Open Questions

- Should `HintSystem` support animated hint playback (auto-executing solution commands with delays)?
- Could A* pathfinding be reused for automated hint generation in simple push puzzles, or is pre-computed solutions the only viable approach?
- Should `PuzzleState` support hex grids in addition to rectangular grids?
- Is `CompositeCommand` sufficient for "simultaneous actions" (e.g. Baba Is You rule evaluation), or does rule-based puzzle logic need a dedicated system?

## Referenzen

- Baba Is You: Rule manipulation, command-based undo, grid logic
- Sokoban: Push mechanics, deadlock detection, par scoring
- The Witness: Environmental puzzle constraints, non-grid puzzles (out of scope)
- Tetris: Time-pressured piece placement (uses TurnTick with forced advance)
- [engine/state-rewind](../engine/state-rewind.md) → Frame-level rewind (NOT used by this template). State-rewind captures snapshots per frame, which is wasteful for turn-based puzzles where state only changes on player action. The command-based `UndoStack` is the correct approach: it stores only the delta per turn (~bytes), while state-rewind stores full snapshots per frame (~kilobytes). State-rewind is appropriate for real-time puzzle games (e.g. Braid-style time rewind) but NOT for discrete turn-based puzzles like Sokoban
- [engine/save-load](../engine/save-load.md) → LevelProgress persistence via SaveManager
- [engine/tilemap](../engine/tilemap.md) → CollisionLayer types (Solid, OneWay, Trigger) for grid tiles
- [engine/camera](../engine/camera.md) → ScreenLock mode for fixed puzzle view
- [engine/tween](../engine/tween.md) → Smooth entity movement animation between grid positions
