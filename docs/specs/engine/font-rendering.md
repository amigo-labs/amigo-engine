---
status: spec
crate: amigo_render
depends_on: ["engine/assets", "engine/ui"]
last_updated: 2026-03-18
---

# Font Rendering

## Purpose

Provide TTF/OTF text rendering for the Amigo Engine: glyph rasterisation into
texture atlases, multi-font management, text measurement, and -- once extended --
a full text layout engine with line wrapping, rich text markup, kerning, and
optional signed-distance-field (SDF) rendering for resolution-independent text.

## Existierende Bausteine

### FontAtlas (`crates/amigo_render/src/font.rs`, lines 37-225)

Rasterises glyphs via `fontdue` into a CPU-side RGBA atlas that is uploaded to
the GPU as a regular texture.

| Field / Method | Description |
|----------------|-------------|
| `font: Font` | Underlying fontdue `Font` handle |
| `id: FontId` | Opaque font identifier (`FontId(u32)`) |
| `px: f32` | Pixel size the font was loaded at |
| `atlas_data: Vec<u8>` | RGBA pixel data (512x512 initial, grows vertically) |
| `atlas_width / atlas_height` | Current atlas dimensions |
| `texture_id: Option<TextureId>` | GPU handle, set after upload |
| `glyphs: FxHashMap<char, GlyphInfo>` | Cached glyph metrics + UV coords |
| `cursor_x, cursor_y, row_height` | Shelf-packing cursor state |
| `dirty: bool` | True when atlas has been modified since last GPU upload |
| `new(font_data, px, id)` | Parse TTF, create 512x512 atlas, pre-cache ASCII `' '..='~'` |
| `cache_glyph(ch)` | Rasterise and blit one glyph into the atlas |
| `glyph(ch)` | Get glyph info, caching on demand |
| `glyph_cached(ch)` | Non-mutating lookup (None if not cached) |
| `measure(text) -> (f32, f32)` | Pixel width/height of a string at native size |
| `to_rgba_image()` | Convert atlas to `image::RgbaImage` for GPU upload |
| `grow_atlas()` | Double atlas height, rescale existing UVs |

### GlyphInfo (`crates/amigo_render/src/font.rs`, lines 16-30)

```rust
pub struct GlyphInfo {
    pub uv_x: f32, pub uv_y: f32,   // atlas UV (normalized)
    pub uv_w: f32, pub uv_h: f32,
    pub width: f32, pub height: f32, // pixel dimensions
    pub advance: f32,                // horizontal advance
    pub offset_x: f32,               // bearing X (from baseline)
    pub offset_y: f32,               // bearing Y
}
```

### FontManager (`crates/amigo_render/src/font.rs`, lines 228-313)

| Method | Description |
|--------|-------------|
| `new()` | Empty manager |
| `load_builtin(px) -> Result<FontId>` | Load embedded `amigo_pixel.ttf` (5x7 pixel font) |
| `load_font(data, px) -> Result<FontId>` | Load arbitrary TTF/OTF bytes |
| `get(id) / get_mut(id)` | Lookup by `FontId` |
| `default_font() / default_font_mut()` | First loaded font (convenience) |
| `dirty_fonts()` | Iterator over atlases needing re-upload |
| `clear_dirty()` | Mark all fonts clean after GPU upload |
| `len(), is_empty(), iter(), iter_mut()` | Standard collection methods |

### Built-in font

`BUILTIN_FONT: &[u8]` -- `amigo_pixel.ttf` (5x7 pixel font) embedded via
`include_bytes!`.

## Public API

### Existing (unchanged)

```rust
pub static BUILTIN_FONT: &[u8];
pub struct GlyphInfo { /* see above */ }
pub struct FontId(pub u32);
pub struct FontAtlas { /* see above */ }
pub struct FontManager { /* see above */ }
```

### Proposed: Text Layout Engine

```rust
/// Horizontal alignment.
#[derive(Clone, Copy, Debug, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Vertical alignment for a text block.
#[derive(Clone, Copy, Debug, Default)]
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
    pub max_width: Option<f32>,       // None = no wrapping
    pub line_height: Option<f32>,     // None = 1.2 * font px
    pub align: TextAlign,
    pub valign: TextVAlign,
    pub scale: f32,                   // multiplier on top of font px
}

/// A positioned glyph ready for rendering.
pub struct LayoutGlyph {
    pub ch: char,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub glyph_info: GlyphInfo,
    pub color: Color,                 // from rich text tags
    pub style: GlyphStyle,           // bold/italic flags
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GlyphStyle {
    pub bold: bool,
    pub italic: bool,
}

/// Result of text layout.
pub struct LayoutResult {
    pub glyphs: Vec<LayoutGlyph>,
    pub bounds: (f32, f32),          // total width, total height
    pub line_count: u32,
}

impl FontAtlas {
    /// Lay out text into positioned glyphs.
    pub fn layout(&mut self, params: &TextLayout) -> LayoutResult;
}
```

### Proposed: Rich Text Markup

Inline tags parsed from the text string:

```
"Normal [b]bold[/b] and [c=#ff0000]red text[/c]"
"[i]italic[/i] with [s=2.0]double size[/s]"
```

