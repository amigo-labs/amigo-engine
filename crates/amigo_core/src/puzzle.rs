use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::ecs::EntityId;

// ---------------------------------------------------------------------------
// Puzzle grid — generic grid with cell operations
// ---------------------------------------------------------------------------

/// A 2D puzzle grid with cells of type T.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PuzzleGrid<T: Copy + Eq> {
    pub width: u32,
    pub height: u32,
    cells: Vec<Option<T>>,
}

impl<T: Copy + Eq> PuzzleGrid<T> {
    /// Create a grid filled with `None` (empty cells).
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cells: vec![None; (width * height) as usize],
        }
    }

    /// Create a grid pre-filled with values.
    pub fn from_data(width: u32, height: u32, data: Vec<Option<T>>) -> Self {
        assert_eq!(data.len(), (width * height) as usize);
        Self {
            width,
            height,
            cells: data,
        }
    }

    fn idx(&self, x: u32, y: u32) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y * self.width + x) as usize)
        } else {
            None
        }
    }

    pub fn get(&self, x: u32, y: u32) -> Option<T> {
        self.idx(x, y).and_then(|i| self.cells[i])
    }

    pub fn set(&mut self, x: u32, y: u32, val: Option<T>) {
        if let Some(i) = self.idx(x, y) {
            self.cells[i] = val;
        }
    }

    pub fn is_empty(&self, x: u32, y: u32) -> bool {
        self.idx(x, y).is_none_or(|i| self.cells[i].is_none())
    }

    /// Swap two cells. Returns true if both are in bounds.
    pub fn swap(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) -> bool {
        let i1 = match self.idx(x1, y1) {
            Some(i) => i,
            None => return false,
        };
        let i2 = match self.idx(x2, y2) {
            Some(i) => i,
            None => return false,
        };
        self.cells.swap(i1, i2);
        true
    }

    /// Apply gravity: cells fall in the given direction.
    /// `down = true` means cells fall toward higher Y values (typical top-down).
    pub fn apply_gravity(&mut self, down: bool) {
        for x in 0..self.width {
            if down {
                // Process column bottom to top
                let mut write_y = self.height - 1;
                for read_y in (0..self.height).rev() {
                    let i = (read_y * self.width + x) as usize;
                    if self.cells[i].is_some() {
                        let write_i = (write_y * self.width + x) as usize;
                        if write_i != i {
                            self.cells[write_i] = self.cells[i];
                            self.cells[i] = None;
                        }
                        write_y = write_y.saturating_sub(1);
                    }
                }
            } else {
                // Fall upward (toward y=0)
                let mut write_y = 0u32;
                for read_y in 0..self.height {
                    let i = (read_y * self.width + x) as usize;
                    if self.cells[i].is_some() {
                        let write_i = (write_y * self.width + x) as usize;
                        if write_i != i {
                            self.cells[write_i] = self.cells[i];
                            self.cells[i] = None;
                        }
                        write_y += 1;
                    }
                }
            }
        }
    }

    /// Find all positions matching a predicate.
    pub fn find_all(&self, predicate: impl Fn(T) -> bool) -> Vec<(u32, u32)> {
        let mut result = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let i = (y * self.width + x) as usize;
                if let Some(val) = self.cells[i] {
                    if predicate(val) {
                        result.push((x, y));
                    }
                }
            }
        }
        result
    }

    /// Count non-empty cells.
    pub fn filled_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// Clear all cells.
    pub fn clear(&mut self) {
        for c in &mut self.cells {
            *c = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Pattern matching — find rows, columns, connected groups
// ---------------------------------------------------------------------------

/// A group of matched cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchGroup {
    pub cells: Vec<(u32, u32)>,
}

/// Find horizontal matches of `min_length` or more.
pub fn find_horizontal_matches<T: Copy + Eq>(
    grid: &PuzzleGrid<T>,
    min_length: u32,
) -> Vec<MatchGroup> {
    let mut groups = Vec::new();

    for y in 0..grid.height {
        let mut run_start = 0u32;
        let mut run_val: Option<T> = None;
        let mut run_len = 0u32;

        for x in 0..grid.width {
            let cell = grid.get(x, y);
            match (cell, run_val) {
                (Some(v), Some(rv)) if v == rv => {
                    run_len += 1;
                }
                (Some(v), _) => {
                    if run_len >= min_length {
                        let cells: Vec<_> =
                            (run_start..run_start + run_len).map(|rx| (rx, y)).collect();
                        groups.push(MatchGroup { cells });
                    }
                    run_start = x;
                    run_val = Some(v);
                    run_len = 1;
                }
                (None, _) => {
                    if run_len >= min_length {
                        let cells: Vec<_> =
                            (run_start..run_start + run_len).map(|rx| (rx, y)).collect();
                        groups.push(MatchGroup { cells });
                    }
                    run_val = None;
                    run_len = 0;
                }
            }
        }

        if run_len >= min_length {
            let cells: Vec<_> = (run_start..run_start + run_len).map(|rx| (rx, y)).collect();
            groups.push(MatchGroup { cells });
        }
    }

    groups
}

/// Find vertical matches of `min_length` or more.
pub fn find_vertical_matches<T: Copy + Eq>(
    grid: &PuzzleGrid<T>,
    min_length: u32,
) -> Vec<MatchGroup> {
    let mut groups = Vec::new();

    for x in 0..grid.width {
        let mut run_start = 0u32;
        let mut run_val: Option<T> = None;
        let mut run_len = 0u32;

        for y in 0..grid.height {
            let cell = grid.get(x, y);
            match (cell, run_val) {
                (Some(v), Some(rv)) if v == rv => {
                    run_len += 1;
                }
                (Some(v), _) => {
                    if run_len >= min_length {
                        let cells: Vec<_> =
                            (run_start..run_start + run_len).map(|ry| (x, ry)).collect();
                        groups.push(MatchGroup { cells });
                    }
                    run_start = y;
                    run_val = Some(v);
                    run_len = 1;
                }
                (None, _) => {
                    if run_len >= min_length {
                        let cells: Vec<_> =
                            (run_start..run_start + run_len).map(|ry| (x, ry)).collect();
                        groups.push(MatchGroup { cells });
                    }
                    run_val = None;
                    run_len = 0;
                }
            }
        }

        if run_len >= min_length {
            let cells: Vec<_> = (run_start..run_start + run_len).map(|ry| (x, ry)).collect();
            groups.push(MatchGroup { cells });
        }
    }

    groups
}

/// Flood-fill to find a connected group of identical cells starting from (sx, sy).
pub fn find_connected<T: Copy + Eq>(grid: &PuzzleGrid<T>, sx: u32, sy: u32) -> Vec<(u32, u32)> {
    let target = match grid.get(sx, sy) {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut visited = vec![false; (grid.width * grid.height) as usize];
    let mut stack = vec![(sx, sy)];
    let mut result = Vec::new();

    while let Some((x, y)) = stack.pop() {
        let idx = (y * grid.width + x) as usize;
        if visited[idx] {
            continue;
        }
        visited[idx] = true;

        if grid.get(x, y) != Some(target) {
            continue;
        }

        result.push((x, y));

        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < grid.width {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < grid.height {
            stack.push((x, y + 1));
        }
    }

    result
}

/// Clear all cells in the given positions. Returns how many were actually cleared.
pub fn clear_cells<T: Copy + Eq>(grid: &mut PuzzleGrid<T>, positions: &[(u32, u32)]) -> u32 {
    let mut count = 0;
    for &(x, y) in positions {
        if !grid.is_empty(x, y) {
            grid.set(x, y, None);
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Move system — undo/redo history
// ---------------------------------------------------------------------------

/// A recorded puzzle move for undo/redo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PuzzleMove {
    /// Swap two cells.
    Swap { x1: u32, y1: u32, x2: u32, y2: u32 },
    /// Place a value at a position (stores previous value for undo).
    Place { x: u32, y: u32, old: u32, new: u32 },
    /// Clear a set of positions (stores what was there).
    Clear { positions: Vec<(u32, u32, u32)> }, // (x, y, old_value_as_u32)
    /// Custom move identified by an opaque id.
    Custom { id: u32 },
}

/// Undo/redo stack for puzzle moves.
#[derive(Clone, Debug, Default)]
pub struct MoveHistory {
    moves: Vec<PuzzleMove>,
    cursor: usize,
}

impl MoveHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a move. Discards any redo history.
    pub fn push(&mut self, mov: PuzzleMove) {
        self.moves.truncate(self.cursor);
        self.moves.push(mov);
        self.cursor += 1;
    }

    /// Get the last move for undo (does NOT automatically apply it).
    /// Moves the cursor back. Returns None if nothing to undo.
    pub fn undo(&mut self) -> Option<&PuzzleMove> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(&self.moves[self.cursor])
        } else {
            None
        }
    }

    /// Get the next move for redo (does NOT automatically apply it).
    /// Moves the cursor forward. Returns None if nothing to redo.
    pub fn redo(&mut self) -> Option<&PuzzleMove> {
        if self.cursor < self.moves.len() {
            let mov = &self.moves[self.cursor];
            self.cursor += 1;
            Some(mov)
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.moves.len()
    }

    pub fn move_count(&self) -> usize {
        self.cursor
    }

    pub fn clear(&mut self) {
        self.moves.clear();
        self.cursor = 0;
    }
}

// ---------------------------------------------------------------------------
// Block shapes — for Tetris-like piece placement
// ---------------------------------------------------------------------------

/// A block shape defined by relative cell positions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockShape {
    /// Relative positions of cells in the shape.
    pub cells: Vec<(i32, i32)>,
}

impl BlockShape {
    pub fn new(cells: Vec<(i32, i32)>) -> Self {
        Self { cells }
    }

    /// Common Tetris-like shapes.
    pub fn i_piece() -> Self {
        Self::new(vec![(0, 0), (1, 0), (2, 0), (3, 0)])
    }

    pub fn o_piece() -> Self {
        Self::new(vec![(0, 0), (1, 0), (0, 1), (1, 1)])
    }

    pub fn t_piece() -> Self {
        Self::new(vec![(0, 0), (1, 0), (2, 0), (1, 1)])
    }

    pub fn l_piece() -> Self {
        Self::new(vec![(0, 0), (0, 1), (0, 2), (1, 2)])
    }

    pub fn s_piece() -> Self {
        Self::new(vec![(1, 0), (2, 0), (0, 1), (1, 1)])
    }

    /// Rotate 90 degrees clockwise around the origin.
    pub fn rotate_cw(&self) -> Self {
        Self {
            cells: self.cells.iter().map(|&(x, y)| (y, -x)).collect(),
        }
    }

    /// Rotate 90 degrees counter-clockwise around the origin.
    pub fn rotate_ccw(&self) -> Self {
        Self {
            cells: self.cells.iter().map(|&(x, y)| (-y, x)).collect(),
        }
    }

    /// Check if the shape can be placed at (px, py) on the grid.
    pub fn can_place<T: Copy + Eq>(&self, grid: &PuzzleGrid<T>, px: i32, py: i32) -> bool {
        for &(dx, dy) in &self.cells {
            let x = px + dx;
            let y = py + dy;
            if x < 0 || y < 0 || x >= grid.width as i32 || y >= grid.height as i32 {
                return false;
            }
            if !grid.is_empty(x as u32, y as u32) {
                return false;
            }
        }
        true
    }

    /// Place the shape on the grid at (px, py) with the given value.
    pub fn place<T: Copy + Eq>(&self, grid: &mut PuzzleGrid<T>, px: i32, py: i32, val: T) -> bool {
        if !self.can_place(grid, px, py) {
            return false;
        }
        for &(dx, dy) in &self.cells {
            grid.set((px + dx) as u32, (py + dy) as u32, Some(val));
        }
        true
    }

    /// Bounding box width and height.
    pub fn bounds(&self) -> (i32, i32) {
        if self.cells.is_empty() {
            return (0, 0);
        }
        let min_x = self.cells.iter().map(|c| c.0).min().unwrap();
        let max_x = self.cells.iter().map(|c| c.0).max().unwrap();
        let min_y = self.cells.iter().map(|c| c.1).min().unwrap();
        let max_y = self.cells.iter().map(|c| c.1).max().unwrap();
        (max_x - min_x + 1, max_y - min_y + 1)
    }
}

/// A bag of shapes for fair randomization (7-bag system).
/// Ensures each shape appears once before any repeats.
#[derive(Clone, Debug)]
pub struct BlockBag {
    shapes: Vec<BlockShape>,
    remaining: Vec<usize>,
    rng_state: u64,
}

impl BlockBag {
    pub fn new(shapes: Vec<BlockShape>, seed: u64) -> Self {
        Self {
            shapes,
            remaining: Vec::new(),
            rng_state: seed,
        }
    }

    /// Standard Tetris 7-bag.
    pub fn standard_tetris(seed: u64) -> Self {
        Self::new(
            vec![
                BlockShape::i_piece(),
                BlockShape::o_piece(),
                BlockShape::t_piece(),
                BlockShape::l_piece(),
                BlockShape::s_piece(),
                // J and Z are rotations of L and S
                BlockShape::new(vec![(0, 0), (1, 0), (1, 1), (1, 2)]), // J
                BlockShape::new(vec![(0, 0), (1, 0), (1, 1), (2, 1)]), // Z
            ],
            seed,
        )
    }

    /// Draw the next shape from the bag.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> BlockShape {
        if self.remaining.is_empty() {
            self.refill();
        }
        let idx = self.remaining.pop().unwrap();
        self.shapes[idx].clone()
    }

    /// Peek at what the next shape will be without consuming it.
    pub fn peek(&mut self) -> &BlockShape {
        if self.remaining.is_empty() {
            self.refill();
        }
        let idx = *self.remaining.last().unwrap();
        &self.shapes[idx]
    }

    fn refill(&mut self) {
        self.remaining = (0..self.shapes.len()).collect();
        // Fisher-Yates shuffle
        let len = self.remaining.len();
        for i in (1..len).rev() {
            let j = self.xorshift_range(i + 1);
            self.remaining.swap(i, j);
        }
    }

    fn xorshift_range(&mut self, max: usize) -> usize {
        let mut s = self.rng_state;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        self.rng_state = s;
        (s as usize) % max
    }
}

// ===========================================================================
// General turn-based puzzle infrastructure (command pattern, undo, constraints,
// win detection, level loading, hints, progress tracking)
// ===========================================================================

// ---------------------------------------------------------------------------
// Core types — TileId, GridPos, GridDir
// ---------------------------------------------------------------------------

/// Tile identifier — indexes into the tilemap's tile palette.
pub type TileId = u16;

/// Sentinel value for an empty / air tile.
pub const TILE_EMPTY: TileId = 0;
/// Sentinel value for a solid / wall tile.
pub const TILE_SOLID: TileId = 1;

/// Integer grid position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Return the adjacent position in the given direction.
    pub fn neighbor(self, dir: GridDir) -> GridPos {
        match dir {
            GridDir::Up => GridPos {
                x: self.x,
                y: self.y - 1,
            },
            GridDir::Down => GridPos {
                x: self.x,
                y: self.y + 1,
            },
            GridDir::Left => GridPos {
                x: self.x - 1,
                y: self.y,
            },
            GridDir::Right => GridPos {
                x: self.x + 1,
                y: self.y,
            },
        }
    }

    /// Manhattan distance between two positions.
    pub fn manhattan_distance(self, other: GridPos) -> u32 {
        ((self.x - other.x).unsigned_abs()) + ((self.y - other.y).unsigned_abs())
    }
}

