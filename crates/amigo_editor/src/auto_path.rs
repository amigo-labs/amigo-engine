//! Auto-pathing — automatically generate waypoint paths through levels.
//!
//! Uses A* pathfinding on the level's collision data to generate
//! smooth waypoint paths between specified points.

use amigo_core::math::IVec2;
use amigo_core::pathfinding::{self, PathRequest, Walkable};

// ---------------------------------------------------------------------------
// Grid adapter for level tile data
// ---------------------------------------------------------------------------

/// Walkability adapter for raw tile data.
pub struct TileGrid<'a> {
    tiles: &'a [u16],
    width: u32,
    height: u32,
    /// Tile IDs considered solid (non-walkable). If empty, any non-zero tile is solid.
    solid_ids: Vec<u16>,
}

impl<'a> TileGrid<'a> {
    pub fn new(tiles: &'a [u16], width: u32, height: u32) -> Self {
        Self {
            tiles,
            width,
            height,
            solid_ids: Vec::new(),
        }
    }

    /// Set specific tile IDs as solid.
    pub fn with_solid_ids(mut self, ids: Vec<u16>) -> Self {
        self.solid_ids = ids;
        self
    }
}

impl<'a> Walkable for TileGrid<'a> {
    fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        let idx = (y as u32 * self.width + x as u32) as usize;
        if idx >= self.tiles.len() {
            return false;
        }
        let tile = self.tiles[idx];
        if self.solid_ids.is_empty() {
            tile == 0 // default: 0 = walkable
        } else {
            !self.solid_ids.contains(&tile)
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-path generation
// ---------------------------------------------------------------------------

/// Configuration for auto-path generation.
#[derive(Clone, Debug)]
pub struct AutoPathConfig {
    /// Allow diagonal movement.
    pub allow_diagonal: bool,
    /// Maximum A* search depth.
    pub max_search: u32,
    /// Simplify the path by removing collinear points.
    pub simplify: bool,
    /// Minimum distance between path points after simplification.
    pub min_point_distance: f32,
}

impl Default for AutoPathConfig {
    fn default() -> Self {
        Self {
            allow_diagonal: false,
            max_search: 5000,
            simplify: true,
            min_point_distance: 2.0,
        }
    }
}

/// Result of auto-path generation.
#[derive(Clone, Debug)]
pub struct GeneratedPath {
    /// Waypoints in world coordinates.
    pub points: Vec<(f32, f32)>,
    /// Whether the path forms a closed loop.
    pub closed: bool,
    /// Total path length in world units.
    pub length: f32,
}

/// Generate a path between two world-space points through a walkable grid.
///
/// The `tile_size` converts between world coordinates and grid coordinates.
pub fn generate_path(
    start: (f32, f32),
    goal: (f32, f32),
    grid: &dyn Walkable,
    tile_size: f32,
    config: &AutoPathConfig,
) -> Option<GeneratedPath> {
    let start_tile = IVec2::new(
        (start.0 / tile_size) as i32,
        (start.1 / tile_size) as i32,
    );
    let goal_tile = IVec2::new(
        (goal.0 / tile_size) as i32,
        (goal.1 / tile_size) as i32,
    );

    let request = PathRequest {
        start: start_tile,
        goal: goal_tile,
        allow_diagonal: config.allow_diagonal,
        max_search: config.max_search,
    };

    let tile_path = pathfinding::find_path(&request, grid)?;

    // Convert to world coordinates (center of each tile)
    let half = tile_size * 0.5;
    let mut points: Vec<(f32, f32)> = tile_path
        .iter()
        .map(|p| (p.x as f32 * tile_size + half, p.y as f32 * tile_size + half))
        .collect();

    // Replace first/last with exact positions
    if let Some(first) = points.first_mut() {
        *first = start;
    }
    if let Some(last) = points.last_mut() {
        *last = goal;
    }

    if config.simplify {
        points = simplify_path(&points, config.min_point_distance);
    }

    let length = path_length(&points);

    Some(GeneratedPath {
        points,
        closed: false,
        length,
    })
}

/// Generate a patrol loop through a series of waypoints.
///
/// Finds a path between each consecutive pair and returns the combined path.
pub fn generate_patrol_loop(
    waypoints: &[(f32, f32)],
    grid: &dyn Walkable,
    tile_size: f32,
    config: &AutoPathConfig,
) -> Option<GeneratedPath> {
    if waypoints.len() < 2 {
        return None;
    }

    let mut all_points = Vec::new();

    for i in 0..waypoints.len() {
        let start = waypoints[i];
        let goal = waypoints[(i + 1) % waypoints.len()];

        let segment = generate_path(start, goal, grid, tile_size, config)?;

        // Skip the first point of subsequent segments (it's the same as the last of the previous)
        if i == 0 {
            all_points.extend_from_slice(&segment.points);
        } else {
            all_points.extend_from_slice(&segment.points[1..]);
        }
    }

    // Remove the duplicate closing point if it matches the start
    if all_points.len() > 1 && all_points.first() == all_points.last() {
        all_points.pop();
    }

    let length = path_length(&all_points);

    Some(GeneratedPath {
        points: all_points,
        closed: true,
        length,
    })
}

// ---------------------------------------------------------------------------
// Path utilities
// ---------------------------------------------------------------------------

/// Simplify a path by removing points that are collinear or too close together.
fn simplify_path(points: &[(f32, f32)], min_dist: f32) -> Vec<(f32, f32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut result = vec![points[0]];
    let min_dist_sq = min_dist * min_dist;

    for i in 1..points.len() - 1 {
        let prev = *result.last().unwrap();
        let curr = points[i];
        let next = points[i + 1];

        // Skip if too close to the previous kept point
        let dx = curr.0 - prev.0;
        let dy = curr.1 - prev.1;
        if dx * dx + dy * dy < min_dist_sq {
            continue;
        }

        // Skip if collinear (same direction)
        let d1x = curr.0 - prev.0;
        let d1y = curr.1 - prev.1;
        let d2x = next.0 - curr.0;
        let d2y = next.1 - curr.1;
        let cross = d1x * d2y - d1y * d2x;
        if cross.abs() < 0.01 {
            continue;
        }

        result.push(curr);
    }

    result.push(*points.last().unwrap());
    result
}

/// Calculate total path length.
fn path_length(points: &[(f32, f32)]) -> f32 {
    let mut len = 0.0;
    for i in 1..points.len() {
        let dx = points[i].0 - points[i - 1].0;
        let dy = points[i].1 - points[i - 1].1;
        len += (dx * dx + dy * dy).sqrt();
    }
    len
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct OpenGrid;
    impl Walkable for OpenGrid {
        fn is_walkable(&self, x: i32, y: i32) -> bool {
            x >= 0 && y >= 0 && x < 20 && y < 20
        }
    }

    struct WalledGrid;
    impl Walkable for WalledGrid {
        fn is_walkable(&self, x: i32, y: i32) -> bool {
            if x < 0 || y < 0 || x >= 10 || y >= 10 {
                return false;
            }
            // Wall at x=5, except y=8
            !(x == 5 && y != 8)
        }
    }

    #[test]
    fn basic_path() {
        let config = AutoPathConfig {
            simplify: false,
            ..Default::default()
        };
        let result = generate_path((8.0, 8.0), (152.0, 8.0), &OpenGrid, 16.0, &config);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.points.len() >= 2);
        assert!(!path.closed);
        assert!(path.length > 0.0);
    }

    #[test]
    fn path_around_wall() {
        let config = AutoPathConfig {
            simplify: false,
            ..Default::default()
        };
        let result = generate_path((16.0, 16.0), (112.0, 16.0), &WalledGrid, 16.0, &config);
        assert!(result.is_some());
        let path = result.unwrap();
        // Path must go around the wall
        assert!(path.points.len() > 3);
    }

    #[test]
    fn unreachable_goal() {
        struct BlockedGrid;
        impl Walkable for BlockedGrid {
            fn is_walkable(&self, x: i32, y: i32) -> bool {
                // Goal tile (9,0) is blocked
                !(x == 9 && y == 0)
                    && x >= 0 && y >= 0 && x < 10 && y < 10
            }
        }
        let config = AutoPathConfig::default();
        let result = generate_path((8.0, 8.0), (152.0, 8.0), &BlockedGrid, 16.0, &config);
        assert!(result.is_none());
    }

    #[test]
    fn simplification() {
        // Straight line should simplify to 2 points
        let points = vec![
            (0.0, 0.0),
            (16.0, 0.0),
            (32.0, 0.0),
            (48.0, 0.0),
            (64.0, 0.0),
        ];
        let simplified = simplify_path(&points, 1.0);
        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0], (0.0, 0.0));
        assert_eq!(simplified[1], (64.0, 0.0));
    }

    #[test]
    fn patrol_loop() {
        let config = AutoPathConfig {
            simplify: false,
            ..Default::default()
        };
        let waypoints = vec![(8.0, 8.0), (152.0, 8.0), (152.0, 152.0), (8.0, 152.0)];
        let result = generate_patrol_loop(&waypoints, &OpenGrid, 16.0, &config);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.closed);
        assert!(path.points.len() > 4);
    }

    #[test]
    fn tile_grid_walkability() {
        let tiles = vec![
            0, 0, 0, 0,
            0, 1, 1, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        let grid = TileGrid::new(&tiles, 4, 4);
        assert!(grid.is_walkable(0, 0));
        assert!(!grid.is_walkable(1, 1)); // tile=1 is solid
        assert!(!grid.is_walkable(-1, 0)); // out of bounds
    }

    #[test]
    fn tile_grid_custom_solid() {
        let tiles = vec![1, 2, 3, 4];
        let grid = TileGrid::new(&tiles, 2, 2).with_solid_ids(vec![3, 4]);
        assert!(grid.is_walkable(0, 0)); // tile=1, not in solid list
        assert!(grid.is_walkable(1, 0)); // tile=2, not in solid list
        assert!(!grid.is_walkable(0, 1)); // tile=3, solid
        assert!(!grid.is_walkable(1, 1)); // tile=4, solid
    }

    #[test]
    fn path_length_calculation() {
        let points = vec![(0.0, 0.0), (3.0, 0.0), (3.0, 4.0)];
        let len = path_length(&points);
        assert!((len - 7.0).abs() < 0.001);
    }
}
