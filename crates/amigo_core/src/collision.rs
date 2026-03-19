use crate::ecs::EntityId;
use crate::math::RenderVec2;
use crate::rect::Rect;
use rustc_hash::{FxHashMap, FxHashSet};

/// Collision shape for an entity.
#[derive(Clone, Copy, Debug)]
pub enum CollisionShape {
    Aabb(Rect),
    Circle { cx: f32, cy: f32, radius: f32 },
}

/// Contact information from a collision check.
#[derive(Clone, Copy, Debug)]
pub struct ContactInfo {
    pub penetration: f32,
    pub normal: RenderVec2,
}

pub fn aabb_vs_aabb(a: &Rect, b: &Rect) -> Option<ContactInfo> {
    let overlap_x = (a.w + b.w) * 0.5 - (a.center_x() - b.center_x()).abs();
    let overlap_y = (a.h + b.h) * 0.5 - (a.center_y() - b.center_y()).abs();
    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }
    if overlap_x < overlap_y {
        let sign = if a.center_x() < b.center_x() {
            -1.0
        } else {
            1.0
        };
        Some(ContactInfo {
            penetration: overlap_x,
            normal: RenderVec2::new(sign, 0.0),
        })
    } else {
        let sign = if a.center_y() < b.center_y() {
            -1.0
        } else {
            1.0
        };
        Some(ContactInfo {
            penetration: overlap_y,
            normal: RenderVec2::new(0.0, sign),
        })
    }
}

pub fn circle_vs_circle(
    ax: f32,
    ay: f32,
    ar: f32,
    bx: f32,
    by: f32,
    br: f32,
) -> Option<ContactInfo> {
    let dx = bx - ax;
    let dy = by - ay;
    let dist_sq = dx * dx + dy * dy;
    let sum_r = ar + br;
    if dist_sq >= sum_r * sum_r {
        return None;
    }
    let dist = dist_sq.sqrt();
    if dist < 0.0001 {
        return Some(ContactInfo {
            penetration: sum_r,
            normal: RenderVec2::new(1.0, 0.0),
        });
    }
    Some(ContactInfo {
        penetration: sum_r - dist,
        normal: RenderVec2::new(dx / dist, dy / dist),
    })
}

