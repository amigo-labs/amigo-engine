//! GPU Instancing: per-instance data for hardware-instanced sprite rendering.
//!
//! A run of sprites that share a texture and blend mode, is at least
//! `Renderer::instancing_threshold` long, and holds only unrotated sprites
//! without explicit geometry or per-sprite shaders is drawn with one
//! instanced call over a unit quad (`SpriteBatcher::build_hybrid`). Other runs
//! keep the indexed path. Tile layers, which draw hundreds of same-texture
//! tiles, are the typical instanced run.

use crate::blend::BlendMode;
use crate::sprite_batcher::SpriteInstance;
use crate::texture::TextureId;

// ---------------------------------------------------------------------------
// InstanceData
// ---------------------------------------------------------------------------

/// Per-instance data uploaded to the GPU instance buffer.
/// Matches the WGSL vertex input layout for instanced sprite rendering.
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
    /// Z-order for depth sorting.
    pub z_order: f32,
    /// Padding to align to 16 bytes.
    pub _pad: [f32; 2],
}

impl InstanceData {
    /// Create instance data for a sprite.
    #[expect(
        clippy::too_many_arguments,
        reason = "flat parameter list mirrors the immediate-mode call site; a params struct would be a breaking change"
    )]
    pub fn new(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv_x: f32,
        uv_y: f32,
        uv_w: f32,
        uv_h: f32,
        tint: [f32; 4],
        flip_x: bool,
        flip_y: bool,
        z_order: f32,
    ) -> Self {
        let flags = (flip_x as u32) | ((flip_y as u32) << 1);
        Self {
            transform: [x, y, w, h],
            uv_rect: [uv_x, uv_y, uv_w, uv_h],
            tint,
            flags,
            z_order,
            _pad: [0.0; 2],
        }
    }

    /// Size of one instance in bytes.
    pub const SIZE: usize = std::mem::size_of::<Self>();

    /// The instance for `sprite`, or `None` when it needs the indexed path:
    /// a rotation, explicit geometry or per-sprite shaders. The pivot is
    /// folded into the position.
    pub fn from_sprite(sprite: &SpriteInstance) -> Option<Self> {
        let rotation = if sprite.rotation.is_finite() {
            sprite.rotation
        } else {
            0.0
        };
        if rotation != 0.0 || sprite.geometry.is_some() || !sprite.shaders.is_empty() {
            return None;
        }
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        Some(Self::new(
            sprite.x - finite(sprite.origin[0]),
            sprite.y - finite(sprite.origin[1]),
            sprite.width,
            sprite.height,
            sprite.uv_x,
            sprite.uv_y,
            sprite.uv_w,
            sprite.uv_h,
            sprite.tint.to_array(),
            sprite.flip_x,
            sprite.flip_y,
            sprite.z_order as f32,
        ))
    }

    /// The instance buffer layout: `transform`, `uv_rect`, `tint`, `flags`
    /// and `z_order` at shader locations 1 to 5, one step per instance.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 5] = [
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: wgpu::VertexFormat::Float32x4,
            },
            wgpu::VertexAttribute {
                offset: 48,
                shader_location: 4,
                format: wgpu::VertexFormat::Uint32,
            },
            wgpu::VertexAttribute {
                offset: 52,
                shader_location: 5,
                format: wgpu::VertexFormat::Float32,
            },
        ];
        wgpu::VertexBufferLayout {
            array_stride: Self::SIZE as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBUTES,
        }
    }
}

/// The unit quad every instance is drawn over: corners (0,0), (1,0), (1,1),
/// (0,1), two triangles.
pub const QUAD_CORNERS: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
/// Indices of the unit quad.
pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

/// The unit quad's vertex layout: one `vec2<f32>` at location 0.
pub fn quad_desc() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x2,
    }];
    wgpu::VertexBufferLayout {
        array_stride: 8,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBUTES,
    }
}

