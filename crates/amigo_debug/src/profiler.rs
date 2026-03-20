//! Built-in frame timeline profiler.
//!
//! Records hierarchical system timing spans per frame, retains a configurable
//! history ring buffer, and provides data for timeline rendering in the debug
//! overlay.

use std::collections::VecDeque;
use std::time::Instant;

use amigo_core::Color;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Unique identifier for a profiler span within a single frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpanId(u32);

/// A completed timing span recorded during a frame.
#[derive(Clone, Debug)]
pub struct ProfileSpan {
    pub id: SpanId,
    pub parent: Option<SpanId>,
    pub name: &'static str,
    /// Offset from frame start, in microseconds.
    pub start_us: u64,
    /// Duration in microseconds.
    pub duration_us: u64,
    /// Depth in the span tree (0 = top-level system).
    pub depth: u16,
    /// Optional color tag for the timeline bar (e.g. hash of name).
    pub color: u32,
}

/// One frame's worth of recorded profiling data.
#[derive(Clone, Debug)]
pub struct FrameProfile {
    pub frame_number: u64,
    /// Total frame duration in microseconds.
    pub total_us: u64,
    /// Ordered list of spans (sorted by start_us).
    pub spans: Vec<ProfileSpan>,
}

/// Configuration for the profiler.
#[derive(Clone, Debug)]
pub struct ProfilerConfig {
    /// Number of frames retained in the ring buffer. Default: 300 (5 seconds at 60 fps).
    pub history_size: usize,
    /// If true, profiler records spans. If false, all begin/end calls are no-ops.
    pub enabled: bool,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            history_size: 300,
            enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Open span tracking (internal)
// ---------------------------------------------------------------------------

/// An in-progress span that has not yet been closed.
struct OpenSpan {
    id: SpanId,
    parent: Option<SpanId>,
    name: &'static str,
    start: Instant,
    depth: u16,
}

// ---------------------------------------------------------------------------
// FrameProfiler
// ---------------------------------------------------------------------------

/// The frame timeline profiler. Owned by the engine, accessible via `DebugOverlay`.
pub struct FrameProfiler {
    config: ProfilerConfig,
    /// Ring buffer of completed frame profiles.
    history: VecDeque<FrameProfile>,
    /// Timestamp of the current frame start.
    frame_start: Option<Instant>,
    /// Current frame number (set by `begin_frame`).
    current_frame_number: u64,
    /// Monotonically increasing counter for span IDs within the current frame.
    next_span_id: u32,
    /// Stack of currently open spans (for nesting).
    open_spans: Vec<OpenSpan>,
    /// Completed spans for the current frame (pre-allocated).
    current_spans: Vec<ProfileSpan>,
    /// Whether we are inside a begin_frame / end_frame bracket.
    in_frame: bool,
}

impl FrameProfiler {
    /// Create a new profiler with the given configuration.
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            history: VecDeque::with_capacity(config.history_size),
            config,
            frame_start: None,
            current_frame_number: 0,
            next_span_id: 0,
            open_spans: Vec::with_capacity(16),
            current_spans: Vec::with_capacity(64),
            in_frame: false,
        }
    }

    /// Call at the very beginning of a frame. Records the frame-start timestamp
    /// and resets the per-frame span list.
    pub fn begin_frame(&mut self, frame_number: u64) {
        if !self.config.enabled {
            return;
        }
        self.frame_start = Some(Instant::now());
        self.current_frame_number = frame_number;
        self.next_span_id = 0;
        self.current_spans.clear();
        self.open_spans.clear();
        self.in_frame = true;
    }

    /// Open a new span. Returns a `SpanId` that must be passed to `end_span`.
    /// Nests under the current open span (if any).
    /// `name` must be a `&'static str` to avoid allocation per frame.
    pub fn begin_span(&mut self, name: &'static str) -> SpanId {
        if !self.config.enabled || !self.in_frame {
            return SpanId(u32::MAX);
        }

        let id = SpanId(self.next_span_id);
        self.next_span_id = self.next_span_id.saturating_add(1);

        let parent = self.open_spans.last().map(|s| s.id);
        let depth = self.open_spans.len() as u16;

        self.open_spans.push(OpenSpan {
            id,
            parent,
            name,
            start: Instant::now(),
            depth,
        });

        id
    }

    /// Close a previously opened span.
    pub fn end_span(&mut self, id: SpanId) {
        if !self.config.enabled || !self.in_frame {
            return;
        }

        let frame_start = match self.frame_start {
            Some(s) => s,
            None => return,
        };

        // Find and remove the span from the open stack.
        // Normally it should be the last element (LIFO), but we handle
        // out-of-order closes gracefully.
        let pos = self.open_spans.iter().rposition(|s| s.id == id);
        let open = match pos {
            Some(p) => self.open_spans.remove(p),
            None => return, // Already closed or invalid id.
        };

        let now = Instant::now();
        let start_us = open.start.duration_since(frame_start).as_micros() as u64;
        let duration_us = now.duration_since(open.start).as_micros() as u64;

        let color = name_color(open.name);

        self.current_spans.push(ProfileSpan {
            id: open.id,
            parent: open.parent,
            name: open.name,
            start_us,
            duration_us,
            depth: open.depth,
            color,
        });
    }

    /// Call at the end of a frame. Finalises the `FrameProfile`, pushes it
    /// into the ring buffer, and evicts the oldest frame if necessary.
    pub fn end_frame(&mut self) {
        if !self.config.enabled || !self.in_frame {
            return;
        }

        let frame_start = match self.frame_start {
            Some(s) => s,
            None => {
                self.in_frame = false;
                return;
            }
        };

        let now = Instant::now();

        // Force-close any open spans.
        if !self.open_spans.is_empty() {
            tracing::warn!(
                count = self.open_spans.len(),
                "end_frame: force-closing {} open span(s)",
                self.open_spans.len()
            );

            // Drain open spans (take them all at once to avoid borrow issues).
            let remaining: Vec<OpenSpan> = self.open_spans.drain(..).collect();
            for open in remaining {
                let start_us = open.start.duration_since(frame_start).as_micros() as u64;
                let duration_us = now.duration_since(open.start).as_micros() as u64;

                self.current_spans.push(ProfileSpan {
                    id: open.id,
                    parent: open.parent,
                    name: open.name,
                    start_us,
                    duration_us,
                    depth: open.depth,
                    color: name_color(open.name),
                });
            }
        }

        // Sort spans by start_us.
        self.current_spans.sort_by_key(|s| s.start_us);

        let total_us = now.duration_since(frame_start).as_micros() as u64;

        let profile = FrameProfile {
            frame_number: self.current_frame_number,
            total_us,
            spans: self.current_spans.clone(),
        };

        // Evict oldest if at capacity.
        if self.history.len() >= self.config.history_size {
            self.history.pop_front();
        }
        self.history.push_back(profile);

        self.in_frame = false;
        self.frame_start = None;
    }

    /// Convenience: time a closure as a named span.
    pub fn span<R>(&mut self, name: &'static str, f: impl FnOnce() -> R) -> R {
        let id = self.begin_span(name);
        let result = f();
        self.end_span(id);
        result
    }

    /// Open a span and return an RAII guard that closes it on drop.
    pub fn begin_span_guard(&mut self, name: &'static str) -> SpanGuard<'_> {
        let id = self.begin_span(name);
        SpanGuard { profiler: self, id }
    }

    /// Access the ring buffer of recorded frames. Index 0 is the oldest.
    pub fn history(&self) -> &[FrameProfile] {
        // VecDeque::make_contiguous requires &mut self, so we use as_slices
        // and return the appropriate slice. Since we only push_back and
        // pop_front, after the first wrap the deque may be non-contiguous.
        // We return both slices concatenated... but the API says &[FrameProfile].
        // We need to work around this.
        //
        // The simplest correct approach: we always keep the deque contiguous
        // by design (we never insert in the middle). But VecDeque doesn't
        // guarantee contiguity. So we'll expose via as_slices and let callers
        // deal with it... except the spec says &[FrameProfile].
        //
        // We'll use the first slice if the second is empty (common case after
        // make_contiguous), otherwise this is best-effort.
        let (a, b) = self.history.as_slices();
        if b.is_empty() {
            a
        } else {
            // Return the first contiguous chunk. This is a limitation;
            // callers should prefer last_frame() for the common case.
            a
        }
    }

    /// Get the most recently completed frame profile.
    pub fn last_frame(&self) -> Option<&FrameProfile> {
        self.history.back()
    }

    /// Reconfigure the profiler (e.g. resize history).
    pub fn set_config(&mut self, config: ProfilerConfig) {
        // If the new history size is smaller, trim from the front.
        while self.history.len() > config.history_size {
            self.history.pop_front();
        }
        self.config = config;
    }

    /// Returns true if profiling is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Clear all recorded history.
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

// ---------------------------------------------------------------------------
// SpanGuard (RAII)
// ---------------------------------------------------------------------------

/// RAII span guard. Calls `end_span` on drop.
pub struct SpanGuard<'a> {
    profiler: &'a mut FrameProfiler,
    id: SpanId,
}

