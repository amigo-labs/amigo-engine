//! How a sprite's colour combines with what is already in the target, and the
//! premultiplied-alpha texture data every blend mode relies on.

use serde::{Deserialize, Serialize};

/// How a sprite's colour combines with what is already in the target.
///
/// Textures and tints are premultiplied by alpha before blending: texture data
/// on upload ([`premultiply_srgb`]), tints in the sprite shader. That is what
/// lets the three modes share one pass without fringes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    /// Source over: `src + dst * (1 - src.a)`.
    #[default]
    Normal,
    /// Light adds up: `src + dst`. For glow, sparks and fire.
    Additive,
    /// Darkens: `src * dst + dst * (1 - src.a)`. For shadows and tinting.
    Multiply,
}

impl BlendMode {
    /// Every mode, in the order the renderer indexes its pipelines.
    pub const ALL: [BlendMode; 3] = [BlendMode::Normal, BlendMode::Additive, BlendMode::Multiply];

    /// Index into [`BlendMode::ALL`].
    pub fn index(self) -> usize {
        match self {
            BlendMode::Normal => 0,
            BlendMode::Additive => 1,
            BlendMode::Multiply => 2,
        }
    }

    /// The wgpu blend state for premultiplied colour.
    pub fn blend_state(self) -> wgpu::BlendState {
        use wgpu::{BlendComponent, BlendFactor as F, BlendOperation as Op};
        let component = |src_factor, dst_factor| BlendComponent {
            src_factor,
            dst_factor,
            operation: Op::Add,
        };
        match self {
            BlendMode::Normal => wgpu::BlendState {
                color: component(F::One, F::OneMinusSrcAlpha),
                alpha: component(F::One, F::OneMinusSrcAlpha),
            },
            BlendMode::Additive => wgpu::BlendState {
                color: component(F::One, F::One),
                alpha: component(F::Zero, F::One),
            },
            BlendMode::Multiply => wgpu::BlendState {
                color: component(F::Dst, F::OneMinusSrcAlpha),
                alpha: component(F::Zero, F::One),
            },
        }
    }
}

/// sRGB-encoded byte to linear light, for every byte value.
fn srgb_decode_table() -> &'static [f32; 256] {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0.0; 256];
        for (i, v) in table.iter_mut().enumerate() {
            *v = srgb_to_linear(i as f32 / 255.0);
        }
        table
    })
}

/// sRGB transfer function, encoded to linear. Input and output in 0..=1.
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB transfer function, linear to encoded. Input and output in 0..=1.
pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Encode a linear value in 0..=1 as an sRGB byte.
pub fn linear_to_srgb_byte(c: f32) -> u8 {
    (linear_to_srgb(c) * 255.0).round() as u8
}

/// The stored value for coverage `a` in a coverage texture (font pages, the
/// white texture): `srgb_encode(a)`, so an sRGB texture samples it back as `a`.
pub fn coverage_byte(a: u8) -> u8 {
    linear_to_srgb_byte(a as f32 / 255.0)
}

/// Premultiply RGBA8 sRGB texels by alpha, in linear light: each texel's RGB is
/// decoded, multiplied by alpha and encoded again. Opaque texels are left
/// unchanged. Returns whether anything changed.
pub fn premultiply_srgb_bytes(data: &mut [u8]) -> bool {
    let table = srgb_decode_table();
    let mut changed = false;
    for texel in data.as_chunks_mut::<4>().0 {
        let a = texel[3];
        if a == 255 {
            continue;
        }
        changed = true;
        let alpha = a as f32 / 255.0;
        for c in &mut texel[..3] {
            *c = linear_to_srgb_byte(table[*c as usize] * alpha);
        }
    }
    changed
}

/// [`premultiply_srgb_bytes`] over an image.
pub fn premultiply_srgb(image: &mut image::RgbaImage) -> bool {
    premultiply_srgb_bytes(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_transparent_white_premultiplies_in_linear_light() {
        let mut img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 128]));
        assert!(premultiply_srgb(&mut img));
        assert_eq!(img.get_pixel(0, 0).0, [188, 188, 188, 128]);
    }

    #[test]
    fn opaque_texels_are_unchanged() {
        let mut img = image::RgbaImage::from_pixel(2, 1, image::Rgba([12, 200, 99, 255]));
        assert!(!premultiply_srgb(&mut img));
        assert_eq!(img.get_pixel(1, 0).0, [12, 200, 99, 255]);
    }

    #[test]
    fn transparent_texels_become_black() {
        let mut img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 10, 0]));
        premultiply_srgb(&mut img);
        assert_eq!(img.get_pixel(0, 0).0, [0, 0, 0, 0]);
    }

    #[test]
    fn coverage_128_decodes_to_one_half() {
        let e = coverage_byte(128);
        assert_eq!(e, 188);
        let decoded = srgb_to_linear(e as f32 / 255.0);
        assert!((decoded - 128.0 / 255.0).abs() < 0.005, "{decoded}");
    }

    #[test]
    fn blend_mode_round_trips_through_ron_names() {
        for mode in BlendMode::ALL {
            assert_eq!(BlendMode::ALL[mode.index()], mode);
        }
        assert_eq!(BlendMode::default(), BlendMode::Normal);
    }
}
