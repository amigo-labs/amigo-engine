//! `DrawContext` CPU side: animated sprites from Aseprite files, and tilemap
//! culling. No GPU: these assert on the sprite list the renderer would batch.

use amigo_engine::prelude::*;
use amigo_render::texture::TextureId;

fn draw<R>(
    ctx: &GameContext,
    view: Rect,
    f: impl FnOnce(&mut DrawContext) -> R,
) -> Vec<SpriteInstance> {
    let mut sprites = Vec::new();
    let mut draw_ctx = DrawContext::new(
        &mut sprites,
        ctx,
        RenderVec2::new(view.x + view.w / 2.0, view.y + view.h / 2.0),
        view.w,
        view.h,
        0.0,
        TextureId(0),
    )
    .with_view(view);
    f(&mut draw_ctx);
    sprites
}

fn filled_layer(width: u32, height: u32) -> TileLayer {
    let mut layer = TileLayer::new("ground", width, height);
    layer.fill_rect(0, 0, width, height, TileId(1));
    layer
}

fn ctx_with_tileset() -> GameContext {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    // A 64x32 tileset of 16px tiles: 4 columns, 2 rows.
    ctx.register_sprite_texture("tiles".into(), TextureId(7), 64, 32);
    ctx
}

#[test]
fn only_tiles_in_view_are_drawn() {
    let ctx = ctx_with_tileset();
    let layer = filled_layer(100, 100);
    let view = Rect::new(0.0, 0.0, 320.0, 180.0);

    let sprites = draw(&ctx, view, |d| {
        d.draw_tilemap_sprite(&layer, 16.0, 16.0, "tiles", 4)
    });
    // 320/16 = 20 columns, 180/16 = 11.25 -> 12 rows, plus one tile of
    // margin on the far side (the near side is clamped at 0).
    assert_eq!(sprites.len(), 21 * 13);

    let colored = draw(&ctx, view, |d| {
        d.draw_tilemap_colored(&layer, 16.0, 16.0, |_| Some(Color::WHITE))
    });
    assert_eq!(colored.len(), sprites.len());

    // A view in the middle of the map gets a margin on both sides.
    let middle = draw(&ctx, Rect::new(800.0, 800.0, 320.0, 180.0), |d| {
        d.draw_tilemap_sprite(&layer, 16.0, 16.0, "tiles", 4)
    });
    assert_eq!(middle.len(), 22 * 14);
    assert!(middle.iter().all(|s| s.x >= 784.0 && s.x < 1136.0));
}

#[test]
fn nothing_is_drawn_off_map_or_for_hidden_layers() {
    let ctx = ctx_with_tileset();
    let mut layer = filled_layer(10, 10);
    let far_away = Rect::new(5000.0, 5000.0, 320.0, 180.0);
    assert!(
        draw(&ctx, far_away, |d| d
            .draw_tilemap_sprite(&layer, 16.0, 16.0, "tiles", 4))
        .is_empty()
    );

    layer.visible = false;
    let view = Rect::new(0.0, 0.0, 320.0, 180.0);
    assert!(
        draw(&ctx, view, |d| d
            .draw_tilemap_sprite(&layer, 16.0, 16.0, "tiles", 4))
        .is_empty()
    );
}

#[test]
fn degenerate_tile_sizes_and_zero_columns_do_not_panic() {
    let ctx = ctx_with_tileset();
    let mut layer = filled_layer(4, 1);
    layer.set(3, 0, TileId(6)); // second row, second column of a 4-wide set
    let view = Rect::new(0.0, 0.0, 320.0, 180.0);

    // `columns == 0` used to divide by zero; it now comes from the texture.
    let sprites = draw(&ctx, view, |d| {
        d.draw_tilemap_sprite(&layer, 16.0, 16.0, "tiles", 0)
    });
    assert_eq!(sprites.len(), 4);
    assert_eq!((sprites[3].uv_x, sprites[3].uv_y), (0.25, 0.5));

    for size in [0.0, -16.0, f32::NAN] {
        assert!(
            draw(&ctx, view, |d| d
                .draw_tilemap_sprite(&layer, size, 16.0, "tiles", 4))
            .is_empty()
        );
    }
}

#[test]
fn animated_sprites_draw_one_frame_of_their_aseprite_strip() {
    let assets = std::env::temp_dir().join(format!("amigo_anim_{}", std::process::id()));
    std::fs::create_dir_all(assets.join("sprites")).unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../amigo_assets/tests/fixtures/strip.aseprite"
        ),
        assets.join("sprites/strip.aseprite"),
    )
    .unwrap();

    let mut ctx = GameContext::new(320.0, 180.0, assets.to_str().unwrap());
    ctx.assets.load_sprites().unwrap();
    // What the engine does after uploading the strip.
    ctx.register_sprite_texture("strip".into(), TextureId(3), 16, 2);

    let mut player = AnimPlayer::new("strip/walk");
    let view = Rect::new(0.0, 0.0, 320.0, 180.0);
    let first = draw(&ctx, view, |d| {
        d.draw_animated("strip", &player, RenderVec2::new(10.0, 20.0))
    });
    assert_eq!(first.len(), 1);
    let s = &first[0];
    assert_eq!((s.width, s.height), (4.0, 2.0), "one frame, not the strip");
    assert_eq!((s.uv_x, s.uv_w), (0.0, 0.25));

    // The first walk frame lasts 100 ms = 6 ticks.
    for _ in 0..6 {
        player.advance(ctx.assets.animations());
    }
    let flipped = draw(&ctx, view, |d| {
        d.draw_animated_ex("strip", &player, RenderVec2::new(10.0, 20.0), |s| {
            s.flip_x = true
        })
    });
    assert_eq!(flipped[0].uv_x, 0.25);
    assert!(flipped[0].flip_x);

    player.play("strip/nope", PlayMode::Loop);
    assert!(
        draw(&ctx, view, |d| d.draw_animated(
            "strip",
            &player,
            RenderVec2::ZERO
        ))
        .is_empty()
    );

    let _ = std::fs::remove_dir_all(&assets);
}
