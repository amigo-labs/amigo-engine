---
status: spec
crate: --
depends_on: ["engine/dialogue", "engine/tween"]
last_updated: 2026-03-18
---

# Visual Novel

## Purpose

Narrative-driven games centered on branching dialogue, character presentation, and player choice. The core loop is reading text, viewing character expressions, and making decisions that branch the story. Minimal or no gameplay mechanics beyond navigation through a dialogue tree.

Examples: Ace Attorney (investigation + courtroom with evidence logic), Danganronpa (class trials + free time events), Steins;Gate (pure branching narrative with phone trigger mechanic).

## Public API

### VnScene

```rust
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

#[derive(Clone, Copy, Debug)]
pub enum BgTransition {
    Cut,
    Fade { duration: f32 },
    Slide { direction: SlideDir, duration: f32 },
    Dissolve { duration: f32 },
}

#[derive(Clone, Copy, Debug)]
pub enum SlideDir {
    Left, Right, Up, Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlotPosition {
    FarLeft,
    Left,
    CenterLeft,
    Center,
    CenterRight,
    Right,
    FarRight,
}

impl VnScene {
    pub fn new() -> Self;
    /// Set background with transition. Uses TweenManager for fade/slide.
    pub fn set_background(&mut self, image: &str, transition: BgTransition, tweens: &mut TweenManager);
    /// Place or update a character in a slot.
    pub fn set_character(&mut self, slot: SlotPosition, display: CharacterDisplay, tweens: &mut TweenManager);
    /// Remove a character from a slot with optional exit animation.
    pub fn remove_character(&mut self, slot: SlotPosition, tweens: &mut TweenManager);
    /// Clear all characters (e.g. scene change).
    pub fn clear_characters(&mut self, tweens: &mut TweenManager);
    /// Advance the scene by one step (called from DialogRunner integration).
    pub fn advance(&mut self, runner: &mut DialogRunner);
}
```

### CharacterDisplay

```rust
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Emotion {
    Neutral,
    Happy,
    Sad,
    Angry,
    Surprised,
    Embarrassed,
    Thinking,
    Scared,
    Smug,
    Crying,
    /// Custom emotion identified by name (for game-specific expressions).
    Custom(u16),
}

impl CharacterDisplay {
    pub fn new(character_id: &str, body: &str, face: &str) -> Self;
    /// Change emotion — updates face_sprite to the matching variant.
    pub fn set_emotion(&mut self, emotion: Emotion);
    /// Dim/brighten the character (inactive speakers are dimmed to ~60% opacity).
    pub fn set_active(&mut self, active: bool, tweens: &mut TweenManager);
}
```

### TypewriterEffect

```rust
/// Character-by-character text reveal with configurable timing.
#[derive(Clone, Debug)]
pub struct TypewriterEffect {
    /// Full text to reveal.
    full_text: String,
    /// Number of characters currently visible.
    visible_chars: usize,
    /// Ticks between each character reveal.
    pub chars_per_tick: f32,
    /// Accumulated fractional characters.
    accumulator: f32,
    /// Pause duration (in ticks) after punctuation (period, comma, etc.).
    pub punctuation_pause: f32,
    /// Whether the full text has been revealed.
    pub complete: bool,
}

impl TypewriterEffect {
    pub fn new(text: &str, chars_per_tick: f32) -> Self;
    /// Advance the typewriter by one tick. Returns newly revealed characters.
    pub fn tick(&mut self) -> &str;
    /// Skip to end — reveal all remaining text immediately.
    pub fn skip_to_end(&mut self);
    /// The currently visible portion of text.
    pub fn visible_text(&self) -> &str;
    pub fn is_complete(&self) -> bool;
    /// Reset with new text.
    pub fn set_text(&mut self, text: &str);
}
```

### TextboxConfig

```rust
/// Configuration for the dialogue textbox appearance and behavior.
#[derive(Clone, Debug)]
pub struct TextboxConfig {
    /// Display mode.
    pub mode: TextboxMode,
    /// Name label configuration.
    pub name_label: NameLabelConfig,
    /// Whether to show a character portrait next to the textbox.
    pub show_portrait: bool,
    /// Background style for the textbox.
    pub background: TextboxBackground,
    /// Text font and size.
    pub font: String,
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Debug)]
pub struct NameLabelConfig {
    pub font: String,
    pub font_size: u16,
    pub color: Color,
    pub background: Option<TextboxBackground>,
}

#[derive(Clone, Debug)]
pub enum TextboxBackground {
    /// Solid color with optional rounded corners.
    SolidColor { color: Color, corner_radius: u32 },
    /// 9-slice sprite for styled borders.
    NineSlice { sprite: String },
    /// No background (text rendered directly over scene).
    Transparent,
}
```

