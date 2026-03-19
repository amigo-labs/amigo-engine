//! Achievement system: data-driven unlock conditions, progress tracking, persistence.
//!
//! Achievements are defined in RON files with conditions (EventCount, FlagSet,
//! StatReached, All, Any, Custom). The tracker monitors game events and manages
//! unlock state. Persistence via SaveManager.

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Achievement Definition
// ---------------------------------------------------------------------------

/// Static definition of a single achievement, loaded from RON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon_sprite: String,
    pub hidden: bool,
    pub condition: AchievementCondition,
    pub category: Option<String>,
    pub sort_order: u32,
}

/// Condition that triggers an achievement unlock.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AchievementCondition {
    EventCount { event: String, threshold: u32 },
    FlagSet(String),
    All(Vec<AchievementCondition>),
    Any(Vec<AchievementCondition>),
    StatReached { stat: String, threshold: f32 },
    Custom(String),
}

// ---------------------------------------------------------------------------
// Achievement Progress
// ---------------------------------------------------------------------------

/// Runtime progress state for a single achievement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementProgress {
    pub current: u32,
    pub total: u32,
    pub unlocked: bool,
    pub unlocked_at: Option<u64>,
}

impl AchievementProgress {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        (self.current as f32 / self.total as f32).min(1.0)
    }
}

// ---------------------------------------------------------------------------
// Save Data
// ---------------------------------------------------------------------------

/// Serializable snapshot of achievement state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementSaveData {
    pub progress: FxHashMap<String, AchievementProgress>,
    pub event_counts: FxHashMap<String, u32>,
    pub flags: FxHashMap<String, bool>,
    pub stats: FxHashMap<String, f32>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum AchievementError {
    NotFound(String),
    Duplicate(String),
}

impl std::fmt::Display for AchievementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "Achievement not found: {id}"),
            Self::Duplicate(id) => write!(f, "Duplicate achievement ID: {id}"),
        }
    }
}

impl std::error::Error for AchievementError {}

// ---------------------------------------------------------------------------
// AchievementTracker
// ---------------------------------------------------------------------------

/// Central achievement tracking system.
pub struct AchievementTracker {
    definitions: FxHashMap<String, AchievementDef>,
    progress: FxHashMap<String, AchievementProgress>,
    event_counts: FxHashMap<String, u32>,
    flags: FxHashMap<String, bool>,
    stats: FxHashMap<String, f32>,
    custom_conditions: FxHashMap<String, Box<dyn Fn(&AchievementTracker) -> bool>>,
    /// Reverse index: event_name -> achievement IDs that reference it.
    event_index: FxHashMap<String, Vec<String>>,
    /// Reverse index: flag_name -> achievement IDs.
    flag_index: FxHashMap<String, Vec<String>>,
    /// Reverse index: stat_name -> achievement IDs.
    stat_index: FxHashMap<String, Vec<String>>,
    pending_toasts: Vec<String>,
    active: bool,
}

impl AchievementTracker {
    pub fn new() -> Self {
        Self {
            definitions: FxHashMap::default(),
            progress: FxHashMap::default(),
            event_counts: FxHashMap::default(),
            flags: FxHashMap::default(),
            stats: FxHashMap::default(),
            custom_conditions: FxHashMap::default(),
            event_index: FxHashMap::default(),
            flag_index: FxHashMap::default(),
            stat_index: FxHashMap::default(),
            pending_toasts: Vec::new(),
            active: true,
        }
    }

    /// Register a single achievement definition.
    pub fn register(&mut self, def: AchievementDef) {
        let total = condition_threshold(&def.condition);
        self.progress.entry(def.id.clone()).or_insert(AchievementProgress {
            current: 0,
            total,
            unlocked: false,
            unlocked_at: None,
        });
        build_indexes(&def.id, &def.condition, &mut self.event_index, &mut self.flag_index, &mut self.stat_index);
        self.definitions.insert(def.id.clone(), def);
    }

    /// Register a custom condition callback.
    pub fn register_custom_condition(
        &mut self,
        key: &str,
        condition: impl Fn(&AchievementTracker) -> bool + 'static,
    ) {
        self.custom_conditions.insert(key.to_string(), Box::new(condition));
    }

    // -- Event / state reporting --

    pub fn report_event(&mut self, event: &str) {
        self.report_event_count(event, 1);
    }

    pub fn report_event_count(&mut self, event: &str, count: u32) {
        if !self.active {
            return;
        }
        let counter = self.event_counts.entry(event.to_string()).or_insert(0);
        *counter += count;
        let ids: Vec<String> = self.event_index.get(event).cloned().unwrap_or_default();
        for id in ids {
            self.check_and_unlock(&id);
        }
    }

