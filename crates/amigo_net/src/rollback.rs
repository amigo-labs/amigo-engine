//! Rollback netcode for peer-to-peer multiplayer. **Experimental.**
//!
//! Implements GGPO-style rollback with input prediction, snapshot/restore,
//! and resimulation on mismatch. Nothing in the engine drives it yet, and it
//! has only been tested against the in-memory scenarios below; the supported
//! way to play over the network is [`lockstep`](crate::lockstep).
//!
//! What it guarantees: it never predicts more than `max_rollback_frames`
//! ticks past the last tick with every player's input ([`RollbackError::Stalled`]),
//! and a correction it cannot apply is an error ([`RollbackError::TooLate`],
//! [`RollbackError::SnapshotMissing`]) rather than a silent desync.

use crate::PlayerId;
use crate::checksum::StateHasher;
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
    /// Capacity of the snapshot ring buffer. Raised to
    /// `max_rollback_frames + 2` if smaller, since a rollback needs the
    /// snapshot of the oldest tick it may redo.
    pub snapshot_buffer_size: usize,
}

/// Why [`RollbackSession::advance_tick`] did not advance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RollbackError {
    /// `waiting_for` is more than `max_rollback_frames` ticks behind, so
    /// predicting further would risk a rollback deeper than the window.
    /// Nothing was simulated; call again (with that player's inputs once
    /// they arrive).
    Stalled { waiting_for: PlayerId, tick: u64 },
    /// A correction arrived for `tick`, which is older than the rollback
    /// window. The state can no longer be repaired: treat it as a desync.
    TooLate { tick: u64 },
    /// The snapshot to roll back to (`tick`) was overwritten. Also a desync.
    SnapshotMissing { tick: u64 },
}

impl std::fmt::Display for RollbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stalled { waiting_for, tick } => {
                write!(f, "tick {tick} waits for player {}", waiting_for.0)
            }
            Self::TooLate { tick } => write!(f, "a correction for tick {tick} came too late"),
            Self::SnapshotMissing { tick } => write!(f, "no snapshot of tick {tick} left"),
        }
    }
}

impl std::error::Error for RollbackError {}

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
    /// Remote inputs dropped: from an unknown or the local player, too far
    /// in the future, or contradicting an input already confirmed.
    pub rejected_inputs: u64,
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
    /// Highest tick for which ALL players have confirmed inputs. Everything
    /// at or below this can never be rolled back again, so older bookkeeping
    /// is pruned relative to it.
    last_confirmed_tick: u64,
    /// Ring buffer of snapshots indexed by `tick % snapshot_buffer_size`.
    snapshots: Vec<Option<Vec<u8>>>,
    /// Confirmed (authoritative) inputs per player per tick.
    confirmed_inputs: FxHashMap<PlayerId, FxHashMap<u64, S::Input>>,
    /// Predicted inputs per player per tick.
    predicted_inputs: FxHashMap<PlayerId, FxHashMap<u64, S::Input>>,
    /// Last known input for each remote player (used for prediction).
    /// Each player's input with the highest tick seen so far.
    last_known_input: FxHashMap<PlayerId, (u64, S::Input)>,
    /// Per-tick checksums for desync detection.
    checksums: FxHashMap<u64, u32>,
    /// Runtime statistics.
    stats: RollbackStats,
}

