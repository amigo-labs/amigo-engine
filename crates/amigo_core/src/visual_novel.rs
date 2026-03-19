//! Visual novel gametype: branching narrative with character presentation and player choice.
//!
//! Provides the core systems for VN-style games: typewriter text reveal, character slot display,
//! background transitions, choice menus, backlog, auto-read, and branching state tracking.

use crate::color::Color;
use crate::dialog::{DialogCondition, DialogEffect, DialogState};
use crate::math::RenderVec2;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Slot positions
// ---------------------------------------------------------------------------

/// Horizontal slot where a character can be placed on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotPosition {
    /// Far left of the screen.
    FarLeft,
    /// Left third of the screen.
    Left,
    /// Between left and center.
    CenterLeft,
    /// Center of the screen.
    Center,
    /// Between center and right.
    CenterRight,
    /// Right third of the screen.
    Right,
    /// Far right of the screen.
    FarRight,
}

// ---------------------------------------------------------------------------
// Background transitions
// ---------------------------------------------------------------------------

/// Direction for slide transitions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SlideDir {
    /// Slide to the left.
    Left,
    /// Slide to the right.
    Right,
    /// Slide upward.
    Up,
    /// Slide downward.
    Down,
}

/// Transition effect used when switching backgrounds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BgTransition {
    /// Instant cut — no animation.
    Cut,
    /// Crossfade between old and new background.
    Fade {
        /// Duration in seconds.
        duration: f32,
    },
    /// Slide the old background out in a direction.
    Slide {
        /// Direction of the slide.
        direction: SlideDir,
        /// Duration in seconds.
        duration: f32,
    },
    /// Dissolve using a noise-based alpha mask.
    Dissolve {
        /// Duration in seconds.
        duration: f32,
    },
}

impl Default for BgTransition {
    fn default() -> Self {
        Self::Cut
    }
}

// ---------------------------------------------------------------------------
// Emotion
// ---------------------------------------------------------------------------

/// Character emotion — determines which face sprite variant to display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Emotion {
    /// Default expression.
    Neutral,
    /// Smiling, cheerful.
    Happy,
    /// Downcast, melancholy.
    Sad,
    /// Furious, irritated.
    Angry,
    /// Shocked, wide-eyed.
    Surprised,
    /// Blushing, flustered.
    Embarrassed,
    /// Pondering, hand on chin.
    Thinking,
    /// Frightened, trembling.
    Scared,
    /// Self-satisfied, sly grin.
    Smug,
    /// Tears streaming.
    Crying,
    /// Custom emotion identified by a numeric index (for game-specific expressions).
    Custom(u16),
}

impl Default for Emotion {
    fn default() -> Self {
        Self::Neutral
    }
}

// ---------------------------------------------------------------------------
// Character display
// ---------------------------------------------------------------------------

/// A character's visual representation on screen, composed of sprite layers.
#[derive(Clone, Debug)]
pub struct CharacterDisplay {
    /// Character identifier (for looking up sprite assets).
    pub character_id: String,
    /// Base body sprite (full body or bust).
    pub body_sprite: String,
    /// Face/expression overlay (swapped for emotions).
    pub face_sprite: String,
    /// Optional outfit overlay (school uniform, casual, etc.).
    pub outfit_sprite: Option<String>,
    /// Current emotion (determines face_sprite variant).
    pub emotion: Emotion,
    /// Render offset from slot anchor point (for fine-tuning).
    pub offset: RenderVec2,
    /// Scale factor (1.0 = normal, useful for depth/distance effect).
    pub scale: f32,
    /// Current opacity (0.0 = invisible, 1.0 = fully visible).
    pub opacity: f32,
    /// Whether this character is "active" (speaking) — inactive characters may be dimmed.
    pub active: bool,
}

impl CharacterDisplay {
    /// Create a new character display with default settings.
    pub fn new(character_id: &str, body: &str, face: &str) -> Self {
        Self {
            character_id: character_id.to_string(),
            body_sprite: body.to_string(),
            face_sprite: face.to_string(),
            outfit_sprite: None,
            emotion: Emotion::Neutral,
            offset: RenderVec2::ZERO,
            scale: 1.0,
            opacity: 1.0,
            active: false,
        }
    }