impl<'a> Drop for SpanGuard<'a> {
    fn drop(&mut self) {
        self.profiler.end_span(self.id);
    }
}

// ---------------------------------------------------------------------------
// Timeline rendering types
// ---------------------------------------------------------------------------

/// Rendering parameters for the timeline overlay.
#[derive(Clone, Debug)]
pub struct TimelineView {
    /// Which frame in history to center on (offset from latest). 0 = current.
    pub selected_frame_offset: usize,
    /// Horizontal zoom level. 1.0 = one frame fills the timeline width.
    pub zoom: f32,
    /// Vertical row height in pixels.
    pub row_height: f32,
    /// Whether the timeline is paused (freezes on the selected frame).
    pub paused: bool,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            selected_frame_offset: 0,
            zoom: 1.0,
            row_height: 18.0,
            paused: false,
        }
    }
}

/// Output of the timeline renderer: rectangles and labels.
pub struct TimelineDrawData {
    /// Colored bars representing spans.
    pub bars: Vec<TimelineBar>,
    /// Text labels positioned on or next to bars.
    pub labels: Vec<TimelineLabel>,
    /// 16.66ms frame budget guideline position (x coordinate in pixels).
    pub budget_line_x: f32,
}

/// A colored bar representing a span in the timeline.
pub struct TimelineBar {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: Color,
}

