use crate::collision::{check_shapes, shape_to_aabb, CollisionShape, ContactInfo, SpatialHash};
use crate::ecs::EntityId;
use crate::math::RenderVec2;
use crate::rect::Rect;
use rustc_hash::FxHashMap;

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
    pub position: RenderVec2,
    pub velocity: RenderVec2,
    pub shape: CollisionShape,
    /// Mass in arbitrary units. Ignored for Static/Kinematic bodies.
    pub mass: f32,
    inv_mass: f32,
    /// Bounciness. 0.0 = no bounce, 1.0 = perfectly elastic.
    pub restitution: f32,
    /// Friction coefficient for tangential velocity damping.
    pub friction: f32,
    /// Multiplier on gravity for this body. 0.0 = no gravity.
    pub gravity_scale: f32,
}

impl RigidBody {
    pub fn dynamic(position: RenderVec2, shape: CollisionShape, mass: f32) -> Self {
        assert!(mass > 0.0, "Dynamic body must have positive mass");
        Self {
            body_type: BodyType::Dynamic,
            position,
            velocity: RenderVec2::new(0.0, 0.0),
            shape,
            mass,
            inv_mass: 1.0 / mass,
            restitution: 0.0,
            friction: 0.2,
            gravity_scale: 1.0,
        }
    }

    pub fn static_body(position: RenderVec2, shape: CollisionShape) -> Self {
        Self {
            body_type: BodyType::Static,
            position,
            velocity: RenderVec2::new(0.0, 0.0),
            shape,
            mass: 0.0,
            inv_mass: 0.0,
            restitution: 0.0,
            friction: 0.5,
            gravity_scale: 0.0,
        }
    }

    pub fn kinematic(position: RenderVec2, shape: CollisionShape) -> Self {
        Self {
            body_type: BodyType::Kinematic,
            position,
            velocity: RenderVec2::new(0.0, 0.0),
            shape,
            mass: 0.0,
            inv_mass: 0.0,
            restitution: 0.0,
            friction: 0.5,
            gravity_scale: 0.0,
        }
    }

    pub fn inverse_mass(&self) -> f32 {
        self.inv_mass
    }

    pub fn set_mass(&mut self, mass: f32) {
        assert!(mass > 0.0);
        self.mass = mass;
        self.inv_mass = 1.0 / mass;
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
    /// Gravity in pixels/tick². Typically (0.0, positive_value) for downward gravity.
    pub gravity: RenderVec2,
    /// Number of iterations for constraint/collision solving per step.
    pub solver_iterations: u32,
    bodies: FxHashMap<EntityId, RigidBody>,
    spatial_hash: SpatialHash,
}

impl PhysicsWorld {
    pub fn new(gravity: RenderVec2, cell_size: f32) -> Self {
        Self {
            gravity,
            solver_iterations: 4,
            bodies: FxHashMap::default(),
            spatial_hash: SpatialHash::new(cell_size),
        }
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
            body.velocity.x += gravity.x * body.gravity_scale;
            body.velocity.y += gravity.y * body.gravity_scale;
            body.position.x += body.velocity.x;
            body.position.y += body.velocity.y;
        }
    }

    fn rebuild_spatial_hash(&mut self) {
        self.spatial_hash.clear();
        for (&entity, body) in &self.bodies {
            let aabb = shape_to_aabb(body.position, &body.shape);
            self.spatial_hash.insert(entity, &aabb);
        }
    }

    fn find_collision_pairs(&self) -> Vec<(EntityId, EntityId, ContactInfo)> {
        let mut pairs = Vec::new();
        let mut checked = rustc_hash::FxHashSet::default();

        for (&entity_a, body_a) in &self.bodies {
            let aabb = shape_to_aabb(body_a.position, &body_a.shape);
            // Expand AABB slightly for the broad phase query
            let query_rect = Rect::new(aabb.x - 1.0, aabb.y - 1.0, aabb.w + 2.0, aabb.h + 2.0);
            let candidates = self.spatial_hash.query_aabb(&query_rect);

            for entity_b in candidates {
                if entity_a == entity_b {
                    continue;
                }
                // Canonical pair ordering to avoid duplicate checks
                let pair = if entity_a < entity_b {
                    (entity_a, entity_b)
                } else {
                    (entity_b, entity_a)
                };
                if !checked.insert(pair) {
                    continue;
                }

                let body_b = match self.bodies.get(&entity_b) {
                    Some(b) => b,
                    None => continue,
                };

                // Skip static-static and kinematic-kinematic pairs
                if body_a.body_type != BodyType::Dynamic
                    && body_b.body_type != BodyType::Dynamic
                {
                    continue;
                }

                if let Some(contact) =
                    check_shapes(body_a.position, &body_a.shape, body_b.position, &body_b.shape)
                {
                    pairs.push((entity_a, entity_b, contact));
                }
            }
        }
        pairs
    }

