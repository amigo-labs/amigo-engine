---
status: spec
crate: amigo_render
depends_on: ["engine/rendering", "engine/input", "engine/ui"]
last_updated: 2026-03-18
---

# Accessibility

## Purpose

Make games built with the Amigo Engine playable by as wide an audience as
possible.  This spec covers colorblind vision filters, high-contrast UI themes,
scalable text, flexible input remapping, input assistance features, screen shake
reduction, and a subtitle system for audio cues.  All features are opt-in and
controllable from an in-game accessibility settings menu.

## Existierende Bausteine

### Post-Processing Pipeline (`crates/amigo_render/src/post_process.rs`, 617 lines)

The engine already has a single-pass fullscreen post-processing system that
applies effects via a uniform-driven WGSL shader.

| Existing component | Description |
|--------------------|-------------|
| `PostEffect` enum | `Bloom`, `ChromaticAberration`, `Vignette`, `ColorGrading`, `CrtFilter` |
| `PostProcessUniforms` | `#[repr(C)]` uniform buffer: thresholds, intensities, screen dimensions, `enabled_flags` bitfield |
| `PostProcessPipeline` | Offscreen render target, fullscreen triangle pass, bind group with scene texture + sampler + uniforms |
| `PostProcessPipeline::set_effects(vec)` | Replace entire effect stack |
| `PostProcessPipeline::apply(encoder, device, queue, output_view)` | Run post-process chain |
| `PostProcessPipeline::resize(device, w, h)` | Recreate offscreen target on window resize |
| `PostProcessPipeline::set_sampler_mode(device, mode)` | Switch between nearest/linear filtering |
| Enabled flags bitfield | bit 0: Bloom, bit 1: Chroma, bit 2: Vignette, bit 3: ColorGrading, bit 4: CRT |

The existing `ColorGrading` effect adjusts brightness, contrast, and saturation.
Colorblind filters extend this same uniform-driven architecture with a color
transformation matrix.

### SpriteShader effects (`crates/amigo_render/src/sprite_batcher.rs`)

Per-sprite visual effects already exist: `Flash`, `Outline`, `Dissolve`,
`PaletteSwap`, `Silhouette`, `Wave`.  High-contrast mode can leverage the
`Outline` shader to add borders around interactive elements.

## Public API

### Proposed: Colorblind Filters

```rust
/// Colorblind simulation/correction modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorblindMode {
    None,
    /// Red-green (most common, ~6% of males).
    Deuteranopia,
    /// Red-blind.
    Protanopia,
    /// Blue-yellow (rare).
    Tritanopia,
    /// Monochromacy (extremely rare).
    Achromatopsia,
}

/// Extends PostEffect with colorblind correction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PostEffect {
    // ... existing variants unchanged ...
    Bloom { threshold: f32, intensity: f32 },
    ChromaticAberration { offset: f32 },
    Vignette { intensity: f32, smoothness: f32 },
    ColorGrading { brightness: f32, contrast: f32, saturation: f32 },
    CrtFilter { scanline_intensity: f32, curvature: f32 },

    /// Colorblind correction filter.
    ColorblindFilter { mode: ColorblindMode, strength: f32 },
}
```

The shader implements Daltonization using a 3x3 color matrix per mode:

```wgsl
// Deuteranopia correction matrix (Machado et al. 2009)
fn daltonize_deuteranopia(c: vec3<f32>) -> vec3<f32> {
    let m = mat3x3<f32>(
        vec3<f32>(0.625, 0.375, 0.0),
        vec3<f32>(0.7,   0.3,   0.0),
        vec3<f32>(0.0,   0.3,   0.7),
    );
    return m * c;
}
```

The uniform buffer gains one additional field:

```rust
// Added to PostProcessUniforms
pub colorblind_mode: u32,      // 0=none, 1=deuter, 2=protan, 3=tritan, 4=achromat
pub colorblind_strength: f32,  // 0.0..1.0 blend with original
// enabled_flags bit 5 (32) = ColorblindFilter
```

### Proposed: High Contrast UI Themes

```rust
/// A high-contrast theme that overrides default UI colors.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HighContrastTheme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub interactive: Color,         // buttons, links
    pub interactive_hover: Color,
    pub border_width: f32,          // thicker borders for visibility
    pub text_shadow: bool,          // dark outline behind text
}

impl HighContrastTheme {
    /// Built-in "White on Black" theme.
    pub fn white_on_black() -> Self;
    /// Built-in "Black on White" theme.
    pub fn black_on_white() -> Self;
    /// Built-in "Yellow on Blue" theme (popular for low vision).
    pub fn yellow_on_blue() -> Self;
}
```

