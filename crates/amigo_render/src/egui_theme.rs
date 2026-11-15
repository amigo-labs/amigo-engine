//! The shared amigo theme as egui visuals and style (ADR-0014).
//!
//! [`crate::theme`] holds the palettes and sizes amigo-labs/website generates;
//! this module turns one palette into [`egui::Visuals`] and an [`egui::Style`]
//! and applies them to an egui context. It styles the editor's chrome only,
//! never what a game draws. Gated behind the `editor` feature.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

use crate::theme::{self, Palette};

/// Which palette of the theme the editor shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    /// Light text on dark surfaces, the default.
    #[default]
    Dark,
    /// Dark text on light surfaces.
    Light,
}

impl ThemeMode {
    /// The generated palette of this mode.
    pub fn palette(self) -> &'static Palette {
        match self {
            Self::Dark => &theme::DARK,
            Self::Light => &theme::LIGHT,
        }
    }

    /// The egui theme this mode's style is stored under.
    pub fn egui_theme(self) -> egui::Theme {
        match self {
            Self::Dark => egui::Theme::Dark,
            Self::Light => egui::Theme::Light,
        }
    }
}

/// A palette colour (`0xRRGGBB`) as an opaque egui colour.
pub const fn color(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// A theme radius as an egui corner radius.
fn radius(px: f32) -> CornerRadius {
    CornerRadius::same(px as u8)
}

/// The egui visuals of one mode, every colour taken from its palette.
///
/// Starts from egui's own dark or light visuals, so what the theme does not
/// name (shadows, stroke widths, animation) keeps egui's value.
pub fn visuals(mode: ThemeMode) -> egui::Visuals {
    let p = mode.palette();
    let mut v = match mode {
        ThemeMode::Dark => egui::Visuals::dark(),
        ThemeMode::Light => egui::Visuals::light(),
    };
    let text = color(p.text);
    let border = Stroke::new(1.0, color(p.border));

    // Surfaces.
    v.panel_fill = color(p.surface);
    v.window_fill = color(p.surface);
    v.window_stroke = border;
    v.extreme_bg_color = color(p.bg);
    v.text_edit_bg_color = None;
    v.faint_bg_color = color(p.surface_raised);
    v.code_bg_color = color(p.surface_raised);

    // Text.
    v.override_text_color = None;
    v.weak_text_color = Some(color(p.text_muted));
    v.hyperlink_color = color(p.accent_ink);
    v.warn_fg_color = color(p.warning_ink);
    v.error_fg_color = color(p.danger_ink);
    v.text_cursor.stroke.color = text;
    v.ime_composition.active_underline_stroke.color = text;
    v.ime_composition.inactive_underline_stroke.color = color(p.text_muted);

    // Selection: an accent tint behind text that keeps its colour.
    v.selection.bg_fill = color(p.accent_subtle);
    v.selection.stroke = Stroke::new(1.0, text);

    // Radii.
    v.window_corner_radius = radius(theme::RADIUS_LG);
    v.menu_corner_radius = radius(theme::RADIUS_MD);

    // Widgets. Stroke widths stay egui's; the colours are the palette's.
    let w = &mut v.widgets;
    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.fg_stroke.color = text;
        state.corner_radius = radius(theme::RADIUS_SM);
    }
    // Labels, separators and frames around groups.
    w.noninteractive.bg_fill = color(p.surface);
    w.noninteractive.weak_bg_fill = color(p.surface);
    w.noninteractive.bg_stroke = border;
    // At rest.
    w.inactive.bg_fill = color(p.surface_raised);
    w.inactive.weak_bg_fill = color(p.surface_raised);
    w.inactive.bg_stroke = border;
    // Under the pointer.
    w.hovered.bg_fill = color(p.hover);
    w.hovered.weak_bg_fill = color(p.hover);
    w.hovered.bg_stroke.color = color(p.border_strong);
    // Pressed or holding the keyboard focus: egui draws both with this state,
    // so its outline is the focus colour.
    w.active.bg_fill = color(p.active);
    w.active.weak_bg_fill = color(p.active);
    w.active.bg_stroke.color = color(p.focus);
    // A menu button whose menu shows.
    w.open.bg_fill = color(p.active);
    w.open.weak_bg_fill = color(p.active);
    w.open.bg_stroke = border;

    v
}

