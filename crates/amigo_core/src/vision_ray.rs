use crate::fog_of_war::{FogOfWarGrid, TileVisibility};
use crate::math::IVec2;
use crate::raycast::{TileBlock, TileQuery};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for vision raycasting.
#[derive(Clone, Debug)]
pub struct VisionConfig {
    /// Maximum vision radius in tiles.
    pub radius: u32,
    /// Tile size in world units (must match the tilemap).
    pub tile_size: f32,
}

impl VisionConfig {
    pub fn new(radius: u32, tile_size: f32) -> Self {
        Self { radius, tile_size }
    }
}

// ---------------------------------------------------------------------------
// Line-of-sight check
// ---------------------------------------------------------------------------

/// Check line-of-sight between two tile positions.
/// Returns true if no solid tile blocks the path (uses Bresenham's line).
pub fn has_line_of_sight(from: IVec2, to: IVec2, tiles: &dyn TileQuery) -> bool {
    if from == to {
        return true;
    }

    // Bresenham's line algorithm to walk tiles from `from` to `to`.
    let dx = (to.x - from.x).abs();
    let dy = (to.y - from.y).abs();
    let sx: i32 = if from.x < to.x { 1 } else { -1 };
    let sy: i32 = if from.y < to.y { 1 } else { -1 };
    let mut err = dx - dy;
    let mut x = from.x;
    let mut y = from.y;

    loop {
        // Check the tile (skip the starting tile).
        if (x != from.x || y != from.y)
            && (x != to.x || y != to.y)
            && matches!(tiles.tile_at(x, y), TileBlock::Solid)
        {
            return false;
        }

        if x == to.x && y == to.y {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Visibility update with wall occlusion
// ---------------------------------------------------------------------------

/// Update visibility for a single observer using line-of-sight raycasting.
///
/// Unlike `fog_of_war::update_visibility` (which sees through walls), this
/// function checks each candidate tile within Chebyshev distance for
/// unobstructed line-of-sight.
///
/// Steps:
/// 1. All `Visible` tiles are downgraded to `Explored`.
/// 2. For each tile within Chebyshev distance `radius`, a Bresenham ray is
///    cast from the observer. If no `Solid` tile blocks the path, the tile
///    is marked `Visible`.
pub fn update_visibility_raycast(
    observer_pos: IVec2,
    config: &VisionConfig,
    grid: &mut FogOfWarGrid,
    tiles: &dyn TileQuery,
) {
    let w = grid.width() as i32;
    let h = grid.height() as i32;
    let r = config.radius as i32;

    // Step 1: Downgrade Visible → Explored.
    for y in 0..grid.height() as i32 {
        for x in 0..grid.width() as i32 {
            if grid.visibility_at(x, y) == TileVisibility::Visible {
                grid.set_visibility(x, y, TileVisibility::Explored);
            }
        }
    }

    // Step 2: Cast rays to each candidate tile within radius.
    let min_x = (observer_pos.x - r).max(0);
    let max_x = (observer_pos.x + r).min(w - 1);
    let min_y = (observer_pos.y - r).max(0);
    let max_y = (observer_pos.y + r).min(h - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            // Chebyshev distance check.
            let dx = (x - observer_pos.x).abs();
            let dy = (y - observer_pos.y).abs();
            if dx.max(dy) > r {
                continue;
            }

            let target = IVec2 { x, y };
            if has_line_of_sight(observer_pos, target, tiles) {
                grid.set_visibility(x, y, TileVisibility::Visible);
            }
        }
    }
}

/// Update visibility for multiple observers.
/// More efficient than calling `update_visibility_raycast` in a loop because
/// the Visible→Explored pass only happens once.
pub fn update_visibility_multi(
    observers: &[(IVec2, VisionConfig)],
    grid: &mut FogOfWarGrid,
    tiles: &dyn TileQuery,
) {
    let w = grid.width() as i32;
    let h = grid.height() as i32;

    // Single Visible → Explored pass.
    for y in 0..h {
        for x in 0..w {
            if grid.visibility_at(x, y) == TileVisibility::Visible {
                grid.set_visibility(x, y, TileVisibility::Explored);
            }
        }
    }

    // Cast rays for each observer.
    for (observer_pos, config) in observers {
        let r = config.radius as i32;
        let min_x = (observer_pos.x - r).max(0);
        let max_x = (observer_pos.x + r).min(w - 1);
        let min_y = (observer_pos.y - r).max(0);
        let max_y = (observer_pos.y + r).min(h - 1);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = (x - observer_pos.x).abs();
                let dy = (y - observer_pos.y).abs();
                if dx.max(dy) > r {
                    continue;
                }

                if grid.visibility_at(x, y) == TileVisibility::Visible {
                    continue; // Already visible from another observer.
                }

                let target = IVec2 { x, y };
                if has_line_of_sight(*observer_pos, target, tiles) {
                    grid.set_visibility(x, y, TileVisibility::Visible);
                }
            }
        }
    }
}

