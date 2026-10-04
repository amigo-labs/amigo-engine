use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use winit::keyboard::KeyCode;

/// A named game action (e.g. "jump", "attack", "move_left").
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(pub String);

impl ActionId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// An input source that can be bound to an action.
/// Keys are stored as strings (e.g. "Space", "KeyW") since winit KeyCode
/// doesn't implement Serialize; [`key_from_name`] lists the accepted names.
///
/// In RON a gamepad button can be written by name or by index:
/// `GamepadButton("South")` and `GamepadButton(0)` are the same binding.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputBinding {
    Key(String),
    MouseButton(u8), // 0=Left, 1=Right, 2=Middle
    GamepadButton(#[serde(deserialize_with = "gamepad_button_index")] u8),
}

/// Accept a gamepad button as its index or as any name
/// [`str_to_button`](super::gamepad::str_to_button) knows ("South", "A",
/// "Cross", ...). Hand-written binding files should not need the index table.
fn gamepad_button_index<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IndexOrName {
        Index(u8),
        Name(String),
    }
    match IndexOrName::deserialize(deserializer)? {
        IndexOrName::Index(index) => Ok(index),
        IndexOrName::Name(name) => super::gamepad::str_to_button(&name)
            .map(button_to_index)
            .filter(|&index| index != UNMAPPED_BUTTON)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown gamepad button {name:?}"))),
    }
}

const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

const NUMPAD_DIGITS: [KeyCode; 10] = [
    KeyCode::Numpad0,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::Numpad5,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad8,
    KeyCode::Numpad9,
];

const FUNCTION_KEYS: [KeyCode; 12] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
];

/// Convert a key name from a binding file to a winit `KeyCode`.
///
/// Accepts winit's own names (`"KeyW"`, `"Digit1"`, `"ArrowUp"`, `"Escape"`,
/// `"ShiftLeft"`, `"Numpad4"`, `"F1"`), and the short forms people write by
/// hand: single letters and digits (`"W"`, `"1"`), `"Up"`/`"Down"`/`"Left"`/
/// `"Right"`, `"Esc"`, `"Return"`, and `"Shift"`/`"Ctrl"`/`"Alt"` for the
/// left-hand modifier.
pub fn key_from_name(name: &str) -> Option<KeyCode> {
    // Single letters and digits, and winit's "KeyX" / "DigitN" / "NumpadN".
    let indexed = |rest: &str, table: &[KeyCode], first: u8| -> Option<KeyCode> {
        match rest.as_bytes() {
            [c] => table
                .get(c.to_ascii_uppercase().checked_sub(first)? as usize)
                .copied(),
            _ => None,
        }
    };
    if let Some(key) = indexed(name, &LETTERS, b'A').or_else(|| indexed(name, &DIGITS, b'0')) {
        return Some(key);
    }
    if let Some(rest) = name.strip_prefix("Key") {
        return indexed(rest, &LETTERS, b'A');
    }
    if let Some(rest) = name.strip_prefix("Digit") {
        return indexed(rest, &DIGITS, b'0');
    }
    if let Some(rest) = name.strip_prefix("Numpad")
        && let Some(key) = indexed(rest, &NUMPAD_DIGITS, b'0')
    {
        return Some(key);
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<usize>().ok()) {
        return n.checked_sub(1).and_then(|i| FUNCTION_KEYS.get(i)).copied();
    }

    Some(match name {
        "Space" => KeyCode::Space,
        "Enter" | "Return" => KeyCode::Enter,
        "Escape" | "Esc" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "CapsLock" => KeyCode::CapsLock,
        "ShiftLeft" | "Shift" => KeyCode::ShiftLeft,
        "ShiftRight" => KeyCode::ShiftRight,
        "ControlLeft" | "Ctrl" | "Control" => KeyCode::ControlLeft,
        "ControlRight" => KeyCode::ControlRight,
        "AltLeft" | "Alt" => KeyCode::AltLeft,
        "AltRight" => KeyCode::AltRight,
        "ArrowUp" | "Up" => KeyCode::ArrowUp,
        "ArrowDown" | "Down" => KeyCode::ArrowDown,
        "ArrowLeft" | "Left" => KeyCode::ArrowLeft,
        "ArrowRight" | "Right" => KeyCode::ArrowRight,
        "Minus" => KeyCode::Minus,
        "Equal" => KeyCode::Equal,
        "Comma" => KeyCode::Comma,
        "Period" => KeyCode::Period,
        "Slash" => KeyCode::Slash,
        "Backslash" => KeyCode::Backslash,
        "Semicolon" => KeyCode::Semicolon,
        "Quote" => KeyCode::Quote,
        "Backquote" => KeyCode::Backquote,
        "BracketLeft" => KeyCode::BracketLeft,
        "BracketRight" => KeyCode::BracketRight,
        "NumpadAdd" => KeyCode::NumpadAdd,
        "NumpadSubtract" => KeyCode::NumpadSubtract,
        "NumpadMultiply" => KeyCode::NumpadMultiply,
        "NumpadDivide" => KeyCode::NumpadDivide,
        "NumpadEnter" => KeyCode::NumpadEnter,
        "NumpadDecimal" => KeyCode::NumpadDecimal,
        _ => return None,
    })
}

