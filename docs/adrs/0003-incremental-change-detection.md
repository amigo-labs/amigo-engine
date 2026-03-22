---
number: "0003"
title: Echtes Strukturelles Change-Detection
status: done
date: 2026-03-20
---

# ADR-0003: Echtes Strukturelles Change-Detection

## Status

proposed

## Context

Change detection in the ECS is currently frame-global and coarse-grained. Each `SparseSet<T>` (`crates/amigo_core/src/ecs/sparse_set.rs`) maintains three tracking structures:

- `changed: BitSet` -- set in `get_mut()` (line 105) whenever a mutable reference is handed out, regardless of whether the value actually changed.
- `added: BitSet` -- set in `insert()` (line 63) when a new component is added.
- `removed_ids: Vec<EntityId>` -- pushed in `remove()` (line 86).

All three are cleared in `SparseSet::flush()` (line 169-173), which is called by `World::flush()` (`world.rs`, line 244-253) at the end of every tick. This means:

1. **No per-system visibility.** If system A modifies a `Position` and system B queries `iter_changed()` later in the same tick, system B sees the change. But if system B runs *before* system A (or in a future parallel schedule), it would see stale data from the previous tick. There is no mechanism to scope change flags to individual systems.

2. **False positives.** `get_mut()` marks the entity as changed even if the caller reads the value without modifying it. This is because Rust's mutable borrow is used as a proxy for mutation, but many systems borrow mutably for conditional updates.

3. **Despawns are batched.** `World::despawn()` (line 141) pushes to `pending_despawn`, and `flush()` (line 228-242) processes them all at once, iterating every storage (both static and dynamic) to remove components. With many dynamic component types, this is O(despawns * component_types).

The `BitSet` itself (`bitset.rs`) is a simple `Vec<u64>` with `set`/`get`/`clear` and an iterator. The `clear()` method (line 35-38) zeroes all words every tick. For 10k entities this is ~160 words = 1.25 KB, which is cheap, but the architectural issue is the frame-global scope.

## Decision

Implement **incremental, per-system-tick change detection** as part of the `ecs_archetypes` feature flag (shared with AP-01). This depends on AP-01 because archetype tables provide the structural grouping needed for efficient per-column change tracking.

The approach:

1. **Tick-stamped change tracking.** Replace the per-frame `BitSet` with a per-column `Vec<u32>` of "last changed tick" values, one entry per row in the archetype table. Each system records the tick at which it last ran. A query like `Changed<Position>` filters to rows where `last_changed_tick[row] > system_last_run_tick`.

2. **Deferred mutation detection.** Instead of marking changed on `get_mut()`, provide a `Mut<T>` wrapper that implements `DerefMut` and sets the change tick only in the `DerefMut` implementation (i.e., only when the value is actually written through). This eliminates false positives from read-only mutable borrows.

3. **Amortized despawns.** Instead of iterating all storages for each despawn, maintain a per-archetype "pending remove" list. When an entity is despawned, its archetype is known from the location table (AP-01), so only the columns in that archetype need to be touched. The swap-remove is done in O(components_in_archetype) rather than O(total_component_types).

### Alternatives Considered

1. **Double-buffered BitSets (ping-pong per system).** Each system gets two BitSets and alternates. Rejected because it doubles memory for change tracking and still does not provide true per-system scoping without tracking which tick each system last observed.

2. **Event-based change notification (emit an event on every mutation).** Rejected because it introduces allocation pressure (one event per mutation) and makes it impossible to batch-query "all entities that changed since I last ran."

## Migration Path

1. **Add `ChangeTicks` column type** -- In the archetype table (from AP-01), add a `Vec<u32>` alongside each component column that stores the tick of last mutation. Add a `SystemTick` type that tracks per-system last-run-tick. Verify: unit test that inserts entities, mutates some, advances the tick, and confirms `Changed<T>` query returns only the mutated entities.

2. **Implement `Mut<T>` wrapper** -- Create a `Mut<T>` type in `crates/amigo_core/src/ecs/change_detection.rs` that wraps `&mut T` and a reference to the change tick slot. `DerefMut` sets the tick; `Deref` does not. Verify: test that borrowing `Mut<T>` immutably (via `Deref`) does not set the change flag, but writing through it does.

3. **Amortized despawn** -- Update `World::despawn()` to look up the entity's archetype location and enqueue the despawn only for that archetype's columns, instead of iterating `self.positions`, `self.velocities`, etc. (currently `world.rs` lines 230-241). Verify: benchmark despawning 1000 entities with 3 components vs. the current approach; confirm no regression.

4. (rough) Wire `Changed<T>` and `Added<T>` query filters into the archetype iteration path.
5. (rough) Update `World::flush()` to no longer clear bitsets (they are replaced by tick comparisons).
6. (rough) Ensure backward compatibility: when `ecs_archetypes` is off, the existing `BitSet`-based tracking remains unchanged.

## Abort Criteria

- If the overhead of per-system change tracking (maintaining and comparing tick stamps) exceeds **0.2ms per tick** in a benchmark with 10k entities and 10 systems, abandon this approach and keep frame-global BitSets.
- If `Mut<T>` wrapper ergonomics are significantly worse than raw `&mut T` (e.g., requires explicit type annotations everywhere), reconsider the API.

## Consequences

### Positive
- Per-system change visibility enables correct behavior in parallel schedules (AP-02).
- False-positive elimination reduces unnecessary work in reactive systems (e.g., re-rendering only truly changed sprites).
- Amortized despawns scale with the entity's actual component set, not the total number of registered component types.

### Negative / Trade-offs
- Per-row tick storage adds 4 bytes per entity per component column (for 10k entities with 5 components: ~200 KB).
- `Mut<T>` wrapper adds a layer of indirection and may confuse users who expect raw `&mut T`.
- Coupled to AP-01; cannot be shipped without archetype storage.

## Updates

- 2026-03-22: Implemented behind `change_detection` feature flag in `crates/amigo_core/src/ecs/change_detection.rs`. Tick type, Mut<T> wrapper, Added<T>/Changed<T> filters, TickStorage, and World-level tick advancement.
