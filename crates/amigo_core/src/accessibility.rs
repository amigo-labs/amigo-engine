//! Accessibility system for the Amigo Engine.
//!
//! Provides colorblind vision filters, subtitle management, screen-shake
//! reduction, high-contrast mode, and input remapping toggles so that games
//! built with the engine are playable by as wide an audience as possible.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::color::Color;

// ---------------------------------------------------------------------------
// Colorblind mode
// ---------------------------------------------------------------------------

/// Colorblind simulation / correction modes.
///
/// The matrices are based on the Machado, Oliveira & Fernandes (2009)
/// physiologically-based simulation model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ColorBlindMode {
    /// No color correction.
    #[default]
    None,
    /// Red-blind.
    Protanopia,
    /// Red-green (most common, ~6 % of males).
    Deuteranopia,
    /// Blue-yellow (rare).
    Tritanopia,
    /// Monochromacy (extremely rare).
    Achromatopsia,
}

// ---------------------------------------------------------------------------
// ColorBlindFilter
// ---------------------------------------------------------------------------

/// CPU-side colorblind correction filter.
///
/// This is the software fallback / preview path.  The GPU path lives in the
/// post-process WGSL shader and uses the same matrices as constants.
pub struct ColorBlindFilter {
    /// Active colorblind mode.
    pub mode: ColorBlindMode,
    /// Blend strength between the original and corrected color (0.0 .. 1.0).
    pub strength: f32,
}

impl ColorBlindFilter {
    /// Create a new filter for the given mode at full strength.
    pub fn new(mode: ColorBlindMode) -> Self {
        Self {
            mode,
            strength: 1.0,
        }
    }

    /// Create a new filter with explicit strength.
    pub fn with_strength(mode: ColorBlindMode, strength: f32) -> Self {
        Self {
            mode,
            strength: strength.clamp(0.0, 1.0),
        }
    }

    /// Remap a single color according to the active mode and strength.
    ///
    /// The alpha channel is preserved unchanged.
    pub fn remap_color(&self, color: Color) -> Color {
        if self.mode == ColorBlindMode::None || self.strength == 0.0 {
            return color;
        }

        let (r, g, b) = (color.r, color.g, color.b);

        // Apply the 3x3 Daltonization matrix for the selected mode.
        let (nr, ng, nb) = match self.mode {
            ColorBlindMode::None => unreachable!(),
            ColorBlindMode::Protanopia => {
                // Protanopia correction (Machado et al. 2009)
                let nr = 0.567 * r + 0.433 * g + 0.000 * b;
                let ng = 0.558 * r + 0.442 * g + 0.000 * b;
                let nb = 0.000 * r + 0.242 * g + 0.758 * b;
                (nr, ng, nb)
            }
            ColorBlindMode::Deuteranopia => {
                // Deuteranopia correction (Machado et al. 2009)
                let nr = 0.625 * r + 0.375 * g + 0.000 * b;
                let ng = 0.700 * r + 0.300 * g + 0.000 * b;
                let nb = 0.000 * r + 0.300 * g + 0.700 * b;
                (nr, ng, nb)
            }
            ColorBlindMode::Tritanopia => {
                // Tritanopia correction (Machado et al. 2009)
                let nr = 0.950 * r + 0.050 * g + 0.000 * b;
                let ng = 0.000 * r + 0.433 * g + 0.567 * b;
                let nb = 0.000 * r + 0.475 * g + 0.525 * b;
                (nr, ng, nb)
            }
            ColorBlindMode::Achromatopsia => {
                // Monochromacy – standard luminance weights (Rec. 709).
                let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                (lum, lum, lum)
            }
        };

        // Blend with the original color according to strength.
        let blend = self.strength;
        let inv = 1.0 - blend;
        Color::new(
            (nr * blend + r * inv).clamp(0.0, 1.0),
            (ng * blend + g * inv).clamp(0.0, 1.0),
            (nb * blend + b * inv).clamp(0.0, 1.0),
            color.a,
        )
    }
}

// ---------------------------------------------------------------------------
// Subtitle system
// ---------------------------------------------------------------------------

/// Category for a subtitle entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubtitleCategory {
    /// Background music / score.
    Music,
    /// Sound effects.
    SoundEffect,
    /// Voiced dialogue.
    Voice,
    /// Ambient environmental sounds.
    Ambient,
}