impl<S: RollbackState> RollbackSession<S> {
    /// Create a new rollback session.
    pub fn new(mut config: RollbackConfig, local_player: PlayerId, players: Vec<PlayerId>) -> Self {
        config.snapshot_buffer_size = config
            .snapshot_buffer_size
            .max(config.max_rollback_frames as usize + 2);
        let snapshot_buf = vec![None; config.snapshot_buffer_size];
        let mut confirmed_inputs = FxHashMap::default();
        let mut predicted_inputs = FxHashMap::default();
        let mut last_known_input = FxHashMap::default();
        for &pid in &players {
            confirmed_inputs.insert(pid, FxHashMap::default());
            predicted_inputs.insert(pid, FxHashMap::default());
            last_known_input.insert(pid, (0, S::Input::default()));
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
    ///
    /// The remote inputs are kept even when this returns an error, so a
    /// stalled caller only passes what arrived since.
    pub fn advance_tick(
        &mut self,
        state: &mut S,
        local_input: S::Input,
        remote_inputs: Vec<(PlayerId, u64, S::Input)>,
    ) -> Result<(), RollbackError> {
        let tick = self.current_tick;
        let window = u64::from(self.config.max_rollback_frames);

        // 1. Store local input as confirmed for this tick.
        self.confirmed_inputs
            .entry(self.local_player)
            .or_default()
            .insert(tick, local_input.clone());
        self.remember_input(self.local_player, tick, &local_input);

        // 2. Validate and store remote inputs.
        let mut accepted = Vec::with_capacity(remote_inputs.len());
        for (pid, remote_tick, input) in remote_inputs {
            let known_player = pid != self.local_player && self.players.contains(&pid);
            let in_range = remote_tick <= tick + self.config.snapshot_buffer_size as u64;
            if !known_player || !in_range {
                self.stats.rejected_inputs += 1;
                continue;
            }
            let confirmed = self.confirmed_inputs.entry(pid).or_default();
            match confirmed.get(&remote_tick) {
                // A resend of a tick everyone has confirmed and that was
                // pruned since: nothing to check it against or apply.
                None if remote_tick < self.last_confirmed_tick => {}
                // A resend of what we have.
                Some(existing) if *existing == input => {}
                // Inputs are final once sent; a different one is a bug or
                // a lie.
                Some(_) => self.stats.rejected_inputs += 1,
                None => {
                    confirmed.insert(remote_tick, input.clone());
                    self.remember_input(pid, remote_tick, &input);
                    accepted.push((pid, remote_tick, input));
                }
            }
        }

        // 3. Check for prediction mismatches — find earliest mismatch tick.
        let mut earliest_mismatch: Option<u64> = None;
        for (pid, remote_tick, input) in &accepted {
            if let Some(predicted) = self
                .predicted_inputs
                .get(pid)
                .and_then(|m| m.get(remote_tick))
            {
                self.stats.total_predictions += 1;
                if predicted == input {
                    self.stats.correct_predictions += 1;
                } else {
                    earliest_mismatch =
                        Some(earliest_mismatch.map_or(*remote_tick, |e| e.min(*remote_tick)));
                }
            }
        }

        // 4. If mismatch: roll back and resimulate, or say why not. Rolling
        // back only part of the way would leave the state wrong for good.
        if let Some(mismatch_tick) = earliest_mismatch {
            if mismatch_tick < tick.saturating_sub(window) {
                return Err(RollbackError::TooLate {
                    tick: mismatch_tick,
                });
            }
            let snap_data = self
                .restore_snapshot(mismatch_tick)
                .ok_or(RollbackError::SnapshotMissing {
                    tick: mismatch_tick,
                })?
                .to_vec();
            state.restore(&snap_data);

            let depth = tick.saturating_sub(mismatch_tick) as u32;
            self.stats.rollbacks_this_second += 1;
            self.stats.max_rollback_depth = self.stats.max_rollback_depth.max(depth);

            for resim_tick in mismatch_tick..tick {
                let inputs = self.gather_inputs(resim_tick);
                state.simulate_tick(&inputs);
                // The old hash described the mispredicted state.
                self.record_checksum(state, resim_tick);
                self.save_snapshot(state, resim_tick + 1);
            }
        }

        // 4b. Do not predict deeper than the window: wait for the player
        // whose input is missing longest.
        self.update_confirmed_tick();
        if tick >= self.last_confirmed_tick + window {
            let missing = self.last_confirmed_tick;
            let waiting_for = self
                .players
                .iter()
                .copied()
                .find(|pid| {
                    !self
                        .confirmed_inputs
                        .get(pid)
                        .is_some_and(|m| m.contains_key(&missing))
                })
                .unwrap_or(self.local_player);
            return Err(RollbackError::Stalled { waiting_for, tick });
        }

        // 5. Save snapshot for current tick (before simulating it).
        self.save_snapshot(state, tick);

        // 6. Gather inputs for current tick and simulate.
        let inputs = self.gather_inputs(tick);
        state.simulate_tick(&inputs);

        // 7. Compute and store checksum.
        self.record_checksum(state, tick);

        // 8. Increment current_tick.
        self.current_tick += 1;

        // 9. Update prediction accuracy stat.
        if self.stats.total_predictions > 0 {
            self.stats.prediction_accuracy =
                self.stats.correct_predictions as f32 / self.stats.total_predictions as f32;
        }

        // 10. Advance the confirmation watermark and prune bookkeeping that
        // can never be rolled back to again. Without this, the input and
        // checksum maps grow by one entry per player per tick for the whole
        // session.
        self.update_confirmed_tick();
        self.prune_history();

        // Reset the per-second rollback counter once per second of sim time
        // (the simulation runs at a fixed 60 ticks/sec).
        if self.current_tick.is_multiple_of(60) {
            self.stats.rollbacks_this_second = 0;
        }
        Ok(())
    }

    fn record_checksum(&mut self, state: &S, tick: u64) {
        let mut hasher = StateHasher::new();
        state.checksum(&mut hasher);
        self.checksums.insert(tick, hasher.finish_crc());
    }

    /// The state hash after `tick`, as last simulated (a rollback replaces
    /// it). Compare it with the other players' to detect a desync. Kept
    /// for about two rollback windows past the last confirmed tick.
    pub fn checksum(&self, tick: u64) -> Option<u32> {
        self.checksums.get(&tick).copied()
    }

    /// Predict the input for a remote player (returns last known or default).
    pub fn predict_input(&self, player: PlayerId) -> S::Input {
        self.last_known_input
            .get(&player)
            .map(|(_, input)| input.clone())
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

    /// Keep `input` as `player`'s latest unless a later tick is known: an
    /// old packet arriving late must not become the prediction.
    fn remember_input(&mut self, player: PlayerId, tick: u64, input: &S::Input) {
        let entry = self
            .last_known_input
            .entry(player)
            .or_insert((0, S::Input::default()));
        if tick >= entry.0 {
            *entry = (tick, input.clone());
        }
    }

    /// The prediction for `player` at `tick`: their latest confirmed input
    /// at or before it. An input from a later tick says nothing about this
    /// one.
    fn predict_at(&self, player: PlayerId, tick: u64) -> S::Input {
        let confirmed = self.confirmed_inputs.get(&player).and_then(|m| {
            m.iter()
                .filter(|(t, _)| **t <= tick)
                .max_by_key(|(t, _)| **t)
                .map(|(_, input)| input.clone())
        });
        confirmed
            .or_else(|| {
                self.last_known_input
                    .get(&player)
                    .filter(|(t, _)| *t <= tick)
                    .map(|(_, input)| input.clone())
            })
            .unwrap_or_default()
    }

    /// Advance `last_confirmed_tick` past every tick for which every player
    /// has a confirmed input.
    fn update_confirmed_tick(&mut self) {
        let mut confirmed = self.last_confirmed_tick;
        // Never past the tick being run: an input from the future does not
        // make a tick confirmed before it happens (and with no players at
        // all the scan would never stop).
        'outer: while confirmed <= self.current_tick {
            for pid in &self.players {
                let has_input = self
                    .confirmed_inputs
                    .get(pid)
                    .is_some_and(|m| m.contains_key(&confirmed));
                if !has_input {
                    break 'outer;
                }
            }
            confirmed += 1;
        }
        self.last_confirmed_tick = confirmed;
    }

    /// Drop inputs and checksums older than the confirmation watermark
    /// (minus the rollback window as a safety margin).
    fn prune_history(&mut self) {
        let cutoff = self
            .last_confirmed_tick
            .saturating_sub(self.config.max_rollback_frames as u64 * 2);
        if cutoff == 0 {
            return;
        }
        for map in self.confirmed_inputs.values_mut() {
            map.retain(|&t, _| t >= cutoff);
        }
        for map in self.predicted_inputs.values_mut() {
            map.retain(|&t, _| t >= cutoff);
        }
        self.checksums.retain(|&t, _| t >= cutoff);
    }

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
                let predicted = self.predict_at(pid, tick);
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
    use crate::PlayerId;
    use crate::checksum::StateHasher;

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
            session.advance_tick(&mut state, 1, remote_inputs).unwrap();
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
        session.advance_tick(&mut state, 1, remote_inputs).unwrap();

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
            session.advance_tick(&mut state, 1, remote_inputs).unwrap();
        }
        // state.value = 50 * 3 = 150
        assert_eq!(state.value, 150);

        // Now run ticks 50..60 with NO remote inputs (player 2 predicted as 2,
        // since last known = 2).
        for _tick in 50..60 {
            session.advance_tick(&mut state, 1, vec![]).unwrap();
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
        session.advance_tick(&mut state, 1, corrections).unwrap();

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
            session
                .advance_tick(&mut state, 1, vec![(p(2), tick, 2)])
                .unwrap();
        }
        assert_eq!(session.stats().rollbacks_this_second, 0);

        // Run tick 5 without remote input (prediction kicks in: predicts 2).
        session.advance_tick(&mut state, 1, vec![]).unwrap();

        // Now deliver a mismatched input for tick 5.
        // Tick 6 with correction for tick 5.
        session
            .advance_tick(&mut state, 1, vec![(p(2), 5, 999)])
            .unwrap();

        // Should have recorded a rollback.
        assert!(session.stats().rollbacks_this_second > 0);
    }

