---
number: "0002"
title: Deklaratives System-Composition
status: done
date: 2026-03-20
---

# ADR-0002: Deklaratives System-Composition

## Status

done

## Context

Game logic currently lives inside a single monolithic `Game::update(&mut self, ctx: &mut GameContext) -> SceneAction` method (defined in `crates/amigo_engine/src/lib.rs`, line 90). The engine calls this once per fixed-timestep tick inside `EngineApp::window_event` (`engine.rs`, line 707-710):

```rust
let action = {
    let _update_span = info_span!("game_update").entered();
    self.game.update(&mut state.game_ctx)
};
```

After the game update, plugins get a sequential `plugin.update(&mut state.game_ctx)` call (line 724-727), and then `world.flush()` and `events.flush()` run at the end of the tick (line 731-733).

This architecture has several limitations:

1. **No declarative ordering.** System execution order is implicit in the procedural code inside `Game::update`. There is no way to express "system A runs before system B" or "system C needs write access to positions but only read access to healths" without manually structuring the code.

2. **No automatic parallelization.** All systems run sequentially on the main thread. The `Plugin::update` trait method (`engine.rs`, line 28) takes `&mut GameContext`, making every plugin mutually exclusive.

3. **No dependency tracking.** There is no way for the engine to detect data races between systems at compile time or runtime. Two plugins writing to `world.positions` in the same tick is silently racy.

The `Plugin` trait (`engine.rs`, line 21-29) provides `build`, `init`, and `update` hooks, but `update` is called in registration order with no dependency metadata.

## Decision

Introduce a **System Graph** behind the feature flag `system_graph`. This depends on AP-01 (archetype storage) for the component access metadata needed to determine parallelism.

A system is a function `fn(SystemContext) -> ()` registered via a builder API:

```rust
engine.add_system(movement_system)
    .label("movement")
    .after("input")
    .before("collision")
    .writes::<Position>()
    .reads::<Velocity>();
```

At startup, the engine builds a DAG from the declared ordering constraints and component access sets. A topological sort produces an execution schedule. Systems that have no data conflicts (disjoint component access sets) and no ordering constraints are grouped into parallel stages.

At runtime, each tick walks the schedule. Sequential stages run on the main thread. Parallel stages dispatch systems to a thread pool (using `rayon::scope` or a lightweight fork-join scheduler). `GameContext` is split into per-system borrows based on the declared access, checked at schedule-build time.

The existing `Game::update` method remains as a compatibility shim -- it runs as a single system at a configurable point in the schedule (default: after all engine systems, before flush).

### Alternatives Considered

1. **Manual stage ordering (Phase enum).** Simpler: define a fixed set of phases (PreUpdate, Update, PostUpdate) and let systems register into phases. Rejected because it does not allow fine-grained parallelism within a phase and forces an artificial phase taxonomy.

2. **Async systems with `async fn update`.** Would leverage Rust's async machinery for cooperative scheduling. Rejected because it infects the entire API with async, conflicts with the fixed-timestep guarantee, and has poor ergonomics for game code.

## Migration Path

1. **Define `System` trait and `SystemDescriptor`** -- Create `crates/amigo_core/src/ecs/system.rs` with a `System` trait (a `run` method taking a `SystemContext`), a `SystemDescriptor` holding label, ordering constraints, and component access sets. Gate behind `#[cfg(feature = "system_graph")]`. Verify: unit test that constructs 5 descriptors with `.after()/.before()` constraints and produces a valid topological order.

2. **Build the scheduler** -- Implement topological sort with Kahn's algorithm in `crates/amigo_core/src/ecs/schedule.rs`. Detect cycles (return an error with the cycle path). Group non-conflicting systems into parallel stages. Verify: test that 30 no-op systems schedule in <0.1ms, and that a cycle is detected and reported.

3. **Integrate with `EngineApp`** -- Behind the feature flag, replace the `game.update()` + `plugin.update()` sequence in `engine.rs` (lines 707-727) with `schedule.run(&mut game_ctx)`. The `Game::update` method is registered as a system labeled `"game_update"`. Verify: existing game code works unchanged when the feature is enabled.

4. (rough) Add `rayon` dependency behind the feature flag; dispatch parallel stages via `rayon::scope`.
5. (rough) Provide a `SystemContext` that borrows only the declared components, enforced by the schedule builder.
6. (rough) Add a debug visualization (F-key toggle) that prints the system graph and per-system timing.

## Abort Criteria

- If scheduling overhead exceeds **0.5ms per tick** with 30 registered systems (measured on a release build, single-threaded), abandon this approach.
- If the API requires more than 3 lines of boilerplate per system registration, simplify or reconsider.

## Consequences

### Positive
- Declarative ordering eliminates implicit execution-order bugs.
- Automatic parallelization of non-conflicting systems across CPU cores.
- Foundation for hot-reloadable systems and editor integration.
- Data-race detection at schedule-build time rather than at runtime.

### Negative / Trade-offs
- Increased complexity in the engine core; the schedule builder is non-trivial.
- Indirection between system registration and execution makes debugging harder (stack traces go through the scheduler).
- Depends on AP-01 for component access metadata; cannot ship independently.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