/// Directional indicator for spatial sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubtitleDirection {
    /// Sound originates to the left.
    Left,
    /// Sound originates to the right.
    Right,
    /// Sound originates above.
    Above,
    /// Sound originates below.
    Below,
    /// Sound originates behind the listener.
    Behind,
}

/// A single subtitle entry for an audio cue.
#[derive(Clone, Debug)]
pub struct Subtitle {
    /// Display text (e.g. dialogue line or "[footsteps]").
    pub text: String,
    /// Optional speaker name for voiced dialogue.
    pub speaker: Option<String>,
    /// Category of the audio cue.
    pub category: SubtitleCategory,
    /// Optional direction the sound comes from.
    pub direction: Option<SubtitleDirection>,
    /// Seconds remaining on screen.
    pub remaining: f32,
    /// Total duration this subtitle was created with (for progress tracking).
    pub duration: f32,
}

/// Manages the on-screen subtitle queue.
///
/// The audio system pushes subtitle entries whenever a sound plays.  The UI
/// layer reads the active list each frame and renders up to
/// [`SubtitleManager::MAX_VISIBLE`] entries.
pub struct SubtitleManager {
    active: Vec<Subtitle>,
    enabled_categories: HashSet<SubtitleCategory>,
}

impl SubtitleManager {
    /// Maximum subtitles visible simultaneously.
    pub const MAX_VISIBLE: usize = 3;

    /// Create a new manager with all categories enabled.
    pub fn new() -> Self {
        let mut enabled = HashSet::new();
        enabled.insert(SubtitleCategory::Music);
        enabled.insert(SubtitleCategory::SoundEffect);
        enabled.insert(SubtitleCategory::Voice);
        enabled.insert(SubtitleCategory::Ambient);
        Self {
            active: Vec::new(),
            enabled_categories: enabled,
        }
    }

    /// Push a new subtitle.  It will be ignored if its category is disabled.
    pub fn push(
        &mut self,
        text: impl Into<String>,
        speaker: Option<String>,
        category: SubtitleCategory,
        direction: Option<SubtitleDirection>,
        duration: f32,
    ) {
        if !self.enabled_categories.contains(&category) {
            return;
        }
        self.active.push(Subtitle {
            text: text.into(),
            speaker,
            category,
            direction,
            remaining: duration,
            duration,
        });
    }

    /// Tick the subtitle timers and remove expired entries.
    pub fn update(&mut self, dt: f32) {
        for sub in &mut self.active {
            sub.remaining -= dt;
        }
        self.active.retain(|s| s.remaining > 0.0);
    }

    /// Get the currently active subtitles for rendering.
    ///
    /// Returns at most [`Self::MAX_VISIBLE`] entries; newest entries take
    /// priority.
    pub fn active(&self) -> &[Subtitle] {
        let len = self.active.len();
        if len <= Self::MAX_VISIBLE {
            &self.active
        } else {
            &self.active[len - Self::MAX_VISIBLE..]
        }
    }

    /// Enable or disable a subtitle category.
    pub fn set_enabled(&mut self, category: SubtitleCategory, enabled: bool) {
        if enabled {
            self.enabled_categories.insert(category);
        } else {
            self.enabled_categories.remove(&category);
        }
    }

    /// Returns `true` if the given category is enabled.
    pub fn is_category_enabled(&self, category: SubtitleCategory) -> bool {
        self.enabled_categories.contains(&category)
    }

    /// Remove all active subtitles.
    pub fn clear(&mut self) {
        self.active.clear();
    }
}

impl Default for SubtitleManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// High Contrast Theme
// ---------------------------------------------------------------------------

/// A high-contrast theme that overrides default UI colors.
///
/// Games can use the built-in presets or register custom themes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HighContrastTheme {
    /// Human-readable theme name.
    pub name: String,
    /// Panel / window background color.
    pub background: Color,
    /// Default text color.
    pub foreground: Color,
    /// Accent color for highlights and focus indicators.
    pub accent: Color,
    /// Color for interactive elements (buttons, links).
    pub interactive: Color,
    /// Hover / focus color for interactive elements.
    pub interactive_hover: Color,
    /// Border width multiplier for visibility (in logical pixels).
    pub border_width: f32,
    /// Whether to draw a dark outline behind text for readability.
    pub text_shadow: bool,
}

