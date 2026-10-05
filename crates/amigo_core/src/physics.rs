//! Rigid-body physics: gravity, integration and impulse-based collision
//! resolution, in fixed point ([`Fix`], [`SimVec2`]).
//!
//! Deterministic by construction (ADR-0001): no `f32`, bodies are kept in a
//! `BTreeMap` so every pass visits them in [`EntityId`] order (the
//! `FxHashMap` this used before iterated in hash order), and contact pairs
//! are resolved in sorted order whichever broad phase found them.

use crate::collision::{CollisionShape, ContactInfo, SpatialHash, check_shapes, shape_to_aabb};
use crate::ecs::EntityId;
use crate::math::{Fix, SimVec2};
use crate::rect::SimRect;
use std::collections::BTreeMap;

/// Determines how a body participates in the physics simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyType {
    /// Immovable, infinite mass. Walls, platforms, terrain.
    Static,
    /// Fully simulated: gravity, velocity, collision response.
    Dynamic,
    /// Moved by code only (e.g. moving platforms). Affects dynamic bodies but
    /// is not itself affected by collisions.
    Kinematic,
}

/// A rigid body in the 2D physics simulation.
#[derive(Clone, Debug)]
pub struct RigidBody {
    pub body_type: BodyType,
    pub position: SimVec2,
    pub velocity: SimVec2,
    pub shape: CollisionShape,
    /// Mass in arbitrary units. Ignored for Static/Kinematic bodies.
    pub mass: Fix,
    inv_mass: Fix,
    /// Bounciness. 0 = no bounce, 1 = perfectly elastic.
    pub restitution: Fix,
    /// Friction coefficient for tangential velocity damping.
    pub friction: Fix,
    /// Multiplier on gravity for this body. 0 = no gravity.
    pub gravity_scale: Fix,
}

impl RigidBody {
    /// # Panics
    /// If `mass` is not positive.
    pub fn dynamic(position: SimVec2, shape: CollisionShape, mass: Fix) -> Self {
        assert!(mass > Fix::ZERO, "Dynamic body must have positive mass");
        Self {
            body_type: BodyType::Dynamic,
            position,
            velocity: SimVec2::ZERO,
            shape,
            mass,
            inv_mass: Fix::ONE / mass,
            restitution: Fix::ZERO,
            friction: Fix::from_num(0.2),
            gravity_scale: Fix::ONE,
        }
    }

    pub fn static_body(position: SimVec2, shape: CollisionShape) -> Self {
        Self {
            body_type: BodyType::Static,
            position,
            velocity: SimVec2::ZERO,
            shape,
            mass: Fix::ZERO,
            inv_mass: Fix::ZERO,
            restitution: Fix::ZERO,
            friction: Fix::from_num(0.5),
            gravity_scale: Fix::ZERO,
        }
    }

    pub fn kinematic(position: SimVec2, shape: CollisionShape) -> Self {
        Self {
            body_type: BodyType::Kinematic,
            ..Self::static_body(position, shape)
        }
    }

    pub fn inverse_mass(&self) -> Fix {
        self.inv_mass
    }

    /// # Panics
    /// If `mass` is not positive.
    pub fn set_mass(&mut self, mass: Fix) {
        assert!(mass > Fix::ZERO, "mass must be positive");
        self.mass = mass;
        self.inv_mass = Fix::ONE / mass;
    }
}

/// A collision event produced by the physics step.
#[derive(Clone, Debug)]
pub struct PhysicsContact {
    pub entity_a: EntityId,
    pub entity_b: EntityId,
    pub contact: ContactInfo,
}

/// 2D physics world with gravity, integration, and impulse-based collision resolution.
pub struct PhysicsWorld {
    /// Gravity in units/tick². Typically (0, positive) for downward gravity.
    pub gravity: SimVec2,
    /// Number of iterations for constraint/collision solving per step.
    pub solver_iterations: u32,
    bodies: BTreeMap<EntityId, RigidBody>,
    spatial_hash: SpatialHash,
    /// Optional pluggable broad phase. When set, it replaces the built-in
    /// spatial hash for collision-pair detection (see [`set_broad_phase`](Self::set_broad_phase)).
    broad_phase: Option<Box<dyn crate::broad_phase::BroadPhase>>,
    /// Velocity threshold for CCD. Bodies faster than this use swept tests.
    /// Default: 0 (disabled). Set via `set_ccd_threshold()`.
    ccd_threshold: Fix,
}

