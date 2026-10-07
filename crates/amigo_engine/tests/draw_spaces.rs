//! `DrawContext` draw spaces, default z, draw-time camera and parallax
//! (rendering-extensions R3, R4, R9). CPU only: these assert on the lists the
//! renderer would batch.

use amigo_engine::prelude::*;
use amigo_render::texture::TextureId;

fn ctx() -> GameContext {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    ctx.register_sprite_texture("hero".into(), TextureId(3), 16, 16);
    ctx.register_sprite_texture("tiles".into(), TextureId(7), 64, 32);
    ctx
}

struct Lists {
    world: Vec<SpriteInstance>,
    screen: Vec<SpriteInstance>,
}

fn draw(ctx: &GameContext, camera: &Camera, f: impl FnOnce(&mut DrawContext)) -> Lists {
    let mut world = Vec::new();
    let mut screen = Vec::new();
    let mut d = DrawContext::new(
        &mut world,
        ctx,
        camera.effective_position(),
        camera.virtual_width,
        camera.virtual_height,
        0.0,
        TextureId(0),
    )
    .with_camera(camera)
    .with_screen_list(&mut screen);
    f(&mut d);
    Lists { world, screen }
}

fn camera_at(x: f32, y: f32) -> Camera {
    let mut camera = Camera::new(320.0, 180.0);
    camera.position = RenderVec2::new(x, y);
    camera
}

#[test]
fn draws_go_to_the_list_of_the_current_space() {
    let ctx = ctx();
    let lists = draw(&ctx, &camera_at(0.0, 0.0), |d| {
        assert_eq!(d.space(), DrawSpace::World);
        d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
        d.set_space(DrawSpace::Screen);
        d.draw_sprite("hero", RenderVec2::new(4.0, 4.0));
        d.draw_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::BLUE);
        d.set_space(DrawSpace::World);
        d.draw_sprite("hero", RenderVec2::new(8.0, 8.0));
    });
    assert_eq!(lists.world.len(), 2);
    assert_eq!(lists.screen.len(), 2);
    assert_eq!(lists.screen[0].texture_id, TextureId(3));
    assert_eq!(lists.world[1].x, 8.0);
}

#[test]
fn in_space_restores_the_previous_space() {
    let ctx = ctx();
    let lists = draw(&ctx, &camera_at(0.0, 0.0), |d| {
        let inner = d.in_space(DrawSpace::Screen, |d| {
            d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
            d.space()
        });
        assert_eq!(inner, DrawSpace::Screen);
        assert_eq!(d.space(), DrawSpace::World);
        d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
    });
    assert_eq!((lists.world.len(), lists.screen.len()), (1, 1));
}

#[test]
fn a_context_without_a_screen_list_drops_screen_draws() {
    let ctx = ctx();
    let mut world = Vec::new();
    let mut d = DrawContext::new(
        &mut world,
        &ctx,
        RenderVec2::ZERO,
        320.0,
        180.0,
        0.0,
        TextureId(0),
    );
    d.set_space(DrawSpace::Screen);
    d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
    d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
    assert!(world.is_empty());
}

#[test]
fn set_z_applies_to_sprites_rects_and_tiles_but_not_text() {
    let mut ctx = ctx();
    ctx.fonts.load_builtin(7.0).expect("builtin font");
    for atlas in ctx.fonts.iter_mut() {
        atlas.texture_id = Some(TextureId(9));
    }
    let mut layer = TileLayer::new("ground", 2, 2);
    layer.fill_rect(0, 0, 2, 2, TileId(1));
    let lists = draw(&ctx, &camera_at(160.0, 90.0), |d| {
        d.set_z(5);
        d.draw_sprite("hero", RenderVec2::ZERO);
        d.draw_sprite_ex("hero", RenderVec2::ZERO, |_| {});
        d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
        d.draw_tilemap_sprite(&layer, 16.0, 16.0, "tiles", 4);
        d.draw_text("A", 0.0, 0.0, Color::WHITE);
    });
    let (text, rest): (Vec<_>, Vec<_>) = lists
        .world
        .iter()
        .partition(|s| s.texture_id == TextureId(9));
    assert!(!text.is_empty());
    assert!(text.iter().all(|s| s.z_order == 100));
    assert_eq!(rest.len(), 3 + 4);
    assert!(rest.iter().all(|s| s.z_order == 5));
}