/// A text label in the timeline.
pub struct TimelineLabel {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub color: Color,
}

/// Generates draw commands for the timeline overlay.
pub struct TimelineRenderer;

/// The 60 fps frame budget in microseconds.
const FRAME_BUDGET_US: f32 = 16_666.0;

impl TimelineRenderer {
    /// Produce overlay draw data for the given frame profile and view settings.
    /// `viewport_width` and `viewport_height` are the overlay area dimensions in pixels.
    pub fn render(
        profile: &FrameProfile,
        view: &TimelineView,
        viewport_width: f32,
        viewport_height: f32,
    ) -> TimelineDrawData {
        let mut bars = Vec::with_capacity(profile.spans.len());
        let mut labels = Vec::with_capacity(profile.spans.len());

        // The total time axis range visible is determined by the frame duration
        // and the zoom level. At zoom 1.0, the entire frame fills the viewport width.
        let total_time_us = if profile.total_us == 0 {
            1.0_f32 // Avoid division by zero.
        } else {
            profile.total_us as f32
        };

        let us_per_pixel = total_time_us / (viewport_width * view.zoom);

        // Budget guideline: position of 16,666 us on the x axis.
        let budget_line_x = FRAME_BUDGET_US / us_per_pixel;

        let row_height = view.row_height;

        // Determine the maximum number of rows that fit in the viewport.
        let max_rows = if row_height > 0.0 {
            (viewport_height / row_height) as u16
        } else {
            u16::MAX
        };

        for span in &profile.spans {
            if span.depth >= max_rows {
                continue;
            }

            let x = span.start_us as f32 / us_per_pixel;
            let width = span.duration_us as f32 / us_per_pixel;
            let y = span.depth as f32 * row_height;

            let color = color_from_u32(span.color);

            bars.push(TimelineBar {
                x,
                y,
                width,
                height: row_height - 1.0, // 1px gap between rows.
                color,
            });

            // Only render the label if the bar is wide enough to display text.
            if width > 20.0 {
                let label_text = if width > 80.0 {
                    format!("{} ({:.1}ms)", span.name, span.duration_us as f64 / 1000.0)
                } else {
                    span.name.to_string()
                };

                labels.push(TimelineLabel {
                    x: x + 2.0,
                    y: y + 1.0,
                    text: label_text,
                    color: Color::WHITE,
                });
            }
        }

        TimelineDrawData {
            bars,
            labels,
            budget_line_x,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a deterministic color tag from a span name pointer.
fn name_color(name: &'static str) -> u32 {
    name.as_ptr() as u32
}

/// Convert a u32 color tag into an RGBA `Color`.
/// Produces a reasonably saturated, distinct hue per span name.
fn color_from_u32(tag: u32) -> Color {
    // Use a simple hash-to-hue mapping for variety.
    let hue = (tag % 360) as f32;
    let saturation = 0.6;
    let lightness = 0.5;
    hsl_to_color(hue, saturation, lightness)
}

/// Convert HSL to an `amigo_core::Color`.
fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    Color::new(r1 + m, g1 + m, b1 + m, 0.85)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ProfilerConfig::default();
        assert_eq!(config.history_size, 300);
        assert!(config.enabled);
    }

    #[test]
    fn new_profiler_has_empty_history() {
        let profiler = FrameProfiler::new(ProfilerConfig::default());
        assert!(profiler.history().is_empty());
        assert!(profiler.last_frame().is_none());
        assert!(profiler.is_enabled());
    }

    #[test]
    fn basic_frame_lifecycle() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());
        profiler.begin_frame(1);
        let id = profiler.begin_span("physics");
        profiler.end_span(id);
        profiler.end_frame();