/// The egui style of one mode: [`visuals`] plus the theme's text sizes,
/// control height and spacing.
pub fn style(mode: ThemeMode) -> egui::Style {
    let mut style = egui::Style {
        visuals: visuals(mode),
        ..egui::Style::default()
    };
    let font = |text: theme::TextStyle| {
        let family = if text.mono {
            FontFamily::Monospace
        } else {
            FontFamily::Proportional
        };
        FontId::new(text.size, family)
    };
    style.text_styles = [
        (TextStyle::Heading, font(theme::TEXT_TITLE)),
        (TextStyle::Body, font(theme::TEXT_BODY)),
        (TextStyle::Button, font(theme::TEXT_BODY)),
        (TextStyle::Small, font(theme::TEXT_CAPTION)),
        (TextStyle::Monospace, font(theme::TEXT_CODE)),
    ]
    .into();
    style.spacing.interact_size.y = theme::CONTROL_SM;
    style.spacing.item_spacing = egui::Vec2::splat(theme::SPACE_2);
    style
}

/// Makes `mode` the look of `ctx`: stores its style and switches egui to it.
///
/// The engine calls this once when the context is created and again when the
/// user picks a theme; a game that sets its own visuals afterwards keeps them.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    ctx.set_style_of(mode.egui_theme(), style(mode));
    ctx.set_theme(mode.egui_theme());
}

