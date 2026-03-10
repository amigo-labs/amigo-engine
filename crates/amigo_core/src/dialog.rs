use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Dialog identifiers
// ---------------------------------------------------------------------------

/// Unique dialog node identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DialogId(pub u32);

// ---------------------------------------------------------------------------
// Conditions and Effects
// ---------------------------------------------------------------------------

/// Condition that must be met for a dialog node or choice to appear.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DialogCondition {
    FlagSet(String),
    FlagNotSet(String),
    FlagEquals(String, i32),
    FlagGreaterThan(String, i32),
    FlagLessThan(String, i32),
    HasItem(u32, u32),
    And(Vec<DialogCondition>),
    Or(Vec<DialogCondition>),
    Not(Box<DialogCondition>),
}

/// Side effect triggered when a dialog node is entered or a choice is made.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DialogEffect {
    SetFlag(String, i32),
    ClearFlag(String),
    IncrementFlag(String, i32),
    GiveItem(u32, u32),
    TakeItem(u32, u32),
    GiveExp(u32),
    Heal,
    StartBattle(u32),
    PlaySound(String),
    Custom(String),
}

// ---------------------------------------------------------------------------
// Dialog nodes and choices
// ---------------------------------------------------------------------------

/// A choice the player can make during dialog.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogChoice {
    pub text: String,
    pub next: DialogId,
    pub condition: Option<DialogCondition>,
    pub effects: Vec<DialogEffect>,
}

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
    pub fn new(id: u32, speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: DialogId(id),
            speaker: speaker.into(),
            portrait: String::new(),
            text: text.into(),
            next: None,
            choices: Vec::new(),
            conditions: Vec::new(),
            effects: Vec::new(),
        }
    }

    pub fn with_next(mut self, next: u32) -> Self {
        self.next = Some(DialogId(next));
        self
    }

    pub fn with_portrait(mut self, portrait: impl Into<String>) -> Self {
        self.portrait = portrait.into();
        self
    }

    pub fn with_choice(mut self, text: impl Into<String>, next: u32) -> Self {
        self.choices.push(DialogChoice {
            text: text.into(),
            next: DialogId(next),
            condition: None,
            effects: Vec::new(),
        });
        self
    }

    pub fn with_conditional_choice(
        mut self,
        text: impl Into<String>,
        next: u32,
        condition: DialogCondition,
    ) -> Self {
        self.choices.push(DialogChoice {
            text: text.into(),
            next: DialogId(next),
            condition: Some(condition),
            effects: Vec::new(),
        });
        self
    }

    pub fn with_effect(mut self, effect: DialogEffect) -> Self {
        self.effects.push(effect);
        self
    }

    pub fn with_condition(mut self, condition: DialogCondition) -> Self {
        self.conditions.push(condition);
        self
    }
}

// ---------------------------------------------------------------------------
// Dialog tree (a complete conversation)
// ---------------------------------------------------------------------------

/// A complete dialog tree containing connected nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogTree {
    pub id: u32,
    pub name: String,
    pub entry_point: DialogId,
    pub nodes: FxHashMap<DialogId, DialogNode>,
}

impl DialogTree {
    pub fn new(id: u32, name: impl Into<String>, entry_point: u32) -> Self {
        Self {
            id,
            name: name.into(),
            entry_point: DialogId(entry_point),
            nodes: FxHashMap::default(),
        }
    }

    pub fn add_node(&mut self, node: DialogNode) {
        self.nodes.insert(node.id, node);
    }

    pub fn with_node(mut self, node: DialogNode) -> Self {
        self.add_node(node);
        self
    }

    pub fn get_node(&self, id: DialogId) -> Option<&DialogNode> {
        self.nodes.get(&id)
    }
}

// ---------------------------------------------------------------------------
// Dialog state (persistent, saveable)
// ---------------------------------------------------------------------------

/// Persistent dialog state (flags, variables). Saved with the game.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DialogState {
    flags: FxHashMap<String, i32>,
}

impl DialogState {
    pub fn new() -> Self {
        Self {
            flags: FxHashMap::default(),
        }
    }

    pub fn set_flag(&mut self, key: impl Into<String>, value: i32) {
        self.flags.insert(key.into(), value);
    }

    pub fn get_flag(&self, key: &str) -> i32 {
        self.flags.get(key).copied().unwrap_or(0)
    }

