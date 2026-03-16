//! Playtest simulation — headless game running with metrics collection.
//!
//! Runs a simulation without rendering to collect statistics about
//! level difficulty, balance, and player experience.

use crate::heatmap::{Heatmap, HeatmapCollection, HeatmapType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Playtest configuration
// ---------------------------------------------------------------------------

/// Configuration for a playtest simulation run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaytestConfig {
    /// Number of simulation ticks to run.
    pub max_ticks: u64,
    /// Number of repeated runs for statistical averaging.
    pub runs: u32,
    /// Random seed (0 = random each run).
    pub seed: u64,
    /// Stop early if all enemies are dead.
    pub stop_on_victory: bool,
    /// Stop early if all lives are lost.
    pub stop_on_defeat: bool,
}

impl Default for PlaytestConfig {
    fn default() -> Self {
        Self {
            max_ticks: 60 * 60 * 5, // 5 minutes at 60 tps
            runs: 1,
            seed: 0,
            stop_on_victory: true,
            stop_on_defeat: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Playtest metrics
// ---------------------------------------------------------------------------

/// Metrics collected during a single playtest run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlaytestMetrics {
    /// Whether the player won.
    pub victory: bool,
    /// Tick when the simulation ended.
    pub end_tick: u64,
    /// Duration in simulated seconds.
    pub duration_secs: f32,
    /// Total enemies spawned.
    pub enemies_spawned: u32,
    /// Total enemies killed.
    pub enemies_killed: u32,
    /// Total enemies that leaked (reached goal).
    pub enemies_leaked: u32,
    /// Lives remaining at end.
    pub lives_remaining: i32,
    /// Gold remaining at end.
    pub gold_remaining: i32,
    /// Gold earned total.
    pub gold_earned: i32,
    /// Gold spent total.
    pub gold_spent: i32,
    /// Number of towers placed.
    pub towers_placed: u32,
    /// Number of towers upgraded.
    pub towers_upgraded: u32,
    /// Total damage dealt by towers.
    pub total_damage: f64,
    /// Damage dealt per tower type.
    pub damage_by_tower: HashMap<String, f64>,
    /// Kills per tower type.
    pub kills_by_tower: HashMap<String, u32>,
    /// Wave reached (1-indexed).
    pub wave_reached: u32,
    /// Per-wave completion time in ticks.
    pub wave_times: Vec<u64>,
    /// Difficulty rating (computed post-run).
    pub difficulty_rating: f32,
}

impl PlaytestMetrics {
    /// Kill-to-leak ratio (higher = easier for player).
    pub fn kill_ratio(&self) -> f32 {
        let total = self.enemies_killed + self.enemies_leaked;
        if total == 0 {
            return 1.0;
        }
        self.enemies_killed as f32 / total as f32
    }

    /// Gold efficiency (spent vs earned).
    pub fn gold_efficiency(&self) -> f32 {
        if self.gold_earned == 0 {
            return 0.0;
        }
        self.gold_spent as f32 / self.gold_earned as f32
    }

    /// Average wave time in seconds.
    pub fn avg_wave_time_secs(&self) -> f32 {
        if self.wave_times.is_empty() {
            return 0.0;
        }
        let total: u64 = self.wave_times.iter().sum();
        (total as f32 / self.wave_times.len() as f32) / 60.0
    }
}

// ---------------------------------------------------------------------------
// Playtest results (aggregated across runs)
// ---------------------------------------------------------------------------

/// Aggregated results from multiple playtest runs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaytestResults {
    pub config: PlaytestConfig,
    /// Individual run metrics.
    pub runs: Vec<PlaytestMetrics>,
}

impl PlaytestResults {
    pub fn new(config: PlaytestConfig) -> Self {
        Self {
            config,
            runs: Vec::new(),
        }
    }

    /// Add a completed run.
    pub fn add_run(&mut self, metrics: PlaytestMetrics) {
        self.runs.push(metrics);
    }

    /// Win rate across all runs (0.0 - 1.0).
    pub fn win_rate(&self) -> f32 {
        if self.runs.is_empty() {
            return 0.0;
        }
        let wins = self.runs.iter().filter(|r| r.victory).count();
        wins as f32 / self.runs.len() as f32
    }

