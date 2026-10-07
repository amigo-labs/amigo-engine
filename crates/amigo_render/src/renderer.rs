use crate::blend::BlendMode;
use crate::blit::BlitPipeline;
use crate::camera::Camera;
use crate::font::{FONT_PAGE_SIZE, FontPage};
use crate::lighting::LightingState;
use crate::lighting_pipeline::LightingPipeline;
use crate::post_process::PostProcessPipeline;
use crate::sprite_batcher::SpriteBatcher;
use crate::texture::{Texture, TextureId, TextureIdAllocator};
use crate::vertex::Vertex;
use crate::viewport::{ScaleMode, Viewport};
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
    // Textures are stored premultiplied; premultiply the tint to match, so
    // every blend mode composites with the same (One, ...) source factor.
    let tex_color = textureSample(t_sprite, s_sprite, in.uv);
    let tint = vec4<f32>(in.color.rgb * in.color.a, in.color.a);
    return tex_color * tint;
}
"#;

/// The main renderer, managing GPU resources and draw calls.
pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    /// One sprite pipeline per [`BlendMode`], indexed by [`BlendMode::index`].
    /// The world and UI passes share them: both render into scene-format
    /// targets with the same layout.
    pub pipelines: [wgpu::RenderPipeline; 3],
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
    /// Screen-space projection for the UI pass.
    ui_uniform_buffer: wgpu::Buffer,
    ui_uniform_bind_group: wgpu::BindGroup,
    /// Sprites for the UI pass, drawn after post-processing in screen space.
    ///
    /// Per docs/specs/conventions.md A.6 the UI stage sits after the
    /// post-processing stack, so UI cannot share the world batch.
    pub ui_batcher: SpriteBatcher,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub textures: FxHashMap<TextureId, Texture>,
    pub white_texture_id: TextureId,
    pub batcher: SpriteBatcher,
    pub camera: Camera,
    pub clear_color: Color,
    pub art_style: ArtStyle,
    /// Post-processing chain. Inert until effects are set; when it has any, the
    /// sprite pass renders into its offscreen target and it composites to the
    /// surface. Nothing consumed this pipeline before — it was constructible and
    /// complete, but no render path ever ran it.
    pub post_process: PostProcessPipeline,
    /// Lighting composite. Runs between the sprite pass and post-processing, and
    /// only when `lighting.is_active()` — neutral ambient with no lights would
    /// cost a fullscreen pass to multiply by 1.0.
    pub lighting_pipeline: LightingPipeline,
    /// Ambient and point lights for this frame.
    pub lighting: LightingState,
    /// Colour of the bars around the scene when the window's aspect ratio or
    /// size does not match the scale mode exactly.
    pub letterbox_color: Color,
    /// The scene target every stage renders into, and the pass that scales it
    /// into [`Renderer::viewport`].
    blit: BlitPipeline,
    scale_mode: ScaleMode,
    viewport: Viewport,
    texture_ids: TextureIdAllocator,
    draw_call_count: u32,
}

/// Why [`Renderer::begin_frame`] produced no frame: the failure cases of
/// [`wgpu::CurrentSurfaceTexture`]. The frame's queued sprites are dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceError {
    /// Acquiring the next frame timed out. Skip it and try again.
    Timeout,
    /// The window is occluded (e.g. minimized). Skip frames until it is shown.
    Occluded,
    /// The surface changed underneath its configuration; call
    /// [`Renderer::resize`] before the next frame.
    Outdated,
    /// The surface was lost. Reconfiguring with [`Renderer::resize`] is the
    /// recovery this renderer offers; recreating it needs a new window surface.
    Lost,
    /// wgpu raised a validation error while acquiring the frame.
    Validation,
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Timeout => "timed out acquiring the next surface texture",
            Self::Occluded => "the window is occluded",
            Self::Outdated => "the surface configuration is outdated",
            Self::Lost => "the surface was lost",
            Self::Validation => "validation error while acquiring the surface texture",
        })
    }
}

impl std::error::Error for SurfaceError {}

impl SurfaceError {
    /// Split wgpu's acquire result into a texture or the reason there is none.
    ///
    /// A suboptimal texture is still used, as wgpu 24 did: the window's next
    /// resize event reconfigures the surface anyway.
    fn acquire(current: wgpu::CurrentSurfaceTexture) -> Result<wgpu::SurfaceTexture, Self> {
        use wgpu::CurrentSurfaceTexture as Current;
        match current {
            Current::Success(texture) | Current::Suboptimal(texture) => Ok(texture),
            Current::Timeout => Err(Self::Timeout),
            Current::Occluded => Err(Self::Occluded),
            Current::Outdated => Err(Self::Outdated),
            Current::Lost => Err(Self::Lost),
            Current::Validation => Err(Self::Validation),
        }
    }
}

