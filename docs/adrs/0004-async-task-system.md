---
number: "0004"
title: Async Task-System
status: done
date: 2026-03-20
---

# ADR-0004: Async Task-System

## Status

proposed

## Context

The engine is currently purely synchronous. The only async usage is `pollster::block_on(Renderer::new(...))` in `EngineApp::resumed` (`crates/amigo_engine/src/engine.rs`, line 427-431), which blocks the main thread during wgpu initialization.

Asset loading is synchronous and eager. `AssetManager::load_sprites()` (`crates/amigo_assets/src/asset_manager.rs`, line 37-44) recursively reads every PNG in the sprites directory at startup via `std::fs::read_dir` and `image::open`. The `load_from_pak` method (line 164-234) similarly blocks while reading and decoding the entire pak file. There is no mechanism for loading assets in the background after initialization.

The game loop in `engine.rs` (lines 700-735) runs a fixed-timestep simulation. Each tick calls `game.update()`, then plugin updates, then `world.flush()` and `events.flush()`. All of this is synchronous. There is no task queue, no background thread pool, and no way to start an I/O operation in one tick and collect the result in a later tick.

This means:

1. **Startup stalls.** Loading 100+ sprites blocks the window from appearing for seconds.
2. **No background I/O.** Saving game state (via `amigo_core::save`), loading levels, or fetching network data must happen on the main thread, causing frame hitches.
3. **No streaming.** Assets cannot be loaded on-demand (required by AP-05).

## Decision

Introduce a lightweight **task scheduler** behind the feature flag `async_tasks`. The design principle is: **the game loop stays synchronous** (`fn update` never becomes `async fn update`), but background work can be spawned and results collected via channels.

The task system has three components:

1. **`TaskPool`** -- A thread pool (defaulting to `num_cpus - 1` threads, minimum 1) that executes `FnOnce + Send + 'static` closures. Built on `std::thread` and a work-stealing deque (crossbeam or a minimal custom implementation). No async runtime (no tokio, no async-std).

2. **`Task<T>`** -- A handle returned by `TaskPool::spawn()`. Internally wraps a `oneshot::Receiver<T>`. The game loop can poll `task.try_recv()` each tick to check if the result is ready, or call `task.block()` for synchronous waiting.

3. **`TaskContext`** -- Added to `GameContext` when the feature is enabled. Provides `ctx.tasks.spawn(|| { ... })` and `ctx.tasks.spawn_io(|| { ... })` (the latter uses a separate I/O-bound pool with more threads). Results are delivered via the `Task<T>` handle.

The game loop itself remains `fn update(&mut self, ctx: &mut GameContext) -> SceneAction`. No `async` anywhere in the public API.

### Alternatives Considered

1. **Tokio runtime.** Full-featured async runtime with I/O reactor. Rejected because it is heavyweight (~1MB binary size increase), infects the API with `async`/`.await`, and is overkill for a game engine that primarily needs "run this closure on another thread."

2. **`async-executor` (smol-based).** Lighter than tokio but still requires an async runtime and `Future`-based API. Rejected for the same "async infection" concern -- game systems should not need to be async.

## Migration Path

1. **Implement `TaskPool` and `Task<T>`** -- Create `crates/amigo_core/src/tasks.rs` with a thread pool backed by `std::thread` and `crossbeam-channel` (or `std::sync::mpsc`). `spawn` returns a `Task<T>` that wraps a `Receiver<T>`. Gate behind `#[cfg(feature = "async_tasks")]`. Verify: test that spawns 100 tasks computing fibonacci numbers, collects all results, and confirms correctness.

2. **Add `TaskContext` to `GameContext`** -- In `crates/amigo_engine/src/context.rs`, add a `tasks: TaskPool` field when the feature is enabled. Initialize the pool in `EngineApp::resumed` (after `GameContext::new`). Verify: a test game that spawns a background file-read task in `init()` and polls for the result in `update()` works without blocking the main loop.

3. **Migrate asset loading** -- Convert `AssetManager::load_sprites()` to use `TaskPool::spawn_io` for individual sprite loads, returning `Task<SpriteData>` handles. The engine polls these during the splash screen phase. Verify: startup time for a 100-sprite project is reduced; no frame drops during loading.

4. (rough) Add `spawn_io` pool variant with higher thread count for I/O-bound work.
5. (rough) Integrate with AP-05 (asset streaming) for on-demand texture loads.
6. (rough) Add task cancellation via `Task::cancel()` (sets an `AtomicBool` that the closure can check).

## Abort Criteria

- If `async` infects the game loop -- i.e., if `Game::update` or any system function must become `async fn` -- abandon this approach. The game loop must remain synchronous.
- If the thread pool adds more than **1ms of overhead per tick** (thread wake/sleep latency) when idle (no tasks spawned), the implementation is too heavy.

## Consequences

### Positive
- Background I/O for asset loading, save games, and network operations without frame hitches.
- Foundation for AP-05 (streaming asset pipeline).
- Simple API: `ctx.tasks.spawn(|| { ... })` returns a pollable handle. No async/await required.
- Thread pool is reusable for compute-heavy tasks (e.g., pathfinding, physics broadphase).

### Negative / Trade-offs
- Adds a thread pool dependency and thread synchronization overhead.
- Game code must handle the "not ready yet" case when polling task results (adds complexity to asset loading paths).
- Results are delivered asynchronously, so game code cannot assume an asset is available in the same tick it was requested.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
