//! The scene target and the pass that scales it into the window.
//!
//! Every stage (sprites, lighting, post-processing, UI) renders into
//! [`BlitPipeline::target_view`], an offscreen texture at the virtual
//! resolution for pixel art. The last pass of a frame clears the surface to
//! the letterbox colour and draws that texture into the [`Viewport`] with a
//! nearest-neighbour sampler. Before this, the projection mapped the virtual
//! resolution straight onto the window, so any window that was not an exact
//! multiple stretched the image and gave virtual pixels uneven sizes.

use crate::post_process::FULLSCREEN_VERTEX_SHADER;
use crate::viewport::Viewport;
use amigo_core::Color;

/// Fragment shader of the blit pass: one texture sample, no processing.
///
/// Public so tests can validate it with `naga` without a GPU.
pub const BLIT_FRAGMENT_SHADER: &str = r#"
@group(0) @binding(0) var t_scene: texture_2d<f32>;
@group(0) @binding(1) var s_scene: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_scene, s_scene, in.uv);
}
"#;

/// Owns the scene render target and the pipeline that draws it to the window.
pub struct BlitPipeline {
    target: wgpu::Texture,
    target_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
    size: (u32, u32),
}

impl BlitPipeline {
    /// `format` is the surface format: the target uses it too, so every stage
    /// that used to draw to the surface can draw to the target unchanged.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let size = (size.0.max(1), size.1.max(1));

        // The vertex shader declares `VertexOutput`, which the fragment
        // shader reads, so both go into one module.
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_shader"),
            source: wgpu::ShaderSource::Wgsl(
                format!("{FULLSCREEN_VERTEX_SHADER}\n{BLIT_FRAGMENT_SHADER}").into(),
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Nearest: each virtual pixel becomes a hard-edged block. A raster-art
        // target is already viewport-sized, so the filter does not matter there.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let (target, target_view) = create_target(device, format, size);
        let bind_group = create_bind_group(device, &bind_group_layout, &target_view, &sampler);

        Self {
            target,
            target_view,
            bind_group,
            bind_group_layout,
            sampler,
            pipeline,
            format,
            size,
        }
    }

    /// The texture every scene stage renders into.
    pub fn target_view(&self) -> &wgpu::TextureView {
        &self.target_view
    }

    /// The scene texture itself, for read-back (screenshots).
    pub fn target(&self) -> &wgpu::Texture {
        &self.target
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Recreate the target at `size`. A no-op when the size is unchanged, so
    /// callers can call it every frame.
    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let size = (size.0.max(1), size.1.max(1));
        if size == self.size {
            return;
        }
        let (target, target_view) = create_target(device, self.format, size);
        self.bind_group =
            create_bind_group(device, &self.bind_group_layout, &target_view, &self.sampler);
        self.target = target;
        self.target_view = target_view;
        self.size = size;
    }

    /// Clear `surface` to `letterbox` and draw the scene target into
    /// `viewport`.
    pub fn apply(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        surface: &wgpu::TextureView,
        viewport: Viewport,
        letterbox: Color,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: letterbox.r as f64,
                        g: letterbox.g as f64,
                        b: letterbox.b as f64,
                        a: letterbox.a as f64,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(
            viewport.x,
            viewport.y,
            viewport.width,
            viewport.height,
            0.0,
            1.0,
        );
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn create_target(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    (width, height): (u32, u32),
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blit_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
