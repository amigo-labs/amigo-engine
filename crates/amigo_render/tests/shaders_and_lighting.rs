//! Shader and lighting-packing checks that need no GPU.
//!
//! The lighting stage had no shader and no pass at all before this: `LightingState`
//! packed bytes that nothing uploaded. Two things can go wrong without a GPU
//! telling you:
//!
//! 1. The WGSL does not compile. `naga` is wgpu's own front-end, so parsing and
//!    validating with it catches the same errors the driver would.
//! 2. The Rust struct layout and the WGSL struct layout disagree. WGSL aligns
//!    `vec4<f32>` to 16 bytes, so a mis-ordered field silently shifts every
//!    subsequent one and lights read garbage. That is checked here by offset.

use amigo_core::{Color, Rect};
use amigo_render::lighting::{AmbientLight, LightData, LightingHeader, LightingState, PointLight};

/// Parse and validate WGSL the way wgpu does before handing it to the driver.
fn validate_wgsl(label: &str, source: &str) {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|e| panic!("{label} failed to parse: {e:?}"));
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::default(),
    );
    validator
        .validate(&module)
        .unwrap_or_else(|e| panic!("{label} failed validation: {e:?}"));
}

#[test]
fn the_lighting_vertex_shader_compiles() {
    validate_wgsl(
        "lighting vertex shader",
        amigo_render::lighting_pipeline::LIGHTING_VERTEX_SHADER,
    );
}

#[test]
fn the_lighting_fragment_shader_compiles() {
    validate_wgsl(
        "lighting fragment shader",
        amigo_render::lighting_pipeline::LIGHTING_FRAGMENT_SHADER,
    );
}

#[test]
fn light_data_matches_the_wgsl_layout() {
    // WGSL: color at 0 (vec4, align 16), position at 16 (vec2, align 8),
    // radius 24, intensity 28, falloff 32, struct size rounded to 48.
    assert_eq!(std::mem::size_of::<LightData>(), 48, "uniform array stride");
    assert_eq!(std::mem::align_of::<LightData>(), 4);

    let light = LightData {
        color: [1.0, 2.0, 3.0, 4.0],
        position: [5.0, 6.0],
        radius: 7.0,
        intensity: 8.0,
        falloff: 9.0,
        _pad: [0.0; 3],
    };
    let bytes = bytemuck::bytes_of(&light);
    let floats: &[f32] = bytemuck::cast_slice(bytes);

    assert_eq!(&floats[0..4], &[1.0, 2.0, 3.0, 4.0], "colour must be first");
    assert_eq!(&floats[4..6], &[5.0, 6.0], "position follows at offset 16");
    assert_eq!(floats[6], 7.0, "radius at offset 24");
    assert_eq!(floats[7], 8.0, "intensity at offset 28");
    assert_eq!(floats[8], 9.0, "falloff at offset 32");
}

#[test]
fn lighting_header_matches_the_wgsl_layout() {
    // ambient vec4 at 0, view_size vec2 at 16, light_count u32 at 24, pad to 32.
    assert_eq!(std::mem::size_of::<LightingHeader>(), 32);
}

#[test]
fn neutral_lighting_is_inactive_so_the_pass_is_skipped() {
    let state = LightingState::new();
    assert!(
        !state.is_active(),
        "white ambient at full intensity multiplies by 1.0; running a fullscreen \
         pass for that would cost a pass to achieve nothing"
    );
}

#[test]
fn any_point_light_activates_the_pass() {
    let mut state = LightingState::new();
    state.add_light(PointLight {
        position: (0.0, 0.0),
        color: Color::WHITE,
        intensity: 1.0,
        radius: 32.0,
        falloff: 2.0,
    });
    assert!(state.is_active());
}

#[test]
fn darkened_ambient_activates_the_pass() {
    let mut state = LightingState::new();
    state.set_ambient(Color::WHITE, 0.2);
    assert!(
        state.is_active(),
        "a dark scene is the main reason to use lighting at all"
    );
}

#[test]
fn tinted_ambient_at_full_intensity_still_activates() {
    let mut state = LightingState::new();
    state.ambient = AmbientLight {
        color: Color::rgb(0.6, 0.7, 1.0),
        intensity: 1.0,
    };
    assert!(state.is_active(), "a blue moonlight tint is not neutral");
}

#[test]
fn packing_converts_light_positions_into_view_local_space() {
    let mut state = LightingState::new();
    state.add_light(PointLight {
        position: (150.0, 220.0),
        color: Color::WHITE,
        intensity: 1.0,
        radius: 40.0,
        falloff: 2.0,
    });
    // Camera showing the world rect starting at (100, 200).
    let view = Rect::new(100.0, 200.0, 320.0, 180.0);

    let packed = pack(&state, view);

    let header: LightingHeader = *bytemuck::from_bytes(&packed[..32]);
    assert_eq!(header.light_count, 1);
    assert_eq!(header.view_size, [320.0, 180.0]);

    let light: LightData = *bytemuck::from_bytes(&packed[32..80]);
    assert_eq!(
        light.position,
        [50.0, 20.0],
        "world position minus the view origin, so the shader can compare it \
         against a fragment's position in the same units"
    );
    assert_eq!(light.radius, 40.0, "radii stay in world units, unscaled");
}

#[test]
fn packing_zero_fills_unused_light_slots() {
    let mut state = LightingState::new();
    state.add_light(PointLight {
        position: (1.0, 1.0),
        color: Color::WHITE,
        intensity: 1.0,
        radius: 10.0,
        falloff: 1.0,
    });

    let packed = pack(&state, Rect::new(0.0, 0.0, 320.0, 180.0));

    // Header + 64 slots, whatever the light count.
    assert_eq!(packed.len(), 32 + 64 * 48);
    assert!(
        packed[32 + 48..].iter().all(|b| *b == 0),
        "a stale light from a previous frame must not be readable"
    );
}

#[test]
fn packing_respects_the_light_cap() {
    let mut state = LightingState::new();
    for i in 0..200 {
        state.add_light(PointLight {
            position: (i as f32, 0.0),
            color: Color::WHITE,
            intensity: 1.0,
            radius: 5.0,
            falloff: 1.0,
        });
    }

    let packed = pack(&state, Rect::new(0.0, 0.0, 320.0, 180.0));

    let header: LightingHeader = *bytemuck::from_bytes(&packed[..32]);
    assert!(
        header.light_count as usize <= 64,
        "the shader loop and the uniform array are both bounded at 64; a larger \
         count would read past the array"
    );
    assert_eq!(packed.len(), 32 + 64 * 48, "buffer size is fixed");
}

/// The engine's own packing, so this cannot drift from what the GPU receives.
fn pack(state: &LightingState, view: Rect) -> Vec<u8> {
    amigo_render::lighting_pipeline::pack_lighting(state, view)
}

// ---------------------------------------------------------------------------
// The shaders that already existed
// ---------------------------------------------------------------------------
//
// These compiled on a GPU before, but nothing checked them in CI, so a WGSL
// regression would only surface for whoever next opened a window.

#[test]
fn the_gpu_broad_phase_shader_compiles() {
    validate_wgsl(
        "gpu_broad_phase.wgsl",
        include_str!("../src/gpu_broad_phase.wgsl"),
    );
}

#[test]
fn the_post_process_shaders_compile() {
    validate_wgsl(
        "post-process vertex shader",
        amigo_render::post_process::FULLSCREEN_VERTEX_SHADER,
    );
    validate_wgsl(
        "post-process fragment shader",
        amigo_render::post_process::POST_PROCESS_FRAGMENT_SHADER,
    );
}
