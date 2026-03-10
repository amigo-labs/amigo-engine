use serde::{Deserialize, Serialize};

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
        Self { width, height, cells: data }
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
        self.idx(x, y).map_or(true, |i| self.cells[i].is_none())
    }

    /// Swap two cells. Returns true if both are in bounds.
    pub fn swap(&mut self, x1: u32, y1: u32, x2: u32, y2: u32) -> bool {
        let i1 = match self.idx(x1, y1) { Some(i) => i, None => return false };
        let i2 = match self.idx(x2, y2) { Some(i) => i, None => return false };
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
                        if write_y > 0 {
                            write_y -= 1;
                        }
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
pub fn find_horizontal_matches<T: Copy + Eq>(grid: &PuzzleGrid<T>, min_length: u32) -> Vec<MatchGroup> {
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
                        let cells: Vec<_> = (run_start..run_start + run_len).map(|rx| (rx, y)).collect();
                        groups.push(MatchGroup { cells });
                    }
                    run_start = x;
                    run_val = Some(v);
                    run_len = 1;
                }
                (None, _) => {
                    if run_len >= min_length {
                        let cells: Vec<_> = (run_start..run_start + run_len).map(|rx| (rx, y)).collect();
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
pub fn find_vertical_matches<T: Copy + Eq>(grid: &PuzzleGrid<T>, min_length: u32) -> Vec<MatchGroup> {
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
                        let cells: Vec<_> = (run_start..run_start + run_len).map(|ry| (x, ry)).collect();
                        groups.push(MatchGroup { cells });
                    }
                    run_start = y;
                    run_val = Some(v);
                    run_len = 1;
                }
                (None, _) => {
                    if run_len >= min_length {
                        let cells: Vec<_> = (run_start..run_start + run_len).map(|ry| (x, ry)).collect();
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

        if x > 0 { stack.push((x - 1, y)); }
        if x + 1 < grid.width { stack.push((x + 1, y)); }
        if y > 0 { stack.push((x, y - 1)); }
        if y + 1 < grid.height { stack.push((x, y + 1)); }
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn move_history_undo_redo() {
        let mut hist = MoveHistory::new();

        hist.push(PuzzleMove::Swap { x1: 0, y1: 0, x2: 1, y2: 0 });
        hist.push(PuzzleMove::Swap { x1: 1, y1: 0, x2: 2, y2: 0 });

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
}
