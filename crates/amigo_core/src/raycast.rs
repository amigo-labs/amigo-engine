//! Raycast API for tile grids and physics bodies.
//!
//! Provides DDA-based tile raycasting and body raycasting via SpatialHash.
//! Used by platformer controllers (ground/wall detection), shmup line-of-sight,
//! and RTS vision queries.

use crate::collision::{CollisionShape, CollisionWorld};
use crate::ecs::EntityId;
use crate::math::RenderVec2;

// ---------------------------------------------------------------------------
// TileQuery trait (for tilemap raycasts without depending on amigo_tilemap)
// ---------------------------------------------------------------------------

/// Whether a tile blocks raycasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileBlock {
    /// Tile does not block raycasts.
    Empty,
    /// Tile blocks raycasts from all directions.
    Solid,
    /// Tile blocks only downward raycasts (one-way platform).
    OneWay,
    /// Slope tile — blocks based on interpolated height.
    Slope { left_height: u8, right_height: u8 },
}

/// Trait for querying tile solidity. Implemented by CollisionLayer in amigo_tilemap.
pub trait TileQuery {
    /// Return the blocking type of the tile at grid position (x, y).
    /// Out-of-bounds should return `TileBlock::Solid`.
    fn tile_at(&self, x: i32, y: i32) -> TileBlock;
}

// ---------------------------------------------------------------------------
// RayHit
// ---------------------------------------------------------------------------

/// Result of a raycast hit.
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    /// World position where the ray hit.
    pub point: RenderVec2,
    /// Surface normal at the hit point.
    pub normal: RenderVec2,
    /// Distance from ray origin to hit point.
    pub distance: f32,
    /// Entity that was hit (None for tilemap hits).
    pub entity: Option<EntityId>,
    /// Tile type that was hit (only for tile raycasts).
    pub tile_block: Option<TileBlock>,
}

// ---------------------------------------------------------------------------
// Tile raycast (DDA algorithm)
// ---------------------------------------------------------------------------

/// Cast a ray against a tile grid using DDA (Digital Differential Analyzer).
/// `tile_size` is the size of each tile in world units.
/// Returns the first blocking tile hit.
pub fn raycast_tiles(
    origin: RenderVec2,
    direction: RenderVec2,
    max_distance: f32,
    tiles: &dyn TileQuery,
    tile_size: f32,
) -> Option<RayHit> {
    let inv_tile = 1.0 / tile_size;

    // Normalize direction
    let len = (direction.x * direction.x + direction.y * direction.y).sqrt();
    if len < 1e-8 {
        return None;
    }
    let dx = direction.x / len;
    let dy = direction.y / len;

    // Current tile coordinates
    let mut tile_x = (origin.x * inv_tile).floor() as i32;
    let mut tile_y = (origin.y * inv_tile).floor() as i32;

    // Step direction
    let step_x: i32 = if dx > 0.0 { 1 } else { -1 };
    let step_y: i32 = if dy > 0.0 { 1 } else { -1 };

    // Distance to next tile boundary along each axis
    let t_delta_x = if dx.abs() < 1e-8 {
        f32::MAX
    } else {
        (tile_size / dx.abs())
    };
    let t_delta_y = if dy.abs() < 1e-8 {
        f32::MAX
    } else {
        (tile_size / dy.abs())
    };

    // Initial t to first boundary
    let t_max_x = if dx.abs() < 1e-8 {
        f32::MAX
    } else {
        let border = if dx > 0.0 {
            (tile_x as f32 + 1.0) * tile_size
        } else {
            tile_x as f32 * tile_size
        };
        (border - origin.x) / dx
    };
    let t_max_y = if dy.abs() < 1e-8 {
        f32::MAX
    } else {
        let border = if dy > 0.0 {
            (tile_y as f32 + 1.0) * tile_size
        } else {
            tile_y as f32 * tile_size
        };
        (border - origin.y) / dy
    };

    let mut t_max_x = t_max_x;
    let mut t_max_y = t_max_y;
    let mut distance = 0.0_f32;

    // DDA loop
    while distance <= max_distance {
        let block = tiles.tile_at(tile_x, tile_y);
        if block == TileBlock::Solid {
            let hit_point = RenderVec2::new(origin.x + dx * distance, origin.y + dy * distance);
            // Normal points back toward the ray origin
            let normal = if t_max_x < t_max_y {
                RenderVec2::new(-step_x as f32, 0.0)
            } else {
                RenderVec2::new(0.0, -step_y as f32)
            };
            return Some(RayHit {
                point: hit_point,
                normal,
                distance,
                entity: None,
                tile_block: Some(block),
            });
        }
        if block == TileBlock::OneWay && dy > 0.0 {
            // OneWay blocks only downward rays
            let hit_point = RenderVec2::new(origin.x + dx * distance, origin.y + dy * distance);
            return Some(RayHit {
                point: hit_point,
                normal: RenderVec2::new(0.0, -1.0),
                distance,
                entity: None,
                tile_block: Some(block),
            });
        }
        if let TileBlock::Slope {
            left_height,
            right_height,
        } = block
        {
            // Interpolate slope height at the ray's x position within the tile
            let local_x = (origin.x + dx * distance) - tile_x as f32 * tile_size;
            let frac = (local_x / tile_size).clamp(0.0, 1.0);
            let slope_height = left_height as f32 + (right_height as f32 - left_height as f32) * frac;
            let tile_top_y = tile_y as f32 * tile_size;
            let surface_y = tile_top_y + tile_size - slope_height;
            let ray_y = origin.y + dy * distance;
            if ray_y >= surface_y {
                return Some(RayHit {
                    point: RenderVec2::new(origin.x + dx * distance, surface_y),
                    normal: RenderVec2::new(0.0, -1.0), // Simplified upward normal
                    distance,
                    entity: None,
                    tile_block: Some(block),
                });
            }
        }

        // Advance to next tile
        if t_max_x < t_max_y {
            distance = t_max_x;
            t_max_x += t_delta_x;
            tile_x += step_x;
        } else {
            distance = t_max_y;
            t_max_y += t_delta_y;
            tile_y += step_y;
        }
    }

    None
}

