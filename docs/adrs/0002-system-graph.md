---
number: "0002"
title: Deklaratives System-Composition
status: proposed
date: 2026-03-20
---

# ADR-0002: Deklaratives System-Composition

## Status

proposed

## Context

The game loop in Amigo Engine is monolithic. The `Game` trait (`crates/amigo_engine/src/lib.rs`, lines 85-105) requires implementors to provide a single `fn update(&mut self, ctx: &mut GameContext) -> SceneAction` that runs all game logic. The engine calls this once per fixed-timestep tick inside `EngineApp::window_event` (`crates/amigo_engine/src/engine.rs`, lines 707-709):

```rust
let action = {
    let _update_span = info_span!("game_update").entered();
    self.game.update(&mut state.game_ctx)
};
```

Plugins get a separate `fn update(&mut self, ctx: &mut GameContext)` call after the game update (engine.rs, lines 722-727), but there is no ordering control between plugins or between game systems -- everything runs sequentially in the order the user writes it inside `Game::update()`.

This has three problems:

1. **No dependency declaration**: Systems that read Position after a physics system writes it must be manually ordered by the developer. Nothing enforces this.
2. **No parallelism**: All systems run single-threaded on the main thread. The `GameContext` borrows `&mut World` exclusively, preventing concurrent read-only access.
3. **Plugin interleaving**: Plugins always run after the game update (engine.rs line 724), making it impossible for a plugin to inject logic between two game systems (e.g., a networking plugin that must run between input-gathering and simulation).

The `Plugin` trait (engine.rs, lines 21-29) has `build`, `init`, and `update` hooks, but no concept of system ordering or dependency edges.

## Decision

Introduce a `SystemGraph` scheduler behind the `system_graph` feature flag. Systems are registered declaratively with ordering constraints, and the scheduler resolves execution order via topological sort.

**API shape**:
```rust
app.add_system(physics_system)
    .label("physics")
    .after("input")
    .before("render_prep");
```

Each system is a `fn(&mut SystemContext)` where `SystemContext` provides scoped access to components (read-only or read-write). The scheduler builds a DAG from labels and `after`/`before` constraints, topologically sorts it, and detects conflicts (two systems writing the same component without an ordering edge).

**Auto-parallelism** (phase 2): Systems with non-overlapping component access (determined by declared read/write sets) and no ordering constraint between them are dispatched to a thread pool. This depends on AP-01's archetype storage enabling safe concurrent read access to disjoint archetypes.

**Backward compatibility**: When `system_graph` is disabled, the existing `Game::update()` monolith continues to work. When enabled, `Game::update()` can still be used as a single "default" system, but users are encouraged to register individual systems.

### Alternatives Considered

1. **Stage-based scheduling (Bevy 0.x style)**: Fixed stages (PreUpdate, Update, PostUpdate) with systems sorted within each stage. Rejected because rigid stage boundaries make it hard to express fine-grained dependencies across stages, and the Bevy ecosystem has already moved away from this model.

2. **Manual thread pool with message passing**: Systems explicitly spawn tasks and communicate via channels. Rejected because it pushes scheduling complexity onto game developers and makes deterministic replay harder (AP-07 requires deterministic system ordering).

## Migration Path

1. **Define `System` trait and `SystemGraph` struct** -- Create `crates/amigo_core/src/scheduler/system_graph.rs` with `System { name, run_fn, reads: Vec<ComponentId>, writes: Vec<ComponentId>, after: Vec<Label>, before: Vec<Label> }` and a topological sort that produces an execution plan. Verify: unit test with 5 systems and explicit before/after constraints produces the expected linear order; cycle detection panics with a clear error message.

2. **Integrate into `Engine::run`** -- Behind `#[cfg(feature = "system_graph")]`, replace the single `game.update(&mut game_ctx)` call in the tick loop (engine.rs line 709) with `system_graph.run(&mut game_ctx)`. The Game trait gains `fn register_systems(&self, graph: &mut SystemGraph)` with a default implementation that wraps `update()` as a single system. Verify: existing `Game` implementations compile and run without changes when the feature is enabled.

3. (rough) Implement `SystemContext` that borrows specific component sets from `World`, enabling the borrow checker to verify non-overlapping access at runtime (or compile time via archetype queries from AP-01).

4. (rough) Add conflict detection: if two systems both write the same component and have no ordering edge, emit a warning (debug builds) or error.

5. (rough) Phase 2: thread-pool dispatch for non-conflicting systems using `rayon::scope` or a custom work-stealing pool. Benchmark with 30 systems to verify scheduling overhead stays under 0.5ms.

## Abort Criteria

- If scheduling overhead (topological sort + system dispatch + context construction) exceeds 0.5ms per tick with 30 registered systems, abandon the graph scheduler and keep the monolithic update.
- If the `SystemContext` borrow model cannot express the common pattern of "read Position, write Velocity" without runtime panics in typical game code, simplify to a stage-based model instead.

## Consequences

### Positive
- Explicit ordering constraints make system dependencies visible and enforceable.
- Enables future auto-parallelism for read-only systems without game code changes.
- Plugins can register systems at specific points in the graph, not just "after everything."

### Negative / Trade-offs
- New API surface that existing games must adopt to benefit from.
- Topological sort and dependency resolution add startup cost (one-time) and per-tick dispatch overhead.
- Deterministic replay (AP-07) requires the system execution order to be stable across runs -- the sort must be deterministic (e.g., tie-break by registration order).

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