    pub fn has_flag(&self, key: &str) -> bool {
        self.flags.contains_key(key)
    }

    pub fn clear_flag(&mut self, key: &str) {
        self.flags.remove(key);
    }

    pub fn increment_flag(&mut self, key: &str, amount: i32) {
        let val = self.get_flag(key);
        self.set_flag(key.to_string(), val + amount);
    }

    /// Check a dialog condition against this state.
    pub fn check_condition(&self, condition: &DialogCondition) -> bool {
        match condition {
            DialogCondition::FlagSet(key) => self.has_flag(key),
            DialogCondition::FlagNotSet(key) => !self.has_flag(key),
            DialogCondition::FlagEquals(key, val) => self.get_flag(key) == *val,
            DialogCondition::FlagGreaterThan(key, val) => self.get_flag(key) > *val,
            DialogCondition::FlagLessThan(key, val) => self.get_flag(key) < *val,
            DialogCondition::HasItem(_, _) => {
                // Games implement this by checking their inventory
                // Default: true (engine doesn't own inventory reference here)
                true
            }
            DialogCondition::And(conds) => conds.iter().all(|c| self.check_condition(c)),
            DialogCondition::Or(conds) => conds.iter().any(|c| self.check_condition(c)),
            DialogCondition::Not(cond) => !self.check_condition(cond),
        }
    }

    /// Apply a dialog effect to this state.
    pub fn apply_effect(&mut self, effect: &DialogEffect) {
        match effect {
            DialogEffect::SetFlag(key, val) => self.set_flag(key.clone(), *val),
            DialogEffect::ClearFlag(key) => self.clear_flag(key),
            DialogEffect::IncrementFlag(key, amount) => self.increment_flag(key, *amount),
            // Item/exp/heal/battle effects must be handled by the game
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Dialog runner (runtime state machine)
// ---------------------------------------------------------------------------

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

/// Runs a dialog, tracking current position and producing events.
pub struct DialogRunner {
    active: bool,
    current_node: Option<DialogId>,
    current_tree: Option<u32>,
    /// Available choices for the current node (after condition filtering).
    available_choices: Vec<usize>,
    /// Game effects emitted this step.
    pending_effects: Vec<DialogGameEffect>,
}

impl DialogRunner {
    pub fn new() -> Self {
        Self {
            active: false,
            current_node: None,
            current_tree: None,
            available_choices: Vec::new(),
            pending_effects: Vec::new(),
        }
    }

    /// Start a dialog tree.
    pub fn start(&mut self, tree: &DialogTree, state: &mut DialogState) {
        self.active = true;
        self.current_tree = Some(tree.id);
        self.enter_node(tree.entry_point, tree, state);
    }

    fn enter_node(&mut self, id: DialogId, tree: &DialogTree, state: &mut DialogState) {
        let Some(node) = tree.get_node(id) else {
            self.active = false;
            return;
        };

        // Check conditions
        if !node.conditions.iter().all(|c| state.check_condition(c)) {
            // Skip this node — go to next if available
            if let Some(next) = node.next {
                self.enter_node(next, tree, state);
            } else {
                self.active = false;
            }
            return;
        }

        self.current_node = Some(id);

        // Apply effects
        for effect in &node.effects {
            state.apply_effect(effect);
            self.emit_game_effect(effect);
        }

        // Calculate available choices
        self.available_choices.clear();
        for (i, choice) in node.choices.iter().enumerate() {
            if let Some(cond) = &choice.condition {
                if !state.check_condition(cond) {
                    continue;
                }
            }
            self.available_choices.push(i);
        }
    }

    fn emit_game_effect(&mut self, effect: &DialogEffect) {
        match effect {
            DialogEffect::GiveItem(id, count) => {
                self.pending_effects.push(DialogGameEffect::GiveItem(*id, *count));
            }
            DialogEffect::TakeItem(id, count) => {
                self.pending_effects.push(DialogGameEffect::TakeItem(*id, *count));
            }
            DialogEffect::GiveExp(amount) => {
                self.pending_effects.push(DialogGameEffect::GiveExp(*amount));
            }
            DialogEffect::Heal => {
                self.pending_effects.push(DialogGameEffect::Heal);
            }
            DialogEffect::StartBattle(id) => {
                self.pending_effects.push(DialogGameEffect::StartBattle(*id));
            }
            DialogEffect::PlaySound(name) => {
                self.pending_effects.push(DialogGameEffect::PlaySound(name.clone()));
            }
            DialogEffect::Custom(data) => {
                self.pending_effects.push(DialogGameEffect::Custom(data.clone()));
            }
            _ => {}
        }
    }

    /// Advance to the next node (when no choices, just press "next").
    pub fn advance(&mut self, tree: &DialogTree, state: &mut DialogState) {
        if !self.active {
            return;
        }
        let Some(current_id) = self.current_node else { return };
        let Some(node) = tree.get_node(current_id) else {
            self.active = false;
            return;
        };

        // If there are choices, don't advance — use choose() instead
        if !self.available_choices.is_empty() {
            return;
        }

        if let Some(next) = node.next {
            self.enter_node(next, tree, state);
        } else {
            self.active = false;
        }
    }

    /// Choose a dialog option by index into the available choices.
    pub fn choose(&mut self, choice_index: usize, tree: &DialogTree, state: &mut DialogState) {
        if !self.active {
            return;
        }
        let Some(current_id) = self.current_node else { return };
        let Some(node) = tree.get_node(current_id) else { return };

        let Some(&original_index) = self.available_choices.get(choice_index) else { return };
        let choice = &node.choices[original_index];

        // Apply choice effects
        for effect in &choice.effects {
            state.apply_effect(effect);
            self.emit_game_effect(effect);
        }

        self.enter_node(choice.next, tree, state);
    }

    /// Is the dialog currently active?
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the current node ID.
    pub fn current_node_id(&self) -> Option<DialogId> {
        self.current_node
    }

    /// Get the current dialog tree ID.
    pub fn current_tree_id(&self) -> Option<u32> {
        self.current_tree
    }

    /// Get the current node from a tree.
    pub fn current_node<'a>(&self, tree: &'a DialogTree) -> Option<&'a DialogNode> {
        self.current_node.and_then(|id| tree.get_node(id))
    }

    /// Get indices of available choices (after condition filtering).
    pub fn available_choices(&self) -> &[usize] {
        &self.available_choices
    }

    /// Does the current node have choices?
    pub fn has_choices(&self) -> bool {
        !self.available_choices.is_empty()
    }

    /// Take pending game effects (returns and clears them).
    pub fn take_effects(&mut self) -> Vec<DialogGameEffect> {
        std::mem::take(&mut self.pending_effects)
    }

    /// Stop the dialog.
    pub fn stop(&mut self) {
        self.active = false;
        self.current_node = None;
        self.current_tree = None;
        self.available_choices.clear();
    }
}

impl Default for DialogRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Dialog registry
// ---------------------------------------------------------------------------

/// Central registry for all dialog trees.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DialogRegistry {
    trees: FxHashMap<u32, DialogTree>,
}

