//! TTF font rendering via fontdue.
//!
//! Glyphs are rasterised on first use into fixed-size atlas pages, which the
//! engine uploads as regular textures. A page never moves or grows, so once a
//! glyph has a page and a UV rectangle neither changes. Glyphs can be cached
//! through `&self` (from `Game::draw`), which is why the atlas state sits
//! behind a mutex.

use crate::blend::{BlendMode, coverage_byte};
use crate::texture::{TextureId, TextureIdAllocator};
use amigo_core::{Color, Rect, RenderVec2};
use fontdue::{Font, FontSettings};
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// Built-in 5×7 pixel font (AmigoPixel), embedded as TTF bytes.
pub static BUILTIN_FONT: &[u8] = include_bytes!("amigo_pixel.ttf");

/// Side length of a font atlas page, in pixels.
pub const FONT_PAGE_SIZE: u32 = 1024;

/// Pixels between glyphs on a page.
const GLYPH_PADDING: u32 = 1;

/// A single cached glyph in the atlas.
#[derive(Clone, Copy, Debug)]
pub struct GlyphInfo {
    /// UV coordinates on its page (normalized 0..1).
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
    /// The texture of the page the glyph lives on. Fixed once assigned.
    pub texture_id: TextureId,
}

/// Identifies a loaded font by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId(pub u32);

/// Errors from [`FontManager`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FontError {
    #[error("no font with id {0:?} is loaded")]
    UnknownFont(FontId),
}

/// One fixed-size page of a font atlas.
pub struct FontPage {
    /// Assigned when the page is created and never changed.
    pub texture_id: TextureId,
    /// RGBA, `FONT_PAGE_SIZE`², premultiplied: coverage `a` is stored as
    /// `(e, e, e, a)` with `e = srgb_encode(a)`, so it samples as `(a, a, a, a)`.
    pub data: Vec<u8>,
    /// Glyphs were added since the last upload.
    pub dirty: bool,
    /// Rows `start..end` changed since the last upload.
    dirty_rows: Option<(u32, u32)>,
    /// The page was uploaded at least once.
    uploaded: bool,
}

impl FontPage {
    fn new(texture_id: TextureId) -> Self {
        Self {
            texture_id,
            data: vec![0; (FONT_PAGE_SIZE * FONT_PAGE_SIZE * 4) as usize],
            dirty: true,
            dirty_rows: None,
            uploaded: false,
        }
    }

    /// Rows changed since the last upload, or `None` when the whole page has
    /// to be uploaded (it never was) or nothing changed.
    pub fn dirty_rows(&self) -> Option<(u32, u32)> {
        if self.uploaded { self.dirty_rows } else { None }
    }

    /// Whether the GPU has the page at all.
    pub fn is_uploaded(&self) -> bool {
        self.uploaded
    }

    /// Record that the page's current data is on the GPU.
    pub fn mark_uploaded(&mut self) {
        self.dirty = false;
        self.dirty_rows = None;
        self.uploaded = true;
    }

    fn mark_rows(&mut self, start: u32, end: u32) {
        self.dirty = true;
        self.dirty_rows = Some(match self.dirty_rows {
            Some((s, e)) => (s.min(start), e.max(end)),
            None => (start, end),
        });
    }
}

struct AtlasState {
    glyphs: FxHashMap<char, Option<GlyphInfo>>,
    pages: Vec<FontPage>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

/// The pages of a [`FontAtlas`], locked for reading or updating.
pub struct FontPages<'a>(MutexGuard<'a, AtlasState>);

impl std::ops::Deref for FontPages<'_> {
    type Target = [FontPage];
    fn deref(&self) -> &[FontPage] {
        &self.0.pages
    }
}

impl std::ops::DerefMut for FontPages<'_> {
    fn deref_mut(&mut self) -> &mut [FontPage] {
        &mut self.0.pages
    }
}