### BranchingSystem

```rust
/// Tracks story flags and route state for branching narrative.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchingSystem {
    /// Named boolean flags set by choices and events.
    flags: FxHashMap<String, bool>,
    /// Named integer counters (e.g. affection points per character).
    counters: FxHashMap<String, i32>,
    /// History of choices made (for backlog and route tracking).
    choice_history: Vec<ChoiceRecord>,
    /// Current route identifier (None = common route).
    pub current_route: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChoiceRecord {
    pub node_id: String,
    pub chosen_index: usize,
    pub chosen_text: String,
}

impl BranchingSystem {
    pub fn new() -> Self;
    pub fn set_flag(&mut self, name: &str, value: bool);
    pub fn get_flag(&self, name: &str) -> bool;
    pub fn add_counter(&mut self, name: &str, amount: i32);
    pub fn get_counter(&self, name: &str) -> i32;
    pub fn record_choice(&mut self, record: ChoiceRecord);
    /// Evaluate a DialogCondition against the current flag/counter state.
    pub fn evaluate(&self, condition: &DialogCondition) -> bool;
    /// Apply a DialogEffect (set flag, modify counter, change route).
    pub fn apply_effect(&mut self, effect: &DialogEffect);
}
```

### ChoiceMenu

```rust
/// An on-screen choice menu presented to the player.
#[derive(Clone, Debug)]
pub struct ChoiceMenu {
    pub prompt: Option<String>,
    pub choices: Vec<ChoiceOption>,
    pub selected_index: usize,
}

#[derive(Clone, Debug)]
pub struct ChoiceOption {
    pub text: String,
    /// Greyed out if condition is not met (still visible, but not selectable).
    pub condition: Option<DialogCondition>,
    /// Whether this choice has been selected in a previous playthrough.
    pub previously_chosen: bool,
}

impl ChoiceMenu {
    pub fn new(choices: Vec<ChoiceOption>) -> Self;
    pub fn move_selection(&mut self, delta: i32);
    pub fn confirm(&self) -> Option<&ChoiceOption>;
    /// Returns true if the currently selected option is selectable.
    pub fn can_confirm(&self) -> bool;
}
```

### BacklogSystem

```rust
/// Scrollable history of previously displayed dialogue lines.
#[derive(Clone, Debug)]
pub struct BacklogSystem {
    entries: Vec<BacklogEntry>,
    /// Maximum number of entries to retain.
    pub max_entries: usize,
    /// Current scroll offset when viewing the backlog (0 = most recent).
    pub scroll_offset: usize,
    /// Whether the backlog overlay is currently visible.
    pub visible: bool,
}

#[derive(Clone, Debug)]
pub struct BacklogEntry {
    pub speaker: Option<String>,
    pub text: String,
    pub emotion: Option<Emotion>,
    /// Index into choice_history if this entry was a choice.
    pub choice_made: Option<usize>,
}

impl BacklogSystem {
    pub fn new(max_entries: usize) -> Self;
    pub fn push(&mut self, entry: BacklogEntry);
    pub fn scroll_up(&mut self, lines: usize);
    pub fn scroll_down(&mut self, lines: usize);
    pub fn toggle_visible(&mut self);
    pub fn entries(&self) -> &[BacklogEntry];
}
```

### AutoRead

```rust
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
    pub fn new(delay_ticks: f32, per_char_delay: f32) -> Self;
    pub fn toggle(&mut self);
    /// Call each tick after typewriter completes. Returns true when it is time to advance.
    pub fn tick(&mut self, text_length: usize) -> bool;
    /// Reset timer (called when new text appears).
    pub fn reset(&mut self);
}
```

## Behavior

