//! Two `status: done` subsystems that had no consumer:
//!
//! * `DebugOverlay::overlay_lines()` was called by nothing but its own unit test,
//!   so F1–F8 flipped flags that were never read out — the overlay was invisible.
//! * `PostProcessPipeline` was complete but never instantiated by any render
//!   path, so `PostEffect` could not reach the screen.
//!
//! There is no GPU here, so these assert on the CPU-side draw list and on the
//! effect stack the renderer would consume — the parts that decide whether the
//! GPU is asked to do anything at all.

use amigo_engine::prelude::*;

/// Draw overlay text the way the engine's frame does, and report the sprites.
fn draw_overlay(ctx: &GameContext, lines: &[(String, Color)]) -> Vec<SpriteInstance> {
    let mut sprites = Vec::new();
    let mut draw_ctx = DrawContext::new(
        &mut sprites,
        ctx,
        RenderVec2 { x: 0.0, y: 0.0 },
        320.0,
        180.0,
        0.0,
        amigo_render::texture::TextureId(0),
    );
    let mut y = 4.0;
    for (text, color) in lines {
        draw_ctx.draw_text(text, 4.0, y, *color);
        y += 9.0;
    }
    sprites
}

fn ctx_with_font() -> GameContext {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    ctx.fonts
        .load_builtin(7.0)
        .expect("built-in font is embedded in the binary");
    // The engine uploads atlases to the GPU and stores the id; without a GPU,
    // stand in for that so text rendering has a texture to point at.
    for atlas in ctx.fonts.iter_mut() {
        atlas.texture_id = Some(amigo_render::texture::TextureId(1));
    }
    ctx
}

#[test]
fn a_hidden_overlay_produces_no_lines() {
    let debug = DebugOverlay::new();
    assert!(
        debug.overlay_lines().is_empty(),
        "the overlay starts hidden, so F1 has something to reveal"
    );
}

#[test]
fn a_visible_overlay_reports_fps_and_counts() {
    let mut debug = DebugOverlay::new();
    debug.toggle();
    debug.update(1.0 / 60.0, 42, 7);

    let lines = debug.overlay_lines();

    assert!(!lines.is_empty(), "F1 must produce something to draw");
    let text: String = lines.iter().map(|(t, _)| t.as_str()).collect();
    assert!(text.contains("Entities: 42"), "got: {text}");
    assert!(text.contains("Draw calls: 7"), "got: {text}");
}

#[test]
fn overlay_lines_become_sprites() {
    let ctx = ctx_with_font();
    let mut debug = DebugOverlay::new();
    debug.toggle();
    debug.update(1.0 / 60.0, 3, 1);
    let lines = debug.overlay_lines();
    assert!(!lines.is_empty());

    let sprites = draw_overlay(&ctx, &lines);

    assert!(
        !sprites.is_empty(),
        "overlay text must reach the sprite list; before this was wired, \
         overlay_lines() had no caller at all"
    );
}

#[test]
fn overlay_text_is_drawn_in_the_colour_the_overlay_asked_for() {
    let ctx = ctx_with_font();
    let lines = vec![("RED".to_string(), Color::RED)];

    let sprites = draw_overlay(&ctx, &lines);

    assert!(!sprites.is_empty());
    assert!(
        sprites.iter().all(|s| s.tint == Color::RED),
        "glyphs should carry the line's colour, so a bad FPS reads red"
    );
}

#[test]
fn no_font_means_no_sprites_rather_than_a_panic() {
    // A game that never loaded a font still has to survive pressing F1.
    let ctx = GameContext::new(320.0, 180.0, "assets");
    let lines = vec![("FPS: 60".to_string(), Color::WHITE)];

    let sprites = draw_overlay(&ctx, &lines);

    assert!(sprites.is_empty());
}

#[test]
fn post_effects_default_to_an_empty_stack() {
    let ctx = GameContext::new(320.0, 180.0, "assets");
    assert!(
        ctx.post_effects.is_empty(),
        "no effects means the scene draws straight to the surface, no extra pass"
    );
}

#[test]
fn a_game_can_declare_post_effects_from_update() {
    struct Effected;
    impl Game for Effected {
        fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
            ctx.post_effects = vec![
                PostEffect::Vignette {
                    intensity: 0.4,
                    smoothness: 0.5,
                },
                PostEffect::ColorGrading {
                    brightness: 1.0,
                    contrast: 1.1,
                    saturation: 0.9,
                },
            ];
            SceneAction::Continue
        }
        fn draw(&self, _ctx: &mut DrawContext) {}
    }

    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let mut stack = GameStack::new(Box::new(Effected));
    stack.enter_root(&mut ctx);
    let action = stack.top_mut().unwrap().update(&mut ctx);
    stack.apply(action, &mut ctx);

    assert_eq!(ctx.post_effects.len(), 2);
    // The engine only re-uploads when the stack differs, so equality is what
    // decides whether the GPU sees a change.
    let same = ctx.post_effects.clone();
    assert_eq!(ctx.post_effects, same);
    assert_ne!(ctx.post_effects, Vec::<PostEffect>::new());
}

