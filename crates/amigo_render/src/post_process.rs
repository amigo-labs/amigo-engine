use amigo_core::ColorBlindMode;
use serde::{Deserialize, Serialize};
use wgpu;
use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// WGSL shaders (embedded as string constants)
// ---------------------------------------------------------------------------

/// Fullscreen vertex shader. Generates a fullscreen triangle from vertex ID
/// (3 vertices, no vertex buffer needed). The UVs cover [0,1] over the screen.
const FULLSCREEN_VERTEX_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate a fullscreen triangle that covers the entire screen.
    // Vertex 0: (-1, -1), Vertex 1: (3, -1), Vertex 2: (-1, 3)
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    // Map clip coords to [0,1] UVs. Y is flipped for texture sampling.
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}
"#;

/// Combined post-processing fragment shader. Applies all effects in a single
/// pass, checking `enabled_flags` to skip disabled effects.
///
/// Bit layout of `enabled_flags`:
///   bit 0 (1)  - Bloom
///   bit 1 (2)  - Chromatic Aberration
///   bit 2 (4)  - Vignette
///   bit 3 (8)  - Color Grading
///   bit 4 (16) - CRT Filter
///   bit 5 (32) - Colorblind Filter
const POST_PROCESS_FRAGMENT_SHADER: &str = r#"
struct PostUniforms {
    // Bloom
    bloom_threshold: f32,
    bloom_intensity: f32,
    // Chromatic Aberration
    chroma_offset: f32,
    // Vignette
    vignette_intensity: f32,
    vignette_smoothness: f32,
    // Color Grading
    brightness: f32,
    contrast: f32,
    saturation: f32,
    // CRT Filter
    scanline_intensity: f32,
    curvature: f32,
    // Flags
    enabled_flags: u32,
    // Screen dimensions for resolution-dependent effects
    screen_width: f32,
    screen_height: f32,
    // Colorblind filter
    colorblind_mode: u32,      // 0=none, 1=protanopia, 2=deuteranopia, 3=tritanopia, 4=achromatopsia
    colorblind_strength: f32,  // 0.0..1.0 blend with original
    _pad0: f32,
};

@group(0) @binding(0) var t_scene: texture_2d<f32>;
@group(0) @binding(1) var s_scene: sampler;
@group(0) @binding(2) var<uniform> params: PostUniforms;

// ---------- helpers ----------

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

/// Simplified single-pass bloom approximation suitable for pixel art.
/// Extracts bright pixels, applies a small box-blur kernel, and adds the
/// result back to the original colour.
fn apply_bloom(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let texel = vec2<f32>(1.0 / params.screen_width, 1.0 / params.screen_height);
    var bloom = vec3<f32>(0.0);
    // 9-tap box kernel (3x3)
    for (var x: i32 = -1; x <= 1; x = x + 1) {
        for (var y: i32 = -1; y <= 1; y = y + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel * 2.0;
            let s = textureSample(t_scene, s_scene, uv + offset).rgb;
            let b = max(s - vec3<f32>(params.bloom_threshold), vec3<f32>(0.0));
            bloom = bloom + b;
        }
    }
    bloom = bloom / 9.0;
    return color + bloom * params.bloom_intensity;
}

/// Chromatic aberration: offset R and B channels in opposite directions.
fn apply_chromatic_aberration(uv: vec2<f32>) -> vec3<f32> {
    let dir = (uv - vec2<f32>(0.5)) * params.chroma_offset;
    let r = textureSample(t_scene, s_scene, uv + dir).r;
    let g = textureSample(t_scene, s_scene, uv).g;
    let b = textureSample(t_scene, s_scene, uv - dir).b;
    return vec3<f32>(r, g, b);
}

/// Vignette: darken edges based on distance from centre.
fn apply_vignette(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let d = distance(uv, vec2<f32>(0.5));
    let v = smoothstep(0.8, 0.8 - params.vignette_smoothness, d * (params.vignette_intensity + params.vignette_intensity));
    return color * v;
}

