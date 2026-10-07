//! Mip chains for raster art, built on the CPU at load time.
//!
//! Mipmaps only go where neighbouring frames cannot bleed into each other: a
//! full chain over a sheet would average a red and a blue frame into a purple
//! level that both sample when drawn small.

use crate::SamplerMode;
use crate::blend::{linear_to_srgb_byte, srgb_to_linear};

/// What a texture holds, which decides how many mip levels it may have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MipSource {
    /// Exactly one frame: a PNG, a one-frame Aseprite file.
    SingleImage,
    /// Several frames packed together: an Aseprite strip, a sheet without
    /// `mip_levels`.
    MultiFrame,
    /// A sheet whose frames were checked for `levels` levels of gutter.
    PaddedSheet { levels: u32 },
    /// A font page; text is rasterised at its output size instead.
    FontPage,
}

/// Number of mip levels (the full image included) for a `width`×`height`
/// texture.
pub fn mip_level_count(source: MipSource, mode: SamplerMode, (width, height): (u32, u32)) -> u32 {
    if mode == SamplerMode::Nearest {
        return 1;
    }
    let full = 32 - width.max(height).max(1).leading_zeros();
    match source {
        MipSource::SingleImage => full,
        MipSource::PaddedSheet { levels } => (levels + 1).min(full),
        MipSource::MultiFrame | MipSource::FontPage => 1,
    }
}

/// The levels below `base` (premultiplied, sRGB-encoded RGBA8), each a 2×2 box
/// average of the one above in linear light, down to `levels` in total.
/// Returns `levels - 1` images; an odd edge repeats its last texel.
pub fn build_mip_chain(base: &image::RgbaImage, levels: u32) -> Vec<image::RgbaImage> {
    let mut chain: Vec<image::RgbaImage> = Vec::new();
    for _ in 1..levels {
        let prev = chain.last().unwrap_or(base);
        let (pw, ph) = prev.dimensions();
        if pw == 1 && ph == 1 {
            break;
        }
        let (w, h) = ((pw / 2).max(1), (ph / 2).max(1));
        let mut next = image::RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let mut sum = [0.0f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let sx = (2 * x + dx).min(pw - 1);
                    let sy = (2 * y + dy).min(ph - 1);
                    let p = prev.get_pixel(sx, sy).0;
                    for c in 0..3 {
                        sum[c] += srgb_to_linear(p[c] as f32 / 255.0);
                    }
                    sum[3] += p[3] as f32 / 255.0;
                }
                let px = [
                    linear_to_srgb_byte(sum[0] / 4.0),
                    linear_to_srgb_byte(sum[1] / 4.0),
                    linear_to_srgb_byte(sum[2] / 4.0),
                    ((sum[3] / 4.0) * 255.0).round() as u8,
                ];
                next.put_pixel(x, y, image::Rgba(px));
            }
        }
        chain.push(next);
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_linear_image_gets_a_full_chain() {
        assert_eq!(
            mip_level_count(MipSource::SingleImage, SamplerMode::Linear, (64, 32)),
            7
        );
        let chain = build_mip_chain(&image::RgbaImage::new(64, 32), 7);
        let sizes: Vec<(u32, u32)> = chain.iter().map(|i| i.dimensions()).collect();
        assert_eq!(
            sizes,
            vec![(32, 16), (16, 8), (8, 4), (4, 2), (2, 1), (1, 1)]
        );
    }

    #[test]
    fn nearest_textures_strips_sheets_and_font_pages_have_one_level() {
        assert_eq!(
            mip_level_count(MipSource::SingleImage, SamplerMode::Nearest, (64, 32)),
            1
        );
        for source in [MipSource::MultiFrame, MipSource::FontPage] {
            assert_eq!(mip_level_count(source, SamplerMode::Linear, (256, 256)), 1);
        }
    }

    #[test]
    fn a_padded_sheet_keeps_frames_apart_at_every_level() {
        // A red frame at (0, 0) and a blue one at (8, 0), 4x4 each, with a
        // 4 px transparent gutter: mip_levels 2.
        let mut sheet = image::RgbaImage::new(16, 4);
        for y in 0..4 {
            for x in 0..4 {
                sheet.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
                sheet.put_pixel(x + 8, y, image::Rgba([0, 0, 255, 255]));
            }
        }
        let levels = mip_level_count(
            MipSource::PaddedSheet { levels: 2 },
            SamplerMode::Linear,
            sheet.dimensions(),
        );
        assert_eq!(levels, 3);
        let chain = build_mip_chain(&sheet, levels);
        assert_eq!(chain.len(), 2);
        for (i, level) in chain.iter().enumerate() {
            let scale = 1 << (i + 1);
            for y in 0..(4 / scale).max(1) {
                for x in 0..(4 / scale).max(1) {
                    let p = level.get_pixel(x, y).0;
                    assert_eq!(p[2], 0, "level {}: blue inside the red frame", i + 1);
                    assert!(p[0] > 0);
                }
            }
        }
    }

    #[test]
    fn averaging_happens_in_linear_light() {
        let mut img = image::RgbaImage::new(2, 1);
        img.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        img.put_pixel(1, 0, image::Rgba([0, 0, 0, 255]));
        let chain = build_mip_chain(&img, 2);
        // Half white in linear light encodes to 188, not 128.
        assert_eq!(chain[0].get_pixel(0, 0).0, [188, 188, 188, 255]);
    }
}
