//! The fixed-timestep accumulator, as a pure function.
//!
//! This logic used to live inline in the windowed frame loop, tangled up with
//! winit events, the renderer and the API state — so the one part with real edge
//! cases (a long stall, a paused simulation, a step request while paused) could
//! not be tested at all.

/// How many simulation ticks a frame should run, and what is left over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickBudget {
    /// Ticks driven by elapsed real time.
    pub from_time: u32,
    /// Ticks explicitly requested over the API, which run even while paused.
    pub forced: u32,
    /// Accumulator remainder, carried to the next frame.
    pub accumulator: f64,
    /// Interpolation factor for rendering: how far into the next tick we are.
    pub alpha: f32,
}

impl TickBudget {
    /// Total ticks to run this frame.
    pub fn total(&self) -> u32 {
        self.from_time + self.forced
    }
}

/// Largest frame delta fed to the accumulator, in seconds.
///
/// Without a cap, one long stall (a breakpoint, a window drag, a slow asset load)
/// queues hundreds of ticks, which take longer than a frame to run, which queues
/// more — the spiral of death. Capping means the simulation falls behind wall
/// clock instead, which is the better failure.
pub const MAX_FRAME_TIME: f64 = 0.25;

/// Maximum ticks one frame may run from accumulated time.
///
/// The `MAX_FRAME_TIME` cap alone still allows 15 ticks per frame at 60 Hz. This
/// second bound keeps a single frame's update cost predictable.
pub const MAX_TICKS_PER_FRAME: u32 = 8;

