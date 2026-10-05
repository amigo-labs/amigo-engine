//! Raycast API for tile grids and physics bodies.
//!
//! Provides DDA-based tile raycasting and body raycasting via SpatialHash.
//! Used by platformer controllers (ground/wall detection), shmup line-of-sight,
//! and RTS vision queries.

use crate::collision::{CollisionShape, CollisionWorld};
use crate::ecs::EntityId;
use crate::math::{Fix, SimVec2};
use crate::rect::SimRect;

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

/// Result of a raycast hit, in simulation space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayHit {
    /// World position where the ray hit.
    pub point: SimVec2,
    /// Surface normal at the hit point.
    pub normal: SimVec2,
    /// Distance from ray origin to hit point.
    pub distance: Fix,
    /// Entity that was hit (None for tilemap hits).
    pub entity: Option<EntityId>,
    /// Tile type that was hit (only for tile raycasts).
    pub tile_block: Option<TileBlock>,
}

/// `num / den`, saturating, with a zero denominator meaning "never".
fn ratio(num: Fix, den: Fix) -> Fix {
    if den == Fix::ZERO {
        Fix::MAX
    } else {
        num.saturating_div(den)
    }
}

// ---------------------------------------------------------------------------
// Tile raycast (DDA algorithm)
// ---------------------------------------------------------------------------

