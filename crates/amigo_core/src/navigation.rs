use crate::math::{Fix, IVec2, RenderVec2, SimVec2};
use crate::pathfinding::{self, PathRequest, Walkable};

/// Navigation agent that handles click-to-move with pathfinding.
#[derive(Clone, Debug)]
pub struct NavAgent {
    /// Current world position (simulation space).
    pub position: SimVec2,
    /// Movement speed in tiles per tick.
    pub speed: Fix,
    /// Current path (tile coordinates).
    path: Vec<IVec2>,
    /// Index of next waypoint in path.
    path_index: usize,
    /// Sub-tile interpolation progress (0..1).
    progress: Fix,
    /// Whether the agent is currently moving.
    pub moving: bool,
    /// Whether diagonal movement is allowed.
    pub allow_diagonal: bool,
    /// Size of a tile in world units (for position conversion).
    pub tile_size: f32,
    /// Smoothing factor for movement (0 = instant snap, 1 = fully smooth).
    pub smoothing: f32,
    /// Current facing direction (for animation).
    pub facing: Direction,
    /// Stopping distance in tiles.
    pub stop_distance: u32,
}

/// Cardinal + diagonal directions for facing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

impl Direction {
    /// Get direction from a delta vector.
    pub fn from_delta(dx: f32, dy: f32) -> Self {
        if dx.abs() < 0.001 && dy.abs() < 0.001 {
            return Direction::Down;
        }
        let angle = dy.atan2(dx);
        let octant = ((angle + std::f32::consts::PI) / (std::f32::consts::PI / 4.0)) as i32 % 8;
        match octant {
            0 => Direction::Left,
            1 => Direction::UpLeft,
            2 => Direction::Up,
            3 => Direction::UpRight,
            4 => Direction::Right,
            5 => Direction::DownRight,
            6 => Direction::Down,
            7 => Direction::DownLeft,
            _ => Direction::Down,
        }
    }

    /// Returns the 8 directions as an array.
    pub fn all() -> [Direction; 8] {
        [
            Direction::Up,
            Direction::UpRight,
            Direction::Right,
            Direction::DownRight,
            Direction::Down,
            Direction::DownLeft,
            Direction::Left,
            Direction::UpLeft,
        ]
    }

    /// Convert to a unit vector (dx, dy).
    pub fn to_vec(self) -> (f32, f32) {
        match self {
            Direction::Up => (0.0, -1.0),
            Direction::UpRight => (0.707, -0.707),
            Direction::Right => (1.0, 0.0),
            Direction::DownRight => (0.707, 0.707),
            Direction::Down => (0.0, 1.0),
            Direction::DownLeft => (-0.707, 0.707),
            Direction::Left => (-1.0, 0.0),
            Direction::UpLeft => (-0.707, -0.707),
        }
    }
}

impl NavAgent {
    pub fn new(position: SimVec2, speed: f32, tile_size: f32) -> Self {
        Self {
            position,
            speed: Fix::from_num(speed),
            path: Vec::new(),
            path_index: 0,
            progress: Fix::ZERO,
            moving: false,
            allow_diagonal: true,
            tile_size,
            smoothing: 0.15,
            facing: Direction::Down,
            stop_distance: 0,
        }
    }

    /// Convert world position to tile coordinates.
    pub fn world_to_tile(&self, world_pos: RenderVec2) -> IVec2 {
        IVec2::new(
            (world_pos.x / self.tile_size).floor() as i32,
            (world_pos.y / self.tile_size).floor() as i32,
        )
    }

    /// Convert tile coordinates to world center position.
    pub fn tile_to_world(&self, tile: IVec2) -> SimVec2 {
        SimVec2::from_f32(
            tile.x as f32 * self.tile_size + self.tile_size * 0.5,
            tile.y as f32 * self.tile_size + self.tile_size * 0.5,
        )
    }

    /// Request movement to a world position. Computes path via A*.
    pub fn move_to(&mut self, target: RenderVec2, map: &dyn Walkable) {
        let start = self.world_to_tile(self.position.to_render());
        let goal = self.world_to_tile(target);

        let request = PathRequest {
            start,
            goal,
            allow_diagonal: self.allow_diagonal,
            max_search: 2000,
        };

        if let Some(path) = pathfinding::find_path(&request, map) {
            self.path = path;
            self.path_index = 0;
            self.progress = Fix::ZERO;
            self.moving = !self.path.is_empty();
        } else {
            self.stop();
        }
    }