impl HighContrastTheme {
    /// Built-in "White on Black" theme.
    pub fn white_on_black() -> Self {
        Self {
            name: "White on Black".into(),
            background: Color::BLACK,
            foreground: Color::WHITE,
            accent: Color::new(0.0, 1.0, 1.0, 1.0),     // cyan
            interactive: Color::new(1.0, 1.0, 0.0, 1.0),  // yellow
            interactive_hover: Color::new(1.0, 0.65, 0.0, 1.0), // orange
            border_width: 3.0,
            text_shadow: true,
        }
    }

    /// Built-in "Black on White" theme.
    pub fn black_on_white() -> Self {
        Self {
            name: "Black on White".into(),
            background: Color::WHITE,
            foreground: Color::BLACK,
            accent: Color::new(0.0, 0.0, 0.8, 1.0),     // dark blue
            interactive: Color::new(0.0, 0.5, 0.0, 1.0),  // dark green
            interactive_hover: Color::new(0.0, 0.3, 0.8, 1.0), // blue
            border_width: 3.0,
            text_shadow: false,
        }
    }

    /// Built-in "Yellow on Blue" theme (popular for low vision).
    pub fn yellow_on_blue() -> Self {
        Self {
            name: "Yellow on Blue".into(),
            background: Color::new(0.0, 0.0, 0.5, 1.0),  // dark blue
            foreground: Color::new(1.0, 1.0, 0.0, 1.0),   // yellow
            accent: Color::WHITE,
            interactive: Color::new(0.0, 1.0, 0.0, 1.0),  // green
            interactive_hover: Color::new(0.5, 1.0, 0.5, 1.0), // light green
            border_width: 3.0,
            text_shadow: true,
        }
    }

    /// Look up a built-in theme by name (case-insensitive).
    pub fn builtin(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "white on black" | "white_on_black" => Some(Self::white_on_black()),
            "black on white" | "black_on_white" => Some(Self::black_on_white()),
            "yellow on blue" | "yellow_on_blue" => Some(Self::yellow_on_blue()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Text Scale Settings
// ---------------------------------------------------------------------------

/// Global text scale factors applied on top of per-font pixel sizes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextScaleSettings {
    /// Global scale factor for all text (default 1.0, range 0.5..3.0).
    pub global_scale: f32,
    /// Separate scale for UI text.
    pub ui_scale: f32,
    /// Separate scale for subtitles.
    pub subtitle_scale: f32,
}

impl Default for TextScaleSettings {
    fn default() -> Self {
        Self {
            global_scale: 1.0,
            ui_scale: 1.0,
            subtitle_scale: 1.0,
        }
    }
}

impl TextScaleSettings {
    /// Clamp all scales to valid range (0.5..3.0).
    pub fn clamped(&self) -> Self {
        Self {
            global_scale: self.global_scale.clamp(0.5, 3.0),
            ui_scale: self.ui_scale.clamp(0.5, 3.0),
            subtitle_scale: self.subtitle_scale.clamp(0.5, 3.0),
        }
    }

    /// Effective subtitle scale (global * subtitle).
    pub fn effective_subtitle_scale(&self) -> f32 {
        (self.global_scale * self.subtitle_scale).clamp(0.5, 9.0)
    }

    /// Effective UI scale (global * ui).
    pub fn effective_ui_scale(&self) -> f32 {
        (self.global_scale * self.ui_scale).clamp(0.5, 9.0)
    }
}

// ---------------------------------------------------------------------------
// Input Assistance Settings
// ---------------------------------------------------------------------------

/// Input assistance settings for accessibility.
///
/// Provides sticky keys, hold-to-activate delays, toggle-mode conversion,
/// and configurable key-repeat behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputAssistSettings {
    /// Actions that use sticky-key mode: a press toggles the action on,
    /// the next press toggles it off.
    pub sticky_keys: HashSet<String>,
    /// Actions that require being held for a duration (in seconds) before
    /// they activate.
    pub hold_to_activate: HashMap<String, f32>,
    /// Actions that are normally hold-to-use but are converted to toggle mode.
    pub toggle_mode: HashSet<String>,
    /// Seconds before key repeat starts when an input is held (default 0.5).
    pub repeat_delay: f32,
    /// Repeats per second while held (default 10.0).
    pub repeat_rate: f32,
}

impl Default for InputAssistSettings {
    fn default() -> Self {
        Self {
            sticky_keys: HashSet::new(),
            hold_to_activate: HashMap::new(),
            toggle_mode: HashSet::new(),
            repeat_delay: 0.5,
            repeat_rate: 10.0,
        }
    }
}

impl InputAssistSettings {
    /// Process raw input through assistance filters.
    ///
    /// - `action`: the action identifier being queried.
    /// - `raw_pressed`: whether the raw input is currently pressed.
    /// - `held_duration`: how long the input has been continuously held (seconds).
    /// - `dt`: frame delta time.
    ///
    /// Returns `true` if the action should be considered active this frame.
    pub fn filter(
        &self,
        action: &str,
        raw_pressed: bool,
        held_duration: f32,
        dt: f32,
    ) -> bool {
        // Hold-to-activate: require held for `duration` before activating.
        if let Some(&required) = self.hold_to_activate.get(action) {
            if raw_pressed && held_duration < required {
                return false;
            }
        }

        // Key repeat logic: after repeat_delay, fire at repeat_rate Hz.
        if raw_pressed && held_duration > self.repeat_delay && self.repeat_rate > 0.0 {
            let repeat_interval = 1.0 / self.repeat_rate;
            let time_since_delay = held_duration - self.repeat_delay;
            let prev_time = time_since_delay - dt;
            // Fire if we crossed a repeat boundary this frame.
            if prev_time < 0.0 {
                return true;
            }
            let current_count = (time_since_delay / repeat_interval) as u32;
            let prev_count = (prev_time / repeat_interval) as u32;
            if current_count > prev_count {
                return true;
            }
        }

        raw_pressed
    }
}

// ---------------------------------------------------------------------------
// Screen Shake Reduction
// ---------------------------------------------------------------------------

/// Controls how screen shake is applied for motion-sensitive players.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShakeSettings {
    /// Global multiplier for screen shake intensity. 0.0 = disabled.
    pub intensity_multiplier: f32,
    /// Maximum pixel displacement cap.
    pub max_displacement: f32,
    /// Replace shake with a brief screen flash instead (for motion-sensitive
    /// players).
    pub flash_instead: bool,
}