impl DialogRegistry {
    pub fn new() -> Self {
        Self { trees: FxHashMap::default() }
    }

    pub fn register(&mut self, tree: DialogTree) {
        self.trees.insert(tree.id, tree);
    }

    pub fn get(&self, id: u32) -> Option<&DialogTree> {
        self.trees.get(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_dialog() -> DialogTree {
        DialogTree::new(1, "Test Dialog", 0)
            .with_node(DialogNode::new(0, "NPC", "Hello traveler!").with_next(1))
            .with_node(DialogNode::new(1, "NPC", "How can I help you?")
                .with_choice("Tell me about the quest", 2)
                .with_choice("Goodbye", 3))
            .with_node(DialogNode::new(2, "NPC", "There is a dragon...")
                .with_effect(DialogEffect::SetFlag("quest_started".into(), 1))
                .with_next(3))
            .with_node(DialogNode::new(3, "NPC", "Farewell!"))
    }

    #[test]
    fn linear_dialog_flow() {
        let tree = DialogTree::new(1, "Linear", 0)
            .with_node(DialogNode::new(0, "A", "First").with_next(1))
            .with_node(DialogNode::new(1, "A", "Second").with_next(2))
            .with_node(DialogNode::new(2, "A", "Third"));

        let mut state = DialogState::new();
        let mut runner = DialogRunner::new();
        runner.start(&tree, &mut state);

        assert!(runner.is_active());
        let node = runner.current_node(&tree).unwrap();
        assert_eq!(node.text, "First");

        runner.advance(&tree, &mut state);
        assert_eq!(runner.current_node(&tree).unwrap().text, "Second");

        runner.advance(&tree, &mut state);
        assert_eq!(runner.current_node(&tree).unwrap().text, "Third");

        runner.advance(&tree, &mut state);
        assert!(!runner.is_active());
    }

    #[test]
    fn dialog_with_choices() {
        let tree = simple_dialog();
        let mut state = DialogState::new();
        let mut runner = DialogRunner::new();
        runner.start(&tree, &mut state);

        // First node: "Hello traveler!"
        runner.advance(&tree, &mut state);

        // Second node: choices
        assert!(runner.has_choices());
        assert_eq!(runner.available_choices().len(), 2);

        // Choose "Tell me about the quest"
        runner.choose(0, &tree, &mut state);
        assert_eq!(runner.current_node(&tree).unwrap().text, "There is a dragon...");
        assert_eq!(state.get_flag("quest_started"), 1);

        // Continue to farewell
        runner.advance(&tree, &mut state);
        assert_eq!(runner.current_node(&tree).unwrap().text, "Farewell!");

        runner.advance(&tree, &mut state);
        assert!(!runner.is_active());
    }

    #[test]
    fn conditional_choices() {
        let tree = DialogTree::new(1, "Conditional", 0)
            .with_node(DialogNode::new(0, "NPC", "Want to trade?")
                .with_choice("Yes", 1)
                .with_conditional_choice(
                    "Special offer",
                    2,
                    DialogCondition::FlagSet("vip".into()),
                ))
            .with_node(DialogNode::new(1, "NPC", "Here are my wares."))
            .with_node(DialogNode::new(2, "NPC", "Secret items for VIPs!"));

        // Without VIP flag
        let mut state = DialogState::new();
        let mut runner = DialogRunner::new();
        runner.start(&tree, &mut state);
        assert_eq!(runner.available_choices().len(), 1); // Only "Yes"

        // With VIP flag
        state.set_flag("vip".to_string(), 1);
        runner.start(&tree, &mut state);
        assert_eq!(runner.available_choices().len(), 2); // Both choices
    }

    #[test]
    fn dialog_effects() {
        let tree = DialogTree::new(1, "Effects", 0)
            .with_node(DialogNode::new(0, "NPC", "Take this!")
                .with_effect(DialogEffect::SetFlag("got_gift".into(), 1))
                .with_effect(DialogEffect::GiveItem(42, 3)));

        let mut state = DialogState::new();
        let mut runner = DialogRunner::new();
        runner.start(&tree, &mut state);

        assert_eq!(state.get_flag("got_gift"), 1);

        let effects = runner.take_effects();
        assert_eq!(effects.len(), 1); // GiveItem
        assert!(matches!(effects[0], DialogGameEffect::GiveItem(42, 3)));
    }

    #[test]
    fn dialog_state_serialization() {
        let mut state = DialogState::new();
        state.set_flag("quest_1".to_string(), 1);
        state.set_flag("reputation".to_string(), 50);

        let json = serde_json::to_string(&state).unwrap();
        let loaded: DialogState = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.get_flag("quest_1"), 1);
        assert_eq!(loaded.get_flag("reputation"), 50);
    }

    #[test]
    fn complex_conditions() {
        let state = {
            let mut s = DialogState::new();
            s.set_flag("level".to_string(), 10);
            s.set_flag("has_key".to_string(), 1);
            s
        };

        let cond = DialogCondition::And(vec![
            DialogCondition::FlagGreaterThan("level".into(), 5),
            DialogCondition::FlagSet("has_key".into()),
        ]);
        assert!(state.check_condition(&cond));

        let cond_fail = DialogCondition::And(vec![
            DialogCondition::FlagGreaterThan("level".into(), 20),
            DialogCondition::FlagSet("has_key".into()),
        ]);
        assert!(!state.check_condition(&cond_fail));

        let or_cond = DialogCondition::Or(vec![
            DialogCondition::FlagGreaterThan("level".into(), 20),
            DialogCondition::FlagSet("has_key".into()),
        ]);
        assert!(state.check_condition(&or_cond));

        let not_cond = DialogCondition::Not(Box::new(DialogCondition::FlagSet("nonexistent".into())));
        assert!(state.check_condition(&not_cond));
    }
}