    /// Change emotion — updates face_sprite to the matching variant.
    ///
    /// The face sprite is resolved using the pattern `"{character_id}_{emotion_name}"`.
    /// For [`Emotion::Custom`] variants, the face sprite is set to `"{character_id}_custom_{n}"`.
    pub fn set_emotion(&mut self, emotion: Emotion) {
        self.emotion = emotion;
        let suffix = match emotion {
            Emotion::Neutral => "neutral",
            Emotion::Happy => "happy",
            Emotion::Sad => "sad",
            Emotion::Angry => "angry",
            Emotion::Surprised => "surprised",
            Emotion::Embarrassed => "embarrassed",
            Emotion::Thinking => "thinking",
            Emotion::Scared => "scared",
            Emotion::Smug => "smug",
            Emotion::Crying => "crying",
            Emotion::Custom(n) => {
                self.face_sprite = format!("{}_custom_{}", self.character_id, n);
                return;
            }
        };
        self.face_sprite = format!("{}_{}", self.character_id, suffix);
    }

    /// Set the active (speaking) state. Active characters are fully opaque;
    /// inactive characters are dimmed to 60% opacity.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        self.opacity = if active { 1.0 } else { 0.6 };
    }
}

// ---------------------------------------------------------------------------
// Typewriter effect
// ---------------------------------------------------------------------------

/// Character-by-character text reveal with configurable timing.
#[derive(Clone, Debug)]
pub struct TypewriterEffect {
    /// Full text to reveal.
    full_text: String,
    /// Number of characters currently visible.
    visible_chars: usize,
    /// Characters to reveal per tick (can be fractional for slow speeds).
    pub chars_per_tick: f32,
    /// Accumulated fractional characters.
    accumulator: f32,
    /// Pause duration (in ticks) after punctuation (period, comma, etc.).
    pub punctuation_pause: f32,
    /// Remaining pause ticks when a punctuation pause is active.
    pause_remaining: f32,
    /// Whether the full text has been revealed.
    pub complete: bool,
}

impl TypewriterEffect {
    /// Create a new typewriter effect for the given text.
    pub fn new(text: &str, chars_per_tick: f32) -> Self {
        let complete = text.is_empty();
        Self {
            full_text: text.to_string(),
            visible_chars: 0,
            chars_per_tick,
            accumulator: 0.0,
            punctuation_pause: 3.0,
            pause_remaining: 0.0,
            complete,
        }
    }

    /// Advance the typewriter by one tick. Returns the newly revealed slice.
    pub fn tick(&mut self) -> &str {
        if self.complete {
            return "";
        }

        // Handle punctuation pause.
        if self.pause_remaining > 0.0 {
            self.pause_remaining -= 1.0;
            return "";
        }

        let prev = self.visible_chars;
        self.accumulator += self.chars_per_tick;
        let chars_to_add = self.accumulator as usize;
        self.accumulator -= chars_to_add as f32;

        let total_chars = self.full_text.chars().count();
        self.visible_chars = (self.visible_chars + chars_to_add).min(total_chars);

        if self.visible_chars >= total_chars {
            self.complete = true;
        }

        // Check if the last revealed character is punctuation.
        if self.visible_chars > prev && self.visible_chars < total_chars {
            if let Some(ch) = self.full_text.chars().nth(self.visible_chars - 1) {
                if matches!(ch, '.' | '!' | '?' | ',') {
                    self.pause_remaining = self.punctuation_pause;
                }
            }
        }

        // Return the newly revealed slice.
        let start = self.char_byte_offset(prev);
        let end = self.char_byte_offset(self.visible_chars);
        &self.full_text[start..end]
    }

    /// Skip to end — reveal all remaining text immediately.
    pub fn skip_to_end(&mut self) {
        self.visible_chars = self.full_text.chars().count();
        self.complete = true;
        self.pause_remaining = 0.0;
    }

    /// The currently visible portion of text.
    pub fn visible_text(&self) -> &str {
        let end = self.char_byte_offset(self.visible_chars);
        &self.full_text[..end]
    }

    /// Whether the full text has been revealed.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Reset with new text.
    pub fn set_text(&mut self, text: &str) {
        self.full_text = text.to_string();
        self.visible_chars = 0;
        self.accumulator = 0.0;
        self.pause_remaining = 0.0;
        self.complete = text.is_empty();
    }

