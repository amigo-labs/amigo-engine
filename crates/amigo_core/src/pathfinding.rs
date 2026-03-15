use crate::math::{Fix, IVec2, SimVec2};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// A* pathfinding request.
#[derive(Clone, Debug)]
pub struct PathRequest {
    pub start: IVec2,
    pub goal: IVec2,
    pub allow_diagonal: bool,
    pub max_search: u32,
}

impl PathRequest {
    pub fn new(start: IVec2, goal: IVec2) -> Self {
        Self { start, goal, allow_diagonal: false, max_search: 1000 }
    }
}

/// Trait for walkability queries on a tile grid.
pub trait Walkable {
    fn is_walkable(&self, x: i32, y: i32) -> bool;
}

#[derive(Clone, Copy)]
struct AStarNode { pos: IVec2, g_cost: u32, f_cost: u32 }

impl Eq for AStarNode {}
impl PartialEq for AStarNode { fn eq(&self, other: &Self) -> bool { self.f_cost == other.f_cost } }
impl Ord for AStarNode { fn cmp(&self, other: &Self) -> Ordering { other.f_cost.cmp(&self.f_cost) } }
impl PartialOrd for AStarNode { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) } }

const DIRS_4: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
const DIRS_8: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

fn heuristic(a: IVec2, b: IVec2) -> u32 {
    ((a.x - b.x).unsigned_abs() + (a.y - b.y).unsigned_abs()) as u32
}

/// Find a path using A* on a tile grid.
pub fn find_path(request: &PathRequest, map: &dyn Walkable) -> Option<Vec<IVec2>> {
    if request.start == request.goal { return Some(vec![request.start]); }
    if !map.is_walkable(request.goal.x, request.goal.y) { return None; }

    let dirs: &[(i32, i32)] = if request.allow_diagonal { &DIRS_8 } else { &DIRS_4 };
    let mut open = BinaryHeap::new();
    let mut came_from = std::collections::HashMap::new();
    let mut g_scores = std::collections::HashMap::new();
    let mut searched = 0u32;

    open.push(AStarNode { pos: request.start, g_cost: 0, f_cost: heuristic(request.start, request.goal) });
    g_scores.insert(request.start, 0u32);

    while let Some(current) = open.pop() {
        if current.pos == request.goal {
            let mut path = vec![request.goal];
            let mut pos = request.goal;
            while let Some(&prev) = came_from.get(&pos) { path.push(prev); pos = prev; }
            path.reverse();
            return Some(path);
        }

        searched += 1;
        if searched >= request.max_search { return None; }

        let current_g = g_scores.get(&current.pos).copied().unwrap_or(u32::MAX);

        for &(dx, dy) in dirs {
            let neighbor = IVec2::new(current.pos.x + dx, current.pos.y + dy);
            if !map.is_walkable(neighbor.x, neighbor.y) { continue; }

            // Diagonal: check adjacent cardinal tiles
            if dx != 0 && dy != 0 {
                if !map.is_walkable(current.pos.x + dx, current.pos.y)
                    || !map.is_walkable(current.pos.x, current.pos.y + dy) { continue; }
            }

            let move_cost = if dx != 0 && dy != 0 { 14 } else { 10 };
            let new_g = current_g + move_cost;

            if new_g < g_scores.get(&neighbor).copied().unwrap_or(u32::MAX) {
                g_scores.insert(neighbor, new_g);
                came_from.insert(neighbor, current.pos);
                open.push(AStarNode { pos: neighbor, g_cost: new_g, f_cost: new_g + heuristic(neighbor, request.goal) });
            }
        }
    }
    None
}

/// A predefined path of waypoints.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaypointPath {
    pub points: Vec<SimVec2>,
}

impl WaypointPath {
    pub fn new(points: Vec<SimVec2>) -> Self { Self { points } }

    pub fn from_f32_pairs(pairs: &[(f32, f32)]) -> Self {
        Self { points: pairs.iter().map(|&(x, y)| SimVec2::from_f32(x, y)).collect() }
    }

    pub fn len(&self) -> usize { self.points.len() }
    pub fn is_empty(&self) -> bool { self.points.is_empty() }
}

/// Component that follows a waypoint path with interpolation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathFollower {
    pub segment: usize,
    pub progress: Fix,
    pub speed: Fix,
    pub finished: bool,
}

impl PathFollower {
    pub fn new(speed: f32) -> Self {
        Self { segment: 0, progress: Fix::ZERO, speed: Fix::from_num(speed), finished: false }
    }

    /// Advance along the path. Returns current interpolated position.
    pub fn update(&mut self, path: &WaypointPath) -> SimVec2 {
        if self.finished || path.points.len() < 2 {
            return path.points.last().copied().unwrap_or(SimVec2::ZERO);
        }
        self.progress += self.speed;
        let one = Fix::from_num(1.0);
        while self.progress >= one {
            self.progress -= one;
            self.segment += 1;
            if self.segment >= path.points.len() - 1 {
                self.segment = path.points.len() - 2;
                self.progress = one;
                self.finished = true;
                break;
            }
        }
        let a = path.points[self.segment];
        let b = path.points[self.segment + 1];
        let t = self.progress;
        SimVec2 { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t }
    }