/// Cast a ray against all bodies in the CollisionWorld.
/// Returns the closest hit. Uses SpatialHash for broad-phase.
pub fn raycast_bodies(
    origin: RenderVec2,
    direction: RenderVec2,
    max_distance: f32,
    world: &CollisionWorld,
    exclude: Option<EntityId>,
) -> Option<RayHit> {
    // Normalize direction
    let len = (direction.x * direction.x + direction.y * direction.y).sqrt();
    if len < 1e-8 {
        return None;
    }
    let dx = direction.x / len;
    let dy = direction.y / len;

    // Build a bounding rect along the ray for broad-phase query
    let end = RenderVec2::new(origin.x + dx * max_distance, origin.y + dy * max_distance);
    let min_x = origin.x.min(end.x) - 1.0;
    let min_y = origin.y.min(end.y) - 1.0;
    let max_x = origin.x.max(end.x) + 1.0;
    let max_y = origin.y.max(end.y) + 1.0;
    let query_rect = crate::rect::Rect::new(min_x, min_y, max_x - min_x, max_y - min_y);

    let candidates = world.query_aabb(&query_rect);
    let mut closest: Option<RayHit> = None;

    for entity in candidates {
        if exclude == Some(entity) {
            continue;
        }
        // Test ray against each candidate's shape
        if let Some(hit) = ray_vs_entity(origin, dx, dy, max_distance, entity, world) {
            if closest.as_ref().map_or(true, |c| hit.distance < c.distance) {
                closest = Some(hit);
            }
        }
    }

    closest
}

fn ray_vs_entity(
    origin: RenderVec2,
    dx: f32,
    dy: f32,
    max_distance: f32,
    entity: EntityId,
    world: &CollisionWorld,
) -> Option<RayHit> {
    let (pos, shape) = world.get_shape(entity)?;

    match shape {
        CollisionShape::Circle { cx, cy, radius } => {
            let center_x = pos.x + cx;
            let center_y = pos.y + cy;
            ray_vs_circle(origin, dx, dy, max_distance, center_x, center_y, *radius, entity)
        }
        CollisionShape::Aabb(rect) => {
            let aabb = crate::rect::Rect::new(pos.x + rect.x, pos.y + rect.y, rect.w, rect.h);
            ray_vs_aabb(origin, dx, dy, max_distance, &aabb, entity)
        }
    }
}

