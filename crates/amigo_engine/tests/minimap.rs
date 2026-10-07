//! Minimap rendering through `DrawContext::draw_minimap`, and click-to-jump.

use amigo_engine::amigo_core::fog_of_war::{FogOfWarGrid, TileVisibility};
use amigo_engine::prelude::*;
use amigo_render::minimap::{Minimap, MinimapConfig, MinimapPin, MinimapStyle, PinType};
use amigo_render::texture::TextureId;

fn minimap(click_to_jump: bool) -> Minimap {
    Minimap::new(MinimapConfig {
        screen_pos: RenderVec2::new(10.0, 20.0),
        size: (16, 16),
        world_bounds: Rect::new(0.0, 0.0, 16.0, 16.0),
        style: MinimapStyle {
            border_color: None,
            ..Default::default()
        },
        click_to_jump,
    })
}

fn grass_and_water() -> TileLayer {
    let mut layer = TileLayer::new("ground", 16, 16);
    layer.fill_rect(0, 0, 8, 16, TileId(1));
    layer.fill_rect(8, 0, 8, 16, TileId(2));
    layer
}

fn color(id: TileId) -> Color {
    match id.0 {
        1 => Color::GREEN,
        2 => Color::BLUE,
        _ => Color::BLACK,
    }
}

fn draw(
    ctx: &GameContext,
    camera: &Camera,
    f: impl FnOnce(&mut DrawContext),
) -> (Vec<SpriteInstance>, Vec<SpriteInstance>) {
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
    (world, screen)
}

#[test]
fn the_minimap_is_drawn_as_one_screen_space_texture() {
    let ctx = GameContext::new(320.0, 180.0, "assets");
    let mut map = minimap(false);
    map.add_pin(MinimapPin::static_pin(
        SimVec2::new(Fix::from_num(12), Fix::from_num(4)),
        PinType::Dot { color: Color::RED },
    ));
    let mut camera = Camera::new(64.0, 64.0);
    camera.position = RenderVec2::new(32.0, 32.0); // tiles 0..8, 8 px each
    let layer = grass_and_water();

    let (world, screen) = draw(&ctx, &camera, |d| {
        d.set_z(50);
        d.draw_minimap(&map, &layer, 8.0, 8.0, color, None);
    });
    assert!(world.is_empty(), "a HUD element, not part of the world");
    assert_eq!(screen.len(), 1);
    let quad = &screen[0];
    assert_eq!(
        (quad.x, quad.y, quad.width, quad.height),
        (10.0, 20.0, 16.0, 16.0)
    );
    assert_eq!(quad.z_order, 50);

    let pending = ctx.textures().take_pending();
    assert_eq!(pending.len(), 1);
    let (id, image) = &pending[0];
    assert_eq!(*id, quad.texture_id);
    assert_eq!(image.dimensions(), (16, 16));
    // Grass on the left half, water on the right, the pin in red.
    assert_eq!(image.get_pixel(10, 12).0, [0, 0, 255, 255]);
    assert_eq!(image.get_pixel(2, 12).0[1], 255);
    assert_eq!(image.get_pixel(12, 4).0, [255, 0, 0, 255]);
    // The camera's view (tiles 0..8) outlined in the indicator colour.
    assert_eq!(image.get_pixel(0, 0).0, [255, 255, 255, 255]);

    // The next frame reuses the texture.
    let (_, again) = draw(&ctx, &camera, |d| {
        d.draw_minimap(&map, &layer, 8.0, 8.0, color, None)
    });
    assert_eq!(again[0].texture_id, quad.texture_id);
}

#[test]
fn fog_hides_what_was_never_seen() {
    let ctx = GameContext::new(320.0, 180.0, "assets");
    let map = minimap(false);
    let mut fog = FogOfWarGrid::new(16, 16);
    for y in 0..16 {
        for x in 0..8 {
            fog.set_visibility(x, y, TileVisibility::Visible);
        }
    }
    let mut camera = Camera::new(8.0, 8.0);
    camera.position = RenderVec2::new(4.0, 4.0);
    draw(&ctx, &camera, |d| {
        d.draw_minimap(&map, &grass_and_water(), 8.0, 8.0, color, Some(&fog))
    });
    let (_, image) = ctx.textures().take_pending().remove(0);
    assert_eq!(image.get_pixel(4, 10).0[1], 255, "visible grass");
    assert_eq!(
        image.get_pixel(12, 10).0,
        [0, 0, 0, 255],
        "hidden water is shroud"
    );
}

#[test]
fn clicking_the_minimap_moves_the_camera() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let map = minimap(true);
    ctx.input.set_mouse_ui_pos(RenderVec2::new(18.0, 24.0)); // minimap (8, 4)
    assert!(!ctx.minimap_click(&map, 16.0, 16.0), "no click, no jump");
    ctx.input
        .handle_mouse_button(MouseButton::Left, winit::event::ElementState::Pressed);
    assert!(ctx.minimap_click(&map, 16.0, 16.0));
    assert_eq!(ctx.camera.position, RenderVec2::new(128.0, 64.0));

    let off = minimap(false);
    assert!(!ctx.minimap_click(&off, 16.0, 16.0));
}
