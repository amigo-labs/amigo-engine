//! GGPO-style rollback netcode.
//!
//! Central types:
//! - [`RollbackState`] -- trait the game implements for snapshot/restore/simulate.
//! - [`RollbackConfig`] -- tuning knobs (max rollback depth, input delay, etc.).
//! - [`RollbackSession`] -- the coordinator that drives prediction, rollback, and
//!   resimulation each tick.

// Re-export crate types that callers may need alongside rollback.
#[allow(unused_imports)]
use crate::PlayerId;

// ---------------------------------------------------------------------------
// RollbackState trait
// ---------------------------------------------------------------------------

/// Trait that game state must implement to participate in rollback netcode.
///
/// All methods must be **deterministic** -- given the same snapshot bytes and
/// the same inputs, `simulate_tick` must produce bit-identical results across
/// all peers.  Use the fixed-point math from `amigo_core::math` to guarantee
/// this.
pub trait RollbackState {
    /// The per-player input type.  Must be cheaply cloneable (it is copied
    /// into prediction buffers).
    type Input: Clone + Default + PartialEq;

    /// Serialize the entire game state into an opaque byte blob.
    /// This is called once per tick to record a snapshot for potential rollback.
    fn snapshot(&self) -> Vec<u8>;

    /// Restore game state from a blob previously produced by [`snapshot`].
    fn restore(&mut self, data: &[u8]);

    /// Advance the simulation by one tick using the provided per-player inputs.
    /// The slice is indexed by player handle (0..num_players).
    fn simulate_tick(&mut self, inputs: &[Self::Input]);
}

// ---------------------------------------------------------------------------
// RollbackConfig
// ---------------------------------------------------------------------------

/// Configuration for a [`RollbackSession`].
#[derive(Clone, Debug)]
pub struct RollbackConfig {
    /// Maximum number of frames we are willing to roll back.
    /// If remote inputs arrive later than this, they are dropped.
    pub max_rollback_frames: usize,
    /// Number of frames of intentional input delay (reduces rollback frequency).
    pub input_delay: usize,
    /// Number of players in the session.
    pub num_players: usize,
    /// Index of the local player (0-based).
    pub local_player: usize,
}

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            max_rollback_frames: 8,
            input_delay: 2,
            num_players: 2,
            local_player: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: per-frame record
// ---------------------------------------------------------------------------

/// Snapshot + inputs for a single frame, stored in the ring buffer.
#[derive(Clone)]
struct FrameRecord<I: Clone> {
    /// The frame number.
    frame: u64,
    /// Game-state snapshot taken *before* this frame's simulate_tick.
    snapshot: Vec<u8>,
    /// Per-player inputs used (or predicted) for this frame.
    inputs: Vec<I>,
    /// Whether each player's input was a prediction (true) or confirmed (false).
    predicted: Vec<bool>,
}

// ---------------------------------------------------------------------------
// RollbackSession
// ---------------------------------------------------------------------------

/// Drives a GGPO-style rollback session.
///
/// # Usage
///
/// ```ignore
/// let mut session = RollbackSession::new(config);
/// // game loop:
/// loop {
///     let local_input = poll_local_input();
///     let events = session.advance(&mut game_state, local_input, &mut transport);
///     render(&game_state);
/// }
/// ```
pub struct RollbackSession<I: Clone + Default + PartialEq> {
    config: RollbackConfig,
    /// Current frame number (0-based, monotonically increasing).
    current_frame: u64,
    /// Ring buffer of frame records.  Index = frame % capacity.
    history: Vec<Option<FrameRecord<I>>>,
    /// Last confirmed input per remote player (used for prediction).
    last_confirmed_input: Vec<I>,
    /// The latest frame for which we have *confirmed* input from each player.
    confirmed_frame: Vec<u64>,
    /// Pending remote inputs that arrived out-of-order.
    /// Maps (player_index, frame) -> input.
    pending_remote: Vec<(usize, u64, I)>,
    /// Stats: number of rollbacks performed so far.
    pub rollback_count: u64,
    /// Stats: total frames resimulated across all rollbacks.
    pub resim_frames_total: u64,
}

