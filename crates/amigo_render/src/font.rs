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
                self.atlas_data[dst] = 255; // R
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

// ---------------------------------------------------------------------------
// Text Layout Engine
// ---------------------------------------------------------------------------

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical text alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextVAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Configuration for laying out a block of text.
pub struct TextLayout {
    pub font_id: FontId,
    pub text: String,
    pub max_width: Option<f32>,
    pub line_height: Option<f32>,
    pub align: TextAlign,
    pub valign: TextVAlign,
    pub scale: f32,
}

impl Default for TextLayout {
    fn default() -> Self {
        Self {
            font_id: FontId(0),
            text: String::new(),
            max_width: None,
            line_height: None,
            align: TextAlign::Left,
            valign: TextVAlign::Top,
            scale: 1.0,
        }
    }
}

/// A positioned glyph ready for rendering.
#[derive(Clone, Debug)]
pub struct LayoutGlyph {
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub glyph_info: GlyphInfo,
    pub color: amigo_core::color::Color,
    pub style: GlyphStyle,
}

/// Style flags for a glyph.
#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphStyle {
    pub bold: bool,
    pub italic: bool,
}

/// Result of text layout.
pub struct LayoutResult {
    pub glyphs: Vec<LayoutGlyph>,
    pub bounds: (f32, f32),
    pub line_count: u32,
}

