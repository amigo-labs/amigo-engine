---
status: done
crate: amigo_render
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Rendering Pipeline

## Purpose

Provides the GPU rendering pipeline for pixel art games: sprite batching, virtual resolution scaling, layered rendering with parallax, tilemap chunk caching, and optional modern effects (lighting, particles, post-processing). All rendering goes through wgpu for cross-platform GPU support.

## Public API

- **Backend:** wgpu (Vulkan/DX12/Metal/WebGPU)
- **Sprite Batcher:** Collect all sprites per frame, sort by texture atlas, one draw call per atlas. Target: 5-10 draw calls for a full TD scene.
- **Virtual Resolution:** Configurable (e.g., 480x270), pixel-perfect scaling via nearest-neighbor to window size.
- **No artificial limits:** Unlimited colors, alpha, blend modes, shaders.

## Behavior

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

---

## Extensions (Sandbox/God Sim)

> Added per gap analysis (`05-sandbox-godsim-gaps.md`). Lighting implementation is in `crates/amigo_render/src/lighting.rs`.

### Layer-Blending: Light-Map as Multiply-Layer

The `LightingState` system collects ambient light and point lights, then serializes them into a GPU-friendly uniform buffer. The light map is applied as a multiply-blend pass over the tile layers.

```rust
// crates/amigo_render/src/lighting.rs

pub struct AmbientLight {
    pub color: Color,
    pub intensity: f32,
}

pub struct PointLight {
    pub position: (f32, f32),
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub falloff: f32,        // Attenuation exponent
}

pub struct LightingState {
    pub ambient: AmbientLight,
    pub lights: Vec<PointLight>,
    pub max_lights: usize,     // Default: 64
}

impl LightingState {
    pub fn add_light(&mut self, light: PointLight) -> usize;
    pub fn remove_light(&mut self, index: usize);
    pub fn set_ambient(&mut self, color: Color, intensity: f32);
    pub fn build_uniform_data(&self) -> Vec<u8>;  // GPU buffer bytes
}
```

The `build_uniform_data()` method produces a `LightingHeader` (ambient color pre-multiplied by intensity, light count) followed by N `LightData` structs (position, color, radius, intensity, falloff). This buffer is uploaded to the GPU and sampled in a multiply-blend fragment shader pass.

For Sandbox games, tile-emissive lights (torches, lava) feed into `LightingState` via the tile property `light_emission` from `TileProperties`.

### Parallax Backgrounds

Multiple background layers with independent scroll speeds create depth. The existing `TileLayer` struct already has `scroll_factor_x` / `scroll_factor_y` fields:

```rust
// crates/amigo_tilemap/src/lib.rs

pub struct TileLayer {
    pub scroll_factor_x: f32,  // 1.0 = normal, 0.5 = half speed, 0.0 = fixed
    pub scroll_factor_y: f32,
    // ...
}
```

The layer model (Background z=0 through Debug z=7) supports this natively. Typical Sandbox parallax setup:

| Layer | Scroll Factor | Content |
|-------|--------------|---------|
| Sky | 0.0 | Fixed gradient or texture |
| Far mountains | 0.1 | Distant silhouettes |
| Clouds | 0.2 | Slow-drifting cloud sprites |
| Near hills | 0.4 | Mid-ground terrain |
| Terrain | 1.0 | Main tilemap |

Each layer renders with its scroll offset = `camera_position * scroll_factor`.

### LOD for Zoom: Sprite Simplification

When the God Sim camera is zoomed far out, rendering thousands of full-detail sprites is wasteful. LOD tiers based on `Camera::zoom`:

| Zoom Range | LOD Level | Strategy |
|-----------|-----------|----------|
| >= 1.0 | Full | All sprites, full animation, all particles |
| 0.5 -- 1.0 | Reduced | Skip decoration layers, reduce particle count |
| 0.2 -- 0.5 | Simplified | Static sprites (no animation), skip effects layer |
| < 0.2 | Icons | Replace entity sprites with colored dots/icons |

Implementation approach:
- The `SpriteBatcher` already sorts by texture atlas and z-order. LOD filtering happens before sprites are submitted to the batcher.
- Game code checks `camera.zoom` and skips `batcher.push()` calls for low-priority sprites when zoom is below threshold.
- For icon mode, substitute the sprite's atlas region with a small icon variant from the same atlas (no extra draw calls).
