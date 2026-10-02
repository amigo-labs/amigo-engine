use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// Identifies a registered callback in the scheduler.
///
/// This is a lightweight handle -- the scheduler does not store closures,
/// only interval metadata keyed by this ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CallbackId(pub u32);

/// Entry tracking when a callback should next fire.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct ScheduleEntry {
    /// Run every `interval` ticks.
    interval: u64,
    /// The tick on which the callback last ran (or was registered).
    last_run: u64,
}

/// A tick-based scheduler that tracks which callbacks should run on a given tick.
///
/// No closures are stored -- the caller checks [`TickScheduler::should_run`] each tick and
/// dispatches externally.
///
/// The scheduler is part of the simulation state: it is `Clone` and
/// serializable so rollback and rewind snapshots can restore it together with
/// the world. Restoring a world without it would keep `last_run` from the
/// abandoned timeline and skip callbacks during re-simulation.
///
/// ```ignore
/// let mut sched = TickScheduler::new();
/// let physics = CallbackId(0);
/// sched.every(2, physics); // run every 2 ticks
///
/// assert!(sched.should_run(physics, 0));  // first tick always runs
/// assert!(!sched.should_run(physics, 1));
/// assert!(sched.should_run(physics, 2));
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TickScheduler {
    entries: FxHashMap<CallbackId, ScheduleEntry>,
}

impl TickScheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            entries: FxHashMap::default(),
        }
    }

    /// Register a callback to run every `interval` ticks, starting immediately.
    ///
    /// If the callback was already registered, its interval is updated and
    /// `last_run` is reset so it fires on the next `should_run` check.
    ///
    /// # Panics
    /// Panics if `interval` is zero.
    pub fn every(&mut self, interval: u64, id: CallbackId) {
        assert!(interval > 0, "interval must be at least 1");
        self.entries.insert(
            id,
            ScheduleEntry {
                interval,
                last_run: 0,
            },
        );
    }

    /// Returns `true` if `id` should run on `current_tick`, and internally
    /// records the tick so the next invocation spaces correctly.
    ///
    /// Returns `false` if the callback has not been registered.
    pub fn should_run(&mut self, id: CallbackId, current_tick: u64) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };

        // On tick 0 (or whenever the entry was just registered with last_run == 0
        // and current_tick == 0), always fire.
        if current_tick == 0 {
            entry.last_run = 0;
            return true;
        }

        if current_tick >= entry.last_run.saturating_add(entry.interval) {
            entry.last_run = current_tick;
            return true;
        }

        false
    }

    /// Remove a callback from the scheduler. Returns `true` if it was present.
    pub fn remove(&mut self, id: CallbackId) -> bool {
        self.entries.remove(&id).is_some()
    }

    /// Returns `true` if a callback with the given id is registered.
    pub fn is_registered(&self, id: CallbackId) -> bool {
        self.entries.contains_key(&id)
    }

    /// Number of registered callbacks.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no callbacks are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_every_n_ticks() {
        let mut sched = TickScheduler::new();
        let id = CallbackId(1);
        sched.every(3, id);

        // Tick 0 always fires.
        assert!(sched.should_run(id, 0));
        assert!(!sched.should_run(id, 1));
        assert!(!sched.should_run(id, 2));
        assert!(sched.should_run(id, 3));
        assert!(!sched.should_run(id, 4));
        assert!(!sched.should_run(id, 5));
        assert!(sched.should_run(id, 6));
    }

    #[test]
    fn every_tick() {
        let mut sched = TickScheduler::new();
        let id = CallbackId(0);
        sched.every(1, id);

        for tick in 0..10 {
            assert!(sched.should_run(id, tick), "should fire on tick {tick}");
        }
    }

    #[test]
    fn unregistered_returns_false() {
        let mut sched = TickScheduler::new();
        assert!(!sched.should_run(CallbackId(99), 0));
    }

    #[test]
    fn remove_stops_firing() {
        let mut sched = TickScheduler::new();
        let id = CallbackId(1);
        sched.every(1, id);
        assert!(sched.should_run(id, 0));
        assert!(sched.remove(id));
        assert!(!sched.should_run(id, 1));
    }

    #[test]
    fn re_register_resets_timing() {
        let mut sched = TickScheduler::new();
        let id = CallbackId(1);
        sched.every(5, id);
        assert!(sched.should_run(id, 0));
        assert!(!sched.should_run(id, 3));

        // Re-register with a shorter interval -- last_run resets to 0.
        sched.every(2, id);
        assert!(sched.should_run(id, 2));
    }

    #[test]
    #[should_panic(expected = "interval must be at least 1")]
    fn zero_interval_panics() {
        let mut sched = TickScheduler::new();
        sched.every(0, CallbackId(0));
    }

    #[test]
    fn multiple_callbacks() {
        let mut sched = TickScheduler::new();
        let fast = CallbackId(0);
        let slow = CallbackId(1);
        sched.every(1, fast);
        sched.every(4, slow);

        assert!(sched.should_run(fast, 0));
        assert!(sched.should_run(slow, 0));

        assert!(sched.should_run(fast, 1));
        assert!(!sched.should_run(slow, 1));

        assert!(sched.should_run(fast, 4));
        assert!(sched.should_run(slow, 4));
    }

    #[test]
    fn restoring_a_snapshot_replays_the_same_callbacks() {
        let id = CallbackId(7);
        let mut sched = TickScheduler::new();
        sched.every(5, id);
        let fired_live: Vec<u64> = (0..=100).filter(|&t| sched.should_run(id, t)).collect();

        // Roll back to tick 95 and re-simulate: a scheduler restored from a
        // snapshot taken at 95 must fire at 100 again, as it did live.
        let mut sched = TickScheduler::new();
        sched.every(5, id);
        (0..=95).for_each(|t| {
            sched.should_run(id, t);
        });
        let snapshot = sched.clone();
        (96..=100).for_each(|t| {
            sched.should_run(id, t);
        });
        let mut restored = snapshot;
        let refired: Vec<u64> = (96..=100).filter(|&t| restored.should_run(id, t)).collect();
        assert_eq!(refired, vec![100]);
        assert!(fired_live.contains(&100));
    }

    #[test]
    fn huge_interval_does_not_overflow() {
        let id = CallbackId(1);
        let mut sched = TickScheduler::new();
        sched.every(u64::MAX - 1, id);
        assert!(sched.should_run(id, 0));
        assert!(!sched.should_run(id, 3));
        assert!(sched.should_run(id, u64::MAX - 1));
        // `last_run + interval` exceeds u64 here; it must not panic.
        let _ = sched.should_run(id, u64::MAX);
    }
}