/// Colour grading: brightness, contrast, and saturation adjustments.
fn apply_color_grading(color: vec3<f32>) -> vec3<f32> {
    // Brightness
    var c = color * params.brightness;
    // Contrast (pivot at 0.5)
    c = (c - vec3<f32>(0.5)) * params.contrast + vec3<f32>(0.5);
    // Saturation
    let lum = luminance(c);
    c = mix(vec3<f32>(lum), c, params.saturation);
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

// ---------- colorblind correction (Machado et al. 2009) ----------

/// Protanopia (red-blind) correction matrix.
fn daltonize_protanopia(c: vec3<f32>) -> vec3<f32> {
    let m = mat3x3<f32>(
        vec3<f32>(0.567, 0.558, 0.0),
        vec3<f32>(0.433, 0.442, 0.242),
        vec3<f32>(0.0,   0.0,   0.758),
    );
    return m * c;
}

/// Deuteranopia (red-green) correction matrix.
fn daltonize_deuteranopia(c: vec3<f32>) -> vec3<f32> {
    let m = mat3x3<f32>(
        vec3<f32>(0.625, 0.7, 0.0),
        vec3<f32>(0.375, 0.3, 0.3),
        vec3<f32>(0.0,   0.0, 0.7),
    );
    return m * c;
}

/// Tritanopia (blue-yellow) correction matrix.
fn daltonize_tritanopia(c: vec3<f32>) -> vec3<f32> {
    let m = mat3x3<f32>(
        vec3<f32>(0.95, 0.0,  0.0),
        vec3<f32>(0.05, 0.433, 0.475),
        vec3<f32>(0.0,  0.567, 0.525),
    );
    return m * c;
}

/// Achromatopsia (monochromacy) — convert to grayscale via luminance.
fn daltonize_achromatopsia(c: vec3<f32>) -> vec3<f32> {
    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    return vec3<f32>(lum, lum, lum);
}

/// Apply colorblind correction based on mode uniform, blended by strength.
fn apply_colorblind(color: vec3<f32>) -> vec3<f32> {
    var corrected: vec3<f32>;
    switch params.colorblind_mode {
        case 1u: { corrected = daltonize_protanopia(color); }
        case 2u: { corrected = daltonize_deuteranopia(color); }
        case 3u: { corrected = daltonize_tritanopia(color); }
        case 4u: { corrected = daltonize_achromatopsia(color); }
        default: { return color; }
    }
    return mix(color, corrected, params.colorblind_strength);
}

/// CRT filter: scanlines + barrel distortion.
fn apply_crt_distortion(uv: vec2<f32>) -> vec2<f32> {
    let centered = uv - vec2<f32>(0.5);
    let r2 = dot(centered, centered);
    let distorted = centered * (1.0 + params.curvature * r2);
    return distorted + vec2<f32>(0.5);
}

fn apply_crt_scanlines(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let scanline = sin(uv.y * params.screen_height * 3.14159) * 0.5 + 0.5;
    let factor = 1.0 - params.scanline_intensity * (1.0 - scanline);
    return color * factor;
}

// ---------- main ----------

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    var working_uv = uv;

    // CRT barrel distortion must happen before sampling.
    let crt_enabled = (params.enabled_flags & 16u) != 0u;
    if crt_enabled {
        working_uv = apply_crt_distortion(working_uv);
        // Discard pixels outside [0,1] after distortion.
        if working_uv.x < 0.0 || working_uv.x > 1.0 || working_uv.y < 0.0 || working_uv.y > 1.0 {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }

    // Sample scene colour (chromatic aberration replaces the base sample).
    var color: vec3<f32>;
    let chroma_enabled = (params.enabled_flags & 2u) != 0u;
    if chroma_enabled {
        color = apply_chromatic_aberration(working_uv);
    } else {
        color = textureSample(t_scene, s_scene, working_uv).rgb;
    }

    // Bloom
    let bloom_enabled = (params.enabled_flags & 1u) != 0u;
    if bloom_enabled {
        color = apply_bloom(color, working_uv);
    }

    // Vignette
    let vignette_enabled = (params.enabled_flags & 4u) != 0u;
    if vignette_enabled {
        color = apply_vignette(color, working_uv);
    }

    // Colour grading
    let grading_enabled = (params.enabled_flags & 8u) != 0u;
    if grading_enabled {
        color = apply_color_grading(color);
    }

    // Colorblind correction (after grading, before CRT).
    let colorblind_enabled = (params.enabled_flags & 32u) != 0u;
    if colorblind_enabled {
        color = apply_colorblind(color);
    }

    // CRT scanlines (applied after all other effects).
    if crt_enabled {
        color = apply_crt_scanlines(color, working_uv);
    }

    return vec4<f32>(color, 1.0);
}
"#;

// ---------------------------------------------------------------------------
// PostEffect enum (RON-serialisable)
// ---------------------------------------------------------------------------

/// Describes a single post-processing effect and its parameters.
///
/// Effects are serialisable with `serde` so they can be loaded from RON
/// config files on a per-scene or per-world basis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PostEffect {
    Bloom {
        threshold: f32,
        intensity: f32,
    },
    ChromaticAberration {
        offset: f32,
    },
    Vignette {
        intensity: f32,
        smoothness: f32,
    },
    ColorGrading {
        brightness: f32,
        contrast: f32,
        saturation: f32,
    },
    CrtFilter {
        scanline_intensity: f32,
        curvature: f32,
    },
    /// Colorblind correction filter (Daltonization).
    ///
    /// Applied after color grading but before CRT scanlines.
    ColorblindFilter {
        mode: ColorBlindMode,
        strength: f32,
    },
}