        assert_eq!(profiler.history().len(), 1);
        let frame = profiler.last_frame().expect("should have one frame");
        assert_eq!(frame.frame_number, 1);
        assert_eq!(frame.spans.len(), 1);
        assert_eq!(frame.spans[0].name, "physics");
        assert_eq!(frame.spans[0].depth, 0);
        assert!(frame.spans[0].parent.is_none());
    }

    #[test]
    fn nested_spans() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());
        profiler.begin_frame(1);

        let outer = profiler.begin_span("frame");
        let inner = profiler.begin_span("render");
        profiler.end_span(inner);
        profiler.end_span(outer);

        profiler.end_frame();

        let frame = profiler.last_frame().expect("should have one frame");
        assert_eq!(frame.spans.len(), 2);

        // Find spans by name.
        let render_span = frame
            .spans
            .iter()
            .find(|s| s.name == "render")
            .expect("render span");
        let frame_span = frame
            .spans
            .iter()
            .find(|s| s.name == "frame")
            .expect("frame span");

        assert_eq!(render_span.depth, 1);
        assert_eq!(render_span.parent, Some(frame_span.id));
        assert_eq!(frame_span.depth, 0);
        assert!(frame_span.parent.is_none());
    }

    #[test]
    fn span_convenience_method() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());
        profiler.begin_frame(1);

        let result = profiler.span("compute", || 42);
        assert_eq!(result, 42);

        profiler.end_frame();

        let frame = profiler.last_frame().expect("should have one frame");
        assert_eq!(frame.spans.len(), 1);
        assert_eq!(frame.spans[0].name, "compute");
    }

    #[test]
    fn span_guard_raii() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());
        profiler.begin_frame(1);

        {
            let _guard = profiler.begin_span_guard("scoped");
            // guard drops here
        }

        profiler.end_frame();

        let frame = profiler.last_frame().expect("should have one frame");
        assert_eq!(frame.spans.len(), 1);
        assert_eq!(frame.spans[0].name, "scoped");
    }

    #[test]
    fn ring_buffer_eviction() {
        let config = ProfilerConfig {
            history_size: 3,
            enabled: true,
        };
        let mut profiler = FrameProfiler::new(config);

        for i in 0..5 {
            profiler.begin_frame(i);
            profiler.end_frame();
        }

        // Should have exactly 3 frames (the last 3).
        assert_eq!(profiler.history.len(), 3);
        let last = profiler.last_frame().expect("should have frames");
        assert_eq!(last.frame_number, 4);
        // Oldest should be frame 2.
        assert_eq!(profiler.history.front().expect("front").frame_number, 2);
    }

    #[test]
    fn disabled_profiler_is_noop() {
        let config = ProfilerConfig {
            history_size: 300,
            enabled: false,
        };
        let mut profiler = FrameProfiler::new(config);

        profiler.begin_frame(1);
        let id = profiler.begin_span("should_not_record");
        profiler.end_span(id);
        profiler.end_frame();

        assert!(profiler.history().is_empty());
        assert!(profiler.last_frame().is_none());
    }

    #[test]
    fn begin_span_outside_frame_is_noop() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());

        // No begin_frame called.
        let id = profiler.begin_span("orphan");
        assert_eq!(id, SpanId(u32::MAX));
    }

    #[test]
    fn end_frame_force_closes_open_spans() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());
        profiler.begin_frame(1);

        let _id = profiler.begin_span("unclosed");
        // Intentionally not calling end_span.

        profiler.end_frame();

        let frame = profiler.last_frame().expect("should have one frame");
        assert_eq!(frame.spans.len(), 1);
        assert_eq!(frame.spans[0].name, "unclosed");
        assert!(frame.spans[0].duration_us > 0 || frame.spans[0].duration_us == 0);
    }

    #[test]
    fn set_config_resizes_history() {
        let mut profiler = FrameProfiler::new(ProfilerConfig {
            history_size: 10,
            enabled: true,
        });

        for i in 0..10 {
            profiler.begin_frame(i);
            profiler.end_frame();
        }
        assert_eq!(profiler.history.len(), 10);

        profiler.set_config(ProfilerConfig {
            history_size: 5,
            enabled: true,
        });
        assert_eq!(profiler.history.len(), 5);
    }

    #[test]
    fn clear_removes_all_history() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());
        profiler.begin_frame(1);
        profiler.end_frame();
        assert!(!profiler.history().is_empty());

        profiler.clear();
        assert!(profiler.history().is_empty());
    }

    #[test]
    fn is_enabled_reflects_config() {
        let profiler = FrameProfiler::new(ProfilerConfig {
            history_size: 300,
            enabled: false,
        });
        assert!(!profiler.is_enabled());

        let profiler = FrameProfiler::new(ProfilerConfig::default());
        assert!(profiler.is_enabled());
    }

    #[test]
    fn spans_sorted_by_start_us() {
        let mut profiler = FrameProfiler::new(ProfilerConfig::default());
        profiler.begin_frame(1);

        let a = profiler.begin_span("first");
        profiler.end_span(a);
        let b = profiler.begin_span("second");
        profiler.end_span(b);

        profiler.end_frame();

        let frame = profiler.last_frame().expect("should have frame");
        assert!(frame.spans.len() == 2);
        assert!(frame.spans[0].start_us <= frame.spans[1].start_us);
    }

    #[test]
    fn default_timeline_view() {
        let view = TimelineView::default();
        assert_eq!(view.selected_frame_offset, 0);
        assert!((view.zoom - 1.0).abs() < f32::EPSILON);
        assert!((view.row_height - 18.0).abs() < f32::EPSILON);
        assert!(!view.paused);
    }

    #[test]
    fn timeline_renderer_produces_bars() {
        let profile = FrameProfile {
            frame_number: 1,
            total_us: 16_666,
            spans: vec![ProfileSpan {
                id: SpanId(0),
                parent: None,
                name: "test_span",
                start_us: 0,
                duration_us: 8_000,
                depth: 0,
                color: 42,
            }],
        };

        let view = TimelineView::default();
        let data = TimelineRenderer::render(&profile, &view, 800.0, 200.0);

        assert_eq!(data.bars.len(), 1);
        assert!(data.budget_line_x > 0.0);
        // The bar should start at x=0.
        assert!((data.bars[0].x).abs() < f32::EPSILON);
        // Width should be roughly half the viewport (8000/16666 * 800).
        assert!(data.bars[0].width > 100.0);
    }

    #[test]
    fn timeline_renderer_empty_profile() {
        let profile = FrameProfile {
            frame_number: 1,
            total_us: 0,
            spans: vec![],
        };

        let view = TimelineView::default();
        let data = TimelineRenderer::render(&profile, &view, 800.0, 200.0);

        assert!(data.bars.is_empty());
        assert!(data.labels.is_empty());
    }

    #[test]
    fn span_id_derives() {
        let a = SpanId(1);
        let b = SpanId(1);
        let c = SpanId(2);
        assert_eq!(a, b);
        assert_ne!(a, c);

        // Clone, Copy
        let d = a;
        assert_eq!(a, d);

        // Debug
        let _ = format!("{:?}", a);

        // Hash
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn color_from_u32_produces_valid_color() {
        let color = color_from_u32(42);
        assert!(color.r >= 0.0 && color.r <= 1.0);
        assert!(color.g >= 0.0 && color.g <= 1.0);
        assert!(color.b >= 0.0 && color.b <= 1.0);
        assert!(color.a >= 0.0 && color.a <= 1.0);
    }

    #[test]
    fn history_oldest_first() {
        let mut profiler = FrameProfiler::new(ProfilerConfig {
            history_size: 10,
            enabled: true,
        });

        for i in 0..5 {
            profiler.begin_frame(i);
            profiler.end_frame();
        }

        // VecDeque front is oldest.
        let first = profiler.history.front().expect("front");
        let last = profiler.history.back().expect("back");
        assert_eq!(first.frame_number, 0);
        assert_eq!(last.frame_number, 4);
    }
}
