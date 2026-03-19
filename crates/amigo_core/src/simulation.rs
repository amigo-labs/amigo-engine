use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Simulation speed
// ---------------------------------------------------------------------------

/// Speed multiplier for the simulation tick.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SimSpeed {
    Paused,
    Normal,
    Fast,
    VeryFast,
    Ultra,
    Custom(f32),
}

impl SimSpeed {
    pub fn multiplier(self) -> f32 {
        match self {
            Self::Paused => 0.0,
            Self::Normal => 1.0,
            Self::Fast => 2.0,
            Self::VeryFast => 5.0,
            Self::Ultra => 10.0,
            Self::Custom(m) => m,
        }
    }

    /// Cycle to the next speed (Paused → Normal → Fast → VeryFast → Ultra → Paused).
    pub fn next(self) -> Self {
        match self {
            Self::Paused => Self::Normal,
            Self::Normal => Self::Fast,
            Self::Fast => Self::VeryFast,
            Self::VeryFast => Self::Ultra,
            Self::Ultra => Self::Paused,
            Self::Custom(_) => Self::Normal,
        }
    }
}

// ---------------------------------------------------------------------------
// SimSystem trait
// ---------------------------------------------------------------------------

/// A registered simulation system that ticks independently of rendering.
///
/// Systems run in priority order (lower = earlier) and can specify their
/// own tick interval to run less frequently than the base sim rate.
pub trait SimSystem {
    /// Lower priority runs first. Default: 100.
    fn priority(&self) -> u8 {
        100
    }

    /// Run every N sim-ticks. 1 = every tick. Default: 1.
    fn tick_interval(&self) -> u32 {
        1
    }

    /// Name for debug display.
    fn name(&self) -> &str;

    /// Perform one simulation step.
    fn update(&mut self, ctx: &mut SimContext);
}

/// Context passed to each SimSystem during its update.
pub struct SimContext {
    /// Current simulation tick.
    pub tick: u64,
    /// Sim tick delta in seconds (1 / sim_ticks_per_second).
    pub dt: f64,
    /// Simulation speed multiplier (for systems that want to adjust behavior).
    pub speed: f32,
}

// ---------------------------------------------------------------------------
// SimulationRunner
// ---------------------------------------------------------------------------

/// Manages simulation ticks decoupled from the render frame rate.
///
/// Designed for God Sim / Sandbox games where world-updates (agent AI,
/// liquid flow, tile updates, day/night) need a fixed tick rate independent
/// of FPS, with speed control (pause, 1x, 2x, 5x, 10x).
pub struct SimulationRunner {
    /// Base sim ticks per real-time second (before speed multiplier).
    pub ticks_per_second: u32,
    /// Current speed.
    pub speed: SimSpeed,
    /// Current simulation tick counter.
    pub tick: u64,
    /// Accumulated real-time (seconds) not yet consumed by sim ticks.
    accumulator: f64,
    /// Registered systems, sorted by priority.
    systems: Vec<Box<dyn SimSystem>>,
    /// Whether systems need re-sorting after an add.
    dirty: bool,
    /// Maximum sim ticks per frame (prevents spiral of death).
    pub max_ticks_per_frame: u32,
}

impl SimulationRunner {
    /// Create a new simulation runner with the given tick rate.
    pub fn new(ticks_per_second: u32) -> Self {
        Self {
            ticks_per_second,
            speed: SimSpeed::Normal,
            tick: 0,
            accumulator: 0.0,
            systems: Vec::new(),
            dirty: false,
            max_ticks_per_frame: 10,
        }
    }

    /// Register a simulation system.
    pub fn add_system(&mut self, system: Box<dyn SimSystem>) {
        self.systems.push(system);
        self.dirty = true;
    }

    /// Set simulation speed.
    pub fn set_speed(&mut self, speed: SimSpeed) {
        self.speed = speed;
    }

    /// Cycle to the next speed preset.
    pub fn toggle_speed(&mut self) {
        self.speed = self.speed.next();
    }