// ---------------------------------------------------------------------------
// Uniform buffer (bytemuck-compatible)
// ---------------------------------------------------------------------------

/// Packed uniform data uploaded to the GPU each frame.
///
/// `enabled_flags` is a bitfield indicating which effects are active:
///   bit 0 (1)  - Bloom
///   bit 1 (2)  - Chromatic Aberration
///   bit 2 (4)  - Vignette
///   bit 3 (8)  - Color Grading
///   bit 4 (16) - CRT Filter
///   bit 5 (32) - Colorblind Filter
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostProcessUniforms {
    pub bloom_threshold: f32,
    pub bloom_intensity: f32,
    pub chroma_offset: f32,
    pub vignette_intensity: f32,
    pub vignette_smoothness: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub scanline_intensity: f32,
    pub curvature: f32,
    pub enabled_flags: u32,
    pub screen_width: f32,
    pub screen_height: f32,
    /// 0=none, 1=protanopia, 2=deuteranopia, 3=tritanopia, 4=achromatopsia.
    pub colorblind_mode: u32,
    /// Blend strength for colorblind correction (0.0..1.0).
    pub colorblind_strength: f32,
    pub _pad0: f32,
}

impl PostProcessUniforms {
    /// Build uniforms from the current effect stack and screen dimensions.
    fn from_effects(effects: &[PostEffect], width: u32, height: u32) -> Self {
        let mut u = Self {
            bloom_threshold: 0.8,
            bloom_intensity: 0.0,
            chroma_offset: 0.0,
            vignette_intensity: 0.0,
            vignette_smoothness: 0.3,
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            scanline_intensity: 0.0,
            curvature: 0.0,
            enabled_flags: 0,
            screen_width: width as f32,
            screen_height: height as f32,
            colorblind_mode: 0,
            colorblind_strength: 0.0,
            _pad0: 0.0,
        };

        for effect in effects {
            match effect {
                PostEffect::Bloom {
                    threshold,
                    intensity,
                } => {
                    u.enabled_flags |= 1;
                    u.bloom_threshold = *threshold;
                    u.bloom_intensity = *intensity;
                }
                PostEffect::ChromaticAberration { offset } => {
                    u.enabled_flags |= 2;
                    u.chroma_offset = *offset;
                }
                PostEffect::Vignette {
                    intensity,
                    smoothness,
                } => {
                    u.enabled_flags |= 4;
                    u.vignette_intensity = *intensity;
                    u.vignette_smoothness = *smoothness;
                }
                PostEffect::ColorGrading {
                    brightness,
                    contrast,
                    saturation,
                } => {
                    u.enabled_flags |= 8;
                    u.brightness = *brightness;
                    u.contrast = *contrast;
                    u.saturation = *saturation;
                }
                PostEffect::CrtFilter {
                    scanline_intensity,
                    curvature,
                } => {
                    u.enabled_flags |= 16;
                    u.scanline_intensity = *scanline_intensity;
                    u.curvature = *curvature;
                }
                PostEffect::ColorblindFilter { mode, strength } => {
                    u.enabled_flags |= 32;
                    u.colorblind_mode = match mode {
                        ColorBlindMode::None => 0,
                        ColorBlindMode::Protanopia => 1,
                        ColorBlindMode::Deuteranopia => 2,
                        ColorBlindMode::Tritanopia => 3,
                        ColorBlindMode::Achromatopsia => 4,
                    };
                    u.colorblind_strength = strength.clamp(0.0, 1.0);
                }
            }
        }

        u
    }
}

