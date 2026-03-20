---
number: "0008"
title: GPU-Compute for Broad-Phase Physics and Collision
status: proposed
date: 2026-03-20
---

# ADR-0008: GPU-Compute for Broad-Phase Physics and Collision

## Status

proposed

## Context

The engine's physics and collision systems are entirely CPU-bound. Two files carry the full workload:

- **`crates/amigo_core/src/collision.rs`** implements a `SpatialHash` (lines 103-191) that buckets entities into grid cells using an `FxHashMap<CellKey, Vec<EntityId>>`. Broad-phase queries (`query_aabb`, `query_point`, `query_circle`) iterate cells and collect candidates. Narrow-phase tests cover AABB-vs-AABB, Circle-vs-Circle, Circle-vs-AABB, Capsule variants, and Swept AABB (CCD). The `CollisionWorld` (lines 249-318) ties them together.

- **`crates/amigo_core/src/physics.rs`** implements `PhysicsWorld` with Euler integration (line 184-195), a `rebuild_spatial_hash` pass that clears and re-inserts every body each step (lines 197-203), `find_collision_pairs` (lines 205-250) which iterates all bodies, queries the spatial hash, performs canonical-pair dedup with an `FxHashSet`, and runs `check_shapes` for narrow-phase. Impulse resolution runs up to `solver_iterations` (default 4) times per step (lines 158-176).

The hot path per frame is `rebuild_spatial_hash` + `find_collision_pairs`. For N dynamic bodies, `rebuild_spatial_hash` is O(N) inserts and `find_collision_pairs` is O(N * K) where K is the average candidate count per entity. At 2000+ dynamic bodies (e.g. a bullet-hell or large TD map), this dominates the frame budget.

The render crate (`crates/amigo_render/`) already initializes a wgpu `Device` and `Queue` (see `Renderer` in `crates/amigo_render/src/renderer.rs`, lines 50-67). The device, queue, and surface are public fields, meaning a compute pipeline can be created from the same adapter without a second GPU context.

## Decision

Introduce a `gpu_physics` feature flag that offloads the **broad-phase** collision detection to a wgpu compute shader. The narrow-phase and impulse resolution remain on the CPU.

### Architecture

1. **GPU Broad-Phase (`GpuBroadPhase`)**: A compute shader that takes a buffer of `(entity_id, aabb_min_x, aabb_min_y, aabb_max_x, aabb_max_y)` structs, sorts them on the X-axis (GPU radix sort), and performs a sweep-and-prune to emit candidate pairs into an output `StorageBuffer`. This replaces `SpatialHash::insert` + `SpatialHash::query_aabb` during `find_collision_pairs`.

2. **CPU Fallback (mandatory)**: When `gpu_physics` is disabled (default) or when no compute-capable adapter is available at runtime, the existing `SpatialHash` broad-phase is used unchanged. The `PhysicsWorld` dispatches through a `BroadPhase` trait:

   ```rust
   pub trait BroadPhase {
       fn find_candidates(&mut self, bodies: &[(EntityId, Rect)]) -> Vec<(EntityId, EntityId)>;
   }
   ```

   `SpatialHashBroadPhase` wraps the current logic. `GpuBroadPhase` wraps the compute path.

3. **Readback**: Candidate pairs are read back via `wgpu::Buffer::map_async` with a staging buffer. The readback is double-buffered: while the CPU processes frame N's pairs, the GPU computes frame N+1's pairs. This hides latency at the cost of one frame of staleness for the broad-phase (acceptable for a 2D engine at 60 fps).

4. **Integration point**: `PhysicsWorld::step()` calls `self.broad_phase.find_candidates(...)` instead of the inline `find_collision_pairs` loop. The rest of `step()` (integration, narrow-phase, impulse resolution, spatial hash rebuild for non-physics queries) is unchanged.

### Alternatives Considered