fn ray_vs_circle(
    origin: RenderVec2,
    dx: f32,
    dy: f32,
    max_distance: f32,
    cx: f32,
    cy: f32,
    radius: f32,
    entity: EntityId,
) -> Option<RayHit> {
    let ocx = origin.x - cx;
    let ocy = origin.y - cy;
    let a = dx * dx + dy * dy; // = 1.0 for normalized direction
    let b = 2.0 * (ocx * dx + ocy * dy);
    let c = ocx * ocx + ocy * ocy - radius * radius;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_d = discriminant.sqrt();
    let t = (-b - sqrt_d) / (2.0 * a);
    if t < 0.0 || t > max_distance {
        return None;
    }
    let hit_x = origin.x + dx * t;
    let hit_y = origin.y + dy * t;
    let nx = (hit_x - cx) / radius;
    let ny = (hit_y - cy) / radius;
    Some(RayHit {
        point: RenderVec2::new(hit_x, hit_y),
        normal: RenderVec2::new(nx, ny),
        distance: t,
        entity: Some(entity),
        tile_block: None,
    })
}

fn ray_vs_aabb(
    origin: RenderVec2,
    dx: f32,
    dy: f32,
    max_distance: f32,
    aabb: &crate::rect::Rect,
    entity: EntityId,
) -> Option<RayHit> {
    let inv_dx = if dx.abs() < 1e-8 {
        f32::MAX.copysign(dx)
    } else {
        1.0 / dx
    };
    let inv_dy = if dy.abs() < 1e-8 {
        f32::MAX.copysign(dy)
    } else {
        1.0 / dy
    };

    let tx1 = (aabb.x - origin.x) * inv_dx;
    let tx2 = (aabb.x + aabb.w - origin.x) * inv_dx;
    let ty1 = (aabb.y - origin.y) * inv_dy;
    let ty2 = (aabb.y + aabb.h - origin.y) * inv_dy;

    let t_min = tx1.min(tx2).max(ty1.min(ty2));
    let t_max = tx1.max(tx2).min(ty1.max(ty2));

    if t_max < 0.0 || t_min > t_max || t_min > max_distance {
        return None;
    }

    let t = if t_min >= 0.0 { t_min } else { t_max };
    if t > max_distance {
        return None;
    }

    let hit_x = origin.x + dx * t;
    let hit_y = origin.y + dy * t;

    // Determine hit normal
    let normal = if (t - tx1.min(tx2)).abs() < 0.001 {
        RenderVec2::new(-dx.signum(), 0.0)
    } else {
        RenderVec2::new(0.0, -dy.signum())
    };

    Some(RayHit {
        point: RenderVec2::new(hit_x, hit_y),
        normal,
        distance: t,
        entity: Some(entity),
        tile_block: None,
    })
}