// ---------------------------------------------------------------------------
// PostProcessPipeline
// ---------------------------------------------------------------------------

/// Manages an offscreen render target and a configurable chain of
/// post-processing effects applied as a single fullscreen pass.
pub struct PostProcessPipeline {
    /// Offscreen texture that sprites are rendered into.
    offscreen_texture: wgpu::Texture,
    /// View of the offscreen texture (used as render target by the sprite pass).
    offscreen_view: wgpu::TextureView,
    /// The texture format used for the offscreen and output targets.
    format: wgpu::TextureFormat,
    /// Dimensions of the current offscreen texture.
    width: u32,
    height: u32,
    /// The active effect stack, applied in order.
    effects: Vec<PostEffect>,
    /// The fullscreen-quad render pipeline.
    pipeline: wgpu::RenderPipeline,
    /// Bind group layout shared between frames.
    bind_group_layout: wgpu::BindGroupLayout,
    /// Sampler for the offscreen texture.
    sampler: wgpu::Sampler,
    /// Uniform buffer written every frame.
    uniform_buffer: wgpu::Buffer,
}

impl PostProcessPipeline {
    /// Create a new post-processing pipeline.
    ///
    /// * `device`  - wgpu device.
    /// * `width`   - initial framebuffer width in pixels.
    /// * `height`  - initial framebuffer height in pixels.
    /// * `format`  - texture format used for the surface and offscreen target.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let (offscreen_texture, offscreen_view) =
            Self::create_offscreen_target(device, width, height, format);

        // Shader modules --------------------------------------------------------
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post_process_vertex"),
            source: wgpu::ShaderSource::Wgsl(FULLSCREEN_VERTEX_SHADER.into()),
        });
        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post_process_fragment"),
            source: wgpu::ShaderSource::Wgsl(POST_PROCESS_FRAGMENT_SHADER.into()),
        });

        // Bind group layout -----------------------------------------------------
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post_process_bind_group_layout"),
            entries: &[
                // Scene texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Uniforms
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
            label: Some("post_process_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline -------------------------------------------------------
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post_process_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[], // fullscreen triangle - no vertex buffers
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
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Sampler ---------------------------------------------------------------
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post_process_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest, // default pixel-art friendly
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Uniform buffer --------------------------------------------------------
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("post_process_uniforms"),
            contents: bytemuck::bytes_of(&PostProcessUniforms::from_effects(&[], width, height)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            offscreen_texture,
            offscreen_view,
            format,
            width,
            height,
            effects: Vec::new(),
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
        }
    }

    // -- public API ---------------------------------------------------------

    /// Returns the texture view that the sprite renderer should draw into.
    pub fn render_target_view(&self) -> &wgpu::TextureView {
        &self.offscreen_view
    }

    /// Recreate the offscreen render targets after a window resize.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        let (tex, view) = Self::create_offscreen_target(device, width, height, self.format);
        self.offscreen_texture = tex;
        self.offscreen_view = view;
    }

    /// Replace the entire effect stack.
    pub fn set_effects(&mut self, effects: Vec<PostEffect>) {
        self.effects = effects;
    }

    /// Remove all effects.
    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }

    /// Recreate the sampler with a different filter mode (e.g. for raster-art).
    pub fn set_sampler_mode(&mut self, device: &wgpu::Device, mode: crate::SamplerMode) {
        let filter = mode.to_wgpu();
        self.sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post_process_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: filter,
            min_filter: filter,
            ..Default::default()
        });
    }

    /// Returns `true` when at least one effect is active.
    pub fn enabled(&self) -> bool {
        !self.effects.is_empty()
    }

    /// Run the post-processing chain and output the result to `output_view`.
    ///
    /// This should be called once per frame *after* the scene has been
    /// rendered into `render_target_view()`.
    pub fn apply(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_view: &wgpu::TextureView,
    ) {
        // Upload uniforms for this frame.
        let uniforms = PostProcessUniforms::from_effects(&self.effects, self.width, self.height);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Build a transient bind group (texture view may change on resize).
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post_process_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.offscreen_view),
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

        // Fullscreen pass -------------------------------------------------------
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("post_process_pass"),
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
            pass.draw(0..3, 0..1); // fullscreen triangle
        }
    }

    // -- internal helpers ---------------------------------------------------

    fn create_offscreen_target(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("post_process_offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
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
