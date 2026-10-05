---
status: spec
crate: amigo_render, amigo_engine, amigo_assets
depends_on: ["engine/rendering", "engine/camera", "engine/font-rendering", "engine/particles", "assets/atlas"]
last_updated: 2026-10-05
---

# Rendering Extensions

## Purpose

The renderer grew up around pixel-art tile games. Its sprites are axis-aligned
quads anchored at their top-left corner. It blends one way, draws only to world
space from `Game::draw`, renders only ASCII text, has rectangles as its only
shape, and offers a closed set of post-processing effects. A game with
high-resolution raster art (`art_style = "raster_art"`), or a game ported from
another engine, runs into each of these limits on its first screen.

This spec closes those gaps. Each section is a self-contained extension of
`amigo_render` and of the `DrawContext` facade in `amigo_engine`, and can land in
its own pull request in the priority order below.

Nothing here is specific to one game. The reference case that exposed the gaps is
[HamsterFlight](https://github.com/daniel-rck/HamsterFlight), a browser port of the
Flash game *Flight of the Hamsters* (TypeScript, pixi.js). The table traces each of
its needs to the section that covers it. Paths in the second column are in that
repository.

| Need | Where in HamsterFlight | Engine today | Section |
|---|---|---|---|
| The hamster rotates every tick about its body | `src/sim/entities/Projectile.ts`, `src/render/interpolate.ts` | no rotation or pivot (`amigo_render/src/sprite_batcher.rs:46-62`, quads built axis-aligned at `:151-170`) | R1 |
| Art is placed by an offset from the entity position (Flash `ox`/`oy`) | `src/assets/sprites.generated.ts` | position is always the top-left | R1, R8 |
| Additive glow particles | `src/render/PixiRenderer.ts:688`, `src/render/effects/Effects.ts` | one pipeline, `BlendState::ALPHA_BLENDING` (`renderer.rs:319`); `particles::BlendMode::Additive` ignored (backlog render-18) | R2 |
| HUD bar, minimap and prompt cards in screen space | `src/render/scene/hud.ts` | every `DrawContext` call lands in the world list (`amigo_engine/src/context.rs:248-300`) | R3 |
| Camera interpolated between simulation ticks; own shake curve | `src/render/interpolate.ts`, `src/render/effects/Effects.ts` | the camera can only be set in `update`; shake decay is a private field fixed at 8.0 (`camera.rs:85, 117`) | R4 |
| Fredoka typeface, German umlauts, letter spacing | `src/render/scene/hud.ts`, `src/app/i18n.ts` | only ASCII is cached (`font.rs:84`, backlog render-11); `draw_text` always uses AmigoPixel | R5 |
| Rounded cards, cloud shapes, a tapered speed trail, sky gradients | `src/render/scene/decor.ts`, `src/render/scene/trail.ts` | `draw_rect` only | R6 |
| One shader for shockwave, glow and chromatic aberration; motion blur | `src/render/effects/SceneFilter.ts`, `src/render/pixi/SceneFilters.ts` | closed `PostEffect` enum, fixed order (`post_process.rs:264-292`, backlog render-25) | R7 |
| 39 sprites, 535 frames on one sheet, per-sprite offsets, 19 fps clips | `src/assets/sprites.generated.ts`, `src/render/PoseClock.ts` | one texture per file; `SpriteDescriptor.origin` is never read; the pak path crops sprites back out (`amigo_assets/src/asset_manager.rs:314-336`) | R8 |
| Stars in three depth layers, clouds and hills at fractional camera speed | `src/render/scene/decor.ts` | `TileLayer.scroll_factor_*` exist but nothing reads them (`amigo_tilemap/src/lib.rs:64-65`) | R9 |
| Vector-rasterised art drawn smaller than authored; the stage follows the window's shape | `src/render/resolution.ts` | no mipmaps (`texture.rs:59`); no way to query the window or viewport; only letterboxed or stretched scaling | R10 |

### Priorities

| Priority | Sections | Meaning |
|---|---|---|
| P1 | R1–R5 | A game of this kind cannot be built without them. |
| P2 | R6–R8 | Workarounds exist (hand-built UVs, rectangles only, no custom effects) but cost real effort or quality. |
| P3 | R9–R10 | Quality and convenience. |

## Public API

Every new type is re-exported from the root of the crate it lives in. The ones
marked **prelude** are also added to `amigo_engine::prelude`.

### R1: Sprite transform

```rust
// amigo_render::sprite_batcher (prelude)

/// A single sprite to be rendered.
#[derive(Clone, Debug)]
pub struct SpriteInstance {
    pub texture_id: TextureId,
    /// World (or screen, see R3) position of the pivot.
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    pub tint: Color,
    pub flip_x: bool,
    pub flip_y: bool,
    pub z_order: i32,
    /// Optional per-sprite shader effects (applied in order). Still not
    /// rendered; see Non-Goals.
    pub shaders: Vec<SpriteShader>,
    // --- new ---
    /// Pivot, measured from the unrotated quad's top-left corner, in the same
    /// units as `width`/`height`. `[0.0, 0.0]` (the default) makes `x`/`y` the
    /// top-left corner, exactly as before this field existed.
    pub origin: [f32; 2],
    /// Rotation about `origin` in radians. Positive turns clockwise on screen
    /// (y points down), like Flash `_rotation` and pixi `rotation`.
    pub rotation: f32,
    /// How the sprite is composited (R2).
    pub blend: BlendMode,
    /// Explicit corner geometry (R6). When `Some`, the batcher uses these
    /// corners and colours and ignores `x`, `y`, `width`, `height`, `origin`,
    /// `rotation`, `flip_x` and `flip_y`.
    pub geometry: Option<QuadGeometry>,
}

/// Four corners in draw order top-left, top-right, bottom-right, bottom-left,
/// each with its own colour (multiplied by `tint`). A triangle repeats its
/// third corner as the fourth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuadGeometry {
    pub corners: [[f32; 2]; 4],
    pub colors: [Color; 4],
}

impl SpriteInstance {
    /// A white-tinted, unrotated, unflipped sprite covering the whole texture,
    /// pivot at the top-left, z 0, `BlendMode::Normal`, no geometry.
    pub fn new(texture_id: TextureId, x: f32, y: f32, width: f32, height: f32) -> Self;

    /// Scale `width`, `height` and `origin` together, so the pivot stays on the
    /// same point of the art. A negative factor mirrors the sprite about its
    /// pivot: the size stays positive, `flip_x`/`flip_y` toggles, and
    /// `origin` becomes `size - origin` on that axis.
    pub fn scale(&mut self, sx: f32, sy: f32);

    /// Set `origin` as a fraction of the current size: `(0.5, 0.5)` is the
    /// centre, `(0.5, 1.0)` the bottom centre.
    pub fn set_origin_normalized(&mut self, nx: f32, ny: f32);

    /// World corners after origin and rotation (or `geometry` when set),
    /// in the order top-left, top-right, bottom-right, bottom-left.
    pub fn corners(&self) -> [[f32; 2]; 4];

    /// Axis-aligned bounding box of [`corners`](Self::corners).
    pub fn bounds(&self) -> Rect;
}
```

### R2: Blend modes

```rust
// amigo_render::blend (prelude); `amigo_render::particles::BlendMode` becomes
// a re-export of this type.

/// How a sprite's colour combines with what is already in the target.
/// Textures and tints are premultiplied by alpha before blending (see Behavior).
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
```

The variant name `Normal` keeps existing particle configs in RON valid.

### R3: Screen-space drawing from `draw()`

```rust
// amigo_engine (prelude)

/// Which coordinate space `DrawContext` draws into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawSpace {
    /// World coordinates, through the camera, lighting and post-processing.
    #[default]
    World,
    /// Virtual-resolution screen coordinates, origin top-left, no camera,
    /// drawn in the UI pass after post-processing (conventions A.6).
    Screen,
}

impl<'a> DrawContext<'a> {
    /// Attach the list that screen-space draws go into. The engine always
    /// attaches one; a `DrawContext` built without it (in a test, say) drops
    /// screen-space draws and logs one warning.
    pub fn with_screen_list(self, screen: &'a mut Vec<SpriteInstance>) -> Self;

    /// Route every following draw call into `space`.
    pub fn set_space(&mut self, space: DrawSpace);

    /// The current draw space.
    pub fn space(&self) -> DrawSpace;

    /// Run `f` with `space` active, then restore the previous space.
    pub fn in_space<R>(&mut self, space: DrawSpace, f: impl FnOnce(&mut Self) -> R) -> R;

    /// Default z-order for the following draws that do not take one
    /// explicitly (`draw_sprite`, `draw_animated`, `draw_frame`, `draw_rect`
    /// and every shape in R6). Starts at 0.
    pub fn set_z(&mut self, z_order: i32);
}
```

### R4: Camera from `draw()`

```rust
// amigo_engine
impl DrawContext<'_> {
    /// Render this frame with the camera centred on `center` instead of the
    /// position the camera reached in `update`. Affects only this frame's
    /// projection; `GameContext::camera` is not changed.
    pub fn set_camera_position(&mut self, center: RenderVec2);

    /// Add `offset` to this frame's camera position, e.g. a game's own shake.
    /// Added after `set_camera_position` and after the camera's built-in shake.
    pub fn set_camera_offset(&mut self, offset: RenderVec2);

    /// The camera position this frame renders with (shake and offset
    /// included), after any override.
    pub fn camera_position(&self) -> RenderVec2;
}

// amigo_render::camera
impl Camera {
    /// How fast `shake` intensity decays, in intensity units per second.
    /// Default 8.0, as before.
    pub fn set_shake_decay(&mut self, per_second: f32);

    /// Map a world position to virtual-resolution screen coordinates
    /// (origin top-left), using the effective position and zoom.
    pub fn world_to_screen(&self, world: RenderVec2) -> RenderVec2;
}
```

### R5: Text

```rust
// amigo_render::font (prelude: TextStyle, TextAlign, TextMetrics)

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// `None` uses the default font (see `FontManager::set_default_font`).
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
    /// Default font at its load size, white, no spacing, left-aligned,
    /// z 100 (as `draw_text` today), `BlendMode::Normal`.
    fn default() -> Self;
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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FontError {
    #[error("no font with id {0:?} is loaded")]
    UnknownFont(FontId),
}

impl FontManager {
    /// Make `id` the font used by `draw_text`, `draw_text_scaled`,
    /// `measure_text`, `TextStyle { font: None, .. }` and every `amigo_ui`
    /// widget.
    pub fn set_default_font(&mut self, id: FontId) -> Result<(), FontError>;

    /// The font `default_font()` returns.
    pub fn default_font_id(&self) -> Option<FontId>;
}

// amigo_engine
impl DrawContext<'_> {
    /// Draw one line of text. `pos.y` is the top of the line box; the
    /// baseline sits at `pos.y + ascent`. Returns the drawn line's bounds.
    pub fn draw_text_ex(&mut self, text: &str, pos: RenderVec2, style: &TextStyle) -> Rect;

    /// Measure one line exactly as `draw_text_ex` would lay it out.
    pub fn measure_text_ex(&self, text: &str, style: &TextStyle) -> TextMetrics;
}
```

### R6: Shapes

```rust
// amigo_engine. Every shape honours the current DrawSpace (R3), z (R3) and
// parallax (R9), and uses the white texture.
impl DrawContext<'_> {
    /// Any convex quad, corners in order TL, TR, BR, BL (or any consistent
    /// winding).
    pub fn draw_quad(&mut self, corners: [RenderVec2; 4], color: Color);

    /// Like `draw_quad` with one colour per corner, interpolated across.
    pub fn draw_quad_colors(&mut self, corners: [RenderVec2; 4], colors: [Color; 4]);

    /// Vertical gradient from `top` to `bottom`.
    pub fn draw_gradient_rect(&mut self, rect: Rect, top: Color, bottom: Color);

    /// A segment of `thickness` virtual pixels, centred on the line from `a`
    /// to `b`, with square ends.
    pub fn draw_line(&mut self, a: RenderVec2, b: RenderVec2, thickness: f32, color: Color);

    /// The outline of `rect`, `thickness` pixels wide, inside the rect.
    pub fn draw_rect_outline(&mut self, rect: Rect, thickness: f32, color: Color);

    pub fn draw_circle(&mut self, center: RenderVec2, radius: f32, color: Color);

    /// `radius` is clamped to half the shorter side.
    pub fn draw_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color);

    /// A convex polygon with at least 3 points, in either winding.
    pub fn draw_convex_polygon(&mut self, points: &[RenderVec2], color: Color);
}
```

### R7: Custom post-processing effects

```rust
// amigo_render::post_process (prelude: PostEffect)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum PostEffect {
    // Existing variants, unchanged: Bloom, ChromaticAberration, Vignette,
    // ColorGrading, CrtFilter, ColorblindFilter.

    /// A ring that displaces the image radially, as from an impact.
    /// Coordinates are virtual-resolution screen units (origin top-left).
    Shockwave {
        center: [f32; 2],
        /// Distance of the ring's middle from `center`.
        radius: f32,
        /// Width of the ring. The displacement fades to zero at both edges.
        thickness: f32,
        /// Peak displacement in virtual pixels; negative pulls inwards.
        strength: f32,
    },
    /// Blur along one direction, as from fast motion.
    DirectionalBlur {
        /// Need not be normalised; a zero vector disables the effect.
        direction: [f32; 2],
        /// Length of the blur in virtual pixels. `<= 0` disables the effect.
        length: f32,
    },
    /// A shader registered with `GameContext::register_post_shader`.
    Custom {
        shader: String,
        /// Read by the shader as `post.params[0..4]` (four `vec4<f32>`).
        params: [f32; 16],
    },
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ShaderError {
    #[error("post shader '{name}': WGSL parse error: {message}")]
    Parse { name: String, message: String },
    #[error("post shader '{name}': validation failed: {message}")]
    Validation { name: String, message: String },
    #[error("post shader '{name}': no `fs_main` fragment entry point")]
    MissingEntryPoint { name: String },
}

// amigo_engine
impl GameContext {
    /// Register (or replace) the post shader `name`. Its source is the
    /// fragment stage only; the engine prepends the prelude below. Parsed and
    /// validated now; compiled for the GPU at the start of the next frame.
    pub fn register_post_shader(&mut self, name: &str, wgsl: &str) -> Result<(), ShaderError>;
}
```

The prelude the engine prepends to every registered shader is part of the
contract:

```wgsl
struct PostInput {
    // Size of the scene target in pixels.
    resolution: vec2<f32>,
    // Virtual resolution; screen-unit parameters are in this space.
    virtual_size: vec2<f32>,
    // `TimeInfo::elapsed` in seconds (presentation only, never simulation).
    time: f32,
    _pad: vec3<f32>,
    params: array<vec4<f32>, 4>,
};
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> post: PostInput;

struct PostVertexOutput {
    @builtin(position) position: vec4<f32>,
    // 0..1 across the scene target, origin top-left.
    @location(0) uv: vec2<f32>,
};

// The game's source must define:
// @fragment fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32>
```

### R8: Atlas with frames and origins

```rust
// amigo_assets::atlas_manifest. Loaded from `assets/sprites/**/*.atlas.ron`.

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtlasManifest {
    /// Sheet image, relative to the manifest.
    pub image: String,
    /// Sprites on the sheet, by name. Names are global like other sprite
    /// names; a clash with another sprite is an error.
    pub sprites: BTreeMap<String, AtlasSprite>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtlasSprite {
    /// At least one frame.
    pub frames: Vec<AtlasFrame>,
    /// Pivot in pixels from each frame's top-left corner.
    #[serde(default)]
    pub origin: (f32, f32),
    /// When set, the sprite also registers an `Animation` of the same name.
    #[serde(default)]
    pub fps: Option<f32>,
    #[serde(default)]
    pub looping: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtlasFrame {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Overrides the sprite's `origin` for this frame.
    #[serde(default)]
    pub origin: Option<(f32, f32)>,
}

#[derive(Debug, thiserror::Error)]
pub enum AtlasError {
    #[error("{path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("{path}: image '{image}' not found or unreadable")]
    Image { path: PathBuf, image: String },
    #[error("{path}: sprite '{sprite}' frame {frame} lies outside the {width}x{height} sheet")]
    FrameOutOfBounds { path: PathBuf, sprite: String, frame: usize, width: u32, height: u32 },
    #[error("{path}: sprite '{sprite}' has no frames")]
    NoFrames { path: PathBuf, sprite: String },
    #[error("{path}: sprite name '{sprite}' is already used by {other}")]
    DuplicateName { path: PathBuf, sprite: String, other: String },
}

// `AssetError` gains `Atlas(#[from] AtlasError)`.

// amigo_engine
impl DrawContext<'_> {
    /// Draw frame `frame` of `sprite` with its pivot at `pos`. Frames past
    /// the last one draw the last frame. A plain (non-atlas) sprite has one
    /// frame with its pivot at the top-left.
    pub fn draw_frame(&mut self, sprite: &str, frame: usize, pos: RenderVec2);

    pub fn draw_frame_ex<F>(&mut self, sprite: &str, frame: usize, pos: RenderVec2, f: F)
    where
        F: FnOnce(&mut SpriteInstance);

    /// Number of frames of `sprite`; 0 when the name is unknown.
    pub fn frame_count(&self, sprite: &str) -> usize;
}
```

### R9: Parallax

```rust
// amigo_engine
impl DrawContext<'_> {
    /// World draws after this call move at `fx`/`fy` times the camera's
    /// speed: 1.0 is the world (default), 0.0 is fixed to the screen,
    /// 0.25 a distant layer. Ignored in `DrawSpace::Screen`.
    pub fn set_parallax(&mut self, fx: f32, fy: f32);

    pub fn parallax(&self) -> (f32, f32);
}
```

`draw_tilemap_colored` and `draw_tilemap_sprite` apply the layer's own
`scroll_factor_x`/`scroll_factor_y` in place of the context's factor.

### R10: Raster-art quality and window shape

```rust
// amigo_render::viewport
pub enum ScaleMode {
    PixelPerfect,
    Fit,
    Stretch,
    /// Keep the configured virtual height and widen or narrow the virtual
    /// width to the window's aspect ratio. No bars, no distortion.
    /// Config value `"expand"`.
    Expand,
}