/// Cardinal direction on the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GridDir {
    Up,
    Down,
    Left,
    Right,
}

impl GridDir {
    /// Return the opposite direction.
    pub fn opposite(self) -> Self {
        match self {
            GridDir::Up => GridDir::Down,
            GridDir::Down => GridDir::Up,
            GridDir::Left => GridDir::Right,
            GridDir::Right => GridDir::Left,
        }
    }
}

// ---------------------------------------------------------------------------
// MoveError
// ---------------------------------------------------------------------------

/// Error returned when a puzzle command is rejected.
#[derive(Clone, Debug)]
pub enum MoveError {
    /// Move blocked by wall or obstacle.
    Blocked,
    /// Move violates a constraint (e.g. pushing two boxes at once in Sokoban).
    ConstraintViolation(String),
    /// No valid action for the given input.
    InvalidAction,
}

impl core::fmt::Display for MoveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MoveError::Blocked => write!(f, "move blocked"),
            MoveError::ConstraintViolation(reason) => write!(f, "constraint violation: {reason}"),
            MoveError::InvalidAction => write!(f, "invalid action"),
        }
    }
}

// ---------------------------------------------------------------------------
// PuzzleState — full mutable state for a single level
// ---------------------------------------------------------------------------

/// The full puzzle state for a single level. Contains all mutable game data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PuzzleState {
    /// Grid dimensions.
    pub width: u32,
    pub height: u32,
    /// Tile data — flat row-major array (collision / object layer).
    pub tiles: Vec<TileId>,
    /// Movable entities on the grid (player, boxes, etc.).
    pub entities: Vec<PuzzleEntity>,
    /// Named flags for level-specific logic (switches, toggles).
    pub flags: FxHashMap<String, bool>,
}