/// Events returned from [`RollbackSession::advance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RollbackEvent {
    /// A rollback was performed; the value is how many frames were resimulated.
    Rollback { resim_frames: usize },
}

impl<I: Clone + Default + PartialEq> RollbackSession<I> {
    /// Create a new session.
    pub fn new(config: RollbackConfig) -> Self {
        let capacity = config.max_rollback_frames + 1;
        let num_players = config.num_players;
        Self {
            config,
            current_frame: 0,
            history: (0..capacity).map(|_| None).collect(),
            last_confirmed_input: vec![I::default(); num_players],
            confirmed_frame: vec![0; num_players],
            pending_remote: Vec::new(),
            rollback_count: 0,
            resim_frames_total: 0,
        }
    }

    /// The current frame the session is on.
    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// The configuration.
    pub fn config(&self) -> &RollbackConfig {
        &self.config
    }

    // -- ring buffer helpers ------------------------------------------------

    fn ring_idx(&self, frame: u64) -> usize {
        (frame as usize) % self.history.len()
    }

    fn store_record(&mut self, record: FrameRecord<I>) {
        let idx = self.ring_idx(record.frame);
        self.history[idx] = Some(record);
    }

    fn get_record(&self, frame: u64) -> Option<&FrameRecord<I>> {
        let idx = self.ring_idx(frame);
        self.history[idx]
            .as_ref()
            .filter(|r| r.frame == frame)
    }

    // -- input helpers ------------------------------------------------------

    /// Build the input vector for `frame`, filling in predictions for any
    /// player whose confirmed input has not yet arrived.
    fn build_inputs(&self, frame: u64) -> (Vec<I>, Vec<bool>) {
        let n = self.config.num_players;
        let mut inputs = Vec::with_capacity(n);
        let mut predicted = Vec::with_capacity(n);
        for player in 0..n {
            if player == self.config.local_player {
                // Local input is always confirmed (set by caller).
                // Placeholder -- caller overwrites slot before simulate.
                inputs.push(I::default());
                predicted.push(false);
            } else {
                // Check pending_remote first, then fall back to last confirmed.
                let maybe = self
                    .pending_remote
                    .iter()
                    .find(|(p, f, _)| *p == player && *f == frame);
                if let Some((_, _, input)) = maybe {
                    inputs.push(input.clone());
                    predicted.push(false);
                } else {
                    // Predict: repeat last known input.
                    inputs.push(self.last_confirmed_input[player].clone());
                    predicted.push(true);
                }
            }
        }
        (inputs, predicted)
    }

    // -- public API ---------------------------------------------------------

    /// Receive remote inputs from the transport and queue them.
    ///
    /// The transport is expected to yield `(PlayerId, Vec<(u64, I)>)` — each
    /// inner tuple is `(frame, input)`.  Because the existing `Transport` trait
    /// works with an opaque command type `C`, we accept pre-decoded tuples here
    /// so callers can bridge their own transport/encoding layer.
    pub fn add_remote_input(&mut self, player: usize, frame: u64, input: I) {
        // Update last confirmed input for this player.
        if frame >= self.confirmed_frame[player] {
            self.last_confirmed_input[player] = input.clone();
            self.confirmed_frame[player] = frame;
        }
        self.pending_remote.push((player, frame, input));
    }