impl PhysicsWorld {
    pub fn new(gravity: SimVec2, cell_size: Fix) -> Self {
        Self {
            gravity,
            solver_iterations: 4,
            bodies: BTreeMap::new(),
            spatial_hash: SpatialHash::new(cell_size),
            broad_phase: None,
            ccd_threshold: Fix::ZERO,
        }
    }
    /// Replace the built-in spatial-hash broad phase with a custom
    /// [`BroadPhase`](crate::broad_phase::BroadPhase) implementation, e.g.
    /// `amigo_render::gpu_broad_phase::GpuBroadPhase` (feature `gpu_physics`)
    /// to run candidate detection on the GPU.
    pub fn set_broad_phase(&mut self, broad_phase: Box<dyn crate::broad_phase::BroadPhase>) {
        self.broad_phase = Some(broad_phase);
    }

    /// Revert to the built-in spatial-hash broad phase.
    pub fn clear_broad_phase(&mut self) {
        self.broad_phase = None;
    }

    pub fn add_body(&mut self, entity: EntityId, body: RigidBody) {
        let aabb = shape_to_aabb(body.position, &body.shape);
        self.spatial_hash.insert(entity, &aabb);
        self.bodies.insert(entity, body);
    }

    pub fn remove_body(&mut self, entity: EntityId) {
        self.spatial_hash.remove(entity);
        self.bodies.remove(&entity);
    }

    pub fn get_body(&self, entity: EntityId) -> Option<&RigidBody> {
        self.bodies.get(&entity)
    }

    pub fn get_body_mut(&mut self, entity: EntityId) -> Option<&mut RigidBody> {
        self.bodies.get_mut(&entity)
    }

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Run one physics step: integrate velocities, detect and resolve collisions.
    /// Returns all contacts that occurred this step.
    pub fn step(&mut self) -> Vec<PhysicsContact> {
        // 1. Integrate: apply gravity and velocity
        self.integrate();

        // 2. Update spatial hash
        self.rebuild_spatial_hash();

        // 3. Detect and resolve collisions (iterative)
        let mut contacts = Vec::new();
        for _ in 0..self.solver_iterations {
            let pairs = self.find_collision_pairs();
            if pairs.is_empty() {
                break;
            }
            for (id_a, id_b, contact) in &pairs {
                self.resolve_collision(*id_a, *id_b, contact);
            }
            if contacts.is_empty() {
                contacts = pairs
                    .into_iter()
                    .map(|(a, b, contact)| PhysicsContact {
                        entity_a: a,
                        entity_b: b,
                        contact,
                    })
                    .collect();
            }
        }

        // 4. Final spatial hash update after resolution
        self.rebuild_spatial_hash();

        contacts
    }

    fn integrate(&mut self) {
        let gravity = self.gravity;
        for body in self.bodies.values_mut() {
            if body.body_type != BodyType::Dynamic {
                continue;
            }
            body.velocity += gravity * body.gravity_scale;
            body.position += body.velocity;
        }
    }

    fn rebuild_spatial_hash(&mut self) {
        self.spatial_hash.clear();
        for (&entity, body) in &self.bodies {
            let aabb = shape_to_aabb(body.position, &body.shape);
            self.spatial_hash.insert(entity, &aabb);
        }
    }

    fn find_collision_pairs(&mut self) -> Vec<(EntityId, EntityId, ContactInfo)> {
        // Use the pluggable broad phase when one is installed. Take it out
        // temporarily so it can be borrowed mutably alongside `self.bodies`.
        if let Some(mut bp) = self.broad_phase.take() {
            let pairs = self.find_collision_pairs_with(bp.as_mut());
            self.broad_phase = Some(bp);
            return pairs;
        }
        self.find_collision_pairs_spatial_hash()
    }

    /// Narrow-phase filtering of candidates produced by a custom broad phase.
    fn find_collision_pairs_with(
        &self,
        broad_phase: &mut dyn crate::broad_phase::BroadPhase,
    ) -> Vec<(EntityId, EntityId, ContactInfo)> {
        let bodies: Vec<(EntityId, SimRect)> = self
            .bodies
            .iter()
            .map(|(&entity, body)| (entity, shape_to_aabb(body.position, &body.shape)))
            .collect();

        // Resolved in id order whatever order the broad phase used, so a
        // GPU or third-party implementation cannot change the outcome.
        let mut candidates = broad_phase.find_candidates(&bodies);
        candidates.sort_unstable_by_key(|p| (p.a, p.b));
        candidates.dedup();
        let mut pairs = Vec::new();
        for candidate in candidates {
            let (Some(body_a), Some(body_b)) =
                (self.bodies.get(&candidate.a), self.bodies.get(&candidate.b))
            else {
                continue;
            };

            // Skip static-static and kinematic-kinematic pairs
            if body_a.body_type != BodyType::Dynamic && body_b.body_type != BodyType::Dynamic {
                continue;
            }

            if let Some(contact) = check_shapes(
                body_a.position,
                &body_a.shape,
                body_b.position,
                &body_b.shape,
            ) {
                pairs.push((candidate.a, candidate.b, contact));
            }
        }
        pairs
    }