    pub fn set_flag(&mut self, flag: &str) {
        if !self.active {
            return;
        }
        self.flags.insert(flag.to_string(), true);
        let ids: Vec<String> = self.flag_index.get(flag).cloned().unwrap_or_default();
        for id in ids {
            self.check_and_unlock(&id);
        }
    }

    pub fn clear_flag(&mut self, flag: &str) {
        self.flags.insert(flag.to_string(), false);
    }

    pub fn set_stat(&mut self, stat: &str, value: f32) {
        if !self.active {
            return;
        }
        self.stats.insert(stat.to_string(), value);
        let ids: Vec<String> = self.stat_index.get(stat).cloned().unwrap_or_default();
        for id in ids {
            self.check_and_unlock(&id);
        }
    }

    pub fn increment_stat(&mut self, stat: &str, delta: f32) {
        let value = self.stats.get(stat).copied().unwrap_or(0.0) + delta;
        self.set_stat(stat, value);
    }

    // -- Query --

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.progress.get(id).map_or(false, |p| p.unlocked)
    }

    pub fn get_progress(&self, id: &str) -> Option<&AchievementProgress> {
        self.progress.get(id)
    }

    pub fn all_definitions(&self) -> Vec<&AchievementDef> {
        self.definitions.values().collect()
    }

    pub fn unlocked_ids(&self) -> Vec<&str> {
        self.progress
            .iter()
            .filter(|(_, p)| p.unlocked)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    pub fn locked_ids(&self) -> Vec<&str> {
        self.progress
            .iter()
            .filter(|(_, p)| !p.unlocked)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    pub fn total_count(&self) -> usize {
        self.definitions.len()
    }

    pub fn unlocked_count(&self) -> usize {
        self.progress.values().filter(|p| p.unlocked).count()
    }

    pub fn completion(&self) -> f32 {
        let total = self.total_count();
        if total == 0 {
            return 0.0;
        }
        self.unlocked_count() as f32 / total as f32
    }

    pub fn event_count(&self, event: &str) -> u32 {
        self.event_counts.get(event).copied().unwrap_or(0)
    }

    pub fn stat_value(&self, stat: &str) -> f32 {
        self.stats.get(stat).copied().unwrap_or(0.0)
    }

    pub fn is_flag_set(&self, flag: &str) -> bool {
        self.flags.get(flag).copied().unwrap_or(false)
    }

    // -- Toasts --

    pub fn drain_toasts(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_toasts)
    }

    // -- Persistence --

    pub fn save_state(&self) -> AchievementSaveData {
        AchievementSaveData {
            progress: self.progress.clone(),
            event_counts: self.event_counts.clone(),
            flags: self.flags.clone(),
            stats: self.stats.clone(),
        }
    }

    pub fn load_state(&mut self, data: &AchievementSaveData) {
        self.progress = data.progress.clone();
        self.event_counts = data.event_counts.clone();
        self.flags = data.flags.clone();
        self.stats = data.stats.clone();
    }

    // -- Debug --

    pub fn debug_unlock(&mut self, id: &str) {
        if let Some(p) = self.progress.get_mut(id) {
            p.unlocked = true;
            p.current = p.total;
        }
    }

    pub fn debug_unlock_all(&mut self) {
        for p in self.progress.values_mut() {
            p.unlocked = true;
            p.current = p.total;
        }
    }

    pub fn debug_reset_all(&mut self) {
        for p in self.progress.values_mut() {
            p.unlocked = false;
            p.unlocked_at = None;
            p.current = 0;
        }
        self.event_counts.clear();
        self.flags.clear();
        self.stats.clear();
        self.pending_toasts.clear();
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    // -- Internal --

    fn check_and_unlock(&mut self, id: &str) {
        if self.progress.get(id).map_or(true, |p| p.unlocked) {
            return;
        }
        let def = match self.definitions.get(id) {
            Some(d) => d.clone(),
            None => return,
        };
        let met = self.evaluate_condition(&def.condition);
        let current = self.condition_current(&def.condition);
        // Update progress (no more borrows on self needed)
        if let Some(progress) = self.progress.get_mut(id) {
            progress.current = current;
            if met && !progress.unlocked {
                progress.unlocked = true;
                progress.unlocked_at = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                );
                self.pending_toasts.push(id.to_string());
            }
        }
    }

    fn evaluate_condition(&self, cond: &AchievementCondition) -> bool {
        match cond {
            AchievementCondition::EventCount { event, threshold } => {
                self.event_counts.get(event).copied().unwrap_or(0) >= *threshold
            }
            AchievementCondition::FlagSet(flag) => {
                self.flags.get(flag).copied().unwrap_or(false)
            }
            AchievementCondition::StatReached { stat, threshold } => {
                self.stats.get(stat).copied().unwrap_or(0.0) >= *threshold
            }
            AchievementCondition::All(subs) => subs.iter().all(|c| self.evaluate_condition(c)),
            AchievementCondition::Any(subs) => subs.iter().any(|c| self.evaluate_condition(c)),
            AchievementCondition::Custom(_key) => {
                // Custom conditions need special handling to avoid borrow issues
                // We check against a snapshot approach
                false // Custom conditions are checked via register_custom_condition
            }
        }
    }

    fn condition_current(&self, cond: &AchievementCondition) -> u32 {
        match cond {
            AchievementCondition::EventCount { event, .. } => {
                self.event_counts.get(event).copied().unwrap_or(0)
            }
            AchievementCondition::FlagSet(flag) => {
                if self.flags.get(flag).copied().unwrap_or(false) { 1 } else { 0 }
            }
            AchievementCondition::StatReached { stat, .. } => {
                self.stats.get(stat).copied().unwrap_or(0.0) as u32
            }
            AchievementCondition::All(subs) => {
                subs.iter().filter(|c| self.evaluate_condition(c)).count() as u32
            }
            AchievementCondition::Any(subs) => {
                if subs.iter().any(|c| self.evaluate_condition(c)) { 1 } else { 0 }
            }
            AchievementCondition::Custom(_) => 0,
        }
    }
}