    #[test]
    fn snapshot_buffer_wraparound() {
        let config = RollbackConfig {
            max_rollback_frames: 4,
            snapshot_buffer_size: 8,
        };
        let players = vec![p(1)];
        let mut session = RollbackSession::<MockState>::new(config, p(1), players);
        let mut state = MockState::default();

        // Run 20 ticks — buffer wraps around at size 8
        for _tick in 0..20u64 {
            session.advance_tick(&mut state, 1, vec![]).unwrap();
        }

        // Recent snapshot should exist (tick 19 was saved during tick 19's advance)
        assert!(session.restore_snapshot(19).is_some());
        // Old snapshot (tick 3) should be overwritten by a later tick
        assert!(session.restore_snapshot(3).is_none());
    }

    #[test]
    fn stale_snapshot_returns_none() {
        let config = RollbackConfig {
            max_rollback_frames: 4,
            snapshot_buffer_size: 6,
        };
        let players = vec![p(1)];
        let mut session = RollbackSession::<MockState>::new(config, p(1), players);
        let mut state = MockState::default();

        // Tick 0 snapshot saved in slot 0
        session.advance_tick(&mut state, 1, vec![]).unwrap();
        assert!(session.restore_snapshot(0).is_some());

        // Run 6 more ticks — slot 0 now holds tick 6, not tick 0
        for _ in 1..7 {
            session.advance_tick(&mut state, 1, vec![]).unwrap();
        }
        assert!(session.restore_snapshot(0).is_none()); // stale — overwritten
        assert!(session.restore_snapshot(6).is_some()); // current occupant of slot 0
    }