The UI system reads the active theme from `AccessibilitySettings` and applies
it to all widget rendering.  Game-specific custom themes can be registered.

### Proposed: Text Scaling

```rust
/// Global text scale factor.  Applied on top of per-font pixel sizes.
/// Depends on the font-rendering spec's `TextLayout::scale` field.
pub struct TextScaleSettings {
    pub global_scale: f32,          // default 1.0, range 0.5..3.0
    pub ui_scale: f32,              // separate scale for UI text
    pub subtitle_scale: f32,        // separate scale for subtitles
}
```

### Proposed: Gamepad Remapping

```rust
/// Represents a bindable game action.
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionId(pub String);

/// A single input binding (one action can have multiple bindings).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InputBinding {
    Key(KeyCode),
    MouseButton(MouseButton),
    GamepadButton(GamepadButton),
    GamepadAxis { axis: GamepadAxis, direction: AxisDirection },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum AxisDirection { Positive, Negative }

/// Maps actions to input bindings.  All bindings are user-remappable.
pub struct InputMap {
    bindings: HashMap<ActionId, Vec<InputBinding>>,
}

impl InputMap {
    pub fn new() -> Self;
    /// Bind an action to an input.  Multiple inputs can map to one action.
    pub fn bind(&mut self, action: ActionId, binding: InputBinding);
    /// Remove all bindings for an action.
    pub fn unbind(&mut self, action: &ActionId);
    /// Check if an action is currently pressed.
    pub fn is_pressed(&self, action: &ActionId, input_state: &InputState) -> bool;
    /// Save bindings to a RON file.
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error>;
    /// Load bindings from a RON file, merging with defaults.
    pub fn load(&mut self, path: &Path) -> Result<(), std::io::Error>;
}
```

### Proposed: Input Helpers

```rust
/// Input assistance settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputAssistSettings {
    /// Sticky keys: a press toggles the action on, next press toggles off.
    pub sticky_keys: HashSet<ActionId>,
    /// Hold-to-activate: action only fires after being held for `duration`.
    pub hold_to_activate: HashMap<ActionId, f32>,
    /// Toggle vs hold mode for actions that are normally hold-to-use.
    /// When true, a single press toggles the action on/off.
    pub toggle_mode: HashSet<ActionId>,
    /// Repeat delay and rate for held inputs (key repeat).
    pub repeat_delay: f32,       // seconds before repeat starts (default 0.5)
    pub repeat_rate: f32,        // repeats per second (default 10.0)
}

impl InputAssistSettings {
    pub fn default() -> Self;
    /// Process raw input through assistance filters.
    pub fn filter(&self, action: &ActionId, raw_pressed: bool,
                  held_duration: f32, dt: f32) -> bool;
}
```

### Proposed: Screen Shake Reduction

```rust
/// Controls how screen shake is applied.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShakeSettings {
    /// Global multiplier for screen shake intensity.  0.0 = disabled.
    pub intensity_multiplier: f32,   // default 1.0
    /// Cap on maximum pixel displacement.
    pub max_displacement: f32,       // default 16.0 pixels
    /// Replace shake with a screen flash (for players sensitive to motion).
    pub flash_instead: bool,         // default false
}
```

### Proposed: Subtitle System

```rust
/// A subtitle entry for an audio cue.
#[derive(Clone, Debug)]
pub struct Subtitle {
    pub text: String,
    pub category: SubtitleCategory,
    pub direction: Option<SubtitleDirection>,
    pub remaining: f32,              // seconds remaining on screen
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubtitleCategory {
    Music,
    SoundEffect,
    Voice,
    Ambient,
}

/// Directional indicator for spatial sounds.
#[derive(Clone, Copy, Debug)]
pub enum SubtitleDirection {
    Left,
    Right,
    Above,
    Below,
    Behind,
}

/// Manages on-screen subtitles for audio cues.
pub struct SubtitleManager {
    active: Vec<Subtitle>,
    enabled_categories: HashSet<SubtitleCategory>,
}

impl SubtitleManager {
    pub fn new() -> Self;
    /// Push a new subtitle. Duration in seconds.
    pub fn push(&mut self, text: impl Into<String>,
                category: SubtitleCategory,
                direction: Option<SubtitleDirection>,
                duration: f32);
    /// Tick: remove expired subtitles.
    pub fn update(&mut self, dt: f32);
    /// Get currently active subtitles for rendering.
    pub fn active(&self) -> &[Subtitle];
    /// Enable/disable subtitle categories.
    pub fn set_enabled(&mut self, category: SubtitleCategory, enabled: bool);
}
```

