---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-10-05
---

# Physics (Rigid Body)

> **Fixed point since 0.2.0.** Positions, velocities, shapes and contacts are
> `Fix`/`SimVec2`/`SimRect` (ADR-0001); they were `f32` before. Bodies are
> iterated in `EntityId` order, spatial-hash queries return sorted ids, and
> contact pairs are resolved in sorted order whichever broad phase found them,
> so a step gives the same bits on every machine.
> `crates/amigo_core/tests/determinism.rs` pins a 100-body, 600-tick golden
> hash on Linux, Windows and macOS.

## Purpose

2D rigid body physics with gravity, impulse-based collision resolution, and spatial hashing for broad-phase acceleration. Custom implementation (no external physics engine) — consistent with the engine's Fixed-Point ecosystem, tile-based collision layer, and minimal-dependency philosophy.

Existing implementation in `crates/amigo_core/src/physics.rs` and `crates/amigo_core/src/collision.rs`.

## Existierende Bausteine

Kernphysik ist bereits implementiert (RigidBody, PhysicsWorld, SpatialHash, CollisionWorld, TriggerZone). Fehlende Features: Capsule Collider, Joints, CCD, Convex Polygon, ECS-Bridge.

## Public API

### CollisionShape

```rust
#[derive(Clone, Copy, Debug)]
pub enum CollisionShape {
    Aabb(SimRect),
    Circle { center: SimVec2, radius: Fix },
}
```

Shapes are defined relative to the body's position. `Aabb` uses a `SimRect` (x, y, w, h) as offset from position. `Circle` uses `center` as offset and `radius`.

### ContactInfo

```rust
#[derive(Clone, Copy, Debug)]
pub struct ContactInfo {
    pub penetration: Fix,
    pub normal: SimVec2,
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
    pub position: SimVec2,
    pub velocity: SimVec2,
    pub shape: CollisionShape,
    pub mass: Fix,
    pub restitution: Fix,   // 0.0 = no bounce, 1.0 = perfectly elastic
    pub friction: Fix,       // Tangential velocity damping
    pub gravity_scale: Fix,  // Per-body gravity multiplier (0.0 = no gravity)
}

impl RigidBody {
    pub fn dynamic(position: SimVec2, shape: CollisionShape, mass: Fix) -> Self;
    pub fn static_body(position: SimVec2, shape: CollisionShape) -> Self;
    pub fn kinematic(position: SimVec2, shape: CollisionShape) -> Self;
    pub fn inverse_mass(&self) -> Fix;
    pub fn set_mass(&mut self, mass: Fix);
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
    pub gravity: SimVec2,
    pub solver_iterations: u32,  // default: 4
}

impl PhysicsWorld {
    pub fn new(gravity: SimVec2, cell_size: Fix) -> Self;
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
    pub fn new(cell_size: Fix) -> Self;
    pub fn insert(&mut self, id: EntityId, aabb: &SimRect);
    pub fn remove(&mut self, id: EntityId);
    pub fn clear(&mut self);
    pub fn query_aabb(&self, aabb: &SimRect) -> Vec<EntityId>;
    pub fn query_point(&self, p: SimVec2) -> Vec<EntityId>;
    pub fn query_circle(&self, center: SimVec2, radius: Fix) -> Vec<EntityId>;
    pub fn cell_count(&self) -> usize;
    pub fn entity_count(&self) -> usize;
}
```

### CollisionWorld

```rust
pub struct CollisionWorld { /* spatial_hash, shapes, triggers */ }

impl CollisionWorld {
    pub fn new(cell_size: Fix) -> Self;
    pub fn update_entity(&mut self, id: EntityId, pos: SimVec2, shape: CollisionShape);
    pub fn remove_entity(&mut self, id: EntityId);
    pub fn query_aabb(&self, rect: &SimRect) -> Vec<EntityId>;
    pub fn query_point(&self, p: SimVec2) -> Vec<EntityId>;
    pub fn query_circle(&self, center: SimVec2, radius: Fix) -> Vec<EntityId>;
    pub fn check_pair(&self, a: EntityId, b: EntityId) -> Option<ContactInfo>;
    pub fn check_triggers(&mut self, entity: EntityId) -> Vec<TriggerEvent>;
    pub fn clear(&mut self);
}
```

### TriggerZone

```rust
pub struct TriggerZone {
    pub id: u32,
    pub rect: SimRect,
    pub active: bool,
}

pub enum TriggerEvent {
    Enter { zone_id: u32, entity: EntityId },
    Exit { zone_id: u32, entity: EntityId },
}

impl TriggerZone {
    pub fn new(id: u32, rect: SimRect) -> Self;
    pub fn check(&mut self, entity: EntityId, entity_rect: &SimRect) -> Option<TriggerEvent>;
    pub fn remove_entity(&mut self, entity: EntityId);
}
```

### Narrow-Phase Functions