    /// Request movement to a specific tile.
    pub fn move_to_tile(&mut self, goal: IVec2, map: &dyn Walkable) {
        let start = self.world_to_tile(self.position.to_render());
        let request = PathRequest {
            start,
            goal,
            allow_diagonal: self.allow_diagonal,
            max_search: 2000,
        };

        if let Some(path) = pathfinding::find_path(&request, map) {
            self.path = path;
            self.path_index = 0;
            self.progress = Fix::ZERO;
            self.moving = !self.path.is_empty();
        } else {
            self.stop();
        }
    }

    /// Stop all movement immediately.
    pub fn stop(&mut self) {
        self.path.clear();
        self.path_index = 0;
        self.moving = false;
    }

    /// Update agent position along the path. Call once per tick.
    pub fn update(&mut self) {
        if !self.moving || self.path.is_empty() {
            self.moving = false;
            return;
        }

        // Check if we've reached the end (with stop_distance)
        let effective_end = self.path.len().saturating_sub(self.stop_distance as usize);
        if self.path_index >= effective_end {
            self.moving = false;
            return;
        }

        // Current target waypoint
        let target_tile = self.path[self.path_index.min(self.path.len() - 1)];
        let target_pos = self.tile_to_world(target_tile);

        // Move toward target
        let dx = target_pos.x - self.position.x;
        let dy = target_pos.y - self.position.y;
        let dist_sq = dx * dx + dy * dy;

        let threshold = Fix::from_num(self.tile_size * self.smoothing);
        let threshold_sq = threshold * threshold;

        if dist_sq <= threshold_sq {
            // Snap to waypoint and advance
            self.position = target_pos;
            self.path_index += 1;

            let effective_end = self.path.len().saturating_sub(self.stop_distance as usize);
            if self.path_index >= effective_end {
                self.moving = false;
            }
        } else {
            // Move toward waypoint using deterministic Fix sqrt (no f32)
            let delta = crate::math::SimVec2::new(dx, dy);
            let dist = delta.length();
            if dist > Fix::ZERO {
                let move_x = dx * self.speed / dist;
                let move_y = dy * self.speed / dist;
                self.position.x += move_x;
                self.position.y += move_y;

                // Update facing direction
                self.facing = Direction::from_delta(dx.to_num::<f32>(), dy.to_num::<f32>());
            }
        }
    }

    /// Get the current path for debug rendering.
    pub fn current_path(&self) -> &[IVec2] {
        &self.path
    }

    /// How many waypoints remain.
    pub fn remaining_waypoints(&self) -> usize {
        if self.path_index >= self.path.len() {
            0
        } else {
            self.path.len() - self.path_index
        }
    }

    /// Distance to final destination in world units.
    pub fn distance_to_goal(&self) -> f32 {
        if let Some(&last) = self.path.last() {
            let goal = self.tile_to_world(last);
            let dx = goal.x - self.position.x;
            let dy = goal.y - self.position.y;
            (dx * dx + dy * dy).to_num::<f32>().sqrt()
        } else {
            0.0
        }
    }

    /// Get the render position (f32).
    pub fn render_pos(&self) -> RenderVec2 {
        self.position.to_render()
    }
}

/// System that processes navigation for multiple agents stored in a SparseSet.
pub fn update_nav_agents(agents: &mut crate::ecs::SparseSet<NavAgent>) {
    for (_id, agent) in agents.iter_mut() {
        agent.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SimpleMap {
        width: i32,
        height: i32,
        blocked: Vec<IVec2>,
    }

    impl Walkable for SimpleMap {
        fn is_walkable(&self, x: i32, y: i32) -> bool {
            x >= 0
                && y >= 0
                && x < self.width
                && y < self.height
                && !self.blocked.contains(&IVec2::new(x, y))
        }
    }

    #[test]
    fn nav_agent_moves_to_target() {
        let map = SimpleMap {
            width: 10,
            height: 10,
            blocked: vec![],
        };
        let mut agent = NavAgent::new(
            SimVec2::from_f32(8.0, 8.0), // tile (0,0) center with tile_size=16
            2.0,
            16.0,
        );

        agent.move_to(RenderVec2::new(48.0, 48.0), &map); // tile (3,3)
        assert!(agent.moving);
        assert!(!agent.current_path().is_empty());

        // Run a bunch of ticks
        for _ in 0..200 {
            agent.update();
        }
        assert!(!agent.moving);
    }

    #[test]
    fn direction_from_delta() {
        assert_eq!(Direction::from_delta(1.0, 0.0), Direction::Right);
        assert_eq!(Direction::from_delta(0.0, 1.0), Direction::Down);
        assert_eq!(Direction::from_delta(-1.0, 0.0), Direction::Left);
        assert_eq!(Direction::from_delta(0.0, -1.0), Direction::Up);
    }
}
