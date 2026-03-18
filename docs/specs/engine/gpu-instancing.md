---
status: spec
crate: amigo_render
depends_on: ["engine/rendering"]
last_updated: 2026-03-18
---

# GPU Instancing

## Purpose

Replace the per-quad indexed draw approach in the sprite batcher with hardware
GPU instancing where beneficial: one draw call renders thousands of sprites that
share the same texture and shader.  The primary targets are tilemap layers
(hundreds of identical tile sprites) and particle systems.  The current batching
system remains as a fallback for small or heterogeneous draw sets.

## Existierende Bausteine

### SpriteBatcher (`crates/amigo_render/src/sprite_batcher.rs`, 218 lines)

Current non-instanced architecture:

| Type / Method | Description |
|---------------|-------------|
| `SpriteShader` | Per-sprite shader effects: `Flash`, `Outline`, `Dissolve`, `PaletteSwap`, `Silhouette`, `Wave` |
| `SpriteInstance` | Per-sprite data: `texture_id`, position, size, UV rect, tint, flip_x/y, z_order, shaders |
| `SpriteBatcher` | Collects sprites per frame (capacity 1024), sorts, generates vertices/indices |
| `SpriteBatch` | Output batch: `texture_id`, `vertex_offset`, `index_offset`, `index_count` |
| `SpriteBatcher::push(sprite)` | Add a sprite to the frame |
| `SpriteBatcher::build() -> Vec<SpriteBatch>` | Sort by z_order then texture_id, emit 4 vertices + 6 indices per quad, group contiguous same-texture sprites into batches |
| `SpriteBatcher::vertices() / indices()` | Access generated vertex/index data |

### Vertex (`crates/amigo_render/src/vertex.rs`)

```rust
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}
```

Each sprite currently emits 4 `Vertex` entries (one per corner) and 6 indices
(two triangles).  The CPU builds the vertex buffer every frame.

### Current rendering path

1. `SpriteBatcher::clear()` at frame start.
2. Game systems call `push()` for every visible sprite.
3. `build()` sorts and generates vertex/index buffers.
4. Renderer uploads vertex + index buffers to GPU.
5. For each `SpriteBatch`: bind texture, call `draw_indexed(index_count)`.

This produces one draw call per texture change.  For a typical scene with
3-5 atlases, this means 3-5 draw calls -- adequate for moderate sprite counts
but CPU-bottlenecked beyond ~10k sprites due to per-quad vertex generation.

## Public API

### Proposed: InstanceData

```rust
/// Per-instance data uploaded to the GPU instance buffer.
/// Matches the WGSL vertex input layout.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceData {
    /// World-space position (x, y) and size (w, h).
    pub transform: [f32; 4],
    /// UV rectangle in the atlas (x, y, w, h).
    pub uv_rect: [f32; 4],
    /// RGBA tint color.
    pub tint: [f32; 4],
    /// Flags packed as u32: bit 0 = flip_x, bit 1 = flip_y.
    pub flags: u32,
    /// Z-order (used for depth testing or sorting).
    pub z_order: f32,
    /// Padding to align to 16 bytes.
    pub _pad: [f32; 2],
}
```

### Proposed: InstanceBuffer

```rust
/// Manages a GPU buffer of per-instance data, double-buffered to avoid
/// stalls from writing while the GPU is still reading the previous frame.
pub struct InstanceBuffer {
    buffers: [wgpu::Buffer; 2],
    current: usize,
    capacity: u32,
    count: u32,
}

impl InstanceBuffer {
    pub fn new(device: &wgpu::Device, initial_capacity: u32) -> Self;

    /// Write instance data for this frame.  Grows the buffer if needed.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue,
                  instances: &[InstanceData]);

    /// Current buffer for binding.
    pub fn buffer(&self) -> &wgpu::Buffer;

    /// Number of instances written this frame.
    pub fn count(&self) -> u32;

    /// Swap to the next buffer (call at end of frame).
    pub fn flip(&mut self);
}
```

### Proposed: InstancedSpritePipeline

```rust
/// A wgpu render pipeline configured for instanced sprite drawing.
pub struct InstancedSpritePipeline {
    pipeline: wgpu::RenderPipeline,
    instance_buffer: InstanceBuffer,
    quad_vertex_buffer: wgpu::Buffer,   // unit quad (4 vertices)
    quad_index_buffer: wgpu::Buffer,    // 6 indices
}

impl InstancedSpritePipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self;

    /// Record an instanced draw call for a batch of sprites sharing one texture.
    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>,
                     bind_group: &'a wgpu::BindGroup,
                     instance_range: std::ops::Range<u32>);
}
```

### Proposed: Hybrid SpriteBatcher

```rust
impl SpriteBatcher {
    /// Build batches, choosing instanced or indexed path per batch.
    /// Batches with more than `instancing_threshold` sprites use instancing;
    /// smaller batches use the existing indexed path.
    pub fn build_hybrid(&mut self, threshold: u32)
        -> (Vec<SpriteBatch>, Vec<InstancedBatch>);
}

/// A batch to be drawn with instancing.
pub struct InstancedBatch {
    pub texture_id: TextureId,
    pub instance_offset: u32,
    pub instance_count: u32,
}
```

