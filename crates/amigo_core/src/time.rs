/// Timing information for the game loop.
#[derive(Clone, Debug)]
pub struct TimeInfo {
    /// Fixed timestep duration in seconds (1/60).
    pub tick_duration: f64,
    /// Ticks per second (60).
    pub ticks_per_second: u32,
    /// Current simulation tick.
    pub tick: u64,
    /// Interpolation alpha for rendering between ticks (0.0 - 1.0).
    pub alpha: f32,
    /// Real elapsed time since engine start, in seconds.
    pub elapsed: f64,
    /// Delta time for the current frame (for rendering/UI, NOT simulation).
    pub dt: f32,
}

impl TimeInfo {
    pub const TICKS_PER_SECOND: u32 = 60;
    pub const TICK_DURATION: f64 = 1.0 / Self::TICKS_PER_SECOND as f64;

    pub fn new() -> Self {
        Self {
            tick_duration: Self::TICK_DURATION,
            ticks_per_second: Self::TICKS_PER_SECOND,
            tick: 0,
            alpha: 0.0,
            elapsed: 0.0,
            dt: 0.0,
        }
    }
}

impl Default for TimeInfo {
    fn default() -> Self {
        Self::new()
    }
}
