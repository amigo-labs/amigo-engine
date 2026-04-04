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
/// doesn't implement Serialize.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputBinding {
    Key(String),
    MouseButton(u8), // 0=Left, 1=Right, 2=Middle
    GamepadButton(u8),
}

/// Convert a string key name to a winit KeyCode.
fn str_to_keycode(s: &str) -> Option<KeyCode> {
    match s {
        "Space" => Some(KeyCode::Space),
        "Enter" => Some(KeyCode::Enter),
        "Escape" => Some(KeyCode::Escape),
        "Tab" => Some(KeyCode::Tab),
        "ShiftLeft" => Some(KeyCode::ShiftLeft),
        "ShiftRight" => Some(KeyCode::ShiftRight),
        "ControlLeft" => Some(KeyCode::ControlLeft),
        "ControlRight" => Some(KeyCode::ControlRight),
        "AltLeft" => Some(KeyCode::AltLeft),
        "AltRight" => Some(KeyCode::AltRight),
        "ArrowUp" => Some(KeyCode::ArrowUp),
        "ArrowDown" => Some(KeyCode::ArrowDown),
        "ArrowLeft" => Some(KeyCode::ArrowLeft),
        "ArrowRight" => Some(KeyCode::ArrowRight),
        "KeyA" => Some(KeyCode::KeyA),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyC" => Some(KeyCode::KeyC),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        "Digit0" => Some(KeyCode::Digit0),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "F11" => Some(KeyCode::F11),
        "F12" => Some(KeyCode::F12),
        _ => None,
    }
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
}

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
        _ => 255,
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
                        let Some(key) = str_to_keycode(key_name) else {
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
    fn unbind_specific() {
        let mut bindings = ActionBindings::new();
        bindings.bind_key("jump", "Space");
        bindings.bind_key("jump", "KeyW");
        bindings.unbind_input("jump", &InputBinding::Key("Space".to_string()));
        assert_eq!(bindings.get_bindings("jump").len(), 1);
    }
}