    pub fn reset(&mut self) {
        self.segment = 0;
        self.progress = Fix::ZERO;
        self.finished = false;
    }
}

/// Flow field: each cell stores direction toward the goal. O(1) per entity lookup.
pub struct FlowField {
    pub width: u32,
    pub height: u32,
    directions: Vec<(i8, i8)>,
    costs: Vec<u32>,
}

impl FlowField {
    /// Compute a flow field from the goal using Dijkstra's algorithm.
    ///
    /// Cardinal moves cost 10, diagonal moves cost 14 (≈10√2).
    /// Diagonal moves are only allowed if both adjacent cardinal cells are walkable.
    pub fn compute(goal: IVec2, width: u32, height: u32, map: &dyn Walkable) -> Self {
        let size = (width * height) as usize;
        let mut costs = vec![u32::MAX; size];
        let mut directions = vec![(0i8, 0i8); size];

        let idx = |x: i32, y: i32| (y as u32 * width + x as u32) as usize;
        let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && x < width as i32 && y < height as i32;

        if !in_bounds(goal.x, goal.y) {
            return Self { width, height, directions, costs };
        }

        let mut queue = BinaryHeap::new();
        costs[idx(goal.x, goal.y)] = 0;
        queue.push(std::cmp::Reverse((0u32, goal)));

        while let Some(std::cmp::Reverse((cost, pos))) = queue.pop() {
            if cost > costs[idx(pos.x, pos.y)] { continue; }

            for &(dx, dy) in &DIRS_8 {
                let nx = pos.x + dx;
                let ny = pos.y + dy;
                if !in_bounds(nx, ny) || !map.is_walkable(nx, ny) { continue; }

                // Diagonal: require both adjacent cardinal cells walkable
                if dx != 0 && dy != 0 {
                    if !map.is_walkable(pos.x + dx, pos.y) || !map.is_walkable(pos.x, pos.y + dy) {
                        continue;
                    }
                }

                let move_cost = if dx != 0 && dy != 0 { 14 } else { 10 };
                let ni = idx(nx, ny);
                let new_cost = cost + move_cost;
                if new_cost < costs[ni] {
                    costs[ni] = new_cost;
                    queue.push(std::cmp::Reverse((new_cost, IVec2::new(nx, ny))));
                }
            }
        }

        // Direction vectors point toward lowest-cost neighbor
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let i = idx(x, y);
                if costs[i] == u32::MAX || costs[i] == 0 { continue; }
                let mut best_dir = (0i8, 0i8);
                let mut best_cost = costs[i];
                for &(dx, dy) in &DIRS_8 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if in_bounds(nx, ny) {
                        let nc = costs[idx(nx, ny)];
                        if nc < best_cost { best_cost = nc; best_dir = (dx as i8, dy as i8); }
                    }
                }
                directions[i] = best_dir;
            }
        }

        Self { width, height, directions, costs }
    }

    pub fn direction_at(&self, x: i32, y: i32) -> (i8, i8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return (0, 0); }
        self.directions[(y as u32 * self.width + x as u32) as usize]
    }

    pub fn cost_at(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 { return u32::MAX; }
        self.costs[(y as u32 * self.width + x as u32) as usize]
    }

    /// Check if a cell is reachable (has a finite cost).
    pub fn is_reachable(&self, x: i32, y: i32) -> bool {
        self.cost_at(x, y) != u32::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple open grid for testing.
    struct OpenGrid { w: i32, h: i32 }
    impl Walkable for OpenGrid {
        fn is_walkable(&self, x: i32, y: i32) -> bool {
            x >= 0 && y >= 0 && x < self.w && y < self.h
        }
    }

    /// Grid with a wall blocking column 5, except at y=0 (gap at top).
    struct WalledGrid { w: i32, h: i32 }
    impl Walkable for WalledGrid {
        fn is_walkable(&self, x: i32, y: i32) -> bool {
            x >= 0 && y >= 0 && x < self.w && y < self.h && !(x == 5 && y > 0)
        }
    }

    /// Grid with a solid wall at column 5, no gaps.
    struct SolidWalledGrid { w: i32, h: i32 }
    impl Walkable for SolidWalledGrid {
        fn is_walkable(&self, x: i32, y: i32) -> bool {
            x >= 0 && y >= 0 && x < self.w && y < self.h && x != 5
        }
    }

    // ── A* tests ────────────────────────────────────────────────

    #[test]
    fn astar_straight_line() {
        let grid = OpenGrid { w: 20, h: 20 };
        let req = PathRequest::new(IVec2::new(0, 0), IVec2::new(5, 0));
        let path = find_path(&req, &grid).unwrap();
        assert_eq!(path.first(), Some(&IVec2::new(0, 0)));
        assert_eq!(path.last(), Some(&IVec2::new(5, 0)));
        assert_eq!(path.len(), 6); // 0,1,2,3,4,5
    }

    #[test]
    fn astar_same_start_goal() {
        let grid = OpenGrid { w: 10, h: 10 };
        let req = PathRequest::new(IVec2::new(3, 3), IVec2::new(3, 3));
        let path = find_path(&req, &grid).unwrap();
        assert_eq!(path.len(), 1);
    }

    #[test]
    fn astar_unreachable_goal() {
        let grid = SolidWalledGrid { w: 10, h: 1 }; // 1-row grid with solid wall at x=5
        let req = PathRequest::new(IVec2::new(0, 0), IVec2::new(9, 0));
        assert!(find_path(&req, &grid).is_none());
    }

    #[test]
    fn astar_around_obstacle() {
        let grid = WalledGrid { w: 10, h: 10 };
        let req = PathRequest::new(IVec2::new(3, 5), IVec2::new(7, 5));
        let path = find_path(&req, &grid).unwrap();
        // Path must not step on any walled cell (x=5, y>0)
        for point in &path {
            assert!(!(point.x == 5 && point.y > 0), "Path should avoid wall at x=5, y>0");
        }
        assert_eq!(*path.first().unwrap(), IVec2::new(3, 5));
        assert_eq!(*path.last().unwrap(), IVec2::new(7, 5));
    }

    // ── FlowField tests ────────────────────────────────────────

    #[test]
    fn flow_field_goal_cost_zero() {
        let grid = OpenGrid { w: 10, h: 10 };
        let field = FlowField::compute(IVec2::new(5, 5), 10, 10, &grid);
        assert_eq!(field.cost_at(5, 5), 0);
    }

    #[test]
    fn flow_field_costs_increase_from_goal() {
        let grid = OpenGrid { w: 10, h: 10 };
        let field = FlowField::compute(IVec2::new(5, 5), 10, 10, &grid);
        // Adjacent cardinal cells cost 10
        assert_eq!(field.cost_at(5, 4), 10);
        assert_eq!(field.cost_at(6, 5), 10);
        // Diagonal neighbor costs 14 (≈10√2)
        assert_eq!(field.cost_at(4, 4), 14);
    }

    #[test]
    fn flow_field_directions_point_toward_goal() {
        let grid = OpenGrid { w: 10, h: 10 };
        let field = FlowField::compute(IVec2::new(5, 5), 10, 10, &grid);

        // Cell to the left of goal should point right (+x)
        let (dx, dy) = field.direction_at(4, 5);
        assert_eq!(dx, 1);
        assert_eq!(dy, 0);

        // Cell above goal should point down (+y)
        let (dx, dy) = field.direction_at(5, 4);
        assert_eq!(dx, 0);
        assert_eq!(dy, 1);
    }

    #[test]
    fn flow_field_unreachable_cells() {
        let grid = SolidWalledGrid { w: 10, h: 1 };
        let field = FlowField::compute(IVec2::new(0, 0), 10, 1, &grid);
        // Wall at x=5 blocks cells beyond it
        assert!(field.is_reachable(0, 0));
        assert!(field.is_reachable(4, 0));
        assert!(!field.is_reachable(5, 0)); // wall
        assert!(!field.is_reachable(6, 0)); // behind wall
    }

    #[test]
    fn flow_field_out_of_bounds() {
        let grid = OpenGrid { w: 5, h: 5 };
        let field = FlowField::compute(IVec2::new(2, 2), 5, 5, &grid);
        assert_eq!(field.cost_at(-1, 0), u32::MAX);
        assert_eq!(field.cost_at(5, 0), u32::MAX);
        assert_eq!(field.direction_at(-1, 0), (0, 0));
    }

    // ── WaypointPath + PathFollower tests ──────────────────────

    #[test]
    fn path_follower_reaches_end() {
        let path = WaypointPath::from_f32_pairs(&[(0.0, 0.0), (10.0, 0.0)]);
        let mut follower = PathFollower::new(0.5);
        let mut pos = SimVec2::ZERO;
        for _ in 0..100 {
            pos = follower.update(&path);
            if follower.finished { break; }
        }
        assert!(follower.finished);
        // Should be at or near the end
        let end_x: f32 = pos.x.to_num();
        assert!((end_x - 10.0).abs() < 0.1);
    }

    #[test]
    fn path_follower_reset() {
        let path = WaypointPath::from_f32_pairs(&[(0.0, 0.0), (10.0, 0.0)]);
        let mut follower = PathFollower::new(1.0);
        follower.update(&path);
        follower.update(&path);
        follower.reset();
        assert_eq!(follower.segment, 0);
        assert!(!follower.finished);
    }
}