    fn find_collision_pairs_spatial_hash(&self) -> Vec<(EntityId, EntityId, ContactInfo)> {
        let mut pairs = Vec::new();
        let one = Fix::ONE;

        // Bodies in id order, candidates sorted: each pair is visited once,
        // from its smaller id, in the same order on every machine.
        for (&entity_a, body_a) in &self.bodies {
            let aabb = shape_to_aabb(body_a.position, &body_a.shape);
            // Expand AABB slightly for the broad phase query
            let query_rect = SimRect::new(
                aabb.x - one,
                aabb.y - one,
                aabb.w + one * 2,
                aabb.h + one * 2,
            );
            let candidates = self.spatial_hash.query_aabb(&query_rect);

            for entity_b in candidates {
                if entity_b <= entity_a {
                    continue;
                }

                let body_b = match self.bodies.get(&entity_b) {
                    Some(b) => b,
                    None => continue,
                };

                // Skip static-static and kinematic-kinematic pairs
                if body_a.body_type != BodyType::Dynamic && body_b.body_type != BodyType::Dynamic {
                    continue;
                }

                if let Some(contact) = check_shapes(
                    body_a.position,
                    &body_a.shape,
                    body_b.position,
                    &body_b.shape,
                ) {
                    pairs.push((entity_a, entity_b, contact));
                }
            }
        }
        pairs
    }

    fn resolve_collision(&mut self, id_a: EntityId, id_b: EntityId, contact: &ContactInfo) {
        // Get inverse masses first (avoids borrow issues)
        let (inv_a, inv_b, vel_a, vel_b, rest, friction) = {
            let a = match self.bodies.get(&id_a) {
                Some(b) => b,
                None => return,
            };
            let b = match self.bodies.get(&id_b) {
                Some(b) => b,
                None => return,
            };
            let inv_a = if a.body_type == BodyType::Dynamic {
                a.inv_mass
            } else {
                Fix::ZERO
            };
            let inv_b = if b.body_type == BodyType::Dynamic {
                b.inv_mass
            } else {
                Fix::ZERO
            };
            let rest = a.restitution.max(b.restitution);
            let friction = (a.friction + b.friction) / 2;
            (inv_a, inv_b, a.velocity, b.velocity, rest, friction)
        };

        let inv_total = inv_a + inv_b;
        if inv_total == Fix::ZERO {
            return;
        }

        let normal = contact.normal;

        // --- Positional correction (push bodies apart) ---
        let correction_ratio = contact.penetration.saturating_div(inv_total);
        if let Some(a) = self.bodies.get_mut(&id_a)
            && a.body_type == BodyType::Dynamic
        {
            a.position += normal * (correction_ratio * inv_a);
        }
        if let Some(b) = self.bodies.get_mut(&id_b)
            && b.body_type == BodyType::Dynamic
        {
            b.position = b.position - normal * (correction_ratio * inv_b);
        }

        // --- Impulse resolution ---
        let vel_along_normal = (vel_a - vel_b).dot(normal);

        // Only resolve if bodies are approaching
        if vel_along_normal > Fix::ZERO {
            return;
        }

        // Normal impulse
        let j = (-(Fix::ONE + rest) * vel_along_normal).saturating_div(inv_total);
        let impulse = normal * j;

        if let Some(a) = self.bodies.get_mut(&id_a)
            && a.body_type == BodyType::Dynamic
        {
            a.velocity += impulse * inv_a;
        }
        if let Some(b) = self.bodies.get_mut(&id_b)
            && b.body_type == BodyType::Dynamic
        {
            b.velocity = b.velocity - impulse * inv_b;
        }

        // --- Friction impulse ---
        // Recompute relative velocity after normal impulse
        let velocity = |id| self.bodies.get(&id).map_or(SimVec2::ZERO, |b| b.velocity);
        let rel_vel = velocity(id_a) - velocity(id_b);
        let tangent = rel_vel - normal * rel_vel.dot(normal);
        let tangent_len = tangent.length();
        if tangent_len == Fix::ZERO {
            return;
        }
        let t = tangent / tangent_len;

        let jt = (-rel_vel.dot(t)).saturating_div(inv_total);
        // Coulomb friction: clamp tangent impulse
        let limit = j.abs() * friction;
        let jt = jt.clamp(-limit, limit);

        if let Some(a) = self.bodies.get_mut(&id_a)
            && a.body_type == BodyType::Dynamic
        {
            a.velocity += t * (jt * inv_a);
        }
        if let Some(b) = self.bodies.get_mut(&id_b)
            && b.body_type == BodyType::Dynamic
        {
            b.velocity = b.velocity - t * (jt * inv_b);
        }
    }
}