/// Decide how many ticks to run this frame.
///
/// * `dt` — seconds since the last frame, uncapped.
/// * `accumulator` — leftover time from previous frames.
/// * `tick_duration` — length of one simulation tick.
/// * `speed` — simulation speed multiplier from `set_speed`.
/// * `paused` — when set, real time stops feeding the accumulator.
/// * `requested` — ticks asked for over the API; these run regardless of `paused`.
pub fn tick_budget(
    dt: f64,
    accumulator: f64,
    tick_duration: f64,
    speed: f32,
    paused: bool,
    requested: u64,
) -> TickBudget {
    debug_assert!(tick_duration > 0.0, "tick duration must be positive");

    let dt = dt.clamp(0.0, MAX_FRAME_TIME);
    let speed = if speed.is_finite() && speed > 0.0 {
        speed as f64
    } else {
        1.0
    };

    let mut accumulator = if paused {
        // Discard the elapsed time rather than banking it: unpausing should not
        // fast-forward through everything that happened while paused.
        accumulator
    } else {
        accumulator + dt * speed
    };

    let mut from_time = 0;
    while accumulator >= tick_duration && from_time < MAX_TICKS_PER_FRAME {
        accumulator -= tick_duration;
        from_time += 1;
    }

    // If the cap bit, drop the backlog instead of carrying it into the next frame
    // where it would immediately hit the cap again.
    if accumulator >= tick_duration {
        accumulator %= tick_duration;
    }

    TickBudget {
        from_time,
        forced: requested.min(MAX_TICKS_PER_FRAME as u64 * 4) as u32,
        accumulator,
        alpha: (accumulator / tick_duration) as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: f64 = 1.0 / 60.0;

    #[test]
    fn a_frame_shorter_than_a_tick_runs_nothing_and_banks_the_time() {
        let budget = tick_budget(TICK / 2.0, 0.0, TICK, 1.0, false, 0);

        assert_eq!(budget.total(), 0);
        assert!((budget.accumulator - TICK / 2.0).abs() < 1e-9);
        assert!((budget.alpha - 0.5).abs() < 1e-6, "alpha {}", budget.alpha);
    }

    #[test]
    fn a_frame_of_exactly_one_tick_runs_one_tick() {
        let budget = tick_budget(TICK, 0.0, TICK, 1.0, false, 0);

        assert_eq!(budget.from_time, 1);
        assert!(budget.accumulator < 1e-9);
    }

    #[test]
    fn banked_time_carries_across_frames() {
        // Three frames of 0.4 ticks each add up to more than one tick.
        let mut acc = 0.0;
        let mut ran = 0;
        for _ in 0..3 {
            let budget = tick_budget(TICK * 0.4, acc, TICK, 1.0, false, 0);
            acc = budget.accumulator;
            ran += budget.total();
        }
        assert_eq!(ran, 1, "0.4 + 0.4 + 0.4 ticks is one whole tick");
    }

    #[test]
    fn a_long_stall_is_capped_instead_of_spiralling() {
        // Ten seconds is 600 ticks of real time.
        let budget = tick_budget(10.0, 0.0, TICK, 1.0, false, 0);

        assert_eq!(
            budget.from_time, MAX_TICKS_PER_FRAME,
            "one frame must not try to run 600 ticks"
        );
        assert!(
            budget.accumulator < TICK,
            "the backlog must be dropped, not carried: {} left",
            budget.accumulator
        );
    }

    #[test]
    fn repeated_stalls_do_not_accumulate_a_growing_backlog() {
        let mut acc = 0.0;
        for _ in 0..20 {
            let budget = tick_budget(1.0, acc, TICK, 1.0, false, 0);
            acc = budget.accumulator;
            assert!(
                acc < TICK,
                "accumulator grew to {acc} — this is the spiral of death"
            );
        }
    }

    #[test]
    fn speed_scales_how_fast_the_simulation_advances() {
        let normal = tick_budget(TICK * 4.0, 0.0, TICK, 1.0, false, 0);
        let double = tick_budget(TICK * 4.0, 0.0, TICK, 2.0, false, 0);
        let half = tick_budget(TICK * 4.0, 0.0, TICK, 0.5, false, 0);

        assert_eq!(normal.from_time, 4);
        assert_eq!(double.from_time, 8);
        assert_eq!(half.from_time, 2);
    }

    #[test]
    fn a_nonsense_speed_falls_back_to_normal() {
        for bad in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            let budget = tick_budget(TICK * 2.0, 0.0, TICK, bad, false, 0);
            assert_eq!(
                budget.from_time, 2,
                "speed {bad} should be ignored, not applied"
            );
        }
    }

    #[test]
    fn pausing_stops_time_driven_ticks() {
        let budget = tick_budget(TICK * 5.0, 0.0, TICK, 1.0, true, 0);

        assert_eq!(budget.from_time, 0);
        assert_eq!(
            budget.accumulator, 0.0,
            "paused time is discarded, not banked"
        );
    }

    #[test]
    fn unpausing_does_not_fast_forward_through_the_pause() {
        // Ten seconds paused, then a normal frame.
        let paused = tick_budget(10.0, 0.0, TICK, 1.0, true, 0);
        let resumed = tick_budget(TICK, paused.accumulator, TICK, 1.0, false, 0);

        assert_eq!(
            resumed.total(),
            1,
            "the pause must not queue up ticks to burn through"
        );
    }

    #[test]
    fn a_step_request_runs_while_paused() {
        let budget = tick_budget(TICK * 3.0, 0.0, TICK, 1.0, true, 4);

        assert_eq!(budget.from_time, 0, "still paused");
        assert_eq!(budget.forced, 4, "but the requested ticks run");
        assert_eq!(budget.total(), 4);
    }

    #[test]
    fn forced_ticks_add_to_time_driven_ticks_when_running() {
        let budget = tick_budget(TICK * 2.0, 0.0, TICK, 1.0, false, 3);

        assert_eq!(budget.from_time, 2);
        assert_eq!(budget.forced, 3);
        assert_eq!(budget.total(), 5);
    }

    #[test]
    fn an_absurd_step_request_is_bounded() {
        let budget = tick_budget(0.0, 0.0, TICK, 1.0, true, 10_000_000);

        assert!(
            budget.forced <= MAX_TICKS_PER_FRAME * 4,
            "a client asking for ten million ticks must not freeze the window"
        );
    }

    #[test]
    fn a_negative_or_absurd_dt_is_clamped() {
        // A clock that went backwards, and one that jumped forward a minute.
        assert_eq!(tick_budget(-1.0, 0.0, TICK, 1.0, false, 0).from_time, 0);
        assert_eq!(
            tick_budget(60.0, 0.0, TICK, 1.0, false, 0).from_time,
            MAX_TICKS_PER_FRAME
        );
    }

    #[test]
    fn alpha_stays_within_one_tick() {
        for dt in [0.0, TICK * 0.1, TICK * 0.99, TICK, TICK * 3.7, 10.0] {
            let budget = tick_budget(dt, 0.0, TICK, 1.0, false, 0);
            assert!(
                (0.0..1.0).contains(&budget.alpha),
                "alpha {} out of range for dt {dt}",
                budget.alpha
            );
        }
    }
}
