//! Rollback netcode for peer-to-peer multiplayer.
//!
//! Implements GGPO-style rollback with input prediction, snapshot/restore,
//! and resimulation on mismatch.

use crate::checksum::StateHasher;
use crate::PlayerId;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the rollback session.
#[derive(Clone, Debug)]
pub struct RollbackConfig {
    /// Maximum number of frames we are willing to roll back.
    pub max_rollback_frames: u32,
    /// Capacity of the snapshot ring buffer. Must be > `max_rollback_frames`.
    pub snapshot_buffer_size: usize,
}

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            max_rollback_frames: 8,
            snapshot_buffer_size: 16,
        }
    }
}

// ---------------------------------------------------------------------------
// RollbackState trait
// ---------------------------------------------------------------------------

/// Trait that game state must implement to participate in rollback.
pub trait RollbackState: Clone {
    /// The per-player input type.
    type Input: Clone + Default + PartialEq + Serialize + for<'de> Deserialize<'de>;

    /// Serialize the full state to bytes.
    fn snapshot(&self) -> Vec<u8>;

    /// Restore state from a previous snapshot.
    fn restore(&mut self, data: &[u8]);

    /// Advance the simulation by one tick with the given inputs.
    fn simulate_tick(&mut self, inputs: &[(PlayerId, Self::Input)]);

    /// Feed state into the hasher for desync detection.
    fn checksum(&self, hasher: &mut StateHasher);
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

/// Runtime statistics for the rollback session.
#[derive(Clone, Debug, Default)]
pub struct RollbackStats {
    /// How many rollbacks occurred in the current second.
    pub rollbacks_this_second: u32,
    /// Deepest rollback depth seen.
    pub max_rollback_depth: u32,
    /// Ratio of correct predictions (0.0 – 1.0).
    pub prediction_accuracy: f32,
    total_predictions: u64,
    correct_predictions: u64,
}

// ---------------------------------------------------------------------------
// RollbackSession
// ---------------------------------------------------------------------------

/// The core rollback session that manages snapshots, prediction, and
/// resimulation.
pub struct RollbackSession<S: RollbackState> {
    config: RollbackConfig,
    local_player: PlayerId,
    players: Vec<PlayerId>,
    current_tick: u64,
    #[allow(dead_code)]
    last_confirmed_tick: u64,
    /// Ring buffer of snapshots indexed by `tick % snapshot_buffer_size`.
    snapshots: Vec<Option<Vec<u8>>>,
    /// Confirmed (authoritative) inputs per player per tick.
    confirmed_inputs: FxHashMap<PlayerId, FxHashMap<u64, S::Input>>,
    /// Predicted inputs per player per tick.
    predicted_inputs: FxHashMap<PlayerId, FxHashMap<u64, S::Input>>,
    /// Last known input for each remote player (used for prediction).
    last_known_input: FxHashMap<PlayerId, S::Input>,
    /// Per-tick checksums for desync detection.
    checksums: FxHashMap<u64, u32>,
    /// Runtime statistics.
    stats: RollbackStats,
}

impl<S: RollbackState> RollbackSession<S> {
    /// Create a new rollback session.
    pub fn new(config: RollbackConfig, local_player: PlayerId, players: Vec<PlayerId>) -> Self {
        let snapshot_buf = vec![None; config.snapshot_buffer_size];
        let mut confirmed_inputs = FxHashMap::default();
        let mut predicted_inputs = FxHashMap::default();
        let mut last_known_input = FxHashMap::default();
        for &pid in &players {
            confirmed_inputs.insert(pid, FxHashMap::default());
            predicted_inputs.insert(pid, FxHashMap::default());
            last_known_input.insert(pid, S::Input::default());
        }

        Self {
            config,
            local_player,
            players,
            current_tick: 0,
            last_confirmed_tick: 0,
            snapshots: snapshot_buf,
            confirmed_inputs,
            predicted_inputs,
            last_known_input,
            checksums: FxHashMap::default(),
            stats: RollbackStats::default(),
        }
    }