/// A complete set of action bindings, serializable for save/load.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActionBindings {
    /// Map from action name to list of bindings.
    pub bindings: FxHashMap<String, Vec<InputBinding>>,
}

impl ActionBindings {
    pub fn new() -> Self {
        Self {
            bindings: FxHashMap::default(),
        }
    }

    /// Bind a key to an action using a string name (e.g. "Space", "KeyW").
    pub fn bind_key(&mut self, action: &str, key: &str) {
        self.bindings
            .entry(action.to_string())
            .or_default()
            .push(InputBinding::Key(key.to_string()));
    }

    /// Bind a mouse button to an action.
    pub fn bind_mouse(&mut self, action: &str, button: u8) {
        self.bindings
            .entry(action.to_string())
            .or_default()
            .push(InputBinding::MouseButton(button));
    }

    /// Bind a gamepad button to an action using a string name (e.g. "South", "A", "DPadUp").
    pub fn bind_gamepad(&mut self, action: &str, button_name: &str) {
        if let Some(idx) = super::gamepad::str_to_button(button_name).map(button_to_index) {
            self.bindings
                .entry(action.to_string())
                .or_default()
                .push(InputBinding::GamepadButton(idx));
        }
    }

    /// Remove all bindings for an action.
    pub fn unbind(&mut self, action: &str) {
        self.bindings.remove(action);
    }

    /// Remove a specific binding from an action.
    pub fn unbind_input(&mut self, action: &str, binding: &InputBinding) {
        if let Some(binds) = self.bindings.get_mut(action) {
            binds.retain(|b| b != binding);
        }
    }

    /// Get all bindings for an action.
    pub fn get_bindings(&self, action: &str) -> &[InputBinding] {
        self.bindings
            .get(action)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Save bindings to a RON string.
    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())
    }

    /// Load bindings from a RON string.
    pub fn from_ron(s: &str) -> Result<Self, String> {
        ron::from_str(s).map_err(|e| e.to_string())
    }

    /// Save to a file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let ron = self.to_ron()?;
        std::fs::write(path, ron).map_err(|e| e.to_string())
    }

    /// Load from a file.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::from_ron(&contents)
    }

    /// Bindings that can never fire: key names [`key_from_name`] does not
    /// know, mouse buttons other than 0-2, and gamepad indices other than
    /// 0-11. `ActionState::update` skips these silently, so check after
    /// loading a hand-written file.
    pub fn unknown_inputs(&self) -> Vec<(String, InputBinding)> {
        let mut unknown: Vec<(String, InputBinding)> = self
            .bindings
            .iter()
            .flat_map(|(action, inputs)| inputs.iter().map(move |input| (action, input)))
            .filter(|(_, input)| match input {
                InputBinding::Key(name) => key_from_name(name).is_none(),
                InputBinding::MouseButton(button) => *button > 2,
                InputBinding::GamepadButton(index) => index_to_button(*index).is_none(),
            })
            .map(|(action, input)| (action.clone(), input.clone()))
            .collect();
        unknown.sort_by(|a, b| a.0.cmp(&b.0));
        unknown
    }
}

