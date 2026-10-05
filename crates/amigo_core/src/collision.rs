//! Collision shapes, narrow-phase tests, a spatial hash, trigger zones and
//! swept AABBs, all in simulation space ([`Fix`], [`SimVec2`], [`SimRect`]).
//!
//! These used to run on `f32`, whose rounding (and libm's `sqrt`, `sin`,
//! `cos`) is not guaranteed to match between platforms, so two machines could
//! disagree about whether a bullet hit (ADR-0001). Queries also returned
//! entities in hash-set order; they now come back sorted by [`EntityId`].

use crate::ecs::EntityId;
use crate::math::{Fix, SimVec2};
use crate::rect::SimRect;
use rustc_hash::{FxHashMap, FxHashSet};

/// Collision shape for an entity, relative to its position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionShape {
    Aabb(SimRect),
    Circle { center: SimVec2, radius: Fix },
}

/// Contact information from a collision check. `normal` points from B to A:
/// moving A along it by `penetration` separates the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactInfo {
    pub penetration: Fix,
    pub normal: SimVec2,
}

fn unit_x(sign: i32) -> SimVec2 {
    SimVec2::new(Fix::from_num(sign), Fix::ZERO)
}

fn unit_y(sign: i32) -> SimVec2 {
    SimVec2::new(Fix::ZERO, Fix::from_num(sign))
}

pub fn aabb_vs_aabb(a: &SimRect, b: &SimRect) -> Option<ContactInfo> {
    let (ca, cb) = (a.center(), b.center());
    let overlap_x = (a.w + b.w) / 2 - (ca.x - cb.x).abs();
    let overlap_y = (a.h + b.h) / 2 - (ca.y - cb.y).abs();
    if overlap_x <= Fix::ZERO || overlap_y <= Fix::ZERO {
        return None;
    }
    if overlap_x < overlap_y {
        let sign = if ca.x < cb.x { -1 } else { 1 };
        Some(ContactInfo {
            penetration: overlap_x,
            normal: unit_x(sign),
        })
    } else {
        let sign = if ca.y < cb.y { -1 } else { 1 };
        Some(ContactInfo {
            penetration: overlap_y,
            normal: unit_y(sign),
        })
    }
}

/// Two circles. The normal points from B to A, like every other shape pair
/// here and as `PhysicsWorld` resolution expects.
pub fn circle_vs_circle(a: SimVec2, ar: Fix, b: SimVec2, br: Fix) -> Option<ContactInfo> {
    let d = a - b;
    // `length` is exact and cannot overflow, unlike squaring in Q16.16.
    let dist = d.length();
    let sum_r = ar + br;
    if dist >= sum_r {
        return None;
    }
    if dist == Fix::ZERO {
        return Some(ContactInfo {
            penetration: sum_r,
            normal: unit_x(1),
        });
    }
    Some(ContactInfo {
        penetration: sum_r - dist,
        normal: d / dist,
    })
}

