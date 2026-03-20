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
// AccessibilityConfig
// ---------------------------------------------------------------------------

/// Central accessibility configuration, saved per-user.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessibilityConfig {
    /// Active colorblind correction mode.
    pub color_blind_mode: ColorBlindMode,
    /// Screen-shake intensity multiplier (0.0 = disabled, 1.0 = full).
    pub screen_shake_intensity: f32,
    /// Whether subtitles are enabled globally.
    pub subtitle_enabled: bool,
    /// Font size for subtitles (in logical pixels).
    pub subtitle_font_size: f32,
    /// Whether high-contrast UI mode is active.
    pub high_contrast_mode: bool,
    /// Whether input remapping is enabled.
    pub input_remapping_enabled: bool,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            color_blind_mode: ColorBlindMode::None,
            screen_shake_intensity: 1.0,
            subtitle_enabled: false,
            subtitle_font_size: 24.0,
            high_contrast_mode: false,
            input_remapping_enabled: false,
        }
    }
}

// ---------------------------------------------------------------------------
// AccessibilityManager
// ---------------------------------------------------------------------------

/// Top-level manager that ties together all accessibility subsystems.
///
/// Holds the user config, the colorblind filter, and the subtitle manager.
/// Game code interacts with this struct to query and apply accessibility
/// features each frame.
pub struct AccessibilityManager {
    /// User-facing configuration (serialisable).
    pub config: AccessibilityConfig,
    /// CPU-side color filter derived from the current config.
    filter: ColorBlindFilter,
    /// Subtitle manager.
    pub subtitles: SubtitleManager,
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
        }
    }

    /// Create a manager from an existing config.
    pub fn from_config(config: AccessibilityConfig) -> Self {
        let filter = ColorBlindFilter::new(config.color_blind_mode);
        Self {
            config,
            filter,
            subtitles: SubtitleManager::new(),
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
        self.filter = ColorBlindFilter::new(mode);
    }

    /// Get the effective screen-shake multiplier (clamped to 0.0 .. 1.0).
    pub fn screen_shake_multiplier(&self) -> f32 {
        self.config.screen_shake_intensity.clamp(0.0, 1.0)
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
        assert!(approx_eq(cfg.screen_shake_intensity, 1.0));
        assert!(!cfg.subtitle_enabled);
        assert!(approx_eq(cfg.subtitle_font_size, 24.0));
        assert!(!cfg.high_contrast_mode);
        assert!(!cfg.input_remapping_enabled);
    }
}