/// `button_to_index` result for buttons with no binding slot.
const UNMAPPED_BUTTON: u8 = 255;

/// Map a gilrs Button to a u8 index for storage in `InputBinding::GamepadButton`.
fn button_to_index(button: gilrs::Button) -> u8 {
    use gilrs::Button::*;
    match button {
        South => 0,
        East => 1,
        North => 2,
        West => 3,
        DPadUp => 4,
        DPadDown => 5,
        DPadLeft => 6,
        DPadRight => 7,
        LeftTrigger => 8,
        RightTrigger => 9,
        Start => 10,
        Select => 11,
        _ => UNMAPPED_BUTTON,
    }
}

/// Map a u8 index back to a gilrs Button.
fn index_to_button(idx: u8) -> Option<gilrs::Button> {
    use gilrs::Button::*;
    match idx {
        0 => Some(South),
        1 => Some(East),
        2 => Some(North),
        3 => Some(West),
        4 => Some(DPadUp),
        5 => Some(DPadDown),
        6 => Some(DPadLeft),
        7 => Some(DPadRight),
        8 => Some(LeftTrigger),
        9 => Some(RightTrigger),
        10 => Some(Start),
        11 => Some(Select),
        _ => None,
    }
}

/// Runtime action state, updated each frame from InputState + ActionBindings.
pub struct ActionState {
    pressed: FxHashSet<String>,
    held: FxHashSet<String>,
    released: FxHashSet<String>,
}

impl ActionState {
    pub fn new() -> Self {
        Self {
            pressed: FxHashSet::default(),
            held: FxHashSet::default(),
            released: FxHashSet::default(),
        }
    }