/// Whether `ctx` currently shows the style of `mode`: its visuals, text
/// sizes and spacing. (Two `egui::Style`s never compare equal as a whole:
/// each carries its own number formatter.)
pub fn shows(ctx: &egui::Context, mode: ThemeMode) -> bool {
    let shown = ctx.global_style();
    let themed = style(mode);
    shown.visuals == themed.visuals
        && shown.text_styles == themed.text_styles
        && shown.spacing == themed.spacing
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [ThemeMode; 2] = [ThemeMode::Dark, ThemeMode::Light];

    /// Every colour of a palette, as egui prints it.
    fn palette_colours(p: &Palette) -> Vec<String> {
        [
            p.bg,
            p.surface,
            p.surface_raised,
            p.hover,
            p.active,
            p.border,
            p.border_strong,
            p.text,
            p.text_secondary,
            p.text_muted,
            p.on_accent,
            p.accent,
            p.accent_ink,
            p.accent_solid,
            p.accent_subtle,
            p.focus,
            p.success,
            p.success_ink,
            p.success_subtle,
            p.warning,
            p.warning_ink,
            p.warning_subtle,
            p.danger,
            p.danger_ink,
            p.danger_subtle,
            p.danger_solid,
        ]
        .iter()
        .map(|&rgb| format!("{:?}", color(rgb)))
        .collect()
    }

    /// Every colour in a `Debug` print (egui prints `#RR_GG_BB_AA`).
    fn printed_colours(debug: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = debug;
        while let Some(at) = rest.find('#') {
            let candidate = rest.get(at..at + 12).unwrap_or("");
            let bytes = candidate.as_bytes();
            let is_colour = bytes.len() == 12
                && bytes.iter().enumerate().all(|(i, b)| match i {
                    0 => *b == b'#',
                    3 | 6 | 9 => *b == b'_',
                    _ => b.is_ascii_hexdigit(),
                });
            if is_colour {
                found.push(candidate.to_string());
            }
            rest = &rest[at + 1..];
        }
        found
    }

    #[test]
    fn color_splits_the_channels() {
        assert_eq!(color(0x12_34_56), Color32::from_rgb(0x12, 0x34, 0x56));
    }

    // AC-03
    #[test]
    fn visuals_take_the_named_colours_from_the_palette() {
        for mode in MODES {
            let p = mode.palette();
            let v = visuals(mode);
            assert_eq!(v.window_fill, color(p.surface), "{mode:?} window");
            assert_eq!(v.panel_fill, color(p.surface), "{mode:?} panel");
            assert_eq!(v.extreme_bg_color, color(p.bg), "{mode:?} behind panels");
            assert_eq!(v.window_stroke.color, color(p.border), "{mode:?}");
            assert_eq!(
                v.widgets.noninteractive.bg_stroke.color,
                color(p.border),
                "{mode:?} separators"
            );
            assert_eq!(v.text_color(), color(p.text), "{mode:?} text");
            assert_eq!(v.weak_text_color(), color(p.text_muted), "{mode:?} weak");
            assert_eq!(v.hyperlink_color, color(p.accent_ink), "{mode:?} link");
            assert_eq!(v.warn_fg_color, color(p.warning_ink), "{mode:?} warning");
            assert_eq!(v.error_fg_color, color(p.danger_ink), "{mode:?} error");
            assert_eq!(v.dark_mode, mode == ThemeMode::Dark);
        }
    }

    // AC-03: "colours all come from the palette". Shadows are the one
    // exception: translucent black, which the theme module does not carry.
    #[test]
    fn visuals_hold_no_colour_outside_the_palette() {
        for mode in MODES {
            let allowed = palette_colours(mode.palette());
            let v = visuals(mode);
            let shadows = [v.window_shadow.color, v.popup_shadow.color].map(|c| format!("{c:?}"));
            let printed = printed_colours(&format!("{v:?}"));
            assert!(printed.len() > 30, "the scan found colours: {printed:?}");
            for colour in printed {
                assert!(
                    allowed.contains(&colour) || shadows.contains(&colour),
                    "{mode:?}: {colour} is not a palette colour"
                );
            }
        }
    }

    // AC-04
    #[test]
    fn widget_states_come_from_the_palette() {
        for mode in MODES {
            let p = mode.palette();
            let w = visuals(mode).widgets;
            for (name, state, fill) in [
                ("rest", w.inactive, p.surface_raised),
                ("hovered", w.hovered, p.hover),
                ("pressed", w.active, p.active),
                ("open", w.open, p.active),
            ] {
                assert_eq!(state.bg_fill, color(fill), "{mode:?} {name} fill");
                assert_eq!(state.weak_bg_fill, color(fill), "{mode:?} {name} button");
                assert_eq!(state.fg_stroke.color, color(p.text), "{mode:?} {name} text");
            }
            assert_eq!(w.inactive.bg_stroke, Stroke::new(1.0, color(p.border)));
            assert_eq!(w.noninteractive.fg_stroke.color, color(p.text));
        }
    }

    // AC-05
    #[test]
    fn selection_is_the_accent_tint_and_focus_the_focus_colour() {
        for mode in MODES {
            let p = mode.palette();
            let v = visuals(mode);
            assert_eq!(v.selection.bg_fill, color(p.accent_subtle), "{mode:?}");
            assert_eq!(v.selection.stroke.color, color(p.text), "{mode:?}");
            // egui outlines a widget that holds the keyboard focus with the
            // `active` state's stroke.
            assert_eq!(v.widgets.active.bg_stroke.color, color(p.focus), "{mode:?}");
            assert!(v.widgets.active.bg_stroke.width >= 1.0);
        }
    }

    // AC-05
    #[test]
    fn no_default_blue_is_left() {
        let blues = [
            Color32::from_rgb(0, 92, 128),
            Color32::from_rgb(90, 170, 255),
            Color32::from_rgb(144, 209, 255),
        ]
        .map(|c| format!("{c:?}"));
        // The scan sees these colours in egui's own visuals …
        let defaults = format!("{:?}{:?}", egui::Visuals::dark(), egui::Visuals::light());
        for blue in &blues {
            assert!(defaults.contains(blue), "egui's defaults hold {blue}");
        }
        // … and in neither mode of the theme.
        for mode in MODES {
            let printed = format!("{:?}", style(mode));
            for blue in &blues {
                assert!(!printed.contains(blue), "{mode:?} still holds {blue}");
            }
        }
    }

    // AC-06
    #[test]
    fn corner_radii_are_the_themes() {
        for mode in MODES {
            let v = visuals(mode);
            let w = &v.widgets;
            for state in [w.noninteractive, w.inactive, w.hovered, w.active, w.open] {
                assert_eq!(state.corner_radius, CornerRadius::same(6));
                assert_eq!(f32::from(state.corner_radius.nw), theme::RADIUS_SM);
            }
            assert_eq!(f32::from(v.menu_corner_radius.nw), theme::RADIUS_MD);
            assert_eq!(v.menu_corner_radius, CornerRadius::same(8));
            assert_eq!(f32::from(v.window_corner_radius.nw), theme::RADIUS_LG);
            assert_eq!(v.window_corner_radius, CornerRadius::same(12));
        }
    }

    // AC-07
    #[test]
    fn style_uses_the_themes_sizes() {
        for mode in MODES {
            let s = style(mode);
            let size = |text_style: TextStyle| s.text_styles[&text_style].size;
            assert_eq!(size(TextStyle::Heading), theme::TEXT_TITLE.size);
            assert_eq!(size(TextStyle::Body), theme::TEXT_BODY.size);
            assert_eq!(size(TextStyle::Button), theme::TEXT_BODY.size);
            assert_eq!(size(TextStyle::Small), theme::TEXT_CAPTION.size);
            assert_eq!(size(TextStyle::Monospace), theme::TEXT_CODE.size);
            assert_eq!(
                s.text_styles[&TextStyle::Monospace].family,
                FontFamily::Monospace
            );
            assert_eq!(
                s.text_styles[&TextStyle::Body].family,
                FontFamily::Proportional
            );
            assert_eq!(s.spacing.interact_size.y, theme::CONTROL_SM);
            assert_eq!(s.spacing.item_spacing, egui::vec2(8.0, 8.0));
            assert_eq!(s.spacing.item_spacing.x, theme::SPACE_2);
            assert_eq!(s.visuals, visuals(mode));
        }
    }

    /// WCAG 2 relative luminance of a `0xRRGGBB` colour.
    fn luminance(rgb: u32) -> f64 {
        let channel = |shift: u32| {
            let c = f64::from((rgb >> shift) & 0xff) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    /// WCAG 2 contrast ratio of two colours.
    fn contrast(a: u32, b: u32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn contrast_of_black_on_white_is_21() {
        assert!((contrast(0x00_00_00, 0xff_ff_ff) - 21.0).abs() < 1e-9);
        assert!((contrast(0x77_77_77, 0x77_77_77) - 1.0).abs() < 1e-9);
    }

    // AC-08
    #[test]
    fn text_and_outlines_are_legible() {
        for mode in MODES {
            let p = mode.palette();
            let texts = [
                ("text", p.text),
                ("text_muted", p.text_muted),
                ("accent_ink", p.accent_ink),
                ("warning_ink", p.warning_ink),
                ("danger_ink", p.danger_ink),
            ];
            let grounds = [("surface", p.surface), ("surface_raised", p.surface_raised)];
            for (text_name, text) in texts {
                for (ground_name, ground) in grounds {
                    let ratio = contrast(text, ground);
                    assert!(
                        ratio >= 4.5,
                        "{mode:?}: {text_name} on {ground_name} is {ratio:.2} : 1"
                    );
                }
            }
            let ratio = contrast(p.text, p.accent_subtle);
            assert!(ratio >= 4.5, "{mode:?}: text on accent_subtle {ratio:.2}");
            for (name, outline) in [("focus", p.focus), ("border_strong", p.border_strong)] {
                let ratio = contrast(outline, p.surface);
                assert!(ratio >= 3.0, "{mode:?}: {name} on surface {ratio:.2} : 1");
            }
        }
    }

    // AC-10: one call switches the whole context.
    #[test]
    fn apply_switches_the_context() {
        let ctx = egui::Context::default();
        apply(&ctx, ThemeMode::Light);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        assert!(shows(&ctx, ThemeMode::Light));
        apply(&ctx, ThemeMode::Dark);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert!(shows(&ctx, ThemeMode::Dark));
        assert!(!shows(&ctx, ThemeMode::Light));
        assert!(!shows(&egui::Context::default(), ThemeMode::Dark));
    }

    #[test]
    fn dark_is_the_default_mode() {
        assert_eq!(ThemeMode::default(), ThemeMode::Dark);
        assert_eq!(ThemeMode::Dark.palette(), &theme::DARK);
        assert_eq!(ThemeMode::Light.palette(), &theme::LIGHT);
    }
}
