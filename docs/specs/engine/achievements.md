---
status: spec
crate: amigo_core
depends_on: ["engine/save-load", "engine/ui"]
last_updated: 2026-03-18
---

# Achievement System

## Purpose

In-game achievement system that defines unlock conditions in RON data files,
monitors game events and state changes at runtime, tracks progress toward
multi-step achievements, persists unlocked achievements via the SaveManager,
and displays toast notifications when achievements unlock. Supports optional
Steam integration behind a feature flag. Designed to be data-driven so that
new achievements can be added without code changes.

## Public API

### Achievement Definition

```rust
use rustc_hash::FxHashMap;

/// Static definition of a single achievement, loaded from RON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementDef {
    /// Unique string identifier (e.g., "first_kill", "collect_100_coins").
    pub id: String,
    /// Display name shown in the UI.
    pub name: String,
    /// Description text shown in the UI.
    pub description: String,
    /// Sprite key for the achievement icon (rendered by the UI system).
    pub icon_sprite: String,
    /// Whether this achievement is hidden until unlocked.
    pub hidden: bool,
    /// Condition that must be met to unlock this achievement.
    pub condition: AchievementCondition,
    /// Optional category for grouping in the achievement list UI.
    pub category: Option<String>,
    /// Sort order within its category.
    pub sort_order: u32,
}

/// Condition that triggers an achievement unlock.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AchievementCondition {
    /// Unlocks when a named event has been counted `threshold` times.
    /// Example: EventCount { event: "enemy_killed", threshold: 100 }
    EventCount {
        event: String,
        threshold: u32,
    },

    /// Unlocks when a boolean flag is set to true.
    /// Flags are game-defined strings tracked by the AchievementTracker.
    FlagSet(String),

    /// Unlocks when multiple conditions are ALL met simultaneously.
    All(Vec<AchievementCondition>),

    /// Unlocks when ANY of the sub-conditions is met.
    Any(Vec<AchievementCondition>),

    /// Unlocks when a numeric stat reaches a threshold.
    /// Stats are game-defined named f32 values.
    StatReached {
        stat: String,
        threshold: f32,
    },

    /// Custom condition evaluated by a registered callback function.
    /// The string is a key into the custom condition registry.
    Custom(String),
}
```

### Achievement Progress

```rust
/// Runtime progress state for a single achievement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementProgress {
    /// Current progress count (for EventCount / StatReached conditions).
    pub current: u32,
    /// Target count required for unlock.
    pub total: u32,
    /// Whether this achievement has been unlocked.
    pub unlocked: bool,
    /// Timestamp (seconds since epoch) when unlocked, if applicable.
    pub unlocked_at: Option<u64>,
}

impl AchievementProgress {
    /// Fraction complete as [0.0, 1.0].
    pub fn fraction(&self) -> f32 {
        if self.total == 0 { return 1.0; }
        (self.current as f32 / self.total as f32).min(1.0)
    }
}
```

### AchievementTracker