```rust
pub fn aabb_vs_aabb(a: &SimRect, b: &SimRect) -> Option<ContactInfo>;
pub fn circle_vs_circle(a: SimVec2, ar: Fix, b: SimVec2, br: Fix) -> Option<ContactInfo>;
pub fn circle_vs_aabb(center: SimVec2, radius: Fix, rect: &SimRect) -> Option<ContactInfo>;
pub fn check_shapes(pos_a: SimVec2, shape_a: &CollisionShape, pos_b: SimVec2, shape_b: &CollisionShape) -> Option<ContactInfo>;
pub fn shape_to_aabb(pos: SimVec2, shape: &CollisionShape) -> SimRect;
```

### Raycast API

Raycasts are required by platformer controllers (ground/wall detection), shmup line-of-sight, and RTS vision queries. Both tile-based and body-based raycasts are provided.

```rust
/// Result of a raycast hit.
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    /// World position where the ray hit.
    pub point: SimVec2,
    /// Surface normal at the hit point.
    pub normal: SimVec2,
    /// Distance from ray origin to hit point.
    pub distance: Fix,
    /// Entity that was hit (None for tilemap hits).
    pub entity: Option<EntityId>,
}

/// Cast a ray against the tilemap collision layer.
/// Returns the first solid tile hit. `max_distance` limits the ray length.
/// Uses DDA (Digital Differential Analyzer) for tile traversal — O(tiles traversed).
pub fn raycast_tiles(
    origin: SimVec2,
    direction: SimVec2,
    max_distance: Fix,
    collision_layer: &CollisionLayer,
    tile_size: Fix,
) -> Option<RayHit>;

/// Cast a ray against all bodies in the CollisionWorld.
/// Returns the closest hit. Uses SpatialHash for broad-phase acceleration.
pub fn raycast_bodies(
    origin: SimVec2,
    direction: SimVec2,
    max_distance: Fix,
    world: &CollisionWorld,
    exclude: Option<EntityId>,
) -> Option<RayHit>;

/// Cast a ray against both tilemap and bodies, returning the closest overall hit.
pub fn raycast(
    origin: SimVec2,
    direction: SimVec2,
    max_distance: Fix,
    collision_layer: &CollisionLayer,
    tile_size: Fix,
    world: &CollisionWorld,
    exclude: Option<EntityId>,
) -> Option<RayHit>;

/// Short-range directional sensor (convenience for platformer controllers).
/// Equivalent to `raycast_tiles(origin, dir, distance, ...)`.
pub fn sensor(
    origin: SimVec2,
    direction: SimVec2,
    distance: Fix,
    collision_layer: &CollisionLayer,
    tile_size: Fix,
) -> bool;
```

**Platformer usage:**
- Ground detection: `sensor(feet_pos, DOWN, 1.0, ...)` returns `true` if solid tile 1px below.
- Wall detection: `sensor(side_pos, LEFT/RIGHT, 1.0, ...)` for wall contact.
- OneWay pass-through: `raycast_tiles` reports `CollisionType::OneWay` in the hit — controller ignores if moving upward.
- Slope detection: `raycast_tiles` returns the exact hit point on slopes via interpolation of `Slope { left_height, right_height }`.

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

### Capsule Collider (nicht implementiert)

```rust
/// Capsule = Liniensegment + Radius. Ideal für längliche Entities (Enemies in TD).
/// Gleitet an Ecken ab wie ein Circle, aber deckt längliche Formen ab.
#[derive(Clone, Copy, Debug)]
pub struct CapsuleShape {
    /// Halbe Länge des Liniensegments (Gesamtlänge = 2 * half_length).
    pub half_length: Fix,
    /// Radius an beiden Enden.
    pub radius: Fix,
    /// Rotation in Radians (0 = horizontal).
    pub angle: Fix,
}

// CollisionShape erweitert um:
pub enum CollisionShape {
    Aabb(SimRect),
    Circle { center: SimVec2, radius: Fix },
    Capsule(CapsuleShape),  // NEU
}

// Neue Narrow-Phase Funktionen:
pub fn capsule_vs_aabb(capsule_pos: SimVec2, capsule: &CapsuleShape, rect: &SimRect) -> Option<ContactInfo>;
pub fn capsule_vs_circle(capsule_pos: SimVec2, capsule: &CapsuleShape, center: SimVec2, r: Fix) -> Option<ContactInfo>;
pub fn capsule_vs_capsule(pos_a: SimVec2, a: &CapsuleShape, pos_b: SimVec2, b: &CapsuleShape) -> Option<ContactInfo>;
```

Implementierung: Punkt-zu-Liniensegment Distanz, dann wie Circle-Kollision behandeln. ~100 Zeilen.

### Joint Constraints (nicht implementiert)

