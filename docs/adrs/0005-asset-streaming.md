---
number: "0005"
title: Streaming Asset Pipeline
status: done
date: 2026-03-20
---

# ADR-0005: Streaming Asset Pipeline

## Status

done

## Context

Assets are currently loaded eagerly at startup. `AssetManager::load_sprites()` (`crates/amigo_assets/src/asset_manager.rs`, line 37) walks the entire `assets/sprites/` directory tree, calls `image::open` on every PNG (line 66-67), converts to RGBA8, and stores the decoded `image::RgbaImage` in a `FxHashMap<String, SpriteData>` (line 23). This means all sprite pixel data lives in CPU memory for the lifetime of the process.

GPU texture upload happens in `EngineApp::resumed` (`crates/amigo_engine/src/engine.rs`, line 433-436) via `assets.load_sprites()`, but the renderer's texture storage (`crates/amigo_render/src/texture.rs`) creates individual `wgpu::Texture` objects per sprite. There is no atlas packing at runtime -- the `AtlasBuilder` (`crates/amigo_render/src/atlas.rs`) exists but is only used during pak export, not at runtime.

The `SpriteData` struct (line 9-17 in `asset_manager.rs`) holds the full decoded image, UV rect, and a `texture_index`. The `handle.rs` module provides `AssetHandle<T>` and `AssetState` (Loading/Loaded/Failed) types, but these are not wired into the sprite loading path -- sprites are always synchronously loaded.

This architecture has scaling problems:

1. **Startup time.** A game with 500 sprites at 64x64 = ~500 * 16KB = 8MB of RGBA data decoded synchronously before the first frame.
2. **Memory pressure.** All sprites reside in CPU memory even if most are not visible. There is no eviction.
3. **No on-demand loading.** New content (level transitions, DLC) cannot be streamed in without a full reload.

The pak system (`crates/amigo_assets/src/pak.rs`) provides `PakReader` with random-access `read_entry(name)`, which is suitable for on-demand loading.

## Decision

Introduce a **streaming asset pipeline** behind the feature flag `asset_streaming`. This depends on AP-04 (async task system) for background I/O.

The system has three layers:

1. **Virtual Texture Registry.** A `TextureRegistry` replaces direct `FxHashMap<String, SpriteData>` lookups. Each sprite name maps to a `TextureSlot` that can be in one of three states: `NotLoaded`, `Loading(Task<RgbaImage>)`, `Resident(AtlasRegion)`. When game code requests a sprite, the registry returns a placeholder UV (1x1 white pixel from `Texture::white_pixel`, `texture.rs` line 123) if the texture is not yet resident, and kicks off a background load.

2. **Dynamic Atlas.** A runtime texture atlas (extending `AtlasBuilder` from `atlas.rs`) that packs sprites on-demand into one or more GPU atlas textures. When a background load completes, the decoded image is inserted into the atlas. If the atlas is full, a new atlas page is allocated (power-of-2, up to 4096x4096). The atlas uses shelf-packing (already implemented in `AtlasBuilder::pack`, line 118).

3. **LRU Eviction.** Each `TextureSlot` tracks a `last_used_tick`. At the end of each frame, if total GPU memory exceeds a configurable budget, the oldest unreferenced slots are evicted (atlas region freed, slot reset to `NotLoaded`). A defragmentation pass repacks the atlas periodically.

### Alternatives Considered

1. **Sparse virtual textures (GPU-side page tables).** True virtual texturing where the GPU samples from a page table and triggers page faults. Rejected because wgpu does not expose sparse texture support, and the complexity is disproportionate for a 2D pixel art engine.

2. **Pre-baked atlas-only (no streaming).** Expand the pak export to produce a single atlas; load it at startup. Simpler, but does not solve the level-transition loading problem and wastes GPU memory on textures not visible in the current scene.

## Migration Path

1. **Implement `TextureRegistry`** -- Create `crates/amigo_assets/src/streaming.rs` with the `TextureSlot` state machine and `TextureRegistry` type. Wire it into `AssetManager` behind the feature flag. The registry takes a `TaskPool` reference (from AP-04) for background loads. Verify: unit test that requests 10 sprites, confirms they start as `NotLoaded`, transitions through `Loading`, and become `Resident` after task completion.

2. **Implement dynamic atlas insertion** -- Extend `AtlasBuilder` (or create a new `DynamicAtlas` type in `crates/amigo_render/src/atlas.rs`) that supports incremental insertion into an existing GPU texture via `queue.write_texture` (used in `texture.rs`, line 67-81). Use shelf-packing with a free-list for evicted regions. Verify: insert 50 sprites one at a time, render them, confirm no UV corruption.

3. **Wire placeholder rendering** -- In the sprite batcher (`crates/amigo_render/src/sprite_batcher.rs`), when a sprite's `TextureSlot` is not yet `Resident`, substitute the white pixel texture and tint with a loading color. Verify: visual test shows sprites "pop in" as they load, with no crashes or black rectangles.

4. (rough) Implement LRU eviction with configurable memory budget.
5. (rough) Add atlas defragmentation (repack surviving regions into a fresh atlas texture).
6. (rough) Integrate with `PakReader` for streaming from pak files.

## Abort Criteria

- If streaming latency causes visible pop-in exceeding **100ms** from request to on-screen (measured as the time from first `sprite()` call to `Resident` state on a warm disk cache), the system is too slow.
- If atlas fragmentation causes more than **30% wasted GPU memory** after 1000 insert/evict cycles, the packing strategy needs revision.

## Consequences

### Positive
- Near-instant startup: only load what is needed for the first frame.
- Memory budget control: total GPU texture memory stays within a configurable limit.
- Enables level streaming and DLC content without full restarts.

### Negative / Trade-offs
- Visible pop-in during the first frame a sprite is requested (mitigated by placeholder rendering and preload hints).
- Atlas fragmentation requires periodic defragmentation passes.
- Increased complexity: three-state texture slots, dynamic atlas management, LRU bookkeeping.
- Depends on AP-04 (async task system) for background I/O.

## Updates

- 2026-03-22: Implemented TextureRegistry (streaming.rs) and DynamicAtlas (dynamic_atlas.rs) behind `asset_streaming` feature flag. Used std::sync::mpsc + background thread instead of AP-04 async task system (not yet available). LRU eviction implemented. Dynamic atlas uses shelf-packing with multi-page support.