    /// Update action states from current input. Call once per frame.
    ///
    /// Pass `Some(gamepad)` to also process gamepad button bindings.
    /// If `None`, gamepad bindings are skipped.
    pub fn update(
        &mut self,
        input: &super::InputState,
        bindings: &ActionBindings,
        gamepad: Option<&super::gamepad::GamepadState>,
    ) {
        self.pressed.clear();
        self.released.clear();
        self.held.clear();

        for (action, inputs) in &bindings.bindings {
            for binding in inputs {
                match binding {
                    InputBinding::Key(key_name) => {
                        let Some(key) = key_from_name(key_name) else {
                            continue;
                        };
                        if input.pressed(key) {
                            self.pressed.insert(action.clone());
                        }
                        if input.held(key) {
                            self.held.insert(action.clone());
                        }
                        if input.released(key) {
                            self.released.insert(action.clone());
                        }
                    }
                    InputBinding::MouseButton(btn) => {
                        let mb = match btn {
                            0 => winit::event::MouseButton::Left,
                            1 => winit::event::MouseButton::Right,
                            2 => winit::event::MouseButton::Middle,
                            _ => continue,
                        };
                        if input.mouse_pressed(mb) {
                            self.pressed.insert(action.clone());
                        }
                        if input.mouse_held(mb) {
                            self.held.insert(action.clone());
                        }
                        if input.mouse_released(mb) {
                            self.released.insert(action.clone());
                        }
                    }
                    InputBinding::GamepadButton(idx) => {
                        let Some(gp) = gamepad else { continue };
                        let Some(button) = index_to_button(*idx) else {
                            continue;
                        };
                        // Check all connected gamepads — any matching triggers the action
                        for pad_id in gp.connected_ids() {
                            if gp.pressed(pad_id, button) {
                                self.pressed.insert(action.clone());
                            }
                            if gp.held(pad_id, button) {
                                self.held.insert(action.clone());
                            }
                            if gp.released(pad_id, button) {
                                self.released.insert(action.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    /// Action was just triggered this frame.
    pub fn pressed(&self, action: &str) -> bool {
        self.pressed.contains(action)
    }

    /// Action is currently active.
    pub fn held(&self, action: &str) -> bool {
        self.held.contains(action)
    }

    /// Action was just released this frame.
    pub fn released(&self, action: &str) -> bool {
        self.released.contains(action)
    }
}

impl Default for ActionState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_and_serialize() {
        let mut bindings = ActionBindings::new();
        bindings.bind_key("jump", "Space");
        bindings.bind_key("jump", "KeyW");
        bindings.bind_mouse("attack", 0);

        let ron = bindings.to_ron().unwrap();
        let loaded = ActionBindings::from_ron(&ron).unwrap();
        assert_eq!(loaded.get_bindings("jump").len(), 2);
        assert_eq!(loaded.get_bindings("attack").len(), 1);
    }

    #[test]
    fn short_and_winit_key_names_resolve_to_the_same_key() {
        for (short, long) in [
            ("W", "KeyW"),
            ("w", "KeyW"),
            ("7", "Digit7"),
            ("Up", "ArrowUp"),
            ("Esc", "Escape"),
            ("Return", "Enter"),
            ("Shift", "ShiftLeft"),
        ] {
            assert_eq!(key_from_name(short), key_from_name(long), "{short}");
            assert!(key_from_name(long).is_some(), "{long}");
        }
        assert_eq!(key_from_name("F12"), Some(KeyCode::F12));
        assert_eq!(key_from_name("Numpad3"), Some(KeyCode::Numpad3));
        assert_eq!(key_from_name("F13"), None);
        assert_eq!(key_from_name("F0"), None);
        assert_eq!(key_from_name("KeyAA"), None);
        assert_eq!(key_from_name("Jump"), None);
    }

    #[test]
    fn gamepad_buttons_load_by_name_or_index() {
        let bindings = ActionBindings::from_ron(
            r#"(bindings: { "jump": [GamepadButton("South"), GamepadButton("A"), GamepadButton(0)] })"#,
        )
        .unwrap();
        assert_eq!(
            bindings.get_bindings("jump"),
            vec![InputBinding::GamepadButton(0); 3].as_slice()
        );
        assert!(
            ActionBindings::from_ron(r#"(bindings: { "jump": [GamepadButton("Turbo")] })"#)
                .is_err()
        );
    }

    #[test]
    fn unknown_inputs_lists_bindings_that_cannot_fire() {
        let mut bindings = ActionBindings::new();
        bindings.bind_key("jump", "Space");
        bindings.bind_key("jump", "Spacebar");
        bindings.bind_mouse("attack", 7);
        bindings
            .bindings
            .entry("menu".to_string())
            .or_default()
            .push(InputBinding::GamepadButton(40));
        assert_eq!(
            bindings.unknown_inputs(),
            vec![
                ("attack".to_string(), InputBinding::MouseButton(7)),
                (
                    "jump".to_string(),
                    InputBinding::Key("Spacebar".to_string())
                ),
                ("menu".to_string(), InputBinding::GamepadButton(40)),
            ]
        );
    }

    #[test]
    fn actions_follow_bound_keys_and_mouse_buttons() {
        use winit::event::{ElementState, MouseButton};
        use winit::keyboard::PhysicalKey;

        let mut bindings = ActionBindings::new();
        bindings.bind_key("jump", "Space");
        bindings.bind_key("jump", "W");
        bindings.bind_mouse("attack", 0);
        let gamepad = super::super::gamepad::GamepadState::disabled();
        let mut input = super::super::InputState::new();
        let mut actions = ActionState::new();

        input.handle_key_event(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed);
        actions.update(&input, &bindings, Some(&gamepad));
        assert!(actions.pressed("jump") && actions.held("jump"));
        assert!(!actions.held("attack"));

        input.begin_frame();
        input.handle_mouse_button(MouseButton::Left, ElementState::Pressed);
        actions.update(&input, &bindings, Some(&gamepad));
        assert!(!actions.pressed("jump") && actions.held("jump"));
        assert!(actions.pressed("attack"));

        input.begin_frame();
        input.handle_key_event(PhysicalKey::Code(KeyCode::KeyW), ElementState::Released);
        actions.update(&input, &bindings, Some(&gamepad));
        assert!(actions.released("jump") && !actions.held("jump"));
    }

    #[test]
    fn unbind_specific() {
        let mut bindings = ActionBindings::new();
        bindings.bind_key("jump", "Space");
        bindings.bind_key("jump", "KeyW");
        bindings.unbind_input("jump", &InputBinding::Key("Space".to_string()));
        assert_eq!(bindings.get_bindings("jump").len(), 1);
    }
}
