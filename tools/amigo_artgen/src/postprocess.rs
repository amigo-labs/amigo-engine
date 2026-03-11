//! Pixel art post-processing pipeline.
//!
//! Transforms raw AI-generated images into clean pixel art by removing
//! anti-aliasing, clamping palettes, adding outlines, and downscaling.

use crate::PostProcessStep;
use crate::style::{OutlineMode, StyleDef};

/// RGBA pixel buffer for processing.
#[derive(Clone, Debug)]
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<[u8; 4]>,
}

impl PixelBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            data: vec![[0, 0, 0, 0]; size],
        }
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        self.data[(y * self.width + x) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, pixel: [u8; 4]) {
        self.data[(y * self.width + x) as usize] = pixel;
    }

    /// Apply a series of post-processing steps.
    pub fn apply_pipeline(&mut self, steps: &[PostProcessStep]) {
        for step in steps {
            match step {
                PostProcessStep::RemoveAntiAliasing => remove_aa(self),
                PostProcessStep::PaletteClamp { max_colors } => palette_clamp(self, *max_colors),
                PostProcessStep::AddOutline { color } => add_outline(self, *color),
                PostProcessStep::Downscale { factor } => downscale(self, *factor),
                PostProcessStep::ForceDimensions { width, height } => {
                    force_dimensions(self, *width, *height);
                }
                PostProcessStep::ApplyPalette { .. } => {
                    // Requires loading external palette file — skip in core pipeline
                }
            }
        }
    }
}

/// Remove anti-aliasing: snap semi-transparent edge pixels to fully
/// opaque or fully transparent based on an alpha threshold.
fn remove_aa(buf: &mut PixelBuffer) {
    let threshold = 128u8;
    for pixel in &mut buf.data {
        if pixel[3] > 0 && pixel[3] < 255 {
            if pixel[3] >= threshold {
                pixel[3] = 255;
            } else {
                *pixel = [0, 0, 0, 0];
            }
        }
    }
}

/// Reduce to at most `max_colors` using simple median-cut quantization.
///
/// This is a simplified implementation. A production version would use
/// a proper octree or median-cut algorithm. For pixel art with already
/// limited palettes, this works well enough.
fn palette_clamp(buf: &mut PixelBuffer, max_colors: u32) {
    if max_colors == 0 {
        return;
    }

    // Collect unique opaque colors
    let mut colors: Vec<[u8; 3]> = Vec::new();
    for pixel in &buf.data {
        if pixel[3] > 0 {
            let rgb = [pixel[0], pixel[1], pixel[2]];
            if !colors.contains(&rgb) {
                colors.push(rgb);
            }
        }
    }

    if colors.len() <= max_colors as usize {
        return; // Already within budget
    }

    // Simple approach: reduce bit depth to fit palette budget
    // Each reduction step halves the color space
    let mut shift = 0u8;
    let mut reduced = colors.len();
    while reduced > max_colors as usize && shift < 6 {
        shift += 1;
        let mask = !((1u8 << shift) - 1);
        let mut unique = std::collections::HashSet::new();
        for c in &colors {
            unique.insert([c[0] & mask, c[1] & mask, c[2] & mask]);
        }
        reduced = unique.len();
    }

    if shift > 0 {
        let mask = !((1u8 << shift) - 1);
        let half = 1u8 << (shift - 1); // round to center of bucket
        for pixel in &mut buf.data {
            if pixel[3] > 0 {
                pixel[0] = (pixel[0] & mask) | half;
                pixel[1] = (pixel[1] & mask) | half;
                pixel[2] = (pixel[2] & mask) | half;
            }
        }
    }
}

/// Add a 1px outline around non-transparent regions.
fn add_outline(buf: &mut PixelBuffer, color: [u8; 4]) {
    let w = buf.width;
    let h = buf.height;
    let original = buf.data.clone();

    let offsets: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            // Only consider transparent pixels
            if original[idx][3] > 0 {
                continue;
            }
            // Check if any neighbor is opaque
            let has_opaque_neighbor = offsets.iter().any(|(dx, dy)| {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                    let ni = (ny as u32 * w + nx as u32) as usize;
                    original[ni][3] > 0
                } else {
                    false
                }
            });
            if has_opaque_neighbor {
                buf.data[idx] = color;
            }
        }
    }
}

