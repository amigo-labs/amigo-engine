//! TTF font rendering via fontdue.
//!
//! Rasterises glyphs into a CPU-side texture atlas, which is then uploaded to
//! the GPU as a regular texture. The atlas grows on demand when new glyphs are
//! encountered.

use crate::texture::TextureId;
use fontdue::{Font, FontSettings};
use rustc_hash::FxHashMap;

/// Built-in 5×7 pixel font (AmigoPixel), embedded as TTF bytes.
pub static BUILTIN_FONT: &[u8] = include_bytes!("amigo_pixel.ttf");

/// A single cached glyph in the atlas.
#[derive(Clone, Copy, Debug)]
pub struct GlyphInfo {
    /// UV coordinates in the atlas (normalized 0..1).
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    /// Pixel metrics.
    pub width: f32,
    pub height: f32,
    /// Horizontal advance for cursor positioning.
    pub advance: f32,
    /// Offset from the baseline.
    pub offset_x: f32,
    pub offset_y: f32,
}

/// Identifies a loaded font by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId(pub u32);

/// A cached font with its glyph atlas.
pub struct FontAtlas {
    pub font: Font,
    pub id: FontId,
    pub px: f32,
    /// The RGBA pixel data for the atlas texture.
    pub atlas_data: Vec<u8>,
    pub atlas_width: u32,
    pub atlas_height: u32,
    /// GPU texture ID (set after upload).
    pub texture_id: Option<TextureId>,
    /// Cached glyphs: char → GlyphInfo.
    glyphs: FxHashMap<char, GlyphInfo>,
    /// Current packing cursor in the atlas.
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    /// Whether the atlas has been modified since last upload.
    pub dirty: bool,
}

impl FontAtlas {
    /// Create a new font atlas from TTF/OTF bytes at a given pixel size.
    pub fn new(font_data: &[u8], px: f32, id: FontId) -> Result<Self, String> {
        let font = Font::from_bytes(font_data, FontSettings::default())
            .map_err(|e| format!("Failed to parse font: {}", e))?;

        // Start with a 512×512 atlas (expandable)
        let atlas_width = 512u32;
        let atlas_height = 512u32;
        let atlas_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];

        let mut atlas = Self {
            font,
            id,
            px,
            atlas_data,
            atlas_width,
            atlas_height,
            texture_id: None,
            glyphs: FxHashMap::default(),
            cursor_x: 1, // 1px padding from edge
            cursor_y: 1,
            row_height: 0,
            dirty: true,
        };

        // Pre-cache ASCII printable range
        for ch in ' '..='~' {
            atlas.cache_glyph(ch);
        }

