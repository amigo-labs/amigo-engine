//! Shapes through `DrawContext` (rendering-extensions R6): what reaches the
//! draw list. Tessellation itself is tested in `amigo_render::shapes`.

use amigo_engine::prelude::*;
use amigo_render::texture::TextureId;

fn draw(style: ArtStyle, f: impl FnOnce(&mut DrawContext)) -> Vec<SpriteInstance> {
    let ctx = GameContext::new(320.0, 180.0, "assets");
    let mut sprites = Vec::new();
    let mut d = DrawContext::new(
        &mut sprites,
        &ctx,
        RenderVec2::new(160.0, 90.0),
        320.0,
        180.0,
        0.0,
        TextureId(0),
    )
    .with_art_style(style)
    .with_render_scale(2.0);
    f(&mut d);
    sprites
}

fn every_shape(d: &mut DrawContext) {
    let p = |x, y| RenderVec2::new(x, y);
    d.draw_quad(
        [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
        Color::RED,
    );
    d.draw_quad_colors(
        [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)],
        [Color::RED, Color::GREEN, Color::BLUE, Color::WHITE],
    );
    d.draw_gradient_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLUE, Color::BLACK);
    d.draw_line(p(0.0, 0.0), p(10.0, 5.0), 2.0, Color::WHITE);
    d.draw_rect_outline(Rect::new(0.0, 0.0, 10.0, 10.0), 1.0, Color::WHITE);
    d.draw_circle(p(20.0, 20.0), 8.0, Color::WHITE);
    d.draw_rounded_rect(Rect::new(0.0, 0.0, 30.0, 20.0), 4.0, Color::WHITE);
    d.draw_convex_polygon(&[p(0.0, 0.0), p(5.0, 0.0), p(2.0, 4.0)], Color::WHITE);
}

#[test]
fn shapes_use_the_white_texture_geometry_and_the_default_z() {
    let sprites = draw(ArtStyle::PixelArt, |d| {
        d.set_z(5);
        every_shape(d);
    });
    assert!(!sprites.is_empty());
    for s in &sprites {
        assert_eq!(s.texture_id, TextureId(0));
        assert_eq!(s.z_order, 5);
        assert_eq!(s.blend, BlendMode::Normal);
        assert!(s.geometry.is_some());
    }
}

#[test]
fn raster_art_feathers_edges_and_pixel_art_does_not() {
    let clear_corner = |sprites: &[SpriteInstance]| {
        sprites
            .iter()
            .filter_map(|s| s.geometry)
            .any(|g| g.colors.iter().any(|c| c.a == 0.0))
    };
    let pixel = draw(ArtStyle::PixelArt, every_shape);
    let raster = draw(ArtStyle::RasterArt, every_shape);
    assert!(!clear_corner(&pixel));
    assert!(clear_corner(&raster));
    assert!(raster.len() > pixel.len());
}

#[test]
fn degenerate_shapes_draw_nothing() {
    let p = |x, y| RenderVec2::new(x, y);
    let sprites = draw(ArtStyle::RasterArt, |d| {
        d.draw_gradient_rect(Rect::new(0.0, 0.0, 0.0, 10.0), Color::BLUE, Color::BLACK);
        d.draw_gradient_rect(Rect::new(0.0, 0.0, 10.0, -1.0), Color::BLUE, Color::BLACK);
        d.draw_line(p(1.0, 1.0), p(1.0, 1.0), 2.0, Color::WHITE);
        d.draw_line(p(0.0, 0.0), p(5.0, 0.0), 0.0, Color::WHITE);
        d.draw_rect_outline(Rect::new(0.0, 0.0, 10.0, 10.0), -1.0, Color::WHITE);
        d.draw_rect_outline(Rect::new(0.0, 0.0, 0.0, 10.0), 1.0, Color::WHITE);
        d.draw_circle(p(0.0, 0.0), 0.0, Color::WHITE);
        d.draw_circle(p(0.0, 0.0), -3.0, Color::WHITE);
        d.draw_circle(p(0.0, 0.0), f32::NAN, Color::WHITE);
        d.draw_rounded_rect(Rect::new(0.0, 0.0, -5.0, 10.0), 2.0, Color::WHITE);
        d.draw_convex_polygon(&[p(0.0, 0.0), p(1.0, 1.0)], Color::WHITE);
        d.draw_convex_polygon(&[], Color::WHITE);
    });
    assert!(sprites.is_empty(), "{} sprites", sprites.len());
}

#[test]
fn a_rounded_rect_without_radius_is_a_plain_rect() {
    let sprites = draw(ArtStyle::PixelArt, |d| {
        d.draw_rounded_rect(Rect::new(2.0, 3.0, 10.0, 6.0), 0.0, Color::WHITE)
    });
    assert_eq!(sprites.len(), 1);
    let g = sprites[0].geometry.expect("geometry");
    assert_eq!(
        g.corners,
        [[2.0, 3.0], [12.0, 3.0], [12.0, 9.0], [2.0, 9.0]]
    );
}

#[test]
fn shapes_honour_screen_space_and_parallax() {
    let ctx = GameContext::new(320.0, 180.0, "assets");
    let mut world = Vec::new();
    let mut screen = Vec::new();
    let mut camera = Camera::new(320.0, 180.0);
    camera.position = RenderVec2::new(100.0, 0.0);
    let mut d = DrawContext::new(
        &mut world,
        &ctx,
        camera.effective_position(),
        320.0,
        180.0,
        0.0,
        TextureId(0),
    )
    .with_camera(&camera)
    .with_screen_list(&mut screen);
    d.set_parallax(0.5, 1.0);
    d.draw_circle(RenderVec2::new(0.0, 0.0), 4.0, Color::WHITE);
    d.in_space(DrawSpace::Screen, |d| {
        d.draw_circle(RenderVec2::new(0.0, 0.0), 4.0, Color::WHITE)
    });
    let world_x = world[0].geometry.expect("geometry").corners[0][0];
    let screen_x = screen[0].geometry.expect("geometry").corners[0][0];
    assert_eq!(world_x - screen_x, 50.0);
}
