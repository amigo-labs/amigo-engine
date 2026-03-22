---
number: "0001"
title: Archetype-basierter ECS-Storage
status: done
date: 2026-03-20
---

# ADR-0001: Archetype-basierter ECS-Storage

## Status

done

## Context

The ECS in `crates/amigo_core/src/ecs/` currently stores every component type in its own `SparseSet<T>` (defined in `sparse_set.rs`). The `World` struct (`world.rs`, line 104) holds five statically-typed `SparseSet` fields (`positions`, `velocities`, `healths`, `sprites`, `state_scoped`) plus a `dynamic: FxHashMap<TypeId, Box<dyn AnyStorage>>` map for game-specific components.

Queries that touch multiple components -- `join`, `join3`, `join4` in `query.rs` -- drive iteration from the smallest set and perform sparse lookups into every other set. For example, `JoinIter3::next()` (line 162) calls `self.a.get(id)`, `self.b.get(id)`, `self.c.get(id)` per entity. Each `.get()` does a bounds check plus an indirect read through the sparse array (`sparse_set.rs`, line 91-97). With 10k entities and a 3-component join this means ~30k random sparse lookups per query.

This access pattern has two performance problems:

1. **Cache misses on multi-component iteration.** Each `SparseSet` stores its dense data contiguously, but the data for entity N in set A and entity N in set B live at unrelated memory addresses. A system that reads `(Position, Velocity, Health)` for 10k entities bounces between three disjoint allocations.

2. **Scaling.** The `join` functions pick the smallest set to drive and probe the others, but the probing is O(1) per lookup only in terms of instructions -- the cache cost is what matters when the sparse arrays grow large (the sparse array in `SparseSet` resizes to hold the maximum entity index; see `ensure_sparse`, line 40).

An archetype-based layout groups entities that share the exact same set of component types into contiguous "archetype tables." Iterating a 3-component query becomes a linear scan over one or more archetype tables with zero sparse lookups.

## Decision

Introduce a hybrid **Archetype + SparseSet** storage model behind the feature flag `ecs_archetypes`.

**Archetype storage** will be the primary path for multi-component iteration:
- An `Archetype` struct contains a `Vec<u8>` column per component type, tightly packed, plus a parallel `Vec<EntityId>`.
- An `ArchetypeId` is a hash of the sorted `TypeId` set. A `FxHashMap<ArchetypeId, Archetype>` in `World` holds all archetypes.
- Entity-to-archetype mapping is maintained in a `Vec<ArchetypeLocation>` indexed by `EntityId::index`, where `ArchetypeLocation = (ArchetypeId, row: u32)`.
- When an entity gains or loses a component, it moves to the appropriate archetype (row swap-remove in the old, push in the new).

**SparseSet storage** remains available as a fallback for singleton/tag components or components that are added and removed very frequently (where archetype moves would be expensive). The existing `SparseSet<T>` is unchanged.

**Query API:** The existing `join`/`join3`/`join4` free functions will be updated to iterate matching archetype tables when the feature is enabled, falling back to the current sparse-set probing path when it is not. The `Component` trait (`query.rs`, line 330) will gain an associated constant indicating storage strategy.

### Alternatives Considered

1. **Pure archetype (no SparseSet fallback).** Rejected because frequent component add/remove (e.g., status effects, temporary tags) causes excessive archetype moves. The hybrid approach lets users opt specific component types into SparseSet storage.

2. **Grouped SparseSet (sort dense arrays to align entity order across sets).** Simpler to implement but does not give contiguous multi-component rows and requires re-sorting each frame, which is O(n log n) per set.

## Migration Path

1. **Add `Archetype` and `ArchetypeMap` types** -- Create `crates/amigo_core/src/ecs/archetype.rs` with the archetype table, column storage, and archetype graph (edges for "add component X" / "remove component X" transitions). Gate behind `#[cfg(feature = "ecs_archetypes")]`. Verify: unit tests that insert 10k entities with 3 components, iterate them, and confirm the data is correct.

2. **Wire archetype storage into `World`** -- Behind the feature flag, add an `archetypes: ArchetypeMap` field to `World` (`world.rs`). Update `spawn`/`despawn`/`add`/`remove_comp` to maintain archetype membership alongside the existing sparse sets. Verify: existing ECS tests pass with the feature both on and off (`cargo test --features ecs_archetypes -p amigo_core`).

3. **Benchmark harness** -- Add a `benches/ecs_iter.rs` criterion benchmark that creates 10k entities with `(Position, Velocity, Health)` and measures `join3` iteration time with and without the feature. Verify: archetype path is at least 2x faster than the SparseSet path. If not, abort (see below).

4. (rough) Update `join`/`join3`/`join4` to dispatch through archetype tables when the feature is enabled.
5. (rough) Add `#[component(storage = "sparse_set")]` attribute macro or trait constant to let users opt specific types out of archetype storage.
6. (rough) Handle archetype edge caching so that repeated add/remove patterns (e.g., `StatusEffect`) do not re-hash every time.

## Abort Criteria

- If archetype-based iteration is not at least **2x faster** than the current SparseSet `join3` for 3-component queries with 10k entities in a criterion benchmark, abandon this approach.
- If archetype move cost causes a measurable regression (>0.5ms per frame) in a stress test that adds/removes a component from 500 entities per tick, fall back to pure SparseSet.

## Consequences

### Positive
- Linear, cache-friendly iteration for the common "process all entities with components A, B, C" pattern.
- Foundation for automatic system parallelization (AP-02): archetype tables can be borrowed independently.
- Matches the mental model of most ECS literature, making the engine easier to learn.

### Negative / Trade-offs
- Adding or removing a component is now an archetype move (memcpy of all columns for that entity), which is more expensive than a single `SparseSet::insert`/`remove`.
- Increased code complexity: two storage backends with feature-gated code paths.
- Entity lookup by ID now requires an indirection through the archetype location table.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
