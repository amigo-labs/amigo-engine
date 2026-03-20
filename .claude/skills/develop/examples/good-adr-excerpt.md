# Example: Good ADR (ECS Storage Migration)

This is an example of a well-written ADR. Note the specific file references, concrete migration steps, and measurable abort criteria.

```yaml
---
number: 0003
title: Migrate ECS from SparseSet to Archetype-Hybrid Storage
status: proposed
date: 2026-03-20
---
```

## Status

proposed

## Context

The ECS uses SparseSet storage (`crates/amigo_core/src/ecs/sparse_set.rs`). Each component type has its own SparseSet — a dense array with a sparse lookup. This gives O(1) access per entity per component.

For queries that iterate over entities with multiple components (Position + Velocity + Sprite), the system must look up each component independently. With 10k entities, the `movement_system` which queries `join3!(world, Position, Velocity, SpriteComp)` takes 3.8ms per tick — 23% of the 16.67ms frame budget.

Archetype-based storage groups entities with identical component sets into contiguous memory. Iteration over common component combinations becomes a linear memory scan instead of per-entity lookups.

## Decision

Introduce a hybrid storage model: **Archetype storage for core components** (Position, Velocity, Health, SpriteComp) that are frequently iterated together, and **SparseSet storage for dynamic/rare components** (game-specific types added via `insert_dynamic`).

### Alternatives Considered

**Full Archetype migration (Bevy-style):** Would require rewriting every system that accesses components. Too invasive for the benefit — most game-specific components are accessed individually, not in bulk queries.

**SIMD-optimized SparseSet iteration:** Would help but doesn't fix the fundamental cache-miss problem of multi-component queries.

## Migration Path

1. **Add `ecs_v2` module** alongside existing `ecs/` — Create `crates/amigo_core/src/ecs_v2/archetype.rs` with `Archetype` struct that stores components in struct-of-arrays layout. Verify: `cargo check --workspace` passes, existing tests still pass.

2. **Implement Archetype core** — `insert()`, `remove()`, `get::<T>()`, `iter::<T>()` with comprehensive unit tests. Verify: 15+ new tests pass covering insert, remove, iteration, and component access.

3. **Add migration bridge** — Function that converts `SparseSet<Position> + SparseSet<Velocity>` data into an Archetype. Round-trip test: populate sparse sets → migrate → verify all data matches. Verify: round-trip test passes for all core component types.

4. (rough) Wire archetype into World behind `cfg(feature = "ecs_v2")` feature flag
5. (rough) Update `join2/join3/join4` macros to use archetype iteration when available
6. (rough) Benchmark: compare old vs new with 10k entity movement system
7. (rough) If benchmark passes abort criteria: make `ecs_v2` the default, deprecate old path

## Abort Criteria

- If archetype iteration is not at least **2x faster** than sparse set iteration for 3-component queries with 10k entities, abandon.
- If the migration bridge cannot handle all 4 core components (Position, Velocity, Health, SpriteComp) without API changes visible to game code, abandon.
- If more than 8 existing tests need modification (not just addition), the migration is too invasive.

## Consequences

### Positive
- 3-component iteration drops from ~3.8ms to ~1ms (estimated from cache-line math)
- Foundation for future SIMD optimization of archetype iteration
- Dynamic components remain unchanged — no game code migration needed

### Negative / Trade-offs
- Two storage systems to maintain until old path is removed
- Component add/remove is slower with archetypes (entity moves between archetypes)
- Increased code complexity in ECS module

## Why This Is Good

1. **Specific context**: Names files, types, and includes a performance number (3.8ms)
2. **Measurable abort criteria**: "2x faster" is testable, not subjective
3. **First 3 steps are detailed**: Each has a clear deliverable and verification method
4. **Later steps are rough**: Acknowledges uncertainty — they'll be refined during implementation
5. **Consequences are honest**: Lists real trade-offs, not just benefits