/// A font rasterised at one pixel size, with its glyph pages.
pub struct FontAtlas {
    pub font: Arc<Font>,
    pub id: FontId,
    /// The size glyphs are rasterised at.
    pub px: f32,
    state: Mutex<AtlasState>,
    ids: TextureIdAllocator,
}

impl FontAtlas {
    /// Create an atlas from TTF/OTF bytes at a given pixel size, taking page
    /// texture ids from an allocator of its own. Inside the engine, atlases
    /// come from a [`FontManager`] that shares the renderer's allocator.
    pub fn new(font_data: &[u8], px: f32, id: FontId) -> Result<Self, String> {
        let font = Font::from_bytes(font_data, FontSettings::default())
            .map_err(|e| format!("Failed to parse font: {}", e))?;
        Ok(Self::with_font(
            Arc::new(font),
            px,
            id,
            TextureIdAllocator::default(),
        ))
    }

    /// An atlas of `font` at `px`, with page ids from `ids`. ASCII is cached
    /// up front.
    pub fn with_font(font: Arc<Font>, px: f32, id: FontId, ids: TextureIdAllocator) -> Self {
        let atlas = Self {
            font,
            id,
            px,
            state: Mutex::new(AtlasState {
                glyphs: FxHashMap::default(),
                pages: Vec::new(),
                cursor_x: GLYPH_PADDING,
                cursor_y: GLYPH_PADDING,
                row_height: 0,
            }),
            ids,
        };
        for ch in ' '..='~' {
            atlas.cache_glyph(ch);
        }
        atlas
    }

