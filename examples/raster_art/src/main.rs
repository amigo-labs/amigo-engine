//! Raster art at the window's own resolution (rendering-extensions):
//!
//! - `art_style = "raster_art"` with `scale_mode = "expand"`: the stage keeps
//!   its height and widens to the window, with no bars;
//! - a rotating sprite from an atlas manifest, drawn about its pivot;
//! - additive sparks;
//! - screen-space text with umlauts in an embedded TTF, on rounded panels;
//! - a custom post shader that pulses a vignette.
//!
//! The sprite sheet and its manifest are generated into a temporary assets
//! directory at startup, so the example ships no binary files.

use amigo_engine::prelude::*;
use std::path::PathBuf;

/// A two-frame sheet: a blue and an orange diamond, 64x64 each, with a
/// 64 px transparent gutter so the sheet can have mipmaps (`mip_levels: 6`).
fn write_assets() -> PathBuf {
    let root = std::env::temp_dir().join("amigo_raster_art").join("assets");
    let sprites = root.join("sprites");
    if let Err(e) = std::fs::create_dir_all(&sprites) {
        eprintln!("cannot create {}: {e}", sprites.display());
    }
    let mut sheet = image::RgbaImage::new(192, 64);
    for (frame_x, color) in [(0u32, [80u8, 160, 255]), (128, [255, 150, 40])] {
        for y in 0..64u32 {
            for x in 0..64u32 {
                let d = (x as i32 - 32).abs() + (y as i32 - 32).abs();
                if d < 30 {
                    let shade = 255 - (d as u8) * 4;
                    sheet.put_pixel(
                        frame_x + x,
                        y,
                        image::Rgba([
                            (color[0] as u32 * shade as u32 / 255) as u8,
                            (color[1] as u32 * shade as u32 / 255) as u8,
                            (color[2] as u32 * shade as u32 / 255) as u8,
                            255,
                        ]),
                    );
                }
            }
        }
    }
    if let Err(e) = sheet.save(sprites.join("gems.png")) {
        eprintln!("cannot write the sheet: {e}");
    }
    let manifest = r#"(
    image: "gems.png",
    mip_levels: 6,
    sprites: {
        "gem": (
            frames: [(x: 0, y: 0, w: 64, h: 64), (x: 128, y: 0, w: 64, h: 64)],
            origin: (32.0, 32.0),
            fps: Some(2.0),
            looping: true,
        ),
    },
)"#;
    if let Err(e) = std::fs::write(sprites.join("gems.atlas.ron"), manifest) {
        eprintln!("cannot write the manifest: {e}");
    }
    root
}

const PULSE: &str = r#"
@fragment
fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(scene, scene_sampler, in.uv);
    let d = distance(in.uv, vec2<f32>(0.5, 0.5));
    let strength = post.params[0].x * (0.75 + 0.25 * sin(post.time * 2.0));
    let shade = 1.0 - smoothstep(0.3, 0.9, d) * strength;
    return vec4<f32>(color.rgb * shade, color.a);
}
"#;

struct RasterArt {
    font: Option<FontId>,
    gem: AnimPlayer,
    ticks: u64,
}

impl Game for RasterArt {
    fn init(&mut self, ctx: &mut GameContext) {
        self.font = ctx.load_font(epaint_default_fonts::UBUNTU_LIGHT, 18.0).ok();
        if let Err(e) = ctx.register_post_shader("pulse", PULSE) {
            eprintln!("{e}");
        }
        let mut params = [0.0; 16];
        params[0] = 0.6;
        ctx.post_effects = vec![PostEffect::Custom {
            shader: "pulse".into(),
            params,
        }];
        ctx.particles.spawn(
            "sparks",
            EmitterConfig {
                max_particles: 200,
                emission_rate: 120.0,
                lifetime_min: 0.4,
                lifetime_max: 0.9,
                speed_min: 40.0,
                speed_max: 120.0,
                spread: std::f32::consts::PI,
                color_start: Color::new(1.0, 0.7, 0.2, 1.0),
                color_end: Color::new(1.0, 0.2, 0.0, 0.0),
                size_start: 4.0,
                size_end: 1.0,
                blend_mode: BlendMode::Additive,
                ..Default::default()
            },
            EmitterShape::Point,
            0.0,
            0.0,
        );
        self.gem.play("gem", PlayMode::Loop);
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        self.ticks += 1;
        self.gem.advance(ctx.assets.animations());
        ctx.camera.position = RenderVec2::ZERO;
        SceneAction::Continue
    }

    fn draw(&self, draw: &mut DrawContext) {
        let info = draw.viewport_info();
        let (w, h) = info.virtual_size;

        // Sky behind everything, in screen space so it fills any window shape.
        draw.in_space(DrawSpace::Screen, |d| {
            d.set_z(-100);
            d.draw_gradient_rect(
                Rect::new(0.0, 0.0, w, h),
                Color::new(0.05, 0.07, 0.2, 1.0),
                Color::new(0.3, 0.15, 0.35, 1.0),
            );
            d.set_z(0);
        });

        // A gem turning about its pivot, in the middle of the world.
        let angle = self.ticks as f32 / 60.0;
        draw.draw_animated_ex("gem", &self.gem, RenderVec2::ZERO, |s| {
            s.rotation = angle;
            s.scale(1.5, 1.5);
        });
        draw.draw_circle(
            RenderVec2::new(0.0, 70.0),
            12.0,
            Color::new(1.0, 1.0, 1.0, 0.4),
        );

        // HUD: rounded panels and text, after post-processing.
        draw.in_space(DrawSpace::Screen, |d| {
            d.set_z(10);
            d.draw_rounded_rect(
                Rect::new(12.0, 12.0, 250.0, 64.0),
                12.0,
                Color::new(0.0, 0.0, 0.0, 0.55),
            );
            let title = TextStyle {
                font: self.font,
                size_px: Some(18.0),
                ..Default::default()
            };
            d.draw_text_ex(
                "Grüße aus dem Rastermodus",
                RenderVec2::new(24.0, 20.0),
                &title,
            );
            let small = TextStyle {
                font: self.font,
                size_px: Some(12.0),
                color: Color::new(0.85, 0.85, 1.0, 1.0),
                letter_spacing: 0.5,
                ..Default::default()
            };
            let line = format!(
                "{:.0}x{:.0} virtual, scale {:.2} — äöü ß",
                w, h, info.render_scale
            );
            d.draw_text_ex(&line, RenderVec2::new(24.0, 48.0), &small);
            let centred = TextStyle {
                align: TextAlign::Center,
                ..small
            };
            d.draw_text_ex(
                "Fenstergröße ändern: die Bühne wächst mit",
                RenderVec2::new(w / 2.0, h - 28.0),
                &centred,
            );
        });
    }
}

fn main() {
    let assets = write_assets();
    let mut config = EngineConfig::load();
    config.render.virtual_width = 640;
    config.render.virtual_height = 360;
    config.render.art_style = "raster_art".into();
    config.render.scale_mode = "expand".into();
    config.window.title = "Raster Art".into();
    config.window.width = 1280;
    config.window.height = 720;
    Engine::build()
        .config(config)
        .assets_path(&assets.to_string_lossy())
        .build()
        .run(RasterArt {
            font: None,
            gem: AnimPlayer::new("gem"),
            ticks: 0,
        });
}
