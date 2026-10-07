//! Where the virtual-resolution image lands in the window.
//!
//! The scene renders into an offscreen target (see [`crate::blit`]); this
//! module decides the rectangle of the window that target is drawn into, and
//! maps window coordinates back into virtual coordinates for input. It is pure
//! arithmetic so it can be tested without a GPU.

use amigo_core::RenderVec2;

/// How the virtual resolution is scaled to the window (`[render] scale_mode`
/// in `amigo.toml`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScaleMode {
    /// Largest whole-number scale that fits, centred, with black bars around
    /// it. Every virtual pixel becomes an exact `n×n` block. When the window is
    /// smaller than the virtual resolution no whole scale fits, and this falls
    /// back to [`ScaleMode::Fit`].
    #[default]
    PixelPerfect,
    /// Largest scale that fits while keeping the aspect ratio, with bars on the
    /// sides or top and bottom. Non-integer scales make some virtual pixels one
    /// window pixel wider than others.
    Fit,
    /// Fill the whole window, distorting the aspect ratio when it differs.
    Stretch,
    /// Keep the configured virtual height and widen or narrow the virtual
    /// width to the window's aspect ratio. No bars, no distortion.
    /// Config value `"expand"`.
    Expand,
}

impl ScaleMode {
    /// Parse a `scale_mode` config value. `None` for anything unrecognised, so
    /// the caller can warn instead of silently picking a mode.
    pub fn from_str_config(s: &str) -> Option<Self> {
        match s {
            "pixel_perfect" | "integer" => Some(Self::PixelPerfect),
            "fit" | "letterbox" => Some(Self::Fit),
            "stretch" => Some(Self::Stretch),
            "expand" => Some(Self::Expand),
            _ => None,
        }
    }

    /// The virtual resolution for a window: the configured one, except under
    /// [`ScaleMode::Expand`], where the width becomes
    /// `round(virtual_height · window_w / window_h)`. Never zero.
    pub fn virtual_size_for(self, configured: (u32, u32), window: (u32, u32)) -> (u32, u32) {
        let (vw, vh) = (configured.0.max(1), configured.1.max(1));
        match self {
            ScaleMode::Expand => {
                let (ww, wh) = (window.0.max(1) as f64, window.1.max(1) as f64);
                (((vh as f64 * ww / wh).round() as u32).max(1), vh)
            }
            _ => (vw, vh),
        }
    }
}

/// What the game needs to lay out for the window it is in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportInfo {
    /// Window size in physical pixels.
    pub window_size: (u32, u32),
    /// Where the scene lands in the window.
    pub viewport: Viewport,
    /// Current virtual resolution (changes under [`ScaleMode::Expand`]).
    pub virtual_size: (f32, f32),
    /// Scene-target pixels per virtual pixel: 1.0 for pixel art, the
    /// viewport scale for raster art.
    pub render_scale: f32,
    /// The OS scale factor (DPI) of the window's monitor.
    pub scale_factor: f64,
}

impl ViewportInfo {
    /// The layout of a `window`-sized window showing a game configured for
    /// `configured` virtual pixels. Zero sizes count as 1, so no field is ever
    /// zero.
    pub fn compute(
        mode: ScaleMode,
        art_style: crate::ArtStyle,
        configured: (u32, u32),
        window: (u32, u32),
        scale_factor: f64,
    ) -> Self {
        let window = (window.0.max(1), window.1.max(1));
        let virtual_size = mode.virtual_size_for(configured, window);
        let viewport = Viewport::compute(mode, virtual_size, window);
        let render_scale = match art_style {
            crate::ArtStyle::RasterArt => {
                (viewport.width / virtual_size.0 as f32).max(f32::MIN_POSITIVE)
            }
            crate::ArtStyle::PixelArt | crate::ArtStyle::Hybrid => 1.0,
        };
        Self {
            window_size: window,
            viewport,
            virtual_size: (virtual_size.0 as f32, virtual_size.1 as f32),
            render_scale,
            scale_factor: if scale_factor.is_finite() && scale_factor > 0.0 {
                scale_factor
            } else {
                1.0
            },
        }
    }
}