pub fn circle_vs_aabb(cx: f32, cy: f32, radius: f32, rect: &Rect) -> Option<ContactInfo> {
    let closest_x = cx.clamp(rect.x, rect.x + rect.w);
    let closest_y = cy.clamp(rect.y, rect.y + rect.h);
    let dx = cx - closest_x;
    let dy = cy - closest_y;
    let dist_sq = dx * dx + dy * dy;
    if dist_sq >= radius * radius {
        return None;
    }
    let dist = dist_sq.sqrt();
    if dist < 0.0001 {
        return Some(ContactInfo {
            penetration: radius,
            normal: RenderVec2::new(0.0, -1.0),
        });
    }
    Some(ContactInfo {
        penetration: radius - dist,
        normal: RenderVec2::new(dx / dist, dy / dist),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CellKey(i32, i32);

/// Spatial hash grid for broad-phase collision detection.
pub struct SpatialHash {
    #[allow(dead_code)]
    cell_size: f32,
    inv_cell_size: f32,
    cells: FxHashMap<CellKey, Vec<EntityId>>,
    entity_cells: FxHashMap<EntityId, Vec<CellKey>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        assert!(cell_size > 0.0);
        Self {
            cell_size,
            inv_cell_size: 1.0 / cell_size,
            cells: FxHashMap::default(),
            entity_cells: FxHashMap::default(),
        }
    }

    fn cell_key(&self, x: f32, y: f32) -> CellKey {
        CellKey(
            (x * self.inv_cell_size).floor() as i32,
            (y * self.inv_cell_size).floor() as i32,
        )
    }

    pub fn insert(&mut self, id: EntityId, aabb: &Rect) {
        self.remove(id);
        let min_key = self.cell_key(aabb.x, aabb.y);
        let max_key = self.cell_key(aabb.x + aabb.w, aabb.y + aabb.h);
        let mut keys = Vec::new();
        for cy in min_key.1..=max_key.1 {
            for cx in min_key.0..=max_key.0 {
                let key = CellKey(cx, cy);
                self.cells.entry(key).or_default().push(id);
                keys.push(key);
            }
        }
        self.entity_cells.insert(id, keys);
    }

    pub fn remove(&mut self, id: EntityId) {
        if let Some(keys) = self.entity_cells.remove(&id) {
            for key in &keys {
                if let Some(cell) = self.cells.get_mut(key) {
                    cell.retain(|&e| e != id);
                    if cell.is_empty() {
                        self.cells.remove(key);
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
    }

    pub fn query_aabb(&self, aabb: &Rect) -> Vec<EntityId> {
        let min_key = self.cell_key(aabb.x, aabb.y);
        let max_key = self.cell_key(aabb.x + aabb.w, aabb.y + aabb.h);
        let mut result = FxHashSet::default();
        for cy in min_key.1..=max_key.1 {
            for cx in min_key.0..=max_key.0 {
                if let Some(cell) = self.cells.get(&CellKey(cx, cy)) {
                    for &id in cell {
                        result.insert(id);
                    }
                }
            }
        }
        result.into_iter().collect()
    }

    pub fn query_point(&self, x: f32, y: f32) -> Vec<EntityId> {
        let key = self.cell_key(x, y);
        self.cells.get(&key).cloned().unwrap_or_default()
    }

    pub fn query_circle(&self, cx: f32, cy: f32, radius: f32) -> Vec<EntityId> {
        self.query_aabb(&Rect::new(
            cx - radius,
            cy - radius,
            radius * 2.0,
            radius * 2.0,
        ))
    }
}

/// Trigger zone that fires events when entities enter/exit.
#[derive(Clone, Debug)]
pub struct TriggerZone {
    pub id: u32,
    pub rect: Rect,
    pub active: bool,
    entities_inside: FxHashSet<EntityId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerEvent {
    Enter { zone_id: u32, entity: EntityId },
    Exit { zone_id: u32, entity: EntityId },
}

impl TriggerZone {
    pub fn new(id: u32, rect: Rect) -> Self {
        Self {
            id,
            rect,
            active: true,
            entities_inside: FxHashSet::default(),
        }
    }

    pub fn check(&mut self, entity: EntityId, entity_rect: &Rect) -> Option<TriggerEvent> {
        if !self.active {
            return None;
        }
        let overlaps = self.rect.overlaps(entity_rect);
        let was_inside = self.entities_inside.contains(&entity);
        match (was_inside, overlaps) {
            (false, true) => {
                self.entities_inside.insert(entity);
                Some(TriggerEvent::Enter {
                    zone_id: self.id,
                    entity,
                })
            }
            (true, false) => {
                self.entities_inside.remove(&entity);
                Some(TriggerEvent::Exit {
                    zone_id: self.id,
                    entity,
                })
            }
            _ => None,
        }
    }

    pub fn remove_entity(&mut self, entity: EntityId) {
        self.entities_inside.remove(&entity);
    }
}

/// High-level collision world managing entities and queries.
pub struct CollisionWorld {
    pub spatial_hash: SpatialHash,
    shapes: FxHashMap<EntityId, (RenderVec2, CollisionShape)>,
    pub triggers: Vec<TriggerZone>,
}

impl CollisionWorld {
    pub fn new(cell_size: f32) -> Self {
        Self {
            spatial_hash: SpatialHash::new(cell_size),
            shapes: FxHashMap::default(),
            triggers: Vec::new(),
        }
    }

    pub fn update_entity(&mut self, id: EntityId, pos: RenderVec2, shape: CollisionShape) {
        let aabb = shape_to_aabb(pos, &shape);
        self.spatial_hash.insert(id, &aabb);
        self.shapes.insert(id, (pos, shape));
    }

    pub fn remove_entity(&mut self, id: EntityId) {
        self.spatial_hash.remove(id);
        self.shapes.remove(&id);
        for trigger in &mut self.triggers {
            trigger.remove_entity(id);
        }
    }

    pub fn query_aabb(&self, rect: &Rect) -> Vec<EntityId> {
        self.spatial_hash.query_aabb(rect)
    }
    pub fn query_point(&self, x: f32, y: f32) -> Vec<EntityId> {
        self.spatial_hash.query_point(x, y)
    }
    pub fn query_circle(&self, cx: f32, cy: f32, radius: f32) -> Vec<EntityId> {
        self.spatial_hash.query_circle(cx, cy, radius)
    }

    pub fn check_pair(&self, a: EntityId, b: EntityId) -> Option<ContactInfo> {
        let (pos_a, shape_a) = self.shapes.get(&a)?;
        let (pos_b, shape_b) = self.shapes.get(&b)?;
        check_shapes(*pos_a, shape_a, *pos_b, shape_b)
    }

    pub fn check_triggers(&mut self, entity: EntityId) -> Vec<TriggerEvent> {
        let Some((pos, shape)) = self.shapes.get(&entity) else {
            return Vec::new();
        };
        let entity_aabb = shape_to_aabb(*pos, shape);
        let mut events = Vec::new();
        for trigger in &mut self.triggers {
            if let Some(event) = trigger.check(entity, &entity_aabb) {
                events.push(event);
            }
        }
        events
    }

    pub fn clear(&mut self) {
        self.spatial_hash.clear();
        self.shapes.clear();
        self.triggers.clear();
    }

    /// Get the position and shape for an entity. Used by raycast module.
    pub fn get_shape(&self, entity: EntityId) -> Option<(RenderVec2, &CollisionShape)> {
        self.shapes.get(&entity).map(|(pos, shape)| (*pos, shape))
    }
}

// ---------------------------------------------------------------------------
// Capsule Collider
// ---------------------------------------------------------------------------

/// Capsule = line segment + radius. Ideal for elongated entities (enemies in TD).
/// Slides around corners like a circle but covers elongated shapes.
#[derive(Clone, Copy, Debug)]
pub struct CapsuleShape {
    /// Half-length of the line segment (total length = 2 * half_length).
    pub half_length: f32,
    /// Radius at both ends.
    pub radius: f32,
    /// Rotation in radians (0 = horizontal).
    pub angle: f32,
}

impl CapsuleShape {
    /// Get the two endpoints of the capsule center line in world space.
    pub fn endpoints(&self, pos: RenderVec2) -> (RenderVec2, RenderVec2) {
        let cos = self.angle.cos();
        let sin = self.angle.sin();
        let dx = cos * self.half_length;
        let dy = sin * self.half_length;
        (
            RenderVec2::new(pos.x - dx, pos.y - dy),
            RenderVec2::new(pos.x + dx, pos.y + dy),
        )
    }
}

/// Find the closest point on a line segment (a, b) to point p.
fn closest_point_on_segment(a: RenderVec2, b: RenderVec2, p: RenderVec2) -> RenderVec2 {
    let ab = RenderVec2::new(b.x - a.x, b.y - a.y);
    let ap = RenderVec2::new(p.x - a.x, p.y - a.y);
    let ab_sq = ab.x * ab.x + ab.y * ab.y;
    if ab_sq < 1e-8 {
        return a;
    }
    let t = ((ap.x * ab.x + ap.y * ab.y) / ab_sq).clamp(0.0, 1.0);
    RenderVec2::new(a.x + ab.x * t, a.y + ab.y * t)
}

/// Capsule vs Circle collision.
pub fn capsule_vs_circle(
    capsule_pos: RenderVec2,
    capsule: &CapsuleShape,
    cx: f32,
    cy: f32,
    r: f32,
) -> Option<ContactInfo> {
    let (a, b) = capsule.endpoints(capsule_pos);
    let p = RenderVec2::new(cx, cy);
    let closest = closest_point_on_segment(a, b, p);
    circle_vs_circle(closest.x, closest.y, capsule.radius, cx, cy, r)
}

/// Capsule vs AABB collision (approximate: treat capsule as circle at closest point).
pub fn capsule_vs_aabb(
    capsule_pos: RenderVec2,
    capsule: &CapsuleShape,
    rect: &Rect,
) -> Option<ContactInfo> {
    let (a, b) = capsule.endpoints(capsule_pos);
    // Find closest point on capsule segment to AABB center
    let rect_center = RenderVec2::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5);
    let closest = closest_point_on_segment(a, b, rect_center);
    circle_vs_aabb(closest.x, closest.y, capsule.radius, rect)
}

/// Capsule vs Capsule collision.
pub fn capsule_vs_capsule(
    pos_a: RenderVec2,
    a: &CapsuleShape,
    pos_b: RenderVec2,
    b: &CapsuleShape,
) -> Option<ContactInfo> {
    let (a1, a2) = a.endpoints(pos_a);
    let (b1, b2) = b.endpoints(pos_b);
    // Find closest point pair between two line segments
    let mut min_dist_sq = f32::MAX;
    let mut best_ca = a1;
    let mut best_cb = b1;
    for &pa in &[a1, a2] {
        let cb = closest_point_on_segment(b1, b2, pa);
        let d = pa.distance_squared(cb);
        if d < min_dist_sq {
            min_dist_sq = d;
            best_ca = pa;
            best_cb = cb;
        }
    }
    for &pb in &[b1, b2] {
        let ca = closest_point_on_segment(a1, a2, pb);
        let d = ca.distance_squared(pb);
        if d < min_dist_sq {
            min_dist_sq = d;
            best_ca = ca;
            best_cb = pb;
        }
    }
    circle_vs_circle(
        best_ca.x, best_ca.y, a.radius, best_cb.x, best_cb.y, b.radius,
    )
}

// ---------------------------------------------------------------------------
// Swept AABB (CCD — Continuous Collision Detection)
// ---------------------------------------------------------------------------

/// Contact from a swept collision test.
#[derive(Clone, Copy, Debug)]
pub struct SweptContact {
    /// Time of impact [0.0, 1.0] within the tick.
    pub time: f32,
    /// Surface normal at impact.
    pub normal: RenderVec2,
    /// Contact info at impact.
    pub contact: ContactInfo,
}

/// Swept AABB collision test for fast-moving objects.
/// Tests a moving AABB against a static obstacle AABB.
/// Returns the first time of impact within the tick (velocity applied over 1 tick).
pub fn swept_aabb(
    pos: RenderVec2,
    velocity: RenderVec2,
    shape: &Rect,
    obstacle: &Rect,
) -> Option<SweptContact> {
    let moving = Rect::new(pos.x + shape.x, pos.y + shape.y, shape.w, shape.h);

    // Distance to entry/exit for each axis
    let (x_entry_dist, x_exit_dist) = if velocity.x > 0.0 {
        (
            obstacle.x - (moving.x + moving.w),
            (obstacle.x + obstacle.w) - moving.x,
        )
    } else {
        (
            (obstacle.x + obstacle.w) - moving.x,
            obstacle.x - (moving.x + moving.w),
        )
    };
    let (y_entry_dist, y_exit_dist) = if velocity.y > 0.0 {
        (
            obstacle.y - (moving.y + moving.h),
            (obstacle.y + obstacle.h) - moving.y,
        )
    } else {
        (
            (obstacle.y + obstacle.h) - moving.y,
            obstacle.y - (moving.y + moving.h),
        )
    };

    // Time of entry/exit for each axis
    let x_entry = if velocity.x.abs() < 1e-8 {
        -f32::MAX
    } else {
        x_entry_dist / velocity.x
    };
    let x_exit = if velocity.x.abs() < 1e-8 {
        f32::MAX
    } else {
        x_exit_dist / velocity.x
    };
    let y_entry = if velocity.y.abs() < 1e-8 {
        -f32::MAX
    } else {
        y_entry_dist / velocity.y
    };
    let y_exit = if velocity.y.abs() < 1e-8 {
        f32::MAX
    } else {
        y_exit_dist / velocity.y
    };

    let entry_time = x_entry.max(y_entry);
    let exit_time = x_exit.min(y_exit);

    // No collision
    if entry_time > exit_time || entry_time > 1.0 || exit_time < 0.0 {
        return None;
    }
    if entry_time < 0.0 {
        return None; // Already overlapping — not a swept hit
    }

    let normal = if x_entry > y_entry {
        RenderVec2::new(if velocity.x > 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else {
        RenderVec2::new(0.0, if velocity.y > 0.0 { -1.0 } else { 1.0 })
    };

    Some(SweptContact {
        time: entry_time,
        normal,
        contact: ContactInfo {
            penetration: 0.0, // At the moment of impact, penetration is zero
            normal,
        },
    })
}

pub fn shape_to_aabb(pos: RenderVec2, shape: &CollisionShape) -> Rect {
    match shape {
        CollisionShape::Aabb(r) => Rect::new(pos.x + r.x, pos.y + r.y, r.w, r.h),
        CollisionShape::Circle { cx, cy, radius } => Rect::new(
            pos.x + cx - radius,
            pos.y + cy - radius,
            radius * 2.0,
            radius * 2.0,
        ),
    }
}

/// Return the number of occupied cells in the spatial hash.
impl SpatialHash {
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn entity_count(&self) -> usize {
        self.entity_cells.len()
    }
}

pub fn check_shapes(
    pos_a: RenderVec2,
    shape_a: &CollisionShape,
    pos_b: RenderVec2,
    shape_b: &CollisionShape,
) -> Option<ContactInfo> {
    match (shape_a, shape_b) {
        (CollisionShape::Aabb(a), CollisionShape::Aabb(b)) => aabb_vs_aabb(
            &Rect::new(pos_a.x + a.x, pos_a.y + a.y, a.w, a.h),
            &Rect::new(pos_b.x + b.x, pos_b.y + b.y, b.w, b.h),
        ),
        (
            CollisionShape::Circle {
                cx: ax,
                cy: ay,
                radius: ar,
            },
            CollisionShape::Circle {
                cx: bx,
                cy: by,
                radius: br,
            },
        ) => circle_vs_circle(
            pos_a.x + ax,
            pos_a.y + ay,
            *ar,
            pos_b.x + bx,
            pos_b.y + by,
            *br,
        ),
        (CollisionShape::Circle { cx, cy, radius }, CollisionShape::Aabb(b)) => circle_vs_aabb(
            pos_a.x + cx,
            pos_a.y + cy,
            *radius,
            &Rect::new(pos_b.x + b.x, pos_b.y + b.y, b.w, b.h),
        ),
        (CollisionShape::Aabb(a), CollisionShape::Circle { cx, cy, radius }) => {
            let c = circle_vs_aabb(
                pos_b.x + cx,
                pos_b.y + cy,
                *radius,
                &Rect::new(pos_a.x + a.x, pos_a.y + a.y, a.w, a.h),
            )?;
            Some(ContactInfo {
                penetration: c.penetration,
                normal: RenderVec2::new(-c.normal.x, -c.normal.y),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    // ── AABB collision tests ───────────────────────────────────

    #[test]
    fn aabb_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert!(aabb_vs_aabb(&a, &b).is_some());
    }

    #[test]
    fn aabb_no_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(aabb_vs_aabb(&a, &b).is_none());
    }

    // ── Circle collision tests ─────────────────────────────────

    #[test]
    fn circle_overlap() {
        assert!(circle_vs_circle(0.0, 0.0, 5.0, 3.0, 0.0, 5.0).is_some());
    }

    #[test]
    fn circle_no_overlap() {
        assert!(circle_vs_circle(0.0, 0.0, 5.0, 20.0, 0.0, 5.0).is_none());
    }

    // ── SpatialHash tests ───────────────────────────────────────

    #[test]
    fn spatial_hash_insert_and_query() {
        let mut hash = SpatialHash::new(32.0);
        let e1 = EntityId::from_raw(1, 0);
        let e2 = EntityId::from_raw(2, 0);
        let e3 = EntityId::from_raw(3, 0);

        hash.insert(e1, &Rect::new(0.0, 0.0, 16.0, 16.0));
        hash.insert(e2, &Rect::new(100.0, 100.0, 16.0, 16.0));
        hash.insert(e3, &Rect::new(8.0, 8.0, 16.0, 16.0));

        // Query near e1 — should find e1 and e3 (overlapping region)
        let near_origin = hash.query_aabb(&Rect::new(0.0, 0.0, 20.0, 20.0));
        assert!(near_origin.contains(&e1));
        assert!(near_origin.contains(&e3));
        assert!(!near_origin.contains(&e2));

        // Query near e2 — should find only e2
        let near_e2 = hash.query_aabb(&Rect::new(90.0, 90.0, 30.0, 30.0));
        assert!(near_e2.contains(&e2));
        assert!(!near_e2.contains(&e1));
    }

    #[test]
    fn spatial_hash_remove() {
        let mut hash = SpatialHash::new(32.0);
        let e1 = EntityId::from_raw(1, 0);

        hash.insert(e1, &Rect::new(0.0, 0.0, 16.0, 16.0));
        assert_eq!(hash.entity_count(), 1);

        hash.remove(e1);
        assert_eq!(hash.entity_count(), 0);

        let result = hash.query_aabb(&Rect::new(0.0, 0.0, 100.0, 100.0));
        assert!(result.is_empty());
    }

    #[test]
    fn spatial_hash_clear() {
        let mut hash = SpatialHash::new(32.0);
        for i in 0..10 {
            hash.insert(
                EntityId::from_raw(i, 0),
                &Rect::new(i as f32 * 10.0, 0.0, 8.0, 8.0),
            );
        }
        assert_eq!(hash.entity_count(), 10);
        hash.clear();
        assert_eq!(hash.entity_count(), 0);
        assert_eq!(hash.cell_count(), 0);
    }

    #[test]
    fn spatial_hash_point_query() {
        let mut hash = SpatialHash::new(32.0);
        let e1 = EntityId::from_raw(1, 0);
        hash.insert(e1, &Rect::new(0.0, 0.0, 32.0, 32.0));

        let found = hash.query_point(16.0, 16.0);
        assert!(found.contains(&e1));

        let not_found = hash.query_point(100.0, 100.0);
        assert!(!not_found.contains(&e1));
    }

    #[test]
    fn spatial_hash_circle_query() {
        let mut hash = SpatialHash::new(32.0);
        let e1 = EntityId::from_raw(1, 0);
        hash.insert(e1, &Rect::new(10.0, 10.0, 8.0, 8.0));

        let found = hash.query_circle(14.0, 14.0, 20.0);
        assert!(found.contains(&e1));

        let not_found = hash.query_circle(200.0, 200.0, 5.0);
        assert!(!not_found.contains(&e1));
    }

    #[test]
    fn spatial_hash_update_position() {
        let mut hash = SpatialHash::new(32.0);
        let e1 = EntityId::from_raw(1, 0);

        hash.insert(e1, &Rect::new(0.0, 0.0, 8.0, 8.0));
        let near_origin = hash.query_point(4.0, 4.0);
        assert!(near_origin.contains(&e1));

        // Move far away
        hash.insert(e1, &Rect::new(500.0, 500.0, 8.0, 8.0));
        let near_origin = hash.query_point(4.0, 4.0);
        assert!(!near_origin.contains(&e1));
        let near_new = hash.query_point(504.0, 504.0);
        assert!(near_new.contains(&e1));
    }

    // ── CollisionWorld tests ────────────────────────────────────

    #[test]
    fn collision_world_check_pair() {
        let mut world = CollisionWorld::new(32.0);
        let e1 = EntityId::from_raw(1, 0);
        let e2 = EntityId::from_raw(2, 0);

        world.update_entity(
            e1,
            RenderVec2::new(0.0, 0.0),
            CollisionShape::Aabb(Rect::new(0.0, 0.0, 10.0, 10.0)),
        );
        world.update_entity(
            e2,
            RenderVec2::new(5.0, 5.0),
            CollisionShape::Aabb(Rect::new(0.0, 0.0, 10.0, 10.0)),
        );

        assert!(world.check_pair(e1, e2).is_some());
    }

    // ── TriggerZone tests ───────────────────────────────────────

    #[test]
    fn trigger_zone_enter_exit() {
        let mut zone = TriggerZone::new(1, Rect::new(0.0, 0.0, 50.0, 50.0));
        let e1 = EntityId::from_raw(1, 0);

        // Enter
        let event = zone.check(e1, &Rect::new(10.0, 10.0, 5.0, 5.0));
        assert!(matches!(event, Some(TriggerEvent::Enter { .. })));

        // Stay inside — no event
        let event = zone.check(e1, &Rect::new(20.0, 20.0, 5.0, 5.0));
        assert!(event.is_none());

        // Exit
        let event = zone.check(e1, &Rect::new(100.0, 100.0, 5.0, 5.0));
        assert!(matches!(event, Some(TriggerEvent::Exit { .. })));
    }
}