## Behavior

### Instanced Rendering Path

1. A single unit quad (0,0)-(1,1) is stored in a static vertex buffer.
2. Per-instance data (`InstanceData`) is written to the instance buffer each
   frame.
3. The vertex shader reads per-vertex data (quad corner) and per-instance data
   (transform, UV, tint) to compute final position and texture coordinates.
4. One `draw_indexed(6, instance_count)` call renders all instances in a batch.

### WGSL Shader

```wgsl
struct VertexInput {
    @location(0) quad_pos: vec2<f32>,
    @location(1) quad_uv: vec2<f32>,
};

struct InstanceInput {
    @location(2) transform: vec4<f32>,   // x, y, w, h
    @location(3) uv_rect: vec4<f32>,     // u, v, uw, uh
    @location(4) tint: vec4<f32>,
    @location(5) flags_z: vec2<f32>,     // packed flags, z_order
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniforms;

@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let flags = u32(inst.flags_z.x);
    var qx = vert.quad_pos.x;
    var qy = vert.quad_pos.y;

    // Flip support
    if (flags & 1u) != 0u { qx = 1.0 - qx; }
    if (flags & 2u) != 0u { qy = 1.0 - qy; }

    let world_pos = vec2<f32>(
        inst.transform.x + qx * inst.transform.z,
        inst.transform.y + qy * inst.transform.w,
    );

    out.position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    out.uv = vec2<f32>(
        inst.uv_rect.x + qx * inst.uv_rect.z,
        inst.uv_rect.y + qy * inst.uv_rect.w,
    );
    out.color = inst.tint;
    return out;
}
```

### Batching Strategy

The hybrid batcher applies these rules:

| Condition | Path | Rationale |
|-----------|------|-----------|
| Batch has >= threshold sprites (default 64) | Instanced | GPU parallelism wins |
| Batch has < threshold sprites | Indexed (current) | Instancing overhead not worth it |
| Sprites have per-sprite shaders (`SpriteShader`) | Indexed | Shader variants need different pipelines |
| Tilemap chunks | Always instanced | Hundreds of identical tiles per chunk |

### Tilemap Instanced Rendering

Tilemap chunks (16x16 = 256 tiles) are a natural fit for instancing:
- All tiles in a chunk share the same atlas texture.
- `InstanceData` is rebuilt only when the chunk is dirty (tile changed).
- Cached instance data is stored per-chunk and reused across frames.
- Invisible chunks (outside camera frustum) skip upload entirely.

### Breakeven Analysis

Estimated crossover point where instancing becomes faster than indexed drawing:

| Sprite Count | Indexed (CPU time) | Instanced (CPU time) | Winner |
|-------------|-------------------|---------------------|--------|
| 10 | ~2us | ~5us | Indexed |
| 64 | ~15us | ~8us | Instanced |
| 256 | ~60us | ~12us | Instanced |
| 1000 | ~230us | ~20us | Instanced |
| 10000 | ~2.3ms | ~50us | Instanced |

The indexed path generates 4 vertices + 6 indices per sprite on the CPU.
The instanced path writes 1 `InstanceData` (64 bytes) per sprite -- 46x less
data.  The GPU does more work per vertex but parallelizes trivially.

## Internal Design

- `InstanceBuffer` uses double buffering (two wgpu buffers, ping-pong each
  frame) to avoid GPU stalls.  Buffer growth is geometric (2x) to amortize
  allocation cost.
- The unit quad vertex buffer is created once at init and never changes.
- `InstanceData` is `repr(C)` + `bytemuck::Pod` for direct memcpy to GPU.
- The instanced pipeline shares the same bind group layout as the existing
  sprite pipeline (camera uniforms + texture + sampler) so they can coexist
  in the same render pass.
- Per-sprite shader effects (`SpriteShader`) are not supported in the instanced
  path.  Sprites with shaders are routed to the indexed path automatically.

## Non-Goals

- Indirect rendering (`draw_indexed_indirect`) -- too complex for the current
  scope and not needed below 100k sprites.
- Compute-shader-based sprite sorting.
- 3D instancing (the engine is 2D only).
- Instanced particle rendering (particles have their own pipeline; may adopt
  instancing separately).

## Open Questions

1. Should the instancing threshold be configurable at runtime, or determined
   automatically by profiling?
2. Should `InstanceData` include a rotation field, or is axis-aligned sufficient
   for 2D pixel art?
3. Should the instance buffer be per-texture (one buffer per atlas) or global
   (one buffer with all instances, offset per batch)?
4. Is double buffering sufficient, or should we use triple buffering for
   pipelined frames?

## Referenzen

- Current batcher: `crates/amigo_render/src/sprite_batcher.rs` (218 lines)
- Vertex definition: `crates/amigo_render/src/vertex.rs`
- Post-process pipeline (wgpu patterns): `crates/amigo_render/src/post_process.rs` (617 lines)
- wgpu instancing tutorial: sotrh.github.io/learn-wgpu/beginner/tutorial7-instancing