/// Cast a ray against a tile grid using DDA (Digital Differential Analyzer).
/// `tile_size` is the size of each tile in world units.
/// Returns the first blocking tile hit.
pub fn raycast_tiles(
    origin: SimVec2,
    direction: SimVec2,
    max_distance: Fix,
    tiles: &dyn TileQuery,
    tile_size: Fix,
) -> Option<RayHit> {
    if tile_size <= Fix::ZERO {
        return None;
    }
    let dir = direction.normalize();
    if dir == SimVec2::ZERO {
        return None;
    }
    let (dx, dy) = (dir.x, dir.y);
    let tile_of = |v: Fix| v.saturating_div(tile_size).floor().to_num::<i32>();

    // Current tile coordinates
    let mut tile_x = tile_of(origin.x);
    let mut tile_y = tile_of(origin.y);

    // Step direction
    let step_x: i32 = if dx > Fix::ZERO { 1 } else { -1 };
    let step_y: i32 = if dy > Fix::ZERO { 1 } else { -1 };

    // Distance along the ray between tile boundaries on each axis
    let t_delta_x = ratio(tile_size, dx.abs());
    let t_delta_y = ratio(tile_size, dy.abs());

    // Distance to the first boundary on each axis
    let first_border = |tile: i32, d: Fix| {
        let edge = if d > Fix::ZERO { tile + 1 } else { tile };
        Fix::saturating_from_num(edge).saturating_mul(tile_size)
    };
    let mut t_max_x = if dx == Fix::ZERO {
        Fix::MAX
    } else {
        ratio(first_border(tile_x, dx) - origin.x, dx)
    };
    let mut t_max_y = if dy == Fix::ZERO {
        Fix::MAX
    } else {
        ratio(first_border(tile_y, dy) - origin.y, dy)
    };
    let mut distance = Fix::ZERO;
    let at = |t: Fix| origin + dir * t;

    // DDA loop
    while distance <= max_distance {
        let block = tiles.tile_at(tile_x, tile_y);
        if block == TileBlock::Solid {
            // Normal points back toward the ray origin
            let normal = if t_max_x < t_max_y {
                SimVec2::from_num(-step_x, 0)
            } else {
                SimVec2::from_num(0, -step_y)
            };
            return Some(RayHit {
                point: at(distance),
                normal,
                distance,
                entity: None,
                tile_block: Some(block),
            });
        }
        if block == TileBlock::OneWay && dy > Fix::ZERO {
            // OneWay blocks only downward rays
            return Some(RayHit {
                point: at(distance),
                normal: SimVec2::from_num(0, -1),
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
            let tile_left = Fix::saturating_from_num(tile_x).saturating_mul(tile_size);
            let local_x = at(distance).x - tile_left;
            let frac = local_x.saturating_div(tile_size).clamp(Fix::ZERO, Fix::ONE);
            let (left, right) = (Fix::from_num(left_height), Fix::from_num(right_height));
            let slope_height = left + (right - left) * frac;
            let tile_top_y = Fix::saturating_from_num(tile_y).saturating_mul(tile_size);
            let surface_y = tile_top_y + tile_size - slope_height;
            if at(distance).y >= surface_y {
                return Some(RayHit {
                    point: SimVec2::new(at(distance).x, surface_y),
                    normal: SimVec2::from_num(0, -1), // Simplified upward normal
                    distance,
                    entity: None,
                    tile_block: Some(block),
                });
            }
        }

        // Advance to next tile
        if t_max_x < t_max_y {
            distance = t_max_x;
            t_max_x = t_max_x.saturating_add(t_delta_x);
            tile_x += step_x;
        } else {
            distance = t_max_y;
            t_max_y = t_max_y.saturating_add(t_delta_y);
            tile_y += step_y;
        }
        if distance == Fix::MAX {
            break;
        }
    }

    None
}

/// Cast a ray against all bodies in the CollisionWorld.
/// Returns the closest hit (the smaller entity id on a tie). Uses SpatialHash
/// for broad-phase.
pub fn raycast_bodies(
    origin: SimVec2,
    direction: SimVec2,
    max_distance: Fix,
    world: &CollisionWorld,
    exclude: Option<EntityId>,
) -> Option<RayHit> {
    let dir = direction.normalize();
    if dir == SimVec2::ZERO {
        return None;
    }

    // Build a bounding rect along the ray for broad-phase query
    let end = origin + dir * max_distance;
    let one = Fix::ONE;
    let min_x = origin.x.min(end.x) - one;
    let min_y = origin.y.min(end.y) - one;
    let max_x = origin.x.max(end.x) + one;
    let max_y = origin.y.max(end.y) + one;
    let query_rect = SimRect::new(min_x, min_y, max_x - min_x, max_y - min_y);

    let mut closest: Option<RayHit> = None;
    // Candidates come back sorted, so ties go to the smaller id everywhere.
    for entity in world.query_aabb(&query_rect) {
        if exclude == Some(entity) {
            continue;
        }
        if let Some(hit) = ray_vs_entity(origin, dir, max_distance, entity, world)
            && closest.as_ref().is_none_or(|c| hit.distance < c.distance)
        {
            closest = Some(hit);
        }
    }

    closest
}

fn ray_vs_entity(
    origin: SimVec2,
    dir: SimVec2,
    max_distance: Fix,
    entity: EntityId,
    world: &CollisionWorld,
) -> Option<RayHit> {
    let (pos, shape) = world.get_shape(entity)?;
    match shape {
        CollisionShape::Circle { center, radius } => {
            ray_vs_circle(origin, dir, max_distance, pos + *center, *radius, entity)
        }
        CollisionShape::Aabb(rect) => {
            ray_vs_aabb(origin, dir, max_distance, &rect.translated(pos), entity)
        }
    }
}

/// Ray (unit direction) against a circle. The quadratic is solved on raw
/// Q16.16 bits in `i128`: its squared terms overflow Q16.16 for anything
/// more than ~181 units away.
fn ray_vs_circle(
    origin: SimVec2,
    dir: SimVec2,
    max_distance: Fix,
    center: SimVec2,
    radius: Fix,
    entity: EntityId,
) -> Option<RayHit> {
    let raw = |v: Fix| i128::from(v.to_bits());
    let oc = origin - center;
    // With |dir| = 1: t = -b ± sqrt(b² - c), b = oc·dir, c = |oc|² - r².
    let b = (raw(oc.x) * raw(dir.x) + raw(oc.y) * raw(dir.y)) >> 16;
    let c = (raw(oc.x) * raw(oc.x) + raw(oc.y) * raw(oc.y) - raw(radius) * raw(radius)) >> 16;
    let disc = ((b * b) >> 16) - c;
    if disc < 0 {
        return None;
    }
    let sqrt_d = ((disc as u128) << 16).isqrt() as i128;
    let t = -b - sqrt_d;
    if t < 0 || t > raw(max_distance) {
        return None;
    }
    let t = Fix::from_bits(t as i32);
    let point = origin + dir * t;
    let normal = if radius > Fix::ZERO {
        (point - center) / radius
    } else {
        -dir
    };
    Some(RayHit {
        point,
        normal,
        distance: t,
        entity: Some(entity),
        tile_block: None,
    })
}

fn ray_vs_aabb(
    origin: SimVec2,
    dir: SimVec2,
    max_distance: Fix,
    aabb: &SimRect,
    entity: EntityId,
) -> Option<RayHit> {
    // Slab test per axis; an axis the ray does not move along either
    // contains the origin (no limit) or misses outright.
    let slab = |o: Fix, d: Fix, lo: Fix, hi: Fix| -> Option<(Fix, Fix)> {
        if d == Fix::ZERO {
            return (o >= lo && o <= hi).then_some((Fix::MIN, Fix::MAX));
        }
        let (t1, t2) = ((lo - o).saturating_div(d), (hi - o).saturating_div(d));
        Some((t1.min(t2), t1.max(t2)))
    };
    let (tx_near, tx_far) = slab(origin.x, dir.x, aabb.x, aabb.right())?;
    let (ty_near, ty_far) = slab(origin.y, dir.y, aabb.y, aabb.bottom())?;

    let t_min = tx_near.max(ty_near);
    let t_max = tx_far.min(ty_far);

    if t_max < Fix::ZERO || t_min > t_max || t_min > max_distance {
        return None;
    }

    let t = if t_min >= Fix::ZERO { t_min } else { t_max };
    if t > max_distance {
        return None;
    }

    // The face hit is on the axis whose slab was entered last.
    let normal = if tx_near >= ty_near {
        SimVec2::new(-dir.x.signum(), Fix::ZERO)
    } else {
        SimVec2::new(Fix::ZERO, -dir.y.signum())
    };

    Some(RayHit {
        point: origin + dir * t,
        normal,
        distance: t,
        entity: Some(entity),
        tile_block: None,
    })
}

/// Cast a ray against both tiles and bodies, returning the closest overall hit.
pub fn raycast(
    origin: SimVec2,
    direction: SimVec2,
    max_distance: Fix,
    tiles: &dyn TileQuery,
    tile_size: Fix,
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
    origin: SimVec2,
    direction: SimVec2,
    distance: Fix,
    tiles: &dyn TileQuery,
    tile_size: Fix,
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

    fn f(v: impl fixed::traits::ToFixed) -> Fix {
        Fix::from_num(v)
    }
    fn v(x: impl fixed::traits::ToFixed, y: impl fixed::traits::ToFixed) -> SimVec2 {
        SimVec2::from_num(x, y)
    }

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
            v(8.0, 8.0), // In tile (0,0) with tile_size=16
            v(1.0, 0.0), // Cast right
            f(200),
            &grid,
            f(16),
        );
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.distance, f(72), "edge of tile 5 is 80, origin at 8");
        assert!(hit.tile_block == Some(TileBlock::Solid));
    }

    #[test]
    fn raycast_hits_boundary() {
        let grid = TestGrid {
            width: 10,
            height: 10,
            solid: vec![],
        };
        let hit = raycast_tiles(v(8.0, 8.0), v(1.0, 0.0), f(200), &grid, f(16));
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
            v(8.0, 8.0),
            v(1.0, 0.0),
            f(10), // Very short range
            &grid,
            f(16),
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
            v(40, 47),   // Bottom of tile (2,2)
            v(0.0, 1.0), // Down
            f(2),        // 2px range
            &grid,
            f(16),
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
        let has_ground = sensor(v(32.0, 32.0), v(0.0, 1.0), f(2), &grid, f(16));
        assert!(!has_ground);
    }

    #[test]
    fn ray_vs_circle_hit() {
        let hit = ray_vs_circle(
            v(0, 0),
            v(1, 0), // Right
            f(100),
            v(50, 0), // Circle center
            f(10),    // Radius
            EntityId::from_raw(1, 0),
        );
        let hit = hit.expect("hit");
        assert_eq!(hit.distance, f(40)); // 50 - 10
        assert_eq!(hit.normal, v(-1, 0));
    }

    #[test]
    fn ray_vs_circle_miss() {
        let hit = ray_vs_circle(
            v(0, 0),
            v(1, 0), // Right
            f(100),
            v(50, 50), // Far off to the side
            f(5),
            EntityId::from_raw(1, 0),
        );
        assert!(hit.is_none());
    }

    #[test]
    fn ray_vs_aabb_hit() {
        let aabb = SimRect::from_num(40, -10, 20, 20);
        let hit = ray_vs_aabb(v(0, 0), v(1, 0), f(100), &aabb, EntityId::from_raw(1, 0));
        let hit = hit.expect("hit");
        assert_eq!(hit.distance, f(40));
        assert_eq!(hit.normal, v(-1, 0));
        // A ray parallel to a slab outside it misses.
        assert!(ray_vs_aabb(v(0, 50), v(1, 0), f(100), &aabb, EntityId::from_raw(1, 0)).is_none());
    }

    /// Far from the origin, the circle quadratic used to overflow Q16.16.
    #[test]
    fn far_circle_hits_without_overflow() {
        let hit = ray_vs_circle(
            v(-15000, 0),
            v(1, 0),
            f(30000),
            v(10000, 3),
            f(5),
            EntityId::from_raw(1, 0),
        )
        .expect("hit");
        assert!(
            (hit.distance - f(24996)).abs() < f(0.01),
            "{}",
            hit.distance
        );
    }

    #[test]
    fn body_raycast_picks_the_nearest_body() {
        let mut world = CollisionWorld::new(f(32));
        let shape = CollisionShape::Circle {
            center: SimVec2::ZERO,
            radius: f(4),
        };
        world.update_entity(EntityId::from_raw(2, 0), v(80, 0), shape);
        world.update_entity(EntityId::from_raw(1, 0), v(40, 0), shape);
        let hit = raycast_bodies(v(0, 0), v(1, 0), f(200), &world, None).expect("hit");
        assert_eq!(hit.entity, Some(EntityId::from_raw(1, 0)));
        assert_eq!(hit.distance, f(36));
        let hit = raycast_bodies(
            v(0, 0),
            v(1, 0),
            f(200),
            &world,
            Some(EntityId::from_raw(1, 0)),
        )
        .expect("hit");
        assert_eq!(hit.entity, Some(EntityId::from_raw(2, 0)));
    }
}