/// What the game needs to lay out for the window it is in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportInfo {
    /// Window size in physical pixels.
    pub window_size: (u32, u32),
    /// Where the scene lands in the window.
    pub viewport: Viewport,
    /// Current virtual resolution (changes under `ScaleMode::Expand`).
    pub virtual_size: (f32, f32),
    /// Scene-target pixels per virtual pixel: 1.0 for pixel art, the
    /// viewport scale for raster art.
    pub render_scale: f32,
    /// The OS scale factor (DPI) of the window's monitor.
    pub scale_factor: f64,
}

// amigo_engine
impl GameContext {
    /// Refreshed by the engine before every `update` and on every resize.
    pub fn viewport_info(&self) -> ViewportInfo;
}
impl DrawContext<'_> {
    pub fn viewport_info(&self) -> ViewportInfo;
}
```

## Behavior

### R1: Sprite transform

- With `origin == [0.0, 0.0]`, `rotation == 0.0` and `geometry == None`, the batcher
  emits exactly the vertices it emits today. Existing games render identically.
- Otherwise the four corners are `rotate(corner - origin, rotation) + (x, y)`, where
  `corner` runs over `(0,0)`, `(width,0)`, `(width,height)` and `(0,height)`.
- `flip_x`/`flip_y` still swap UVs on the same quad, so the art mirrors about the quad
  centre. To mirror about the pivot, use `scale(-1.0, 1.0)`.
- A non-finite `rotation` or `origin` component is treated as 0. A sprite is never
  dropped for it, because the frame path must not fail.
- `draw_sprite`, `draw_sprite_ex`, `draw_animated` and `draw_animated_ex` set `origin`
  from the sprite's registered origin (R8), which is `[0, 0]` for PNG and Aseprite
  sprites. Their behaviour is therefore unchanged for every sprite that exists today.

### R2: Blend modes

- **Premultiplied alpha everywhere.** Texture data is premultiplied on upload: sprites,
  font atlases and the white texture. The sprite shader premultiplies the tint
  (`rgb * a, a`) before it multiplies the sample. This removes the dark fringes that
  linear filtering of straight alpha produces, and it is what makes `Additive` and
  `Multiply` composable with `Normal` in one pass.
- **Blend states.**
  - `Normal`: colour `(One, OneMinusSrcAlpha)`, alpha `(One, OneMinusSrcAlpha)`.
  - `Additive`: colour `(One, One)`, alpha `(Zero, One)`.
  - `Multiply`: colour `(Dst, OneMinusSrcAlpha)`, alpha `(Zero, One)`.
  - An opaque `Normal` sprite with a white tint renders exactly as before.
- **Batching.** Sorting stays a stable sort by `z_order`. A batch breaks when the
  texture *or* the blend mode changes, so painter's order within a z is kept across
  blend modes.
- **Pipelines.** Both the world pass and the UI pass have one pipeline per blend mode.
- **Particles.** `ParticleSystem::collect_sprites` copies `EmitterConfig::blend_mode`
  into each instance, so additive emitters render additively. This fixes the
  additive part of backlog render-18.

### R3: Screen-space drawing

- After `set_space(DrawSpace::Screen)`, every draw call appends to the screen list.
  The engine pushes that list into `ui_batcher`, so it renders in the UI pass:
  - after lighting and post-processing;
  - with the virtual-resolution projection the UI pass already uses (`renderer.rs:692-775`);
  - without camera, zoom, shake or parallax.
- Within the UI pass the engine pushes the game's screen list first and the `ctx.ui`
  widgets second. The stable z sort therefore puts widgets over game screen draws at
  equal z.
- `in_space` restores the previous space when `f` returns, so callers do not need to
  restore it by hand.
- `view_rect()` returns the screen rectangle `(0, 0, virtual_w, virtual_h)` while in
  `Screen`, so tilemap culling stays correct there.
- The space and the default z reset to `World` and 0 at the start of every frame.

### R4: Camera from `draw()`

- The override and offset are stored in the `DrawContext`. After `Game::draw` returns,
  the engine applies them to the renderer's camera for this frame's projection. They
  do not persist into the next frame.
- `set_camera_position` also updates `camera_pos` and `view_rect()` immediately, so
  draws issued after it (parallax layers, tilemap culling) see the new camera. Games
  should call it before drawing anything.
- Draws issued before the call keep their world positions; the projection applies the
  new camera to them as well.
- Calling either function in `DrawSpace::Screen` has the same effect: the camera is
  frame state, not per space.
- Mouse world coordinates (`input.mouse_world_pos`) are computed from the camera as of
  `update`, not from a draw-time override. A game that both overrides the camera and
  picks with the mouse must keep `ctx.camera` close to its override.
- `world_to_screen` is the inverse of `screen_to_world` for in-view points; the two
  round-trip to within 1e-3 virtual pixels.

### R5: Text

- **Unicode.** Every `char` can be drawn. A glyph missing from the atlas is rasterised
  on first use, from `draw()` as well as from `update()`, and drawn in the same frame.
- **Missing glyphs.** A character the font has no glyph for is drawn as the font's
  `.notdef` glyph (glyph index 0) and advances the pen by that glyph's advance. It is
  never silently skipped, as it is today (backlog render-11).
- **Legacy calls.** `draw_text`, `draw_text_scaled`, `draw_text_font`, `measure_text`
  and `measure_text_font` keep their layout but gain Unicode support. They do not gain
  kerning, so existing pixel-font output is unchanged.
- **`draw_text_ex` layout.** It applies kerning between adjacent characters plus
  `letter_spacing` after each one, then anchors the line per `align`.
  - Text is single-line: `\n` is treated like any other control character (no glyph,
    no advance).
  - `measure_text_ex(text, style).width` equals the width of the `Rect` that
    `draw_text_ex` returns for the same arguments.
- **Rasterisation size.** For `draw_text_ex`, glyphs are rasterised at
  `size_px * render_scale` (R10) scene pixels, rounded to the nearest whole pixel, and
  the quads are drawn at `size_px` virtual pixels. The legacy calls keep rasterising at
  the font's load size.
  - Under `raster_art`, text is therefore crisp at the output resolution instead of
    being upscaled from virtual pixels.
  - Under `pixel_art`, `render_scale` is 1 and text renders as today.
- **Atlases.** One atlas is kept per (font, rasterised pixel size). Measuring uses font
  metrics and needs no atlas.
- **Default font.** The engine still loads AmigoPixel first, and it stays the default
  until `set_default_font` is called. After the call, `amigo_ui` widget text uses the
  new default too.

### R6: Shapes

- **Tessellation.** Shapes are tessellated on the CPU into `SpriteInstance`s carrying
  `geometry` on the white texture. They sort and batch like sprites and blend with
  `BlendMode::Normal`.
- **Circle segments.** Circles and the corners of rounded rects use
  `clamp(ceil(2π · r · render_scale / 4), 12, 128)` segments per full turn, so curves
  stay smooth at any output size.
- **Edges.**
  - Under `raster_art`, every shape gets a one-scene-pixel alpha feather on its outer
    edge, so edges are anti-aliased without MSAA.
  - Under `pixel_art`, edges are hard.
- **Degenerate input.** Zero or negative sizes, `thickness <= 0`, `radius <= 0` (for
  circles) and fewer than 3 polygon points draw nothing; they are not errors.
  `draw_rounded_rect` with `radius <= 0` draws a plain rectangle.
- **Non-convex input.** `draw_convex_polygon` with a non-convex point list draws a
  triangle fan from the first point. The result is defined but not the filled
  polygon.

### R7: Custom post-processing effects

- **Chain order.** Effects run in `Vec` order, one fullscreen pass each, ping-ponging
  between two scene-sized targets. The last pass writes the scene target the UI pass
  draws over.
  - This replaces today's fused pass with its fixed order (backlog render-25).
  - An implementation may still fuse adjacent built-in effects into one pass when the
    output is identical.
- **Registration.** `register_post_shader` parses and validates the prelude plus the
  game's source on the CPU with naga and returns the first error. On `Ok` the renderer
  builds the pipeline at the start of the next frame.
  - A GPU-side pipeline failure after a successful validation logs an error and skips
    that effect; it never panics.
  - Registering an existing name replaces the shader; the next frame uses the new one.
- **Skipped effects.**
  - A `Custom` effect whose name is not registered is skipped, with one `warn!` per
    name.
  - A `Shockwave` with `thickness <= 0` or `strength == 0`, and a `DirectionalBlur`
    with a zero direction or `length <= 0`, are skipped silently. That is their "off"
    state, so a game can animate them to zero.
  - When every effect is skipped, the scene is not copied through an extra pass.
- **Built-in effects.** `Shockwave` and `DirectionalBlur` are implemented as built-in
  WGSL on the same prelude and binding layout, as a check that the contract is
  sufficient.
  - `Shockwave` offsets each sample along the direction from `center`. The offset is
    `strength · w(d)` virtual pixels, where `w` is a smooth bump that is 1 at
    `d == radius` and 0 at `|d - radius| >= thickness / 2`.
  - `DirectionalBlur` averages 9 taps spread evenly over `length` virtual pixels,
    centred on the pixel.
- **Constraints on shaders.** Built-in and prelude code uses only features available
  on WebGL2: no storage buffers, no compute, uniform arrays of `vec4`. Registered
  shaders are not restricted, but the prelude does not rule a web target out.
- **Configs.** `PostEffect` is `#[non_exhaustive]`, and `Custom` references shaders by
  name, so effect stacks stay serialisable to RON.