/// The instanced sprite shader. Same bind groups as the indexed sprite
/// shader (projection, then texture and sampler), and the same premultiplied
/// tint, so the two paths can share a pass.
pub const INSTANCED_SPRITE_SHADER: &str = r#"
struct Uniforms {
    projection: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) quad: vec2<f32>,
    @location(1) transform: vec4<f32>,
    @location(2) uv_rect: vec4<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) flags: u32,
    @location(5) z_order: f32,
) -> VertexOutput {
    // Flips swap UVs on the same quad, as the indexed path does.
    var u = quad.x;
    var v = quad.y;
    if (flags & 1u) != 0u { u = 1.0 - u; }
    if (flags & 2u) != 0u { v = 1.0 - v; }
    let world = vec2<f32>(
        transform.x + quad.x * transform.z,
        transform.y + quad.y * transform.w,
    );
    var out: VertexOutput;
    out.clip_position = uniforms.projection * vec4<f32>(world, 0.0, 1.0);
    out.uv = vec2<f32>(uv_rect.x + u * uv_rect.z, uv_rect.y + v * uv_rect.w);
    out.color = tint;
    return out;
}

@group(1) @binding(0) var t_sprite: texture_2d<f32>;
@group(1) @binding(1) var s_sprite: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_sprite, s_sprite, in.uv);
    let tint = vec4<f32>(in.color.rgb * in.color.a, in.color.a);
    return tex_color * tint;
}
"#;

// ---------------------------------------------------------------------------
// InstanceBuffer
// ---------------------------------------------------------------------------

/// The capacity, in instances, a buffer of `capacity` grows to so that it
/// holds `needed`: doubled until it fits, never below 64.
pub fn grown_capacity(capacity: u32, needed: u32) -> u32 {
    let mut capacity = capacity.max(64);
    while capacity < needed {
        capacity = capacity.saturating_mul(2);
    }
    capacity
}

/// A GPU buffer of per-instance data, double-buffered so that a frame never
/// writes the buffer the previous frame's draw reads. Grows geometrically.
pub struct InstanceBuffer {
    buffers: [wgpu::Buffer; 2],
    capacities: [u32; 2],
    current: usize,
    count: u32,
}

impl InstanceBuffer {
    pub fn new(device: &wgpu::Device, initial_capacity: u32) -> Self {
        let capacity = grown_capacity(initial_capacity, 0);
        Self {
            buffers: [
                Self::create(device, capacity),
                Self::create(device, capacity),
            ],
            capacities: [capacity; 2],
            current: 0,
            count: 0,
        }
    }

    fn create(device: &wgpu::Device, capacity: u32) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_instances"),
            size: capacity as u64 * InstanceData::SIZE as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Write instance data for this frame. Grows the buffer if needed.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &[InstanceData],
    ) {
        let needed = instances.len() as u32;
        if needed > self.capacities[self.current] {
            let capacity = grown_capacity(self.capacities[self.current], needed);
            self.buffers[self.current] = Self::create(device, capacity);
            self.capacities[self.current] = capacity;
        }
        if !instances.is_empty() {
            queue.write_buffer(
                &self.buffers[self.current],
                0,
                bytemuck::cast_slice(instances),
            );
        }
        self.count = needed;
    }

    /// Current buffer for binding.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffers[self.current]
    }

    /// Number of instances written this frame.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Swap to the other buffer (call at the end of the frame).
    pub fn flip(&mut self) {
        self.current = 1 - self.current;
    }
}

// ---------------------------------------------------------------------------
// InstancedBatch
// ---------------------------------------------------------------------------

/// A batch to be drawn with hardware instancing.
#[derive(Clone, Debug, PartialEq)]
pub struct InstancedBatch {
    /// Texture atlas to bind for this batch.
    pub texture_id: TextureId,
    /// Every instance blends this way.
    pub blend: BlendMode,
    /// Offset into the instance buffer (in instances, not bytes).
    pub instance_offset: u32,
    /// Number of instances to draw.
    pub instance_count: u32,
}