```rust
/// Joint-Typen für Verbindungen zwischen zwei Bodies.
pub enum JointType {
    /// Drehgelenk: Bodies verbunden an einem Punkt, frei rotierbar.
    Revolute { anchor_a: SimVec2, anchor_b: SimVec2 },
    /// Schiene: Body B kann nur entlang einer Achse relativ zu A gleiten.
    Prismatic { axis: SimVec2, anchor_a: SimVec2, anchor_b: SimVec2 },
    /// Fest verbunden: Bodies bewegen sich als Einheit.
    Fixed { anchor_a: SimVec2, anchor_b: SimVec2 },
    /// Distanz-Joint: hält Bodies in festem Abstand.
    Distance { anchor_a: SimVec2, anchor_b: SimVec2, length: Fix },
}

pub struct Joint {
    pub joint_type: JointType,
    pub entity_a: EntityId,
    pub entity_b: EntityId,
    pub stiffness: Fix,  // 0.0 = weich, 1.0 = starr
}

impl PhysicsWorld {
    pub fn add_joint(&mut self, joint: Joint) -> JointId;
    pub fn remove_joint(&mut self, id: JointId);
}
```

Implementierung: Positional Correction pro Constraint-Typ im Solver-Loop. ~200-300 Zeilen pro Joint-Typ.

### CCD — Continuous Collision Detection (nicht implementiert)

```rust
/// Swept collision test für schnelle Objekte.
/// Verhindert Tunneling durch dünne Wände.
pub fn swept_aabb(
    pos: SimVec2,
    velocity: SimVec2,
    shape: &CollisionShape,
    obstacle: &SimRect,
) -> Option<SweptContact>;

pub struct SweptContact {
    pub time: Fix,       // 0.0..1.0, wann im Tick die Kollision auftritt
    pub normal: SimVec2,
    pub contact: ContactInfo,
}

impl PhysicsWorld {
    /// Aktiviert CCD für Bodies mit Geschwindigkeit > threshold.
    pub fn set_ccd_threshold(&mut self, threshold: Fix);
}
```

Implementierung: Swept AABB via Minkowski-Differenz oder Raycasting. ~150 Zeilen.

### ECS-Bridge (nicht implementiert)

```rust
/// Synchronisiert PhysicsWorld-Positionen zurück ins ECS.
pub fn sync_physics_to_ecs(world: &PhysicsWorld, positions: &mut SparseSet<Position>);
/// Synchronisiert ECS-Positionen in die PhysicsWorld (für Kinematic bodies).
pub fn sync_ecs_to_physics(positions: &SparseSet<Position>, world: &mut PhysicsWorld);
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

- **Fixed point (since 0.2.0):** Physics, collision, raycasts and broad phase compute in `Fix`/`SimVec2`/`SimRect`, like the rest of the simulation (ADR-0001). The earlier decision to keep rigid-body physics in `f32` as "visual only" is withdrawn: games used it for gameplay, and `f32` results (and libm calls) are not guaranteed identical across platforms, which breaks replays and lockstep.
- **Simulation → Render:** `SimVec2::to_render()`, `SimRect::to_render()` at draw time. **Render → Simulation:** `SimVec2::from_num(..)` only at system boundaries (input, editor), never per frame back and forth.
- **Order:** bodies live in a `BTreeMap<EntityId, RigidBody>`, so every pass visits them in id order. Spatial-hash queries return ids sorted and deduplicated; pairs from a pluggable broad phase are sorted before resolution. Each pair is checked once, from its smaller id.
- **Overflow:** squared distances and segment projections are computed on widened raw bits (`i128`) or via the exact `SimVec2::length`, since `x * x` leaves the Q16.16 range above ~181 units. Divisions that can blow up (swept AABB, ray slabs) saturate.
- **Multiplayer:** `PhysicsWorld::step()` gives bit-identical results everywhere; `crates/amigo_core/tests/determinism.rs` pins a 100-body, 600-tick golden hash on all CI platforms. Gameplay that depends on physics can therefore run in lockstep or be replayed.
- Spatial hash cell size should match typical entity size (64px recommended for most games).

## Non-Goals

- **Rapier2D integration.** Custom-Implementierung deckt alle Anwendungsfälle ab. Rapier2D würde nalgebra, parry2d, simba als Dependencies hinzufügen. Kann als optionales Feature-Flag revisited werden.
- **Convex polygon collider.** SAT-basierte Polygon-Kollision. Für Tile-basierte Genres nicht nötig.
- **3D physics.** Die Engine ist 2D-only.

## Open Questions

- Soll `solver_iterations` per-Szene konfigurierbar sein, oder reichen 4 Iterationen global?
- Soll CCD automatisch für Bodies über einer Geschwindigkeitsschwelle aktiviert werden, oder explizit per Body?
- Braucht es einen `ConvexHull` Collider zusätzlich zu Capsule, oder reicht Capsule + Circle + AABB?

## Referenzen

- [engine/core](core.md) → RenderVec2, EntityId, Rect
- [engine/memory-performance](memory-performance.md) → SpatialHash broad-phase
- [engine/tilemap](tilemap.md) → CollisionLayer, CollisionType
- [gametypes/platformer](../gametypes/platformer.md) → Hauptabnehmer
