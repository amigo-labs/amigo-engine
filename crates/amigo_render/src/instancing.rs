//! GPU Instancing: per-instance data for hardware-instanced sprite rendering.
//!
//! Provides InstanceData layout, InstancedBatch grouping, and the CPU-side
//! logic for the hybrid batching strategy. The actual wgpu buffer management
//! and draw calls are handled by the Renderer using these data structures.

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
}

// ---------------------------------------------------------------------------
// InstancedBatch
// ---------------------------------------------------------------------------

/// A batch to be drawn with hardware instancing.
#[derive(Clone, Debug)]
pub struct InstancedBatch {
    /// Texture atlas to bind for this batch.
    pub texture_id: TextureId,
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
