use amigo_core::Color;
use std::collections::BTreeMap;
use std::time::Instant;

#[cfg(feature = "builtin_profiler")]
mod profiler;

#[cfg(feature = "builtin_profiler")]
pub use profiler::{
    FrameProfile, FrameProfiler, ProfileSpan, ProfilerConfig, SpanGuard, SpanId, TimelineBar,
    TimelineDrawData, TimelineLabel, TimelineRenderer, TimelineView,
};

/// Debug overlay state.
pub struct DebugOverlay {
    pub visible: bool,
    pub show_fps: bool,
    pub show_entity_count: bool,
    pub show_draw_calls: bool,
    pub show_memory: bool,
    pub show_systems: bool,
    // Visual debug toggles
    pub show_grid: bool,
    pub show_collision: bool,
    pub show_paths: bool,
    pub show_spawn_zones: bool,
    // F5–F8 debug overlays
    pub show_entity_ids: bool,
    pub show_tile_ids: bool,
    pub show_audio_debug: bool,
    pub show_network_debug: bool,

    // Stats
    fps: f64,
    frame_time_ms: f64,
    entity_count: usize,
    draw_calls: u32,
    frame_count: u64,
    fps_timer: f64,
    fps_frame_count: u32,

    // Per-system profiling
    system_timings: BTreeMap<String, SystemTiming>,
    active_measurement: Option<(String, Instant)>,

    // Built-in profiler integration
    #[cfg(feature = "builtin_profiler")]
    profiler: FrameProfiler,
    #[cfg(feature = "builtin_profiler")]
    active_profiler_span: Option<SpanId>,
    /// Whether the timeline view is visible (toggled via F9).
    #[cfg(feature = "builtin_profiler")]
    pub show_timeline: bool,
    /// Timeline view settings.
    #[cfg(feature = "builtin_profiler")]
    pub timeline_view: TimelineView,
}

/// Timing data for a single system.
#[derive(Clone, Debug)]
struct SystemTiming {
    last_ms: f64,
    avg_ms: f64,
    max_ms: f64,
    sample_count: u32,
}

impl SystemTiming {
    fn new() -> Self {
        Self {
            last_ms: 0.0,
            avg_ms: 0.0,
            max_ms: 0.0,
            sample_count: 0,
        }
    }

