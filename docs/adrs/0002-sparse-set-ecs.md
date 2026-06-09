# ADR-0002: SparseSet-Based ECS with Dynamic Components

- Status: accepted
- Date: 2026-06-09

## Context

The engine needs an entity-component store that is cache-friendly to iterate,
cheap to add/remove components on, and simple enough that game code stays
readable. The two mainstream designs are archetype storage (tables per
component combination, as in Bevy/Flecs) and sparse sets (per-component
arrays, as in EnTT). Archetypes iterate joins faster but make structural
changes (add/remove component) expensive and the implementation considerably
more complex.

## Decision

`amigo_core::ecs` stores each component type in a `SparseSet<T>`: a dense
array of component data plus dense entity-id array for iteration, and a
sparse index keyed by entity for O(1) lookup. Entities are generational ids
(`EntityId` = index + generation) so stale references fail to resolve instead
of aliasing recycled slots.

Two registration paths exist:

- Built-in engine components live as typed fields on `World`.
- Game-defined components use the dynamic path: `World::insert_dynamic<T>` /
  `dynamic::<T>()` registers a `SparseSet<T>` keyed by `TypeId` at runtime, so
  games never have to modify the engine to add components.

Multi-component iteration uses `ecs::join` over sparse sets, walking the
smallest set and probing the others. Change tracking is bitset-based
(`ecs::bitset`, `change_detection`).

## Consequences

- Component add/remove is O(1) and never moves other components, which suits
  gameplay code that toggles components frequently.
- Joins are slower than archetype tables in the worst case; the engine
  compensates by keeping hot loops (physics, particles) on dedicated
  structures rather than generic queries.
- The dynamic-component path means scripts/games can extend the world without
  engine changes, at the cost of a `TypeId` hash-map lookup per set access.
- Determinism: iteration order follows dense insertion order, which is stable
  for identical input sequences and therefore safe for the deterministic
  simulation (see ADR-0001).