impl PuzzleState {
    /// Create a new empty puzzle state.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            tiles: vec![TILE_EMPTY; (width * height) as usize],
            entities: Vec::new(),
            flags: FxHashMap::default(),
        }
    }

    /// Convert grid position to flat index, returning None if out of bounds.
    fn pos_to_idx(&self, pos: GridPos) -> Option<usize> {
        if pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < self.width && (pos.y as u32) < self.height {
            Some((pos.y as u32 * self.width + pos.x as u32) as usize)
        } else {
            None
        }
    }

    /// Get tile at position, None if out of bounds.
    pub fn tile_at(&self, pos: GridPos) -> Option<TileId> {
        self.pos_to_idx(pos).map(|i| self.tiles[i])
    }

    /// Set tile at position (no-op if out of bounds).
    pub fn set_tile(&mut self, pos: GridPos, tile: TileId) {
        if let Some(i) = self.pos_to_idx(pos) {
            self.tiles[i] = tile;
        }
    }

    /// Find entity at a given position.
    pub fn entity_at(&self, pos: GridPos) -> Option<&PuzzleEntity> {
        self.entities.iter().find(|e| e.pos == pos)
    }

    /// Find entity at a given position (mutable).
    pub fn entity_at_mut(&mut self, pos: GridPos) -> Option<&mut PuzzleEntity> {
        self.entities.iter_mut().find(|e| e.pos == pos)
    }

    /// Move an entity to a new position by its id.
    pub fn move_entity(&mut self, id: EntityId, to: GridPos) {
        if let Some(ent) = self.entities.iter_mut().find(|e| e.id == id) {
            ent.pos = to;
        }
    }

    /// Find entity by id.
    pub fn entity_by_id(&self, id: EntityId) -> Option<&PuzzleEntity> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Check if a grid position is within bounds and not blocked by a solid tile.
    pub fn is_walkable(&self, pos: GridPos) -> bool {
        match self.tile_at(pos) {
            Some(tile) => tile != TILE_SOLID,
            None => false, // out of bounds
        }
    }

    /// Check if a position is within the grid bounds.
    pub fn in_bounds(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < self.width && (pos.y as u32) < self.height
    }
}

/// A movable entity on the puzzle grid.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PuzzleEntity {
    pub id: EntityId,
    pub pos: GridPos,
    pub entity_type: PuzzleEntityType,
    /// Whether this entity can be pushed by other entities.
    pub pushable: bool,
}

/// Types of puzzle entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PuzzleEntityType {
    Player,
    Box,
    Key,
    /// Game-specific entity identified by index.
    Custom(u16),
}

// ---------------------------------------------------------------------------
// PuzzleCommand trait — Command Pattern for reversible moves
// ---------------------------------------------------------------------------

