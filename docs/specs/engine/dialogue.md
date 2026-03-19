---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/ui"]
last_updated: 2026-03-18
---

# Dialogue System

## Purpose

Provides a branching dialogue tree system with conditional choices, side effects, persistent flag-based state, and a runtime state machine for advancing conversations. Used for NPC interactions, quest conversations, and story sequences.

Existing implementation in `crates/amigo_core/src/dialog.rs` (657 lines).

## Public API

### DialogId

```rust
/// Unique dialog node identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DialogId(pub u32);
```

### DialogCondition

```rust
/// Condition that must be met for a dialog node or choice to appear.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DialogCondition {
    FlagSet(String),
    FlagNotSet(String),
    FlagEquals(String, i32),
    FlagGreaterThan(String, i32),
    FlagLessThan(String, i32),
    HasItem(u32, u32),          // item_id, quantity
    And(Vec<DialogCondition>),
    Or(Vec<DialogCondition>),
    Not(Box<DialogCondition>),
}
```

### DialogEffect

```rust
/// Side effect triggered when a dialog node is entered or a choice is made.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DialogEffect {
    SetFlag(String, i32),
    ClearFlag(String),
    IncrementFlag(String, i32),
    GiveItem(u32, u32),         // item_id, count
    TakeItem(u32, u32),
    GiveExp(u32),
    Heal,
    StartBattle(u32),           // battle_id
    PlaySound(String),
    /// Game-specific effect not covered by built-in variants.
    /// Convention: use a namespaced prefix to avoid collisions between game types.
    /// Format: "namespace:command:arg1:arg2:..."
    /// Examples:
    ///   "vn:bg:forest_night:fade:0.5"     — Visual Novel: set background with fade
    ///   "vn:char:enter:left:sakura:neutral" — Visual Novel: enter character
    ///   "quest:advance:main_quest:step_3"   — Quest system: advance quest state
    /// The dialogue system passes Custom effects unchanged to the game layer.
    /// Parsing and execution is the game code's responsibility.
    Custom(String),
}
```

### DialogChoice

```rust
/// A choice the player can make during dialog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogChoice {
    pub text: String,
    pub next: DialogId,
    pub condition: Option<DialogCondition>,
    pub effects: Vec<DialogEffect>,
}
```

### DialogNode

```rust
/// A single dialog node (one "screen" of dialog).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogNode {
    pub id: DialogId,
    pub speaker: String,
    pub portrait: String,
    pub text: String,
    /// If no choices, this is the next node (None = dialog ends).
    pub next: Option<DialogId>,
    /// Player choices (if non-empty, `next` is ignored).
    pub choices: Vec<DialogChoice>,
    /// Conditions for this node to be shown (all must pass).
    pub conditions: Vec<DialogCondition>,
    /// Effects triggered when this node is entered.
    pub effects: Vec<DialogEffect>,
}

impl DialogNode {
    pub fn new(id: u32, speaker: impl Into<String>, text: impl Into<String>) -> Self;
    pub fn with_next(self, next: u32) -> Self;
    pub fn with_portrait(self, portrait: impl Into<String>) -> Self;
    pub fn with_choice(self, text: impl Into<String>, next: u32) -> Self;
    pub fn with_conditional_choice(
        self, text: impl Into<String>, next: u32, condition: DialogCondition,
    ) -> Self;
    pub fn with_effect(self, effect: DialogEffect) -> Self;
    pub fn with_condition(self, condition: DialogCondition) -> Self;
}
```

### DialogTree

```rust
/// A complete dialog tree containing connected nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogTree {
    pub id: u32,
    pub name: String,
    pub entry_point: DialogId,
    pub nodes: FxHashMap<DialogId, DialogNode>,
}

impl DialogTree {
    pub fn new(id: u32, name: impl Into<String>, entry_point: u32) -> Self;
    pub fn add_node(&mut self, node: DialogNode);
    pub fn with_node(self, node: DialogNode) -> Self;
    pub fn get_node(&self, id: DialogId) -> Option<&DialogNode>;
}
```

### DialogState