#[test]
fn set_camera_position_moves_the_view_immediately() {
    let ctx = ctx();
    let mut camera = camera_at(10.0, 10.0);
    camera.set_zoom_immediate(2.0);
    draw(&ctx, &camera, |d| {
        d.set_camera_position(RenderVec2::new(500.0, 300.0));
        d.set_camera_offset(RenderVec2::new(3.0, -2.0));
        let pos = d.camera_position();
        assert_eq!(pos, RenderVec2::new(503.0, 298.0));
        assert_eq!(d.camera_pos, pos);
        let view = d.view_rect();
        // Zoom 2: a 160x90 world view centred on the camera.
        assert_eq!((view.w, view.h), (160.0, 90.0));
        assert_eq!(view.x + view.w / 2.0, 503.0);
        assert_eq!(view.y + view.h / 2.0, 298.0);
    });
    // GameContext's camera is the caller's; the override never touched it.
    assert_eq!(camera.position, RenderVec2::new(10.0, 10.0));
}

#[test]
fn parallax_shifts_world_draws_by_the_camera() {
    let ctx = ctx();
    let lists = draw(&ctx, &camera_at(100.0, 50.0), |d| {
        d.set_parallax(0.5, 0.0);
        d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
        let view = d.view_rect();
        // The view is shifted the opposite way, so culling matches.
        assert_eq!((view.x, view.y), (100.0 - 160.0 - 50.0, 50.0 - 90.0 - 50.0));
        d.set_space(DrawSpace::Screen);
        d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
        assert_eq!(d.view_rect(), Rect::new(0.0, 0.0, 320.0, 180.0));
    });
    assert_eq!((lists.world[0].x, lists.world[0].y), (50.0, 50.0));
    // Screen space ignores parallax.
    assert_eq!((lists.screen[0].x, lists.screen[0].y), (0.0, 0.0));
}

#[test]
fn non_finite_parallax_is_one() {
    let ctx = ctx();
    draw(&ctx, &camera_at(0.0, 0.0), |d| {
        d.set_parallax(f32::NAN, f32::INFINITY);
        assert_eq!(d.parallax(), (1.0, 1.0));
    });
}

#[test]
fn the_factor_resets_with_every_frame() {
    let ctx = ctx();
    let camera = camera_at(100.0, 50.0);
    draw(&ctx, &camera, |d| d.set_parallax(0.25, 0.25));
    let lists = draw(&ctx, &camera, |d| {
        assert_eq!(d.parallax(), (1.0, 1.0));
        assert_eq!(d.space(), DrawSpace::World);
        d.draw_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::RED);
    });
    assert_eq!(lists.world[0].z_order, 0);
    assert_eq!(lists.world[0].x, 0.0);
}

#[test]
fn tilemaps_use_the_layers_scroll_factor() {
    let ctx = ctx();
    let mut layer = TileLayer::new("far", 1, 1);
    layer.fill_rect(0, 0, 1, 1, TileId(1));
    layer.scroll_factor_x = 0.5;
    layer.scroll_factor_y = 1.0;
    // The tile at (0, 0) lands at (100, 0) on screen-relative coordinates,
    // which the camera at (200, 0) shows near its centre.
    let lists = draw(&ctx, &camera_at(200.0, 0.0), |d| {
        d.set_parallax(0.0, 0.0);
        d.draw_tilemap_sprite(&layer, 16.0, 16.0, "tiles", 4);
        d.draw_tilemap_colored(&layer, 16.0, 16.0, |_| Some(Color::WHITE));
        // The context's own factor is back afterwards.
        assert_eq!(d.parallax(), (0.0, 0.0));
    });
    assert_eq!(lists.world.len(), 2);
    for tile in &lists.world {
        assert_eq!((tile.x, tile.y), (100.0, 0.0));
    }
}