### R8: Atlas with frames and origins

- **Loading.** `AssetManager::load_sprites` picks up `*.atlas.ron` next to the PNG and
  Aseprite files. The sheet is uploaded once as one texture. Each sprite is registered
  under its name with its frame rectangles (in UV space of that one texture) and its
  origins. Nothing is cropped or re-uploaded per sprite.
- **Drawing frames.** `draw_frame` sets `width`/`height` to the frame size, UVs to the
  frame rectangle and `origin` to the frame's origin. `pos` is therefore the pivot,
  which is where Flash's `ox`/`oy` placed the art.
- **Animations.** With `fps` set, the sprite also registers an `Animation` named like
  the sprite.
  - Frame durations are converted to ticks so that frame `k` starts at tick
    `round(k · TICKS_PER_SECOND / fps)`. The clip's total length is right to within
    one tick even when the frame rate does not divide the tick rate (19 fps at 60 Hz
    gives 3, 3, 3, 4, … ticks).
  - `draw_animated` on an atlas sprite draws frame `player.frame_index` through
    `draw_frame`, origins included.
- **Errors.** Each `AtlasError` variant is reported for its file. Like other sprite
  load errors it is logged, and that manifest's sprites are skipped; the remaining
  assets still load.
- **Hot reload.** A change to the manifest or its image reloads all of that manifest's
  sprites.
