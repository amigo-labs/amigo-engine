//! Default splash screen: "Powered by Amigo Engine".
//!
//! Shown for [`SPLASH_DURATION_SECS`] seconds when the engine starts, unless
//! disabled via [`EngineBuilder::splash(false)`](super::EngineBuilder::splash)
//! or `splash.enabled = false` in `amigo.toml`.

use amigo_core::Color;
use amigo_render::sprite_batcher::SpriteInstance;
use amigo_render::texture::TextureId;

/// How long the splash screen is displayed.
pub const SPLASH_DURATION_SECS: f64 = 2.0;

/// Splash screen state tracked inside the engine loop.
pub struct SplashState {
    pub elapsed: f64,
    pub finished: bool,
}

impl Default for SplashState {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            finished: false,
        }
    }
}

impl SplashState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance the splash timer. Returns `true` once the duration is over.
    pub fn tick(&mut self, dt: f64) -> bool {
        if self.finished {
            return true;
        }
        self.elapsed += dt;
        if self.elapsed >= SPLASH_DURATION_SECS {
            self.finished = true;
        }
        self.finished
    }

    /// Compute a fade alpha (0.0..1.0) for the splash text.
    /// Fades in during the first 0.3s, holds, then fades out during the last 0.4s.
    pub fn alpha(&self) -> f32 {
        let t = self.elapsed;
        let fade_in = 0.3;
        let fade_out_start = SPLASH_DURATION_SECS - 0.4;

        if t < fade_in {
            (t / fade_in) as f32
        } else if t > fade_out_start {
            ((SPLASH_DURATION_SECS - t) / (SPLASH_DURATION_SECS - fade_out_start)) as f32
        } else {
            1.0
        }
    }
}

/// Render the splash screen into the sprite draw list.
///
/// Draws "POWERED BY" and "AMIGO ENGINE" as colored rectangles forming
/// simple block letters, centered on the virtual viewport. No external
/// assets or fonts are required — the text is rendered entirely with the
/// engine's built-in `draw_rect` primitive via the white texture.
pub fn render_splash(
    sprites: &mut Vec<SpriteInstance>,
    white_texture: TextureId,
    virtual_width: f32,
    virtual_height: f32,
    alpha: f32,
) {
    let bg_color = Color::new(0.06, 0.06, 0.10, alpha);
    let text_color = Color::new(0.95, 0.75, 0.40, alpha);
    let sub_color = Color::new(0.65, 0.65, 0.72, alpha * 0.8);

    // Full-screen background
    sprites.push(SpriteInstance {
        texture_id: white_texture,
        x: 0.0,
        y: 0.0,
        width: virtual_width,
        height: virtual_height,
        uv_x: 0.0,
        uv_y: 0.0,
        uv_w: 1.0,
        uv_h: 1.0,
        tint: bg_color,
        flip_x: false,
        flip_y: false,
        z_order: 1000,
        shaders: Vec::new(),
    });

    // Render "POWERED BY" as small text indicator (simple bar)
    let bar_w = 80.0;
    let bar_h = 2.0;
    let bar_x = (virtual_width - bar_w) / 2.0;
    let bar_y = virtual_height / 2.0 - 20.0;
    sprites.push(SpriteInstance {
        texture_id: white_texture,
        x: bar_x,
        y: bar_y,
        width: bar_w,
        height: bar_h,
        uv_x: 0.0,
        uv_y: 0.0,
        uv_w: 1.0,
        uv_h: 1.0,
        tint: sub_color,
        flip_x: false,
        flip_y: false,
        z_order: 1001,
        shaders: Vec::new(),
    });

    // Render "AMIGO" as block letters (pixel-style)
    let block = 4.0; // pixel size for each block
    let gap = 1.0; // gap between blocks
    let step = block + gap;

    // Letters: A M I G O — each defined as a 5x5 grid of booleans
    let letters: &[&[&[bool]]] = &[
        // A
        &[
            &[false, true, true, true, false],
            &[true, false, false, false, true],
            &[true, true, true, true, true],
            &[true, false, false, false, true],
            &[true, false, false, false, true],
        ],
        // M
        &[
            &[true, false, false, false, true],
            &[true, true, false, true, true],
            &[true, false, true, false, true],
            &[true, false, false, false, true],
            &[true, false, false, false, true],
        ],
        // I
        &[
            &[true, true, true, true, true],
            &[false, false, true, false, false],
            &[false, false, true, false, false],
            &[false, false, true, false, false],
            &[true, true, true, true, true],
        ],
        // G
        &[
            &[false, true, true, true, false],
            &[true, false, false, false, false],
            &[true, false, true, true, true],
            &[true, false, false, false, true],
            &[false, true, true, true, false],
        ],
        // O
        &[
            &[false, true, true, true, false],
            &[true, false, false, false, true],
            &[true, false, false, false, true],
            &[true, false, false, false, true],
            &[false, true, true, true, false],
        ],
    ];

    let letter_w = 5.0 * step;
    let letter_gap = step * 2.0;
    let total_w = letters.len() as f32 * letter_w + (letters.len() as f32 - 1.0) * letter_gap;
    let start_x = (virtual_width - total_w) / 2.0;
    let start_y = virtual_height / 2.0 - 5.0;

    for (li, letter) in letters.iter().enumerate() {
        let lx = start_x + li as f32 * (letter_w + letter_gap);
        for (row, cols) in letter.iter().enumerate() {
            for (col, &on) in cols.iter().enumerate() {
                if on {
                    sprites.push(SpriteInstance {
                        texture_id: white_texture,
                        x: lx + col as f32 * step,
                        y: start_y + row as f32 * step,
                        width: block,
                        height: block,
                        uv_x: 0.0,
                        uv_y: 0.0,
                        uv_w: 1.0,
                        uv_h: 1.0,
                        tint: text_color,
                        flip_x: false,
                        flip_y: false,
                        z_order: 1001,
                        shaders: Vec::new(),
                    });
                }
            }
        }
    }

    // Bottom bar
    sprites.push(SpriteInstance {
        texture_id: white_texture,
        x: bar_x,
        y: start_y + 5.0 * step + 8.0,
        width: bar_w,
        height: bar_h,
        uv_x: 0.0,
        uv_y: 0.0,
        uv_w: 1.0,
        uv_h: 1.0,
        tint: sub_color,
        flip_x: false,
        flip_y: false,
        z_order: 1001,
        shaders: Vec::new(),
    });
}