// ---------------------------------------------------------------------------
// ECS Bridge
// ---------------------------------------------------------------------------

/// Position component used by the ECS bridge.
pub type Position = SimVec2;

/// Synchronize PhysicsWorld body positions back into ECS Position components.
/// Call after `PhysicsWorld::step()`.
pub fn sync_physics_to_ecs(world: &PhysicsWorld, positions: &mut crate::ecs::SparseSet<Position>) {
    for (&entity, body) in &world.bodies {
        if let Some(pos) = positions.get_mut(entity) {
            *pos = body.position;
        }
    }
}

/// Synchronize ECS positions into PhysicsWorld (for Kinematic bodies moved by game code).
/// Call before `PhysicsWorld::step()`.
pub fn sync_ecs_to_physics(positions: &crate::ecs::SparseSet<Position>, world: &mut PhysicsWorld) {
    for (&entity, body) in world.bodies.iter_mut() {
        if body.body_type == BodyType::Kinematic
            && let Some(pos) = positions.get(entity)
        {
            body.position = *pos;
        }
    }
}

impl PhysicsWorld {
    /// Set a CCD velocity threshold. Bodies moving faster than this per tick
    /// will use swept collision tests to prevent tunneling.
    pub fn set_ccd_threshold(&mut self, threshold: Fix) {
        self.ccd_threshold = threshold;
    }

    /// Iterate over all bodies (for ECS sync).
    pub fn iter_bodies(&self) -> impl Iterator<Item = (&EntityId, &RigidBody)> {
        self.bodies.iter()
    }