    /// Helper: byte offset of the n-th character.
    fn char_byte_offset(&self, n: usize) -> usize {
        self.full_text
            .char_indices()
            .nth(n)
            .map(|(i, _)| i)
            .unwrap_or(self.full_text.len())
    }
}

// ---------------------------------------------------------------------------
// Textbox configuration
// ---------------------------------------------------------------------------

/// Display mode for the textbox.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TextboxMode {
    /// ADV mode: textbox fixed at bottom of screen, shows one speech block at a time.
    Adv {
        /// Height of the textbox region in pixels.
        height: u32,
    },
    /// NVL mode: fullscreen text, accumulates paragraphs until a page break.
    Nvl {
        /// Margin from screen edges in pixels.
        margin: u32,
    },
}

impl Default for TextboxMode {
    fn default() -> Self {
        Self::Adv { height: 200 }
    }
}

/// Background style for the textbox.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TextboxBackground {
    /// Solid color with optional rounded corners.
    SolidColor {
        /// Fill color.
        color: Color,
        /// Corner radius in pixels.
        corner_radius: u32,
    },
    /// 9-slice sprite for styled borders.
    NineSlice {
        /// Sprite asset name.
        sprite: String,
    },
    /// No background (text rendered directly over scene).
    Transparent,
}

impl Default for TextboxBackground {
    fn default() -> Self {
        Self::SolidColor {
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.8,
            },
            corner_radius: 8,
        }
    }
}

/// Configuration for the speaker name label.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NameLabelConfig {
    /// Font asset name.
    pub font: String,
    /// Font size in pixels.
    pub font_size: u16,
    /// Text color.
    pub color: Color,
    /// Optional background for the name label.
    pub background: Option<TextboxBackground>,
}

impl Default for NameLabelConfig {
    fn default() -> Self {
        Self {
            font: String::new(),
            font_size: 20,
            color: Color::WHITE,
            background: None,
        }
    }
}

/// Configuration for the dialogue textbox appearance and behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextboxConfig {
    /// Display mode.
    pub mode: TextboxMode,
    /// Name label configuration.
    pub name_label: NameLabelConfig,
    /// Whether to show a character portrait next to the textbox.
    pub show_portrait: bool,
    /// Background style for the textbox.
    pub background: TextboxBackground,
    /// Text font asset name.
    pub font: String,
    /// Font size in pixels.
    pub font_size: u16,
    /// Text color.
    pub text_color: Color,
    /// Maximum characters per line before wrapping.
    pub line_width: u32,
    /// Maximum visible lines.
    pub max_lines: u32,
    /// Advance indicator sprite (the little bouncing arrow).
    pub advance_indicator: Option<String>,
}

impl Default for TextboxConfig {
    fn default() -> Self {
        Self {
            mode: TextboxMode::default(),
            name_label: NameLabelConfig::default(),
            show_portrait: false,
            background: TextboxBackground::default(),
            font: String::new(),
            font_size: 24,
            text_color: Color::WHITE,
            line_width: 60,
            max_lines: 3,
            advance_indicator: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Textbox state (runtime)
// ---------------------------------------------------------------------------

/// Runtime state of the dialogue textbox.
#[derive(Clone, Debug)]
pub struct TextboxState {
    /// Textbox configuration.
    pub config: TextboxConfig,
    /// Current speaker name (None for narration).
    pub speaker: Option<String>,
    /// Typewriter effect for the current line.
    pub typewriter: TypewriterEffect,
    /// Whether the textbox is currently visible.
    pub visible: bool,
}

impl TextboxState {
    /// Create a new textbox state with default configuration.
    pub fn new() -> Self {
        Self {
            config: TextboxConfig::default(),
            speaker: None,
            typewriter: TypewriterEffect::new("", 1.0),
            visible: false,
        }
    }

    /// Set new dialogue text, resetting the typewriter.
    pub fn set_line(&mut self, speaker: Option<&str>, text: &str) {
        self.speaker = speaker.map(|s| s.to_string());
        self.typewriter.set_text(text);
        self.visible = true;
    }
}

impl Default for TextboxState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Branching system
// ---------------------------------------------------------------------------

/// Record of a choice the player made.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChoiceRecord {
    /// The dialog node where the choice was made.
    pub node_id: String,
    /// Index of the chosen option.
    pub chosen_index: usize,
    /// Display text of the chosen option.
    pub chosen_text: String,
}

/// Tracks story flags and route state for branching narrative.
///
/// Wraps [`DialogState`] from the dialogue system and adds VN-specific state
/// (choice history, route tracking, named counters).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchingSystem {
    /// Delegates to DialogState for flag/condition evaluation.
    pub dialog_state: DialogState,
    /// Named integer counters beyond DialogState flags (e.g. affection points).
    counters: FxHashMap<String, i32>,
    /// History of choices made (for backlog and route tracking).
    choice_history: Vec<ChoiceRecord>,
    /// Current route identifier (None = common route).
    pub current_route: Option<String>,
}

impl BranchingSystem {
    /// Create a new branching system with empty state.
    pub fn new() -> Self {
        Self {
            dialog_state: DialogState::new(),
            counters: FxHashMap::default(),
            choice_history: Vec::new(),
            current_route: None,
        }
    }