/// Check if one position can see another within a given radius.
/// Combines Chebyshev distance check with LOS raycast.
pub fn can_see(observer: IVec2, target: IVec2, radius: u32, tiles: &dyn TileQuery) -> bool {
    let dx = (target.x - observer.x).abs();
    let dy = (target.y - observer.y).abs();
    if dx.max(dy) > radius as i32 {
        return false;
    }
    has_line_of_sight(observer, target, tiles)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fog_of_war::FogOfWarGrid;

    struct TestTiles {
        walls: Vec<(i32, i32)>,
        width: i32,
        height: i32,
    }

    impl TileQuery for TestTiles {
        fn tile_at(&self, x: i32, y: i32) -> TileBlock {
            if x < 0 || y < 0 || x >= self.width || y >= self.height {
                return TileBlock::Solid;
            }
            if self.walls.contains(&(x, y)) {
                TileBlock::Solid
            } else {
                TileBlock::Empty
            }
        }
    }

    #[test]
    fn los_clear_path() {
        let tiles = TestTiles {
            walls: vec![],
            width: 10,
            height: 10,
        };
        assert!(has_line_of_sight(
            IVec2 { x: 0, y: 0 },
            IVec2 { x: 5, y: 5 },
            &tiles
        ));
    }

    #[test]
    fn los_blocked_by_wall() {
        let tiles = TestTiles {
            walls: vec![(3, 3)],
            width: 10,
            height: 10,
        };
        assert!(!has_line_of_sight(
            IVec2 { x: 0, y: 0 },
            IVec2 { x: 5, y: 5 },
            &tiles
        ));
    }

    #[test]
    fn los_same_position() {
        let tiles = TestTiles {
            walls: vec![],
            width: 10,
            height: 10,
        };
        assert!(has_line_of_sight(
            IVec2 { x: 5, y: 5 },
            IVec2 { x: 5, y: 5 },
            &tiles
        ));
    }

    #[test]
    fn visibility_respects_walls() {
        let tiles = TestTiles {
            walls: vec![(3, 5)], // Wall between observer and far side
            width: 10,
            height: 10,
        };
        let mut grid = FogOfWarGrid::new(10, 10);
        let config = VisionConfig::new(5, 16.0);

        update_visibility_raycast(IVec2 { x: 5, y: 5 }, &config, &mut grid, &tiles);

        // Observer's tile is visible.
        assert_eq!(grid.visibility_at(5, 5), TileVisibility::Visible);
        // Nearby open tile is visible.
        assert_eq!(grid.visibility_at(6, 5), TileVisibility::Visible);
        // Tile behind wall should not be visible (depending on exact geometry).
        // The wall tile itself (3,5) blocks tiles further left.
        assert_eq!(grid.visibility_at(1, 5), TileVisibility::Hidden);
    }

    #[test]
    fn can_see_within_radius() {
        let tiles = TestTiles {
            walls: vec![],
            width: 10,
            height: 10,
        };
        assert!(can_see(
            IVec2 { x: 5, y: 5 },
            IVec2 { x: 7, y: 5 },
            3,
            &tiles
        ));
        // Out of radius.
        assert!(!can_see(
            IVec2 { x: 5, y: 5 },
            IVec2 { x: 9, y: 9 },
            3,
            &tiles
        ));
    }

    #[test]
    fn multi_observer_visibility() {
        let tiles = TestTiles {
            walls: vec![(5, 5)], // Wall in the middle
            width: 10,
            height: 10,
        };
        let mut grid = FogOfWarGrid::new(10, 10);
        let observers = vec![
            (IVec2 { x: 2, y: 5 }, VisionConfig::new(3, 16.0)),
            (IVec2 { x: 8, y: 5 }, VisionConfig::new(3, 16.0)),
        ];

        update_visibility_multi(&observers, &mut grid, &tiles);

        // Both observers can see their own area.
        assert_eq!(grid.visibility_at(2, 5), TileVisibility::Visible);
        assert_eq!(grid.visibility_at(8, 5), TileVisibility::Visible);
    }
}