// ---------------------------------------------------------------------------
// Pixel UI bridge
// ---------------------------------------------------------------------------

mod ui {
    use super::*;
    use amigo_engine::ui_bridge::emit_ui_sprites;
    use amigo_render::texture::TextureId;

    const WHITE: TextureId = TextureId(0);

    fn emit(ctx: &GameContext) -> Vec<SpriteInstance> {
        let mut out = Vec::new();
        emit_ui_sprites(ctx.ui.draw_commands(), ctx, &mut out, WHITE);
        out
    }

    #[test]
    fn a_filled_rect_becomes_one_sprite() {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.ui.begin();
        ctx.ui
            .filled_rect(Rect::new(10.0, 20.0, 30.0, 40.0), Color::RED);

        let sprites = emit(&ctx);

        assert_eq!(sprites.len(), 1);
        assert_eq!((sprites[0].x, sprites[0].y), (10.0, 20.0));
        assert_eq!((sprites[0].width, sprites[0].height), (30.0, 40.0));
        assert_eq!(sprites[0].tint, Color::RED);
    }

    #[test]
    fn an_outline_becomes_four_edges_that_do_not_overlap() {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.ui.begin();
        ctx.ui
            .rect_outline(Rect::new(0.0, 0.0, 10.0, 10.0), Color::WHITE);

        let sprites = emit(&ctx);

        assert_eq!(sprites.len(), 4, "top, bottom and two sides");
        // Total area must equal the outline's area exactly: 2 full-width bars
        // plus two sides of the remaining height. Overlapping corners would
        // double-draw and show through a translucent colour.
        let area: f32 = sprites.iter().map(|s| s.width * s.height).sum();
        assert_eq!(area, 10.0 + 10.0 + 8.0 + 8.0);
    }

    #[test]
    fn a_progress_bar_draws_background_then_fill() {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.ui.begin();
        ctx.ui
            .progress_bar(Rect::new(0.0, 0.0, 100.0, 10.0), 0.25, Color::GREEN);

        let sprites = emit(&ctx);

        assert_eq!(sprites.len(), 2, "background plus fill");
        assert_eq!(sprites[0].width, 100.0, "background spans the full width");
        assert_eq!(sprites[1].width, 25.0, "fill is 25% of the width");
    }

    #[test]
    fn a_full_or_empty_progress_bar_stays_inside_its_rect() {
        for (fraction, expected_sprites) in [(0.0, 1), (1.0, 2)] {
            let mut ctx = GameContext::new(320.0, 180.0, "assets");
            ctx.ui.begin();
            ctx.ui
                .progress_bar(Rect::new(0.0, 0.0, 50.0, 5.0), fraction, Color::GREEN);

            let sprites = emit(&ctx);

            assert_eq!(
                sprites.len(),
                expected_sprites,
                "fraction {fraction} should not emit an empty fill"
            );
            assert!(sprites.iter().all(|s| s.width <= 50.0));
        }
    }

    #[test]
    fn an_out_of_range_fraction_is_clamped() {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.ui.begin();
        ctx.ui
            .progress_bar(Rect::new(0.0, 0.0, 40.0, 4.0), 3.0, Color::GREEN);

        let sprites = emit(&ctx);

        assert!(
            sprites.iter().all(|s| s.width <= 40.0),
            "a fraction above 1.0 must not spill past the bar"
        );
    }

    #[test]
    fn ui_text_reaches_the_sprite_list() {
        let ctx = ctx_with_font();
        let mut ctx = ctx;
        ctx.ui.begin();
        ctx.ui.pixel_text("HP", 4.0, 4.0, Color::WHITE);

        let sprites = emit(&ctx);

        assert!(
            !sprites.is_empty(),
            "UiDrawCommand::Text had no consumer before the bridge existed"
        );
    }

    #[test]
    fn scaled_ui_text_is_larger_than_unscaled() {
        let base = {
            let mut ctx = ctx_with_font();
            ctx.ui.begin();
            ctx.ui.pixel_text("M", 0.0, 0.0, Color::WHITE);
            emit(&ctx)
        };
        let scaled = {
            let mut ctx = ctx_with_font();
            ctx.ui.begin();
            ctx.ui.pixel_text_scaled("M", 0.0, 0.0, Color::WHITE, 3.0);
            emit(&ctx)
        };

        assert_eq!(base.len(), scaled.len(), "same glyph count");
        assert!(
            scaled[0].width > base[0].width,
            "scale must reach the glyph quads: {} vs {}",
            scaled[0].width,
            base[0].width
        );
    }

    #[test]
    fn begin_clears_the_previous_frames_widgets() {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.ui.begin();
        ctx.ui
            .filled_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
        assert_eq!(emit(&ctx).len(), 1);

        ctx.ui.begin();

        assert!(
            emit(&ctx).is_empty(),
            "without this the command list would grow every tick"
        );
    }

    #[test]
    fn no_widgets_means_no_sprites() {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.ui.begin();

        assert!(emit(&ctx).is_empty());
    }
}