    fn resolve_collision(
        &mut self,
        id_a: EntityId,
        id_b: EntityId,
        contact: &ContactInfo,
    ) {
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
            let inv_a = if a.body_type == BodyType::Dynamic { a.inv_mass } else { 0.0 };
            let inv_b = if b.body_type == BodyType::Dynamic { b.inv_mass } else { 0.0 };
            let rest = a.restitution.max(b.restitution);
            let friction = (a.friction + b.friction) * 0.5;
            (inv_a, inv_b, a.velocity, b.velocity, rest, friction)
        };

        let inv_total = inv_a + inv_b;
        if inv_total == 0.0 {
            return;
        }

        let normal = contact.normal;

        // --- Positional correction (push bodies apart) ---
        let correction_ratio = contact.penetration / inv_total;
        if let Some(a) = self.bodies.get_mut(&id_a) {
            if a.body_type == BodyType::Dynamic {
                a.position.x += normal.x * correction_ratio * inv_a;
                a.position.y += normal.y * correction_ratio * inv_a;
            }
        }
        if let Some(b) = self.bodies.get_mut(&id_b) {
            if b.body_type == BodyType::Dynamic {
                b.position.x -= normal.x * correction_ratio * inv_b;
                b.position.y -= normal.y * correction_ratio * inv_b;
            }
        }

        // --- Impulse resolution ---
        let rel_vel = RenderVec2::new(vel_a.x - vel_b.x, vel_a.y - vel_b.y);
        let vel_along_normal = rel_vel.x * normal.x + rel_vel.y * normal.y;

        // Only resolve if bodies are approaching
        if vel_along_normal > 0.0 {
            return;
        }

        // Normal impulse
        let j = -(1.0 + rest) * vel_along_normal / inv_total;
        let impulse_x = j * normal.x;
        let impulse_y = j * normal.y;

        if let Some(a) = self.bodies.get_mut(&id_a) {
            if a.body_type == BodyType::Dynamic {
                a.velocity.x += impulse_x * inv_a;
                a.velocity.y += impulse_y * inv_a;
            }
        }
        if let Some(b) = self.bodies.get_mut(&id_b) {
            if b.body_type == BodyType::Dynamic {
                b.velocity.x -= impulse_x * inv_b;
                b.velocity.y -= impulse_y * inv_b;
            }
        }

        // --- Friction impulse ---
        // Recompute relative velocity after normal impulse
        let vel_a = self.bodies.get(&id_a).map(|b| b.velocity).unwrap_or(RenderVec2::new(0.0, 0.0));
        let vel_b = self.bodies.get(&id_b).map(|b| b.velocity).unwrap_or(RenderVec2::new(0.0, 0.0));
        let rel_vel = RenderVec2::new(vel_a.x - vel_b.x, vel_a.y - vel_b.y);
        let vel_along_normal = rel_vel.x * normal.x + rel_vel.y * normal.y;
        let tangent_x = rel_vel.x - vel_along_normal * normal.x;
        let tangent_y = rel_vel.y - vel_along_normal * normal.y;
        let tangent_len = (tangent_x * tangent_x + tangent_y * tangent_y).sqrt();
        if tangent_len < 0.0001 {
            return;
        }
        let tx = tangent_x / tangent_len;
        let ty = tangent_y / tangent_len;

        let jt = -(rel_vel.x * tx + rel_vel.y * ty) / inv_total;
        // Coulomb friction: clamp tangent impulse
        let jt = jt.clamp(-j.abs() * friction, j.abs() * friction);