// ---------------------------------------------------------------------------
// Hybrid Batching
// ---------------------------------------------------------------------------

/// Default threshold: batches with >= this many sprites use instancing.
pub const DEFAULT_INSTANCING_THRESHOLD: u32 = 64;

/// Partition sprites into instanced and non-instanced batches.
/// `sprites` is a pre-sorted list of (texture_id, has_shader, InstanceData).
/// Returns (instanced_batches, indexed_sprite_indices) where indexed_sprite_indices
/// are the indices of sprites that should use the traditional indexed path.
pub fn partition_batches(
    sprites: &[(TextureId, bool, InstanceData)],
    threshold: u32,
) -> (Vec<InstancedBatch>, Vec<usize>) {
    let mut instanced = Vec::new();
    let mut indexed_indices = Vec::new();
    let mut instance_data_offset = 0u32;

    let mut i = 0;
    while i < sprites.len() {
        let (tex, _has_shader, _) = &sprites[i];

        // Collect contiguous sprites with the same texture
        let batch_start = i;
        while i < sprites.len() && sprites[i].0 == *tex {
            i += 1;
        }
        let batch_size = (i - batch_start) as u32;

        // Sprites with per-sprite shaders always use indexed path
        let any_shader = sprites[batch_start..i].iter().any(|(_, s, _)| *s);

        if !any_shader && batch_size >= threshold {
            instanced.push(InstancedBatch {
                texture_id: *tex,
                blend: BlendMode::Normal,
                instance_offset: instance_data_offset,
                instance_count: batch_size,
            });
            instance_data_offset += batch_size;
        } else {
            for j in batch_start..i {
                indexed_indices.push(j);
            }
        }
    }

    (instanced, indexed_indices)
}