    #[test]
    fn the_snapshot_buffer_covers_the_window() {
        let config = RollbackConfig {
            max_rollback_frames: 8,
            snapshot_buffer_size: 2,
        };
        let session = RollbackSession::<MockState>::new(config, p(1), vec![p(1)]);
        assert_eq!(session.config.snapshot_buffer_size, 10);
    }

    #[test]
    fn contradicting_and_foreign_inputs_are_rejected() {
        let config = RollbackConfig {
            max_rollback_frames: 4,
            snapshot_buffer_size: 4,
        };
        let players = vec![p(1), p(2)];
        let mut session = RollbackSession::<MockState>::new(config, p(1), players);
        let mut state = MockState::default();
        for tick in 0..20u64 {
            session
                .advance_tick(&mut state, 1, vec![(p(2), tick, 2)])
                .unwrap();
        }
        assert_eq!(state.value, 60);

        // Tick 15 was confirmed as 2; a different value used to be applied
        // half-way, leaving the state wrong for good.
        session
            .advance_tick(
                &mut state,
                1,
                vec![
                    (p(2), 15, 999), // contradicts a confirmed input
                    (p(2), 2, 999),  // too old to check, ignored
                    (p(2), 3, 2),    // a harmless resend
                    (p(9), 20, 5),   // nobody in this session
                    (p(1), 20, 5),   // claims to be us
                    (p(2), 900, 5),  // far beyond any snapshot
                    (p(2), 20, 2),   // the real one
                ],
            )
            .unwrap();
        assert_eq!(session.stats().rejected_inputs, 4);
        assert_eq!(state.value, 63);
        assert_eq!(session.current_tick(), 21);
    }