    /// Average kill ratio across runs.
    pub fn avg_kill_ratio(&self) -> f32 {
        if self.runs.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.runs.iter().map(|r| r.kill_ratio()).sum();
        sum / self.runs.len() as f32
    }

    /// Average duration in seconds.
    pub fn avg_duration_secs(&self) -> f32 {
        if self.runs.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.runs.iter().map(|r| r.duration_secs).sum();
        sum / self.runs.len() as f32
    }

    /// Average wave reached.
    pub fn avg_wave_reached(&self) -> f32 {
        if self.runs.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.runs.iter().map(|r| r.wave_reached).sum();
        sum as f32 / self.runs.len() as f32
    }

    /// Overall difficulty assessment.
    pub fn difficulty_assessment(&self) -> DifficultyRating {
        let wr = self.win_rate();
        let kr = self.avg_kill_ratio();

        if wr >= 0.9 && kr >= 0.95 {
            DifficultyRating::TooEasy
        } else if wr >= 0.6 && kr >= 0.8 {
            DifficultyRating::Easy
        } else if wr >= 0.3 && kr >= 0.6 {
            DifficultyRating::Balanced
        } else if wr >= 0.1 && kr >= 0.4 {
            DifficultyRating::Hard
        } else {
            DifficultyRating::TooHard
        }
    }
}

/// Overall difficulty rating from playtest analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DifficultyRating {
    TooEasy,
    Easy,
    Balanced,
    Hard,
    TooHard,
}

// ---------------------------------------------------------------------------
// Balance suggestions
// ---------------------------------------------------------------------------

/// A suggestion for adjusting game balance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BalanceSuggestion {
    pub category: String,
    pub description: String,
    pub severity: Severity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Analyze playtest results and generate balance suggestions.
