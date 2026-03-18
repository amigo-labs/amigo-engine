---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Physics (Rigid Body)

## Purpose

2D rigid body physics with gravity, impulse-based collision resolution, and spatial hashing for broad-phase acceleration. Custom implementation (no external physics engine) — consistent with the engine's Fixed-Point ecosystem, tile-based collision layer, and minimal-dependency philosophy.

Existing implementation in `crates/amigo_core/src/physics.rs` and `crates/amigo_core/src/collision.rs`.

## Public API

### CollisionShape

```rust
#[derive(Clone, Copy, Debug)]
pub enum CollisionShape {
    Aabb(Rect),
    Circle { cx: f32, cy: f32, radius: f32 },
}
```

Shapes are defined relative to the body's position. `Aabb` uses `Rect` (x, y, w, h) as offset from position. `Circle` uses (cx, cy) as center offset and `radius`.

### ContactInfo

```rust
#[derive(Clone, Copy, Debug)]
pub struct ContactInfo {
    pub penetration: f32,
    pub normal: RenderVec2,
}
```

### BodyType

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyType {
    Static,     // Immovable, infinite mass. Walls, platforms, terrain.
    Dynamic,    // Fully simulated: gravity, velocity, collision response.
    Kinematic,  // Moved by code only. Affects dynamic bodies but is not affected by collisions.
}
```

### RigidBody

```rust
pub struct RigidBody {
    pub body_type: BodyType,
    pub position: RenderVec2,
    pub velocity: RenderVec2,
    pub shape: CollisionShape,
    pub mass: f32,
    pub restitution: f32,   // 0.0 = no bounce, 1.0 = perfectly elastic
    pub friction: f32,       // Tangential velocity damping
    pub gravity_scale: f32,  // Per-body gravity multiplier (0.0 = no gravity)
}

impl RigidBody {
    pub fn dynamic(position: RenderVec2, shape: CollisionShape, mass: f32) -> Self;
    pub fn static_body(position: RenderVec2, shape: CollisionShape) -> Self;
    pub fn kinematic(position: RenderVec2, shape: CollisionShape) -> Self;
    pub fn inverse_mass(&self) -> f32;
    pub fn set_mass(&mut self, mass: f32);
}
```

### PhysicsContact

```rust
pub struct PhysicsContact {
    pub entity_a: EntityId,
    pub entity_b: EntityId,
    pub contact: ContactInfo,
}
```

### PhysicsWorld

```rust
pub struct PhysicsWorld {
    pub gravity: RenderVec2,
    pub solver_iterations: u32,  // default: 4
}

impl PhysicsWorld {
    pub fn new(gravity: RenderVec2, cell_size: f32) -> Self;
    pub fn add_body(&mut self, entity: EntityId, body: RigidBody);
    pub fn remove_body(&mut self, entity: EntityId);
    pub fn get_body(&self, entity: EntityId) -> Option<&RigidBody>;
    pub fn get_body_mut(&mut self, entity: EntityId) -> Option<&mut RigidBody>;
    pub fn body_count(&self) -> usize;
    pub fn step(&mut self) -> Vec<PhysicsContact>;
}
```

### SpatialHash

```rust
pub struct SpatialHash { /* cell_size, inv_cell_size, cells, entity_cells */ }

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self;
    pub fn insert(&mut self, id: EntityId, aabb: &Rect);
    pub fn remove(&mut self, id: EntityId);
    pub fn clear(&mut self);
    pub fn query_aabb(&self, aabb: &Rect) -> Vec<EntityId>;
    pub fn query_point(&self, x: f32, y: f32) -> Vec<EntityId>;
    pub fn query_circle(&self, cx: f32, cy: f32, radius: f32) -> Vec<EntityId>;
    pub fn cell_count(&self) -> usize;
    pub fn entity_count(&self) -> usize;
}
```

### CollisionWorld

```rust
pub struct CollisionWorld { /* spatial_hash, shapes, triggers */ }

impl CollisionWorld {
    pub fn new(cell_size: f32) -> Self;
    pub fn update_entity(&mut self, id: EntityId, pos: RenderVec2, shape: CollisionShape);
    pub fn remove_entity(&mut self, id: EntityId);
    pub fn query_aabb(&self, rect: &Rect) -> Vec<EntityId>;
    pub fn query_point(&self, x: f32, y: f32) -> Vec<EntityId>;
    pub fn query_circle(&self, cx: f32, cy: f32, radius: f32) -> Vec<EntityId>;
    pub fn check_pair(&self, a: EntityId, b: EntityId) -> Option<ContactInfo>;
    pub fn check_triggers(&mut self, entity: EntityId) -> Vec<TriggerEvent>;
    pub fn clear(&mut self);
}
```

### TriggerZone

```rust
pub struct TriggerZone {
    pub id: u32,
    pub rect: Rect,
    pub active: bool,
}

