---
number: "0001"
title: Archetype-basierter ECS-Storage
status: proposed
date: 2026-03-20
---

# ADR-0001: Archetype-basierter ECS-Storage

## Status

proposed

## Context

All entity/component data in Amigo Engine lives in per-component `SparseSet<T>` instances inside `World` (`crates/amigo_core/src/ecs/world.rs`). The five built-in component types (Position, Velocity, Health, SpriteComp, StateScoped) are stored as named public fields (lines 108-112), while game-specific components go through a `dynamic: FxHashMap<TypeId, Box<dyn AnyStorage>>` map (line 115) with type-erased access via `AnyStorage` (lines 73-101).

Queries use the `join`, `join3`, and `join4` free functions in `crates/amigo_core/src/ecs/query.rs`. Each join drives iteration from the smallest SparseSet and performs `O(1)` sparse lookups into the other sets per entity (e.g., `JoinIter3::next()` at line 162 calls `self.a.get(id)`, `self.b.get(id)`, `self.c.get(id)` for every candidate). For a 3-component join over 10k entities where most entities share the same component set, this means 2 random sparse-array lookups per entity per tick -- these are not cache-friendly because each `SparseSet` stores its dense data in a separate `Vec<T>` (`crates/amigo_core/src/ecs/sparse_set.rs`, line 11).

The `Component` trait (`crates/amigo_core/src/ecs/query.rs`, lines 330-335) statically routes to the correct `SparseSet` field, so changing the underlying storage is invisible to game code that uses `world.storage::<T>()`.

For games with many entities sharing the same component combination (e.g., 10k NPCs each with Position + Velocity + Health + SpriteComp), archetype storage would pack all four component arrays contiguously per archetype, enabling linear iteration with no sparse lookups and significantly better cache locality.

## Decision

Introduce a hybrid Archetype + SparseSet storage model behind the `ecs_archetypes` feature flag.

**Archetype storage**: entities with the same set of component types are grouped into an `Archetype` struct containing parallel `Vec<T>` columns for each component type. A `ComponentId`-keyed archetype graph tracks edges for add/remove operations. Iteration over archetypes matching a query is a linear scan of contiguous arrays -- no per-entity sparse lookup.

**Hybrid approach**: SparseSet remains the default for dynamic and infrequently-queried components. The 5 built-in component fields on `World` migrate to archetype storage; dynamic components added via `insert_dynamic` stay in SparseSets unless explicitly opted in. This avoids forcing archetype moves for debug-only or singleton components.

**Query API**: The existing `join`/`join3`/`join4` functions will dispatch to archetype iteration when all queried components are archetype-managed, falling back to the current SparseSet join otherwise. The `Component` trait gains an associated const `ARCHETYPE_MANAGED: bool` to enable compile-time dispatch.

### Alternatives Considered

1. **Pure archetype (Bevy-style)**: Every component in archetypes, archetype fragmentation handled by table-of-tables. Rejected because Amigo's dynamic component system (`insert_dynamic`) would cause excessive archetype fragmentation for game-specific tags and markers, and the migration cost is higher.

2. **SparseSet with columnar iteration optimization**: Keep SparseSets but add a `BitSet` intersection pre-pass to skip non-matching entities. Rejected because it still requires per-entity random access into each set's dense array; for large homogeneous populations the cache miss pattern is the bottleneck, not the branch.

## Migration Path

1. **Add `Archetype` and `ArchetypeId` types** -- Create `crates/amigo_core/src/ecs/archetype.rs` containing `Archetype { id, component_ids: Vec<ComponentId>, columns: Vec<Box<dyn AnyColumn>>, entity_ids: Vec<EntityId> }` and a `AnyColumn` trait mirroring `AnyStorage` but without change-tracking (change detection moves to ADR-0003). Verify: unit test spawning 1k entities into a single archetype and reading back all components.

2. **Build the archetype graph in `World`** -- Add `archetypes: Vec<Archetype>` and `entity_archetype: Vec<ArchetypeId>` to `World` behind `#[cfg(feature = "ecs_archetypes")]`. Implement `spawn_archetype()` that places the entity directly into the correct archetype based on the component bundle. Verify: `cargo test --features ecs_archetypes` passes; `world.get::<Position>(id)` returns the same value whether stored in archetype or SparseSet.

3. (rough) Implement archetype-aware `join`/`join3` that iterate matching archetypes linearly, falling back to SparseSet join for mixed queries.

4. (rough) Benchmark 3-component join over 10k entities (Position + Velocity + Health). Compare archetype iteration vs. current SparseSet join. If not 2x faster, abort.

5. (rough) Migrate the 5 built-in `SparseSet` fields on `World` behind the feature flag so they are backed by archetype columns when `ecs_archetypes` is enabled.

6. (rough) Update `flush()` to handle archetype-level despawn (swap-remove within archetype, update entity-to-archetype mapping).

## Abort Criteria

- If archetype-based 3-component iteration (Position + Velocity + Health) over 10,000 entities is not at least 2x faster than the current SparseSet `join3` implementation, abandon this approach and keep SparseSet-only storage.
- If archetype fragmentation exceeds 50 archetypes in the standard game template (indicating component combinations are too diverse for archetype grouping to help), reconsider the hybrid boundary.

## Consequences

### Positive
- Linear, cache-friendly iteration for the common case of many entities sharing the same component set.
- No per-entity sparse lookup overhead for matched archetype queries.
- SparseSet remains available for dynamic/singleton components, preserving flexibility.

### Negative / Trade-offs
- Adding or removing a component from an entity requires moving it between archetypes (memcpy of all columns), which is more expensive than the current SparseSet insert/remove.
- Two storage backends increase code complexity and the surface area for bugs.
- Feature-flag gating means CI must test both paths.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