/// The window-pixel rectangle the rendered scene is drawn into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Viewport {
    /// Compute the viewport for a window of `window` pixels showing a virtual
    /// resolution of `virtual_size`. Zero sizes are treated as 1 so the result
    /// is always a valid, non-empty rectangle.
    pub fn compute(mode: ScaleMode, virtual_size: (u32, u32), window: (u32, u32)) -> Self {
        let (vw, vh) = (virtual_size.0.max(1) as f32, virtual_size.1.max(1) as f32);
        let (ww, wh) = (window.0.max(1) as f32, window.1.max(1) as f32);

        let fit_scale = (ww / vw).min(wh / vh);
        let (width, height) = match mode {
            // Expand changes the virtual width to the window's shape, so the
            // image always covers the window.
            ScaleMode::Stretch | ScaleMode::Expand => (ww, wh),
            ScaleMode::PixelPerfect if fit_scale >= 1.0 => {
                let s = fit_scale.floor();
                (vw * s, vh * s)
            }
            // Fit, and PixelPerfect in a window smaller than the virtual size.
            _ => (
                (vw * fit_scale).round().clamp(1.0, ww),
                (vh * fit_scale).round().clamp(1.0, wh),
            ),
        };

        // Whole-pixel offsets: a half-pixel origin would make the nearest
        // sampler pick neighbouring texels unevenly across the image.
        Self {
            x: ((ww - width) / 2.0).floor(),
            y: ((wh - height) / 2.0).floor(),
            width,
            height,
        }
    }

    /// Map a window-pixel position into virtual coordinates. Positions on the
    /// letterbox bars map outside `0..virtual_size`; callers that need a
    /// position on the image can clamp.
    pub fn window_to_virtual(&self, pos: RenderVec2, virtual_size: (f32, f32)) -> RenderVec2 {
        RenderVec2::new(
            (pos.x - self.x) * virtual_size.0 / self.width,
            (pos.y - self.y) * virtual_size.1 / self.height,
        )
    }

    /// Whether a window-pixel position lies on the image rather than a bar.
    pub fn contains(&self, pos: RenderVec2) -> bool {
        pos.x >= self.x
            && pos.y >= self.y
            && pos.x < self.x + self.width
            && pos.y < self.y + self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_covers_the_window_and_widens_the_virtual_resolution() {
        assert_eq!(
            ScaleMode::from_str_config("expand"),
            Some(ScaleMode::Expand)
        );
        assert_eq!(
            vp(ScaleMode::Expand, (640, 360), (1000, 500)),
            Viewport {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 500.0
            }
        );
        assert_eq!(
            ScaleMode::Expand.virtual_size_for((640, 360), (1000, 500)),
            (720, 360)
        );
        // A portrait window narrows it.
        assert_eq!(
            ScaleMode::Expand.virtual_size_for((640, 360), (500, 1000)),
            (180, 360)
        );
        assert_eq!(
            ScaleMode::Fit.virtual_size_for((640, 360), (1000, 500)),
            (640, 360)
        );
    }

    #[test]
    fn viewport_info_never_reports_zero_sizes() {
        let info = ViewportInfo::compute(
            ScaleMode::Expand,
            crate::ArtStyle::RasterArt,
            (640, 360),
            (0, 0),
            f64::NAN,
        );
        assert!(info.window_size.0 > 0 && info.window_size.1 > 0);
        assert!(info.virtual_size.0 > 0.0 && info.virtual_size.1 > 0.0);
        assert!(info.render_scale > 0.0);
        assert_eq!(info.scale_factor, 1.0);
    }

    #[test]
    fn raster_art_reports_the_viewport_scale() {
        let raster = ViewportInfo::compute(
            ScaleMode::Expand,
            crate::ArtStyle::RasterArt,
            (640, 360),
            (1920, 1080),
            2.0,
        );
        assert_eq!(raster.virtual_size, (640.0, 360.0));
        assert_eq!(raster.render_scale, 3.0);
        let pixel = ViewportInfo::compute(
            ScaleMode::PixelPerfect,
            crate::ArtStyle::PixelArt,
            (640, 360),
            (1920, 1080),
            2.0,
        );
        assert_eq!(pixel.render_scale, 1.0);
    }

    fn vp(mode: ScaleMode, virt: (u32, u32), win: (u32, u32)) -> Viewport {
        Viewport::compute(mode, virt, win)
    }

    #[test]
    fn pixel_perfect_uses_the_largest_whole_scale_and_centres_it() {
        // 1280x720 / 320x180 is exactly 4.
        assert_eq!(
            vp(ScaleMode::PixelPerfect, (320, 180), (1280, 720)),
            Viewport {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0
            }
        );
        // 1366x768 / 320x180 = 4.27 x 4.27 -> 4, bars of 43 and 24 pixels.
        assert_eq!(
            vp(ScaleMode::PixelPerfect, (320, 180), (1366, 768)),
            Viewport {
                x: 43.0,
                y: 24.0,
                width: 1280.0,
                height: 720.0
            }
        );
        // A 16:10 window around a 16:9 game: width decides, bars top and bottom.
        assert_eq!(
            vp(ScaleMode::PixelPerfect, (480, 270), (1920, 1200)),
            Viewport {
                x: 0.0,
                y: 60.0,
                width: 1920.0,
                height: 1080.0
            }
        );
    }

    #[test]
    fn pixel_perfect_falls_back_to_fit_below_scale_one() {
        let small = vp(ScaleMode::PixelPerfect, (480, 270), (400, 300));
        assert_eq!(small, vp(ScaleMode::Fit, (480, 270), (400, 300)));
        assert!(small.width <= 400.0 && small.height <= 300.0);
    }

    #[test]
    fn fit_keeps_the_aspect_ratio() {
        let v = vp(ScaleMode::Fit, (320, 180), (1000, 1000));
        assert_eq!(v.width, 1000.0);
        assert_eq!(v.height, 563.0); // 180 * 3.125 = 562.5, rounded
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 218.0);
    }

    #[test]
    fn stretch_fills_the_window() {
        assert_eq!(
            vp(ScaleMode::Stretch, (320, 180), (1000, 1000)),
            Viewport {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 1000.0
            }
        );
    }

    #[test]
    fn zero_sizes_do_not_produce_an_empty_or_nan_viewport() {
        for mode in [ScaleMode::PixelPerfect, ScaleMode::Fit, ScaleMode::Stretch] {
            let v = vp(mode, (0, 0), (0, 0));
            assert!(v.width >= 1.0 && v.height >= 1.0, "{mode:?}: {v:?}");
            assert!(v.x.is_finite() && v.y.is_finite());
        }
    }

    #[test]
    fn window_positions_map_back_to_virtual_pixels() {
        let v = vp(ScaleMode::PixelPerfect, (320, 180), (1366, 768));
        let virt = (320.0, 180.0);
        // Top-left corner of the image is virtual (0, 0).
        assert_eq!(
            v.window_to_virtual(RenderVec2::new(43.0, 24.0), virt),
            RenderVec2::new(0.0, 0.0)
        );
        // Bottom-right corner is the virtual size.
        assert_eq!(
            v.window_to_virtual(RenderVec2::new(43.0 + 1280.0, 24.0 + 720.0), virt),
            RenderVec2::new(320.0, 180.0)
        );
        // One window pixel inside a 4x block is still the same virtual pixel.
        let p = v.window_to_virtual(RenderVec2::new(43.0 + 4.0 * 10.0 + 3.0, 24.0), virt);
        assert_eq!(p.x.floor(), 10.0);
        // The left bar maps to negative x.
        assert!(v.window_to_virtual(RenderVec2::new(0.0, 100.0), virt).x < 0.0);
        assert!(!v.contains(RenderVec2::new(0.0, 100.0)));
        assert!(v.contains(RenderVec2::new(100.0, 100.0)));
    }

    #[test]
    fn config_strings_parse_and_unknown_ones_do_not() {
        assert_eq!(
            ScaleMode::from_str_config("pixel_perfect"),
            Some(ScaleMode::PixelPerfect)
        );
        assert_eq!(ScaleMode::from_str_config("fit"), Some(ScaleMode::Fit));
        assert_eq!(
            ScaleMode::from_str_config("stretch"),
            Some(ScaleMode::Stretch)
        );
        assert_eq!(ScaleMode::from_str_config("pixelperfect"), None);
    }
}