/// Cast a ray against both tiles and bodies, returning the closest overall hit.
pub fn raycast(
    origin: RenderVec2,
    direction: RenderVec2,
    max_distance: f32,
    tiles: &dyn TileQuery,
    tile_size: f32,
    world: &CollisionWorld,
    exclude: Option<EntityId>,
) -> Option<RayHit> {
    let tile_hit = raycast_tiles(origin, direction, max_distance, tiles, tile_size);
    let body_hit = raycast_bodies(origin, direction, max_distance, world, exclude);
    match (tile_hit, body_hit) {
        (Some(t), Some(b)) => {
            if t.distance <= b.distance {
                Some(t)
            } else {
                Some(b)
            }
        }
        (Some(t), None) => Some(t),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Short-range directional sensor (convenience for platformer controllers).
/// Returns `true` if any solid tile is within `distance` along `direction`.
pub fn sensor(
    origin: RenderVec2,
    direction: RenderVec2,
    distance: f32,
    tiles: &dyn TileQuery,
    tile_size: f32,
) -> bool {
    raycast_tiles(origin, direction, distance, tiles, tile_size).is_some()
}

// Note: CollisionWorld::get_shape() is defined in collision.rs to access private fields.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct TestGrid {
        width: i32,
        height: i32,
        solid: Vec<(i32, i32)>,
    }

    impl TileQuery for TestGrid {
        fn tile_at(&self, x: i32, y: i32) -> TileBlock {
            if x < 0 || y < 0 || x >= self.width || y >= self.height {
                return TileBlock::Solid;
            }
            if self.solid.contains(&(x, y)) {
                TileBlock::Solid
            } else {
                TileBlock::Empty
            }
        }
    }

    #[test]
    fn raycast_hits_solid_tile() {
        let grid = TestGrid {
            width: 10,
            height: 10,
            solid: vec![(5, 0)],
        };
        let hit = raycast_tiles(
            RenderVec2::new(8.0, 8.0), // In tile (0,0) with tile_size=16
            RenderVec2::new(1.0, 0.0), // Cast right
            200.0,
            &grid,
            16.0,
        );
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert!(hit.distance > 0.0);
        assert!(hit.tile_block == Some(TileBlock::Solid));
    }

    #[test]
    fn raycast_hits_boundary() {
        let grid = TestGrid {
            width: 10,
            height: 10,
            solid: vec![],
        };
        let hit = raycast_tiles(
            RenderVec2::new(8.0, 8.0),
            RenderVec2::new(1.0, 0.0),
            200.0,
            &grid,
            16.0,
        );
        // Should hit the out-of-bounds boundary (treated as Solid)
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.tile_block, Some(TileBlock::Solid));
    }

    #[test]
    fn raycast_respects_max_distance() {
        let grid = TestGrid {
            width: 100,
            height: 100,
            solid: vec![(50, 0)], // Far away solid
        };
        let hit = raycast_tiles(
            RenderVec2::new(8.0, 8.0),
            RenderVec2::new(1.0, 0.0),
            10.0, // Very short range
            &grid,
            16.0,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn sensor_detects_ground() {
        let grid = TestGrid {
            width: 10,
            height: 10,
            solid: vec![(2, 3)],
        };
        // Standing above solid tile (2,3), cast down
        let has_ground = sensor(
            RenderVec2::new(2.0 * 16.0 + 8.0, 2.0 * 16.0 + 15.0), // Bottom of tile (2,2)
            RenderVec2::new(0.0, 1.0),                               // Down
            2.0,                                                       // 2px range
            &grid,
            16.0,
        );
        assert!(has_ground);
    }

    #[test]
    fn sensor_no_ground() {
        let grid = TestGrid {
            width: 10,
            height: 10,
            solid: vec![],
        };
        let has_ground = sensor(
            RenderVec2::new(32.0, 32.0),
            RenderVec2::new(0.0, 1.0),
            2.0,
            &grid,
            16.0,
        );
        assert!(!has_ground);
    }

    #[test]
    fn ray_vs_circle_hit() {
        let hit = ray_vs_circle(
            RenderVec2::new(0.0, 0.0),
            1.0, 0.0,   // Right
            100.0,
            50.0, 0.0,  // Circle center
            10.0,        // Radius
            EntityId::from_raw(1, 0),
        );
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert!((hit.distance - 40.0).abs() < 0.01); // 50 - 10 = 40
    }

    #[test]
    fn ray_vs_circle_miss() {
        let hit = ray_vs_circle(
            RenderVec2::new(0.0, 0.0),
            1.0, 0.0,    // Right
            100.0,
            50.0, 50.0,  // Far off to the side
            5.0,
            EntityId::from_raw(1, 0),
        );
        assert!(hit.is_none());
    }

    #[test]
    fn ray_vs_aabb_hit() {
        let aabb = crate::rect::Rect::new(40.0, -10.0, 20.0, 20.0);
        let hit = ray_vs_aabb(
            RenderVec2::new(0.0, 0.0),
            1.0, 0.0,
            100.0,
            &aabb,
            EntityId::from_raw(1, 0),
        );
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert!((hit.distance - 40.0).abs() < 0.01);
    }
}