impl Default for ShakeSettings {
    fn default() -> Self {
        Self {
            intensity_multiplier: 1.0,
            max_displacement: 16.0,
            flash_instead: false,
        }
    }
}

impl ShakeSettings {
    /// Apply shake settings to a raw displacement value, returning the
    /// effective displacement.
    ///
    /// Returns `0.0` if `flash_instead` is true (caller should trigger a
    /// flash effect instead).
    pub fn apply(&self, raw_displacement: f32) -> f32 {
        if self.flash_instead {
            return 0.0;
        }
        (raw_displacement * self.intensity_multiplier).min(self.max_displacement)
    }
}

// ---------------------------------------------------------------------------
// AccessibilityConfig
// ---------------------------------------------------------------------------

/// Central accessibility configuration, saved per-user.
///
/// Serializes to RON for persistence in the user's save directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessibilityConfig {
    /// Active colorblind correction mode.
    pub color_blind_mode: ColorBlindMode,
    /// Colorblind correction strength (0.0..1.0).
    pub colorblind_strength: f32,
    /// Screen-shake intensity multiplier (0.0 = disabled, 1.0 = full).
    pub screen_shake_intensity: f32,
    /// Whether subtitles are enabled globally.
    pub subtitle_enabled: bool,
    /// Font size for subtitles (in logical pixels).
    pub subtitle_font_size: f32,
    /// Whether high-contrast UI mode is active.
    pub high_contrast_mode: bool,
    /// Name of the active high-contrast theme (if any).
    pub high_contrast_theme: Option<String>,
    /// Whether input remapping is enabled.
    pub input_remapping_enabled: bool,
    /// Text scaling settings.
    pub text_scale: TextScaleSettings,
    /// Input assistance settings.
    pub input_assist: InputAssistSettings,
    /// Screen shake settings.
    pub shake: ShakeSettings,
    /// Which subtitle categories are enabled.
    pub subtitle_categories: Vec<SubtitleCategory>,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            color_blind_mode: ColorBlindMode::None,
            colorblind_strength: 1.0,
            screen_shake_intensity: 1.0,
            subtitle_enabled: false,
            subtitle_font_size: 24.0,
            high_contrast_mode: false,
            high_contrast_theme: None,
            input_remapping_enabled: false,
            text_scale: TextScaleSettings::default(),
            input_assist: InputAssistSettings::default(),
            shake: ShakeSettings::default(),
            subtitle_categories: vec![
                SubtitleCategory::Music,
                SubtitleCategory::SoundEffect,
                SubtitleCategory::Voice,
                SubtitleCategory::Ambient,
            ],
        }
    }
}