        Ok(atlas)
    }

    /// Cache a single glyph if not already present.
    pub fn cache_glyph(&mut self, ch: char) -> Option<GlyphInfo> {
        if let Some(&info) = self.glyphs.get(&ch) {
            return Some(info);
        }

        let (metrics, bitmap) = self.font.rasterize(ch, self.px);
        if metrics.width == 0 || metrics.height == 0 {
            // Space or zero-width character — store with zero UV
            let info = GlyphInfo {
                uv_x: 0.0,
                uv_y: 0.0,
                uv_w: 0.0,
                uv_h: 0.0,
                width: 0.0,
                height: 0.0,
                advance: metrics.advance_width,
                offset_x: metrics.xmin as f32,
                offset_y: metrics.ymin as f32,
            };
            self.glyphs.insert(ch, info);
            return Some(info);
        }

        let gw = metrics.width as u32;
        let gh = metrics.height as u32;
        let padding = 1u32;

        // Check if we need to wrap to the next row
        if self.cursor_x + gw + padding > self.atlas_width {
            self.cursor_x = 1;
            self.cursor_y += self.row_height + padding;
            self.row_height = 0;
        }

        // Check if we need to grow the atlas vertically
        if self.cursor_y + gh + padding > self.atlas_height {
            self.grow_atlas();
        }

        // Blit the glyph bitmap (alpha channel only) into the RGBA atlas
        let ax = self.cursor_x;
        let ay = self.cursor_y;
        for row in 0..gh {
            for col in 0..gw {
                let src = (row * gw + col) as usize;
                let alpha = bitmap[src];
                let dst = ((ay + row) * self.atlas_width + (ax + col)) as usize * 4;
                self.atlas_data[dst] = 255;     // R
                self.atlas_data[dst + 1] = 255; // G
                self.atlas_data[dst + 2] = 255; // B
                self.atlas_data[dst + 3] = alpha; // A
            }
        }

        let info = GlyphInfo {
            uv_x: ax as f32 / self.atlas_width as f32,
            uv_y: ay as f32 / self.atlas_height as f32,
            uv_w: gw as f32 / self.atlas_width as f32,
            uv_h: gh as f32 / self.atlas_height as f32,
            width: gw as f32,
            height: gh as f32,
            advance: metrics.advance_width,
            offset_x: metrics.xmin as f32,
            offset_y: metrics.ymin as f32,
        };

        self.glyphs.insert(ch, info);
        self.cursor_x += gw + padding;
        self.row_height = self.row_height.max(gh);
        self.dirty = true;

        Some(info)
    }

    /// Get cached glyph info (caches on demand).
    pub fn glyph(&mut self, ch: char) -> Option<GlyphInfo> {
        if self.glyphs.contains_key(&ch) {
            return self.glyphs.get(&ch).copied();
        }
        self.cache_glyph(ch)
    }

    /// Get glyph info without mutating (returns None if not cached).
    pub fn glyph_cached(&self, ch: char) -> Option<GlyphInfo> {
        self.glyphs.get(&ch).copied()
    }

    /// Measure the pixel width and height of a string at the font's native size.
    pub fn measure(&self, text: &str) -> (f32, f32) {
        let mut width = 0.0f32;
        let mut max_h = 0.0f32;
        for ch in text.chars() {
            if let Some(info) = self.glyphs.get(&ch) {
                width += info.advance;
                let h = info.height - info.offset_y;
                if h > max_h {
                    max_h = h;
                }
            }
        }
        (width, max_h.max(self.px))
    }

    /// Convert atlas data to an `image::RgbaImage` for GPU upload.
    pub fn to_rgba_image(&self) -> image::RgbaImage {
        image::RgbaImage::from_raw(self.atlas_width, self.atlas_height, self.atlas_data.clone())
            .expect("atlas dimensions match data length")
    }

    /// Double the atlas height when it runs out of space.
    fn grow_atlas(&mut self) {
        let new_height = self.atlas_height * 2;
        let mut new_data = vec![0u8; (self.atlas_width * new_height * 4) as usize];
        let old_row_bytes = (self.atlas_width * 4) as usize;

        for row in 0..self.atlas_height {
            let src_start = row as usize * old_row_bytes;
            let dst_start = row as usize * old_row_bytes;
            new_data[dst_start..dst_start + old_row_bytes]
                .copy_from_slice(&self.atlas_data[src_start..src_start + old_row_bytes]);
        }

        self.atlas_data = new_data;
        self.atlas_height = new_height;

        // All existing UV coords need rescaling
        for info in self.glyphs.values_mut() {
            info.uv_y *= 0.5;
            info.uv_h *= 0.5;
        }

        self.dirty = true;
    }
}

/// Manages multiple loaded fonts.
pub struct FontManager {
    fonts: Vec<FontAtlas>,
    next_id: u32,
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            fonts: Vec::new(),
            next_id: 0,
        }
    }

    /// Load the built-in AmigoPixel font at a given pixel size.
    /// Returns a `FontId` handle for later use.
    pub fn load_builtin(&mut self, px: f32) -> Result<FontId, String> {
        self.load_font(BUILTIN_FONT, px)
    }

    /// Load a font from TTF/OTF bytes at a given pixel size.
    /// Returns a `FontId` handle for later use.
    pub fn load_font(&mut self, data: &[u8], px: f32) -> Result<FontId, String> {
        let id = FontId(self.next_id);
        self.next_id += 1;
        let atlas = FontAtlas::new(data, px, id)?;
        self.fonts.push(atlas);
        Ok(id)
    }

    /// Get a font atlas by ID.
    pub fn get(&self, id: FontId) -> Option<&FontAtlas> {
        self.fonts.iter().find(|f| f.id == id)
    }

    /// Get a mutable font atlas by ID.
    pub fn get_mut(&mut self, id: FontId) -> Option<&mut FontAtlas> {
        self.fonts.iter_mut().find(|f| f.id == id)
    }

    /// Get the first loaded font (convenience for single-font games).
    pub fn default_font(&self) -> Option<&FontAtlas> {
        self.fonts.first()
    }

    /// Get the first loaded font mutably.
    pub fn default_font_mut(&mut self) -> Option<&mut FontAtlas> {
        self.fonts.first_mut()
    }

    /// Iterate over all fonts that need their atlas re-uploaded.
    pub fn dirty_fonts(&self) -> impl Iterator<Item = &FontAtlas> {
        self.fonts.iter().filter(|f| f.dirty)
    }

    /// Mark all fonts as clean (call after re-uploading).
    pub fn clear_dirty(&mut self) {
        for f in &mut self.fonts {
            f.dirty = false;
        }
    }

    /// Number of loaded fonts.
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Iterate over all font atlases.
    pub fn iter(&self) -> impl Iterator<Item = &FontAtlas> {
        self.fonts.iter()
    }

    /// Iterate over all font atlases mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut FontAtlas> {
        self.fonts.iter_mut()
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}