/// Downscale by integer factor using nearest-neighbor.
fn downscale(buf: &mut PixelBuffer, factor: u32) {
    if factor <= 1 {
        return;
    }
    let new_w = buf.width / factor;
    let new_h = buf.height / factor;
    if new_w == 0 || new_h == 0 {
        return;
    }

    let mut new_data = vec![[0u8; 4]; (new_w * new_h) as usize];
    for y in 0..new_h {
        for x in 0..new_w {
            let src_x = x * factor;
            let src_y = y * factor;
            new_data[(y * new_w + x) as usize] = buf.get(src_x, src_y);
        }
    }

    buf.width = new_w;
    buf.height = new_h;
    buf.data = new_data;
}

/// Force exact dimensions by cropping or padding.
fn force_dimensions(buf: &mut PixelBuffer, target_w: u32, target_h: u32) {
    let mut new_data = vec![[0u8; 4]; (target_w * target_h) as usize];

    let copy_w = buf.width.min(target_w);
    let copy_h = buf.height.min(target_h);
    // Center the content
    let offset_x = (target_w - copy_w) / 2;
    let offset_y = (target_h - copy_h) / 2;
    let src_offset_x = if buf.width > target_w { (buf.width - target_w) / 2 } else { 0 };
    let src_offset_y = if buf.height > target_h { (buf.height - target_h) / 2 } else { 0 };

    for y in 0..copy_h {
        for x in 0..copy_w {
            let src = buf.get(src_offset_x + x, src_offset_y + y);
            new_data[((offset_y + y) * target_w + (offset_x + x)) as usize] = src;
        }
    }

    buf.width = target_w;
    buf.height = target_h;
    buf.data = new_data;
}

/// Clamp each opaque pixel to the nearest color in the given palette
/// using Euclidean distance in RGB space.
pub fn palette_clamp_to_colors(buf: &mut PixelBuffer, palette: &[[u8; 3]]) {
    if palette.is_empty() {
        return;
    }
    for pixel in &mut buf.data {
        if pixel[3] == 0 {
            continue;
        }
        let mut best = palette[0];
        let mut best_dist = u32::MAX;
        for &color in palette {
            let dr = pixel[0] as i32 - color[0] as i32;
            let dg = pixel[1] as i32 - color[1] as i32;
            let db = pixel[2] as i32 - color[2] as i32;
            let dist = (dr * dr + dg * dg + db * db) as u32;
            if dist < best_dist {
                best_dist = dist;
                best = color;
            }
        }
        pixel[0] = best[0];
        pixel[1] = best[1];
        pixel[2] = best[2];
    }
}

/// Clean up transparency: alpha < 128 becomes fully transparent,
/// alpha >= 128 becomes fully opaque.
pub fn cleanup_transparency(buf: &mut PixelBuffer) {
    for pixel in &mut buf.data {
        if pixel[3] < 128 {
            *pixel = [0, 0, 0, 0];
        } else {
            pixel[3] = 255;
        }
    }
}

/// Add a 1px inner outline: opaque pixels adjacent to a transparent pixel
/// are replaced with the outline color.
pub fn add_outline_inner(buf: &mut PixelBuffer, color: [u8; 4]) {
    let w = buf.width;
    let h = buf.height;
    let original = buf.data.clone();

    let offsets: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            // Only consider opaque pixels
            if original[idx][3] == 0 {
                continue;
            }
            // Check if any neighbor is transparent (or out of bounds)
            let has_transparent_neighbor = offsets.iter().any(|(dx, dy)| {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                    return true; // edges count as transparent
                }
                let ni = (ny as u32 * w + nx as u32) as usize;
                original[ni][3] == 0
            });
            if has_transparent_neighbor {
                buf.data[idx] = color;
            }
        }
    }
}

/// Check if left/right and top/bottom edges are compatible for tiling.
/// Returns (horizontal_mismatches, vertical_mismatches) — the number of
/// pixels that differ between opposing edges.
pub fn tile_edge_check(buf: &PixelBuffer) -> (u32, u32) {
    let w = buf.width;
    let h = buf.height;

    let mut h_mismatches = 0u32;
    for y in 0..h {
        let left = buf.get(0, y);
        let right = buf.get(w - 1, y);
        if left != right {
            h_mismatches += 1;
        }
    }

    let mut v_mismatches = 0u32;
    for x in 0..w {
        let top = buf.get(x, 0);
        let bottom = buf.get(x, h - 1);
        if top != bottom {
            v_mismatches += 1;
        }
    }

    (h_mismatches, v_mismatches)
}

