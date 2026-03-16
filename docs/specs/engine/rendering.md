# Rendering Pipeline

> Status: draft
> Crate: amigo_render
> Depends on: [engine/core](../engine/core.md)
> Last updated: 2026-03-16

## Zweck

Provides the GPU rendering pipeline for pixel art games: sprite batching, virtual resolution scaling, layered rendering with parallax, tilemap chunk caching, and optional modern effects (lighting, particles, post-processing). All rendering goes through wgpu for cross-platform GPU support.

## Public API

- **Backend:** wgpu (Vulkan/DX12/Metal/WebGPU)
- **Sprite Batcher:** Collect all sprites per frame, sort by texture atlas, one draw call per atlas. Target: 5-10 draw calls for a full TD scene.
- **Virtual Resolution:** Configurable (e.g., 480x270), pixel-perfect scaling via nearest-neighbor to window size.
- **No artificial limits:** Unlimited colors, alpha, blend modes, shaders.

## Verhalten

### Layer Model (SNES-inspired)

| Layer | Z-Order | Content |
|-------|---------|---------|
| Background | 0 | Sky, distant scenery (parallax) |
| Terrain | 1 | Tilemap ground layer |
| Decoration (back) | 2 | Behind-entity decorations |
| Entities | 3 | Towers, enemies, projectiles |
| Decoration (front) | 4 | In-front decorations |
| Effects | 5 | Particles, explosions |
| UI | 6 | HUD, menus |
| Debug | 7 | Debug overlay (dev only) |

Each layer has independent scroll factor for parallax.

### Tilemap Rendering

Chunk-based (16x16 tiles) with render texture caching. Only dirty chunks re-rendered. Chunks outside camera frustum culled.

### Modern Effects (optional)

Dynamic lighting (normal maps, point lights), particles (pixel-sized), post-processing (bloom, chromatic aberration, CRT filter), screen shake, hitstop, custom WGSL shaders.
