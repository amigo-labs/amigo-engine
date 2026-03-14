use crate::camera::Camera;
use crate::sprite_batcher::{SpriteBatch, SpriteBatcher};
use crate::texture::{Texture, TextureId};
use crate::vertex::Vertex;
use crate::{ArtStyle, SamplerMode};
use amigo_core::Color;
use rustc_hash::FxHashMap;
use tracing::{info, warn};
use wgpu::util::DeviceExt;

/// Shader source for the sprite pipeline.
const SPRITE_SHADER: &str = r#"
struct Uniforms {
    projection: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.projection * vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@group(1) @binding(0) var t_sprite: texture_2d<f32>;
@group(1) @binding(1) var s_sprite: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_sprite, s_sprite, in.uv);
    return tex_color * in.color;
}
"#;

/// The main renderer, managing GPU resources and draw calls.
pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub textures: FxHashMap<TextureId, Texture>,
    pub white_texture_id: TextureId,
    pub batcher: SpriteBatcher,
    pub camera: Camera,
    pub clear_color: Color,
    pub art_style: ArtStyle,
    next_texture_id: u32,
    draw_call_count: u32,
}

impl Renderer {
    pub async fn new(
        window: std::sync::Arc<winit::window::Window>,
        virtual_width: u32,
        virtual_height: u32,
    ) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        info!("GPU adapter: {:?}", adapter.get_info().name);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("amigo_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            }, None)
            .await
            .expect("Failed to create GPU device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Uniform buffer for projection matrix
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform_buffer"),
            contents: bytemuck::cast_slice(&[0.0f32; 16]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Texture bind group layout
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        // Render pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite_shader"),
            source: wgpu::ShaderSource::Wgsl(SPRITE_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create white fallback texture
        let white_texture = Texture::white_pixel(&device, &queue, &texture_bind_group_layout);
        let white_id = white_texture.id;
        let mut textures = FxHashMap::default();
        textures.insert(white_id, white_texture);

        let camera = Camera::new(virtual_width as f32, virtual_height as f32);

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,
            textures,
            white_texture_id: white_id,
            batcher: SpriteBatcher::new(),
            camera,
            clear_color: Color::CORNFLOWER_BLUE,
            art_style: ArtStyle::PixelArt,
            next_texture_id: 1,
            draw_call_count: 0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn load_texture(&mut self, image: &image::RgbaImage, label: &str) -> TextureId {
        self.load_texture_with_mode(image, label, self.art_style.default_sampler_mode())
    }

    /// Load a texture with a specific sampler mode (overrides the global art style).
    pub fn load_texture_with_mode(
        &mut self,
        image: &image::RgbaImage,
        label: &str,
        mode: SamplerMode,
    ) -> TextureId {
        let id = TextureId(self.next_texture_id);
        self.next_texture_id += 1;
        let texture = Texture::from_image_with_mode(
            &self.device,
            &self.queue,
            &self.texture_bind_group_layout,
            image,
            id,
            label,
            mode,
        );
        self.textures.insert(id, texture);
        id
    }

    /// Set the global art style. Affects default sampler mode for newly loaded textures.
    pub fn set_art_style(&mut self, style: ArtStyle) {
        self.art_style = style;
        self.camera.pixel_snap = style.pixel_snap();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Update projection uniform
        let proj = self.camera.projection_matrix();
        let proj_flat: [f32; 16] = unsafe { std::mem::transmute(proj) };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&proj_flat));

        // Build sprite batches
        let batches = self.batcher.build();
        self.draw_call_count = batches.len() as u32;

        // Create vertex and index buffers
        let vertex_buffer = if !self.batcher.vertices().is_empty() {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sprite_vertex_buffer"),
                        contents: bytemuck::cast_slice(self.batcher.vertices()),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            None
        };

        let index_buffer = if !self.batcher.indices().is_empty() {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sprite_index_buffer"),
                        contents: bytemuck::cast_slice(self.batcher.indices()),
                        usage: wgpu::BufferUsages::INDEX,
                    }),
            )
        } else {
            None
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sprite_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color.r as f64,
                            g: self.clear_color.g as f64,
                            b: self.clear_color.b as f64,
                            a: self.clear_color.a as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let (Some(vb), Some(ib)) = (&vertex_buffer, &index_buffer) {
                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);

                for batch in &batches {
                    if let Some(texture) = self.textures.get(&batch.texture_id) {
                        render_pass.set_bind_group(1, &texture.bind_group, &[]);
                        render_pass.draw_indexed(
                            batch.index_offset..batch.index_offset + batch.index_count,
                            0,
                            0..1,
                        );
                    } else {
                        warn!("Missing texture {:?}", batch.texture_id);
                    }
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.batcher.clear();
        Ok(())
    }

    pub fn draw_call_count(&self) -> u32 {
        self.draw_call_count
    }

    pub fn window_size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }
}