impl FontAtlas {
    /// Lay out text into positioned glyphs with word wrapping and alignment.
    pub fn layout(&mut self, params: &TextLayout) -> LayoutResult {
        let segments = parse_rich_text(&params.text);
        let line_h = params.line_height.unwrap_or(self.px * 1.2) * params.scale;
        let scale = params.scale;

        let mut glyphs: Vec<LayoutGlyph> = Vec::new();
        let mut cursor_x = 0.0_f32;
        let mut cursor_y = 0.0_f32;
        let mut line_start_idx = 0usize;
        let mut last_space_idx: Option<usize> = None;
        let mut line_widths: Vec<f32> = Vec::new();
        let mut max_width_seen = 0.0_f32;

        for seg in &segments {
            let color = seg.color.unwrap_or(amigo_core::color::Color::WHITE);
            let style = GlyphStyle {
                bold: seg.bold,
                italic: seg.italic,
            };
            let seg_scale = scale * seg.scale;

            for ch in seg.text.chars() {
                if ch == '\n' {
                    line_widths.push(cursor_x);
                    if cursor_x > max_width_seen {
                        max_width_seen = cursor_x;
                    }
                    cursor_x = 0.0;
                    cursor_y += line_h;
                    line_start_idx = glyphs.len();
                    last_space_idx = None;
                    continue;
                }

                let gi = match self.glyph(ch) {
                    Some(g) => g,
                    None => continue,
                };
                let advance = gi.advance * seg_scale;
                let bold_extra = if seg.bold { 1.0 * seg_scale } else { 0.0 };

                // Word wrap
                if let Some(max_w) = params.max_width {
                    if cursor_x + advance + bold_extra > max_w && cursor_x > 0.0 {
                        // Wrap at last space or at current position
                        if let Some(space_idx) = last_space_idx {
                            // Move glyphs after last space to next line
                            line_widths.push(
                                glyphs
                                    .get(space_idx)
                                    .map(|g| g.x + g.glyph_info.advance * g.scale)
                                    .unwrap_or(cursor_x),
                            );
                            let wrap_x = glyphs.get(space_idx + 1).map(|g| g.x).unwrap_or(cursor_x);
                            cursor_y += line_h;
                            let shift = wrap_x;
                            for g in &mut glyphs[space_idx + 1..] {
                                g.x -= shift;
                                g.y = cursor_y;
                            }
                            cursor_x -= shift;
                            line_start_idx = space_idx + 1;
                        } else {
                            line_widths.push(cursor_x);
                            cursor_x = 0.0;
                            cursor_y += line_h;
                            line_start_idx = glyphs.len();
                        }
                        if cursor_x > max_width_seen {
                            max_width_seen = cursor_x;
                        }
                        last_space_idx = None;
                    }
                }

                if ch == ' ' {
                    last_space_idx = Some(glyphs.len());
                }

                glyphs.push(LayoutGlyph {
                    ch,
                    x: cursor_x,
                    y: cursor_y,
                    scale: seg_scale,
                    glyph_info: gi,
                    color,
                    style,
                });

                cursor_x += advance + bold_extra;
            }
        }

        // Final line
        line_widths.push(cursor_x);
        if cursor_x > max_width_seen {
            max_width_seen = cursor_x;
        }

        let total_height = cursor_y + line_h;
        let line_count = line_widths.len() as u32;

        // Apply horizontal alignment
        if params.align != TextAlign::Left && params.max_width.is_some() {
            let max_w = params.max_width.unwrap();
            let mut line_idx = 0usize;
            let mut current_line = 0usize;
            for g in &mut glyphs {
                // Detect line change by y position
                let glyph_line = (g.y / line_h).round() as usize;
                if glyph_line != current_line {
                    current_line = glyph_line;
                }
                let lw = line_widths.get(current_line).copied().unwrap_or(0.0);
                let shift = match params.align {
                    TextAlign::Center => (max_w - lw) * 0.5,
                    TextAlign::Right => max_w - lw,
                    _ => 0.0,
                };
                g.x += shift;
            }
            let _ = line_idx;
        }

        let bounds_w = params.max_width.unwrap_or(max_width_seen);

        LayoutResult {
            glyphs,
            bounds: (bounds_w, total_height),
            line_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Rich Text Parsing
// ---------------------------------------------------------------------------

/// A segment of styled text from rich text parsing.
#[derive(Clone, Debug)]
pub struct RichTextSegment {
    pub text: String,
    pub color: Option<amigo_core::color::Color>,
    pub bold: bool,
    pub italic: bool,
    pub scale: f32,
}

/// Parse rich text markup into styled segments.
/// Supports: [b]...[/b], [i]...[/i], [c=#RRGGBB]...[/c], [s=N]...[/s]
pub fn parse_rich_text(input: &str) -> Vec<RichTextSegment> {
    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut color: Option<amigo_core::color::Color> = None;
    let mut scale = 1.0_f32;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            // Try to parse a tag
            let mut tag = String::new();
            let mut found_close = false;
            for tc in chars.by_ref() {
                if tc == ']' {
                    found_close = true;
                    break;
                }
                tag.push(tc);
            }
            if !found_close {
                current_text.push('[');
                current_text.push_str(&tag);
                continue;
            }

            // Flush current text
            if !current_text.is_empty() {
                segments.push(RichTextSegment {
                    text: std::mem::take(&mut current_text),
                    color,
                    bold,
                    italic,
                    scale,
                });
            }

            // Process tag
            match tag.as_str() {
                "b" => bold = true,
                "/b" => bold = false,
                "i" => italic = true,
                "/i" => italic = false,
                "/c" => color = None,
                "/s" => scale = 1.0,
                _ if tag.starts_with("c=#") || tag.starts_with("c=") => {
                    let hex_str = tag.trim_start_matches("c=#").trim_start_matches("c=");
                    if let Ok(hex) = u32::from_str_radix(hex_str, 16) {
                        color = Some(amigo_core::color::Color::from_hex(hex));
                    }
                }
                _ if tag.starts_with("s=") => {
                    if let Ok(s) = tag[2..].parse::<f32>() {
                        scale = s;
                    }
                }
                _ => {
                    // Unknown tag, treat as literal text
                    current_text.push('[');
                    current_text.push_str(&tag);
                    current_text.push(']');
                }
            }
        } else {
            current_text.push(ch);
        }
    }

    // Flush remaining text
    if !current_text.is_empty() {
        segments.push(RichTextSegment {
            text: current_text,
            color,
            bold,
            italic,
            scale,
        });
    }

    // If no segments, return one empty segment
    if segments.is_empty() {
        segments.push(RichTextSegment {
            text: String::new(),
            color: None,
            bold: false,
            italic: false,
            scale: 1.0,
        });
    }

    segments
}

// ---------------------------------------------------------------------------
// Kerning
// ---------------------------------------------------------------------------

impl FontAtlas {
    /// Get kerning offset between two characters.
    /// Returns 0.0 if kerning data is not available or the pair is unknown.
    pub fn kern(&self, left: char, right: char) -> f32 {
        // fontdue's Font::horizontal_kern() provides kerning if available
        self.font
            .horizontal_kern(left, right, self.px)
            .unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text() {
        let segs = parse_rich_text("Hello world");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "Hello world");
        assert!(!segs[0].bold);
        assert!(!segs[0].italic);
    }

    #[test]
    fn parse_bold() {
        let segs = parse_rich_text("Normal [b]bold[/b] text");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].text, "Normal ");
        assert!(!segs[0].bold);
        assert_eq!(segs[1].text, "bold");
        assert!(segs[1].bold);
        assert_eq!(segs[2].text, " text");
        assert!(!segs[2].bold);
    }

    #[test]
    fn parse_color() {
        let segs = parse_rich_text("[c=#ff0000]red[/c]");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "red");
        assert!(segs[0].color.is_some());
        let c = segs[0].color.unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.0).abs() < 0.01);
    }

    #[test]
    fn parse_scale() {
        let segs = parse_rich_text("[s=2.0]big[/s]");
        assert_eq!(segs.len(), 1);
        assert!((segs[0].scale - 2.0).abs() < 0.01);
    }

    #[test]
    fn parse_nested() {
        let segs = parse_rich_text("[b][i]bold italic[/i][/b]");
        assert_eq!(segs.len(), 1);
        assert!(segs[0].bold);
        assert!(segs[0].italic);
    }

    #[test]
    fn parse_unclosed_tag_is_literal() {
        let segs = parse_rich_text("text [unclosed");
        assert_eq!(segs.len(), 1);
        assert!(segs[0].text.contains("[unclosed"));
    }
}