impl AccessibilityConfig {
    /// Save configuration to a RON file.
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let ron = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        std::fs::write(path, ron)
    }

    /// Load configuration from a RON file.
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let contents = std::fs::read_to_string(path)?;
        ron::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// AccessibilityManager
// ---------------------------------------------------------------------------

/// Top-level manager that ties together all accessibility subsystems.
///
/// Holds the user config, the colorblind filter, the subtitle manager,
/// and the active high-contrast theme.  Game code interacts with this
/// struct to query and apply accessibility features each frame.
pub struct AccessibilityManager {
    /// User-facing configuration (serialisable).
    pub config: AccessibilityConfig,
    /// CPU-side color filter derived from the current config.
    filter: ColorBlindFilter,
    /// Subtitle manager.
    pub subtitles: SubtitleManager,
    /// The currently active high-contrast theme (resolved from config).
    active_theme: Option<HighContrastTheme>,
    /// Registry of custom high-contrast themes.
    custom_themes: HashMap<String, HighContrastTheme>,
}

impl AccessibilityManager {
    /// Create a new manager with default settings.
    pub fn new() -> Self {
        let config = AccessibilityConfig::default();
        let filter = ColorBlindFilter::new(config.color_blind_mode);
        Self {
            config,
            filter,
            subtitles: SubtitleManager::new(),
            active_theme: None,
            custom_themes: HashMap::new(),
        }
    }

    /// Create a manager from an existing config.
    pub fn from_config(config: AccessibilityConfig) -> Self {
        let filter = ColorBlindFilter::with_strength(
            config.color_blind_mode,
            config.colorblind_strength,
        );
        let active_theme = config
            .high_contrast_theme
            .as_deref()
            .and_then(HighContrastTheme::builtin);
        Self {
            config,
            filter,
            subtitles: SubtitleManager::new(),
            active_theme,
            custom_themes: HashMap::new(),
        }
    }

    /// Apply the current colorblind filter to a color.
    pub fn apply_color_filter(&self, color: Color) -> Color {
        self.filter.remap_color(color)
    }

    /// Check whether a specific accessibility feature is enabled.
    pub fn is_feature_enabled(&self, feature: AccessibilityFeature) -> bool {
        match feature {
            AccessibilityFeature::ColorBlindFilter => {
                self.config.color_blind_mode != ColorBlindMode::None
            }
            AccessibilityFeature::HighContrast => self.config.high_contrast_mode,
            AccessibilityFeature::Subtitles => self.config.subtitle_enabled,
            AccessibilityFeature::InputRemapping => self.config.input_remapping_enabled,
            AccessibilityFeature::ReducedScreenShake => self.config.screen_shake_intensity < 1.0,
        }
    }

    /// Update the colorblind mode and rebuild the internal filter.
    pub fn set_color_blind_mode(&mut self, mode: ColorBlindMode) {
        self.config.color_blind_mode = mode;
        self.filter = ColorBlindFilter::with_strength(mode, self.config.colorblind_strength);
    }

    /// Update the colorblind correction strength (0.0..1.0).
    pub fn set_colorblind_strength(&mut self, strength: f32) {
        self.config.colorblind_strength = strength.clamp(0.0, 1.0);
        self.filter = ColorBlindFilter::with_strength(
            self.config.color_blind_mode,
            self.config.colorblind_strength,
        );
    }

    /// Get the effective screen-shake multiplier (clamped to 0.0 .. 1.0).
    pub fn screen_shake_multiplier(&self) -> f32 {
        self.config.screen_shake_intensity.clamp(0.0, 1.0)
    }

    /// Get the active high-contrast theme, if any.
    pub fn active_theme(&self) -> Option<&HighContrastTheme> {
        self.active_theme.as_ref()
    }