/// A frame in progress. Holds the command encoder and surface output so
/// additional render passes (e.g. egui overlay) can be appended before submit.
pub struct FrameInProgress {
    pub encoder: wgpu::CommandEncoder,
    pub view: wgpu::TextureView,
    pub output: wgpu::SurfaceTexture,
}

impl Renderer {
    pub async fn new(
        window: std::sync::Arc<winit::window::Window>,
        virtual_width: u32,
        virtual_height: u32,
    ) -> Self {
        let size = window.inner_size();

        // GLES needs the display connection up front to present on Wayland.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(window.clone()))
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        info!("GPU adapter: {:?}", adapter.get_info().name);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("amigo_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
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
            color_space: wgpu::SurfaceColorSpace::Auto,
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

        // A second projection for the UI pass. UI coordinates are screen space in
        // virtual-resolution units, so it must not carry the camera's translation
        // or zoom — a world-space HUD would scroll and scale with the camera.
        let ui_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ui_uniform_buffer"),
            contents: bytemuck::cast_slice(&[0.0f32; 16]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let ui_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ui_uniform_buffer.as_entire_binding(),
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
            bind_group_layouts: &[
                Some(&uniform_bind_group_layout),
                Some(&texture_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let pipelines = BlendMode::ALL.map(|mode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("sprite_pipeline_{mode:?}")),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(Vertex::desc())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(mode.blend_state()),
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
                multiview_mask: None,
                cache: None,
            })
        });

        // Create white fallback texture
        let white_texture = Texture::white_pixel(&device, &queue, &texture_bind_group_layout);
        let white_id = white_texture.id;
        let mut textures = FxHashMap::default();
        textures.insert(white_id, white_texture);

        let camera = Camera::new(virtual_width as f32, virtual_height as f32);

        // Every stage renders at the virtual resolution (the default pixel-art
        // style); the blit pass scales the result into the window.
        let scale_mode = ScaleMode::default();
        let virtual_size = (virtual_width.max(1), virtual_height.max(1));
        let viewport = Viewport::compute(
            scale_mode,
            virtual_size,
            (surface_config.width, surface_config.height),
        );
        let blit = BlitPipeline::new(&device, surface_format, virtual_size);
        let post_process = PostProcessPipeline::new(
            &device,
            virtual_size.0,
            virtual_size.1,
            surface_config.format,
        );
        let lighting_pipeline = LightingPipeline::new(
            &device,
            virtual_size.0,
            virtual_size.1,
            surface_config.format,
        );

        Self {
            device,
            queue,
            surface,
            surface_config,
            pipelines,
            uniform_buffer,
            uniform_bind_group,
            ui_uniform_buffer,
            ui_uniform_bind_group,
            ui_batcher: SpriteBatcher::new(),
            texture_bind_group_layout,
            textures,
            white_texture_id: white_id,
            batcher: SpriteBatcher::new(),
            camera,
            clear_color: Color::CORNFLOWER_BLUE,
            art_style: ArtStyle::PixelArt,
            post_process,
            lighting_pipeline,
            lighting: LightingState::new(),
            letterbox_color: Color::BLACK,
            blit,
            scale_mode,
            viewport,
            texture_ids: TextureIdAllocator::new(white_id.0 + 1),
            draw_call_count: 0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            // Pixel-art targets stay at the virtual resolution; only the
            // viewport moves. Raster-art targets follow the viewport size.
            self.sync_scene_targets();
        }
    }

    /// How the scene is scaled into the window.
    pub fn scale_mode(&self) -> ScaleMode {
        self.scale_mode
    }

    /// Set how the scene is scaled into the window (`[render] scale_mode`).
    pub fn set_scale_mode(&mut self, mode: ScaleMode) {
        self.scale_mode = mode;
        self.sync_scene_targets();
    }