    /// Set a boolean flag (stored as 1 for true, cleared for false).
    pub fn set_flag(&mut self, name: &str, value: bool) {
        if value {
            self.dialog_state.set_flag(name, 1);
        } else {
            self.dialog_state.clear_flag(name);
        }
    }

    /// Get a boolean flag (true if the flag exists and is non-zero).
    pub fn get_flag(&self, name: &str) -> bool {
        self.dialog_state.get_flag(name) != 0
    }

    /// Add to a named counter (creates it at 0 if it doesn't exist).
    pub fn add_counter(&mut self, name: &str, amount: i32) {
        let entry = self.counters.entry(name.to_string()).or_insert(0);
        *entry += amount;
    }

    /// Get the current value of a named counter.
    pub fn get_counter(&self, name: &str) -> i32 {
        self.counters.get(name).copied().unwrap_or(0)
    }

    /// Record a player choice in the history.
    pub fn record_choice(&mut self, record: ChoiceRecord) {
        self.choice_history.push(record);
    }

    /// Get the full choice history.
    pub fn choice_history(&self) -> &[ChoiceRecord] {
        &self.choice_history
    }

    /// Evaluate a [`DialogCondition`] against the current flag/counter state.
    pub fn evaluate(&self, condition: &DialogCondition) -> bool {
        self.dialog_state.check_condition(condition)
    }

    /// Apply a [`DialogEffect`] (set flag, modify counter, change route).
    pub fn apply_effect(&mut self, effect: &DialogEffect) {
        self.dialog_state.apply_effect(effect);
    }
}

impl Default for BranchingSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Choice menu
// ---------------------------------------------------------------------------

/// A single selectable option in a choice menu.
#[derive(Clone, Debug)]
pub struct ChoiceOption {
    /// Display text for this choice.
    pub text: String,
    /// Optional condition — greyed out if not met (still visible, but not selectable).
    pub condition: Option<DialogCondition>,
    /// Whether this choice has been selected in a previous playthrough.
    pub previously_chosen: bool,
}

/// An on-screen choice menu presented to the player.
#[derive(Clone, Debug)]
pub struct ChoiceMenu {
    /// Optional prompt displayed above the choices.
    pub prompt: Option<String>,
    /// Available choices.
    pub choices: Vec<ChoiceOption>,
    /// Index of the currently highlighted choice.
    pub selected_index: usize,
}

impl ChoiceMenu {
    /// Create a new choice menu. Panics if `choices` is empty.
    pub fn new(choices: Vec<ChoiceOption>) -> Self {
        assert!(
            !choices.is_empty(),
            "ChoiceMenu requires at least one choice"
        );
        Self {
            prompt: None,
            choices,
            selected_index: 0,
        }
    }

    /// Move the selection by `delta` (positive = down, negative = up), wrapping around.
    pub fn move_selection(&mut self, delta: i32) {
        let len = self.choices.len() as i32;
        let new_idx = ((self.selected_index as i32 + delta) % len + len) % len;
        self.selected_index = new_idx as usize;
    }

