//! Text through `DrawContext` (rendering-extensions R5): Unicode in the legacy
//! calls, `draw_text_ex`, and the default font reaching `amigo_ui` widgets.

use amigo_engine::prelude::*;
use amigo_render::texture::TextureId;

fn ctx_with(font: &[u8]) -> (GameContext, FontId) {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let id = ctx.fonts.load_font(font, 12.0).expect("font parses");
    (ctx, id)
}

fn draw(ctx: &GameContext, f: impl FnOnce(&mut DrawContext)) -> Vec<SpriteInstance> {
    let mut sprites = Vec::new();
    let mut d = DrawContext::new(
        &mut sprites,
        ctx,
        RenderVec2::new(160.0, 90.0),
        320.0,
        180.0,
        0.0,
        TextureId(0),
    );
    f(&mut d);
    sprites
}

#[test]
fn legacy_draw_text_draws_umlauts() {
    let (ctx, _) = ctx_with(epaint_default_fonts::UBUNTU_LIGHT);
    let sprites = draw(&ctx, |d| d.draw_text("Grüße", 0.0, 0.0, Color::WHITE));
    assert_eq!(sprites.len(), 5);
    // Without kerning: each quad starts at the previous pen plus an offset.
    assert!(sprites.windows(2).all(|w| w[0].x < w[1].x));
}

#[test]
fn draw_text_ex_bounds_match_the_measurement() {
    let (ctx, font) = ctx_with(epaint_default_fonts::HACK_REGULAR);
    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
        let style = TextStyle {
            font: Some(font),
            size_px: Some(16.0),
            align,
            z_order: 7,
            blend: BlendMode::Additive,
            ..Default::default()
        };
        let mut bounds = Rect::default();
        let mut measured = TextMetrics::default();
        let sprites = draw(&ctx, |d| {
            d.set_z(3);
            bounds = d.draw_text_ex("Größe", RenderVec2::new(50.0, 20.0), &style);
            measured = d.measure_text_ex("Größe", &style);
        });
        assert_eq!(bounds.w, measured.width, "{align:?}");
        assert_eq!(bounds.y, 20.0);
        assert_eq!(sprites.len(), 5);
        // The style's z and blend, not set_z.
        assert!(sprites.iter().all(|s| s.z_order == 7));
        assert!(sprites.iter().all(|s| s.blend == BlendMode::Additive));
    }
}

#[test]
fn ui_text_uses_the_chosen_default_font() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let pixel = ctx.fonts.load_builtin(7.0).expect("builtin");
    let hack = ctx
        .fonts
        .load_font(epaint_default_fonts::HACK_REGULAR, 12.0)
        .expect("hack parses");
    let pixel_page = ctx.fonts.get(pixel).expect("pixel").pages()[0].texture_id;
    let hack_page = ctx.fonts.get(hack).expect("hack").pages()[0].texture_id;

    let ui_text = |ctx: &GameContext| {
        let commands = vec![UiDrawCommand::Text {
            text: "HP".into(),
            x: 4.0,
            y: 4.0,
            color: Color::WHITE,
            scale: 1.0,
        }];
        let mut out = Vec::new();
        amigo_engine::ui_bridge::emit_ui_sprites(&commands, ctx, &mut out, TextureId(0));
        out
    };
    assert!(ui_text(&ctx).iter().all(|s| s.texture_id == pixel_page));
    ctx.fonts.set_default_font(hack).expect("hack is loaded");
    let sprites = ui_text(&ctx);
    assert!(!sprites.is_empty());
    assert!(sprites.iter().all(|s| s.texture_id == hack_page));
}