    /// The window-pixel rectangle the scene is drawn into. Input maps window
    /// positions through it with [`Viewport::window_to_virtual`].
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Size of the scene target: the virtual resolution for pixel art, so a
    /// virtual pixel is one texel and scales up as a hard-edged block; the
    /// viewport's window-pixel size for raster art, so high-resolution art is
    /// not first squeezed down to the virtual resolution.
    pub fn scene_size(&self) -> (u32, u32) {
        match self.art_style {
            ArtStyle::RasterArt => (
                self.viewport.width.max(1.0) as u32,
                self.viewport.height.max(1.0) as u32,
            ),
            ArtStyle::PixelArt | ArtStyle::Hybrid => self.virtual_size(),
        }
    }

    fn virtual_size(&self) -> (u32, u32) {
        (
            self.camera.virtual_width.round().max(1.0) as u32,
            self.camera.virtual_height.round().max(1.0) as u32,
        )
    }

    /// Recompute the viewport and resize the scene targets to match. Cheap
    /// when nothing changed, so it runs every frame: a game may change the
    /// camera's virtual resolution at any time.
    fn sync_scene_targets(&mut self) {
        self.viewport = Viewport::compute(
            self.scale_mode,
            self.virtual_size(),
            (self.surface_config.width, self.surface_config.height),
        );
        let size = self.scene_size();
        if size != self.blit.size() {
            self.blit.resize(&self.device, size);
            self.post_process.resize(&self.device, size.0, size.1);
            self.lighting_pipeline.resize(&self.device, size.0, size.1);
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
        let id = self.texture_ids.allocate();
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

    /// The allocator `load_texture` and font pages draw their ids from.
    pub fn texture_ids(&self) -> TextureIdAllocator {
        self.texture_ids.clone()
    }

    /// Create the texture under `id`, or replace it. A same-size replacement
    /// writes into the existing GPU texture, so its bind group stays valid.
    /// `image` has straight alpha, like `load_texture`'s.
    pub fn upload_texture(&mut self, id: TextureId, image: &image::RgbaImage, mode: SamplerMode) {
        if let Some(existing) = self.textures.get_mut(&id)
            && (existing.width, existing.height) == image.dimensions()
            && existing.sampler_mode == mode
        {
            let mut premultiplied = image.clone();
            crate::blend::premultiply_srgb(&mut premultiplied);
            existing.write_rows(&self.queue, premultiplied.as_raw(), 0, image.height());
            return;
        }
        let texture = Texture::from_image_with_mode(
            &self.device,
            &self.queue,
            &self.texture_bind_group_layout,
            image,
            id,
            &format!("texture_{}", id.0),
            mode,
        );
        self.textures.insert(id, texture);
    }

    /// Upload a font page under its own id: the whole page the first time,
    /// only its changed rows afterwards.
    pub fn upload_font_page(&mut self, page: &mut FontPage) {
        let size = (FONT_PAGE_SIZE, FONT_PAGE_SIZE);
        let mode = self.art_style.default_sampler_mode();
        match (self.textures.get_mut(&page.texture_id), page.dirty_rows()) {
            (Some(texture), Some((start, end))) if page.is_uploaded() => {
                texture.write_rows(&self.queue, &page.data, start, end);
            }
            (Some(_), None) if page.is_uploaded() => {}
            _ => {
                if let Some(texture) = Texture::from_premultiplied(
                    &self.device,
                    &self.queue,
                    &self.texture_bind_group_layout,
                    &page.data,
                    size,
                    page.texture_id,
                    &format!("font_page_{}", page.texture_id.0),
                    mode,
                ) {
                    self.textures.insert(page.texture_id, texture);
                }
            }
        }
        page.mark_uploaded();
    }

    /// Scene-target pixels per virtual pixel: 1.0 for pixel art, the viewport
    /// scale for raster art.
    pub fn render_scale(&self) -> f32 {
        let (w, _) = self.scene_size();
        w as f32 / self.camera.virtual_width.max(1.0)
    }

    /// Set the global art style. Affects default sampler mode for newly loaded textures.
    ///
    /// The engine swaps the game's camera into [`Renderer::camera`] only for
    /// the duration of a frame, so the camera's `pixel_snap` must also be set
    /// on the game's camera; this sets it on whichever camera the renderer
    /// holds right now.
    pub fn set_art_style(&mut self, style: ArtStyle) {
        self.art_style = style;
        self.camera.pixel_snap = style.pixel_snap();
        self.sync_scene_targets();
    }

    pub fn render(&mut self) -> Result<(), SurfaceError> {
        let frame = self.begin_frame()?;
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        self.queue.present(frame.output);
        self.batcher.clear();
        self.ui_batcher.clear();
        Ok(())
    }

    /// Begin a frame: render sprites and return the in-progress frame so
    /// additional render passes (e.g. egui) can be appended before submit.
    ///
    /// The queued sprites are consumed either way: on success they are
    /// uploaded and recorded into the returned frame, and on a surface error
    /// (a minimized window reports Outdated, a hidden Wayland window
    /// Timeout) they are dropped. Keeping them let every failed frame pile
    /// the next frame's draw list on top, until the vertex buffer outgrew
    /// wgpu's limit and the first visible frame panicked.
    pub fn begin_frame(&mut self) -> Result<FrameInProgress, SurfaceError> {
        let output = match SurfaceError::acquire(self.surface.get_current_texture()) {
            Ok(output) => output,
            Err(e) => {
                self.batcher.clear();
                self.ui_batcher.clear();
                return Err(e);
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });
        self.record_scene(&mut encoder);
        // Last: scale the finished scene, UI included, into the window. An
        // editor overlay drawn on `view` afterwards stays at window resolution.
        self.blit
            .apply(&mut encoder, &view, self.viewport, self.letterbox_color);

        // Everything queued is now in GPU buffers recorded into `encoder`.
        // Clearing here rather than in end_frame also covers callers that
        // finish the frame themselves (the editor submits egui's encoder and
        // never cleared ui_batcher, so UI sprites accumulated every frame).
        self.batcher.clear();
        self.ui_batcher.clear();

        Ok(FrameInProgress {
            encoder,
            view,
            output,
        })
    }

    /// Record every scene stage into the scene target. Leaves the batchers
    /// filled, so a screenshot can record the same frame again.
    fn record_scene(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.sync_scene_targets();

        // Update projection uniform
        let proj = self.camera.projection_matrix();
        let proj_flat: [f32; 16] = bytemuck::cast(proj);
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

        // Stage chain, per conventions A.6: sprites -> lighting -> post -> UI,
        // all into the scene target. Each inactive stage drops out of the
        // chain entirely, so a game using neither draws straight into it.
        let view = self.blit.target_view().clone();
        let post_enabled = self.post_process.enabled();
        let lighting_enabled = self.lighting.is_active();
        let scene_view = if lighting_enabled {
            self.lighting_pipeline.scene_view()
        } else if post_enabled {
            self.post_process.render_target_view()
        } else {
            &view
        };

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sprite_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_view,
                    depth_slice: None,
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
                multiview_mask: None,
            });

            if let (Some(vb), Some(ib)) = (&vertex_buffer, &index_buffer) {
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);

                let mut blend = None;
                for batch in &batches {
                    if blend != Some(batch.blend) {
                        render_pass.set_pipeline(&self.pipelines[batch.blend.index()]);
                        blend = Some(batch.blend);
                    }
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

        if lighting_enabled {
            // Lighting writes into post's input when post is active, otherwise
            // straight to the scene target.
            let target = if post_enabled {
                self.post_process.render_target_view()
            } else {
                &view
            };
            self.lighting_pipeline.apply(
                encoder,
                &self.device,
                &self.queue,
                &self.lighting,
                self.camera.view_rect(),
                target,
            );
        }

        if post_enabled {
            self.post_process
                .apply(encoder, &self.device, &self.queue, &view);
        }

        // UI pass: after post-processing, in screen space, loading rather than
        // clearing so it composites over the scene.
        self.draw_ui_pass(encoder, &view);
    }

    /// Draw `ui_batcher` over `target` with a screen-space projection.
    ///
    /// A no-op when nothing queued UI this frame, so games that draw no HUD pay
    /// only for the emptiness check.
    fn draw_ui_pass(&mut self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let batches = self.ui_batcher.build();
        if batches.is_empty() || self.ui_batcher.vertices().is_empty() {
            return;
        }
        self.draw_call_count += batches.len() as u32;

        // Orthographic projection over the virtual resolution, y down, with no
        // camera translation, zoom or shake.
        let (w, h) = (
            self.camera.virtual_width.max(1.0),
            self.camera.virtual_height.max(1.0),
        );
        let proj: [f32; 16] = [
            2.0 / w,
            0.0,
            0.0,
            0.0,
            0.0,
            -2.0 / h,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            -1.0,
            1.0,
            0.0,
            1.0,
        ];
        self.queue
            .write_buffer(&self.ui_uniform_buffer, 0, bytemuck::cast_slice(&proj));

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui_vertex_buffer"),
                contents: bytemuck::cast_slice(self.ui_batcher.vertices()),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ui_index_buffer"),
                contents: bytemuck::cast_slice(self.ui_batcher.indices()),
                usage: wgpu::BufferUsages::INDEX,
            });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load, not Clear: the scene is already there.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_bind_group(0, &self.ui_uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        let mut blend = None;
        for batch in &batches {
            if blend != Some(batch.blend) {
                pass.set_pipeline(&self.pipelines[batch.blend.index()]);
                blend = Some(batch.blend);
            }
            if let Some(texture) = self.textures.get(&batch.texture_id) {
                pass.set_bind_group(1, &texture.bind_group, &[]);
                pass.draw_indexed(
                    batch.index_offset..batch.index_offset + batch.index_count,
                    0,
                    0..1,
                );
            } else {
                warn!("UI pass: missing texture {:?}", batch.texture_id);
            }
        }
    }