pub fn analyze_balance(results: &PlaytestResults) -> Vec<BalanceSuggestion> {
    let mut suggestions = Vec::new();

    let wr = results.win_rate();
    let kr = results.avg_kill_ratio();

    // Win rate analysis
    if wr > 0.95 {
        suggestions.push(BalanceSuggestion {
            category: "difficulty".into(),
            description: format!(
                "Win rate is {:.0}% — level is too easy. Consider adding more enemies or tougher types.",
                wr * 100.0
            ),
            severity: Severity::Warning,
        });
    } else if wr < 0.1 {
        suggestions.push(BalanceSuggestion {
            category: "difficulty".into(),
            description: format!(
                "Win rate is {:.0}% — level is too hard. Consider reducing enemy HP or giving more starting gold.",
                wr * 100.0
            ),
            severity: Severity::Warning,
        });
    }

    // Kill ratio analysis
    if kr < 0.5 {
        suggestions.push(BalanceSuggestion {
            category: "tower_power".into(),
            description: "Over half the enemies are leaking. Towers may be too weak or too expensive.".into(),
            severity: Severity::Critical,
        });
    }

    // Economy analysis
    for run in &results.runs {
        if run.gold_efficiency() < 0.3 && run.victory {
            suggestions.push(BalanceSuggestion {
                category: "economy".into(),
                description: "Player is winning without spending much gold. Starting gold may be too high or towers too cheap.".into(),
                severity: Severity::Info,
            });
            break;
        }
    }

    // Per-tower analysis
    let mut tower_usage: HashMap<String, u32> = HashMap::new();
    for run in &results.runs {
        for (tower, &kills) in &run.kills_by_tower {
            *tower_usage.entry(tower.clone()).or_default() += kills;
        }
    }

    // Find underperforming towers
    if tower_usage.len() >= 2 {
        let total_kills: u32 = tower_usage.values().sum();
        let avg_per_tower = total_kills as f32 / tower_usage.len() as f32;

        for (tower, &kills) in &tower_usage {
            if (kills as f32) < avg_per_tower * 0.2 {
                suggestions.push(BalanceSuggestion {
                    category: "tower_balance".into(),
                    description: format!(
                        "Tower '{}' has very few kills ({:.0}% of average). May need a buff.",
                        tower,
                        kills as f32 / avg_per_tower * 100.0
                    ),
                    severity: Severity::Info,
                });
            }
        }
    }

    suggestions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metrics(victory: bool, kills: u32, leaks: u32) -> PlaytestMetrics {
        PlaytestMetrics {
            victory,
            end_tick: 3600,
            duration_secs: 60.0,
            enemies_spawned: kills + leaks,
            enemies_killed: kills,
            enemies_leaked: leaks,
            lives_remaining: if victory { 10 } else { 0 },
            gold_remaining: 100,
            gold_earned: 500,
            gold_spent: 400,
            towers_placed: 5,
            towers_upgraded: 2,
            total_damage: 1000.0,
            damage_by_tower: HashMap::new(),
            kills_by_tower: HashMap::new(),
            wave_reached: 10,
            wave_times: vec![300, 350, 400],
            difficulty_rating: 0.5,
        }
    }

    #[test]
    fn kill_ratio() {
        let m = sample_metrics(true, 90, 10);
        assert!((m.kill_ratio() - 0.9).abs() < 0.001);
    }

    #[test]
    fn gold_efficiency() {
        let m = sample_metrics(true, 100, 0);
        assert!((m.gold_efficiency() - 0.8).abs() < 0.001);
    }

    #[test]
    fn win_rate() {
        let mut results = PlaytestResults::new(PlaytestConfig::default());
        results.add_run(sample_metrics(true, 100, 0));
        results.add_run(sample_metrics(true, 90, 10));
        results.add_run(sample_metrics(false, 50, 50));

        assert!((results.win_rate() - 0.6667).abs() < 0.01);
    }

    #[test]
    fn difficulty_assessment_balanced() {
        let mut results = PlaytestResults::new(PlaytestConfig::default());
        for _ in 0..5 {
            results.add_run(sample_metrics(true, 80, 20));
        }
        for _ in 0..5 {
            results.add_run(sample_metrics(false, 50, 50));
        }
        assert_eq!(results.difficulty_assessment(), DifficultyRating::Balanced);
    }

    #[test]
    fn difficulty_assessment_too_easy() {
        let mut results = PlaytestResults::new(PlaytestConfig::default());
        for _ in 0..10 {
            results.add_run(sample_metrics(true, 100, 0));
        }
        assert_eq!(results.difficulty_assessment(), DifficultyRating::TooEasy);
    }

    #[test]
    fn difficulty_assessment_too_hard() {
        let mut results = PlaytestResults::new(PlaytestConfig::default());
        for _ in 0..10 {
            results.add_run(sample_metrics(false, 20, 80));
        }
        assert_eq!(results.difficulty_assessment(), DifficultyRating::TooHard);
    }

    #[test]
    fn balance_suggestions_too_easy() {
        let mut results = PlaytestResults::new(PlaytestConfig::default());
        for _ in 0..10 {
            results.add_run(sample_metrics(true, 100, 0));
        }
        let suggestions = analyze_balance(&results);
        assert!(suggestions.iter().any(|s| s.category == "difficulty"));
    }

    #[test]
    fn balance_suggestions_tower_underperforming() {
        let mut results = PlaytestResults::new(PlaytestConfig::default());
        let mut m = sample_metrics(true, 100, 0);
        m.kills_by_tower.insert("arrow".into(), 80);
        m.kills_by_tower.insert("cannon".into(), 18);
        m.kills_by_tower.insert("ice".into(), 2); // underperforming
        results.add_run(m);

        let suggestions = analyze_balance(&results);
        assert!(suggestions
            .iter()
            .any(|s| s.category == "tower_balance" && s.description.contains("ice")));
    }

    #[test]
    fn avg_wave_time() {
        let m = PlaytestMetrics {
            wave_times: vec![600, 900, 1200],
            ..Default::default()
        };
        // (600+900+1200) / 3 / 60 = 15.0 seconds
        assert!((m.avg_wave_time_secs() - 15.0).abs() < 0.1);
    }
}