- **Packing.** `amigo pack` copies atlas sheets into the pak unchanged instead of
  re-packing their frames.

### R9: Parallax

- **Offset.** A world draw at parallax `(fx, fy)` is shifted by
  `(camera.x · (1 - fx), camera.y · (1 - fy))` when it is pushed. Zoom applies to every
  layer alike.
- **Culling.** `view_rect()` is shifted the opposite way, so tilemap culling matches
  the shifted layer.
- **Scope.**
  - `DrawSpace::Screen` ignores parallax.
  - The factor resets to `(1.0, 1.0)` at the start of every frame.
  - A non-finite factor is treated as 1.0.

### R10: Raster-art quality and window shape

- **Mipmaps.** Textures sampled with `SamplerMode::Linear` get a full mip chain at
  upload, and their sampler filters between levels linearly. Art drawn smaller than
  authored then stops shimmering. `Nearest` textures keep a single level.
- **`ScaleMode::Expand`.** The virtual height stays as configured and the virtual
  width becomes `round(virtual_height · window_w / window_h)`, recomputed on every
  resize.
  - The viewport covers the whole window.
  - The engine updates `GameContext::camera.virtual_width`, `DrawContext.virtual_width`
    and `ViewportInfo::virtual_size` before the next `update`.
- **`ViewportInfo`.** Before the first window event, `viewport_info()` describes the
  configured window size. It never returns zero sizes.

