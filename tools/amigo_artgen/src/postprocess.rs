//! Pixel art post-processing pipeline.
//!
//! Transforms raw AI-generated images into clean pixel art by removing
//! anti-aliasing, clamping palettes, adding outlines, and downscaling.

use crate::PostProcessStep;

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
}