/// Collect InstanceData from the instanced batches.
pub fn collect_instance_data(
    sprites: &[(TextureId, bool, InstanceData)],
    batches: &[InstancedBatch],
) -> Vec<InstanceData> {
    let total: usize = batches.iter().map(|b| b.instance_count as usize).sum();
    let mut data = Vec::with_capacity(total);

    // Rebuild from sprites based on batch boundaries
    let mut sprite_idx = 0;
    for batch in batches {
        // Find sprites for this batch (they are contiguous and same texture)
        while sprite_idx < sprites.len() && sprites[sprite_idx].0 != batch.texture_id {
            sprite_idx += 1;
        }
        for _ in 0..batch.instance_count {
            if sprite_idx < sprites.len() {
                data.push(sprites[sprite_idx].2);
                sprite_idx += 1;
            }
        }
    }

    data
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sprite(tex: u32, has_shader: bool) -> (TextureId, bool, InstanceData) {
        (
            TextureId(tex),
            has_shader,
            InstanceData::new(
                0.0, 0.0, 16.0, 16.0, 0.0, 0.0, 1.0, 1.0, [1.0; 4], false, false, 0.0,
            ),
        )
    }

    #[test]
    fn the_shader_validates() {
        let module = naga::front::wgsl::parse_str(INSTANCED_SPRITE_SHADER).expect("parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&module)
        .expect("validates");
    }

    #[test]
    fn the_instance_layout_matches_the_struct() {
        let data = InstanceData::new(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, [9.0; 4], true, false, 10.0,
        );
        let bytes = bytemuck::bytes_of(&data);
        let desc = InstanceData::desc();
        assert_eq!(desc.array_stride, 64);
        let at = |i: usize| desc.attributes[i].offset as usize;
        assert_eq!(
            f32::from_le_bytes(bytes[at(0)..at(0) + 4].try_into().unwrap_or_default()),
            1.0
        );
        assert_eq!(
            f32::from_le_bytes(bytes[at(1)..at(1) + 4].try_into().unwrap_or_default()),
            5.0
        );
        assert_eq!(
            f32::from_le_bytes(bytes[at(2)..at(2) + 4].try_into().unwrap_or_default()),
            9.0
        );
        assert_eq!(
            u32::from_le_bytes(bytes[at(3)..at(3) + 4].try_into().unwrap_or_default()),
            1
        );
        assert_eq!(
            f32::from_le_bytes(bytes[at(4)..at(4) + 4].try_into().unwrap_or_default()),
            10.0
        );
    }

    #[test]
    fn buffers_grow_geometrically() {
        assert_eq!(grown_capacity(0, 10), 64);
        assert_eq!(grown_capacity(64, 65), 128);
        assert_eq!(grown_capacity(128, 1000), 1024);
        assert_eq!(grown_capacity(1024, 10), 1024);
    }

    #[test]
    fn only_plain_sprites_become_instances() {
        let mut s = SpriteInstance::new(TextureId(1), 10.0, 20.0, 4.0, 4.0);
        s.origin = [2.0, 1.0];
        let inst = InstanceData::from_sprite(&s).expect("plain");
        assert_eq!(inst.transform, [8.0, 19.0, 4.0, 4.0]);
        s.rotation = 0.5;
        assert!(InstanceData::from_sprite(&s).is_none());
        s.rotation = f32::NAN;
        assert!(
            InstanceData::from_sprite(&s).is_some(),
            "NaN rotation renders as 0"
        );
    }

    #[test]
    fn instance_data_size() {
        assert_eq!(InstanceData::SIZE, 64); // 16 floats * 4 bytes = 64
    }

    #[test]
    fn small_batch_uses_indexed() {
        let sprites: Vec<_> = (0..10).map(|_| make_sprite(1, false)).collect();
        let (instanced, indexed) = partition_batches(&sprites, 64);
        assert!(instanced.is_empty());
        assert_eq!(indexed.len(), 10);
    }

    #[test]
    fn large_batch_uses_instancing() {
        let sprites: Vec<_> = (0..100).map(|_| make_sprite(1, false)).collect();
        let (instanced, indexed) = partition_batches(&sprites, 64);
        assert_eq!(instanced.len(), 1);
        assert_eq!(instanced[0].instance_count, 100);
        assert!(indexed.is_empty());
    }

    #[test]
    fn shader_sprites_use_indexed() {
        let sprites: Vec<_> = (0..100).map(|_| make_sprite(1, true)).collect();
        let (instanced, indexed) = partition_batches(&sprites, 64);
        assert!(instanced.is_empty());
        assert_eq!(indexed.len(), 100);
    }

    #[test]
    fn mixed_textures_separate_batches() {
        let mut sprites = Vec::new();
        for _ in 0..80 {
            sprites.push(make_sprite(1, false));
        }
        for _ in 0..80 {
            sprites.push(make_sprite(2, false));
        }
        let (instanced, indexed) = partition_batches(&sprites, 64);
        assert_eq!(instanced.len(), 2);
        assert_eq!(instanced[0].instance_count, 80);
        assert_eq!(instanced[1].instance_count, 80);
        assert!(indexed.is_empty());
    }

    #[test]
    fn flip_flags_pack_correctly() {
        let inst = InstanceData::new(
            0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, [1.0; 4], true, true, 0.0,
        );
        assert_eq!(inst.flags, 3); // bit 0 + bit 1
        let inst2 = InstanceData::new(
            0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, [1.0; 4], true, false, 0.0,
        );
        assert_eq!(inst2.flags, 1); // bit 0 only
    }

    #[test]
    fn collect_instance_data_matches_batches() {
        let sprites: Vec<_> = (0..100)
            .map(|i| {
                let mut s = make_sprite(1, false);
                s.2.z_order = i as f32;
                s
            })
            .collect();
        let (batches, _) = partition_batches(&sprites, 64);
        let data = collect_instance_data(&sprites, &batches);
        assert_eq!(data.len(), 100);
        assert!((data[0].z_order - 0.0).abs() < 0.01);
        assert!((data[99].z_order - 99.0).abs() < 0.01);
    }
}