## Internal Design

These are suggestions, not part of the contract.

- **R1.** Corner rotation happens in `SpriteBatcher::build` on the CPU, where vertices
  are already written in world space (`sprite_batcher.rs:151-170`). It costs one
  `sin_cos` per rotated sprite; unrotated sprites skip it. `SpriteInstance` grows by
  about 112 bytes, mostly `Option<QuadGeometry>`. That is acceptable at the current
  1 024-sprite capacity; revisit if instancing (`instancing.rs`) is ever wired.
- **R2.** `SpriteBatch` gains a `blend: BlendMode` field. The renderer keeps
  `[wgpu::RenderPipeline; 3]` per pass and switches pipeline when consecutive batches
  differ.
  - Premultiplication runs once in `Texture::from_image_with_mode`.
  - Font atlases store coverage in alpha with white RGB, so premultiplied texels are
    `(a, a, a, a)`.
- **R3/R4.** `DrawContext` gets a second `Option<&mut Vec<SpriteInstance>>` and a
  small `FrameOverrides` struct (camera position, offset). The engine reads it back
  through a crate-private accessor after `draw`. `engine.rs` already holds a
  `ui_draw_list`; the screen list is pushed into `ui_batcher` just before it.
- **R5.**
  - Glyph caching from `&self` needs interior mutability in `FontAtlas`: a `RefCell`
    around the glyph map and atlas pixels, or a `Mutex` if `GameContext` must stay
    `Sync`.
  - `upload_font_atlases` (`engine.rs:582`) moves to after `Game::draw`, so glyphs
    added during `draw` upload before the frame renders.
  - `.notdef` comes from `fontdue::Font::lookup_glyph_index` returning 0 and
    `rasterize_indexed(0, px)`.
  - Atlases for sizes not used for 600 frames can be dropped once textures can be
    removed (backlog render-10). Until then their number is bounded by the distinct
    sizes a game uses.
