//! Post-processing shaders on the prelude contract: the WGSL the engine
//! prepends, validation of a game's source with naga, the registry games add
//! shaders to, and the built-in shockwave and directional blur.

use std::collections::BTreeMap;

/// The WGSL the engine prepends to every post shader. Part of the public
/// contract (docs/specs/engine/rendering-extensions.md, R7).
pub const POST_PRELUDE: &str = r#"struct PostInput {
    // Size of the scene target in pixels.
    resolution: vec2<f32>,
    // Virtual resolution; screen-unit parameters are in this space.
    virtual_size: vec2<f32>,
    // `TimeInfo::elapsed` in seconds (presentation only, never simulation).
    time: f32,
    _pad: vec3<f32>,
    params: array<vec4<f32>, 4>,
};
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> post: PostInput;

struct PostVertexOutput {
    @builtin(position) position: vec4<f32>,
    // 0..1 across the scene target, origin top-left.
    @location(0) uv: vec2<f32>,
};

// The game's source must define:
// @fragment fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32>"#;

/// The built-in shockwave: `params[0] = (center.x, center.y, radius,
/// thickness)`, `params[1].x = strength`, all in virtual pixels.
pub const SHOCKWAVE_WGSL: &str = r#"
@fragment
fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * post.virtual_size;
    let center = post.params[0].xy;
    let radius = post.params[0].z;
    let half_thickness = max(post.params[0].w * 0.5, 0.0001);
    let strength = post.params[1].x;
    let to_p = p - center;
    let d = length(to_p);
    // A smooth bump: 1 on the ring, 0 at half a thickness from it.
    let x = clamp((d - radius) / half_thickness, -1.0, 1.0);
    let w = 0.5 + 0.5 * cos(x * 3.14159265);
    let dir = select(vec2<f32>(0.0, 0.0), to_p / max(d, 0.0001), d > 0.0001);
    let sample_at = (p - dir * strength * w) / post.virtual_size;
    return textureSample(scene, scene_sampler, sample_at);
}
"#;

/// The built-in directional blur: `params[0] = (dir.x, dir.y, length, _)`
/// with a unit direction and the length in virtual pixels. 9 taps centred on
/// the pixel.
pub const DIRECTIONAL_BLUR_WGSL: &str = r#"
@fragment
fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32> {
    let step = post.params[0].xy * post.params[0].z / post.virtual_size;
    var sum = vec4<f32>(0.0);
    for (var i: i32 = 0; i < 9; i = i + 1) {
        let t = f32(i) / 8.0 - 0.5;
        sum = sum + textureSample(scene, scene_sampler, in.uv + step * t);
    }
    return sum / 9.0;
}
"#;

/// The full WGSL of a post shader: prelude plus the game's source.
pub fn full_source(source: &str) -> String {
    format!("{POST_PRELUDE}\n{source}")
}

/// Why a post shader was refused.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ShaderError {
    #[error("post shader '{name}': WGSL parse error: {message}")]
    Parse { name: String, message: String },
    #[error("post shader '{name}': validation failed: {message}")]
    Validation { name: String, message: String },
    #[error("post shader '{name}': no `fs_main` fragment entry point")]
    MissingEntryPoint { name: String },
}

/// Parse and validate the prelude plus `source` on the CPU.
pub fn validate_post_shader(name: &str, source: &str) -> Result<(), ShaderError> {
    let full = full_source(source);
    let module = naga::front::wgsl::parse_str(&full).map_err(|e| ShaderError::Parse {
        name: name.to_string(),
        message: e.emit_to_string(&full),
    })?;
    let has_entry = module
        .entry_points
        .iter()
        .any(|e| e.name == "fs_main" && e.stage == naga::ShaderStage::Fragment);
    if !has_entry {
        return Err(ShaderError::MissingEntryPoint {
            name: name.to_string(),
        });
    }
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    )
    .validate(&module)
    .map_err(|e| ShaderError::Validation {
        name: name.to_string(),
        message: e.emit_to_string(&full),
    })?;
    Ok(())
}

/// The post shaders a game registered, by name, each with a generation that
/// changes when it is replaced.
#[derive(Clone, Debug, Default)]
pub struct PostShaderRegistry {
    shaders: BTreeMap<String, (String, u64)>,
    next_generation: u64,
}