| Tag | Effect |
|-----|--------|
| `[b]...[/b]` | Bold (synthetic: +1px offset, drawn twice) |
| `[i]...[/i]` | Italic (synthetic: shear transform) |
| `[c=#RRGGBB]...[/c]` | Color change |
| `[s=N]...[/s]` | Scale factor |

```rust
/// Parse rich text markup into segments.
pub fn parse_rich_text(input: &str) -> Vec<RichTextSegment>;

pub struct RichTextSegment {
    pub text: String,
    pub color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub scale: f32,
}
```

### Proposed: Kerning Pairs

```rust
impl FontAtlas {
    /// Pre-compute kerning pairs for cached glyphs.
    /// Requires fontdue >= 0.8 kern table support.
    pub fn build_kerning_table(&mut self);

    /// Get the kerning offset between two characters (0.0 if unknown).
    pub fn kern(&self, left: char, right: char) -> f32;
}
```

### Proposed: SDF Rendering (optional)

```rust
/// Generate an SDF atlas instead of a rasterised one.
/// Allows smooth scaling without re-rasterisation.
pub struct SdfFontAtlas {
    pub spread: f32,              // SDF spread in pixels (default 4.0)
    pub atlas: FontAtlas,         // underlying atlas with SDF data
}

impl SdfFontAtlas {
    pub fn new(font_data: &[u8], px: f32, spread: f32, id: FontId) -> Result<Self, String>;
}
```

SDF rendering requires a dedicated WGSL fragment shader that reads the distance
field and applies `smoothstep` for crisp edges at any zoom level.

### Proposed: Asset Pipeline Integration

```rust
impl FontManager {
    /// Load a font from the asset pipeline by asset path.
    /// Resolves through the engine's AssetLoader.
    pub fn load_from_asset(&mut self, path: &str, px: f32,
                            loader: &AssetLoader) -> Result<FontId, String>;

    /// Load all `.ttf` / `.otf` files in an asset directory.
    pub fn load_directory(&mut self, dir: &str, px: f32,
                           loader: &AssetLoader) -> Vec<FontId>;
}
```

## Behavior

### Text Layout

1. Parse input text for rich text tags (if any). Produce a list of styled runs.
2. For each character, look up (or cache on demand) its `GlyphInfo`.
3. Advance the cursor by `glyph.advance + kern(prev, current)`.
4. If `max_width` is set and the cursor exceeds it, word-wrap: backtrack to the
   last whitespace boundary and move the remaining word to the next line.
5. After all glyphs are positioned, apply horizontal alignment per line:
   - Left: no adjustment.
   - Center: shift each line by `(max_width - line_width) / 2`.
   - Right: shift each line by `max_width - line_width`.
   - Justify: distribute extra space evenly between words (last line left-aligned).
6. Apply vertical alignment to the entire block.

### Synthetic Bold / Italic

Bold: render the glyph at (x, y) and again at (x+1, y). The layout engine
counts bold glyphs as 1px wider for correct cursor advance.

Italic: apply a horizontal shear of `0.2 * glyph_height` when emitting vertex
data.  No atlas modification needed.

### SDF Pipeline

SDF atlases are generated offline or at load time.  The SDF spread is stored
per-font.  At render time, a dedicated shader reads the atlas:

```wgsl
let dist = textureSample(sdf_atlas, sdf_sampler, uv).a;
let alpha = smoothstep(0.5 - smoothing, 0.5 + smoothing, dist);
```

where `smoothing` is derived from screen-space derivatives for automatic
anti-aliasing at any scale.

## Internal Design

- The layout engine is purely CPU-side.  It outputs `LayoutGlyph` structs that
  the sprite batcher consumes like any other textured quad.
- Rich text parsing uses a simple stack-based state machine (no regex dependency).
- Kerning data is stored in a `FxHashMap<(char, char), f32>` populated lazily
  from fontdue's kern table.  Only pairs involving cached glyphs are stored.
- SDF generation uses the dead-reckoning distance transform on the rasterised
  bitmap.  This adds ~2ms per glyph at 64px but is done once at load time.

## Non-Goals

- Complex text shaping (Arabic, Devanagari ligatures) -- use HarfBuzz if needed.
- Editable text fields (that is the UI system's responsibility).
- Dynamic font size without re-rasterisation (except via SDF path).
- Emoji rendering (color bitmap fonts).

## Open Questions

1. Should SDF be opt-in per font, or should all fonts generate SDF by default?
2. What is the maximum number of fonts a typical game will load (impacts atlas
   memory)?
3. Should rich text support nested tags (e.g. bold inside colored), or only flat?
4. Should the layout engine support bidirectional text (RTL)?

## Referenzen

- Existing implementation: `crates/amigo_render/src/font.rs` (314 lines)
- Built-in font: `crates/amigo_render/src/amigo_pixel.ttf`
- fontdue crate: github.com/mooman219/fontdue
- SDF text rendering: Valve, "Improved Alpha-Tested Magnification" (SIGGRAPH 2007)
- Green, "Improved techniques for distance-field text rendering" (2007)