    /// Core per-tick function.  Call this exactly once per game-loop iteration.
    ///
    /// 1. Checks if any queued remote inputs contradict earlier predictions.
    /// 2. If so, rolls back to the earliest mispredicted frame and resimulates.
    /// 3. Advances the simulation by one tick with the local player's input.
    /// 4. Returns a list of events (e.g. rollback notifications).
    pub fn advance<S: RollbackState<Input = I>>(
        &mut self,
        state: &mut S,
        local_input: I,
        ) -> Vec<RollbackEvent> {
        let mut events = Vec::new();

        // ── Step 1: find earliest misprediction ──────────────────────────
        let mut earliest_misprediction: Option<u64> = None;

        for &(player, frame, ref input) in &self.pending_remote {
            // Only care about frames we have already simulated with a prediction.
            if let Some(record) = self.get_record(frame) {
                if record.predicted[player] && record.inputs[player] != *input {
                    earliest_misprediction = Some(match earliest_misprediction {
                        Some(prev) => prev.min(frame),
                        None => frame,
                    });
                }
            }
        }

        // ── Step 2: rollback + resimulate if needed ──────────────────────
        if let Some(rb_frame) = earliest_misprediction {
            // Restore state to the snapshot at rb_frame (snapshot is state
            // *before* that frame's simulate_tick).
            if let Some(record) = self.get_record(rb_frame) {
                state.restore(&record.snapshot);
            }

            let resim_count = (self.current_frame - rb_frame) as usize;
            self.rollback_count += 1;
            self.resim_frames_total += resim_count as u64;

            // Resimulate from rb_frame up to (but not including) current_frame.
            for f in rb_frame..self.current_frame {
                let (mut inputs, predicted) = self.build_inputs(f);

                // For the local player, reuse the stored confirmed input.
                if let Some(record) = self.get_record(f) {
                    inputs[self.config.local_player] =
                        record.inputs[self.config.local_player].clone();
                }

                // Snapshot before simulate.
                let snap = state.snapshot();
                state.simulate_tick(&inputs);

                self.store_record(FrameRecord {
                    frame: f,
                    snapshot: snap,
                    inputs,
                    predicted,
                });
            }

            events.push(RollbackEvent::Rollback {
                resim_frames: resim_count,
            });
        }

        // Drain pending (we've consumed them).
        self.pending_remote.clear();

        // ── Step 3: advance current frame ────────────────────────────────
        let (mut inputs, predicted) = self.build_inputs(self.current_frame);
        inputs[self.config.local_player] = local_input;

        let snap = state.snapshot();
        state.simulate_tick(&inputs);

        self.store_record(FrameRecord {
            frame: self.current_frame,
            snapshot: snap,
            inputs,
            predicted,
        });

        self.current_frame += 1;

        events
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Minimal test game state -------------------------------------------

    /// Trivial game state: a per-player counter incremented by each player's
    /// input value every tick.  Easy to verify determinism.
    #[derive(Clone, Default)]
    struct TestState {
        counters: Vec<i32>,
    }

    #[derive(Clone, Default, PartialEq)]
    struct TestInput {
        delta: i32,
    }

    impl RollbackState for TestState {
        type Input = TestInput;

        fn snapshot(&self) -> Vec<u8> {
            self.counters
                .iter()
                .flat_map(|c| c.to_le_bytes())
                .collect()
        }

        fn restore(&mut self, data: &[u8]) {
            self.counters = data
                .chunks_exact(4)
                .map(|chunk| i32::from_le_bytes(chunk.try_into().unwrap()))
                .collect();
        }

        fn simulate_tick(&mut self, inputs: &[TestInput]) {
            for (counter, input) in self.counters.iter_mut().zip(inputs.iter()) {
                *counter += input.delta;
            }
        }
    }

    // -- Tests -------------------------------------------------------------

    #[test]
    fn basic_advance_no_rollback() {
        let config = RollbackConfig {
            max_rollback_frames: 8,
            input_delay: 0,
            num_players: 2,
            local_player: 0,
        };
        let mut session = RollbackSession::<TestInput>::new(config);
        let mut state = TestState {
            counters: vec![0, 0],
        };

        // Advance 10 frames with constant inputs.
        for _ in 0..10 {
            let local = TestInput { delta: 1 };
            // Remote player: feed confirmed input for this frame (no prediction).
            session.add_remote_input(
                1,
                session.current_frame(),
                TestInput { delta: 2 },
            );
            let events = session.advance(&mut state, local);
            assert!(events.is_empty(), "no rollback expected");
        }

        assert_eq!(state.counters[0], 10); // local: +1 * 10
        assert_eq!(state.counters[1], 20); // remote: +2 * 10
        assert_eq!(session.current_frame(), 10);
        assert_eq!(session.rollback_count, 0);
    }

    #[test]
    fn rollback_on_misprediction() {
        let config = RollbackConfig {
            max_rollback_frames: 8,
            input_delay: 0,
            num_players: 2,
            local_player: 0,
        };
        let mut session = RollbackSession::<TestInput>::new(config);
        let mut state = TestState {
            counters: vec![0, 0],
        };

        // Frame 0: no remote input yet -> prediction (default = delta 0).
        let events = session.advance(&mut state, TestInput { delta: 1 });
        assert!(events.is_empty());
        // After frame 0: counters = [1, 0] (predicted remote = 0).
        assert_eq!(state.counters, vec![1, 0]);

        // Frame 1: still no remote input.
        let events = session.advance(&mut state, TestInput { delta: 1 });
        assert!(events.is_empty());
        assert_eq!(state.counters, vec![2, 0]);

        // Now the remote input for frame 0 arrives, and it was delta=5
        // (mispredicted -- we predicted 0).
        session.add_remote_input(0 + 1, 0, TestInput { delta: 5 });

        // Frame 2: should trigger rollback to frame 0, resim frames 0 and 1,
        // then advance frame 2.
        let events = session.advance(&mut state, TestInput { delta: 1 });
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            RollbackEvent::Rollback { resim_frames: 2 }
        );

        // After rollback+resim+advance:
        // frame 0: local=1, remote=5 -> [1, 5]
        // frame 1: local=1, remote=5 (predicted=last confirmed=5) -> [2, 10]
        // frame 2: local=1, remote=5 (predicted) -> [3, 15]
        assert_eq!(state.counters, vec![3, 15]);
        assert_eq!(session.rollback_count, 1);
        assert_eq!(session.resim_frames_total, 2);
    }

