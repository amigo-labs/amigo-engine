---
status: spec
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Physics (Rigid Body)

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

### Raycast API

Raycasts are required by platformer controllers (ground/wall detection), shmup line-of-sight, and RTS vision queries. Both tile-based and body-based raycasts are provided.

```rust
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
}

/// Cast a ray against the tilemap collision layer.
/// Returns the first solid tile hit. `max_distance` limits the ray length.
/// Uses DDA (Digital Differential Analyzer) for tile traversal — O(tiles traversed).
pub fn raycast_tiles(
    origin: RenderVec2,
    direction: RenderVec2,
    max_distance: f32,
    collision_layer: &CollisionLayer,
    tile_size: f32,
) -> Option<RayHit>;

/// Cast a ray against all bodies in the CollisionWorld.
/// Returns the closest hit. Uses SpatialHash for broad-phase acceleration.
pub fn raycast_bodies(
    origin: RenderVec2,
    direction: RenderVec2,
    max_distance: f32,
    world: &CollisionWorld,
    exclude: Option<EntityId>,
) -> Option<RayHit>;

/// Cast a ray against both tilemap and bodies, returning the closest overall hit.
pub fn raycast(
    origin: RenderVec2,
    direction: RenderVec2,
    max_distance: f32,
    collision_layer: &CollisionLayer,
    tile_size: f32,
    world: &CollisionWorld,
    exclude: Option<EntityId>,
) -> Option<RayHit>;

/// Short-range directional sensor (convenience for platformer controllers).
/// Equivalent to `raycast_tiles(origin, dir, distance, ...)`.
pub fn sensor(
    origin: RenderVec2,
    direction: RenderVec2,
    distance: f32,
    collision_layer: &CollisionLayer,
    tile_size: f32,
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
    pub half_length: f32,
    /// Radius an beiden Enden.
    pub radius: f32,
    /// Rotation in Radians (0 = horizontal).
    pub angle: f32,
}

// CollisionShape erweitert um:
pub enum CollisionShape {
    Aabb(Rect),
    Circle { cx: f32, cy: f32, radius: f32 },
    Capsule(CapsuleShape),  // NEU
}

// Neue Narrow-Phase Funktionen:
pub fn capsule_vs_aabb(capsule_pos: RenderVec2, capsule: &CapsuleShape, rect: &Rect) -> Option<ContactInfo>;
pub fn capsule_vs_circle(capsule_pos: RenderVec2, capsule: &CapsuleShape, cx: f32, cy: f32, r: f32) -> Option<ContactInfo>;
pub fn capsule_vs_capsule(pos_a: RenderVec2, a: &CapsuleShape, pos_b: RenderVec2, b: &CapsuleShape) -> Option<ContactInfo>;
```

Implementierung: Punkt-zu-Liniensegment Distanz, dann wie Circle-Kollision behandeln. ~100 Zeilen.

### Joint Constraints (nicht implementiert)

```rust
/// Joint-Typen für Verbindungen zwischen zwei Bodies.
pub enum JointType {
    /// Drehgelenk: Bodies verbunden an einem Punkt, frei rotierbar.
    Revolute { anchor_a: RenderVec2, anchor_b: RenderVec2 },
    /// Schiene: Body B kann nur entlang einer Achse relativ zu A gleiten.
    Prismatic { axis: RenderVec2, anchor_a: RenderVec2, anchor_b: RenderVec2 },
    /// Fest verbunden: Bodies bewegen sich als Einheit.
    Fixed { anchor_a: RenderVec2, anchor_b: RenderVec2 },
    /// Distanz-Joint: hält Bodies in festem Abstand.
    Distance { anchor_a: RenderVec2, anchor_b: RenderVec2, length: f32 },
}

pub struct Joint {
    pub joint_type: JointType,
    pub entity_a: EntityId,
    pub entity_b: EntityId,
    pub stiffness: f32,  // 0.0 = weich, 1.0 = starr
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
    pos: RenderVec2,
    velocity: RenderVec2,
    shape: &CollisionShape,
    obstacle: &Rect,
) -> Option<SweptContact>;

pub struct SweptContact {
    pub time: f32,       // 0.0..1.0, wann im Tick die Kollision auftritt
    pub normal: RenderVec2,
    pub contact: ContactInfo,
}

impl PhysicsWorld {
    /// Aktiviert CCD für Bodies mit Geschwindigkeit > threshold.
    pub fn set_ccd_threshold(&mut self, threshold: f32);
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

- **f32 vs Fixed-Point (bewusste Entscheidung):** Physics nutzt `f32` (RenderVec2), nicht Fixed-Point. Das ist kein Widerspruch zum Fixed-Point-Prinzip der Engine — es gibt zwei Kollisions-Ebenen:
  1. **Simulation (deterministisch):** Tile-basierte Kollision in `SimVec2` (Fixed-Point). Für Gameplay-Logik: Enemies bewegen sich auf Tile-Grid, Tower-Ranges, Pathfinding. Identisch auf allen Plattformen.
  2. **Physik (visuell):** RigidBody-Physik in `RenderVec2` (f32). Für visuelle Effekte: Bouncing, Ragdolls, Partikel-Interaktion. Nicht multiplayer-relevant, daher kein Determinismus nötig.
- **SimVec2 ↔ RenderVec2 Konvertierungsprotokoll:** Game-Systeme, die beide Welten berühren (z.B. Platformer Controller, Shmup Hitbox), konvertieren an klar definierten Grenzen:
  - **Simulation → Render:** `SimVec2::to_f32() -> RenderVec2` — verlustfrei bei typischen Spielwelt-Koordinaten (±32768 range).
  - **Render → Simulation:** `RenderVec2::to_fixed() -> SimVec2` — rundet auf nächsten Fixed-Point-Wert. Nur an Systemgrenzen verwenden, nie pro Frame hin-und-her konvertieren.
  - **Platformer:** Controller rechnet in SimVec2. Ergebnis-Velocity wird in RenderVec2 umgewandelt und auf RigidBody.velocity geschrieben. Collision-Response (Tilemap-basiert) läuft in SimVec2 über `sensor()` und `raycast_tiles()`.
  - **Shmup:** Player-Position in SimVec2, Bullet-Positionen in f32. Konvertierung geschieht einmal pro Frame für `hit_test()` und `graze_test()`. Präzisionsverlust ist bei Spielfeld-Größen (<1000px) irrelevant.
- **Multiplayer-Relevanz:** Physics (`PhysicsWorld::step()`) wird NICHT über das Netzwerk synchronisiert. Nur Tile-basierte Simulation ist deterministisch und replizierbar. Visuelle Physik (Ragdolls, Partikel-Bouncing) darf sich zwischen Clients unterscheiden.
- `FxHashMap<EntityId, RigidBody>` for O(1) body lookup.
- `FxHashSet` for duplicate pair elimination during broad phase.
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