- **R6.** A triangle is a quad whose fourth corner repeats the third, so the index
  buffer layout stays two triangles per instance. The feather ring is a second strip
  of quads whose outer corners have alpha 0.
- **R7.**
  - naga is already in `Cargo.lock` through wgpu; depend on it at the same version
    (`[workspace.dependencies]`).
  - The registry lives in `GameContext` (name → source, plus a generation counter).
    The renderer compiles entries whose generation changed.
  - The existing uber-shader can stay as the implementation of the six classic
    effects: one pass per effect with only that effect's flag set, or fused when
    adjacent and already in canonical order.
- **R8.** The sprite registry in `GameContext` (`sprite_textures`, `context.rs:118`)
  becomes `name → SpriteEntry { texture, size, frames: Vec<FrameEntry { uv: Rect,
  size, origin }> }`. Plain sprites hold one frame covering the texture, so
  `find_sprite_texture` keeps its signature.
- **R10.** Generate mip levels on the CPU with `image::imageops::resize` (Triangle
  filter) at load. That is simple and fast enough for load-time use; a GPU blit chain
  is an option if load times matter.

## Breaking changes

`SpriteInstance`, `PostEffect`, `DrawContext` and `GameContext` are in
`amigo_engine::prelude`. Pre-1.0, each of these goes into `CHANGELOG.md` under
`[Unreleased]` as breaking, with the migration:

