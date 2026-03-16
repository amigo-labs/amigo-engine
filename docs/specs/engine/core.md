---
status: draft
crate: amigo_core
depends_on: []
last_updated: 2026-03-16
---

# Core Types, Math & ECS

## Purpose

Provides the foundational types, fixed-point math, Entity Component System (ECS), tick scheduling, determinism guarantees, and the public API surface (Game trait, Builder pattern, design principles) that all other engine modules build upon.

## Public API

### Fixed-Point Arithmetic (Simulation)

All simulation uses Q16.16 fixed-point (`I16F16` from the `fixed` crate). Float (`f32`) is used only for rendering.

| Domain | Number Type | Deterministic | Used For |
|--------|-------------|---------------|----------|
| Simulation | `Fix` (I16F16) | Yes | Positions, velocities, health, damage, range, timers, cooldowns, pathfinding |
| Rendering | `f32` | No (irrelevant) | Screen positions, particles, camera, screen shake, UI |
| Data Files | `f32` literals | N/A | Converted to `Fix` on load |

### Key Types

```rust
pub type Fix = I16F16;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimVec2 { pub x: Fix, pub y: Fix }

#[derive(Clone, Copy)]
pub struct RenderVec2 { pub x: f32, pub y: f32 }

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId { index: u32, generation: u32 }
```

### ECS (SparseSet + Change Tracking)

The engine uses a custom Lightweight ECS. Not Bevy-style with macros and schedulers -- normal Rust code, explicit control flow, but with the flexibility of component composition.

**Storage: SparseSet per Component type.** Each component type gets a SparseSet -- a dense array (cache-friendly iteration) with a sparse lookup array (O(1) access by EntityId). No HashMap, no pointer chasing.

```rust
pub struct SparseSet<T> {
    sparse: Vec<u32>,           // EntityId.index -> dense index
    dense_ids: Vec<EntityId>,   // dense -> EntityId
    dense_data: Vec<T>,         // dense -> component data (cache-friendly)
    changed: BitSet,            // tracks mutations this tick
    added: BitSet,              // tracks insertions this tick
    removed: BitSet,            // tracks removals this tick
}
```

**Change Tracking** is automatic. Calling `get_mut()` marks the entity as changed. Systems can query only changed/added/removed entities:

```rust
// All enemies whose health changed this tick
for (id, health) in world.query_changed::<&Health, With<EnemyData>>() { ... }

// All entities that got a StatusEffect this tick
for (id, status) in world.query_added::<&StatusEffect>() { ... }
```

**World struct** holds all component storages:

```rust
pub struct World {
    entities: GenerationalArena,
    positions: SparseSet<Position>,
    velocities: SparseSet<Velocity>,
    healths: SparseSet<Health>,
    // ... one SparseSet per component type
}

impl World {
    pub fn spawn(&mut self) -> EntityId;
    pub fn despawn(&mut self, id: EntityId);
    pub fn add<T>(&mut self, id: EntityId, component: T);
    pub fn get<T>(&self, id: EntityId) -> Option<&T>;
    pub fn get_mut<T>(&mut self, id: EntityId) -> Option<&mut T>; // marks changed
    pub fn remove<T>(&mut self, id: EntityId);
    pub fn query<T: QueryParam>(&self) -> QueryIter<T>;
    pub fn query_changed<T: QueryParam>(&self) -> QueryIter<T>;
    pub fn query_added<T: QueryParam>(&self) -> QueryIter<T>;
    pub fn flush(&mut self); // end-of-tick: process despawns, clear tracking
}
```

**Game code is normal Rust** -- no macro magic, no dependency injection:

```rust
fn shooting_system(world: &mut World, audio: &mut AudioManager) {
    let towers: Vec<_> = world.query::<(&Position, &TowerData)>().collect();
    for (id, pos, tower) in &towers {
        for (eid, epos, mut health) in world.query::<(&Position, &mut Health), With<EnemyData>>() {
            if pos.distance(epos) < tower.range {
                health.current -= tower.damage;
            }
        }
    }
}

// Called explicitly, in order you control:
fn update(world: &mut World, ctx: &mut GameContext) {
    movement_system(world);
    shooting_system(world, &mut ctx.audio);
    collision_system(world);
    cleanup_dead_entities(world);
    world.flush();
}
```

### State-Scoped Entity Cleanup (from Bevy, improved)

Entities can be tagged with a game state. When the state changes, tagged entities are automatically despawned:

```rust
world.add(enemy, StateScoped(GameState::Playing));
// When transitioning to GameOver -> all StateScoped(Playing) entities auto-despawned
```

### Tick Scheduler

Systems that don't need to run every tick can be scheduled at intervals:

```rust
scheduler.every(10, |world| pathfinding_system(world));  // every 10 ticks
scheduler.every(3, |world| tower_targeting(world));       // every 3 ticks
scheduler.every(60, |world| cleanup_system(world));       // once per second
```

## Behavior

### Determinism Rules

1. Fixed timestep (60 ticks/sec)
2. Fixed-point arithmetic (Q16.16) for all simulation
3. Seeded RNG (`StdRng`) as part of GameState
4. No `HashMap` iteration in simulation (use `BTreeMap` / `IndexMap`)
5. No `f32` in simulation logic
6. All state changes through validated Commands

## API Design

### Minimal Example

```rust
use amigo::prelude::*;

struct MyGame;

impl Game for MyGame {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        if ctx.input.pressed(Key::Escape) {
            return SceneAction::Quit;
        }
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_sprite("player", vec2(100.0, 50.0));
    }
}

fn main() {
    Engine::build()
        .title("My Game")
        .virtual_resolution(480, 270)
        .build()
        .run(MyGame);
}
```

### Design Principles

- **One Context object** (`GameContext` for update, `DrawContext` for rendering). No ECS macro magic, no dependency injection.
- **No lifetime parameters** in user-facing API. Entities referenced by copy-type `EntityId`.
- **Builder pattern** for setup, short function calls in game loop.
- **String-based** for prototyping (`draw_sprite("name", pos)`), **typed handles** for performance (`draw_sprite_handle(HANDLE, pos)`).
- **`_ex` variant** with closure for extended options: `draw_sprite_ex("x", pos, |s| s.flip_x().tint(RED))`.
- **Events as structs** in a queue (no callbacks, no lifetime issues).
- **Scene/State Machine:** `Push` (overlay), `Pop` (back), `Replace` (transition), `Quit` (exit).
- **Dev mode:** fuzzy matching for asset names. `"playe_walk"` -> `"Did you mean 'player_walk'?"`.
- **Release mode:** fallback sprite (magenta rect) for missing assets, never crash.