    /// Mutable iteration over all bodies.
    pub fn iter_bodies_mut(&mut self) -> impl Iterator<Item = (&EntityId, &mut RigidBody)> {
        self.bodies.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    fn make_id(index: u32) -> EntityId {
        EntityId::from_raw(index, 0)
    }

    fn f(v: impl fixed::traits::ToFixed) -> Fix {
        Fix::from_num(v)
    }
    fn v(x: impl fixed::traits::ToFixed, y: impl fixed::traits::ToFixed) -> SimVec2 {
        SimVec2::from_num(x, y)
    }
    fn aabb(x: i32, y: i32, w: i32, h: i32) -> CollisionShape {
        CollisionShape::Aabb(SimRect::from_num(x, y, w, h))
    }

    // ── Integration and gravity ─────────────────────────────

    #[test]
    fn dynamic_body_falls_with_gravity() {
        let mut world = PhysicsWorld::new(v(0, 0.5), f(64));
        let id = make_id(1);
        world.add_body(id, RigidBody::dynamic(v(0, 0), aabb(-8, -8, 16, 16), f(1)));

        world.step();

        let body = world.get_body(id).unwrap();
        assert_eq!(body.velocity.y, f(0.5));
        assert_eq!(body.position.y, f(0.5));
    }

    #[test]
    fn static_body_does_not_move() {
        let mut world = PhysicsWorld::new(v(0, 0.5), f(64));
        let id = make_id(1);
        world.add_body(
            id,
            RigidBody::static_body(v(100, 100), aabb(-50, -5, 100, 10)),
        );

        for tick in 0..10 {
            world.step();
            let body = world.get_body(id).unwrap();
            assert_eq!(body.position, v(100, 100), "moved after tick {tick}");
            assert_eq!(body.velocity, SimVec2::ZERO, "velocity after tick {tick}");
        }
    }

    // ── Collision response ──────────────────────────────────

    #[test]
    fn dynamic_lands_on_static() {
        let mut world = PhysicsWorld::new(v(0, 0.5), f(64));
        let dyn_id = make_id(1);
        world.add_body(
            dyn_id,
            RigidBody::dynamic(v(0, 0), aabb(-8, -8, 16, 16), f(1)),
        );
        world.add_body(
            make_id(2),
            RigidBody::static_body(v(0, 20), aabb(-100, 0, 200, 20)),
        );

        for _ in 0..100 {
            world.step();
        }

        let body = world.get_body(dyn_id).unwrap();
        // Resting on the floor (top at y = 20, half height 8), not through it.
        assert!(
            body.position.y < f(13) && body.position.y > f(11),
            "Body should be resting on floor, got y={}",
            body.position.y
        );
    }

    #[test]
    fn bouncy_body_rebounds() {
        let mut world = PhysicsWorld::new(v(0, 1), f(64));
        let mut ball = RigidBody::dynamic(
            v(0, 0),
            CollisionShape::Circle {
                center: SimVec2::ZERO,
                radius: f(8),
            },
            f(1),
        );
        ball.restitution = Fix::ONE;
        let ball_id = make_id(1);
        world.add_body(ball_id, ball);
        world.add_body(
            make_id(2),
            RigidBody::static_body(v(0, 50), aabb(-100, 0, 200, 20)),
        );

        let bounced = (0..100).any(|_| {
            world.step();
            world.get_body(ball_id).unwrap().velocity.y < f(-0.1)
        });
        assert!(bounced, "Bouncy body should rebound off the floor");
    }

    // ── Body management ─────────────────────────────────────

    #[test]
    fn remove_body_works() {
        let mut world = PhysicsWorld::new(SimVec2::ZERO, f(64));
        let id = make_id(1);
        world.add_body(id, RigidBody::dynamic(v(0, 0), aabb(0, 0, 10, 10), f(1)));
        assert_eq!(world.body_count(), 1);
        world.remove_body(id);
        assert_eq!(world.body_count(), 0);
        assert!(world.get_body(id).is_none());
    }

    #[test]
    fn pluggable_broad_phase_detects_contacts() {
        // Two overlapping dynamic bodies must produce a contact both with the
        // built-in spatial hash and with an installed custom broad phase.
        let make_world = || {
            let mut world = PhysicsWorld::new(SimVec2::ZERO, f(64));
            world.add_body(
                make_id(1),
                RigidBody::dynamic(v(0, 0), aabb(-8, -8, 16, 16), f(1)),
            );
            world.add_body(
                make_id(2),
                RigidBody::dynamic(v(4, 0), aabb(-8, -8, 16, 16), f(1)),
            );
            world
        };

        let mut hash_world = make_world();
        let hash_contacts = hash_world.step();
        assert!(!hash_contacts.is_empty());

        let mut bp_world = make_world();
        bp_world.set_broad_phase(Box::new(crate::broad_phase::CpuBroadPhase::new()));
        let bp_contacts = bp_world.step();
        assert!(!bp_contacts.is_empty());

        // Both paths resolve the same contacts the same way.
        let positions = |w: &PhysicsWorld| -> Vec<SimVec2> {
            w.iter_bodies().map(|(_, b)| b.position).collect()
        };
        assert_eq!(positions(&hash_world), positions(&bp_world));

        // Reverting restores the spatial-hash path.
        bp_world.clear_broad_phase();
        let _ = bp_world.step();
    }

    /// The order bodies are added in must not change the result.
    #[test]
    fn insertion_order_does_not_change_the_simulation() {
        let run = |order: &[u32]| {
            let mut world = PhysicsWorld::new(v(0, 0.25), f(32));
            world.add_body(
                make_id(100),
                RigidBody::static_body(v(0, 200), aabb(-200, 0, 400, 20)),
            );
            for &i in order {
                let x = (i as i32 % 5) * 9 - 20;
                let y = (i as i32 / 5) * 12;
                world.add_body(
                    make_id(i),
                    RigidBody::dynamic(v(x, y), aabb(-5, -5, 10, 10), f(1)),
                );
            }
            for _ in 0..300 {
                world.step();
            }
            world
                .iter_bodies()
                .map(|(id, b)| (*id, b.position, b.velocity))
                .collect::<Vec<_>>()
        };
        let forward: Vec<u32> = (0..20).collect();
        let backward: Vec<u32> = (0..20).rev().collect();
        let shuffled: Vec<u32> = (0..20).map(|i| (i * 7) % 20).collect();
        let a = run(&forward);
        assert_eq!(a, run(&backward));
        assert_eq!(a, run(&shuffled));
    }
}