    /// Advance the simulation by the given real-time delta (seconds).
    /// Returns the number of sim ticks executed this frame.
    pub fn advance(&mut self, real_dt: f64) -> u32 {
        if self.speed.multiplier() == 0.0 {
            return 0;
        }

        // Sort systems by priority if needed.
        if self.dirty {
            self.systems.sort_by_key(|s| s.priority());
            self.dirty = false;
        }

        let tick_duration = 1.0 / self.ticks_per_second as f64;
        self.accumulator += real_dt * self.speed.multiplier() as f64;

        let mut ticks_this_frame = 0u32;

        while self.accumulator >= tick_duration && ticks_this_frame < self.max_ticks_per_frame {
            self.accumulator -= tick_duration;
            self.tick += 1;
            ticks_this_frame += 1;

            let mut ctx = SimContext {
                tick: self.tick,
                dt: tick_duration,
                speed: self.speed.multiplier(),
            };

            for system in &mut self.systems {
                let interval = system.tick_interval() as u64;
                if interval <= 1 || self.tick.is_multiple_of(interval) {
                    system.update(&mut ctx);
                }
            }
        }

        // Clamp accumulator to prevent runaway when very behind.
        if self.accumulator > tick_duration * self.max_ticks_per_frame as f64 {
            self.accumulator = 0.0;
        }

        ticks_this_frame
    }

    /// Reset simulation state.
    pub fn reset(&mut self) {
        self.tick = 0;
        self.accumulator = 0.0;
    }

    /// Number of registered systems.
    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    /// Is the simulation paused?
    pub fn is_paused(&self) -> bool {
        matches!(self.speed, SimSpeed::Paused)
    }

    /// Interpolation alpha for rendering between sim ticks (0.0 - 1.0).
    pub fn alpha(&self) -> f32 {
        let tick_duration = 1.0 / self.ticks_per_second as f64;
        if tick_duration > 0.0 {
            (self.accumulator / tick_duration) as f32
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct CounterSystem {
        count: u32,
        interval: u32,
        prio: u8,
    }

    impl SimSystem for CounterSystem {
        fn priority(&self) -> u8 {
            self.prio
        }
        fn tick_interval(&self) -> u32 {
            self.interval
        }
        fn name(&self) -> &str {
            "counter"
        }
        fn update(&mut self, _ctx: &mut SimContext) {
            self.count += 1;
        }
    }

    // ── Tick advancement ────────────────────────────────────

    #[test]
    fn basic_advance() {
        let mut runner = SimulationRunner::new(10); // 10 ticks/sec
        let ticks = runner.advance(1.0); // 1 second = 10 ticks
        assert_eq!(ticks, 10);
        assert_eq!(runner.tick, 10);
    }

    #[test]
    fn paused_no_ticks() {
        let mut runner = SimulationRunner::new(10);
        runner.set_speed(SimSpeed::Paused);
        let ticks = runner.advance(1.0);
        assert_eq!(ticks, 0);
        assert_eq!(runner.tick, 0);
    }

    #[test]
    fn speed_multiplier() {
        let mut runner = SimulationRunner::new(10);
        runner.set_speed(SimSpeed::Fast); // 2x
        runner.max_ticks_per_frame = 100;
        let ticks = runner.advance(0.5); // 0.5 sec * 2x = 10 ticks
        assert_eq!(ticks, 10);
    }

    #[test]
    fn max_ticks_per_frame_cap() {
        let mut runner = SimulationRunner::new(10);
        runner.max_ticks_per_frame = 5;
        let ticks = runner.advance(1.0); // wants 10, capped to 5
        assert_eq!(ticks, 5);
    }

    #[test]
    fn system_tick_interval() {
        let mut runner = SimulationRunner::new(10);
        runner.add_system(Box::new(CounterSystem {
            count: 0,
            interval: 3,
            prio: 100,
        }));
        runner.advance(1.0); // 10 ticks
        assert_eq!(runner.tick, 10);
    }

    // ── Speed control ───────────────────────────────────────

    #[test]
    fn speed_cycle() {
        assert_eq!(SimSpeed::Paused.next(), SimSpeed::Normal);
        assert_eq!(SimSpeed::Normal.next(), SimSpeed::Fast);
        assert_eq!(SimSpeed::Fast.next(), SimSpeed::VeryFast);
        assert_eq!(SimSpeed::VeryFast.next(), SimSpeed::Ultra);
        assert_eq!(SimSpeed::Ultra.next(), SimSpeed::Paused);
        assert_eq!(SimSpeed::Custom(3.0).next(), SimSpeed::Normal);
    }

    #[test]
    fn reset_clears_state() {
        let mut runner = SimulationRunner::new(10);
        runner.advance(1.0);
        assert!(runner.tick > 0);
        runner.reset();
        assert_eq!(runner.tick, 0);
    }
}