| Change | Who breaks | Migration |
|---|---|---|
| `SpriteInstance` gains `origin`, `rotation`, `blend`, `geometry` | struct literals outside the engine | `SpriteInstance::new(..)` plus field assignment, or `..SpriteInstance::new(..)` |
| `PostEffect` gains variants and `#[non_exhaustive]` | exhaustive `match`es on `PostEffect` | add a `_ => {}` arm |
| `particles::BlendMode` becomes a re-export of `BlendMode` with a third variant | exhaustive `match`es on it | add the `Multiply` arm |
| `ScaleMode` gains `Expand` | exhaustive `match`es on `ScaleMode` | add the arm |

Rendering output changes only where a game opts into the new fields. The one global
change is premultiplied alpha (R2), which gives identical results for opaque sprites
and removes fringes on translucent edges under linear filtering.

## Non-Goals

- **A web/WASM target** ("Later" in `docs/specs/index.md`). R7 keeps its own shaders
  WebGL2-compatible so it does not stand in the way.
- **A configurable simulation tick rate.** `TICKS_PER_SECOND` is a constant
  (`amigo_core/src/time.rs:19`); that is a core-loop change for its own spec.
- **Touch input.**
- **Per-sprite `SpriteShader` effects** (the other half of backlog render-18). The
  field stays, unrendered.
- **Particles:** textured or rotated engine particles, and GPU particles. A game that
  needs them draws its own sprites with R1 and R2.
- **Text beyond R5:** SDF/MSDF text, multi-line layout and wrapping (`TextLayout`
  stays unwired), shaping of complex scripts, and bidirectional text. fontdue does no
  shaping.
- **Vector graphics:** concave or self-intersecting polygons, strokes with joins, and
  vector or SWF shape import.
- **The sRGB colour-space fix** (backlog render-8) and MSAA.
- **HamsterFlight itself:** porting it, or shipping any of its assets. Its sprites and
  sounds are extracted from the original SWF and belong to their owners.

## Open Questions

These do not change the public API.

- Mipmap generation on the CPU at load or with a GPU blit chain.
- Whether font-atlas eviction waits for a texture-removal API (backlog render-10) or
  lands with it.
- Whether the six classic post effects keep a fused pass when they appear adjacent and
  in canonical order, or always run one pass each.

## Acceptance Criteria

Rendering cannot be checked on a GPU in CI. Every criterion below is checked on the
CPU: vertex and batch output, draw lists, tessellation, shader validation, and pure
layout functions. The example at the end only has to compile.

### R1: Sprite transform
- [ ] `SpriteInstance` has the fields `origin: [f32; 2]`, `rotation: f32`, `blend: BlendMode`, `geometry: Option<QuadGeometry>`
- [ ] `QuadGeometry { corners: [[f32; 2]; 4], colors: [Color; 4] }` exists and derives `Clone, Copy, Debug, PartialEq`
- [ ] `SpriteInstance::new(texture_id, x, y, width, height) -> Self` yields white tint, z 0, no flip, origin `[0, 0]`, rotation 0, `BlendMode::Normal`, `geometry: None`, full UV rect
- [ ] Test: a sprite with default origin and rotation produces the same four vertices as before the change
- [ ] Test: `rotation = π/2`, origin at the centre of a 2×4 quad at (10, 10) yields corners rotated clockwise about (10, 10)
- [ ] Test: `scale(2.0, 2.0)` doubles `width`, `height` and `origin`; `scale(-1.0, 1.0)` toggles `flip_x` and sets `origin[0] = width - origin[0]`
- [ ] Test: `set_origin_normalized(0.5, 1.0)` sets `origin = [width / 2, height]`
- [ ] Test: `corners()` and `bounds()` agree with the vertices the batcher writes
- [ ] Test: non-finite `rotation` or `origin` renders as 0 instead of being dropped
- [ ] Every `SpriteInstance { .. }` literal in the workspace uses `SpriteInstance::new` or struct update syntax

### R2: Blend modes
- [ ] `amigo_render::BlendMode { Normal, Additive, Multiply }` exists, derives `Serialize, Deserialize, Default` (`Normal`), and is in the prelude
- [ ] `amigo_render::particles::BlendMode` is the same type (a re-export); an existing RON emitter config with `blend_mode: Normal` still deserialises
- [ ] Test: batches break on a blend change between same-texture sprites, and z-order with submission order is preserved
- [ ] `SpriteBatch` carries its blend mode; the renderer creates one pipeline per mode for the world and the UI pass with the blend states listed in Behavior
- [ ] Test: texture upload premultiplies RGB by alpha (CPU helper, e.g. `premultiply(&mut RgbaImage)`)
- [ ] Test: `ParticleSystem::collect_sprites` sets `blend: Additive` for an emitter configured additive

