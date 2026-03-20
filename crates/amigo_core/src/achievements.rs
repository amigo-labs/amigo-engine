//! Achievement system: data-driven unlock conditions, progress tracking, persistence.
//!
//! Achievements are defined in RON files with conditions (EventCount, FlagSet,
//! StatReached, All, Any, Custom). The tracker monitors game events and manages
//! unlock state. Persistence via SaveManager.

use std::path::Path;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::rect::Rect;

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
    Io(std::io::Error),
    Parse(String),
    NotFound(String),
    Duplicate(String),
}

impl std::fmt::Display for AchievementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error loading definitions: {e}"),
            Self::Parse(msg) => write!(f, "RON parse error: {msg}"),
            Self::NotFound(id) => write!(f, "Achievement not found: {id}"),
            Self::Duplicate(id) => write!(f, "Duplicate achievement ID: {id}"),
        }
    }
}

impl std::error::Error for AchievementError {}

impl From<std::io::Error> for AchievementError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// AchievementTracker
// ---------------------------------------------------------------------------

/// Central achievement tracking system.
#[allow(clippy::type_complexity)]
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

    // -----------------------------------------------------------------------
    // Definition loading
    // -----------------------------------------------------------------------

    /// Load achievement definitions from a RON file.
    /// Initializes progress entries for all defined achievements.
    pub fn load_definitions(&mut self, path: &Path) -> Result<(), AchievementError> {
        let content = std::fs::read_to_string(path)?;
        let defs: Vec<AchievementDef> =
            ron::from_str(&content).map_err(|e| AchievementError::Parse(e.to_string()))?;
        for def in defs {
            if self.definitions.contains_key(&def.id) {
                return Err(AchievementError::Duplicate(def.id));
            }
            self.register(def);
        }
        Ok(())
    }

    /// Register a single achievement definition programmatically.
    pub fn register(&mut self, def: AchievementDef) {
        let total = condition_threshold(&def.condition);
        self.progress
            .entry(def.id.clone())
            .or_insert(AchievementProgress {
                current: 0,
                total,
                unlocked: false,
                unlocked_at: None,
            });
        build_indexes(
            &def.id,
            &def.condition,
            &mut self.event_index,
            &mut self.flag_index,
            &mut self.stat_index,
        );
        self.definitions.insert(def.id.clone(), def);
    }

    /// Register a custom condition callback.
    pub fn register_custom_condition(
        &mut self,
        key: &str,
        condition: impl Fn(&AchievementTracker) -> bool + 'static,
    ) {
        self.custom_conditions
            .insert(key.to_string(), Box::new(condition));
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
        self.progress.get(id).is_some_and(|p| p.unlocked)
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

    /// Print all achievements and their progress to the log.
    pub fn debug_list(&self) {
        let mut defs: Vec<&AchievementDef> = self.definitions.values().collect();
        defs.sort_by(|a, b| a.sort_order.cmp(&b.sort_order).then(a.id.cmp(&b.id)));
        for def in defs {
            let progress = self.progress.get(&def.id);
            let (current, total, unlocked) = match progress {
                Some(p) => (p.current, p.total, p.unlocked),
                None => (0, 0, false),
            };
            let status = if unlocked { "UNLOCKED" } else { "locked" };
            eprintln!(
                "[achievement] {} ({}) - {} [{}/{}] [{}]",
                def.id, def.name, def.description, current, total, status,
            );
        }
        eprintln!(
            "[achievement] Total: {}/{} ({:.0}%)",
            self.unlocked_count(),
            self.total_count(),
            self.completion() * 100.0,
        );
    }

    /// Enable or disable tracking. When disabled, `report_event` / `set_flag`
    /// calls are ignored.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    // -- Internal --

    fn check_and_unlock(&mut self, id: &str) {
        if self.progress.get(id).is_none_or(|p| p.unlocked) {
            return;
        }
        let def = match self.definitions.get(id) {
            Some(d) => d.clone(),
            None => return,
        };
        // Evaluate custom conditions separately to avoid borrow issues:
        // we need to temporarily take the callbacks out of self, evaluate,
        // then put them back.
        let custom_results = self.snapshot_custom_results(&def.condition);
        let met = Self::evaluate_condition_with(
            &def.condition,
            &self.event_counts,
            &self.flags,
            &self.stats,
            &custom_results,
        );
        let current = Self::condition_current_with(
            &def.condition,
            &self.event_counts,
            &self.flags,
            &self.stats,
            &custom_results,
        );
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

    /// Snapshot the results of all custom conditions referenced by `cond`.
    /// This evaluates callback functions while `&self` is still available,
    /// producing a map of key -> bool that the static evaluator can use.
    fn snapshot_custom_results(
        &self,
        cond: &AchievementCondition,
    ) -> FxHashMap<String, bool> {
        let mut results = FxHashMap::default();
        Self::collect_custom_keys(cond, &mut results);
        for (key, val) in results.iter_mut() {
            if let Some(cb) = self.custom_conditions.get(key) {
                *val = cb(self);
            }
        }
        results
    }

    fn collect_custom_keys(cond: &AchievementCondition, out: &mut FxHashMap<String, bool>) {
        match cond {
            AchievementCondition::Custom(key) => {
                out.insert(key.clone(), false);
            }
            AchievementCondition::All(subs) | AchievementCondition::Any(subs) => {
                for sub in subs {
                    Self::collect_custom_keys(sub, out);
                }
            }
            _ => {}
        }
    }

    fn evaluate_condition_with(
        cond: &AchievementCondition,
        event_counts: &FxHashMap<String, u32>,
        flags: &FxHashMap<String, bool>,
        stats: &FxHashMap<String, f32>,
        custom_results: &FxHashMap<String, bool>,
    ) -> bool {
        match cond {
            AchievementCondition::EventCount { event, threshold } => {
                event_counts.get(event).copied().unwrap_or(0) >= *threshold
            }
            AchievementCondition::FlagSet(flag) => flags.get(flag).copied().unwrap_or(false),
            AchievementCondition::StatReached { stat, threshold } => {
                stats.get(stat).copied().unwrap_or(0.0) >= *threshold
            }
            AchievementCondition::All(subs) => subs
                .iter()
                .all(|c| Self::evaluate_condition_with(c, event_counts, flags, stats, custom_results)),
            AchievementCondition::Any(subs) => subs
                .iter()
                .any(|c| Self::evaluate_condition_with(c, event_counts, flags, stats, custom_results)),
            AchievementCondition::Custom(key) => {
                custom_results.get(key).copied().unwrap_or(false)
            }
        }
    }

    fn condition_current_with(
        cond: &AchievementCondition,
        event_counts: &FxHashMap<String, u32>,
        flags: &FxHashMap<String, bool>,
        stats: &FxHashMap<String, f32>,
        custom_results: &FxHashMap<String, bool>,
    ) -> u32 {
        match cond {
            AchievementCondition::EventCount { event, .. } => {
                event_counts.get(event).copied().unwrap_or(0)
            }
            AchievementCondition::FlagSet(flag) => {
                if flags.get(flag).copied().unwrap_or(false) {
                    1
                } else {
                    0
                }
            }
            AchievementCondition::StatReached { stat, .. } => {
                stats.get(stat).copied().unwrap_or(0.0) as u32
            }
            AchievementCondition::All(subs) => subs
                .iter()
                .filter(|c| {
                    Self::evaluate_condition_with(c, event_counts, flags, stats, custom_results)
                })
                .count() as u32,
            AchievementCondition::Any(subs) => {
                if subs
                    .iter()
                    .any(|c| Self::evaluate_condition_with(c, event_counts, flags, stats, custom_results))
                {
                    1
                } else {
                    0
                }
            }
            AchievementCondition::Custom(key) => {
                if custom_results.get(key).copied().unwrap_or(false) {
                    1
                } else {
                    0
                }
            }
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
            event_idx
                .entry(event.clone())
                .or_default()
                .push(id.to_string());
        }
        AchievementCondition::FlagSet(flag) => {
            flag_idx
                .entry(flag.clone())
                .or_default()
                .push(id.to_string());
        }
        AchievementCondition::StatReached { stat, .. } => {
            stat_idx
                .entry(stat.clone())
                .or_default()
                .push(id.to_string());
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
// Toast Notification Renderer
// ---------------------------------------------------------------------------

/// Phase of a toast animation.
#[derive(Clone, Copy, Debug)]
enum ToastPhase {
    FadeIn,
    Hold,
    FadeOut,
}

/// State for a single on-screen toast notification.
struct ToastState {
    achievement_id: String,
    elapsed: f32,
    phase: ToastPhase,
}

/// Renders achievement toast popups. Manages a display queue with timed
/// fade-in / hold / fade-out animation. Up to 3 toasts are visible at once.
pub struct AchievementToastRenderer {
    /// Currently displaying toasts (max 3 visible at once).
    active_toasts: Vec<ToastState>,
    /// Waiting queue when more than 3 toasts are pending.
    queued: Vec<String>,
    /// Duration each toast is visible (hold phase) in seconds.
    display_duration: f32,
    /// Fade-in/out duration in seconds.
    fade_duration: f32,
}

/// Maximum number of toasts visible simultaneously.
const MAX_VISIBLE_TOASTS: usize = 3;
/// Default hold duration in seconds.
const DEFAULT_DISPLAY_DURATION: f32 = 4.0;
/// Default fade duration in seconds.
const DEFAULT_FADE_DURATION: f32 = 0.5;
/// Toast panel width in pixels.
const TOAST_WIDTH: f32 = 280.0;
/// Toast panel height in pixels.
const TOAST_HEIGHT: f32 = 64.0;
/// Vertical spacing between stacked toasts.
const TOAST_SPACING: f32 = 8.0;
/// Horizontal margin from the right edge of the screen.
const TOAST_MARGIN_RIGHT: f32 = 16.0;
/// Vertical margin from the top of the screen.
const TOAST_MARGIN_TOP: f32 = 16.0;
/// Padding inside the toast panel.
const TOAST_PADDING: f32 = 8.0;
/// Size reserved for the achievement icon.
const TOAST_ICON_SIZE: f32 = 48.0;

impl AchievementToastRenderer {
    pub fn new() -> Self {
        Self {
            active_toasts: Vec::new(),
            queued: Vec::new(),
            display_duration: DEFAULT_DISPLAY_DURATION,
            fade_duration: DEFAULT_FADE_DURATION,
        }
    }

    /// Queue a toast for an unlocked achievement.
    pub fn queue(&mut self, achievement_id: String) {
        if self.active_toasts.len() < MAX_VISIBLE_TOASTS {
            self.active_toasts.push(ToastState {
                achievement_id,
                elapsed: 0.0,
                phase: ToastPhase::FadeIn,
            });
        } else {
            self.queued.push(achievement_id);
        }
    }

    /// Update toast timers and transitions. Call once per frame.
    pub fn update(&mut self, dt: f32) {
        let fade = self.fade_duration;
        let hold = self.display_duration;

        // Update each active toast.
        for toast in &mut self.active_toasts {
            toast.elapsed += dt;
            match toast.phase {
                ToastPhase::FadeIn => {
                    if toast.elapsed >= fade {
                        toast.elapsed -= fade;
                        toast.phase = ToastPhase::Hold;
                    }
                }
                ToastPhase::Hold => {
                    if toast.elapsed >= hold {
                        toast.elapsed -= hold;
                        toast.phase = ToastPhase::FadeOut;
                    }
                }
                ToastPhase::FadeOut => {
                    // Removal handled below.
                }
            }
        }

        // Remove finished toasts (fade-out elapsed).
        self.active_toasts
            .retain(|t| !matches!(t.phase, ToastPhase::FadeOut if t.elapsed >= fade));

        // Promote from queue.
        while self.active_toasts.len() < MAX_VISIBLE_TOASTS {
            if let Some(id) = self.queued.pop() {
                self.active_toasts.push(ToastState {
                    achievement_id: id,
                    elapsed: 0.0,
                    phase: ToastPhase::FadeIn,
                });
            } else {
                break;
            }
        }
    }

    /// Render active toasts. Draws icon + name + description in a small panel
    /// sliding in from the top-right corner.
    ///
    /// Because `UiContext` lives in `amigo_ui` (which depends on `amigo_core`),
    /// rendering is done through a callback. The callback receives
    /// `(rect, alpha, icon_sprite, name, description)` for each visible toast.
    ///
    /// Example integration with `UiContext`:
    /// ```ignore
    /// toast_renderer.draw_with(&tracker, screen_width, |rect, alpha, icon, name, desc| {
    ///     let bg = Color::new(0, 0, 0, (alpha * 200.0) as u8);
    ///     ui.filled_rect(rect, bg);
    ///     ui.sprite(icon, rect.x + 8.0, rect.y + 8.0);
    ///     let text_color = Color::new(255, 255, 255, (alpha * 255.0) as u8);
    ///     ui.pixel_text(name, rect.x + 64.0, rect.y + 8.0, text_color);
    ///     ui.pixel_text(desc, rect.x + 64.0, rect.y + 28.0, text_color);
    /// });
    /// ```
    pub fn draw_with(
        &self,
        tracker: &AchievementTracker,
        screen_width: f32,
        mut draw_fn: impl FnMut(Rect, f32, &str, &str, &str),
    ) {
        for (i, toast) in self.active_toasts.iter().enumerate() {
            let alpha = self.toast_alpha(toast);
            if alpha <= 0.0 {
                continue;
            }
            let def = match tracker.definitions.get(&toast.achievement_id) {
                Some(d) => d,
                None => continue,
            };
            let x = screen_width - TOAST_WIDTH - TOAST_MARGIN_RIGHT;
            let y =
                TOAST_MARGIN_TOP + (i as f32) * (TOAST_HEIGHT + TOAST_SPACING);
            let rect = Rect::new(x, y, TOAST_WIDTH, TOAST_HEIGHT);
            draw_fn(rect, alpha, &def.icon_sprite, &def.name, &def.description);
        }
    }

    /// Convenience method that renders toasts with a default visual style.
    /// Uses `filled_rect` for background, `sprite` for icon, and `pixel_text`
    /// for name/description. Accepts closures matching the `UiContext` API.
    pub fn draw_default(
        &self,
        tracker: &AchievementTracker,
        screen_width: f32,
        mut filled_rect_fn: impl FnMut(Rect, Color),
        mut sprite_fn: impl FnMut(&str, f32, f32),
        mut text_fn: impl FnMut(&str, f32, f32, Color),
    ) {
        self.draw_with(tracker, screen_width, |rect, alpha, icon, name, desc| {
            let a = alpha * (200.0 / 255.0);
            let bg = Color::new(0.0, 0.0, 0.0, a);
            filled_rect_fn(rect, bg);
            sprite_fn(icon, rect.x + TOAST_PADDING, rect.y + TOAST_PADDING);
            let text_x = rect.x + TOAST_PADDING + TOAST_ICON_SIZE + TOAST_PADDING;
            let text_a = alpha;
            let text_color = Color::new(1.0, 1.0, 1.0, text_a);
            text_fn(name, text_x, rect.y + TOAST_PADDING, text_color);
            text_fn(
                desc,
                text_x,
                rect.y + TOAST_PADDING + 20.0,
                text_color,
            );
        });
    }

    fn toast_alpha(&self, toast: &ToastState) -> f32 {
        match toast.phase {
            ToastPhase::FadeIn => (toast.elapsed / self.fade_duration).min(1.0),
            ToastPhase::Hold => 1.0,
            ToastPhase::FadeOut => {
                (1.0 - toast.elapsed / self.fade_duration).max(0.0)
            }
        }
    }
}

impl Default for AchievementToastRenderer {
    fn default() -> Self {
        Self::new()
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
                AchievementCondition::EventCount {
                    event: "kill".into(),
                    threshold: 1,
                },
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