    fn lock(&self) -> MutexGuard<'_, AtlasState> {
        // A panic while holding the lock leaves the atlas usable: at worst
        // one glyph is missing.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The atlas pages. Holds the atlas lock while alive.
    pub fn pages(&self) -> FontPages<'_> {
        FontPages(self.lock())
    }

    /// Whether any page has data the GPU does not.
    pub fn needs_upload(&self) -> bool {
        self.lock().pages.iter().any(|p| p.dirty || !p.uploaded)
    }

    /// Give every page a new id from `ids` and mark it for upload. Cached
    /// glyphs follow. For the engine's startup, before anything is drawn.
    pub fn set_texture_ids(&mut self, ids: TextureIdAllocator) {
        let state = self.state.get_mut().unwrap_or_else(|e| e.into_inner());
        let mut remap = FxHashMap::default();
        for page in &mut state.pages {
            let new = ids.allocate();
            remap.insert(page.texture_id, new);
            page.texture_id = new;
            page.dirty = true;
            page.uploaded = false;
        }
        for glyph in state.glyphs.values_mut().flatten() {
            if let Some(new) = remap.get(&glyph.texture_id) {
                glyph.texture_id = *new;
            }
        }
        self.ids = ids;
    }

    /// Cache a single glyph if not already present. A character the font has
    /// no glyph for caches the font's `.notdef` glyph (index 0). Control
    /// characters have no glyph.
    pub fn cache_glyph(&self, ch: char) -> Option<GlyphInfo> {
        let mut state = self.lock();
        if let Some(info) = state.glyphs.get(&ch) {
            return *info;
        }
        let info = self.rasterize(&mut state, ch);
        state.glyphs.insert(ch, info);
        info
    }

    /// Get glyph info, caching it on demand.
    pub fn glyph(&self, ch: char) -> Option<GlyphInfo> {
        self.cache_glyph(ch)
    }

    /// Get glyph info without caching (`None` if not cached yet).
    pub fn glyph_cached(&self, ch: char) -> Option<GlyphInfo> {
        self.lock().glyphs.get(&ch).copied().flatten()
    }

    fn rasterize(&self, state: &mut AtlasState, ch: char) -> Option<GlyphInfo> {
        if ch.is_control() {
            return None;
        }
        // Index 0 is `.notdef`: a missing character draws it rather than
        // vanishing.
        let index = self.font.lookup_glyph_index(ch);
        let (metrics, bitmap) = self.font.rasterize_indexed(index, self.px);
        let empty = |texture_id| GlyphInfo {
            uv_x: 0.0,
            uv_y: 0.0,
            uv_w: 0.0,
            uv_h: 0.0,
            width: 0.0,
            height: 0.0,
            advance: metrics.advance_width,
            offset_x: metrics.xmin as f32,
            offset_y: metrics.ymin as f32,
            texture_id,
        };
        if metrics.width == 0 || metrics.height == 0 {
            return Some(empty(TextureId(0)));
        }

        let (gw, gh) = (metrics.width as u32, metrics.height as u32);
        if gw + 2 * GLYPH_PADDING > FONT_PAGE_SIZE || gh + 2 * GLYPH_PADDING > FONT_PAGE_SIZE {
            tracing::warn!(
                "Glyph '{}' ({}x{}) does not fit a {}px font page; drawn as empty",
                ch.escape_debug(),
                gw,
                gh,
                FONT_PAGE_SIZE
            );
            return Some(empty(TextureId(0)));
        }

        // Next row, then a new page, when the glyph does not fit.
        if state.cursor_x + gw + GLYPH_PADDING > FONT_PAGE_SIZE {
            state.cursor_x = GLYPH_PADDING;
            state.cursor_y += state.row_height + GLYPH_PADDING;
            state.row_height = 0;
        }
        if state.pages.is_empty() || state.cursor_y + gh + GLYPH_PADDING > FONT_PAGE_SIZE {
            state.pages.push(FontPage::new(self.ids.allocate()));
            state.cursor_x = GLYPH_PADDING;
            state.cursor_y = GLYPH_PADDING;
            state.row_height = 0;
        }

        let (ax, ay) = (state.cursor_x, state.cursor_y);
        let page = state.pages.last_mut()?;
        for row in 0..gh {
            for col in 0..gw {
                let alpha = bitmap[(row * gw + col) as usize];
                let dst = (((ay + row) * FONT_PAGE_SIZE + ax + col) * 4) as usize;
                let e = coverage_byte(alpha);
                page.data[dst..dst + 4].copy_from_slice(&[e, e, e, alpha]);
            }
        }
        page.mark_rows(ay, ay + gh);
        let texture_id = page.texture_id;

        state.cursor_x += gw + GLYPH_PADDING;
        state.row_height = state.row_height.max(gh);

        let size = FONT_PAGE_SIZE as f32;
        Some(GlyphInfo {
            uv_x: ax as f32 / size,
            uv_y: ay as f32 / size,
            uv_w: gw as f32 / size,
            uv_h: gh as f32 / size,
            width: gw as f32,
            height: gh as f32,
            advance: metrics.advance_width,
            offset_x: metrics.xmin as f32,
            offset_y: metrics.ymin as f32,
            texture_id,
        })
    }

    /// Measure the pixel width and height of a string at the font's native
    /// size, as the legacy `draw_text` lays it out (no kerning).
    pub fn measure(&self, text: &str) -> (f32, f32) {
        let mut width = 0.0f32;
        let mut max_h = 0.0f32;
        for ch in text.chars() {
            if let Some(info) = self.glyph(ch) {
                width += info.advance;
                let h = info.height - info.offset_y;
                if h > max_h {
                    max_h = h;
                }
            }
        }
        (width, max_h.max(self.px))
    }
}

/// How a line of text drawn with `draw_text_ex` looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// `None` uses the default font (see [`FontManager::set_default_font`]).
    pub font: Option<FontId>,
    /// Size in virtual pixels. `None` uses the size the font was loaded at.
    pub size_px: Option<f32>,
    pub color: Color,
    /// Extra advance after every glyph, in virtual pixels. May be negative.
    pub letter_spacing: f32,
    /// Horizontal anchor: `pos.x` is the left edge, the centre or the right
    /// edge of the line.
    pub align: TextAlign,
    pub z_order: i32,
    pub blend: BlendMode,
}

