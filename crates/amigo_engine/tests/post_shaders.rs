//! `GameContext::register_post_shader` (rendering-extensions R7). Pipelines
//! need a GPU; registration and validation do not.

use amigo_engine::prelude::*;

#[test]
fn a_game_registers_a_post_shader_and_uses_it_from_a_custom_effect() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let source = r#"
@fragment
fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(scene, scene_sampler, in.uv) * post.params[0];
}
"#;
    assert_eq!(ctx.register_post_shader("tint", source), Ok(()));
    assert!(ctx.post_shaders().contains("tint"));
    ctx.post_effects = vec![PostEffect::Custom {
        shader: "tint".into(),
        params: [1.0; 16],
    }];
    let (passes, unknown) = amigo_render::post_process::plan_passes(&ctx.post_effects, |n| {
        ctx.post_shaders().contains(n)
    });
    assert_eq!(passes.len(), 1);
    assert!(unknown.is_empty());
}

#[test]
fn a_broken_shader_is_refused_with_its_name() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let err = ctx
        .register_post_shader("broken", "@fragment fn fs_main(")
        .unwrap_err();
    assert!(matches!(err, ShaderError::Parse { ref name, .. } if name == "broken"));
    assert!(!ctx.post_shaders().contains("broken"));
}
