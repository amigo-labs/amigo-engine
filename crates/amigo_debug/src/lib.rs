use amigo_core::Color;

/// Debug overlay state.
pub struct DebugOverlay {
    pub visible: bool,
    pub show_fps: bool,
    pub show_entity_count: bool,
    pub show_draw_calls: bool,
    pub show_memory: bool,
    // Visual debug toggles
    pub show_grid: bool,
    pub show_collision: bool,
    pub show_paths: bool,
    pub show_spawn_zones: bool,

    // Stats
    fps: f64,
    frame_time_ms: f64,
    entity_count: usize,
    draw_calls: u32,
    frame_count: u64,
    fps_timer: f64,
    fps_frame_count: u32,
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            show_fps: true,
            show_entity_count: true,
            show_draw_calls: true,
            show_memory: false,
            show_grid: false,
            show_collision: false,
            show_paths: false,
            show_spawn_zones: false,
            fps: 0.0,
            frame_time_ms: 0.0,
            entity_count: 0,
            draw_calls: 0,
            frame_count: 0,
            fps_timer: 0.0,
            fps_frame_count: 0,
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

    /// Generate debug text lines for overlay display.
    pub fn overlay_lines(&self) -> Vec<(String, Color)> {
        let mut lines = Vec::new();
        if !self.visible {
            return lines;
        }

        if self.show_fps {
            lines.push((
                format!("FPS: {:.0} ({:.1}ms)", self.fps, self.frame_time_ms),
                Color::GREEN,
            ));
        }
        if self.show_entity_count {
            lines.push((
                format!("Entities: {}", self.entity_count),
                Color::WHITE,
            ));
        }
        if self.show_draw_calls {
            lines.push((
                format!("Draw calls: {}", self.draw_calls),
                Color::WHITE,
            ));
        }

        lines
    }
}

impl Default for DebugOverlay {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the tracing subscriber with env filter.
pub fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_env("AMIGO_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