```rust
/// Central achievement tracking system. Monitors events, checks conditions,
/// and manages unlock state.
pub struct AchievementTracker {
    /// All achievement definitions, keyed by ID.
    definitions: FxHashMap<String, AchievementDef>,
    /// Runtime progress for each achievement.
    progress: FxHashMap<String, AchievementProgress>,
    /// Event counters: event_name -> cumulative count.
    event_counts: FxHashMap<String, u32>,
    /// Boolean flags: flag_name -> is_set.
    flags: FxHashMap<String, bool>,
    /// Numeric stats: stat_name -> current value.
    stats: FxHashMap<String, f32>,
    /// Custom condition callbacks: key -> evaluation function.
    custom_conditions: FxHashMap<String, Box<dyn Fn(&AchievementTracker) -> bool>>,
    /// Queue of achievements that just unlocked this frame (for toast display).
    pending_toasts: Vec<String>,
    /// Whether tracking is active (disabled during rewind, menus, etc.).
    active: bool,
}

impl AchievementTracker {
    pub fn new() -> Self;

    // -----------------------------------------------------------------------
    // Definition loading
    // -----------------------------------------------------------------------

    /// Load achievement definitions from a RON file.
    /// Initializes progress entries for all defined achievements.
    pub fn load_definitions(&mut self, path: &Path) -> Result<(), AchievementError>;

    /// Register a single achievement definition programmatically.
    pub fn register(&mut self, def: AchievementDef);

    /// Register a custom condition callback.
    pub fn register_custom_condition(
        &mut self,
        key: &str,
        condition: impl Fn(&AchievementTracker) -> bool + 'static,
    );

    // -----------------------------------------------------------------------
    // Event / state reporting (call from game code)
    // -----------------------------------------------------------------------

    /// Report that a named event occurred. Increments the event counter
    /// and checks all achievements with EventCount conditions for this event.
    pub fn report_event(&mut self, event: &str);

    /// Report that a named event occurred N times at once.
    pub fn report_event_count(&mut self, event: &str, count: u32);

    /// Set a boolean flag. Checks achievements with FlagSet conditions.
    pub fn set_flag(&mut self, flag: &str);

    /// Clear a boolean flag (does not un-unlock achievements).
    pub fn clear_flag(&mut self, flag: &str);

    /// Set a numeric stat value. Checks achievements with StatReached conditions.
    pub fn set_stat(&mut self, stat: &str, value: f32);

    /// Increment a numeric stat by a delta.
    pub fn increment_stat(&mut self, stat: &str, delta: f32);

    // -----------------------------------------------------------------------
    // Query
    // -----------------------------------------------------------------------

    /// Check if a specific achievement is unlocked.
    pub fn is_unlocked(&self, id: &str) -> bool;

    /// Get the progress for a specific achievement.
    pub fn get_progress(&self, id: &str) -> Option<&AchievementProgress>;

    /// List all achievement definitions.
    pub fn all_definitions(&self) -> Vec<&AchievementDef>;

    /// List all unlocked achievement IDs.
    pub fn unlocked_ids(&self) -> Vec<&str>;

    /// List all locked achievement IDs.
    pub fn locked_ids(&self) -> Vec<&str>;

    /// Total number of achievements defined.
    pub fn total_count(&self) -> usize;

    /// Number of unlocked achievements.
    pub fn unlocked_count(&self) -> usize;

    /// Overall completion percentage [0.0, 1.0].
    pub fn completion(&self) -> f32;

    /// Get the current value of a named event counter.
    pub fn event_count(&self, event: &str) -> u32;

    /// Get the current value of a named stat.
    pub fn stat_value(&self, stat: &str) -> f32;

    /// Check if a flag is set.
    pub fn is_flag_set(&self, flag: &str) -> bool;

    // -----------------------------------------------------------------------
    // Toast management
    // -----------------------------------------------------------------------

    /// Drain the pending toast queue. Call once per frame to get the list
    /// of achievements that unlocked since the last drain.
    pub fn drain_toasts(&mut self) -> Vec<String>;

    // -----------------------------------------------------------------------
    // Persistence
    // -----------------------------------------------------------------------

    /// Serialize achievement state (progress, counters, flags, stats) for saving.
    /// Returns a serializable snapshot.
    pub fn save_state(&self) -> AchievementSaveData;

    /// Restore achievement state from a loaded save.
    pub fn load_state(&mut self, data: &AchievementSaveData);

    // -----------------------------------------------------------------------
    // Debug
    // -----------------------------------------------------------------------

    /// Force-unlock an achievement regardless of conditions (debug only).
    pub fn debug_unlock(&mut self, id: &str);

    /// Force-unlock all achievements (debug only).
    pub fn debug_unlock_all(&mut self);

    /// Reset all progress and unlocks (debug only).
    pub fn debug_reset_all(&mut self);

    /// Print all achievements and their progress to the log.
    pub fn debug_list(&self);

    /// Enable or disable tracking. When disabled, report_event / set_flag
    /// calls are ignored.
    pub fn set_active(&mut self, active: bool);
}
```

### Save Data

```rust
/// Serializable snapshot of achievement state for persistence via SaveManager.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementSaveData {
    pub progress: FxHashMap<String, AchievementProgress>,
    pub event_counts: FxHashMap<String, u32>,
    pub flags: FxHashMap<String, bool>,
    pub stats: FxHashMap<String, f32>,
}
```

### Toast Notification UI

```rust
/// Renders achievement toast popups using the UiContext immediate-mode API.
/// Manages a display queue with timed fade-in / hold / fade-out animation.
pub struct AchievementToastRenderer {
    /// Currently displaying toasts (max 3 visible at once).
    active_toasts: Vec<ToastState>,
    /// Duration each toast is visible in seconds.
    display_duration: f32,
    /// Fade-in/out duration in seconds.
    fade_duration: f32,
}

struct ToastState {
    achievement_id: String,
    elapsed: f32,
    phase: ToastPhase,
}

#[derive(Clone, Copy, Debug)]
enum ToastPhase {
    FadeIn,
    Hold,
    FadeOut,
}

impl AchievementToastRenderer {
    pub fn new() -> Self;

    /// Queue a toast for an unlocked achievement.
    pub fn queue(&mut self, achievement_id: String);

    /// Update toast timers and transitions. Call once per frame.
    pub fn update(&mut self, dt: f32);

    /// Render active toasts. Draws icon + name + description in a small
    /// panel sliding in from the top-right corner.
    pub fn draw(
        &self,
        ui: &mut UiContext,
        tracker: &AchievementTracker,
        screen_width: f32,
    );
}
```

### Steam Integration (Optional)

```rust
/// Optional Steam achievement sync. Enabled via cargo feature `steam`.
#[cfg(feature = "steam")]
pub struct SteamAchievementSync {
    client: steamworks::Client,
}

#[cfg(feature = "steam")]
impl SteamAchievementSync {
    pub fn new(client: steamworks::Client) -> Self;

    /// Sync an unlocked achievement to Steam.
    /// Achievement IDs must match Steam's achievement API names.
    pub fn sync_unlock(&self, achievement_id: &str) -> Result<(), SteamSyncError>;

    /// Sync all unlocked achievements to Steam (call at startup).
    pub fn sync_all(&self, tracker: &AchievementTracker) -> Result<(), SteamSyncError>;

    /// Pull Steam achievement state and merge into tracker (for cloud saves).
    pub fn pull_from_steam(&self, tracker: &mut AchievementTracker) -> Result<(), SteamSyncError>;
}
```