1. **GPU Narrow-Phase + Resolution**: Moving SAT/impulse resolution to compute shaders would require complex atomic operations for position correction and would couple physics tightly to the GPU. The narrow-phase is already O(P) where P is the number of actual collisions (typically small). Rejected because complexity outweighs benefit.

2. **Parallel CPU (rayon)**: A rayon-based parallel broad-phase could split the spatial hash query across threads. This gives ~2-4x speedup on typical hardware without GPU readback latency. This is a valid intermediate step and can coexist with the GPU path. Rejected as the *primary* solution because it does not scale as well for 5000+ entities, but it should be considered as a complementary optimization.

3. **Uniform Grid on GPU**: Instead of sort-and-sweep, build a uniform grid in a 2D texture and use atomic writes. Simpler shader but wastes memory for sparse worlds and has limited cell capacity. Rejected in favor of sort-and-sweep which handles non-uniform distributions better.

## Migration Path

1. **Extract `BroadPhase` trait** -- Move the body-iteration and candidate-pair logic from `PhysicsWorld::find_collision_pairs` (physics.rs lines 205-250) into a new `SpatialHashBroadPhase` struct implementing the `BroadPhase` trait. `PhysicsWorld` stores a `Box<dyn BroadPhase>`. All existing tests must pass unchanged. Verify: `cargo test -p amigo_core` passes, no behavior change in the TD sample game.

2. **Implement `GpuBroadPhase` behind `gpu_physics` flag** -- Create `crates/amigo_physics_gpu/` with a wgpu compute pipeline. The shader performs radix sort on the X-axis min coordinate, then a sweep pass emitting overlapping pairs. Use `Renderer.device` and `Renderer.queue` references (both are `pub` on the `Renderer` struct). The crate depends on `amigo_core` (for types) and `wgpu`. Feature-gate with `cfg(feature = "gpu_physics")` at the engine level. Verify: write a benchmark comparing `SpatialHashBroadPhase` vs `GpuBroadPhase` at 1000, 2000, and 5000 entities. GPU path must be faster at 2000+ entities on integrated GPU.

3. (rough) Double-buffered readback: implement staging buffer ping-pong so CPU and GPU overlap. Measure readback latency independently.

4. (rough) Wire `GpuBroadPhase` into `PhysicsWorld` construction in `amigo_engine` based on feature flag and runtime adapter capability check.

5. (rough) Add a runtime toggle so the debug overlay (F3 collision view) can switch between CPU and GPU broad-phase for comparison.

## Abort Criteria

- If GPU readback latency (measured from `map_async` callback to data availability on CPU) exceeds **4 ms** on integrated GPUs (Intel UHD 630 / Apple M1 class), the double-buffering trick cannot hide it within a 16.6 ms frame budget alongside rendering. Abandon the GPU path and invest in the rayon-based CPU parallel broad-phase instead.
- If the wgpu compute shader dispatch + readback is slower than the CPU `SpatialHash` at **2000 entities** on the benchmark hardware, the overhead is not justified. Abandon.
- If the `BroadPhase` trait abstraction introduces more than **5% regression** on the CPU-only path (due to dynamic dispatch or data marshaling), redesign the abstraction before proceeding.

## Consequences

### Positive
- Broad-phase scales to 5000+ dynamic entities without dominating the CPU frame budget.
- The `BroadPhase` trait extraction (step 1) is valuable regardless of the GPU path -- it enables future alternative algorithms (BVH, quad-tree) and parallel CPU implementations.
- Leverages the existing wgpu device from `amigo_render`, no second GPU context needed.

### Negative / Trade-offs
- One frame of broad-phase staleness due to double-buffered readback. For a 2D engine at 60 fps this is 16.6 ms of positional drift in the candidate list, which the narrow-phase corrects.
- Increased complexity: a new crate, compute shaders, staging buffer management, and a runtime capability check.
- Debug tooling becomes harder -- GPU broad-phase candidates are not trivially inspectable. Need to add a debug readback path.
- WebGPU compute shader support varies across browsers; the CPU fallback is essential for web targets.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