- **Scene Flow**: The game loop is driven by `DialogRunner` stepping through a `DialogTree`. Each `DialogNode` maps to a VN action: show text, change background, enter/exit character, present choice, play sound. `VnScene::advance()` reads the current node and dispatches to the appropriate handler.
- **Character Presentation**: Characters are composed from layered sprites (body + face + optional outfit). When a character speaks, `CharacterDisplay::set_active(true)` tweens their opacity to 1.0 while all other characters tween to 0.6 (dimmed). Emotion changes swap the face sprite instantly. Characters enter/exit via `TweenSequence` — slide from off-screen + fade in (configurable per character).
- **Typewriter Text**: Each dialogue line starts a `TypewriterEffect`. Per tick, `chars_per_tick` characters are revealed. After punctuation marks (`.`, `!`, `?`, `,`), a `punctuation_pause` is inserted. The player can press confirm to `skip_to_end()`. Once complete, the advance indicator bounces (via Tween with `BounceOut` easing) and the system waits for input (or `AutoRead` timer).
- **Textbox Modes**: In `Adv` mode, the textbox occupies the bottom portion of the screen and clears between speakers. In `Nvl` mode, text accumulates on a fullscreen overlay until a page break node, at which point the screen clears. Mode can switch mid-scene via `DialogEffect`.
- **Branching**: `ChoiceMenu` appears when the `DialogRunner` hits a choice node. Each `ChoiceOption` may have a `DialogCondition` (evaluated via `BranchingSystem::evaluate()`). Selecting a choice records it in `BranchingSystem::choice_history`, applies any `DialogEffect`s, and resumes the `DialogRunner` on the chosen branch. Previously-chosen options can be marked (e.g. different text color) for replay value.
- **Backlog**: At any time, the player can open the `BacklogSystem` overlay to scroll through previous dialogue. Each entry records speaker, text, emotion, and any choice made. Scrolling uses `Tween` for smooth movement.
- **Auto/Skip**: `AutoRead` advances text automatically after a computed delay. A separate "skip" mode fast-forwards through already-read text (checking `previously_chosen` flags and `BacklogSystem` history). Skip pauses on unread text and choice menus.
- **Background Transitions**: `VnScene::set_background()` uses `TweenManager` to animate the transition. `Fade` tweens opacity of the old background down while the new one fades in. `Slide` tweens x/y position. `Dissolve` uses a noise-based alpha mask animated via Tween.
- **Save/Load**: `BranchingSystem` (flags, counters, choice_history, current_route) and the current `DialogRunner` node position are serialized via `SaveManager`. Loading a save restores the exact narrative position and reconstructs the `VnScene` from the current node's metadata.

## Internal Design

- `VnScene` is a render-layer struct, not an ECS entity. It owns `CharacterDisplay` instances and renders them as ordered sprite layers. Background is a fullscreen sprite behind all characters.
- `DialogRunner` from the dialogue system is the authoritative source of narrative progression. `VnScene` reads `DialogState` each frame and updates visuals accordingly. VN-specific commands (character enter/exit, background change, emotion set) are encoded as `DialogEffect` variants.
- `TypewriterEffect` operates on a `String` slice. It does not interact with the font renderer directly — it provides `visible_text()` which the UI layer renders. This keeps text logic separate from rendering.
- `BranchingSystem` wraps `DialogCondition` and `DialogEffect` from the dialogue system, adding VN-specific tracking (choice history, route divergence, counters).
- All sprite transitions (character enter/exit, background fade, dimming) are driven by `TweenManager` with `TweenHandle`s stored in `VnScene` for cancellation on interrupts (e.g. player skipping during a transition).
- `Camera::CinematicPan` is used for special scenes (panning across a CG illustration, slow zoom on a dramatic moment).

## Non-Goals

- **Lip-sync / Live2D animation.** Character sprites are static layers with emotion swaps. Animated characters (mouth flaps, breathing) are out of scope — use `AnimPlayer` directly for that.
- **Voice acting playback.** Audio system integration is referenced but voice-per-line management (timing, interruption, language switching) is a separate concern.
- **Gameplay mini-games.** Investigation sequences (Ace Attorney evidence), class trials (Danganronpa), or puzzle segments embedded in VN flow are game-specific implementations, not part of the VN gametype.
- **Script editor / visual scripting.** Dialogue trees are authored in RON or via external tools. No built-in VN script editor.
- **Gallery / CG collection system.** Unlockable image galleries are game-specific. The engine provides `SaveManager` for persistence, but gallery logic is not part of this spec.

## Open Questions

- Should `TextboxConfig` support a "name color per character" mapping for automatic color coding?
- Is a Ren'Py-compatible script import format worth supporting, or is RON-based `DialogTree` sufficient?
- Should `TypewriterEffect` support inline formatting tags (bold, italic, color, speed change) within a single text block?
- How should CG (full-screen illustration) presentation differ from background display — same system or separate?

## Referenzen

- Ren'Py: De-facto VN engine — ADV/NVL modes, character layering, backlog
- Ace Attorney: Investigation + courtroom as VN sub-modes, evidence as branching condition
- Steins;Gate: Phone trigger mechanic as implicit choice, route divergence
- [engine/dialogue](../engine/dialogue.md) → DialogTree, DialogRunner, DialogState, DialogCondition, DialogEffect
- [engine/tween](../engine/tween.md) → Sprite fade/slide transitions, textbox animations
- [engine/camera](../engine/camera.md) → CinematicPan for dramatic scenes
- [engine/animation](../engine/animation.md) → AnimPlayer for animated character sprites
- [engine/save-load](../engine/save-load.md) → SaveManager for narrative state persistence
- [engine/audio](../engine/audio.md) → Scene music and ambient audio
