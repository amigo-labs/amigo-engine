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
    /// Compute a flow field from the goal via BFS.
    pub fn compute(goal: IVec2, width: u32, height: u32, map: &dyn Walkable) -> Self {
        let size = (width * height) as usize;
        let mut costs = vec![u32::MAX; size];
        let mut directions = vec![(0i8, 0i8); size];

        let idx = |x: i32, y: i32| (y as u32 * width + x as u32) as usize;
        let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && x < width as i32 && y < height as i32;

        if !in_bounds(goal.x, goal.y) {
            return Self { width, height, directions, costs };
        }

        let mut queue = std::collections::VecDeque::new();
        costs[idx(goal.x, goal.y)] = 0;
        queue.push_back(goal);

        while let Some(pos) = queue.pop_front() {
            let current_cost = costs[idx(pos.x, pos.y)];
            for &(dx, dy) in &DIRS_4 {
                let nx = pos.x + dx;
                let ny = pos.y + dy;
                if !in_bounds(nx, ny) || !map.is_walkable(nx, ny) { continue; }
                let ni = idx(nx, ny);
                let new_cost = current_cost + 1;
                if new_cost < costs[ni] {
                    costs[ni] = new_cost;
                    queue.push_back(IVec2::new(nx, ny));
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
}