pub enum TriggerEvent {
    Enter { zone_id: u32, entity: EntityId },
    Exit { zone_id: u32, entity: EntityId },
}

impl TriggerZone {
    pub fn new(id: u32, rect: Rect) -> Self;
    pub fn check(&mut self, entity: EntityId, entity_rect: &Rect) -> Option<TriggerEvent>;
    pub fn remove_entity(&mut self, entity: EntityId);
}
```

### Narrow-Phase Functions

```rust
pub fn aabb_vs_aabb(a: &Rect, b: &Rect) -> Option<ContactInfo>;
pub fn circle_vs_circle(ax: f32, ay: f32, ar: f32, bx: f32, by: f32, br: f32) -> Option<ContactInfo>;
pub fn circle_vs_aabb(cx: f32, cy: f32, radius: f32, rect: &Rect) -> Option<ContactInfo>;
pub fn check_shapes(pos_a: RenderVec2, shape_a: &CollisionShape, pos_b: RenderVec2, shape_b: &CollisionShape) -> Option<ContactInfo>;
pub fn shape_to_aabb(pos: RenderVec2, shape: &CollisionShape) -> Rect;
```

### Tilemap Collision Layer

Defined in `crates/amigo_tilemap/src/lib.rs`:

```rust
pub enum CollisionType {
    Empty,
    Solid,
    OneWay,
    Slope { left_height: u8, right_height: u8 },
    Trigger { id: u32 },
}

pub struct CollisionLayer {
    pub data: Vec<CollisionType>,
    pub width: u32,
    pub height: u32,
}

impl CollisionLayer {
    pub fn get(&self, x: i32, y: i32) -> CollisionType;
    pub fn is_solid(&self, x: i32, y: i32) -> bool;  // Out-of-bounds → Solid
}
```

## Behavior

- **Physics step** runs per fixed tick: (1) integrate gravity + velocity on Dynamic bodies, (2) rebuild spatial hash, (3) iterative collision detection + resolution (up to `solver_iterations` passes), (4) final spatial hash update. Returns all contacts from the first collision pass.
- **Collision resolution** uses positional correction (push apart proportional to inverse mass) followed by impulse-based velocity response. Restitution uses `max(a, b)`, friction uses `avg(a, b)`.
- **Coulomb friction**: tangential impulse is clamped to `|normal_impulse| * friction` to prevent unrealistic sliding.
- **Static-static and kinematic-kinematic pairs** are skipped entirely. At least one body must be Dynamic for collision response.
- **Broad phase**: `SpatialHash` with grid-based cell lookup. Each entity is inserted into all cells its AABB overlaps. Query expands AABB by 1px for safety margin. Canonical pair ordering prevents duplicate checks.
- **Tilemap collision**: Separate system. `CollisionLayer` provides per-tile collision types (Solid, OneWay, Slope, Trigger). Game code checks tile collision before/instead of physics bodies for tile-based movement.
- **Trigger zones**: Enter/exit event system. `TriggerZone` tracks which entities are inside and fires `Enter`/`Exit` events on state change. Inactive zones produce no events.

## Internal Design

- All physics uses `f32` (RenderVec2), not Fixed-Point. Physics is a visual-layer system — deterministic simulation uses tile-based collision in SimVec2 space.
- `FxHashMap<EntityId, RigidBody>` for O(1) body lookup.
- `FxHashSet` for duplicate pair elimination during broad phase.
- Spatial hash cell size should match typical entity size (64px recommended for most games).

## Non-Goals

- **Rapier2D integration.** The custom implementation covers all current use cases. Rapier2D would add nalgebra, parry2d, simba dependencies for features (CCD, complex joints) that tile-based 2D games rarely need. Can be revisited as optional feature flag if complex physics becomes necessary.
- **Joints/Constraints.** Revolute, prismatic, fixed joints are not implemented. Can be added incrementally (~200-300 lines per joint type) if specific games need them.
- **Capsule collider.** Not yet implemented. Useful for elongated enemies in Tower Defense (prevents getting stuck on tile corners). Implementation: line-segment + radius, collision reduces to point-to-segment distance check. ~100 lines.
- **CCD (Continuous Collision Detection).** Fast-moving objects could tunnel through thin walls. Not needed at pixel-art speeds, but can be added via swept AABB if required.
- **Convex polygon collider.** SAT-based. Not needed for current tile-based genres.

## Open Questions

- Should capsule collider be promoted from non-goal to implemented feature for TD enemy pathfinding?
- Should physics bodies optionally sync back to ECS `Position(SimVec2)` via a bridge system?
- Is the current 4-iteration solver sufficient, or should `solver_iterations` be configurable per-scene?

## Referenzen

- [engine/core](core.md) → RenderVec2, EntityId, Rect
- [engine/memory-performance](memory-performance.md) → SpatialHash broad-phase
- [engine/tilemap](tilemap.md) → CollisionLayer, CollisionType
- [gametypes/platformer](../gametypes/platformer.md) → Hauptabnehmer
