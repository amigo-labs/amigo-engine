//! `GameContext::viewport_info` (rendering-extensions R10).

use amigo_engine::prelude::*;

#[test]
fn a_new_context_describes_its_virtual_window_without_zero_sizes() {
    let ctx = GameContext::new(640.0, 360.0, "assets");
    let info = ctx.viewport_info();
    assert_eq!(info.virtual_size, (640.0, 360.0));
    assert_eq!(info.window_size, (640, 360));
    assert_eq!(info.render_scale, 1.0);
    assert!(info.viewport.width > 0.0 && info.viewport.height > 0.0);

    let mut sprites = Vec::new();
    let draw = DrawContext::new(
        &mut sprites,
        &ctx,
        RenderVec2::ZERO,
        640.0,
        360.0,
        0.0,
        amigo_render::TextureId(0),
    );
    assert_eq!(draw.viewport_info(), info);
}