    /// Main per-frame entry point.
    ///
    /// * `state` — the mutable game state
    /// * `local_input` — this player's input for the current tick
    /// * `remote_inputs` — newly received remote inputs: `(player, tick, input)`
    pub fn advance_tick(
        &mut self,
        state: &mut S,
        local_input: S::Input,
        remote_inputs: Vec<(PlayerId, u64, S::Input)>,
    ) {
        let tick = self.current_tick;

        // 1. Store local input as confirmed for this tick.
        self.confirmed_inputs
            .entry(self.local_player)
            .or_default()
            .insert(tick, local_input.clone());
        self.last_known_input.insert(self.local_player, local_input);

        // 2. Process remote inputs — store as confirmed.
        for (pid, remote_tick, input) in &remote_inputs {
            self.confirmed_inputs
                .entry(*pid)
                .or_default()
                .insert(*remote_tick, input.clone());
            self.last_known_input.insert(*pid, input.clone());
        }

        // 3. Check for prediction mismatches — find earliest mismatch tick.
        let mut earliest_mismatch: Option<u64> = None;
        for (pid, remote_tick, input) in &remote_inputs {
            if let Some(predicted) = self
                .predicted_inputs
                .get(pid)
                .and_then(|m| m.get(remote_tick))
            {
                // Track prediction accuracy.
                self.stats.total_predictions += 1;
                if predicted == input {
                    self.stats.correct_predictions += 1;
                } else {
                    // Mismatch!
                    match earliest_mismatch {
                        Some(prev) => {
                            if *remote_tick < prev {
                                earliest_mismatch = Some(*remote_tick);
                            }
                        }
                        None => {
                            earliest_mismatch = Some(*remote_tick);
                        }
                    }
                }
            }
        }

        // 4. If mismatch: rollback and resimulate.
        if let Some(mismatch_tick) = earliest_mismatch {
            // Clamp: don't roll back further than max_rollback_frames.
            let min_tick = tick.saturating_sub(self.config.max_rollback_frames as u64);
            let rollback_to = mismatch_tick.max(min_tick);

            if let Some(snap_data) = self.restore_snapshot(rollback_to) {
                let snap_data = snap_data.to_vec();
                state.restore(&snap_data);

                let depth = tick.saturating_sub(rollback_to) as u32;
                self.stats.rollbacks_this_second += 1;
                if depth > self.stats.max_rollback_depth {
                    self.stats.max_rollback_depth = depth;
                }

                // Resimulate from rollback_to up to (but not including) current tick.
                for resim_tick in rollback_to..tick {
                    let inputs = self.gather_inputs(resim_tick);
                    state.simulate_tick(&inputs);
                    // Re-save snapshot after resimulation.
                    self.save_snapshot(state, resim_tick + 1);
                }
            }
        }

        // 5. Save snapshot for current tick (before simulating it).
        self.save_snapshot(state, tick);

        // 6. Gather inputs for current tick and simulate.
        let inputs = self.gather_inputs(tick);
        state.simulate_tick(&inputs);

        // 7. Compute and store checksum.
        let mut hasher = StateHasher::new();
        state.checksum(&mut hasher);
        self.checksums.insert(tick, hasher.finish_crc());

        // 8. Increment current_tick.
        self.current_tick += 1;

        // 9. Update prediction accuracy stat.
        if self.stats.total_predictions > 0 {
            self.stats.prediction_accuracy =
                self.stats.correct_predictions as f32 / self.stats.total_predictions as f32;
        }
    }

    /// Predict the input for a remote player (returns last known or default).
    pub fn predict_input(&self, player: PlayerId) -> S::Input {
        self.last_known_input
            .get(&player)
            .cloned()
            .unwrap_or_default()
    }

    /// Save a snapshot into the ring buffer at the given tick.
    /// Stores `(tick_le_bytes ++ data)` so we can validate on restore.
    pub fn save_snapshot(&mut self, state: &S, tick: u64) {
        let idx = tick as usize % self.config.snapshot_buffer_size;
        let data = state.snapshot();
        let mut stored = Vec::with_capacity(8 + data.len());
        stored.extend_from_slice(&tick.to_le_bytes());
        stored.extend_from_slice(&data);
        self.snapshots[idx] = Some(stored);
    }

    /// Retrieve a snapshot from the ring buffer, verifying the tick matches.
    /// Returns `None` if the slot was overwritten by a newer tick.
    pub fn restore_snapshot(&self, tick: u64) -> Option<&[u8]> {
        let idx = tick as usize % self.config.snapshot_buffer_size;
        let stored = self.snapshots[idx].as_deref()?;
        if stored.len() < 8 {
            return None;
        }
        let mut tick_bytes = [0u8; 8];
        tick_bytes.copy_from_slice(&stored[..8]);
        if u64::from_le_bytes(tick_bytes) != tick {
            return None;
        }
        Some(&stored[8..])
    }

    /// The current simulation tick.
    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    /// Runtime statistics.
    pub fn stats(&self) -> &RollbackStats {
        &self.stats
    }

    // ── Internal helpers ────────────────────────────────────────

