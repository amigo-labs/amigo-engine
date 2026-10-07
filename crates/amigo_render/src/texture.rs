use crate::SamplerMode;
use crate::mipmap::{MipSource, build_mip_chain, mip_level_count};

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Unique ID for a loaded texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub u32);

/// Hands out `TextureId`s. The renderer and the font manager share one, so a
/// font page created while `Game::draw` runs already has its final id. Ids
/// are never reused, across clones included.
#[derive(Clone, Debug)]
pub struct TextureIdAllocator {
    next: Arc<AtomicU32>,
}

impl TextureIdAllocator {
    /// An allocator whose first id is `first`.
    pub fn new(first: u32) -> Self {
        Self {
            next: Arc::new(AtomicU32::new(first)),
        }
    }

    /// A fresh id.
    pub fn allocate(&self) -> TextureId {
        TextureId(self.next.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for TextureIdAllocator {
    /// Starts at 1: id 0 is the renderer's white texture.
    fn default() -> Self {
        Self::new(1)
    }
}

/// A GPU texture with its bind group.
pub struct Texture {
    pub id: TextureId,
    pub gpu_texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
    pub sampler_mode: SamplerMode,
}

impl Texture {
    /// Create a texture with the default pixel-art (nearest-neighbor) sampler.
    pub fn from_image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        image: &image::RgbaImage,
        id: TextureId,
        label: &str,
    ) -> Self {
        Self::from_image_with_mode(
            device,
            queue,
            bind_group_layout,
            image,
            id,
            label,
            SamplerMode::Nearest,
        )
    }

    /// Create a texture with a specific sampler mode and a single level.
    pub fn from_image_with_mode(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        image: &image::RgbaImage,
        id: TextureId,
        label: &str,
        mode: SamplerMode,
    ) -> Self {
        Self::from_image_mipped(
            device,
            queue,
            bind_group_layout,
            image,
            id,
            label,
            mode,
            MipSource::MultiFrame,
        )
    }

    /// Create a texture with as many mip levels as `source` allows under
    /// `mode` (see [`mip_level_count`]). Levels are built on the CPU from
    /// premultiplied texels in linear light.
    #[expect(
        clippy::too_many_arguments,
        reason = "from_image_with_mode plus the mip source"
    )]
    pub fn from_image_mipped(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        image: &image::RgbaImage,
        id: TextureId,
        label: &str,
        mode: SamplerMode,
        source: MipSource,
    ) -> Self {
        // Every sprite blend mode expects premultiplied texels.
        let mut premultiplied;
        let image = if image.pixels().any(|p| p.0[3] != 255) {
            premultiplied = image.clone();
            crate::blend::premultiply_srgb(&mut premultiplied);
            &premultiplied
        } else {
            image
        };
        let levels = mip_level_count(source, mode, image.dimensions());
        let chain = build_mip_chain(image, levels);
        let mut data: Vec<&[u8]> = vec![image.as_raw()];
        data.extend(chain.iter().map(|level| level.as_raw().as_slice()));
        Self::create(
            device,
            queue,
            bind_group_layout,
            &data,
            image.dimensions(),
            id,
            label,
            mode,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the shared body of the public constructors"
    )]
    fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        levels: &[&[u8]],
        (width, height): (u32, u32),
        id: TextureId,
        label: &str,
        mode: SamplerMode,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let gpu_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: levels.len().max(1) as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        for (level, data) in levels.iter().enumerate() {
            let (w, h) = ((width >> level).max(1), (height >> level).max(1));
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu_texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = gpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let filter = mode.to_wgpu();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            // Between levels, linearly, when there are any.
            mipmap_filter: if levels.len() > 1 {
                wgpu::MipmapFilterMode::Linear
            } else {
                mode.to_wgpu_mipmap()
            },
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label}_bind_group")),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            id,
            gpu_texture,
            view,
            sampler,
            bind_group,
            width,
            height,
            sampler_mode: mode,
        }
    }

    /// Create a texture from RGBA8 data that is already premultiplied (font
    /// pages); unlike [`from_image_with_mode`](Self::from_image_with_mode) it
    /// is uploaded as is.
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors from_image_with_mode with the image split into data and size"
    )]
    pub fn from_premultiplied(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
        data: &[u8],
        (width, height): (u32, u32),
        id: TextureId,
        label: &str,
        mode: SamplerMode,
    ) -> Option<Self> {
        if width == 0 || height == 0 || data.len() != (width * height * 4) as usize {
            return None;
        }
        Some(Self::create(
            device,
            queue,
            bind_group_layout,
            &[data],
            (width, height),
            id,
            label,
            mode,
        ))
    }

    /// Overwrite rows `start..end` with the same rows of `data`, a full
    /// premultiplied RGBA8 image of this texture's size. Out-of-range rows are
    /// clamped; a mismatched `data` length writes nothing.
    pub fn write_rows(&mut self, queue: &wgpu::Queue, data: &[u8], start: u32, end: u32) {
        let end = end.min(self.height);
        let row_bytes = (4 * self.width) as usize;
        if start >= end || data.len() != row_bytes * self.height as usize {
            return;
        }
        let rows = &data[start as usize * row_bytes..end as usize * row_bytes];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.gpu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: start,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            rows,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.width),
                rows_per_image: Some(end - start),
            },
            wgpu::Extent3d {
                width: self.width,
                height: end - start,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Create a 1x1 white fallback texture.
    pub fn white_pixel(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        Self::from_image(
            device,
            queue,
            bind_group_layout,
            &img,
            TextureId(0),
            "white_pixel",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_allocator_never_repeats_an_id_across_clones() {
        let a = TextureIdAllocator::default();
        let b = a.clone();
        let mut seen = std::collections::HashSet::new();
        for i in 0..100 {
            let id = if i % 2 == 0 {
                a.allocate()
            } else {
                b.allocate()
            };
            assert!(seen.insert(id), "{id:?} handed out twice");
        }
        assert!(!seen.contains(&TextureId(0)));
    }
}