    #[test]
    fn no_rollback_when_prediction_correct() {
        let config = RollbackConfig {
            max_rollback_frames: 8,
            input_delay: 0,
            num_players: 2,
            local_player: 0,
        };
        let mut session = RollbackSession::<TestInput>::new(config);
        let mut state = TestState {
            counters: vec![0, 0],
        };

        // Frame 0: no remote input (predicted = default = delta 0).
        session.advance(&mut state, TestInput { delta: 1 });

        // Remote input for frame 0 arrives and matches the prediction (delta 0).
        session.add_remote_input(1, 0, TestInput { delta: 0 });

        // Frame 1: should NOT rollback since prediction was correct.
        let events = session.advance(&mut state, TestInput { delta: 1 });
        assert!(events.is_empty());
        assert_eq!(session.rollback_count, 0);
    }

    #[test]
    fn snapshot_restore_roundtrip() {
        let state = TestState {
            counters: vec![10, -20, 300],
        };
        let snap = state.snapshot();
        let mut restored = TestState::default();
        restored.restore(&snap);
        assert_eq!(restored.counters, vec![10, -20, 300]);
    }

    #[test]
    fn input_prediction_repeats_last_known() {
        let config = RollbackConfig {
            max_rollback_frames: 8,
            input_delay: 0,
            num_players: 2,
            local_player: 0,
        };
        let mut session = RollbackSession::<TestInput>::new(config);
        let mut state = TestState {
            counters: vec![0, 0],
        };

        // Feed confirmed remote input for frame 0 with delta=3.
        session.add_remote_input(1, 0, TestInput { delta: 3 });
        session.advance(&mut state, TestInput { delta: 1 });
        assert_eq!(state.counters, vec![1, 3]);

        // Frame 1: no remote input -- should predict delta=3 (last known).
        session.advance(&mut state, TestInput { delta: 1 });
        assert_eq!(state.counters, vec![2, 6]);

        // Frame 2: still predicting delta=3.
        session.advance(&mut state, TestInput { delta: 1 });
        assert_eq!(state.counters, vec![3, 9]);
    }

    #[test]
    fn many_frames_within_rollback_window() {
        let config = RollbackConfig {
            max_rollback_frames: 4,
            input_delay: 0,
            num_players: 2,
            local_player: 0,
        };
        let mut session = RollbackSession::<TestInput>::new(config);
        let mut state = TestState {
            counters: vec![0, 0],
        };

        // Run 20 frames feeding remote inputs on time.
        for _ in 0..20 {
            session.add_remote_input(
                1,
                session.current_frame(),
                TestInput { delta: 1 },
            );
            session.advance(&mut state, TestInput { delta: 1 });
        }

        assert_eq!(state.counters, vec![20, 20]);
        assert_eq!(session.rollback_count, 0);
    }
}
