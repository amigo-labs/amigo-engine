//! Drawing atlas sprites by frame (rendering-extensions R8).

use amigo_engine::SpriteEntry;
use amigo_engine::amigo_assets::SpriteFrame;
use amigo_engine::prelude::*;
use amigo_render::texture::TextureId;

fn ctx() -> GameContext {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    ctx.register_sprite_texture("dot".into(), TextureId(2), 3, 3);
    ctx.register_sprite(
        "hamster".into(),
        SpriteEntry {
            texture: TextureId(9),
            size: (64, 32),
            frames: vec![
                SpriteFrame::on_texture(0, 0, 16, 32, (64, 32), [8.0, 30.0]),
                SpriteFrame::on_texture(16, 0, 20, 30, (64, 32), [10.0, 28.0]),
            ],
            atlas: true,
        },
    );
    ctx
}

fn draw(ctx: &GameContext, f: impl FnOnce(&mut DrawContext)) -> Vec<SpriteInstance> {
    let mut sprites = Vec::new();
    let mut d = DrawContext::new(
        &mut sprites,
        ctx,
        RenderVec2::ZERO,
        320.0,
        180.0,
        0.0,
        TextureId(0),
    );
    f(&mut d);
    sprites
}

#[test]
fn draw_frame_takes_size_uvs_and_origin_from_the_frame() {
    let ctx = ctx();
    let sprites = draw(&ctx, |d| {
        d.draw_frame("hamster", 1, RenderVec2::new(100.0, 50.0));
        d.draw_frame("hamster", 99, RenderVec2::new(100.0, 50.0));
        d.draw_frame("nope", 0, RenderVec2::ZERO);
    });
    assert_eq!(sprites.len(), 2);
    for s in &sprites {
        assert_eq!(s.texture_id, TextureId(9));
        assert_eq!((s.width, s.height), (20.0, 30.0));
        assert_eq!((s.uv_x, s.uv_w), (0.25, 20.0 / 64.0));
        assert_eq!(s.origin, [10.0, 28.0]);
        assert_eq!((s.x, s.y), (100.0, 50.0), "pos is the pivot");
    }
}

#[test]
fn frame_counts() {
    let ctx = ctx();
    draw(&ctx, |d| {
        assert_eq!(d.frame_count("hamster"), 2);
        assert_eq!(d.frame_count("dot"), 1);
        assert_eq!(d.frame_count("nope"), 0);
    });
}

#[test]
fn draw_sprite_on_an_atlas_sprite_draws_its_first_frame() {
    let ctx = ctx();
    let sprites = draw(&ctx, |d| {
        d.draw_sprite("hamster", RenderVec2::new(1.0, 2.0));
        d.draw_sprite("dot", RenderVec2::new(1.0, 2.0));
    });
    assert_eq!((sprites[0].width, sprites[0].height), (16.0, 32.0));
    assert_eq!(sprites[0].origin, [8.0, 30.0]);
    assert_eq!(
        (sprites[1].width, sprites[1].height, sprites[1].uv_w),
        (3.0, 3.0, 1.0)
    );
    assert_eq!(sprites[1].origin, [0.0, 0.0]);
}

#[test]
fn draw_animated_on_an_atlas_sprite_draws_the_players_frame() {
    let ctx = ctx();
    let mut player = AnimPlayer::new("hamster");
    player.frame_index = 1;
    let sprites = draw(&ctx, |d| {
        d.draw_animated("hamster", &player, RenderVec2::new(5.0, 5.0));
    });
    assert_eq!(sprites.len(), 1);
    assert_eq!(sprites[0].origin, [10.0, 28.0]);
    assert_eq!(sprites[0].uv_x, 0.25);
}