impl Default for TextStyle {
    /// Default font at its load size, white, no spacing, left-aligned, z 100
    /// (as `draw_text`), [`BlendMode::Normal`].
    fn default() -> Self {
        Self {
            font: None,
            size_px: None,
            color: Color::WHITE,
            letter_spacing: 0.0,
            align: TextAlign::Left,
            z_order: 100,
            blend: BlendMode::Normal,
        }
    }
}

/// Size of one line of text, in virtual pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    /// Distance from the top of the line box to the baseline.
    pub ascent: f32,
    /// Distance from the baseline to the bottom of the line box (positive).
    pub descent: f32,
    /// Recommended distance between baselines.
    pub line_height: f32,
}

/// One glyph quad of a laid-out line, in virtual pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphQuad {
    pub texture_id: TextureId,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// UV rectangle `(x, y, w, h)` on the glyph's page.
    pub uv: [f32; 4],
}

/// A laid-out line: its quads and its bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    pub quads: Vec<GlyphQuad>,
    pub bounds: Rect,
    pub metrics: TextMetrics,
}

/// Manages loaded fonts and the atlases they are rasterised into.
pub struct FontManager {
    /// One atlas per font at its load size.
    fonts: Vec<Arc<FontAtlas>>,
    /// Atlases at other sizes, made on demand by `draw_text_ex`.
    sized: Mutex<Vec<Arc<FontAtlas>>>,
    next_id: u32,
    default: Option<FontId>,
    ids: TextureIdAllocator,
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            fonts: Vec::new(),
            sized: Mutex::new(Vec::new()),
            next_id: 0,
            default: None,
            ids: TextureIdAllocator::default(),
        }
    }

    /// Take page texture ids from `ids` from now on (the renderer's, see
    /// `Renderer::texture_ids`). Pages that exist already get new ids from it
    /// and are uploaded again.
    pub fn set_texture_ids(&mut self, ids: TextureIdAllocator) {
        for atlas in &mut self.fonts {
            if let Some(atlas) = Arc::get_mut(atlas) {
                atlas.set_texture_ids(ids.clone());
            }
        }
        let sized = self.sized.get_mut().unwrap_or_else(|e| e.into_inner());
        for atlas in sized.iter_mut() {
            if let Some(atlas) = Arc::get_mut(atlas) {
                atlas.set_texture_ids(ids.clone());
            }
        }
        self.ids = ids;
    }

    /// The allocator page texture ids come from.
    pub fn texture_ids(&self) -> TextureIdAllocator {
        self.ids.clone()
    }

    /// Load the built-in AmigoPixel font at a given pixel size.
    /// Returns a `FontId` handle for later use.
    pub fn load_builtin(&mut self, px: f32) -> Result<FontId, String> {
        self.load_font(BUILTIN_FONT, px)
    }

    /// Load a font from TTF/OTF bytes at a given pixel size.
    /// Returns a `FontId` handle for later use.
    pub fn load_font(&mut self, data: &[u8], px: f32) -> Result<FontId, String> {
        let font = Font::from_bytes(data, FontSettings::default())
            .map_err(|e| format!("Failed to parse font: {}", e))?;
        let id = FontId(self.next_id);
        self.next_id += 1;
        let atlas = FontAtlas::with_font(Arc::new(font), px, id, self.ids.clone());
        self.fonts.push(Arc::new(atlas));
        Ok(id)
    }

    /// Get a font atlas (at its load size) by ID.
    pub fn get(&self, id: FontId) -> Option<&FontAtlas> {
        self.fonts.iter().find(|f| f.id == id).map(|f| &**f)
    }

    /// Get a mutable font atlas by ID. `None` while an atlas is shared by a
    /// draw in progress.
    pub fn get_mut(&mut self, id: FontId) -> Option<&mut FontAtlas> {
        self.fonts
            .iter_mut()
            .find(|f| f.id == id)
            .and_then(Arc::get_mut)
    }

    /// Make `id` the font used by `draw_text`, `draw_text_scaled`,
    /// `measure_text`, `TextStyle { font: None, .. }` and every `amigo_ui`
    /// widget.
    pub fn set_default_font(&mut self, id: FontId) -> Result<(), FontError> {
        if self.get(id).is_none() {
            return Err(FontError::UnknownFont(id));
        }
        self.default = Some(id);
        Ok(())
    }

    /// The font `default_font()` returns: the one set with
    /// [`set_default_font`](Self::set_default_font), else the first loaded.
    pub fn default_font_id(&self) -> Option<FontId> {
        self.default.or_else(|| self.fonts.first().map(|f| f.id))
    }

    /// The default font at its load size.
    pub fn default_font(&self) -> Option<&FontAtlas> {
        self.default_font_id().and_then(|id| self.get(id))
    }

    /// The default font, mutably.
    pub fn default_font_mut(&mut self) -> Option<&mut FontAtlas> {
        let id = self.default_font_id()?;
        self.get_mut(id)
    }

    /// The atlas of font `id` rasterised at `px` whole pixels, created on
    /// first use.
    pub fn atlas_at(&self, id: FontId, px: u32) -> Option<Arc<FontAtlas>> {
        let base = self.fonts.iter().find(|f| f.id == id)?;
        if base.px == px as f32 {
            return Some(base.clone());
        }
        let mut sized = self.sized.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(atlas) = sized.iter().find(|a| a.id == id && a.px == px as f32) {
            return Some(atlas.clone());
        }
        let atlas = Arc::new(FontAtlas::with_font(
            base.font.clone(),
            px as f32,
            id,
            self.ids.clone(),
        ));
        sized.push(atlas.clone());
        Some(atlas)
    }

    /// Every atlas, at every size, for uploading pages.
    pub fn all_atlases(&self) -> Vec<Arc<FontAtlas>> {
        let mut all = self.fonts.clone();
        all.extend(
            self.sized
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .cloned(),
        );
        all
    }

    /// Measure one line exactly as [`layout_line`](Self::layout_line) lays it
    /// out. Uses font metrics only; no glyph is rasterised.
    pub fn measure_line(&self, text: &str, style: &TextStyle) -> TextMetrics {
        let Some((font, size)) = self.style_font(style) else {
            return TextMetrics::default();
        };
        let mut metrics = line_metrics(&font.font, size);
        metrics.width = advances(&font.font, text, size, style.letter_spacing)
            .last()
            .map_or(0.0, |&(_, _, end)| end);
        metrics
    }

    /// Lay out one line of `text` with `style`, anchored at `pos` (`pos.y` is
    /// the top of the line box). Glyphs are rasterised at
    /// `round(size_px * render_scale)` pixels and drawn at `size_px`.
    pub fn layout_line(
        &self,
        text: &str,
        pos: RenderVec2,
        style: &TextStyle,
        render_scale: f32,
    ) -> TextLine {
        let Some((font, size)) = self.style_font(style) else {
            return TextLine {
                quads: Vec::new(),
                bounds: Rect::new(pos.x, pos.y, 0.0, 0.0),
                metrics: TextMetrics::default(),
            };
        };
        let mut metrics = line_metrics(&font.font, size);
        let pen = advances(&font.font, text, size, style.letter_spacing);
        metrics.width = pen.last().map_or(0.0, |&(_, _, end)| end);
        let x0 = match style.align {
            TextAlign::Left => pos.x,
            TextAlign::Center => pos.x - metrics.width / 2.0,
            TextAlign::Right => pos.x - metrics.width,
        };
        let baseline = pos.y + metrics.ascent;

        let scale = if render_scale.is_finite() && render_scale > 0.0 {
            render_scale
        } else {
            1.0
        };
        let raster_px = (size * scale).round().max(1.0) as u32;
        let k = size / raster_px as f32;
        let mut quads = Vec::with_capacity(pen.len());
        if let Some(atlas) = self.atlas_at(font.id, raster_px) {
            for &(ch, x, _) in &pen {
                let Some(glyph) = atlas.glyph(ch) else {
                    continue;
                };
                if glyph.width <= 0.0 || glyph.height <= 0.0 {
                    continue;
                }
                quads.push(GlyphQuad {
                    texture_id: glyph.texture_id,
                    x: x0 + x + glyph.offset_x * k,
                    y: baseline - (glyph.height + glyph.offset_y) * k,
                    width: glyph.width * k,
                    height: glyph.height * k,
                    uv: [glyph.uv_x, glyph.uv_y, glyph.uv_w, glyph.uv_h],
                });
            }
        }
        TextLine {
            quads,
            bounds: Rect::new(x0, pos.y, metrics.width, metrics.ascent + metrics.descent),
            metrics,
        }
    }

    /// The load-size atlas and the size in virtual pixels a style draws with.
    fn style_font(&self, style: &TextStyle) -> Option<(&FontAtlas, f32)> {
        let id = style.font.or_else(|| self.default_font_id())?;
        let font = self.get(id)?;
        let size = style
            .size_px
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(font.px);
        Some((font, size))
    }

    /// Iterate over all fonts that need their atlas uploaded.
    pub fn dirty_fonts(&self) -> impl Iterator<Item = &FontAtlas> {
        self.fonts.iter().filter(|f| f.needs_upload()).map(|f| &**f)
    }

    /// Number of loaded fonts.
    pub fn len(&self) -> usize {
        self.fonts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fonts.is_empty()
    }

    /// Iterate over all font atlases (at their load size).
    pub fn iter(&self) -> impl Iterator<Item = &FontAtlas> {
        self.fonts.iter().map(|f| &**f)
    }

    /// Iterate over all font atlases (at their load size) mutably. Atlases
    /// shared by a draw in progress are skipped.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut FontAtlas> {
        self.fonts.iter_mut().filter_map(Arc::get_mut)
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Ascent, descent and line height of `font` at `size`.
fn line_metrics(font: &Font, size: f32) -> TextMetrics {
    match font.horizontal_line_metrics(size) {
        Some(m) => TextMetrics {
            width: 0.0,
            ascent: m.ascent,
            descent: -m.descent,
            line_height: m.new_line_size,
        },
        None => TextMetrics {
            width: 0.0,
            ascent: size,
            descent: 0.0,
            line_height: size,
        },
    }
}