    #[test]
    fn prediction_stops_at_the_window() {
        let config = RollbackConfig {
            max_rollback_frames: 4,
            snapshot_buffer_size: 8,
        };
        let mut session = RollbackSession::<MockState>::new(config, p(1), vec![p(1), p(2)]);
        let mut state = MockState::default();
        session
            .advance_tick(&mut state, 1, vec![(p(2), 0, 2)])
            .unwrap();
        // Ticks 1..5 run on predictions...
        for _ in 1..5 {
            session.advance_tick(&mut state, 1, vec![]).unwrap();
        }
        // ...and the fifth predicted tick waits.
        assert_eq!(
            session.advance_tick(&mut state, 1, vec![]),
            Err(RollbackError::Stalled {
                waiting_for: p(2),
                tick: 5
            })
        );
        assert_eq!(session.current_tick(), 5);
        let before = state.value;
        assert_eq!(
            session.advance_tick(&mut state, 1, vec![]),
            Err(RollbackError::Stalled {
                waiting_for: p(2),
                tick: 5
            })
        );
        assert_eq!(state.value, before, "a stalled call simulates nothing");

        // The late inputs arrive, differ from the prediction, and are
        // rolled back in, all within the window.
        let late = (1..6).map(|t| (p(2), t, 7)).collect();
        session.advance_tick(&mut state, 1, late).unwrap();
        // 2 + 1 for tick 0, then 1 + 7 for ticks 1..=5.
        assert_eq!(state.value, 3 + 5 * 8);
        assert_eq!(session.current_tick(), 6);
    }

    #[test]
    fn checksums_follow_the_rollback() {
        let config = RollbackConfig::default();
        let mut session = RollbackSession::<MockState>::new(config, p(1), vec![p(1), p(2)]);
        let mut state = MockState::default();
        session
            .advance_tick(&mut state, 1, vec![(p(2), 0, 2)])
            .unwrap();
        session.advance_tick(&mut state, 1, vec![]).unwrap(); // tick 1 predicted 2
        let predicted = session.checksum(1).unwrap();
        session
            .advance_tick(&mut state, 1, vec![(p(2), 1, 5)])
            .unwrap();

        let mut expected = StateHasher::new();
        MockState { value: 3 + 6 }.checksum(&mut expected);
        assert_eq!(session.checksum(1), Some(expected.finish_crc()));
        assert_ne!(session.checksum(1), Some(predicted));
    }

    #[test]
    fn no_players_do_not_hang_the_session() {
        let mut session =
            RollbackSession::<MockState>::new(RollbackConfig::default(), p(1), vec![]);
        let mut state = MockState::default();
        for _ in 0..3 {
            session.advance_tick(&mut state, 1, vec![]).unwrap();
        }
        assert_eq!(session.current_tick(), 3);
    }

    #[test]
    fn a_late_old_input_does_not_become_the_prediction() {
        let config = RollbackConfig {
            max_rollback_frames: 8,
            snapshot_buffer_size: 16,
        };
        let mut session = RollbackSession::<MockState>::new(config, p(1), vec![p(1), p(2)]);
        let mut state = MockState::default();
        session
            .advance_tick(&mut state, 1, vec![(p(2), 0, 2)])
            .unwrap();
        // Tick 1's input and, early, tick 3's arrive together.
        session
            .advance_tick(&mut state, 1, vec![(p(2), 1, 2), (p(2), 3, 9)])
            .unwrap();
        // Tick 2 is missing: predicted from tick 1 (2), not from tick 3 (9).
        session.advance_tick(&mut state, 1, vec![]).unwrap();
        session.advance_tick(&mut state, 1, vec![]).unwrap();
        assert_eq!(state.value, 3 + 3 + 3 + 10);
        // Tick 2's input arrives last; it matches the prediction, and it
        // does not replace tick 3's as the newest.
        session
            .advance_tick(&mut state, 1, vec![(p(2), 2, 2)])
            .unwrap();
        assert_eq!(session.predict_input(p(2)), 9);
        assert_eq!(session.stats().rollbacks_this_second, 0);
    }
}