### Proposed: Unified AccessibilitySettings

```rust
/// Central accessibility configuration, saved per-user.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessibilitySettings {
    pub colorblind_mode: ColorblindMode,
    pub colorblind_strength: f32,
    pub high_contrast_theme: Option<String>,
    pub text_scale: TextScaleSettings,
    pub input_assist: InputAssistSettings,
    pub shake: ShakeSettings,
    pub subtitles_enabled: bool,
    pub subtitle_categories: Vec<SubtitleCategory>,
}

impl AccessibilitySettings {
    pub fn default() -> Self;
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error>;
    pub fn load(path: &Path) -> Result<Self, std::io::Error>;
    /// Apply visual settings to the post-process pipeline.
    pub fn apply_to_pipeline(&self, pipeline: &mut PostProcessPipeline);
}
```

## Behavior

### Colorblind Filters

The fragment shader applies the colorblind correction matrix after all other
post-processing effects (bloom, vignette, etc.) but before CRT scanlines.
The `strength` parameter linearly interpolates between the original color and
the corrected color, allowing partial correction.  The matrices are based on
Machado, Oliveira & Fernandes (2009) simulation model.

### High Contrast

When a high-contrast theme is active:
1. All UI panel backgrounds use `theme.background`.
2. All text uses `theme.foreground` with an optional text shadow.
3. Interactive elements (buttons, sliders) use `theme.interactive`.
4. Border widths increase to `theme.border_width`.
5. Game-world sprites are unaffected (only UI changes).

### Input Assistance

The input filter processes raw input each frame:
1. If `sticky_keys` contains the action, toggle state on key-down.
2. If `hold_to_activate` has a duration, only return true after the key has
   been held for that duration continuously.
3. If `toggle_mode` contains the action, convert hold semantics to toggle.
4. Repeat logic: after `repeat_delay`, fire at `repeat_rate` Hz.

### Subtitle Rendering

Subtitles render as an overlay in the UI layer:
- Positioned at the bottom center of the screen.
- Each subtitle shows category icon + text + optional directional arrow.
- Music subtitles show `[Music: text]` in italics.
- SFX subtitles show `[SFX: text]` with a directional indicator if spatial.
- Maximum 3 subtitles visible simultaneously; oldest scrolls out.

### Settings Persistence

`AccessibilitySettings` serializes to `accessibility.ron` in the user's save
directory.  Loaded at startup before the first frame renders.

## Internal Design

- Colorblind matrices are compiled into the WGSL shader as constants (one per
  mode).  The uniform selects which matrix to use via a switch.  This avoids
  uploading a matrix buffer and keeps the shader simple.
- `InputMap` and `InputAssistSettings` are separate from the rendering crate;
  they live in `amigo_input` but are referenced by `AccessibilitySettings`.
- `SubtitleManager` is a simple ring buffer that the audio system pushes into
  whenever a sound plays.  The UI layer reads it each frame.
- The `enabled_flags` bitfield in `PostProcessUniforms` gains bit 5 (32) for
  colorblind filtering.  The uniform struct grows by 8 bytes (mode + strength).

## Non-Goals

- Screen reader / text-to-speech integration (platform-specific, out of scope).
- Full WCAG compliance (the engine targets games, not web applications).
- Seizure-safe animation analysis (developers must test manually).
- Voice input.
- Eye tracking.

## Open Questions

1. Should colorblind filters correct or simulate?  Correction shifts colors to
   be distinguishable; simulation shows what a colorblind person sees (useful
   for developers testing).  Both modes, or just correction?
2. Should subtitle duration be automatic (based on text length) or manually
   specified per cue?
3. Should the input map support chords (multiple simultaneous buttons for one
   action)?
4. Should high-contrast themes affect game-world elements (e.g. enemy outlines)
   or only the UI layer?
5. How should accessibility settings interact with the post-process effect stack
   ordering?  Should colorblind correction always be last?

## Referenzen

- Post-process pipeline: `crates/amigo_render/src/post_process.rs` (617 lines)
- Sprite shaders: `crates/amigo_render/src/sprite_batcher.rs` (218 lines)
- Machado, Oliveira, Fernandes: "A Physiologically-based Model for Simulation of Color Vision Deficiency" (IEEE TVCG, 2009)
- Game Accessibility Guidelines: gameaccessibilityguidelines.com
- Xbox Accessibility Guidelines: learn.microsoft.com/gaming/accessibility