    /// Gather the best-known inputs for all players at a given tick.
    /// Uses confirmed inputs when available, otherwise predicts and records
    /// the prediction.
    fn gather_inputs(&mut self, tick: u64) -> Vec<(PlayerId, S::Input)> {
        let players = self.players.clone();
        let mut result = Vec::with_capacity(players.len());
        for &pid in &players {
            if let Some(input) = self.confirmed_inputs.get(&pid).and_then(|m| m.get(&tick)) {
                result.push((pid, input.clone()));
            } else {
                let predicted = self.predict_input(pid);
                self.predicted_inputs
                    .entry(pid)
                    .or_default()
                    .insert(tick, predicted.clone());
                result.push((pid, predicted));
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::StateHasher;
    use crate::PlayerId;

    // ── Mock state ──────────────────────────────────────────────

    #[derive(Clone, Default)]
    struct MockState {
        value: i32,
    }

    impl RollbackState for MockState {
        type Input = i32;

        fn snapshot(&self) -> Vec<u8> {
            self.value.to_le_bytes().to_vec()
        }

        fn restore(&mut self, data: &[u8]) {
            self.value = i32::from_le_bytes(data.try_into().unwrap());
        }

        fn simulate_tick(&mut self, inputs: &[(PlayerId, i32)]) {
            for (_, v) in inputs {
                self.value += v;
            }
        }

        fn checksum(&self, hasher: &mut StateHasher) {
            hasher.write_i32(self.value);
        }
    }

    // ── Helpers ─────────────────────────────────────────────────

    fn p(id: u32) -> PlayerId {
        PlayerId(id)
    }

    // ── Tests ───────────────────────────────────────────────────

    #[test]
    fn test_snapshot_roundtrip() {
        let state = MockState { value: 42 };
        let data = state.snapshot();
        let mut restored = MockState::default();
        restored.restore(&data);
        assert_eq!(restored.value, 42);
    }

    #[test]
    fn test_advance_no_rollback() {
        // Two players, feed confirmed inputs immediately every tick.
        let config = RollbackConfig {
            max_rollback_frames: 8,

            snapshot_buffer_size: 16,
        };
        let players = vec![p(1), p(2)];
        let mut session = RollbackSession::<MockState>::new(config, p(1), players);
        let mut state = MockState::default();

        // Run 100 ticks. Each tick: player 1 inputs 1, player 2 inputs 2.
        for tick in 0..100u64 {
            let remote_inputs = vec![(p(2), tick, 2)];
            session.advance_tick(&mut state, 1, remote_inputs);
        }

        // Each tick adds 1 + 2 = 3. After 100 ticks: 300.
        assert_eq!(state.value, 300);
        assert_eq!(session.current_tick(), 100);
    }

    #[test]
    fn test_prediction_uses_last_input() {
        let config = RollbackConfig::default();
        let players = vec![p(1), p(2)];
        let mut session = RollbackSession::<MockState>::new(config, p(1), players);

        // Initially, prediction should be default (0).
        assert_eq!(session.predict_input(p(2)), 0);

        // After receiving an input from player 2, prediction should reflect it.
        let mut state = MockState::default();
        let remote_inputs = vec![(p(2), 0, 7)];
        session.advance_tick(&mut state, 1, remote_inputs);

        assert_eq!(session.predict_input(p(2)), 7);
    }

    #[test]
    fn test_rollback_on_mismatch() {
        let config = RollbackConfig {
            max_rollback_frames: 64,

            snapshot_buffer_size: 128,
        };
        let players = vec![p(1), p(2)];
        let mut session = RollbackSession::<MockState>::new(config, p(1), players);
        let mut state = MockState::default();

        // Run 50 ticks: player 1 always inputs 1, player 2 always inputs 2
        // (confirmed immediately).
        for tick in 0..50u64 {
            let remote_inputs = vec![(p(2), tick, 2)];
            session.advance_tick(&mut state, 1, remote_inputs);
        }
        // state.value = 50 * 3 = 150
        assert_eq!(state.value, 150);

        // Now run ticks 50..60 with NO remote inputs (player 2 predicted as 2,
        // since last known = 2).
        for _tick in 50..60 {
            session.advance_tick(&mut state, 1, vec![]);
        }
        // Predicted: each tick still adds 1 + 2 = 3. state.value = 150 + 30 = 180
        assert_eq!(state.value, 180);

        // Now we get the REAL inputs for ticks 50..60 from player 2: they were 10
        // instead of 2. This triggers a rollback.
        let mut corrections: Vec<(PlayerId, u64, i32)> = Vec::new();
        for tick in 50..60u64 {
            corrections.push((p(2), tick, 10));
        }
        // Advance tick 60 with these corrections.
        session.advance_tick(&mut state, 1, corrections);

        // After rollback and resimulation:
        // Ticks 0..50: each adds 3 → 150
        // Ticks 50..60: each adds 1 + 10 = 11 → 110
        // Tick 60: adds 1 + 10 = 11 (last known for p2 is now 10)
        // Total: 150 + 110 + 11 = 271
        assert_eq!(state.value, 271);

        // Verify rollback stats show at least one rollback.
        assert!(session.stats().rollbacks_this_second > 0);
    }

    #[test]
    fn test_stats_tracking() {
        let config = RollbackConfig {
            max_rollback_frames: 64,

            snapshot_buffer_size: 128,
        };
        let players = vec![p(1), p(2)];
        let mut session = RollbackSession::<MockState>::new(config, p(1), players);
        let mut state = MockState::default();

        // Run a few ticks with confirmed inputs.
        for tick in 0..5u64 {
            session.advance_tick(&mut state, 1, vec![(p(2), tick, 2)]);
        }
        assert_eq!(session.stats().rollbacks_this_second, 0);

        // Run tick 5 without remote input (prediction kicks in: predicts 2).
        session.advance_tick(&mut state, 1, vec![]);

        // Now deliver a mismatched input for tick 5.
        // Tick 6 with correction for tick 5.
        session.advance_tick(&mut state, 1, vec![(p(2), 5, 999)]);

        // Should have recorded a rollback.
        assert!(session.stats().rollbacks_this_second > 0);
    }
}