### Errors

```rust
#[derive(Debug, Error)]
pub enum AchievementError {
    #[error("IO error loading definitions: {0}")]
    Io(#[from] std::io::Error),
    #[error("RON parse error: {0}")]
    Parse(String),
    #[error("Achievement not found: {0}")]
    NotFound(String),
    #[error("Duplicate achievement ID: {0}")]
    Duplicate(String),
}
```

## Behavior

- **Condition evaluation**: When `report_event()`, `set_flag()`, or `set_stat()`
  is called, the tracker iterates over all locked achievements whose conditions
  reference the changed value. For `AchievementCondition::All`, all sub-conditions
  must be satisfied simultaneously. For `Any`, one suffices.

- **Progress tracking**: For `EventCount` conditions, `current` is the event
  counter value and `total` is the threshold. For `StatReached`, `current` is
  the stat value cast to u32 and `total` is the threshold cast to u32. For
  `FlagSet`, progress is either 0/1 or 1/1.

- **Unlock flow**: When a condition is met:
  1. Set `progress.unlocked = true` and record `unlocked_at` timestamp.
  2. Push the achievement ID onto `pending_toasts`.
  3. If Steam sync is active, call `sync_unlock()`.

- **Toast display**: `AchievementToastRenderer` consumes the pending toast
  queue each frame and animates a slide-in panel from the top-right corner.
  Each toast shows the icon sprite, achievement name, and description for
  `display_duration` seconds. Multiple toasts stack vertically.

- **Persistence**: `save_state()` returns `AchievementSaveData` which is
  included in the game's save payload passed to `SaveManager::save()`.
  On load, `load_state()` restores all progress. Achievements unlocked
  offline are synced to Steam on the next startup.

- **RON format**:
  ```ron
  [
      AchievementDef(
          id: "first_kill",
          name: "First Blood",
          description: "Defeat your first enemy.",
          icon_sprite: "achievement_sword",
          hidden: false,
          condition: EventCount(event: "enemy_killed", threshold: 1),
          category: Some("Combat"),
          sort_order: 1,
      ),
      AchievementDef(
          id: "collect_100_coins",
          name: "Coin Collector",
          description: "Collect 100 coins.",
          icon_sprite: "achievement_coin",
          hidden: false,
          condition: EventCount(event: "coin_collected", threshold: 100),
          category: Some("Exploration"),
          sort_order: 10,
      ),
  ]
  ```

## Internal Design

- Achievement definitions are loaded into `FxHashMap<String, AchievementDef>`
  keyed by ID for O(1) lookup.
- Condition checking is optimized by maintaining a reverse index:
  `event_to_achievements: FxHashMap<String, Vec<String>>` mapping event names
  to achievement IDs that depend on them. This avoids scanning all achievements
  on every event.
- `Custom` conditions are evaluated lazily: only when explicitly triggered by
  `report_event("check_custom")` or at the end of each frame.
- Toast rendering uses `UiContext::sprite()` for the icon and
  `UiContext::pixel_text()` for name/description. The panel background uses
  `UiContext::filled_rect()` with alpha for fade animation.
- `AchievementSaveData` serializes with `serde_json` through the existing
  `SaveManager` pipeline, inheriting CRC32 integrity checks.

## Non-Goals

- **Achievement editor UI.** Achievements are authored in RON files, not via
  an in-game editor.
- **Revocable achievements.** Once unlocked, achievements cannot be re-locked
  (except via `debug_reset_all`). This matches platform conventions.
- **Leaderboards.** Score tracking and leaderboards are a separate system.
  Achievements only track unlock state and progress.
- **Achievement dependencies / chains.** Achievements are independent. If one
  achievement should require another, model it as a FlagSet condition where
  the first achievement's unlock sets the flag.
- **Platform-specific icons.** The `icon_sprite` is an engine sprite key.
  Platform storefronts (Steam, itch) have their own icon upload workflows
  outside the engine.

## Open Questions

- Should `AchievementCondition::Custom` callbacks receive a `&World` reference
  for querying ECS state, or should all relevant data be pushed through
  events/flags/stats?
- Should the toast renderer support different visual themes (position, size,
  animation style) or is top-right slide-in sufficient for all games?
- Should achievement definitions support localization keys instead of inline
  `name` and `description` strings? If so, `name` and `description` would
  be keys into the `LocaleManager`.
- Is RON the best format for achievement definitions, or should they be
  embedded in a larger game data file?
- Should `AchievementTracker` emit an event/callback on unlock so that other
  systems (e.g., particle confetti) can react?

## Referenzen

- [engine/save-load](save-load.md) -- SaveManager for achievement persistence
- [engine/ui](ui.md) -- UiContext immediate-mode API for toast rendering
- [engine/localization](localization.md) -- Potential integration for localized names
- steamworks-rs crate for Steam achievement API
- Xbox Live Achievements, PlayStation Trophies as design references