    fn record(&mut self, ms: f64) {
        self.last_ms = ms;
        self.sample_count += 1;
        // Exponential moving average
        let alpha = 0.1;
        self.avg_ms = if self.sample_count == 1 {
            ms
        } else {
            self.avg_ms * (1.0 - alpha) + ms * alpha
        };
        if ms > self.max_ms {
            self.max_ms = ms;
        }
    }
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            show_fps: true,
            show_entity_count: true,
            show_draw_calls: true,
            show_memory: false,
            show_systems: false,
            show_grid: false,
            show_collision: false,
            show_paths: false,
            show_spawn_zones: false,
            show_entity_ids: false,
            show_tile_ids: false,
            show_audio_debug: false,
            show_network_debug: false,
            fps: 0.0,
            frame_time_ms: 0.0,
            entity_count: 0,
            draw_calls: 0,
            frame_count: 0,
            fps_timer: 0.0,
            fps_frame_count: 0,
            system_timings: BTreeMap::new(),
            active_measurement: None,
            #[cfg(feature = "builtin_profiler")]
            profiler: FrameProfiler::new(ProfilerConfig::default()),
            #[cfg(feature = "builtin_profiler")]
            active_profiler_span: None,
            #[cfg(feature = "builtin_profiler")]
            show_timeline: false,
            #[cfg(feature = "builtin_profiler")]
            timeline_view: TimelineView::default(),
        }
    }

    pub fn update(&mut self, dt: f64, entity_count: usize, draw_calls: u32) {
        self.frame_count += 1;
        self.fps_frame_count += 1;
        self.fps_timer += dt;
        self.frame_time_ms = dt * 1000.0;
        self.entity_count = entity_count;
        self.draw_calls = draw_calls;

        if self.fps_timer >= 1.0 {
            self.fps = self.fps_frame_count as f64 / self.fps_timer;
            self.fps_frame_count = 0;
            self.fps_timer = 0.0;
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn fps(&self) -> f64 {
        self.fps
    }

    pub fn frame_time_ms(&self) -> f64 {
        self.frame_time_ms
    }

    pub fn entity_count(&self) -> usize {
        self.entity_count
    }

    pub fn draw_calls(&self) -> u32 {
        self.draw_calls
    }

    // -- Per-system profiling ------------------------------------------------

    /// Start timing a named system. Call before the system runs.
    ///
    /// When the `builtin_profiler` feature is enabled, this also opens a
    /// profiler span that feeds into the timeline view.
    pub fn begin_system(&mut self, name: &str) {
        self.active_measurement = Some((name.to_string(), Instant::now()));

        #[cfg(feature = "builtin_profiler")]
        {
            // SAFETY: system names in the engine are string literals, but the
            // public API accepts `&str`. We leak a tiny amount per unique name
            // to satisfy the `&'static str` requirement. In practice the set of
            // system names is small and fixed.
            let static_name: &'static str = leak_name(name);
            let span_id = self.profiler.begin_span(static_name);
            self.active_profiler_span = Some(span_id);
        }
    }

    /// End timing the current system. Call after the system runs.
    ///
    /// When the `builtin_profiler` feature is enabled, this also closes the
    /// corresponding profiler span.
    pub fn end_system(&mut self) {
        if let Some((name, start)) = self.active_measurement.take() {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            self.system_timings
                .entry(name)
                .or_insert_with(SystemTiming::new)
                .record(elapsed_ms);
        }

        #[cfg(feature = "builtin_profiler")]
        {
            if let Some(span_id) = self.active_profiler_span.take() {
                self.profiler.end_span(span_id);
            }
        }
    }

    /// Convenience: time a closure as a named system.
    pub fn time_system<F: FnOnce()>(&mut self, name: &str, f: F) {
        self.begin_system(name);
        f();
        self.end_system();
    }

    /// Reset max timings (useful periodically).
    pub fn reset_max_timings(&mut self) {
        for timing in self.system_timings.values_mut() {
            timing.max_ms = 0.0;
        }
    }

    /// Generate debug text lines for overlay display.
    pub fn overlay_lines(&self) -> Vec<(String, Color)> {
        let mut lines = Vec::new();
        if !self.visible {
            return lines;
        }

        if self.show_fps {
            let fps_color = if self.fps >= 55.0 {
                Color::GREEN
            } else if self.fps >= 30.0 {
                Color::YELLOW
            } else {
                Color::RED
            };
            lines.push((
                format!("FPS: {:.0} ({:.1}ms)", self.fps, self.frame_time_ms),
                fps_color,
            ));
        }
        if self.show_entity_count {
            lines.push((format!("Entities: {}", self.entity_count), Color::WHITE));
        }
        if self.show_draw_calls {
            lines.push((format!("Draw calls: {}", self.draw_calls), Color::WHITE));
        }
        if self.show_memory {
            lines.push((
                format!("Frame: {}", self.frame_count),
                Color::new(0.7, 0.7, 0.7, 1.0),
            ));
        }

        if self.show_systems && !self.system_timings.is_empty() {
            lines.push((
                "--- Systems ---".to_string(),
                Color::new(0.6, 0.8, 1.0, 1.0),
            ));
            for (name, timing) in &self.system_timings {
                let color = if timing.avg_ms > 2.0 {
                    Color::RED
                } else if timing.avg_ms > 0.5 {
                    Color::YELLOW
                } else {
                    Color::GREEN
                };
                lines.push((
                    format!(
                        "  {}: {:.2}ms (avg:{:.2} max:{:.2})",
                        name, timing.last_ms, timing.avg_ms, timing.max_ms
                    ),
                    color,
                ));
            }
        }

        lines
    }

    // -- Built-in profiler integration ---------------------------------------

    /// Begin a profiler frame. Call at the very start of the engine frame loop.
    ///
    /// This is a no-op when the `builtin_profiler` feature is not enabled.
    #[cfg(feature = "builtin_profiler")]
    pub fn begin_profiler_frame(&mut self, frame_number: u64) {
        self.profiler.begin_frame(frame_number);
    }

    /// End a profiler frame. Call at the very end of the engine frame loop.
    ///
    /// This is a no-op when the `builtin_profiler` feature is not enabled.
    #[cfg(feature = "builtin_profiler")]
    pub fn end_profiler_frame(&mut self) {
        self.profiler.end_frame();
    }

    /// Access the underlying `FrameProfiler`.
    #[cfg(feature = "builtin_profiler")]
    pub fn profiler(&self) -> &FrameProfiler {
        &self.profiler
    }

    /// Mutably access the underlying `FrameProfiler`.
    #[cfg(feature = "builtin_profiler")]
    pub fn profiler_mut(&mut self) -> &mut FrameProfiler {
        &mut self.profiler
    }

    /// Toggle the timeline view visibility.
    #[cfg(feature = "builtin_profiler")]
    pub fn toggle_timeline(&mut self) {
        self.show_timeline = !self.show_timeline;
    }

    /// Render timeline draw data for the currently selected frame.
    ///
    /// Returns `None` if the timeline is not visible, the profiler is disabled,
    /// or there is no frame data to display.
    #[cfg(feature = "builtin_profiler")]
    pub fn timeline_draw_data(
        &self,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<TimelineDrawData> {
        if !self.show_timeline {
            return None;
        }

        let history = self.profiler.history();
        if history.is_empty() {
            return None;
        }

        // Select the frame based on offset from latest.
        let index = if self.timeline_view.selected_frame_offset >= history.len() {
            0
        } else {
            history.len() - 1 - self.timeline_view.selected_frame_offset
        };

        let profile = &history[index];
        Some(TimelineRenderer::render(
            profile,
            &self.timeline_view,
            viewport_width,
            viewport_height,
        ))
    }
}

impl Default for DebugOverlay {
    fn default() -> Self {
        Self::new()
    }
}

/// Mark the end of a frame for Tracy profiling.
///
/// When the `tracy` feature is enabled, this calls Tracy's frame mark
/// which gives the profiler frame-by-frame timing data. When disabled,
/// this is a no-op.
#[inline]
pub fn frame_mark() {
    #[cfg(feature = "tracy")]
    tracy_client::Client::running()
        .expect("Tracy client not running")
        .frame_mark();
}

/// Check whether Tracy profiling is enabled at compile time.
pub fn tracy_enabled() -> bool {
    cfg!(feature = "tracy")
}

/// Initialize the tracing subscriber with env filter.
///
/// When the `tracy` feature is enabled, a Tracy profiler layer is added
/// alongside the console output. Connect with the Tracy profiler GUI to
/// see real-time spans, zones, and frame markers.
pub fn init_logging() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, Layer};

    let filter = EnvFilter::try_from_env("AMIGO_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_filter(filter);

    let registry = tracing_subscriber::registry().with(fmt_layer);

    #[cfg(feature = "tracy")]
    {
        let tracy_layer = tracing_tracy::TracyLayer::default();
        registry.with(tracy_layer).init();
    }

    #[cfg(not(feature = "tracy"))]
    {
        registry.init();
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Intern a system name as a `&'static str`.
///
/// Uses a global set to avoid leaking duplicate names. The number of unique
/// system names in a typical engine is small (< 100), so the memory cost is
/// negligible.
#[cfg(feature = "builtin_profiler")]
fn leak_name(name: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::Mutex;

    static INTERNED: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);

    let mut guard = match INTERNED.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    let set = guard.get_or_insert_with(HashSet::new);

    // Check if we already interned this name.
    for &interned in set.iter() {
        if interned == name {
            return interned;
        }
    }

    // Leak a new copy.
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_timing_records() {
        let mut overlay = DebugOverlay::new();
        overlay.visible = true;
        overlay.show_systems = true;

        overlay.time_system("physics", || {
            std::thread::sleep(std::time::Duration::from_millis(1));
        });

        assert!(overlay.system_timings.contains_key("physics"));
        let timing = &overlay.system_timings["physics"];
        assert!(timing.last_ms > 0.0);
    }

    #[test]
    fn fps_color_coding() {
        let mut overlay = DebugOverlay::new();
        overlay.visible = true;
        overlay.show_fps = true;

        // Simulate 60fps
        for _ in 0..60 {
            overlay.update(1.0 / 60.0, 0, 0);
        }
        let lines = overlay.overlay_lines();
        assert!(!lines.is_empty());
    }

    #[cfg(feature = "builtin_profiler")]
    #[test]
    fn overlay_profiler_integration() {
        let mut overlay = DebugOverlay::new();
        overlay.begin_profiler_frame(1);
        overlay.begin_system("test_system");
        overlay.end_system();
        overlay.end_profiler_frame();

        let frame = overlay.profiler().last_frame().expect("should have frame");
        assert_eq!(frame.frame_number, 1);
        assert_eq!(frame.spans.len(), 1);
        assert_eq!(frame.spans[0].name, "test_system");

        // SystemTiming should also be updated.
        assert!(overlay.system_timings.contains_key("test_system"));
    }

    #[cfg(feature = "builtin_profiler")]
    #[test]
    fn overlay_toggle_timeline() {
        let mut overlay = DebugOverlay::new();
        assert!(!overlay.show_timeline);
        overlay.toggle_timeline();
        assert!(overlay.show_timeline);
        overlay.toggle_timeline();
        assert!(!overlay.show_timeline);
    }

    #[cfg(feature = "builtin_profiler")]
    #[test]
    fn timeline_draw_data_returns_none_when_hidden() {
        let overlay = DebugOverlay::new();
        assert!(overlay.timeline_draw_data(800.0, 600.0).is_none());
    }

    #[cfg(feature = "builtin_profiler")]
    #[test]
    fn timeline_draw_data_returns_some_when_visible() {
        let mut overlay = DebugOverlay::new();
        overlay.show_timeline = true;

        overlay.begin_profiler_frame(1);
        overlay.begin_system("physics");
        overlay.end_system();
        overlay.end_profiler_frame();

        let data = overlay.timeline_draw_data(800.0, 600.0);
        assert!(data.is_some());
        let draw_data = data.expect("should have draw data");
        assert!(!draw_data.bars.is_empty());
    }
}