### R3: Screen space
- [ ] `DrawSpace { World, Screen }` exists (default `World`) and is in the prelude
- [ ] `DrawContext::{with_screen_list, set_space, space, in_space, set_z}` exist with the signatures above
- [ ] Test: draws after `set_space(Screen)` land in the screen list, draws after `set_space(World)` in the world list
- [ ] Test: `in_space` restores the previous space after the closure returns
- [ ] Test: a context without a screen list drops screen draws without panicking
- [ ] Test: `set_z(5)` sets `z_order` 5 on `draw_sprite`, `draw_rect` and shapes, and not on `draw_text_ex` (which uses the style's z)
- [ ] `engine.rs` pushes the screen list into `ui_batcher` before the `ctx.ui` widgets

### R4: Camera from draw
- [ ] `DrawContext::{set_camera_position, set_camera_offset, camera_position}` exist
- [ ] Test: after `set_camera_position(p)`, `camera_position()` and `view_rect()` are centred on `p` (plus offset)
- [ ] The engine applies the override to the renderer camera after `Game::draw`, and `GameContext::camera.position` is unchanged afterwards
- [ ] `Camera::set_shake_decay(f32)` and `Camera::world_to_screen(RenderVec2) -> RenderVec2` exist
- [ ] Test: `world_to_screen(screen_to_world(p)) == p` within 1e-3 for points in view, with zoom ≠ 1

### R5: Text
- [ ] `TextStyle`, `TextAlign`, `TextMetrics`, `FontError` exist with the fields above; `TextStyle::default()` matches the documented defaults
- [ ] `FontManager::set_default_font(FontId) -> Result<(), FontError>` and `default_font_id() -> Option<FontId>` exist; an unknown id returns `FontError::UnknownFont`
- [ ] Test: `draw_text("Grüße", ..)` with a TTF containing those glyphs pushes 5 glyph quads (it pushes 3 today)
- [ ] Test: a character missing from the font draws the `.notdef` glyph and advances the pen
- [ ] Test: `measure_text_ex(t, s).width` equals the width of the `Rect` from `draw_text_ex(t, pos, s)` for left, centre and right alignment
- [ ] Test: `letter_spacing` adds `n · spacing` to the width of an `n`-character line
- [ ] Test: with `render_scale = 2.0`, a 10 px style rasterises a 20 px atlas and draws 10 px quads
- [ ] Test: after `set_default_font`, `amigo_ui` text uses the new font's texture
- [ ] `upload_font_atlases` runs after `Game::draw`

### R6: Shapes
- [ ] `DrawContext::{draw_quad, draw_quad_colors, draw_gradient_rect, draw_line, draw_rect_outline, draw_circle, draw_rounded_rect, draw_convex_polygon}` exist with the signatures above
- [ ] Test: each shape's tessellation covers the expected area (sum of triangle areas within 1 % of the analytic area for circle and rounded rect at radius 50)
- [ ] Test: segment count follows `clamp(ceil(2π · r · render_scale / 4), 12, 128)`
- [ ] Test: in `raster_art` the outer feather quads have alpha 0 at their outer corners; in `pixel_art` there are none
- [ ] Test: each degenerate input listed in Behavior draws nothing and does not panic

### R7: Post effects
- [ ] `PostEffect::{Shockwave, DirectionalBlur, Custom}` exist; `PostEffect` is `#[non_exhaustive]` and still round-trips through RON
- [ ] `ShaderError` exists with `Parse`, `Validation` and `MissingEntryPoint`
- [ ] `GameContext::register_post_shader(&str, &str) -> Result<(), ShaderError>` exists
- [ ] Test: a valid shader registers; a syntax error returns `Parse`; a type error returns `Validation`; a source without `fs_main` returns `MissingEntryPoint`
- [ ] Test: the built-in Shockwave and DirectionalBlur sources validate against the prelude with naga
- [ ] Test: the pass plan for `[Vignette, Custom("a"), Bloom]` runs in that order (CPU function returning the pass list)
- [ ] Test: unregistered `Custom` names and disabled built-ins are dropped from the pass plan; an all-skipped stack produces no passes
- [ ] The prelude WGSL in this spec matches the source the engine prepends (one test reads both)

### R8: Atlas
- [ ] `AtlasManifest`, `AtlasSprite`, `AtlasFrame`, `AtlasError` exist with the fields and variants above; `AssetError::Atlas` wraps `AtlasError`
- [ ] Test: a manifest with two sprites loads one texture and registers both names
- [ ] Test: each `AtlasError` variant is produced by a matching malformed manifest, and the other assets still load
- [ ] `DrawContext::{draw_frame, draw_frame_ex, frame_count}` exist
- [ ] Test: `draw_frame` sets size, UVs and origin from the frame; a frame index past the end draws the last frame; a plain PNG sprite reports `frame_count == 1`
- [ ] Test: a 19 fps sprite yields frame durations whose running sum stays within one tick of `k · 60 / 19`
- [ ] Test: `draw_animated` on an atlas sprite draws the player's current frame with its origin
- [ ] `amigo pack` keeps atlas sheets intact (test on the pack output)

### R9: Parallax
- [ ] `DrawContext::{set_parallax, parallax}` exist
- [ ] Test: with camera at (100, 50) and parallax (0.5, 0.0), a sprite drawn at (0, 0) is pushed at (50, 50)
- [ ] Test: `draw_tilemap_*` uses the layer's `scroll_factor_x/y`
- [ ] Test: screen-space draws ignore parallax; the factor resets each frame

### R10: Quality and window shape
- [ ] `ScaleMode::Expand` exists and parses from `"expand"`
- [ ] Test: `Viewport::compute(Expand, (640, 360), (1000, 500))` covers the window, and the expanded virtual width is 720
- [ ] `ViewportInfo` exists; `GameContext::viewport_info()` and `DrawContext::viewport_info()` return it
- [ ] Test: mip chain generation for a 64×32 `Linear` texture yields 7 levels (64×32 down to 1×1); a `Nearest` texture yields 1

### Wiring
- [ ] A new example `examples/raster_art` (workspace member) uses `art_style = "raster_art"` with `scale_mode = "expand"`. It draws a rotated sprite from an atlas, additive particles, screen-space text with umlauts in an embedded TTF, rounded rects and a `Custom` post shader. It compiles in `cargo check --workspace`.

### Quality gates
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`, and the rest of `just ci` (feature matrix, doc tests, rustdoc)
- [ ] No `unwrap()` in library code on the frame path; a bad input degrades as described in Behavior

### Conventions
- [ ] Nothing in this spec reaches simulation state: every new value is `f32` presentation data (ADR-0001)
- [ ] Screen-space draws and UI stay after post-processing (conventions A.6)
- [ ] `CHANGELOG.md` `[Unreleased]` lists each breaking change from the table above
- [ ] The spec's `status:` moves to `done` only when every section is reachable from a game, per `docs/specs/index.md`