    /// Set the active high-contrast theme by name.
    ///
    /// Looks up built-in themes first, then custom-registered themes.
    /// Pass `None` to disable.
    pub fn set_high_contrast_theme(&mut self, name: Option<&str>) {
        match name {
            Some(n) => {
                self.config.high_contrast_theme = Some(n.to_string());
                self.config.high_contrast_mode = true;
                self.active_theme = HighContrastTheme::builtin(n)
                    .or_else(|| self.custom_themes.get(n).cloned());
            }
            None => {
                self.config.high_contrast_theme = None;
                self.config.high_contrast_mode = false;
                self.active_theme = None;
            }
        }
    }

    /// Register a custom high-contrast theme.
    pub fn register_theme(&mut self, theme: HighContrastTheme) {
        self.custom_themes.insert(theme.name.clone(), theme);
    }

    /// Apply raw screen-shake displacement through the shake settings.
    pub fn apply_shake(&self, raw_displacement: f32) -> f32 {
        self.config.shake.apply(raw_displacement)
    }

    /// Returns `true` if shake should be replaced with a flash effect.
    pub fn should_flash_instead_of_shake(&self) -> bool {
        self.config.shake.flash_instead
    }

    /// Get text scale settings.
    pub fn text_scale(&self) -> &TextScaleSettings {
        &self.config.text_scale
    }

    /// Get input assistance settings.
    pub fn input_assist(&self) -> &InputAssistSettings {
        &self.config.input_assist
    }

    /// Tick per-frame systems (subtitles, etc.).
    pub fn update(&mut self, dt: f32) {
        if self.config.subtitle_enabled {
            self.subtitles.update(dt);
        }
    }
}

impl Default for AccessibilityManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Feature enum (for is_feature_enabled queries)
// ---------------------------------------------------------------------------