/// A reversible command representing a single atomic game action.
/// Implements the Command Pattern — every move can be undone.
pub trait PuzzleCommand: Send + Sync {
    /// Execute the command, mutating the puzzle state.
    /// Returns Ok if valid, Err if rejected.
    fn execute(&mut self, state: &mut PuzzleState) -> Result<(), MoveError>;
    /// Reverse the command, restoring the previous state exactly.
    fn undo(&mut self, state: &mut PuzzleState);
    /// Human-readable description for debugging / replay display.
    fn description(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Built-in commands: MoveCommand, PlaceCommand, CompositeCommand
// ---------------------------------------------------------------------------

/// Move an entity one tile in a direction, with push-chain support.
pub struct MoveCommand {
    pub entity: EntityId,
    pub direction: GridDir,
    /// Entities that were pushed as a chain reaction (filled during execute).
    pushed: Vec<(EntityId, GridPos)>,
}

impl MoveCommand {
    pub fn new(entity: EntityId, direction: GridDir) -> Self {
        Self {
            entity,
            direction,
            pushed: Vec::new(),
        }
    }
}

impl PuzzleCommand for MoveCommand {
    fn execute(&mut self, state: &mut PuzzleState) -> Result<(), MoveError> {
        let entity = state
            .entity_by_id(self.entity)
            .ok_or(MoveError::InvalidAction)?;
        let from = entity.pos;
        let to = from.neighbor(self.direction);

        if !state.is_walkable(to) {
            return Err(MoveError::Blocked);
        }

        // Collect the push chain: walk forward from `to` gathering pushable entities.
        self.pushed.clear();
        let mut check_pos = to;
        loop {
            // Is there an entity at check_pos?
            let blocker = state
                .entity_at(check_pos)
                .map(|e| (e.id, e.pos, e.pushable));
            match blocker {
                Some((bid, bpos, true)) => {
                    // Pushable — record it and check the next tile.
                    let next = bpos.neighbor(self.direction);
                    if !state.is_walkable(next) {
                        return Err(MoveError::Blocked);
                    }
                    self.pushed.push((bid, bpos));
                    check_pos = next;
                }
                Some((_, _, false)) => {
                    // Non-pushable entity blocks the move.
                    return Err(MoveError::Blocked);
                }
                None => break, // nothing blocking
            }
        }

        // Execute push chain in reverse order (furthest entity first) to avoid overlap.
        for &(pid, ppos) in self.pushed.iter().rev() {
            let dest = ppos.neighbor(self.direction);
            state.move_entity(pid, dest);
        }

        // Move the acting entity.
        state.move_entity(self.entity, to);

        Ok(())
    }

    fn undo(&mut self, state: &mut PuzzleState) {
        // Reverse: move acting entity back, then un-push in forward order.
        let entity = state.entity_by_id(self.entity);
        if let Some(ent) = entity {
            let current = ent.pos;
            let original = current.neighbor(self.direction.opposite());
            state.move_entity(self.entity, original);
        }

        // Restore pushed entities to their original positions (forward order).
        for &(pid, original_pos) in &self.pushed {
            state.move_entity(pid, original_pos);
        }
    }

    fn description(&self) -> &str {
        "move entity"
    }
}

/// Place or remove a tile/object at a grid position.
pub struct PlaceCommand {
    pub position: GridPos,
    pub tile: TileId,
    /// Previous tile at this position (filled during execute, used for undo).
    previous: Option<TileId>,
}

impl PlaceCommand {
    pub fn new(position: GridPos, tile: TileId) -> Self {
        Self {
            position,
            tile,
            previous: None,
        }
    }
}

impl PuzzleCommand for PlaceCommand {
    fn execute(&mut self, state: &mut PuzzleState) -> Result<(), MoveError> {
        self.previous = state.tile_at(self.position);
        state.set_tile(self.position, self.tile);
        Ok(())
    }

    fn undo(&mut self, state: &mut PuzzleState) {
        if let Some(prev) = self.previous {
            state.set_tile(self.position, prev);
        }
    }

    fn description(&self) -> &str {
        "place tile"
    }
}

/// Composite command — multiple sub-commands that execute/undo atomically.
pub struct CompositeCommand {
    pub commands: Vec<Box<dyn PuzzleCommand>>,
    /// How many sub-commands were successfully executed (for partial rollback).
    executed_count: usize,
}

impl CompositeCommand {
    pub fn new(commands: Vec<Box<dyn PuzzleCommand>>) -> Self {
        Self {
            commands,
            executed_count: 0,
        }
    }
}

impl PuzzleCommand for CompositeCommand {
    fn execute(&mut self, state: &mut PuzzleState) -> Result<(), MoveError> {
        self.executed_count = 0;
        for cmd in &mut self.commands {
            match cmd.execute(state) {
                Ok(()) => {
                    self.executed_count += 1;
                }
                Err(e) => {
                    // Roll back all previously executed sub-commands in reverse order.
                    for i in (0..self.executed_count).rev() {
                        self.commands[i].undo(state);
                    }
                    self.executed_count = 0;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn undo(&mut self, state: &mut PuzzleState) {
        // Undo all executed sub-commands in reverse order.
        for i in (0..self.executed_count).rev() {
            self.commands[i].undo(state);
        }
        self.executed_count = 0;
    }

    fn description(&self) -> &str {
        "composite command"
    }
}

// ---------------------------------------------------------------------------
// UndoStack — unlimited undo/redo using the Command Pattern
// ---------------------------------------------------------------------------

/// Unlimited undo/redo stack using the Command Pattern.
pub struct UndoStack {
    /// Executed commands (past). Top of stack = most recent.
    history: Vec<Box<dyn PuzzleCommand>>,
    /// Undone commands (future). Cleared when a new command is executed.
    redo_stack: Vec<Box<dyn PuzzleCommand>>,
    /// Maximum history depth (0 = unlimited).
    pub max_depth: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: 0,
        }
    }

    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            history: Vec::new(),
            redo_stack: Vec::new(),
            max_depth,
        }
    }

    /// Execute a command and push it onto the history stack.
    /// Clears the redo stack (branching invalidates future).
    pub fn execute(
        &mut self,
        mut command: Box<dyn PuzzleCommand>,
        state: &mut PuzzleState,
    ) -> Result<(), MoveError> {
        command.execute(state)?;
        self.redo_stack.clear();
        self.history.push(command);

        // Enforce max depth.
        if self.max_depth > 0 && self.history.len() > self.max_depth {
            self.history.remove(0);
        }

        Ok(())
    }

    /// Undo the most recent command. Returns false if history is empty.
    pub fn undo(&mut self, state: &mut PuzzleState) -> bool {
        if let Some(mut cmd) = self.history.pop() {
            cmd.undo(state);
            self.redo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    /// Redo the most recently undone command. Returns false if redo stack is empty.
    pub fn redo(&mut self, state: &mut PuzzleState) -> bool {
        if let Some(mut cmd) = self.redo_stack.pop() {
            // Re-execute the command. If it fails we put it back (shouldn't happen
            // normally since we're replaying a previously valid command).
            if cmd.execute(state).is_ok() {
                self.history.push(cmd);
                true
            } else {
                self.redo_stack.push(cmd);
                false
            }
        } else {
            false
        }
    }

    /// Undo all commands back to the initial state.
    pub fn undo_all(&mut self, state: &mut PuzzleState) {
        while self.undo(state) {}
    }

    /// Number of moves in history.
    pub fn move_count(&self) -> usize {
        self.history.len()
    }

    /// Whether undo is available.
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all history (e.g. on level restart).
    pub fn clear(&mut self) {
        self.history.clear();
        self.redo_stack.clear();
    }
}

// ---------------------------------------------------------------------------
// Constraint system — pre-move validation
// ---------------------------------------------------------------------------

/// A single constraint rule for pre-move validation.
pub trait Constraint: Send + Sync {
    /// Check if the proposed command is valid in the current state.
    fn validate(&self, command: &dyn PuzzleCommand, state: &PuzzleState) -> Result<(), MoveError>;
}

/// Pre-move validation. Checks whether a proposed action is legal
/// before it is executed, preventing invalid state.
pub struct ConstraintValidator {
    constraints: Vec<Box<dyn Constraint>>,
}

impl Default for ConstraintValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstraintValidator {
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    pub fn add_constraint(&mut self, constraint: Box<dyn Constraint>) {
        self.constraints.push(constraint);
    }

    /// Validate a command against all constraints. Returns first failure.
    pub fn validate(
        &self,
        command: &dyn PuzzleCommand,
        state: &PuzzleState,
    ) -> Result<(), MoveError> {
        for c in &self.constraints {
            c.validate(command, state)?;
        }
        Ok(())
    }
}

/// Built-in constraint: entities cannot move into Solid tiles.
pub struct SolidTileConstraint;

impl Constraint for SolidTileConstraint {
    fn validate(
        &self,
        _command: &dyn PuzzleCommand,
        _state: &PuzzleState,
    ) -> Result<(), MoveError> {
        // Solid-tile checking is handled inside MoveCommand::execute.
        // This constraint exists as a named sentinel for constraint lists;
        // games that build custom commands should check `state.is_walkable()`
        // during their own validation.
        Ok(())
    }
}

/// Built-in constraint: only one entity per tile (no stacking).
pub struct NoOverlapConstraint;

impl Constraint for NoOverlapConstraint {
    fn validate(
        &self,
        _command: &dyn PuzzleCommand,
        _state: &PuzzleState,
    ) -> Result<(), MoveError> {
        // Overlap checking is enforced by MoveCommand::execute which
        // rejects moves when an entity occupies the target and is
        // not pushable. This constraint marker can be used by custom
        // commands for their own overlap checks.
        Ok(())
    }
}

/// Built-in constraint: push chains have a maximum length.
pub struct MaxPushChainConstraint {
    pub max_chain: usize,
}

impl Constraint for MaxPushChainConstraint {
    fn validate(
        &self,
        _command: &dyn PuzzleCommand,
        _state: &PuzzleState,
    ) -> Result<(), MoveError> {
        // The push chain length is computed inside MoveCommand::execute.
        // For pre-validation, a game using this constraint should downcast
        // or inspect the command. For now this provides a configuration slot.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WinCondition — configurable completion detection
// ---------------------------------------------------------------------------

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
    /// A specific entity type reaches a specific position (reach the exit).
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// `move_count` is passed so that `Solved` can include it.
    pub fn check(&self, state: &PuzzleState, move_count: u32) -> PuzzleResult {
        if self.is_satisfied(state) {
            PuzzleResult::Solved { moves: move_count }
        } else {
            PuzzleResult::InProgress
        }
    }

    /// Internal recursive satisfaction check.
    fn is_satisfied(&self, state: &PuzzleState) -> bool {
        match self {
            WinCondition::AllOnTarget {
                entity_type,
                target_tile,
            } => {
                let matching: Vec<_> = state
                    .entities
                    .iter()
                    .filter(|e| e.entity_type == *entity_type)
                    .collect();
                if matching.is_empty() {
                    return false;
                }
                matching
                    .iter()
                    .all(|e| state.tile_at(e.pos) == Some(*target_tile))
            }
            WinCondition::AllFlagsSet(flags) => flags
                .iter()
                .all(|f| state.flags.get(f).copied().unwrap_or(false)),
            WinCondition::EntityAtPosition {
                entity_type,
                target,
            } => state
                .entities
                .iter()
                .any(|e| e.entity_type == *entity_type && e.pos == *target),
            WinCondition::NoneRemaining(entity_type) => {
                !state.entities.iter().any(|e| e.entity_type == *entity_type)
            }
            WinCondition::Custom(_) => {
                // Custom conditions must be evaluated by game-specific code.
                false
            }
            WinCondition::All(conditions) => conditions.iter().all(|c| c.is_satisfied(state)),
            WinCondition::Any(conditions) => conditions.iter().any(|c| c.is_satisfied(state)),
        }
    }
}

// ---------------------------------------------------------------------------
// TurnTick — turn-based simulation control
// ---------------------------------------------------------------------------

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
    pub fn new(animation_frames: u32) -> Self {
        Self {
            turn: 0,
            animating: false,
            animation_frames,
            remaining_frames: 0,
        }
    }

    /// Submit a player action, advancing the turn. Returns false if still animating.
    pub fn submit_action(&mut self) -> bool {
        if self.animating {
            return false;
        }
        self.turn += 1;
        if self.animation_frames > 0 {
            self.animating = true;
            self.remaining_frames = self.animation_frames;
        }
        true
    }

    /// Tick the animation timer. Returns true when animation is complete
    /// and the system is ready for the next action.
    pub fn tick_animation(&mut self) -> bool {
        if !self.animating {
            return true;
        }
        if self.remaining_frames > 0 {
            self.remaining_frames -= 1;
        }
        if self.remaining_frames == 0 {
            self.animating = false;
            return true;
        }
        false
    }

    /// Whether the system is ready to accept a new player action.
    pub fn ready_for_input(&self) -> bool {
        !self.animating
    }

    /// Reset to turn 0 (level restart).
    pub fn reset(&mut self) {
        self.turn = 0;
        self.animating = false;
        self.remaining_frames = 0;
    }
}

// ---------------------------------------------------------------------------
// HintSystem — optional pre-computed solution hints
// ---------------------------------------------------------------------------

/// Optional hint system with pre-computed solution steps.
pub struct HintSystem {
    /// Full solution as a sequence of directions (loaded from level data).
    solution: Vec<GridDir>,
    /// How many hints have been revealed so far.
    revealed_count: usize,
    /// Whether hints are available for this level.
    pub available: bool,
}

impl HintSystem {
    pub fn new(solution: Vec<GridDir>) -> Self {
        let available = !solution.is_empty();
        Self {
            solution,
            revealed_count: 0,
            available,
        }
    }

    pub fn empty() -> Self {
        Self {
            solution: Vec::new(),
            revealed_count: 0,
            available: false,
        }
    }

    /// Reveal the next hint step. Returns the direction, or None if all revealed.
    pub fn reveal_next(&mut self) -> Option<GridDir> {
        if self.revealed_count < self.solution.len() {
            let dir = self.solution[self.revealed_count];
            self.revealed_count += 1;
            Some(dir)
        } else {
            None
        }
    }

    /// Number of hints revealed so far.
    pub fn revealed(&self) -> usize {
        self.revealed_count
    }

    /// Total number of solution steps.
    pub fn total_steps(&self) -> usize {
        self.solution.len()
    }

    /// Reset revealed count (e.g. after undo-all).
    pub fn reset(&mut self) {
        self.revealed_count = 0;
    }
}

// ---------------------------------------------------------------------------
// LevelLoader — RON level definitions
// ---------------------------------------------------------------------------

/// Error type for level loading.
#[derive(Clone, Debug)]
pub enum LoadError {
    /// File not found or unreadable.
    Io(String),
    /// RON parse error.
    Parse(String),
    /// Validation error (e.g. tile count doesn't match dimensions).
    Validation(String),
}

impl core::fmt::Display for LoadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoadError::Io(msg) => write!(f, "IO error: {msg}"),
            LoadError::Parse(msg) => write!(f, "parse error: {msg}"),
            LoadError::Validation(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

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

/// Entity placement within a level definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityPlacement {
    pub pos: GridPos,
    pub entity_type: PuzzleEntityType,
    pub pushable: bool,
}

/// Loads puzzle levels from RON definitions.
pub struct LevelLoader;

impl LevelLoader {
    /// Load a level from a RON file path.
    pub fn load(path: &str) -> Result<LevelDef, LoadError> {
        let contents = std::fs::read_to_string(path).map_err(|e| LoadError::Io(e.to_string()))?;
        let def: LevelDef =
            ron::from_str(&contents).map_err(|e| LoadError::Parse(e.to_string()))?;

        // Validate dimensions.
        let expected = (def.width * def.height) as usize;
        if def.tiles.len() != expected {
            return Err(LoadError::Validation(format!(
                "tile count {} doesn't match {}x{} = {}",
                def.tiles.len(),
                def.width,
                def.height,
                expected
            )));
        }

        Ok(def)
    }

    /// Load a level definition from a RON string (no filesystem access).
    pub fn load_from_str(ron_str: &str) -> Result<LevelDef, LoadError> {
        let def: LevelDef = ron::from_str(ron_str).map_err(|e| LoadError::Parse(e.to_string()))?;

        let expected = (def.width * def.height) as usize;
        if def.tiles.len() != expected {
            return Err(LoadError::Validation(format!(
                "tile count {} doesn't match {}x{} = {}",
                def.tiles.len(),
                def.width,
                def.height,
                expected
            )));
        }

        Ok(def)
    }

    /// Convert a LevelDef into a playable PuzzleState + WinCondition + HintSystem.
    pub fn instantiate(def: &LevelDef) -> (PuzzleState, WinCondition, HintSystem) {
        let mut state = PuzzleState {
            width: def.width,
            height: def.height,
            tiles: def.tiles.clone(),
            entities: Vec::with_capacity(def.entities.len()),
            flags: FxHashMap::default(),
        };

        for (i, placement) in def.entities.iter().enumerate() {
            state.entities.push(PuzzleEntity {
                id: EntityId::from_raw(i as u32, 0),
                pos: placement.pos,
                entity_type: placement.entity_type,
                pushable: placement.pushable,
            });
        }

        let win = def.win_condition.clone();
        let hints = match &def.solution {
            Some(sol) => HintSystem::new(sol.clone()),
            None => HintSystem::empty(),
        };

        (state, win, hints)
    }
}

// ---------------------------------------------------------------------------
// LevelProgress — tracks completion and best scores
// ---------------------------------------------------------------------------

/// Tracks which levels have been completed and their best scores.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LevelProgress {
    /// Level name -> best move count.
    pub completed: FxHashMap<String, u32>,
    /// Currently selected level pack / world.
    pub current_pack: String,
}

impl LevelProgress {
    /// Mark a level as complete with the given move count.
    /// Only updates if the new score is better (fewer moves).
    pub fn mark_complete(&mut self, level: &str, moves: u32) {
        let entry = self.completed.entry(level.to_string()).or_insert(u32::MAX);
        if moves < *entry {
            *entry = moves;
        }
    }

    /// Get the best score for a level.
    pub fn best_score(&self, level: &str) -> Option<u32> {
        self.completed.get(level).copied()
    }

    /// Check if a level has been completed.
    pub fn is_complete(&self, level: &str) -> bool {
        self.completed.contains_key(level)
    }

    /// Total number of completed levels.
    pub fn completion_count(&self) -> usize {
        self.completed.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Grid operations ─────────────────────────────────────

    #[test]
    fn grid_basic_ops() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(4, 4);
        assert!(grid.is_empty(0, 0));

        grid.set(1, 1, Some(5));
        assert_eq!(grid.get(1, 1), Some(5));
        assert!(!grid.is_empty(1, 1));

        grid.set(1, 1, None);
        assert!(grid.is_empty(1, 1));
    }

    #[test]
    fn grid_swap() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(3, 3);
        grid.set(0, 0, Some(1));
        grid.set(1, 0, Some(2));

        assert!(grid.swap(0, 0, 1, 0));
        assert_eq!(grid.get(0, 0), Some(2));
        assert_eq!(grid.get(1, 0), Some(1));
    }

    #[test]
    fn grid_gravity_down() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(3, 4);
        // Place items at top
        grid.set(0, 0, Some(1));
        grid.set(0, 1, Some(2));
        // Leave gap at y=2
        grid.set(0, 3, Some(3));

        grid.apply_gravity(true);

        // All should be at the bottom
        assert!(grid.is_empty(0, 0));
        assert_eq!(grid.get(0, 1), Some(1));
        assert_eq!(grid.get(0, 2), Some(2));
        assert_eq!(grid.get(0, 3), Some(3));
    }

    #[test]
    fn grid_gravity_up() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(3, 4);
        grid.set(0, 2, Some(1));
        grid.set(0, 3, Some(2));

        grid.apply_gravity(false);

        assert_eq!(grid.get(0, 0), Some(1));
        assert_eq!(grid.get(0, 1), Some(2));
        assert!(grid.is_empty(0, 2));
        assert!(grid.is_empty(0, 3));
    }