pub fn circle_vs_aabb(center: SimVec2, radius: Fix, rect: &SimRect) -> Option<ContactInfo> {
    let closest = SimVec2::new(
        center.x.clamp(rect.x, rect.right()),
        center.y.clamp(rect.y, rect.bottom()),
    );
    let d = center - closest;
    let dist = d.length();
    if dist >= radius {
        return None;
    }
    if dist == Fix::ZERO {
        return Some(ContactInfo {
            penetration: radius,
            normal: unit_y(-1),
        });
    }
    Some(ContactInfo {
        penetration: radius - dist,
        normal: d / dist,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CellKey(i32, i32);

/// Spatial hash grid for broad-phase collision detection.
pub struct SpatialHash {
    cell_size: Fix,
    cells: FxHashMap<CellKey, Vec<EntityId>>,
    entity_cells: FxHashMap<EntityId, Vec<CellKey>>,
}

impl SpatialHash {
    /// # Panics
    /// If `cell_size` is not positive.
    pub fn new(cell_size: Fix) -> Self {
        assert!(cell_size > Fix::ZERO, "cell size must be positive");
        Self {
            cell_size,
            cells: FxHashMap::default(),
            entity_cells: FxHashMap::default(),
        }
    }

    fn cell_key(&self, p: SimVec2) -> CellKey {
        let cell = |v: Fix| v.saturating_div(self.cell_size).floor().to_num::<i32>();
        CellKey(cell(p.x), cell(p.y))
    }

    pub fn insert(&mut self, id: EntityId, aabb: &SimRect) {
        self.remove(id);
        let min_key = self.cell_key(SimVec2::new(aabb.x, aabb.y));
        let max_key = self.cell_key(SimVec2::new(aabb.right(), aabb.bottom()));
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

    /// Entities whose cells overlap `aabb`, sorted by id (each once).
    pub fn query_aabb(&self, aabb: &SimRect) -> Vec<EntityId> {
        let min_key = self.cell_key(SimVec2::new(aabb.x, aabb.y));
        let max_key = self.cell_key(SimVec2::new(aabb.right(), aabb.bottom()));
        let mut result = Vec::new();
        for cy in min_key.1..=max_key.1 {
            for cx in min_key.0..=max_key.0 {
                if let Some(cell) = self.cells.get(&CellKey(cx, cy)) {
                    result.extend_from_slice(cell);
                }
            }
        }
        // Hash-set order differs between runs and builds; a sorted list does
        // not, so whatever the caller does with the hits happens in the same
        // order everywhere.
        result.sort_unstable();
        result.dedup();
        result
    }

    /// Entities in the cell containing `p`, sorted by id.
    pub fn query_point(&self, p: SimVec2) -> Vec<EntityId> {
        let mut found = self
            .cells
            .get(&self.cell_key(p))
            .cloned()
            .unwrap_or_default();
        found.sort_unstable();
        found
    }

    pub fn query_circle(&self, center: SimVec2, radius: Fix) -> Vec<EntityId> {
        self.query_aabb(&SimRect::new(
            center.x - radius,
            center.y - radius,
            radius * 2,
            radius * 2,
        ))
    }

    /// Number of occupied cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn entity_count(&self) -> usize {
        self.entity_cells.len()
    }
}

/// Trigger zone that fires events when entities enter/exit.
#[derive(Clone, Debug)]
pub struct TriggerZone {
    pub id: u32,
    pub rect: SimRect,
    pub active: bool,
    entities_inside: FxHashSet<EntityId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerEvent {
    Enter { zone_id: u32, entity: EntityId },
    Exit { zone_id: u32, entity: EntityId },
}

impl TriggerZone {
    pub fn new(id: u32, rect: SimRect) -> Self {
        Self {
            id,
            rect,
            active: true,
            entities_inside: FxHashSet::default(),
        }
    }

    pub fn check(&mut self, entity: EntityId, entity_rect: &SimRect) -> Option<TriggerEvent> {
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
    shapes: FxHashMap<EntityId, (SimVec2, CollisionShape)>,
    pub triggers: Vec<TriggerZone>,
}

impl CollisionWorld {
    pub fn new(cell_size: Fix) -> Self {
        Self {
            spatial_hash: SpatialHash::new(cell_size),
            shapes: FxHashMap::default(),
            triggers: Vec::new(),
        }
    }

    pub fn update_entity(&mut self, id: EntityId, pos: SimVec2, shape: CollisionShape) {
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

    pub fn query_aabb(&self, rect: &SimRect) -> Vec<EntityId> {
        self.spatial_hash.query_aabb(rect)
    }
    pub fn query_point(&self, p: SimVec2) -> Vec<EntityId> {
        self.spatial_hash.query_point(p)
    }
    pub fn query_circle(&self, center: SimVec2, radius: Fix) -> Vec<EntityId> {
        self.spatial_hash.query_circle(center, radius)
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
    pub fn get_shape(&self, entity: EntityId) -> Option<(SimVec2, &CollisionShape)> {
        self.shapes.get(&entity).map(|(pos, shape)| (*pos, shape))
    }
}

// ---------------------------------------------------------------------------
// Capsule Collider
// ---------------------------------------------------------------------------

/// Capsule = line segment + radius. Ideal for elongated entities (enemies in TD).
/// Slides around corners like a circle but covers elongated shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapsuleShape {
    /// Half-length of the line segment (total length = 2 * half_length).
    pub half_length: Fix,
    /// Radius at both ends.
    pub radius: Fix,
    /// Rotation in radians (0 = horizontal).
    pub angle: Fix,
}

impl CapsuleShape {
    /// Get the two endpoints of the capsule center line in world space.
    pub fn endpoints(&self, pos: SimVec2) -> (SimVec2, SimVec2) {
        let d = SimVec2::from_angle(self.angle) * self.half_length;
        (pos - d, pos + d)
    }
}

/// Find the closest point on a line segment (a, b) to point p.
///
/// The projection is computed on widened integers: `|ab|²` in Q16.16
/// overflows for segments longer than ~181 units.
pub fn closest_point_on_segment(a: SimVec2, b: SimVec2, p: SimVec2) -> SimVec2 {
    let ab = b - a;
    let ap = p - a;
    let wide = |v: Fix| i128::from(v.to_bits());
    let ab_sq = wide(ab.x) * wide(ab.x) + wide(ab.y) * wide(ab.y);
    if ab_sq == 0 {
        return a;
    }
    let dot = wide(ap.x) * wide(ab.x) + wide(ap.y) * wide(ab.y);
    // t in Q16.16, clamped to [0, 1].
    let t = ((dot << 16) / ab_sq).clamp(0, 1 << 16);
    let t = Fix::from_bits(t as i32);
    a + ab * t
}

/// Where segments `a1-a2` and `b1-b2` cross, if they do (touching counts).
/// Integer cross products on widened raw bits, so the test is exact.
fn segment_intersection(a1: SimVec2, a2: SimVec2, b1: SimVec2, b2: SimVec2) -> Option<SimVec2> {
    let raw = |v: Fix| i128::from(v.to_bits());
    let cross = |o: SimVec2, p: SimVec2, q: SimVec2| {
        (raw(p.x) - raw(o.x)) * (raw(q.y) - raw(o.y))
            - (raw(p.y) - raw(o.y)) * (raw(q.x) - raw(o.x))
    };
    let d1 = cross(b1, b2, a1);
    let d2 = cross(b1, b2, a2);
    let d3 = cross(a1, a2, b1);
    let d4 = cross(a1, a2, b2);
    let straddles = |p: i128, q: i128| (p <= 0 && q >= 0) || (p >= 0 && q <= 0);
    if !(straddles(d1, d2) && straddles(d3, d4)) || (d1 == d2 && d3 == d4) {
        // Apart, or collinear (handled by the endpoint projections).
        return None;
    }
    // Point along a: a1 + (a2 - a1) * d1 / (d1 - d2).
    let den = d1 - d2;
    if den == 0 {
        return None;
    }
    let t = ((d1 << 16) / den).clamp(0, 1 << 16);
    Some(a1 + (a2 - a1) * Fix::from_bits(t as i32))
}

/// Capsule vs Circle collision.
pub fn capsule_vs_circle(
    capsule_pos: SimVec2,
    capsule: &CapsuleShape,
    center: SimVec2,
    r: Fix,
) -> Option<ContactInfo> {
    let (a, b) = capsule.endpoints(capsule_pos);
    let closest = closest_point_on_segment(a, b, center);
    circle_vs_circle(closest, capsule.radius, center, r)
}

/// Capsule vs AABB collision (approximate: treat capsule as circle at closest point).
pub fn capsule_vs_aabb(
    capsule_pos: SimVec2,
    capsule: &CapsuleShape,
    rect: &SimRect,
) -> Option<ContactInfo> {
    let (a, b) = capsule.endpoints(capsule_pos);
    let closest = closest_point_on_segment(a, b, rect.center());
    circle_vs_aabb(closest, capsule.radius, rect)
}

/// Capsule vs Capsule collision.
pub fn capsule_vs_capsule(
    pos_a: SimVec2,
    a: &CapsuleShape,
    pos_b: SimVec2,
    b: &CapsuleShape,
) -> Option<ContactInfo> {
    let (a1, a2) = a.endpoints(pos_a);
    let (b1, b2) = b.endpoints(pos_b);
    // Crossing segments are at distance zero; the endpoint projections below
    // miss that case (an X of two capsules used to report no contact).
    if let Some(p) = segment_intersection(a1, a2, b1, b2) {
        return circle_vs_circle(p, a.radius, p, b.radius);
    }
    // Otherwise the closest pair involves an endpoint: take the best of the
    // four endpoint-to-segment projections; ties keep the first found.
    let mut best = (Fix::MAX, a1, b1);
    for pa in [a1, a2] {
        let cb = closest_point_on_segment(b1, b2, pa);
        let d = pa.distance_squared(cb);
        if d < best.0 {
            best = (d, pa, cb);
        }
    }
    for pb in [b1, b2] {
        let ca = closest_point_on_segment(a1, a2, pb);
        let d = ca.distance_squared(pb);
        if d < best.0 {
            best = (d, ca, pb);
        }
    }
    circle_vs_circle(best.1, a.radius, best.2, b.radius)
}

// ---------------------------------------------------------------------------
// Swept AABB (CCD — Continuous Collision Detection)
// ---------------------------------------------------------------------------

/// Contact from a swept collision test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SweptContact {
    /// Time of impact [0, 1] within the tick.
    pub time: Fix,
    /// Surface normal at impact.
    pub normal: SimVec2,
    /// Contact info at impact.
    pub contact: ContactInfo,
}

/// Swept AABB collision test for fast-moving objects.
/// Tests a moving AABB against a static obstacle AABB.
/// Returns the first time of impact within the tick (velocity applied over 1 tick).
pub fn swept_aabb(
    pos: SimVec2,
    velocity: SimVec2,
    shape: &SimRect,
    obstacle: &SimRect,
) -> Option<SweptContact> {
    let moving = shape.translated(pos);

    // Distance to entry/exit for each axis
    let (x_entry_dist, x_exit_dist) = if velocity.x > Fix::ZERO {
        (obstacle.x - moving.right(), obstacle.right() - moving.x)
    } else {
        (obstacle.right() - moving.x, obstacle.x - moving.right())
    };
    let (y_entry_dist, y_exit_dist) = if velocity.y > Fix::ZERO {
        (obstacle.y - moving.bottom(), obstacle.bottom() - moving.y)
    } else {
        (obstacle.bottom() - moving.y, obstacle.y - moving.bottom())
    };

    // Time of entry/exit per axis; an axis without movement never limits.
    // Saturating: a tiny velocity gives a time far past 1 tick, not a panic.
    let time = |dist: Fix, v: Fix, none: Fix| {
        if v == Fix::ZERO {
            none
        } else {
            dist.saturating_div(v)
        }
    };
    let x_entry = time(x_entry_dist, velocity.x, Fix::MIN);
    let x_exit = time(x_exit_dist, velocity.x, Fix::MAX);
    let y_entry = time(y_entry_dist, velocity.y, Fix::MIN);
    let y_exit = time(y_exit_dist, velocity.y, Fix::MAX);

    let entry_time = x_entry.max(y_entry);
    let exit_time = x_exit.min(y_exit);

    // No collision
    if entry_time > exit_time || entry_time > Fix::ONE || exit_time < Fix::ZERO {
        return None;
    }
    if entry_time < Fix::ZERO {
        return None; // Already overlapping — not a swept hit
    }

    let normal = if x_entry > y_entry {
        unit_x(if velocity.x > Fix::ZERO { -1 } else { 1 })
    } else {
        unit_y(if velocity.y > Fix::ZERO { -1 } else { 1 })
    };

    Some(SweptContact {
        time: entry_time,
        normal,
        contact: ContactInfo {
            penetration: Fix::ZERO, // At the moment of impact, penetration is zero
            normal,
        },
    })
}

pub fn shape_to_aabb(pos: SimVec2, shape: &CollisionShape) -> SimRect {
    match shape {
        CollisionShape::Aabb(r) => r.translated(pos),
        CollisionShape::Circle { center, radius } => {
            let c = pos + *center;
            SimRect::new(c.x - *radius, c.y - *radius, *radius * 2, *radius * 2)
        }
    }
}

pub fn check_shapes(
    pos_a: SimVec2,
    shape_a: &CollisionShape,
    pos_b: SimVec2,
    shape_b: &CollisionShape,
) -> Option<ContactInfo> {
    match (shape_a, shape_b) {
        (CollisionShape::Aabb(a), CollisionShape::Aabb(b)) => {
            aabb_vs_aabb(&a.translated(pos_a), &b.translated(pos_b))
        }
        (
            CollisionShape::Circle {
                center: ca,
                radius: ra,
            },
            CollisionShape::Circle {
                center: cb,
                radius: rb,
            },
        ) => circle_vs_circle(pos_a + *ca, *ra, pos_b + *cb, *rb),
        (CollisionShape::Circle { center, radius }, CollisionShape::Aabb(b)) => {
            circle_vs_aabb(pos_a + *center, *radius, &b.translated(pos_b))
        }
        (CollisionShape::Aabb(a), CollisionShape::Circle { center, radius }) => {
            let c = circle_vs_aabb(pos_b + *center, *radius, &a.translated(pos_a))?;
            Some(ContactInfo {
                penetration: c.penetration,
                normal: -c.normal,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    fn f(v: impl fixed::traits::ToFixed) -> Fix {
        Fix::from_num(v)
    }
    fn v(x: impl fixed::traits::ToFixed, y: impl fixed::traits::ToFixed) -> SimVec2 {
        SimVec2::from_num(x, y)
    }
    fn r(
        x: impl fixed::traits::ToFixed,
        y: impl fixed::traits::ToFixed,
        w: impl fixed::traits::ToFixed,
        h: impl fixed::traits::ToFixed,
    ) -> SimRect {
        SimRect::from_num(x, y, w, h)
    }

    // ── AABB collision tests ───────────────────────────────────

    #[test]
    fn aabb_overlap() {
        assert!(aabb_vs_aabb(&r(0, 0, 10, 10), &r(5, 5, 10, 10)).is_some());
    }

    #[test]
    fn aabb_no_overlap() {
        assert!(aabb_vs_aabb(&r(0, 0, 10, 10), &r(20, 20, 10, 10)).is_none());
    }

    // ── Circle collision tests ─────────────────────────────────

    #[test]
    fn circle_overlap() {
        assert!(circle_vs_circle(v(0, 0), f(5), v(3, 0), f(5)).is_some());
    }

    #[test]
    fn circle_no_overlap() {
        assert!(circle_vs_circle(v(0, 0), f(5), v(20, 0), f(5)).is_none());
    }

    /// Circles far apart used to be compared by squaring the distance in
    /// Q16.16, which overflows past ~181 units.
    #[test]
    fn distant_circles_do_not_overflow() {
        assert!(circle_vs_circle(v(-15000, 0), f(5), v(15000, 0), f(5)).is_none());
        let hit = circle_vs_circle(v(10000, 0), f(300), v(10500, 0), f(300)).unwrap();
        assert_eq!(hit.penetration, f(100));
        assert_eq!(hit.normal, v(-1, 0));
    }

    #[test]
    fn every_shape_pair_reports_the_normal_from_b_to_a() {
        // A sits left of B in each case, so the normal points left.
        let c = circle_vs_circle(v(0, 0), f(5), v(8, 0), f(5)).unwrap();
        assert_eq!(c.normal, v(-1, 0));
        assert_eq!(c.penetration, f(2));

        let a = r(0, 0, 10, 10);
        let b = r(8, 0, 10, 10);
        assert_eq!(aabb_vs_aabb(&a, &b).unwrap().normal, v(-1, 0));

        let c = circle_vs_aabb(v(5, 5), f(5), &b).unwrap();
        assert_eq!(c.normal, v(-1, 0));

        let shape = CollisionShape::Aabb(r(0, 0, 10, 10));
        let circle = CollisionShape::Circle {
            center: v(0, 0),
            radius: f(5),
        };
        let c = check_shapes(v(0, 0), &shape, v(13, 5), &circle).unwrap();
        assert_eq!(
            c.normal,
            v(-1, 0),
            "AABB vs circle flips the circle's normal"
        );
    }

    // ── SpatialHash tests ───────────────────────────────────────

    #[test]
    fn spatial_hash_insert_and_query() {
        let mut hash = SpatialHash::new(f(32));
        let e1 = EntityId::from_raw(1, 0);
        let e2 = EntityId::from_raw(2, 0);
        let e3 = EntityId::from_raw(3, 0);

        hash.insert(e1, &r(0, 0, 16, 16));
        hash.insert(e2, &r(100, 100, 16, 16));
        hash.insert(e3, &r(8, 8, 16, 16));

        assert_eq!(hash.query_aabb(&r(0, 0, 20, 20)), [e1, e3]);
        assert_eq!(hash.query_aabb(&r(90, 90, 30, 30)), [e2]);
    }

    /// Results come back sorted and without duplicates, whatever the
    /// insertion order and however many cells an entity spans.
    #[test]
    fn spatial_hash_queries_are_sorted_and_unique() {
        let ids: Vec<EntityId> = (0..20).map(|i| EntityId::from_raw(i, 0)).collect();
        let mut forward = SpatialHash::new(f(8));
        let mut backward = SpatialHash::new(f(8));
        for (i, &id) in ids.iter().enumerate() {
            forward.insert(id, &r(i as i32 * 3, 0, 20, 20));
        }
        for (i, &id) in ids.iter().enumerate().rev() {
            backward.insert(id, &r(i as i32 * 3, 0, 20, 20));
        }
        let all = r(-10, -10, 200, 50);
        assert_eq!(forward.query_aabb(&all), ids);
        assert_eq!(backward.query_aabb(&all), ids);
    }

    #[test]
    fn spatial_hash_handles_negative_coordinates() {
        let mut hash = SpatialHash::new(f(32));
        let e = EntityId::from_raw(1, 0);
        hash.insert(e, &r(-40, -40, 8, 8));
        assert_eq!(hash.query_point(v(-36, -36)), [e]);
        assert!(hash.query_point(v(4, 4)).is_empty());
    }

    #[test]
    fn spatial_hash_remove() {
        let mut hash = SpatialHash::new(f(32));
        let e1 = EntityId::from_raw(1, 0);

        hash.insert(e1, &r(0, 0, 16, 16));
        assert_eq!(hash.entity_count(), 1);

        hash.remove(e1);
        assert_eq!(hash.entity_count(), 0);
        assert!(hash.query_aabb(&r(0, 0, 100, 100)).is_empty());
    }

    #[test]
    fn spatial_hash_clear() {
        let mut hash = SpatialHash::new(f(32));
        for i in 0..10 {
            hash.insert(EntityId::from_raw(i, 0), &r(i as i32 * 10, 0, 8, 8));
        }
        assert_eq!(hash.entity_count(), 10);
        hash.clear();
        assert_eq!(hash.entity_count(), 0);
        assert_eq!(hash.cell_count(), 0);
    }

    #[test]
    fn spatial_hash_point_and_circle_queries() {
        let mut hash = SpatialHash::new(f(32));
        let e1 = EntityId::from_raw(1, 0);
        hash.insert(e1, &r(0, 0, 32, 32));
        assert!(hash.query_point(v(16, 16)).contains(&e1));
        assert!(!hash.query_point(v(100, 100)).contains(&e1));

        let e2 = EntityId::from_raw(2, 0);
        hash.insert(e2, &r(10, 10, 8, 8));
        assert!(hash.query_circle(v(14, 14), f(20)).contains(&e2));
        assert!(hash.query_circle(v(200, 200), f(5)).is_empty());
    }

    #[test]
    fn spatial_hash_update_position() {
        let mut hash = SpatialHash::new(f(32));
        let e1 = EntityId::from_raw(1, 0);

        hash.insert(e1, &r(0, 0, 8, 8));
        assert!(hash.query_point(v(4, 4)).contains(&e1));

        // Move far away
        hash.insert(e1, &r(500, 500, 8, 8));
        assert!(!hash.query_point(v(4, 4)).contains(&e1));
        assert!(hash.query_point(v(504, 504)).contains(&e1));
    }

    // ── CollisionWorld tests ────────────────────────────────────

    #[test]
    fn collision_world_check_pair() {
        let mut world = CollisionWorld::new(f(32));
        let e1 = EntityId::from_raw(1, 0);
        let e2 = EntityId::from_raw(2, 0);
        let square = CollisionShape::Aabb(r(0, 0, 10, 10));
        world.update_entity(e1, v(0, 0), square);
        world.update_entity(e2, v(5, 5), square);
        assert!(world.check_pair(e1, e2).is_some());
    }

    // ── TriggerZone tests ───────────────────────────────────────

    #[test]
    fn trigger_zone_enter_exit() {
        let mut zone = TriggerZone::new(1, r(0, 0, 50, 50));
        let e1 = EntityId::from_raw(1, 0);

        let event = zone.check(e1, &r(10, 10, 5, 5));
        assert!(matches!(event, Some(TriggerEvent::Enter { .. })));

        // Stay inside — no event
        assert!(zone.check(e1, &r(20, 20, 5, 5)).is_none());

        let event = zone.check(e1, &r(100, 100, 5, 5));
        assert!(matches!(event, Some(TriggerEvent::Exit { .. })));
    }

    // ── Capsules and swept AABBs ────────────────────────────────

    #[test]
    fn capsules_collide_along_their_length() {
        let horizontal = CapsuleShape {
            half_length: f(10),
            radius: f(2),
            angle: Fix::ZERO,
        };
        let (a, b) = horizontal.endpoints(v(0, 0));
        assert_eq!((a, b), (v(-10, 0), v(10, 0)));
        // A circle above the middle of the segment touches it.
        assert!(capsule_vs_circle(v(0, 0), &horizontal, v(8, 3), f(2)).is_some());
        assert!(capsule_vs_circle(v(0, 0), &horizontal, v(14, 0), f(1)).is_none());

        let vertical = CapsuleShape {
            angle: crate::math::trig::FRAC_PI_2,
            ..horizontal
        };
        assert!(capsule_vs_capsule(v(0, 0), &horizontal, v(5, 0), &vertical).is_some());
        assert!(capsule_vs_capsule(v(0, 0), &horizontal, v(5, 20), &vertical).is_none());
        assert!(capsule_vs_aabb(v(0, 0), &horizontal, &r(9, -1, 4, 2)).is_some());
    }

    #[test]
    fn crossing_segments_intersect_where_they_cross() {
        assert_eq!(
            segment_intersection(v(-10, 0), v(10, 0), v(5, -10), v(5, 10)),
            Some(v(5, 0))
        );
        assert_eq!(
            segment_intersection(v(-10, 0), v(10, 0), v(5, 1), v(5, 10)),
            None
        );
        assert_eq!(
            segment_intersection(v(0, 0), v(4, 0), v(6, 0), v(9, 0)),
            None,
            "collinear"
        );
    }

    #[test]
    fn closest_point_handles_long_segments() {
        // |ab|² is far past Q16.16 here.
        let p = closest_point_on_segment(v(-10000, 0), v(10000, 0), v(2500, 77));
        assert_eq!(p, v(2500, 0));
        assert_eq!(closest_point_on_segment(v(1, 1), v(1, 1), v(5, 5)), v(1, 1));
    }

    #[test]
    fn swept_aabb_finds_the_time_of_impact() {
        let shape = r(0, 0, 10, 10);
        let wall = r(30, 0, 10, 10);
        let hit = swept_aabb(v(0, 0), v(40, 0), &shape, &wall).expect("hits the wall");
        assert_eq!(hit.time, f(0.5));
        assert_eq!(hit.normal, v(-1, 0));
        assert!(
            swept_aabb(v(0, 0), v(10, 0), &shape, &wall).is_none(),
            "too slow"
        );
        // A tiny velocity used to divide into huge times; it saturates now.
        assert!(swept_aabb(v(0, 0), v(0.001, 0), &shape, &wall).is_none());
        assert!(swept_aabb(v(0, 0), v(0, 0), &shape, &wall).is_none());
    }
}