/// Named accessibility features that can be queried via
/// [`AccessibilityManager::is_feature_enabled`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccessibilityFeature {
    /// Colorblind correction filter is active.
    ColorBlindFilter,
    /// High-contrast UI theme is active.
    HighContrast,
    /// Subtitles are shown for audio cues.
    Subtitles,
    /// Input remapping is turned on.
    InputRemapping,
    /// Screen shake is reduced below the default intensity.
    ReducedScreenShake,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    // -- ColorBlindFilter tests ---------------------------------------------

    #[test]
    fn color_filter_none_is_identity() {
        let filter = ColorBlindFilter::new(ColorBlindMode::None);
        let c = Color::new(0.5, 0.3, 0.8, 0.9);
        let out = filter.remap_color(c);
        assert_eq!(out, c);
    }

    #[test]
    fn color_filter_zero_strength_is_identity() {
        let filter = ColorBlindFilter::with_strength(ColorBlindMode::Deuteranopia, 0.0);
        let c = Color::RED;
        let out = filter.remap_color(c);
        assert_eq!(out, c);
    }

    #[test]
    fn deuteranopia_remaps_pure_red() {
        let filter = ColorBlindFilter::new(ColorBlindMode::Deuteranopia);
        let out = filter.remap_color(Color::RED);
        // Deuteranopia matrix: r' = 0.625*1 + 0.375*0 = 0.625
        //                      g' = 0.700*1 + 0.300*0 = 0.700
        //                      b' = 0.0
        assert!(approx_eq(out.r, 0.625));
        assert!(approx_eq(out.g, 0.700));
        assert!(approx_eq(out.b, 0.000));
        assert!(approx_eq(out.a, 1.0));
    }

    #[test]
    fn protanopia_remaps_pure_green() {
        let filter = ColorBlindFilter::new(ColorBlindMode::Protanopia);
        let out = filter.remap_color(Color::GREEN);
        // Protanopia matrix on (0,1,0):
        //   r' = 0.567*0 + 0.433*1 = 0.433
        //   g' = 0.558*0 + 0.442*1 = 0.442
        //   b' = 0.000*0 + 0.242*1 = 0.242
        assert!(approx_eq(out.r, 0.433));
        assert!(approx_eq(out.g, 0.442));
        assert!(approx_eq(out.b, 0.242));
    }

    #[test]
    fn tritanopia_remaps_pure_blue() {
        let filter = ColorBlindFilter::new(ColorBlindMode::Tritanopia);
        let out = filter.remap_color(Color::BLUE);
        // Tritanopia matrix on (0,0,1):
        //   r' = 0.0
        //   g' = 0.567
        //   b' = 0.525
        assert!(approx_eq(out.r, 0.0));
        assert!(approx_eq(out.g, 0.567));
        assert!(approx_eq(out.b, 0.525));
    }

    #[test]
    fn achromatopsia_produces_grayscale() {
        let filter = ColorBlindFilter::new(ColorBlindMode::Achromatopsia);
        let c = Color::new(1.0, 0.0, 0.0, 1.0); // pure red
        let out = filter.remap_color(c);
        // Luminance of pure red = 0.2126
        assert!(approx_eq(out.r, 0.2126));
        assert!(approx_eq(out.g, 0.2126));
        assert!(approx_eq(out.b, 0.2126));
        // All channels equal => grayscale
        assert!(approx_eq(out.r, out.g));
        assert!(approx_eq(out.g, out.b));
    }

    #[test]
    fn half_strength_blends_with_original() {
        let filter = ColorBlindFilter::with_strength(ColorBlindMode::Deuteranopia, 0.5);
        let out = filter.remap_color(Color::RED);
        // Full correction: (0.625, 0.700, 0.0)
        // Half blend with original (1.0, 0.0, 0.0):
        //   r = 0.625*0.5 + 1.0*0.5 = 0.8125
        //   g = 0.700*0.5 + 0.0*0.5 = 0.35
        //   b = 0.0
        assert!(approx_eq(out.r, 0.8125));
        assert!(approx_eq(out.g, 0.35));
        assert!(approx_eq(out.b, 0.0));
    }

    #[test]
    fn alpha_is_preserved() {
        let filter = ColorBlindFilter::new(ColorBlindMode::Deuteranopia);
        let c = Color::new(1.0, 0.0, 0.0, 0.42);
        let out = filter.remap_color(c);
        assert!(approx_eq(out.a, 0.42));
    }

    // -- SubtitleManager tests ----------------------------------------------

    #[test]
    fn subtitle_push_and_expire() {
        let mut mgr = SubtitleManager::new();
        mgr.push("Hello!", None, SubtitleCategory::Voice, None, 2.0);
        assert_eq!(mgr.active().len(), 1);

        mgr.update(1.5);
        assert_eq!(mgr.active().len(), 1);

        mgr.update(1.0); // 2.5 total > 2.0 duration
        assert_eq!(mgr.active().len(), 0);
    }

    #[test]
    fn subtitle_disabled_category_ignored() {
        let mut mgr = SubtitleManager::new();
        mgr.set_enabled(SubtitleCategory::Music, false);
        mgr.push("[ambient music]", None, SubtitleCategory::Music, None, 5.0);
        assert_eq!(mgr.active().len(), 0);
    }

    #[test]
    fn subtitle_max_visible() {
        let mut mgr = SubtitleManager::new();
        for i in 0..5 {
            mgr.push(
                format!("Line {i}"),
                None,
                SubtitleCategory::Voice,
                None,
                10.0,
            );
        }
        // 5 active, but only MAX_VISIBLE (3) returned
        assert_eq!(mgr.active().len(), SubtitleManager::MAX_VISIBLE);
        // Newest entries take priority (last 3)
        assert_eq!(mgr.active()[0].text, "Line 2");
        assert_eq!(mgr.active()[2].text, "Line 4");
    }

    // -- AccessibilityManager tests -----------------------------------------

    #[test]
    fn manager_feature_queries() {
        let mut mgr = AccessibilityManager::new();
        assert!(!mgr.is_feature_enabled(AccessibilityFeature::ColorBlindFilter));
        assert!(!mgr.is_feature_enabled(AccessibilityFeature::ReducedScreenShake));

        mgr.set_color_blind_mode(ColorBlindMode::Protanopia);
        assert!(mgr.is_feature_enabled(AccessibilityFeature::ColorBlindFilter));

        mgr.config.screen_shake_intensity = 0.5;
        assert!(mgr.is_feature_enabled(AccessibilityFeature::ReducedScreenShake));
    }

    #[test]
    fn manager_apply_color_filter_uses_active_mode() {
        let mut mgr = AccessibilityManager::new();
        let red = Color::RED;

        // No filter => identity
        let out = mgr.apply_color_filter(red);
        assert_eq!(out, red);

        // Switch to deuteranopia
        mgr.set_color_blind_mode(ColorBlindMode::Deuteranopia);
        let out = mgr.apply_color_filter(red);
        assert!(approx_eq(out.r, 0.625));
        assert!(approx_eq(out.g, 0.700));
    }

    #[test]
    fn config_default_values() {
        let cfg = AccessibilityConfig::default();
        assert_eq!(cfg.color_blind_mode, ColorBlindMode::None);
        assert!(approx_eq(cfg.colorblind_strength, 1.0));
        assert!(approx_eq(cfg.screen_shake_intensity, 1.0));
        assert!(!cfg.subtitle_enabled);
        assert!(approx_eq(cfg.subtitle_font_size, 24.0));
        assert!(!cfg.high_contrast_mode);
        assert!(cfg.high_contrast_theme.is_none());
        assert!(!cfg.input_remapping_enabled);
        assert!(approx_eq(cfg.text_scale.global_scale, 1.0));
        assert!(approx_eq(cfg.input_assist.repeat_delay, 0.5));
        assert!(approx_eq(cfg.shake.intensity_multiplier, 1.0));
    }

    // -- HighContrastTheme tests ------------------------------------------

    #[test]
    fn builtin_themes_exist() {
        assert!(HighContrastTheme::builtin("white on black").is_some());
        assert!(HighContrastTheme::builtin("black_on_white").is_some());
        assert!(HighContrastTheme::builtin("Yellow on Blue").is_some());
        assert!(HighContrastTheme::builtin("nonexistent").is_none());
    }

    // -- TextScaleSettings tests ------------------------------------------

    #[test]
    fn text_scale_clamping() {
        let ts = TextScaleSettings {
            global_scale: 5.0,
            ui_scale: 0.1,
            subtitle_scale: 2.0,
        };
        let clamped = ts.clamped();
        assert!(approx_eq(clamped.global_scale, 3.0));
        assert!(approx_eq(clamped.ui_scale, 0.5));
        assert!(approx_eq(clamped.subtitle_scale, 2.0));
    }

    // -- InputAssistSettings tests ----------------------------------------

    #[test]
    fn input_assist_hold_to_activate() {
        let mut assist = InputAssistSettings::default();
        assist
            .hold_to_activate
            .insert("heavy_attack".into(), 0.5);

        // Not held long enough
        assert!(!assist.filter("heavy_attack", true, 0.3, 0.016));
        // Held long enough
        assert!(assist.filter("heavy_attack", true, 0.6, 0.016));
        // Not pressed at all
        assert!(!assist.filter("heavy_attack", false, 0.0, 0.016));
    }

    // -- ShakeSettings tests ----------------------------------------------

    #[test]
    fn shake_settings_apply() {
        let shake = ShakeSettings {
            intensity_multiplier: 0.5,
            max_displacement: 8.0,
            flash_instead: false,
        };
        assert!(approx_eq(shake.apply(10.0), 5.0));
        assert!(approx_eq(shake.apply(20.0), 8.0)); // capped
    }

    #[test]
    fn shake_flash_instead() {
        let shake = ShakeSettings {
            intensity_multiplier: 1.0,
            max_displacement: 16.0,
            flash_instead: true,
        };
        assert!(approx_eq(shake.apply(10.0), 0.0));
    }

    // -- AccessibilityManager theme tests ---------------------------------

    #[test]
    fn manager_set_high_contrast_theme() {
        let mut mgr = AccessibilityManager::new();
        assert!(mgr.active_theme().is_none());

        mgr.set_high_contrast_theme(Some("white on black"));
        assert!(mgr.active_theme().is_some());
        assert!(mgr.config.high_contrast_mode);

        mgr.set_high_contrast_theme(None);
        assert!(mgr.active_theme().is_none());
        assert!(!mgr.config.high_contrast_mode);
    }

    #[test]
    fn manager_custom_theme() {
        let mut mgr = AccessibilityManager::new();
        let custom = HighContrastTheme {
            name: "My Theme".into(),
            background: Color::BLACK,
            foreground: Color::WHITE,
            accent: Color::RED,
            interactive: Color::GREEN,
            interactive_hover: Color::BLUE,
            border_width: 4.0,
            text_shadow: false,
        };
        mgr.register_theme(custom);
        mgr.set_high_contrast_theme(Some("My Theme"));
        assert!(mgr.active_theme().is_some());
        assert_eq!(mgr.active_theme().unwrap().name, "My Theme");
    }
}