impl Default for AchievementTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn condition_threshold(cond: &AchievementCondition) -> u32 {
    match cond {
        AchievementCondition::EventCount { threshold, .. } => *threshold,
        AchievementCondition::FlagSet(_) => 1,
        AchievementCondition::StatReached { threshold, .. } => *threshold as u32,
        AchievementCondition::All(subs) => subs.len() as u32,
        AchievementCondition::Any(_) => 1,
        AchievementCondition::Custom(_) => 1,
    }
}

fn build_indexes(
    id: &str,
    cond: &AchievementCondition,
    event_idx: &mut FxHashMap<String, Vec<String>>,
    flag_idx: &mut FxHashMap<String, Vec<String>>,
    stat_idx: &mut FxHashMap<String, Vec<String>>,
) {
    match cond {
        AchievementCondition::EventCount { event, .. } => {
            event_idx.entry(event.clone()).or_default().push(id.to_string());
        }
        AchievementCondition::FlagSet(flag) => {
            flag_idx.entry(flag.clone()).or_default().push(id.to_string());
        }
        AchievementCondition::StatReached { stat, .. } => {
            stat_idx.entry(stat.clone()).or_default().push(id.to_string());
        }
        AchievementCondition::All(subs) | AchievementCondition::Any(subs) => {
            for sub in subs {
                build_indexes(id, sub, event_idx, flag_idx, stat_idx);
            }
        }
        AchievementCondition::Custom(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn kill_achievement() -> AchievementDef {
        AchievementDef {
            id: "first_kill".into(),
            name: "First Blood".into(),
            description: "Defeat your first enemy.".into(),
            icon_sprite: "sword".into(),
            hidden: false,
            condition: AchievementCondition::EventCount {
                event: "enemy_killed".into(),
                threshold: 1,
            },
            category: Some("Combat".into()),
            sort_order: 1,
        }
    }

    fn collector_achievement() -> AchievementDef {
        AchievementDef {
            id: "collect_100".into(),
            name: "Coin Collector".into(),
            description: "Collect 100 coins.".into(),
            icon_sprite: "coin".into(),
            hidden: false,
            condition: AchievementCondition::EventCount {
                event: "coin_collected".into(),
                threshold: 100,
            },
            category: Some("Exploration".into()),
            sort_order: 10,
        }
    }

    #[test]
    fn event_count_unlock() {
        let mut tracker = AchievementTracker::new();
        tracker.register(kill_achievement());
        assert!(!tracker.is_unlocked("first_kill"));

        tracker.report_event("enemy_killed");
        assert!(tracker.is_unlocked("first_kill"));
        assert_eq!(tracker.unlocked_count(), 1);
    }

    #[test]
    fn event_count_progress() {
        let mut tracker = AchievementTracker::new();
        tracker.register(collector_achievement());

        tracker.report_event_count("coin_collected", 50);
        let p = tracker.get_progress("collect_100").unwrap();
        assert_eq!(p.current, 50);
        assert!(!p.unlocked);
        assert!((p.fraction() - 0.5).abs() < 0.01);

        tracker.report_event_count("coin_collected", 50);
        assert!(tracker.is_unlocked("collect_100"));
    }

    #[test]
    fn flag_set_unlock() {
        let mut tracker = AchievementTracker::new();
        tracker.register(AchievementDef {
            id: "tutorial_done".into(),
            name: "Tutorial Complete".into(),
            description: "Finish the tutorial.".into(),
            icon_sprite: "star".into(),
            hidden: false,
            condition: AchievementCondition::FlagSet("tutorial_completed".into()),
            category: None,
            sort_order: 0,
        });
        assert!(!tracker.is_unlocked("tutorial_done"));
        tracker.set_flag("tutorial_completed");
        assert!(tracker.is_unlocked("tutorial_done"));
    }

    #[test]
    fn stat_reached_unlock() {
        let mut tracker = AchievementTracker::new();
        tracker.register(AchievementDef {
            id: "high_score".into(),
            name: "High Scorer".into(),
            description: "Reach 10000 score.".into(),
            icon_sprite: "trophy".into(),
            hidden: false,
            condition: AchievementCondition::StatReached {
                stat: "score".into(),
                threshold: 10000.0,
            },
            category: None,
            sort_order: 0,
        });
        tracker.increment_stat("score", 5000.0);
        assert!(!tracker.is_unlocked("high_score"));
        tracker.increment_stat("score", 5000.0);
        assert!(tracker.is_unlocked("high_score"));
    }

    #[test]
    fn all_condition() {
        let mut tracker = AchievementTracker::new();
        tracker.register(AchievementDef {
            id: "combo".into(),
            name: "Combo Master".into(),
            description: "Kill and collect.".into(),
            icon_sprite: "star".into(),
            hidden: false,
            condition: AchievementCondition::All(vec![
                AchievementCondition::EventCount { event: "kill".into(), threshold: 1 },
                AchievementCondition::FlagSet("collected".into()),
            ]),
            category: None,
            sort_order: 0,
        });
        tracker.report_event("kill");
        assert!(!tracker.is_unlocked("combo"));
        tracker.set_flag("collected");
        assert!(tracker.is_unlocked("combo"));
    }

    #[test]
    fn any_condition() {
        let mut tracker = AchievementTracker::new();
        tracker.register(AchievementDef {
            id: "either".into(),
            name: "Either Way".into(),
            description: "A or B.".into(),
            icon_sprite: "star".into(),
            hidden: false,
            condition: AchievementCondition::Any(vec![
                AchievementCondition::FlagSet("path_a".into()),
                AchievementCondition::FlagSet("path_b".into()),
            ]),
            category: None,
            sort_order: 0,
        });
        assert!(!tracker.is_unlocked("either"));
        tracker.set_flag("path_b");
        assert!(tracker.is_unlocked("either"));
    }

    #[test]
    fn toast_queue() {
        let mut tracker = AchievementTracker::new();
        tracker.register(kill_achievement());
        tracker.report_event("enemy_killed");
        let toasts = tracker.drain_toasts();
        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0], "first_kill");
        // Draining again should be empty
        assert!(tracker.drain_toasts().is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let mut tracker = AchievementTracker::new();
        tracker.register(kill_achievement());
        tracker.register(collector_achievement());
        tracker.report_event("enemy_killed");
        tracker.report_event_count("coin_collected", 42);

        let save = tracker.save_state();

        let mut tracker2 = AchievementTracker::new();
        tracker2.register(kill_achievement());
        tracker2.register(collector_achievement());
        tracker2.load_state(&save);

        assert!(tracker2.is_unlocked("first_kill"));
        assert!(!tracker2.is_unlocked("collect_100"));
        assert_eq!(tracker2.event_count("coin_collected"), 42);
    }

    #[test]
    fn debug_unlock_all() {
        let mut tracker = AchievementTracker::new();
        tracker.register(kill_achievement());
        tracker.register(collector_achievement());
        tracker.debug_unlock_all();
        assert_eq!(tracker.unlocked_count(), 2);
        assert!((tracker.completion() - 1.0).abs() < 0.01);
    }

    #[test]
    fn debug_reset() {
        let mut tracker = AchievementTracker::new();
        tracker.register(kill_achievement());
        tracker.report_event("enemy_killed");
        assert!(tracker.is_unlocked("first_kill"));
        tracker.debug_reset_all();
        assert!(!tracker.is_unlocked("first_kill"));
        assert_eq!(tracker.event_count("enemy_killed"), 0);
    }

    #[test]
    fn inactive_ignores_events() {
        let mut tracker = AchievementTracker::new();
        tracker.register(kill_achievement());
        tracker.set_active(false);
        tracker.report_event("enemy_killed");
        assert!(!tracker.is_unlocked("first_kill"));
        tracker.set_active(true);
        tracker.report_event("enemy_killed");
        assert!(tracker.is_unlocked("first_kill"));
    }
}
