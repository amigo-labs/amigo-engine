//! Metrics collection stubs for AI evaluation during headless simulation.
//!
//! The engine records lightweight metrics (death locations, completion times,
//! resource usage) that AI agents can query to evaluate game balance and
//! quality.  See ADR-0013 §3 "AI Evaluation System".

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Core metrics collector
// ---------------------------------------------------------------------------

/// Collects gameplay metrics during a simulation run.
///
/// Designed to be stored in [`amigo_core::Resources`] and updated by game
/// systems each tick.  AI agents retrieve the snapshot via the
/// `amigo_metrics_snapshot` JSON-RPC method.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MetricsCollector {
    /// Positions where the player died `[x, y]`.
    pub death_positions: Vec<[f32; 2]>,

    /// Per-level completion times in seconds.
    pub completion_times: HashMap<String, f32>,

    /// Tick-level resource usage samples (CPU-time per tick in microseconds).
    pub tick_durations_us: Vec<u64>,

    /// Arbitrary counters keyed by name (e.g. "enemies_killed", "items_used").
    pub counters: HashMap<String, u64>,

    /// Whether collection is currently enabled.
    pub enabled: bool,
}

impl MetricsCollector {
    /// Create a new, enabled collector.
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Record a player death at the given world position.
    pub fn record_death(&mut self, x: f32, y: f32) {
        if self.enabled {
            self.death_positions.push([x, y]);
        }
    }

    /// Record level completion time.
    pub fn record_completion(&mut self, level: impl Into<String>, seconds: f32) {
        if self.enabled {
            self.completion_times.insert(level.into(), seconds);
        }
    }

    /// Record a single tick's duration in microseconds.
    pub fn record_tick_duration(&mut self, us: u64) {
        if self.enabled {
            self.tick_durations_us.push(us);
        }
    }

    /// Increment a named counter by `delta`.
    pub fn increment(&mut self, name: impl Into<String>, delta: u64) {
        if self.enabled {
            *self.counters.entry(name.into()).or_default() += delta;
        }
    }

    /// Reset all collected data.
    pub fn clear(&mut self) {
        self.death_positions.clear();
        self.completion_times.clear();
        self.tick_durations_us.clear();
        self.counters.clear();
    }

    /// Produce a JSON snapshot for the API layer.
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_snapshot() {
        let mut m = MetricsCollector::new();
        m.record_death(10.0, 20.0);
        m.record_completion("world1-1", 42.5);
        m.record_tick_duration(1200);
        m.increment("enemies_killed", 3);

        let snap = m.snapshot();
        assert_eq!(snap["death_positions"][0][0], 10.0);
        assert_eq!(snap["completion_times"]["world1-1"], 42.5);
        assert_eq!(snap["tick_durations_us"][0], 1200);
        assert_eq!(snap["counters"]["enemies_killed"], 3);
    }

    #[test]
    fn disabled_collector_ignores_events() {
        let mut m = MetricsCollector::new();
        m.enabled = false;
        m.record_death(1.0, 2.0);
        m.increment("x", 1);
        assert!(m.death_positions.is_empty());
        assert!(m.counters.is_empty());
    }

    #[test]
    fn clear_resets_everything() {
        let mut m = MetricsCollector::new();
        m.record_death(0.0, 0.0);
        m.increment("a", 1);
        m.clear();
        assert!(m.death_positions.is_empty());
        assert!(m.counters.is_empty());
    }
}