    /// Finish a frame that was started with `begin_frame()`.
    pub fn end_frame(&mut self, frame: FrameInProgress) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        self.queue.present(frame.output);
        self.batcher.clear();
        self.ui_batcher.clear();
    }

    /// Capture the current frame to a PNG file at `path`.
    ///
    /// Records the whole stage chain (sprites, lighting, post-processing, UI)
    /// into the scene target and reads it back, so the image is what the
    /// player sees minus the letterbox bars, at the scene resolution. The
    /// batchers are NOT cleared — call this before `render()` so sprites are
    /// still queued.
    pub fn capture_screenshot(&mut self, path: &str) -> Result<(), String> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("screenshot_encoder"),
            });
        self.record_scene(&mut encoder);

        let (width, height) = self.blit.size();
        // The scene target uses the surface format, which is BGRA on most
        // desktop backends; PNG wants RGBA.
        let swap_red_blue = match self.blit.format() {
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
            other => {
                return Err(format!(
                    "Screenshots of a {other:?} surface are not supported"
                ));
            }
        };
        let offscreen_texture = self.blit.target();

        // Copy texture to readback buffer
        let bytes_per_row = (4 * width + 255) & !255; // align to 256
        let buffer_size = (bytes_per_row * height) as u64;
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot_readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: offscreen_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read back the buffer (blocking)
        let buffer_slice = readback_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("Screenshot readback poll failed: {e}"))?;

        rx.recv()
            .map_err(|e| format!("Screenshot readback channel error: {e}"))?
            .map_err(|e| format!("Screenshot buffer map failed: {e:?}"))?;

        let data = buffer_slice
            .get_mapped_range()
            .map_err(|e| format!("Screenshot buffer range failed: {e:?}"))?;

        // Copy to image (removing row padding)
        let mut img = image::RgbaImage::new(width, height);
        for y in 0..height {
            let src_offset = (y * bytes_per_row) as usize;
            let row = &data[src_offset..src_offset + (4 * width) as usize];
            for x in 0..width {
                let i = (x * 4) as usize;
                let (r, b) = if swap_red_blue {
                    (row[i + 2], row[i])
                } else {
                    (row[i], row[i + 2])
                };
                img.put_pixel(x, y, image::Rgba([r, row[i + 1], b, row[i + 3]]));
            }
        }

        drop(data);
        readback_buffer.unmap();

        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(path).parent()
            && !parent.as_os_str().is_empty()
        {
            let _ = std::fs::create_dir_all(parent);
        }

        img.save(path)
            .map_err(|e| format!("Failed to save screenshot: {e}"))?;
        info!("Screenshot saved: {} ({}x{})", path, width, height);

        Ok(())
    }

    pub fn draw_call_count(&self) -> u32 {
        self.draw_call_count
    }

    pub fn window_size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }
}

#[cfg(test)]
mod tests {
    use super::SurfaceError;
    use wgpu::CurrentSurfaceTexture as Current;

    #[test]
    fn acquire_maps_every_failure_to_its_surface_error() {
        let cases = [
            (Current::Timeout, SurfaceError::Timeout),
            (Current::Occluded, SurfaceError::Occluded),
            (Current::Outdated, SurfaceError::Outdated),
            (Current::Lost, SurfaceError::Lost),
            (Current::Validation, SurfaceError::Validation),
        ];
        for (current, expected) in cases {
            assert_eq!(SurfaceError::acquire(current).err(), Some(expected));
        }
    }
}