    /// Confirm the current selection. Returns the selected option if it is selectable.
    pub fn confirm(&self) -> Option<&ChoiceOption> {
        if self.can_confirm() {
            Some(&self.choices[self.selected_index])
        } else {
            None
        }
    }

    /// Returns true if the currently selected option is selectable (has no unmet condition).
    pub fn can_confirm(&self) -> bool {
        self.choices
            .get(self.selected_index)
            .map(|opt| opt.condition.is_none())
            .unwrap_or(false)
    }

    /// Check if the currently selected option is selectable against a branching system.
    pub fn can_confirm_with(&self, branching: &BranchingSystem) -> bool {
        self.choices
            .get(self.selected_index)
            .map(|opt| match &opt.condition {
                None => true,
                Some(cond) => branching.evaluate(cond),
            })
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Backlog system
// ---------------------------------------------------------------------------

/// A single entry in the dialogue backlog.
#[derive(Clone, Debug)]
pub struct BacklogEntry {
    /// Speaker name (None for narration).
    pub speaker: Option<String>,
    /// Full dialogue text.
    pub text: String,
    /// Emotion displayed when this line was spoken.
    pub emotion: Option<Emotion>,
    /// Index into choice_history if this entry was a choice.
    pub choice_made: Option<usize>,
}

/// Scrollable history of previously displayed dialogue lines.
#[derive(Clone, Debug)]
pub struct BacklogSystem {
    /// Stored entries (oldest first).
    entries: Vec<BacklogEntry>,
    /// Maximum number of entries to retain.
    pub max_entries: usize,
    /// Current scroll offset when viewing the backlog (0 = most recent).
    pub scroll_offset: usize,
    /// Whether the backlog overlay is currently visible.
    pub visible: bool,
}

impl BacklogSystem {
    /// Create a new backlog system with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            scroll_offset: 0,
            visible: false,
        }
    }

    /// Add an entry to the backlog, evicting the oldest if at capacity.
    pub fn push(&mut self, entry: BacklogEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Scroll up (towards older entries).
    pub fn scroll_up(&mut self, lines: usize) {
        let max_offset = self.entries.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_offset);
    }

    /// Scroll down (towards newer entries).
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Toggle the backlog overlay visibility. Resets scroll on open.
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.scroll_offset = 0;
        }
    }

    /// Get all backlog entries.
    pub fn entries(&self) -> &[BacklogEntry] {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// Auto-read
// ---------------------------------------------------------------------------

/// Automatic text advance without player input.
#[derive(Clone, Debug)]
pub struct AutoRead {
    /// Whether auto-read is currently enabled.
    pub enabled: bool,
    /// Ticks to wait after typewriter completes before advancing.
    pub delay_ticks: f32,
    /// Additional delay per character in the text (longer lines wait longer).
    pub per_char_delay: f32,
    /// Accumulated wait time.
    elapsed: f32,
    /// Total wait time for the current line.
    target: f32,
}

impl AutoRead {
    /// Create a new auto-read system.
    pub fn new(delay_ticks: f32, per_char_delay: f32) -> Self {
        Self {
            enabled: false,
            delay_ticks,
            per_char_delay,
            elapsed: 0.0,
            target: 0.0,
        }
    }

    /// Toggle auto-read on/off.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.elapsed = 0.0;
    }

    /// Call each tick after the typewriter completes. Returns true when it is time to advance.
    pub fn tick(&mut self, text_length: usize) -> bool {
        if !self.enabled {
            return false;
        }
        if self.target == 0.0 {
            self.target = self.delay_ticks + (text_length as f32 * self.per_char_delay);
        }
        self.elapsed += 1.0;
        self.elapsed >= self.target
    }

    /// Reset timer (called when new text appears).
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
        self.target = 0.0;
    }
}

// ---------------------------------------------------------------------------
// VnScene
// ---------------------------------------------------------------------------

/// Represents the current visual state of a VN scene.
#[derive(Clone, Debug)]
pub struct VnScene {
    /// Currently displayed background image.
    pub background: Option<String>,
    /// Transition used when switching backgrounds.
    pub bg_transition: BgTransition,
    /// Characters currently on screen, indexed by slot.
    pub characters: FxHashMap<SlotPosition, CharacterDisplay>,
    /// Current textbox state.
    pub textbox: TextboxState,
    /// Whether the scene is waiting for player input.
    pub waiting_for_input: bool,
    /// Active choice menu (None if no choice is being presented).
    pub active_choice: Option<ChoiceMenu>,
}