```rust
/// Persistent dialog state (flags, variables). Saved with the game.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DialogState {
    flags: FxHashMap<String, i32>,
}

impl DialogState {
    pub fn new() -> Self;
    pub fn set_flag(&mut self, key: impl Into<String>, value: i32);
    pub fn get_flag(&self, key: &str) -> i32;    // returns 0 if absent
    pub fn has_flag(&self, key: &str) -> bool;
    pub fn clear_flag(&mut self, key: &str);
    pub fn increment_flag(&mut self, key: &str, amount: i32);
    pub fn check_condition(&self, condition: &DialogCondition) -> bool;
    pub fn apply_effect(&mut self, effect: &DialogEffect);
}
```

### DialogGameEffect

```rust
/// An effect that the game needs to handle (not flag-related).
#[derive(Clone, Debug)]
pub enum DialogGameEffect {
    GiveItem(u32, u32),
    TakeItem(u32, u32),
    GiveExp(u32),
    Heal,
    StartBattle(u32),
    PlaySound(String),
    Custom(String),
}
```

### DialogRunner

```rust
/// Runs a dialog, tracking current position and producing events.
pub struct DialogRunner {
    // private fields: active, current_node, current_tree,
    //                 available_choices, pending_effects
}

impl DialogRunner {
    pub fn new() -> Self;
    pub fn start(&mut self, tree: &DialogTree, state: &mut DialogState);
    pub fn advance(&mut self, tree: &DialogTree, state: &mut DialogState);
    pub fn choose(&mut self, choice_index: usize, tree: &DialogTree, state: &mut DialogState);
    pub fn is_active(&self) -> bool;
    pub fn current_node_id(&self) -> Option<DialogId>;
    pub fn current_tree_id(&self) -> Option<u32>;
    pub fn current_node<'a>(&self, tree: &'a DialogTree) -> Option<&'a DialogNode>;
    pub fn available_choices(&self) -> &[usize];
    pub fn has_choices(&self) -> bool;
    pub fn take_effects(&mut self) -> Vec<DialogGameEffect>;
    pub fn stop(&mut self);
}
```

### DialogRegistry

```rust
/// Central registry for all dialog trees.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DialogRegistry {
    trees: FxHashMap<u32, DialogTree>,
}

impl DialogRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, tree: DialogTree);
    pub fn get(&self, id: u32) -> Option<&DialogTree>;
}
```

## Behavior

### Dialog Flow

A dialog session follows this lifecycle:

1. **Start**: Call `runner.start(tree, state)`. The runner enters the tree's `entry_point` node, checks its conditions, applies its effects, and computes the list of available choices.
2. **Display**: The game reads `runner.current_node(tree)` to display speaker, portrait, and text. If `has_choices()` is true, display the choice buttons for `available_choices()`.
3. **Advance**: If no choices, call `runner.advance()` to move to the `next` node. If choices exist, `advance()` is a no-op -- use `choose(index)` instead.
4. **Choose**: `runner.choose(choice_index, tree, state)` applies the choice's effects and enters its target node.
5. **End**: When a node has no `next` and no choices, advancing deactivates the runner (`is_active()` returns false).

### Condition Evaluation

`DialogState::check_condition()` evaluates conditions recursively:

- `FlagSet` / `FlagNotSet`: Checks whether a key exists in the flags map.
- `FlagEquals`, `FlagGreaterThan`, `FlagLessThan`: Compares the flag value (defaulting to 0 if absent).
- `HasItem`: Always returns `true` at the engine level. Games override this by checking their own inventory system.
- `And`: All sub-conditions must pass.
- `Or`: At least one sub-condition must pass.
- `Not`: Inverts a single sub-condition.

Conditions are checked at two points:

- **Node conditions**: When entering a node, all `conditions` must pass. If they fail, the node is skipped and the runner follows `next` (or ends the dialog if `next` is `None`).
- **Choice conditions**: Each choice's `condition` is evaluated to build the `available_choices` list. Choices whose conditions fail are hidden from the player.

### Effect Processing

Effects split into two categories:

1. **Flag effects** (`SetFlag`, `ClearFlag`, `IncrementFlag`): Applied directly by `DialogState::apply_effect()`.
2. **Game effects** (`GiveItem`, `TakeItem`, `GiveExp`, `Heal`, `StartBattle`, `PlaySound`, `Custom`): Stored in `pending_effects` as `DialogGameEffect` values. The game retrieves them via `runner.take_effects()`, which drains the buffer.

Effects trigger both when entering a node (from `node.effects`) and when making a choice (from `choice.effects`).

### Builder Pattern Usage

`DialogNode` and `DialogTree` use the builder pattern for fluent construction:

```rust
let tree = DialogTree::new(1, "Test Dialog", 0)
    .with_node(DialogNode::new(0, "NPC", "Hello traveler!").with_next(1))
    .with_node(
        DialogNode::new(1, "NPC", "How can I help you?")
            .with_choice("Tell me about the quest", 2)
            .with_choice("Goodbye", 3),
    )
    .with_node(
        DialogNode::new(2, "NPC", "There is a dragon...")
            .with_effect(DialogEffect::SetFlag("quest_started".into(), 1))
            .with_next(3),
    )
    .with_node(DialogNode::new(3, "NPC", "Farewell!"));
```

## Internal Design

### State Storage

`DialogState` uses `FxHashMap<String, i32>` (from the `rustc_hash` crate) for fast flag lookups. All flags are stored as `i32`, enabling boolean flags (0/1), counters, and numeric variables in a single map. Absent keys default to 0 via `get_flag()`.

`DialogState` derives `Serialize` and `Deserialize`, enabling save/load of dialog progress alongside other game state. Serialization produces a JSON object of key-value pairs.

### Node Lookup

`DialogTree` stores nodes in a `FxHashMap<DialogId, DialogNode>` for O(1) lookups by ID. The `DialogId` newtype wraps a `u32` and implements `Hash` and `Eq`.

### Choice Filtering

When the runner enters a node, it iterates all choices and evaluates each one's condition against the current `DialogState`. The `available_choices` vector stores the original indices into the node's `choices` vector. When the player selects choice N from the filtered list, the runner maps it through this indirection (via `available_choices[choice_index]`) to find the original choice in the node's choices array.

### Effect Separation

The runner distinguishes between flag-related effects (handled immediately by `DialogState::apply_effect()`) and game-specific effects (items, exp, battles, sounds). Game effects are buffered in `pending_effects` and returned via `take_effects()`, which uses `std::mem::take` to drain the buffer. This design avoids coupling the dialog system to any specific game systems -- the game loop polls for effects and handles them in its own way.

### Node Skipping

If a node's conditions fail during `enter_node()`, the runner automatically follows the `next` pointer to skip to the subsequent node. If no `next` is set, the dialog ends. This enables conditional node sequences where certain story beats are only shown to players who meet specific criteria.

## Non-Goals

- Rich text formatting or markup parsing within dialog text.
- Voice-over integration (audio playback is delegated via `PlaySound` effect).
- Visual layout or dialog box rendering (the system provides data; the UI layer renders it).
- Localization (dialog text is stored as-is; localization is handled externally).
- Inventory integration (`HasItem` condition always returns `true` at the engine level).
- Automatic NPC scheduling or proximity triggers (the game decides when to start a dialog).
- Typewriter text animation (the UI layer handles text display timing).

## Open Questions

- Whether to add a `DialogEvent` enum for external subscribers (dialog started, choice made, dialog ended).
- Whether `HasItem` should accept a callback or trait object for game-specific inventory checks.
- Whether to support inline text variables (e.g., `"You have {gold} gold"`).
- Whether to add a `DialogValidator` that checks trees for unreachable nodes or broken links.
- Whether to support an Ink-compatible import format alongside the native RON format.

## Referenzen

- [engine/core](core.md) -- ECS integration and game state serialization
- [engine/ui](ui.md) -- Dialog box rendering and choice display
- [engine/save-load](save-load.md) -- Persisting `DialogState` across sessions
- [engine/localization](localization.md) -- Translating dialog text
- [gametypes/visual-novel](../gametypes/visual-novel.md) -- Extended use case for branching narratives