impl PixelBuffer {
    /// Apply post-processing based on a StyleDef's configuration.
    pub fn apply_style_pipeline(&mut self, style: &StyleDef) {
        let config = &style.post_processing;

        if config.cleanup_transparency {
            cleanup_transparency(self);
        }

        if config.remove_anti_aliasing {
            remove_aa(self);
        }

        if config.palette_clamp {
            let palette = style.palette_rgb();
            if !palette.is_empty() {
                palette_clamp_to_colors(self, &palette);
            }
        }

        if config.add_outline {
            let color = style.outline_rgba();
            match config.outline_mode {
                OutlineMode::Outer => add_outline(self, color),
                OutlineMode::Inner => add_outline_inner(self, color),
                OutlineMode::Both => {
                    add_outline_inner(self, color);
                    add_outline(self, color);
                }
            }
        }

        // tile_edge_check is informational only — we don't mutate,
        // but callers can invoke tile_edge_check() separately.
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_aa_snaps_alpha() {
        let mut buf = PixelBuffer::new(2, 1);
        buf.data[0] = [255, 0, 0, 200]; // above threshold → opaque
        buf.data[1] = [0, 255, 0, 50]; // below threshold → transparent

        remove_aa(&mut buf);

        assert_eq!(buf.data[0][3], 255);
        assert_eq!(buf.data[1][3], 0);
    }

    #[test]
    fn add_outline_around_pixel() {
        let mut buf = PixelBuffer::new(3, 3);
        buf.set(1, 1, [255, 0, 0, 255]); // center pixel opaque

        add_outline(&mut buf, [0, 0, 0, 255]);

        // Cardinal neighbors should be outline
        assert_eq!(buf.get(1, 0)[3], 255); // top
        assert_eq!(buf.get(0, 1)[3], 255); // left
        assert_eq!(buf.get(2, 1)[3], 255); // right
        assert_eq!(buf.get(1, 2)[3], 255); // bottom
        // Diagonal should still be transparent
        assert_eq!(buf.get(0, 0)[3], 0);
    }

    #[test]
    fn downscale_halves() {
        let mut buf = PixelBuffer::new(4, 4);
        buf.set(0, 0, [255, 0, 0, 255]);
        buf.set(2, 0, [0, 255, 0, 255]);

        downscale(&mut buf, 2);

        assert_eq!(buf.width, 2);
        assert_eq!(buf.height, 2);
        assert_eq!(buf.get(0, 0), [255, 0, 0, 255]);
        assert_eq!(buf.get(1, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn force_dimensions_pads() {
        let mut buf = PixelBuffer::new(2, 2);
        buf.set(0, 0, [255, 0, 0, 255]);

        force_dimensions(&mut buf, 4, 4);

        assert_eq!(buf.width, 4);
        assert_eq!(buf.height, 4);
        // Original content should be centered
        assert_eq!(buf.get(1, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn pipeline_applies_steps() {
        let mut buf = PixelBuffer::new(2, 1);
        buf.data[0] = [255, 0, 0, 100]; // semi-transparent
        buf.data[1] = [0, 255, 0, 200]; // semi-transparent

        buf.apply_pipeline(&[PostProcessStep::RemoveAntiAliasing]);

        assert_eq!(buf.data[0][3], 0); // was below threshold
        assert_eq!(buf.data[1][3], 255); // was above threshold
    }

    #[test]
    fn palette_clamp_under_budget_noop() {
        let mut buf = PixelBuffer::new(2, 1);
        buf.data[0] = [255, 0, 0, 255];
        buf.data[1] = [0, 255, 0, 255];
        let original = buf.data.clone();

        palette_clamp(&mut buf, 32);

        assert_eq!(buf.data, original); // Only 2 colors, budget is 32
    }

    #[test]
    fn palette_clamp_to_colors_nearest() {
        let mut buf = PixelBuffer::new(2, 1);
        buf.data[0] = [250, 10, 10, 255]; // close to red
        buf.data[1] = [10, 10, 240, 255]; // close to blue

        let palette = vec![[255, 0, 0], [0, 0, 255]];
        palette_clamp_to_colors(&mut buf, &palette);

        assert_eq!(buf.data[0], [255, 0, 0, 255]);
        assert_eq!(buf.data[1], [0, 0, 255, 255]);
    }

    #[test]
    fn palette_clamp_to_colors_skips_transparent() {
        let mut buf = PixelBuffer::new(2, 1);
        buf.data[0] = [250, 10, 10, 0]; // transparent — should be untouched
        buf.data[1] = [10, 10, 240, 255];

        let palette = vec![[255, 0, 0], [0, 0, 255]];
        palette_clamp_to_colors(&mut buf, &palette);

        assert_eq!(buf.data[0], [250, 10, 10, 0]); // unchanged
        assert_eq!(buf.data[1], [0, 0, 255, 255]);
    }

    #[test]
    fn palette_clamp_to_colors_empty_palette_noop() {
        let mut buf = PixelBuffer::new(1, 1);
        buf.data[0] = [128, 128, 128, 255];
        let original = buf.data.clone();

        palette_clamp_to_colors(&mut buf, &[]);

        assert_eq!(buf.data, original);
    }

    #[test]
    fn cleanup_transparency_snaps() {
        let mut buf = PixelBuffer::new(3, 1);
        buf.data[0] = [255, 0, 0, 50];  // below threshold → transparent
        buf.data[1] = [0, 255, 0, 200]; // above threshold → opaque
        buf.data[2] = [0, 0, 255, 128]; // exactly 128 → opaque

        cleanup_transparency(&mut buf);

        assert_eq!(buf.data[0], [0, 0, 0, 0]);
        assert_eq!(buf.data[1][3], 255);
        assert_eq!(buf.data[2][3], 255);
    }

    #[test]
    fn add_outline_inner_marks_edge_pixels() {
        let mut buf = PixelBuffer::new(3, 3);
        // Fill entire 3x3 with opaque red
        for y in 0..3 {
            for x in 0..3 {
                buf.set(x, y, [255, 0, 0, 255]);
            }
        }

        let outline = [0, 0, 0, 255];
        add_outline_inner(&mut buf, outline);

        // Center pixel (1,1) has all opaque neighbors → should stay red
        assert_eq!(buf.get(1, 1), [255, 0, 0, 255]);
        // Edge pixels are adjacent to out-of-bounds (treated as transparent)
        assert_eq!(buf.get(0, 0), outline); // corner
        assert_eq!(buf.get(1, 0), outline); // top edge
        assert_eq!(buf.get(0, 1), outline); // left edge
    }

    #[test]
    fn add_outline_inner_transparent_center() {
        let mut buf = PixelBuffer::new(3, 3);
        // Ring of opaque around a transparent center
        for y in 0..3 {
            for x in 0..3 {
                buf.set(x, y, [255, 0, 0, 255]);
            }
        }
        buf.set(1, 1, [0, 0, 0, 0]); // center transparent

        let outline = [0, 255, 0, 255];
        add_outline_inner(&mut buf, outline);

        // Pixels adjacent to the transparent center should become outline
        assert_eq!(buf.get(1, 0), outline); // above center
        assert_eq!(buf.get(0, 1), outline); // left of center
        assert_eq!(buf.get(2, 1), outline); // right of center
        assert_eq!(buf.get(1, 2), outline); // below center
    }

    #[test]
    fn tile_edge_check_uniform() {
        // All same color → 0 mismatches
        let mut buf = PixelBuffer::new(4, 4);
        for pixel in &mut buf.data {
            *pixel = [128, 128, 128, 255];
        }

        let (h, v) = tile_edge_check(&buf);
        assert_eq!(h, 0);
        assert_eq!(v, 0);
    }

    #[test]
    fn tile_edge_check_different_edges() {
        let mut buf = PixelBuffer::new(4, 4);
        // Left column = red, right column = blue
        for y in 0..4 {
            buf.set(0, y, [255, 0, 0, 255]);
            buf.set(3, y, [0, 0, 255, 255]);
        }
        // Top row = green, bottom row = yellow
        for x in 0..4 {
            buf.set(x, 0, [0, 255, 0, 255]);
            buf.set(x, 3, [255, 255, 0, 255]);
        }

        let (h, v) = tile_edge_check(&buf);
        assert!(h > 0, "expected horizontal mismatches");
        assert!(v > 0, "expected vertical mismatches");
    }

    #[test]
    fn apply_style_pipeline_runs() {
        use crate::style::StyleDef;

        let style = StyleDef::find("caribbean").unwrap();
        let mut buf = PixelBuffer::new(4, 4);
        // Fill with semi-transparent pixels of varied colors
        for (i, pixel) in buf.data.iter_mut().enumerate() {
            let v = (i * 17) as u8;
            *pixel = [v, 255 - v, v / 2, 200];
        }

        buf.apply_style_pipeline(&style);

        // After pipeline: all alpha should be 255 (cleanup + remove_aa)
        for pixel in &buf.data {
            assert!(
                pixel[3] == 0 || pixel[3] == 255,
                "expected fully transparent or opaque, got alpha={}",
                pixel[3]
            );
        }
    }
}
