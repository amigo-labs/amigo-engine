//! GPU side of the lighting stage.
//!
//! [`crate::lighting::LightingState`] has always been able to collect ambient and
//! point lights and pack them for the GPU, but there was no shader, no pipeline
//! and no pass — `build_uniform_data()` produced bytes that nothing uploaded.
//! Light was purely a CPU-side bookkeeping exercise.
//!
//! This is a fullscreen composite: the scene is rendered into an offscreen
//! target, then multiplied by ambient plus the accumulated point-light
//! contribution. It sits between the sprite pass and post-processing, matching
//! the stage order in docs/specs/conventions.md A.6.

use crate::lighting::{LightData, LightingHeader, LightingState};
use amigo_core::Rect;
use wgpu::util::DeviceExt;

/// Fixed capacity of the light uniform buffer.
///
/// Uniform buffers need a compile-time size, so the array is always this long and
/// the header's `light_count` says how much of it to read. 64 lights is
/// `LightingState::max_lights`' default; at 48 bytes each that is ~3 KiB, well
/// inside the 64 KiB uniform binding limit.
pub const MAX_LIGHTS: usize = 64;

/// Fullscreen vertex shader: a single triangle covering the viewport.
///
/// Public so tests can validate it with `naga` — the only way to catch a WGSL
/// error without a GPU.
pub const LIGHTING_VERTEX_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    // Oversized triangle; the rasterizer clips it to the viewport.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    let pos = positions[index];
    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    // Flip y: clip space is y-up, texture space is y-down.
    out.uv = vec2<f32>((pos.x + 1.0) * 0.5, 1.0 - (pos.y + 1.0) * 0.5);
    return out;
}
"#;

/// Multiplies the scene by ambient light plus point-light contributions.
///
/// Public so tests can validate it with `naga`.
pub const LIGHTING_FRAGMENT_SHADER: &str = r#"
struct Light {
    color: vec4<f32>,
    position: vec2<f32>,
    radius: f32,
    intensity: f32,
    falloff: f32,
};

struct Lighting {
    ambient_color: vec4<f32>,
    view_size: vec2<f32>,
    light_count: u32,
    _padding: u32,
    lights: array<Light, 64>,
};

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> lighting: Lighting;

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_texture, scene_sampler, uv);

    // Light positions are view-local in world units, and so is this: the visible
    // rect's size times the fragment's normalized position.
    let frag_pos = uv * lighting.view_size;

    var accum = lighting.ambient_color.rgb;
    let count = min(lighting.light_count, 64u);
    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let light = lighting.lights[i];
        if (light.radius <= 0.0) {
            continue;
        }
        let dist = distance(frag_pos, light.position);
        // Linear falloff raised to `falloff`: 1 at the centre, 0 at the radius.
        let normalized = clamp(1.0 - dist / light.radius, 0.0, 1.0);
        let attenuation = pow(normalized, max(light.falloff, 0.0001)) * light.intensity;
        accum = accum + light.color.rgb * light.color.a * attenuation;
    }

    return vec4<f32>(scene.rgb * accum, scene.a);
}
"#;

/// Offscreen target plus the pipeline that composites lighting onto it.
pub struct LightingPipeline {
    scene_texture: wgpu::Texture,
    scene_view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
}

impl LightingPipeline {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let (scene_texture, scene_view) = Self::create_target(device, width, height, format);

        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lighting_vertex"),
            source: wgpu::ShaderSource::Wgsl(LIGHTING_VERTEX_SHADER.into()),
        });
        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lighting_fragment"),
            source: wgpu::ShaderSource::Wgsl(LIGHTING_FRAGMENT_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lighting_bind_group_layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lighting_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lighting_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_module,
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
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lighting_sampler"),
            // Nearest: the scene is pixel art, and a filtered read would soften
            // it before post-processing ever sees it.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniform_size =
            std::mem::size_of::<LightingHeader>() + MAX_LIGHTS * std::mem::size_of::<LightData>();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lighting_uniform_buffer"),
            contents: &vec![0u8; uniform_size],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            scene_texture,
            scene_view,
            format,
            width,
            height,
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
        }
    }

    /// Render target the sprite pass should draw into when lighting is active.
    pub fn scene_view(&self) -> &wgpu::TextureView {
        &self.scene_view
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }
        let (texture, view) = Self::create_target(device, width, height, self.format);
        self.scene_texture = texture;
        self.scene_view = view;
        self.width = width;
        self.height = height;
    }

    /// Composite the scene target through `state` onto `output_view`.
    ///
    /// `view` is the camera's visible world rect: light positions are given in
    /// world space and converted to view-local here, so the shader can compare
    /// them against fragment positions in the same units. Radii need no scaling
    /// because both sides are world units.
    pub fn apply(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        state: &LightingState,
        view: Rect,
        output_view: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.uniform_buffer, 0, &self.pack(state, view));

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lighting_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lighting_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Build the uniform payload for `state`. See [`pack_lighting`].
    pub fn pack(&self, state: &LightingState, view: Rect) -> Vec<u8> {
        pack_lighting(state, view)
    }
}

/// Build the lighting uniform payload: header, then a fixed-length light array.
///
/// A free function so tests can exercise the real packing without a GPU device —
/// packing is exactly where a struct-layout mistake hides, and a mirrored copy in
/// the test would be free to drift away from this one.
pub fn pack_lighting(state: &LightingState, view: Rect) -> Vec<u8> {
    let count = state.lights.len().min(MAX_LIGHTS.min(state.max_lights));
    let ambient = &state.ambient;

    let header = LightingHeader {
        ambient_color: [
            ambient.color.r * ambient.intensity,
            ambient.color.g * ambient.intensity,
            ambient.color.b * ambient.intensity,
            ambient.color.a,
        ],
        view_size: [view.w.max(1.0), view.h.max(1.0)],
        light_count: count as u32,
        _padding: 0,
    };

    let mut buf = Vec::with_capacity(
        std::mem::size_of::<LightingHeader>() + MAX_LIGHTS * std::mem::size_of::<LightData>(),
    );
    buf.extend_from_slice(bytemuck::bytes_of(&header));

    for light in state.lights.iter().take(count) {
        let data = LightData {
            color: [light.color.r, light.color.g, light.color.b, light.color.a],
            // World space to view-local, matching the shader's frag_pos.
            position: [light.position.0 - view.x, light.position.1 - view.y],
            radius: light.radius,
            intensity: light.intensity,
            falloff: light.falloff,
            _pad: [0.0; 3],
        };
        buf.extend_from_slice(bytemuck::bytes_of(&data));
    }

    // The uniform buffer has a fixed length; zero-fill the unused tail so a
    // stale light from a previous frame cannot be read back.
    buf.resize(
        std::mem::size_of::<LightingHeader>() + MAX_LIGHTS * std::mem::size_of::<LightData>(),
        0,
    );
    buf
}

impl LightingPipeline {
    fn create_target(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lighting_scene_target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}
