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
- Component types must be `Send + Sync`. This is a consequence of the parallel
  dispatch below, not of sparse sets themselves, and it applies to the dynamic
  path too (`register_dynamic`, `insert_dynamic`, `register_reflected`).

## Parallel dispatch (`system_graph`)

Optional, behind the `system_graph` feature. `SystemGraph` groups systems into
batches whose declared component access (`.reads::<T>()` / `.writes::<T>()`) is
pairwise disjoint. A system that declares nothing gets a full `&mut World` and
therefore always runs alone.

There are two ways to execute the graph:

- `SystemGraph::run` (safe) executes every batch one system at a time, in batch
  order. It is sound whatever the systems do.
- `SystemGraph::run_parallel` (`unsafe fn`) runs each batch through
  `rayon::scope`. Every batch member still receives a reconstructed
  `&mut World`, so the compiler cannot check what it touches. The caller
  guarantees that each declaring system touches only its declared storages and
  makes no structural change (spawn, despawn, flush, or `insert_dynamic` of a
  type not registered yet — registration can rehash the dynamic-storage map
  under a sibling). Breaking that is a data race, which is why the function is
  `unsafe` rather than safe-by-convention.

`run_parallel` used to be the behaviour of the safe `run`. That let entirely
safe code race — two systems with correct declarations, one of them inserting
the first component of a new dynamic type — so the threading moved behind
`unsafe` (2026-10 audit).

What else guards `run_parallel`:

- Undeclared systems are treated as conflicting with everything
  (`has_conflict`).
- `World` and all component storage are `Send + Sync`, so nothing thread-affine
  can be reached from a worker thread. `SendPtr<T>` carries `T: Send` / `T: Sync`
  bounds rather than a blanket impl, so this is checked by the compiler.
- Debug builds compare a structural fingerprint before and after each batch and
  panic if one changed — after the fact, so it reports a violation but cannot
  prevent it.

Making the parallel path safe requires `SystemContext` to hand out only the
declared storages (split `World`'s typed fields by destructuring, the dynamic
ones with `HashMap::get_disjoint_mut`) plus a command buffer for structural
changes. That changes every system signature and is tracked as follow-up work.