impl VnScene {
    /// Create a new, empty VN scene.
    pub fn new() -> Self {
        Self {
            background: None,
            bg_transition: BgTransition::default(),
            characters: FxHashMap::default(),
            textbox: TextboxState::new(),
            waiting_for_input: false,
            active_choice: None,
        }
    }

    /// Set background with transition.
    pub fn set_background(&mut self, image: &str, transition: BgTransition) {
        self.background = Some(image.to_string());
        self.bg_transition = transition;
    }

    /// Place or update a character in a slot.
    pub fn set_character(&mut self, slot: SlotPosition, display: CharacterDisplay) {
        self.characters.insert(slot, display);
    }

    /// Remove a character from a slot.
    pub fn remove_character(&mut self, slot: SlotPosition) {
        self.characters.remove(&slot);
    }

    /// Clear all characters (e.g. scene change).
    pub fn clear_characters(&mut self) {
        self.characters.clear();
    }

    /// Dim all characters except the one at the given slot (the active speaker).
    pub fn focus_character(&mut self, active_slot: SlotPosition) {
        for (slot, display) in &mut self.characters {
            display.set_active(*slot == active_slot);
        }
    }
}

impl Default for VnScene {
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

    #[test]
    fn typewriter_reveals_text_progressively() {
        let mut tw = TypewriterEffect::new("Hello", 1.0);
        assert!(!tw.is_complete());
        assert_eq!(tw.visible_text(), "");

        tw.tick();
        assert_eq!(tw.visible_text(), "H");

        tw.tick();
        assert_eq!(tw.visible_text(), "He");
    }

    #[test]
    fn typewriter_skip_to_end() {
        let mut tw = TypewriterEffect::new("Hello world", 1.0);
        tw.tick();
        tw.skip_to_end();
        assert!(tw.is_complete());
        assert_eq!(tw.visible_text(), "Hello world");
    }

    #[test]
    fn typewriter_set_text_resets() {
        let mut tw = TypewriterEffect::new("First", 1.0);
        tw.skip_to_end();
        assert!(tw.is_complete());

        tw.set_text("Second");
        assert!(!tw.is_complete());
        assert_eq!(tw.visible_text(), "");
    }

    #[test]
    fn typewriter_empty_text_is_complete() {
        let tw = TypewriterEffect::new("", 1.0);
        assert!(tw.is_complete());
    }