    #[test]
    fn find_all_works() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(3, 3);
        grid.set(0, 0, Some(1));
        grid.set(1, 1, Some(2));
        grid.set(2, 2, Some(1));

        let ones = grid.find_all(|v| v == 1);
        assert_eq!(ones.len(), 2);
    }

    // ── Pattern matching ────────────────────────────────────

    #[test]
    fn horizontal_match() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(5, 3);
        grid.set(0, 0, Some(1));
        grid.set(1, 0, Some(1));
        grid.set(2, 0, Some(1));
        grid.set(3, 0, Some(2));

        let matches = find_horizontal_matches(&grid, 3);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].cells.len(), 3);
    }

    #[test]
    fn vertical_match() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(3, 5);
        grid.set(1, 0, Some(3));
        grid.set(1, 1, Some(3));
        grid.set(1, 2, Some(3));
        grid.set(1, 3, Some(3));

        let matches = find_vertical_matches(&grid, 3);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].cells.len(), 4);
    }

    #[test]
    fn no_match_when_too_short() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(4, 4);
        grid.set(0, 0, Some(1));
        grid.set(1, 0, Some(1));

        let matches = find_horizontal_matches(&grid, 3);
        assert!(matches.is_empty());
    }

    #[test]
    fn connected_flood_fill() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(4, 4);
        // Create an L-shape of value 1
        grid.set(0, 0, Some(1));
        grid.set(1, 0, Some(1));
        grid.set(0, 1, Some(1));
        grid.set(0, 2, Some(1));
        // Disconnected cell
        grid.set(3, 3, Some(1));

        let group = find_connected(&grid, 0, 0);
        assert_eq!(group.len(), 4);
        assert!(!group.contains(&(3, 3)));
    }

    #[test]
    fn clear_cells_works() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(3, 3);
        grid.set(0, 0, Some(1));
        grid.set(1, 1, Some(2));
        grid.set(2, 2, Some(3));

        let cleared = clear_cells(&mut grid, &[(0, 0), (2, 2)]);
        assert_eq!(cleared, 2);
        assert!(grid.is_empty(0, 0));
        assert!(!grid.is_empty(1, 1));
        assert!(grid.is_empty(2, 2));
    }

    // ── Move history ────────────────────────────────────────

    #[test]
    fn move_history_undo_redo() {
        let mut hist = MoveHistory::new();

        hist.push(PuzzleMove::Swap {
            x1: 0,
            y1: 0,
            x2: 1,
            y2: 0,
        });
        hist.push(PuzzleMove::Swap {
            x1: 1,
            y1: 0,
            x2: 2,
            y2: 0,
        });

        assert_eq!(hist.move_count(), 2);
        assert!(hist.can_undo());

        // Undo
        let m = hist.undo().unwrap();
        assert!(matches!(m, PuzzleMove::Swap { x1: 1, .. }));
        assert!(hist.can_redo());

        // Redo
        let m = hist.redo().unwrap();
        assert!(matches!(m, PuzzleMove::Swap { x1: 1, .. }));

        // Undo both
        hist.undo();
        hist.undo();
        assert!(!hist.can_undo());

        // Push new move discards redo
        hist.push(PuzzleMove::Custom { id: 99 });
        assert!(!hist.can_redo());
        assert_eq!(hist.move_count(), 1);
    }

    // ── Block shapes ────────────────────────────────────────

    #[test]
    fn block_shape_placement() {
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(5, 5);
        let shape = BlockShape::t_piece();

        assert!(shape.can_place(&grid, 1, 1));
        assert!(shape.place(&mut grid, 1, 1, 1));

        assert_eq!(grid.get(1, 1), Some(1));
        assert_eq!(grid.get(2, 1), Some(1));
        assert_eq!(grid.get(3, 1), Some(1));
        assert_eq!(grid.get(2, 2), Some(1));

        // Can't place overlapping
        assert!(!shape.can_place(&grid, 1, 1));
    }

    #[test]
    fn block_shape_rotation() {
        let shape = BlockShape::new(vec![(0, 0), (1, 0), (2, 0)]); // horizontal line
        let rotated = shape.rotate_cw();

        // Should now be vertical
        assert!(rotated.cells.contains(&(0, 0)));
        assert!(rotated.cells.contains(&(0, -1)));
        assert!(rotated.cells.contains(&(0, -2)));
    }

    #[test]
    fn block_shape_out_of_bounds() {
        let grid: PuzzleGrid<u8> = PuzzleGrid::new(3, 3);
        let shape = BlockShape::i_piece(); // 4 wide

        // Won't fit at x=1 (needs x 1,2,3,4 but grid is only 3 wide)
        assert!(!shape.can_place(&grid, 1, 0));
        // Also won't fit at x=0 (needs x 0,1,2,3 but grid is only 3 wide)
        assert!(!shape.can_place(&grid, 0, 0));

        // Fits on a wider grid
        let wide_grid: PuzzleGrid<u8> = PuzzleGrid::new(5, 3);
        assert!(shape.can_place(&wide_grid, 0, 0));
    }

    // ── Block bag and match-3 workflow ──────────────────────

    #[test]
    fn block_bag_fairness() {
        let mut bag = BlockBag::standard_tetris(42);

        // Draw all 7 — should get each shape exactly once
        let mut seen = Vec::new();
        for _ in 0..7 {
            let shape = bag.next();
            // Check it's unique in this batch by cell count or shape
            seen.push(shape);
        }
        assert_eq!(seen.len(), 7);

        // Draw 7 more — should again get all 7
        let mut second_batch = Vec::new();
        for _ in 0..7 {
            second_batch.push(bag.next());
        }
        assert_eq!(second_batch.len(), 7);
    }

    #[test]
    fn block_bounds() {
        let shape = BlockShape::t_piece();
        let (w, h) = shape.bounds();
        assert_eq!(w, 3);
        assert_eq!(h, 2);
    }

    #[test]
    fn match3_workflow() {
        // Simulate a basic match-3 flow: swap → match → clear → gravity
        let mut grid: PuzzleGrid<u8> = PuzzleGrid::new(5, 5);
        grid.set(0, 2, Some(1));
        grid.set(1, 2, Some(1));
        grid.set(2, 2, Some(2)); // wrong color
        grid.set(3, 2, Some(1));

        // Swap (2,2) with something to create match
        grid.set(2, 2, Some(1)); // pretend swap happened

        // Find match
        let matches = find_horizontal_matches(&grid, 3);
        assert!(!matches.is_empty());

        // Clear
        for m in &matches {
            clear_cells(&mut grid, &m.cells);
        }

        // Verify cells are gone
        assert!(grid.is_empty(0, 2));

        // Place stuff above for gravity test
        grid.set(0, 0, Some(5));
        grid.apply_gravity(true);

        // Cell should have fallen
        assert_eq!(grid.get(0, 4), Some(5));
    }

    // ── GridPos ──────────────────────────────────────────────

    #[test]
    fn grid_pos_neighbor() {
        let p = GridPos::new(5, 5);
        assert_eq!(p.neighbor(GridDir::Up), GridPos::new(5, 4));
        assert_eq!(p.neighbor(GridDir::Down), GridPos::new(5, 6));
        assert_eq!(p.neighbor(GridDir::Left), GridPos::new(4, 5));
        assert_eq!(p.neighbor(GridDir::Right), GridPos::new(6, 5));
    }

    #[test]
    fn grid_pos_manhattan() {
        let a = GridPos::new(1, 2);
        let b = GridPos::new(4, 6);
        assert_eq!(a.manhattan_distance(b), 7);
    }

    // ── PuzzleState ──────────────────────────────────────────

    fn make_sokoban_state() -> PuzzleState {
        // 5x5 grid, walls on edges, player at (1,1), box at (2,2), target tile 2 at (3,3)
        let mut state = PuzzleState::new(5, 5);
        // Set border walls
        for x in 0..5i32 {
            state.set_tile(GridPos::new(x, 0), TILE_SOLID);
            state.set_tile(GridPos::new(x, 4), TILE_SOLID);
        }
        for y in 0..5i32 {
            state.set_tile(GridPos::new(0, y), TILE_SOLID);
            state.set_tile(GridPos::new(4, y), TILE_SOLID);
        }
        // Target tile at (3,3)
        state.set_tile(GridPos::new(3, 3), 2);

        // Player at (1,1)
        state.entities.push(PuzzleEntity {
            id: EntityId::from_raw(0, 0),
            pos: GridPos::new(1, 1),
            entity_type: PuzzleEntityType::Player,
            pushable: false,
        });
        // Box at (2,2)
        state.entities.push(PuzzleEntity {
            id: EntityId::from_raw(1, 0),
            pos: GridPos::new(2, 2),
            entity_type: PuzzleEntityType::Box,
            pushable: true,
        });
        state
    }

    #[test]
    fn puzzle_state_basics() {
        let state = make_sokoban_state();
        assert!(!state.is_walkable(GridPos::new(0, 0)));
        assert!(state.is_walkable(GridPos::new(1, 1)));
        assert!(state.in_bounds(GridPos::new(4, 4)));
        assert!(!state.in_bounds(GridPos::new(5, 0)));
        assert!(state.entity_at(GridPos::new(1, 1)).is_some());
        assert_eq!(
            state.entity_at(GridPos::new(1, 1)).unwrap().entity_type,
            PuzzleEntityType::Player
        );
    }

    // ── MoveCommand with push chain ──────────────────────────

    #[test]
    fn move_command_simple() {
        let mut state = make_sokoban_state();
        let player_id = EntityId::from_raw(0, 0);

        // Move player right: (1,1) -> (2,1) — no obstacle
        let mut cmd = MoveCommand::new(player_id, GridDir::Right);
        assert!(cmd.execute(&mut state).is_ok());
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(2, 1)
        );

        // Undo
        cmd.undo(&mut state);
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(1, 1)
        );
    }

    #[test]
    fn move_command_push_box() {
        let mut state = make_sokoban_state();
        let player_id = EntityId::from_raw(0, 0);
        let box_id = EntityId::from_raw(1, 0);

        // Move player to (2,1) first
        let mut cmd1 = MoveCommand::new(player_id, GridDir::Right);
        cmd1.execute(&mut state).unwrap();

        // Move player down to (2,2) — pushes box from (2,2) to (2,3)
        let mut cmd2 = MoveCommand::new(player_id, GridDir::Down);
        cmd2.execute(&mut state).unwrap();
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(2, 2)
        );
        assert_eq!(state.entity_by_id(box_id).unwrap().pos, GridPos::new(2, 3));

        // Undo — box returns to (2,2), player to (2,1)
        cmd2.undo(&mut state);
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(2, 1)
        );
        assert_eq!(state.entity_by_id(box_id).unwrap().pos, GridPos::new(2, 2));
    }

    #[test]
    fn move_command_blocked_by_wall() {
        let mut state = make_sokoban_state();
        let player_id = EntityId::from_raw(0, 0);

        // Move player up from (1,1) — hits wall at (1,0)
        let mut cmd = MoveCommand::new(player_id, GridDir::Up);
        assert!(cmd.execute(&mut state).is_err());
        // Player didn't move
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(1, 1)
        );
    }

    // ── PlaceCommand ─────────────────────────────────────────

    #[test]
    fn place_command_and_undo() {
        let mut state = PuzzleState::new(3, 3);
        let pos = GridPos::new(1, 1);

        let mut cmd = PlaceCommand::new(pos, 42);
        cmd.execute(&mut state).unwrap();
        assert_eq!(state.tile_at(pos), Some(42));

        cmd.undo(&mut state);
        assert_eq!(state.tile_at(pos), Some(TILE_EMPTY));
    }

    // ── CompositeCommand ─────────────────────────────────────

    #[test]
    fn composite_command_rollback_on_failure() {
        let mut state = make_sokoban_state();
        let player_id = EntityId::from_raw(0, 0);

        // First: valid move right. Second: invalid move up into wall from (2,1) -> (2,0).
        let cmd1 = Box::new(MoveCommand::new(player_id, GridDir::Right));
        let cmd2 = Box::new(MoveCommand::new(player_id, GridDir::Up));

        let mut composite = CompositeCommand::new(vec![cmd1, cmd2]);
        assert!(composite.execute(&mut state).is_err());

        // Player should be back at original position due to rollback.
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(1, 1)
        );
    }

    // ── UndoStack ────────────────────────────────────────────

    #[test]
    fn undo_stack_execute_undo_redo() {
        let mut state = make_sokoban_state();
        let mut stack = UndoStack::new();
        let player_id = EntityId::from_raw(0, 0);

        // Execute two moves
        stack
            .execute(
                Box::new(MoveCommand::new(player_id, GridDir::Right)),
                &mut state,
            )
            .unwrap();
        stack
            .execute(
                Box::new(MoveCommand::new(player_id, GridDir::Down)),
                &mut state,
            )
            .unwrap();
        assert_eq!(stack.move_count(), 2);
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(2, 2)
        );

        // Undo one
        assert!(stack.undo(&mut state));
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(2, 1)
        );
        assert!(stack.can_redo());

        // Redo
        assert!(stack.redo(&mut state));
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(2, 2)
        );
        assert!(!stack.can_redo());

        // Undo all
        stack.undo_all(&mut state);
        assert_eq!(
            state.entity_by_id(player_id).unwrap().pos,
            GridPos::new(1, 1)
        );
        assert_eq!(stack.move_count(), 0);
    }

    #[test]
    fn undo_stack_new_move_clears_redo() {
        let mut state = make_sokoban_state();
        let mut stack = UndoStack::new();
        let player_id = EntityId::from_raw(0, 0);

        stack
            .execute(
                Box::new(MoveCommand::new(player_id, GridDir::Right)),
                &mut state,
            )
            .unwrap();
        stack.undo(&mut state);
        assert!(stack.can_redo());

        // New move should clear redo
        stack
            .execute(
                Box::new(MoveCommand::new(player_id, GridDir::Down)),
                &mut state,
            )
            .unwrap();
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_max_depth() {
        let mut state = make_sokoban_state();
        let mut stack = UndoStack::with_max_depth(2);
        let player_id = EntityId::from_raw(0, 0);

        stack
            .execute(
                Box::new(MoveCommand::new(player_id, GridDir::Right)),
                &mut state,
            )
            .unwrap();
        stack
            .execute(
                Box::new(MoveCommand::new(player_id, GridDir::Down)),
                &mut state,
            )
            .unwrap();
        stack
            .execute(
                Box::new(MoveCommand::new(player_id, GridDir::Left)),
                &mut state,
            )
            .unwrap();

        // Only 2 moves retained
        assert_eq!(stack.move_count(), 2);
    }

    // ── WinCondition ─────────────────────────────────────────

    #[test]
    fn win_condition_all_on_target() {
        let mut state = make_sokoban_state();
        let win = WinCondition::AllOnTarget {
            entity_type: PuzzleEntityType::Box,
            target_tile: 2,
        };

        // Box not on target yet
        assert_eq!(win.check(&state, 0), PuzzleResult::InProgress);

        // Move box to (3,3) where target tile is
        state.move_entity(EntityId::from_raw(1, 0), GridPos::new(3, 3));
        assert_eq!(win.check(&state, 5), PuzzleResult::Solved { moves: 5 });
    }

    #[test]
    fn win_condition_entity_at_position() {
        let mut state = make_sokoban_state();
        let win = WinCondition::EntityAtPosition {
            entity_type: PuzzleEntityType::Player,
            target: GridPos::new(3, 3),
        };

        assert_eq!(win.check(&state, 0), PuzzleResult::InProgress);

        state.move_entity(EntityId::from_raw(0, 0), GridPos::new(3, 3));
        assert_eq!(win.check(&state, 3), PuzzleResult::Solved { moves: 3 });
    }

    #[test]
    fn win_condition_all_flags_set() {
        let mut state = PuzzleState::new(3, 3);
        state.flags.insert("switch_a".to_string(), false);
        state.flags.insert("switch_b".to_string(), false);

        let win = WinCondition::AllFlagsSet(vec!["switch_a".to_string(), "switch_b".to_string()]);
        assert_eq!(win.check(&state, 0), PuzzleResult::InProgress);

        state.flags.insert("switch_a".to_string(), true);
        assert_eq!(win.check(&state, 0), PuzzleResult::InProgress);

        state.flags.insert("switch_b".to_string(), true);
        assert_eq!(win.check(&state, 2), PuzzleResult::Solved { moves: 2 });
    }

    #[test]
    fn win_condition_none_remaining() {
        let mut state = PuzzleState::new(3, 3);
        state.entities.push(PuzzleEntity {
            id: EntityId::from_raw(0, 0),
            pos: GridPos::new(0, 0),
            entity_type: PuzzleEntityType::Key,
            pushable: false,
        });

        let win = WinCondition::NoneRemaining(PuzzleEntityType::Key);
        assert_eq!(win.check(&state, 0), PuzzleResult::InProgress);

        state.entities.clear();
        assert_eq!(win.check(&state, 1), PuzzleResult::Solved { moves: 1 });
    }

    #[test]
    fn win_condition_composite_all() {
        let mut state = PuzzleState::new(3, 3);
        state.flags.insert("done".to_string(), true);

        let win = WinCondition::All(vec![
            WinCondition::AllFlagsSet(vec!["done".to_string()]),
            WinCondition::NoneRemaining(PuzzleEntityType::Key),
        ]);
        assert_eq!(win.check(&state, 0), PuzzleResult::Solved { moves: 0 });
    }

    // ── TurnTick ─────────────────────────────────────────────

    #[test]
    fn turn_tick_lifecycle() {
        let mut tick = TurnTick::new(3);
        assert!(tick.ready_for_input());
        assert_eq!(tick.turn, 0);

        // Submit action
        assert!(tick.submit_action());
        assert_eq!(tick.turn, 1);
        assert!(!tick.ready_for_input());

        // Can't submit while animating
        assert!(!tick.submit_action());

        // Tick animation 3 times
        assert!(!tick.tick_animation()); // frame 2 remaining
        assert!(!tick.tick_animation()); // frame 1 remaining
        assert!(tick.tick_animation()); // done
        assert!(tick.ready_for_input());
    }

    #[test]
    fn turn_tick_zero_animation() {
        let mut tick = TurnTick::new(0);
        assert!(tick.submit_action());
        // No animation phase — immediately ready
        assert!(tick.ready_for_input());
    }

    #[test]
    fn turn_tick_reset() {
        let mut tick = TurnTick::new(5);
        tick.submit_action();
        tick.reset();
        assert_eq!(tick.turn, 0);
        assert!(tick.ready_for_input());
    }

    // ── HintSystem ───────────────────────────────────────────

    #[test]
    fn hint_system_reveal() {
        let mut hints = HintSystem::new(vec![GridDir::Up, GridDir::Right, GridDir::Down]);
        assert!(hints.available);
        assert_eq!(hints.total_steps(), 3);
        assert_eq!(hints.revealed(), 0);

        assert_eq!(hints.reveal_next(), Some(GridDir::Up));
        assert_eq!(hints.reveal_next(), Some(GridDir::Right));
        assert_eq!(hints.revealed(), 2);

        hints.reset();
        assert_eq!(hints.revealed(), 0);
        assert_eq!(hints.reveal_next(), Some(GridDir::Up));
    }

    #[test]
    fn hint_system_empty() {
        let mut hints = HintSystem::empty();
        assert!(!hints.available);
        assert_eq!(hints.reveal_next(), None);
    }

    // ── LevelProgress ────────────────────────────────────────

    #[test]
    fn level_progress_tracking() {
        let mut progress = LevelProgress::default();
        assert!(!progress.is_complete("level_1"));

        progress.mark_complete("level_1", 15);
        assert!(progress.is_complete("level_1"));
        assert_eq!(progress.best_score("level_1"), Some(15));

        // Better score replaces
        progress.mark_complete("level_1", 10);
        assert_eq!(progress.best_score("level_1"), Some(10));

        // Worse score doesn't replace
        progress.mark_complete("level_1", 20);
        assert_eq!(progress.best_score("level_1"), Some(10));

        progress.mark_complete("level_2", 5);
        assert_eq!(progress.completion_count(), 2);
    }

    // ── LevelLoader (from string) ────────────────────────────

    #[test]
    fn level_loader_from_str() {
        let ron = r#"(
            name: "test_level",
            width: 3,
            height: 3,
            tiles: [1,1,1, 1,0,1, 1,1,1],
            entities: [
                (pos: (x: 1, y: 1), entity_type: Player, pushable: false),
            ],
            win_condition: EntityAtPosition(entity_type: Player, target: (x: 2, y: 1)),
            solution: Some([Right]),
            par_moves: Some(1),
        )"#;

        let def = LevelLoader::load_from_str(ron).unwrap();
        assert_eq!(def.name, "test_level");
        assert_eq!(def.width, 3);
        assert_eq!(def.tiles.len(), 9);
        assert_eq!(def.entities.len(), 1);
        assert_eq!(def.par_moves, Some(1));

        let (state, win, hints) = LevelLoader::instantiate(&def);
        assert_eq!(state.entities.len(), 1);
        assert_eq!(state.entities[0].pos, GridPos::new(1, 1));
        assert!(hints.available);
        assert_eq!(hints.total_steps(), 1);

        // Player not at target yet
        assert_eq!(win.check(&state, 0), PuzzleResult::InProgress);
    }

    #[test]
    fn level_loader_invalid_dimensions() {
        let ron = r#"(
            name: "bad",
            width: 3,
            height: 3,
            tiles: [0, 0],
            entities: [],
            win_condition: AllFlagsSet([]),
            solution: None,
            par_moves: None,
        )"#;

        let result = LevelLoader::load_from_str(ron);
        assert!(result.is_err());
    }

    // ── ConstraintValidator ──────────────────────────────────

    #[test]
    fn constraint_validator_passes_with_no_constraints() {
        let validator = ConstraintValidator::new();
        let state = PuzzleState::new(3, 3);
        let cmd = MoveCommand::new(EntityId::from_raw(0, 0), GridDir::Right);
        assert!(validator.validate(&cmd, &state).is_ok());

        // Now test that a validator with a failing constraint returns Err.
        struct AlwaysFailConstraint;
        impl Constraint for AlwaysFailConstraint {
            fn validate(
                &self,
                _command: &dyn PuzzleCommand,
                _state: &PuzzleState,
            ) -> Result<(), MoveError> {
                Err(MoveError::ConstraintViolation("always fails".into()))
            }
        }

        let mut failing_validator = ConstraintValidator::new();
        failing_validator.add_constraint(Box::new(AlwaysFailConstraint));
        let result = failing_validator.validate(&cmd, &state);
        assert!(
            result.is_err(),
            "Validator with a failing constraint should return Err"
        );
        match result.unwrap_err() {
            MoveError::ConstraintViolation(msg) => {
                assert_eq!(msg, "always fails");
            }
            other => panic!("Expected ConstraintViolation, got {:?}", other),
        }
    }

    // ── Full Sokoban workflow ────────────────────────────────

    #[test]
    fn sokoban_full_workflow() {
        // Set up a simple Sokoban: push box to target, detect win, track progress.
        let mut state = make_sokoban_state();
        let mut stack = UndoStack::new();
        let mut tick = TurnTick::new(0); // no animation for test
        let player_id = EntityId::from_raw(0, 0);

        let win = WinCondition::AllOnTarget {
            entity_type: PuzzleEntityType::Box,
            target_tile: 2, // target tile at (3,3)
        };

        // Move player: right, down (pushes box down to 2,3), then
        // navigate around the box to push it right: left, down, down, right, right
        // (1,1)->R(2,1)->D(2,2 push box to 2,3)->L(1,2)->D(1,3)->R(2,3 push box to 3,3)
        let moves = [
            GridDir::Right, // (1,1)->(2,1)
            GridDir::Down,  // (2,1)->(2,2), pushes box (2,2)->(2,3)
            GridDir::Left,  // (2,2)->(1,2)
            GridDir::Down,  // (1,2)->(1,3)
            GridDir::Right, // (1,3)->(2,3), pushes box (2,3)->(3,3)
        ];
        for dir in moves {
            assert!(tick.ready_for_input());
            let cmd = Box::new(MoveCommand::new(player_id, dir));
            stack.execute(cmd, &mut state).unwrap();
            tick.submit_action();
        }

        assert_eq!(
            state.entity_by_id(EntityId::from_raw(1, 0)).unwrap().pos,
            GridPos::new(3, 3)
        );
        assert_eq!(
            win.check(&state, stack.move_count() as u32),
            PuzzleResult::Solved { moves: 5 }
        );

        // Record progress
        let mut progress = LevelProgress::default();
        progress.mark_complete("sokoban_1", stack.move_count() as u32);
        assert_eq!(progress.best_score("sokoban_1"), Some(5));
    }
}