/// The pen position of every drawn character: `(char, start, end)` with kerning
/// before and `spacing` after each glyph. Control characters (`\n` included)
/// have no glyph and no advance. A missing character advances like `.notdef`.
fn advances(font: &Font, text: &str, size: f32, spacing: f32) -> Vec<(char, f32, f32)> {
    let spacing = if spacing.is_finite() { spacing } else { 0.0 };
    let mut out = Vec::with_capacity(text.len());
    let mut pen = 0.0f32;
    let mut prev: Option<u16> = None;
    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }
        let index = font.lookup_glyph_index(ch);
        if let Some(prev) = prev {
            pen += font
                .horizontal_kern_indexed(prev, index, size)
                .unwrap_or(0.0);
        }
        let start = pen;
        pen += font.metrics_indexed(index, size).advance_width + spacing;
        out.push((ch, start, pen));
        prev = Some(index);
    }
    out
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
    pub fn layout(&self, params: &TextLayout) -> LayoutResult {
        let segments = parse_rich_text(&params.text);
        let line_h = params.line_height.unwrap_or(self.px * 1.2) * params.scale;
        let scale = params.scale;

        let mut glyphs: Vec<LayoutGlyph> = Vec::new();
        let mut cursor_x = 0.0_f32;
        let mut cursor_y = 0.0_f32;
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
                if let Some(max_w) = params.max_width
                    && cursor_x + advance + bold_extra > max_w
                    && cursor_x > 0.0
                {
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
                    } else {
                        line_widths.push(cursor_x);
                        cursor_x = 0.0;
                        cursor_y += line_h;
                    }
                    if cursor_x > max_width_seen {
                        max_width_seen = cursor_x;
                    }
                    last_space_idx = None;
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
        if let Some(max_w) = params.max_width.filter(|_| params.align != TextAlign::Left) {
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
/// Supports: `[b]...[/b]`, `[i]...[/i]`, `[c=#RRGGBB]...[/c]`, `[s=N]...[/s]`
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

    fn manager_with(font: &[u8], px: f32) -> (FontManager, FontId) {
        let mut fonts = FontManager::new();
        let id = fonts.load_font(font, px).expect("test font parses");
        (fonts, id)
    }

    fn hack(px: f32) -> (FontManager, FontId) {
        manager_with(epaint_default_fonts::HACK_REGULAR, px)
    }

    #[test]
    fn text_style_defaults() {
        let style = TextStyle::default();
        assert_eq!(style.font, None);
        assert_eq!(style.size_px, None);
        assert_eq!(style.color, Color::WHITE);
        assert_eq!(style.letter_spacing, 0.0);
        assert_eq!(style.align, TextAlign::Left);
        assert_eq!(style.z_order, 100);
        assert_eq!(style.blend, BlendMode::Normal);
    }

    #[test]
    fn the_default_font_can_be_chosen() {
        let (mut fonts, first) = hack(12.0);
        let second = fonts
            .load_font(epaint_default_fonts::UBUNTU_LIGHT, 12.0)
            .expect("ubuntu parses");
        assert_eq!(fonts.default_font_id(), Some(first));
        assert_eq!(fonts.set_default_font(second), Ok(()));
        assert_eq!(fonts.default_font_id(), Some(second));
        assert_eq!(fonts.default_font().map(|f| f.id), Some(second));
        assert_eq!(
            fonts.set_default_font(FontId(99)),
            Err(FontError::UnknownFont(FontId(99)))
        );
        assert_eq!(fonts.default_font_id(), Some(second));
    }

    #[test]
    fn umlauts_are_rasterised_on_first_use() {
        let (fonts, id) = hack(12.0);
        let atlas = fonts.get(id).expect("loaded");
        assert!(
            atlas.glyph_cached('ü').is_none(),
            "only ASCII is pre-cached"
        );
        let glyph = atlas.glyph('ü').expect("ü has a glyph");
        assert!(glyph.width > 0.0);
        assert!(atlas.glyph_cached('ü').is_some());
    }

    #[test]
    fn a_missing_character_draws_notdef_and_advances() {
        let (fonts, id) = hack(16.0);
        let atlas = fonts.get(id).expect("loaded");
        let missing = '\u{E123}';
        assert_eq!(atlas.font.lookup_glyph_index(missing), 0);
        let notdef = atlas.glyph(missing).expect(".notdef stands in");
        assert!(notdef.advance > 0.0);
        assert!(notdef.width > 0.0 && notdef.height > 0.0, "{notdef:?}");

        let style = TextStyle::default();
        let line = fonts.layout_line("a\u{E123}b", RenderVec2::ZERO, &style, 1.0);
        assert_eq!(line.quads.len(), 3);
        assert!(line.quads[2].x > line.quads[1].x);
    }

    #[test]
    fn control_characters_have_no_glyph_and_no_advance() {
        let (fonts, _) = hack(16.0);
        let style = TextStyle::default();
        let plain = fonts.measure_line("ab", &style);
        let with_newline = fonts.measure_line("a\nb", &style);
        assert_eq!(plain.width, with_newline.width);
    }

    #[test]
    fn measured_width_matches_the_drawn_bounds_for_every_alignment() {
        let (fonts, _) = hack(14.0);
        for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
            let style = TextStyle {
                align,
                letter_spacing: 0.75,
                ..Default::default()
            };
            let text = "AVATAR Wägen";
            let measured = fonts.measure_line(text, &style);
            let line = fonts.layout_line(text, RenderVec2::new(100.0, 10.0), &style, 1.0);
            assert_eq!(measured.width, line.bounds.w, "{align:?}");
            let expected_x = match align {
                TextAlign::Left => 100.0,
                TextAlign::Center => 100.0 - measured.width / 2.0,
                TextAlign::Right => 100.0 - measured.width,
            };
            assert_eq!(line.bounds.x, expected_x);
            assert_eq!(line.bounds.h, measured.ascent + measured.descent);
        }
    }

    #[test]
    fn letter_spacing_adds_to_every_character() {
        let (fonts, _) = hack(14.0);
        let base = fonts.measure_line("hello", &TextStyle::default());
        let spaced = fonts.measure_line(
            "hello",
            &TextStyle {
                letter_spacing: 2.0,
                ..Default::default()
            },
        );
        assert!((spaced.width - base.width - 5.0 * 2.0).abs() < 1e-4);
    }

    #[test]
    fn render_scale_rasterises_at_the_output_size() {
        let (fonts, id) = hack(12.0);
        let style = TextStyle {
            size_px: Some(10.0),
            ..Default::default()
        };
        let line = fonts.layout_line("H", RenderVec2::ZERO, &style, 2.0);
        let atlas = fonts.atlas_at(id, 20).expect("a 20 px atlas exists");
        let glyph = atlas.glyph_cached('H').expect("H was rasterised at 20 px");
        let quad = line.quads[0];
        assert_eq!(quad.texture_id, glyph.texture_id);
        assert!((quad.height - glyph.height / 2.0).abs() < 1e-4);
        assert!(quad.height < 10.5, "drawn at 10 px, not 20");
    }

    #[test]
    fn page_texels_are_premultiplied_coverage() {
        let (fonts, id) = hack(24.0);
        let atlas = fonts.get(id).expect("loaded");
        let pages = atlas.pages();
        let mut partial = 0;
        for texel in pages[0].data.chunks_exact(4) {
            let e = coverage_byte(texel[3]);
            assert_eq!(&texel[..3], &[e, e, e]);
            if texel[3] > 0 && texel[3] < 255 {
                partial += 1;
            }
        }
        assert!(partial > 0, "anti-aliased edges exist");
    }

    #[test]
    fn a_full_page_opens_a_new_one_and_keeps_old_glyphs_in_place() {
        let (fonts, id) = hack(12.0);
        let atlas = fonts.atlas_at(id, 160).expect("atlas");
        let first = atlas.glyph('A').expect("A");
        let first_page = atlas.pages()[0].texture_id;
        assert_eq!(first.texture_id, first_page);
        for ch in ('a'..='z').chain('0'..='9').chain('À'..='ÿ') {
            atlas.glyph(ch);
        }
        let pages: Vec<TextureId> = atlas.pages().iter().map(|p| p.texture_id).collect();
        assert!(pages.len() >= 2, "{} pages", pages.len());
        let again = atlas.glyph_cached('A').expect("still cached");
        assert_eq!(again.texture_id, first.texture_id);
        assert_eq!(
            (again.uv_x, again.uv_y, again.uv_w, again.uv_h),
            (first.uv_x, first.uv_y, first.uv_w, first.uv_h)
        );
        let last = atlas.glyph_cached('ÿ').expect("ÿ");
        assert_eq!(last.texture_id, *pages.last().expect("pages"));
    }

    #[test]
    fn set_texture_ids_renumbers_pages_and_glyphs() {
        let (mut fonts, id) = hack(12.0);
        let ids = TextureIdAllocator::new(500);
        fonts.set_texture_ids(ids);
        let atlas = fonts.get(id).expect("loaded");
        let page = atlas.pages()[0].texture_id;
        assert!(page.0 >= 500);
        assert_eq!(atlas.glyph_cached('A').map(|g| g.texture_id), Some(page));
        assert!(atlas.needs_upload());
    }

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