impl PostShaderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate `wgsl` and register it under `name`, replacing any shader of
    /// that name.
    pub fn register(&mut self, name: &str, wgsl: &str) -> Result<(), ShaderError> {
        validate_post_shader(name, wgsl)?;
        self.next_generation += 1;
        self.shaders
            .insert(name.to_string(), (wgsl.to_string(), self.next_generation));
        Ok(())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.shaders.contains_key(name)
    }

    /// Every shader as `(name, source, generation)`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, u64)> {
        self.shaders
            .iter()
            .map(|(name, (source, generation))| (name.as_str(), source.as_str(), *generation))
    }
}

/// The uniform block of the prelude, laid out as WGSL lays out `PostInput`.
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostInput {
    pub resolution: [f32; 2],
    pub virtual_size: [f32; 2],
    pub time: f32,
    /// `_pad: vec3<f32>` is 16-byte aligned, so it starts at byte 32.
    _align: [f32; 3],
    _pad: [f32; 3],
    _tail: f32,
    pub params: [[f32; 4]; 4],
}

impl PostInput {
    pub fn new(resolution: [f32; 2], virtual_size: [f32; 2], time: f32, params: [f32; 16]) -> Self {
        let mut p = [[0.0; 4]; 4];
        for (i, v) in params.iter().enumerate() {
            p[i / 4][i % 4] = *v;
        }
        Self {
            resolution,
            virtual_size,
            time,
            _align: [0.0; 3],
            _pad: [0.0; 3],
            _tail: 0.0,
            params: p,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVERT: &str = r#"
@fragment
fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(scene, scene_sampler, in.uv);
    return vec4<f32>(vec3<f32>(c.a) - c.rgb, c.a) * post.params[0].x;
}
"#;

    #[test]
    fn a_valid_shader_registers() {
        let mut registry = PostShaderRegistry::new();
        assert_eq!(registry.register("invert", INVERT), Ok(()));
        assert!(registry.contains("invert"));
        let first = registry.iter().next().map(|(_, _, g)| g);
        registry.register("invert", INVERT).expect("replaces");
        let second = registry.iter().next().map(|(_, _, g)| g);
        assert_ne!(first, second, "replacing bumps the generation");
    }

    #[test]
    fn a_syntax_error_is_a_parse_error() {
        let err = validate_post_shader("bad", "@fragment fn fs_main( {").unwrap_err();
        assert!(matches!(err, ShaderError::Parse { .. }), "{err}");
    }

    #[test]
    fn a_shader_the_validator_rejects_is_a_validation_error() {
        // A vertex-only built-in on a fragment input passes the front end
        // and fails validation.
        let source = r#"
@fragment
fn fs_main(@builtin(vertex_index) i: u32) -> @location(0) vec4<f32> {
    return vec4<f32>(f32(i));
}
"#;
        let err = validate_post_shader("branchy", source).unwrap_err();
        assert!(matches!(err, ShaderError::Validation { .. }), "{err}");
    }

    #[test]
    fn a_shader_without_fs_main_is_refused() {
        let source = "fn helper() -> f32 { return 1.0; }";
        assert_eq!(
            validate_post_shader("helper", source),
            Err(ShaderError::MissingEntryPoint {
                name: "helper".into()
            })
        );
        let mut registry = PostShaderRegistry::new();
        assert!(registry.register("helper", source).is_err());
        assert!(!registry.contains("helper"));
    }

    #[test]
    fn the_built_in_shaders_validate_against_the_prelude() {
        assert_eq!(validate_post_shader("shockwave", SHOCKWAVE_WGSL), Ok(()));
        assert_eq!(
            validate_post_shader("directional_blur", DIRECTIONAL_BLUR_WGSL),
            Ok(())
        );
    }

    #[test]
    fn post_input_matches_the_wgsl_layout() {
        assert_eq!(std::mem::size_of::<PostInput>(), 112);
        let input = PostInput::new(
            [1.0, 2.0],
            [3.0, 4.0],
            5.0,
            std::array::from_fn(|i| i as f32),
        );
        let floats: &[f32] = bytemuck::cast_slice(std::slice::from_ref(&input));
        // `params` starts at byte 48.
        assert_eq!(floats[12], 0.0);
        assert_eq!(floats[27], 15.0);
    }

    #[test]
    fn the_prelude_is_the_one_in_the_spec() {
        // A Windows checkout may have CRLF line endings.
        let spec = include_str!("../../../docs/specs/engine/rendering-extensions.md")
            .replace("\r\n", "\n");
        let start = spec
            .find("The prelude the engine prepends")
            .expect("spec has the prelude section");
        let block = &spec[start..];
        let open = block.find("```wgsl\n").expect("wgsl block") + "```wgsl\n".len();
        let close = block[open..].find("\n```").expect("block end");
        assert_eq!(&block[open..open + close], POST_PRELUDE);
    }
}