        if let Some(a) = self.bodies.get_mut(&id_a) {
            if a.body_type == BodyType::Dynamic {
                a.velocity.x += jt * tx * inv_a;
                a.velocity.y += jt * ty * inv_a;
            }
        }
        if let Some(b) = self.bodies.get_mut(&id_b) {
            if b.body_type == BodyType::Dynamic {
                b.velocity.x -= jt * tx * inv_b;
                b.velocity.y -= jt * ty * inv_b;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;
    use crate::rect::Rect;

    fn make_id(index: u32) -> EntityId {
        EntityId::from_raw(index, 0)
    }

    #[test]
    fn dynamic_body_falls_with_gravity() {
        let mut world = PhysicsWorld::new(RenderVec2::new(0.0, 0.5), 64.0);
        let body = RigidBody::dynamic(
            RenderVec2::new(0.0, 0.0),
            CollisionShape::Aabb(Rect::new(-8.0, -8.0, 16.0, 16.0)),
            1.0,
        );
        let id = make_id(1);
        world.add_body(id, body);

        world.step();

        let body = world.get_body(id).unwrap();
        assert!(body.position.y > 0.0, "Body should have fallen");
        assert!(body.velocity.y > 0.0, "Body should have downward velocity");
    }

    #[test]
    fn static_body_does_not_move() {
        let mut world = PhysicsWorld::new(RenderVec2::new(0.0, 0.5), 64.0);
        let body = RigidBody::static_body(
            RenderVec2::new(100.0, 100.0),
            CollisionShape::Aabb(Rect::new(-50.0, -5.0, 100.0, 10.0)),
        );
        let id = make_id(1);
        world.add_body(id, body);

        world.step();

        let body = world.get_body(id).unwrap();
        assert_eq!(body.position.x, 100.0);
        assert_eq!(body.position.y, 100.0);
    }

    #[test]
    fn dynamic_lands_on_static() {
        let mut world = PhysicsWorld::new(RenderVec2::new(0.0, 0.5), 64.0);

        // Dynamic body above
        let mut dyn_body = RigidBody::dynamic(
            RenderVec2::new(0.0, 0.0),
            CollisionShape::Aabb(Rect::new(-8.0, -8.0, 16.0, 16.0)),
            1.0,
        );
        dyn_body.restitution = 0.0;
        let dyn_id = make_id(1);
        world.add_body(dyn_id, dyn_body);

        // Static floor
        let floor = RigidBody::static_body(
            RenderVec2::new(0.0, 20.0),
            CollisionShape::Aabb(Rect::new(-100.0, 0.0, 200.0, 20.0)),
        );
        let floor_id = make_id(2);
        world.add_body(floor_id, floor);

        // Run enough steps for the dynamic body to hit the floor
        for _ in 0..100 {
            world.step();
        }

        let body = world.get_body(dyn_id).unwrap();
        // Body should rest on or near the floor, not fall through
        assert!(body.position.y < 25.0, "Body should be resting on floor, got y={}", body.position.y);
    }

    #[test]
    fn bouncy_body_rebounds() {
        let mut world = PhysicsWorld::new(RenderVec2::new(0.0, 1.0), 64.0);

        let mut ball = RigidBody::dynamic(
            RenderVec2::new(0.0, 0.0),
            CollisionShape::Circle { cx: 0.0, cy: 0.0, radius: 8.0 },
            1.0,
        );
        ball.restitution = 1.0;
        let ball_id = make_id(1);
        world.add_body(ball_id, ball);

        let floor = RigidBody::static_body(
            RenderVec2::new(0.0, 50.0),
            CollisionShape::Aabb(Rect::new(-100.0, 0.0, 200.0, 20.0)),
        );
        let floor_id = make_id(2);
        world.add_body(floor_id, floor);

        // Let the ball fall and bounce
        let mut bounced = false;
        for _ in 0..100 {
            world.step();
            let body = world.get_body(ball_id).unwrap();
            if body.velocity.y < -0.1 {
                bounced = true;
                break;
            }
        }
        assert!(bounced, "Bouncy body should rebound off the floor");
    }

    #[test]
    fn remove_body_works() {
        let mut world = PhysicsWorld::new(RenderVec2::new(0.0, 0.0), 64.0);
        let id = make_id(1);
        world.add_body(
            id,
            RigidBody::dynamic(
                RenderVec2::new(0.0, 0.0),
                CollisionShape::Aabb(Rect::new(0.0, 0.0, 10.0, 10.0)),
                1.0,
            ),
        );
        assert_eq!(world.body_count(), 1);
        world.remove_body(id);
        assert_eq!(world.body_count(), 0);
        assert!(world.get_body(id).is_none());
    }
}