    #[test]
    fn choice_menu_wraps_selection() {
        let choices = vec![
            ChoiceOption {
                text: "A".into(),
                condition: None,
                previously_chosen: false,
            },
            ChoiceOption {
                text: "B".into(),
                condition: None,
                previously_chosen: false,
            },
            ChoiceOption {
                text: "C".into(),
                condition: None,
                previously_chosen: false,
            },
        ];
        let mut menu = ChoiceMenu::new(choices);
        assert_eq!(menu.selected_index, 0);

        menu.move_selection(-1);
        assert_eq!(menu.selected_index, 2);

        menu.move_selection(1);
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn choice_menu_confirm_with_no_condition() {
        let choices = vec![ChoiceOption {
            text: "Go left".into(),
            condition: None,
            previously_chosen: false,
        }];
        let menu = ChoiceMenu::new(choices);
        assert!(menu.can_confirm());
        assert!(menu.confirm().is_some());
    }

    #[test]
    fn choice_menu_blocks_confirm_with_condition() {
        let choices = vec![ChoiceOption {
            text: "Locked".into(),
            condition: Some(DialogCondition::FlagSet("key".into())),
            previously_chosen: false,
        }];
        let menu = ChoiceMenu::new(choices);
        // can_confirm returns false when there is a condition (condition-based check needs branching system)
        assert!(!menu.can_confirm());
    }

    #[test]
    fn backlog_push_and_eviction() {
        let mut log = BacklogSystem::new(3);
        log.push(BacklogEntry {
            speaker: None,
            text: "Line 1".into(),
            emotion: None,
            choice_made: None,
        });
        log.push(BacklogEntry {
            speaker: None,
            text: "Line 2".into(),
            emotion: None,
            choice_made: None,
        });
        log.push(BacklogEntry {
            speaker: None,
            text: "Line 3".into(),
            emotion: None,
            choice_made: None,
        });
        assert_eq!(log.entries().len(), 3);

        log.push(BacklogEntry {
            speaker: None,
            text: "Line 4".into(),
            emotion: None,
            choice_made: None,
        });
        assert_eq!(log.entries().len(), 3);
        assert_eq!(log.entries()[0].text, "Line 2");
    }

    #[test]
    fn backlog_scroll() {
        let mut log = BacklogSystem::new(100);
        for i in 0..10 {
            log.push(BacklogEntry {
                speaker: None,
                text: format!("Line {}", i),
                emotion: None,
                choice_made: None,
            });
        }
        assert_eq!(log.scroll_offset, 0);

        log.scroll_up(3);
        assert_eq!(log.scroll_offset, 3);

        log.scroll_down(1);
        assert_eq!(log.scroll_offset, 2);

        // Scroll down past zero should clamp to 0.
        log.scroll_down(100);
        assert_eq!(log.scroll_offset, 0);
    }

    #[test]
    fn auto_read_timing() {
        let mut auto = AutoRead::new(10.0, 0.5);
        auto.enabled = true;

        // Text length 10 => target = 10.0 + 10 * 0.5 = 15.0
        for _ in 0..14 {
            assert!(!auto.tick(10));
        }
        assert!(auto.tick(10));
    }

    #[test]
    fn auto_read_toggle() {
        let mut auto = AutoRead::new(5.0, 0.0);
        assert!(!auto.enabled);
        auto.toggle();
        assert!(auto.enabled);
        auto.toggle();
        assert!(!auto.enabled);
    }

    #[test]
    fn branching_system_flags_and_counters() {
        let mut bs = BranchingSystem::new();
        assert!(!bs.get_flag("met_sakura"));

        bs.set_flag("met_sakura", true);
        assert!(bs.get_flag("met_sakura"));

        bs.set_flag("met_sakura", false);
        assert!(!bs.get_flag("met_sakura"));

        bs.add_counter("affection", 5);
        bs.add_counter("affection", 3);
        assert_eq!(bs.get_counter("affection"), 8);
    }

    #[test]
    fn character_display_emotion_changes_face_sprite() {
        let mut ch = CharacterDisplay::new("sakura", "sakura_body", "sakura_neutral");
        assert_eq!(ch.face_sprite, "sakura_neutral");

        ch.set_emotion(Emotion::Happy);
        assert_eq!(ch.face_sprite, "sakura_happy");
        assert_eq!(ch.emotion, Emotion::Happy);

        ch.set_emotion(Emotion::Custom(42));
        assert_eq!(ch.face_sprite, "sakura_custom_42");
    }

    #[test]
    fn vn_scene_character_management() {
        let mut scene = VnScene::new();
        let ch = CharacterDisplay::new("sakura", "sakura_body", "sakura_neutral");
        scene.set_character(SlotPosition::Center, ch);
        assert!(scene.characters.contains_key(&SlotPosition::Center));

        scene.remove_character(SlotPosition::Center);
        assert!(!scene.characters.contains_key(&SlotPosition::Center));
    }

    #[test]
    fn vn_scene_focus_character_dims_others() {
        let mut scene = VnScene::new();
        scene.set_character(
            SlotPosition::Left,
            CharacterDisplay::new("a", "a_body", "a_neutral"),
        );
        scene.set_character(
            SlotPosition::Center,
            CharacterDisplay::new("b", "b_body", "b_neutral"),
        );
        scene.set_character(
            SlotPosition::Right,
            CharacterDisplay::new("c", "c_body", "c_neutral"),
        );

        scene.focus_character(SlotPosition::Center);

        assert!(!scene.characters[&SlotPosition::Left].active);
        assert!(scene.characters[&SlotPosition::Center].active);
        assert!(!scene.characters[&SlotPosition::Right].active);
        assert!((scene.characters[&SlotPosition::Left].opacity - 0.6).abs() < f32::EPSILON);
        assert!((scene.characters[&SlotPosition::Center].opacity - 1.0).abs() < f32::EPSILON);
    }
}
