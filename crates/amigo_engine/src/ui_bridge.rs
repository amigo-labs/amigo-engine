//! Turning [`UiDrawCommand`]s into sprites.
//!
//! `amigo_ui` implements a full immediate-mode widget set — HUD text, panels,
//! progress bars, buttons, sliders, dropdowns, trees — and produces a
//! `Vec<UiDrawCommand>` from it. Nothing consumed that list: outside `amigo_ui`
//! itself, `UiDrawCommand` appeared only in `amigo_engine`'s prelude re-export.
//! Every widget a game built therefore drew nothing at all.
//!
//! UI is rendered in its own pass, after post-processing and in screen space, per
//! docs/specs/conventions.md A.6. That is why these sprites go into the
//! renderer's `ui_batcher` rather than the world draw list: sharing the world
//! batch would run the HUD through bloom and CRT curvature, and scale it with the
//! camera's zoom.

use crate::context::{DrawContext, GameContext};
use amigo_core::{Color, Rect};
use amigo_render::sprite_batcher::SpriteInstance;
use amigo_render::texture::TextureId;
use amigo_ui::UiDrawCommand;

/// Thickness of a `Rect { filled: false }` outline, in virtual pixels.
const OUTLINE_THICKNESS: f32 = 1.0;

/// Translate `commands` into sprite instances appended to `out`.
///
/// `white_texture` backs solid rectangles; text and sprites resolve their own
/// textures through `ctx`.
pub fn emit_ui_sprites(
    commands: &[UiDrawCommand],
    ctx: &GameContext,
    out: &mut Vec<SpriteInstance>,
    white_texture: TextureId,
) {
    if commands.is_empty() {
        return;
    }

    // A DrawContext gives us the existing glyph layout and sprite lookup rather
    // than a second copy of both. The camera position is zero because UI is
    // screen space: the UI pass uses its own projection with no camera in it.
    let mut draw = DrawContext::new(
        out,
        ctx,
        amigo_core::RenderVec2 { x: 0.0, y: 0.0 },
        ctx.camera.virtual_width,
        ctx.camera.virtual_height,
        0.0,
        white_texture,
    );

    for command in commands {
        match command {
            UiDrawCommand::Text {
                text,
                x,
                y,
                color,
                scale,
            } => {
                draw.draw_text_scaled(text, *x, *y, *color, *scale);
            }
            UiDrawCommand::Rect {
                rect,
                color,
                filled,
            } => {
                if *filled {
                    draw.draw_rect(*rect, *color);
                } else {
                    emit_outline(&mut draw, *rect, *color);
                }
            }
            UiDrawCommand::Sprite { name, x, y } => {
                draw.draw_sprite(name, amigo_core::RenderVec2 { x: *x, y: *y });
            }
            UiDrawCommand::ProgressBar {
                rect,
                fraction,
                color,
                bg_color,
            } => {
                draw.draw_rect(*rect, *bg_color);
                let fraction = fraction.clamp(0.0, 1.0);
                if fraction > 0.0 {
                    draw.draw_rect(Rect::new(rect.x, rect.y, rect.w * fraction, rect.h), *color);
                }
            }
        }
    }
}

/// Four thin rectangles, since the sprite pipeline has no line primitive.
fn emit_outline(draw: &mut DrawContext<'_>, rect: Rect, color: Color) {
    let t = OUTLINE_THICKNESS;
    // Top and bottom span the full width; the sides sit between them so the
    // corners are not drawn twice (visible with a translucent colour).
    draw.draw_rect(Rect::new(rect.x, rect.y, rect.w, t), color);
    draw.draw_rect(Rect::new(rect.x, rect.y + rect.h - t, rect.w, t), color);
    let inner_h = (rect.h - 2.0 * t).max(0.0);
    if inner_h > 0.0 {
        draw.draw_rect(Rect::new(rect.x, rect.y + t, t, inner_h), color);
        draw.draw_rect(
            Rect::new(rect.x + rect.w - t, rect.y + t, t, inner_h),
            color,
        );
    }
}
