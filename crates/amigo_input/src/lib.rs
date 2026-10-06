pub mod action_map;
pub mod gamepad;

pub use action_map::{
    ActionBindings, ActionSnapshot, ActionState, InputBinding, key_from_name, key_name, named_keys,
};
pub use gamepad::GamepadState;
/// Gamepad types used by [`GamepadState`]'s queries, re-exported so games do
/// not need their own `gilrs` dependency. Renamed because `Button` and `Axis`
/// are too generic for a prelude.
pub use gilrs::{Axis as GamepadAxis, Button as GamepadButton, GamepadId};

use amigo_core::RenderVec2;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use winit::event::{ElementState, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Abstract action for input mapping.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Action(pub String);

impl Action {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

/// Key binding definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyBinding {
    pub action: String,
    pub key: String,
}

/// The input state, updated each frame from winit events.
pub struct InputState {
    keys_down: FxHashSet<KeyCode>,
    keys_pressed: FxHashSet<KeyCode>,
    keys_released: FxHashSet<KeyCode>,

    mouse_buttons_down: FxHashSet<MouseButton>,
    mouse_buttons_pressed: FxHashSet<MouseButton>,
    mouse_buttons_released: FxHashSet<MouseButton>,
    mouse_position: RenderVec2,
    mouse_ui_position: RenderVec2,
    mouse_world_position: RenderVec2,
    mouse_scroll_delta: f32,
    typed_chars: Vec<char>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys_down: FxHashSet::default(),
            keys_pressed: FxHashSet::default(),
            keys_released: FxHashSet::default(),
            mouse_buttons_down: FxHashSet::default(),
            mouse_buttons_pressed: FxHashSet::default(),
            mouse_buttons_released: FxHashSet::default(),
            mouse_position: RenderVec2::ZERO,
            mouse_ui_position: RenderVec2::ZERO,
            mouse_world_position: RenderVec2::ZERO,
            mouse_scroll_delta: 0.0,
            typed_chars: Vec::new(),
        }
    }

    /// Call at the start of each frame to clear per-frame events.
    pub fn begin_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.mouse_buttons_pressed.clear();
        self.mouse_buttons_released.clear();
        self.mouse_scroll_delta = 0.0;
        self.typed_chars.clear();
    }

    /// Release every held key and mouse button, reporting each as released
    /// this frame.
    ///
    /// Call when the window loses focus: the release events for keys let go
    /// while another window has focus never arrive, so without this they
    /// stay held and a character keeps walking until the key is tapped again.
    pub fn release_all(&mut self) {
        self.keys_released.extend(self.keys_down.drain());
        self.mouse_buttons_released
            .extend(self.mouse_buttons_down.drain());
    }

    /// Process a keyboard event.
    pub fn handle_key_event(&mut self, key: PhysicalKey, state: ElementState) {
        if let PhysicalKey::Code(code) = key {
            match state {
                ElementState::Pressed => {
                    if self.keys_down.insert(code) {
                        self.keys_pressed.insert(code);
                    }
                }
                ElementState::Released => {
                    self.keys_down.remove(&code);
                    self.keys_released.insert(code);
                }
            }
        }
    }

    /// Process a mouse button event.
    pub fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match state {
            ElementState::Pressed => {
                if self.mouse_buttons_down.insert(button) {
                    self.mouse_buttons_pressed.insert(button);
                }
            }
            ElementState::Released => {
                self.mouse_buttons_down.remove(&button);
                self.mouse_buttons_released.insert(button);
            }
        }
    }

    /// Update mouse screen position.
    ///
    /// Also sets the UI-space position to the same point; the engine then
    /// overrides it with [`set_mouse_ui_pos`](Self::set_mouse_ui_pos) once it
    /// knows the window-to-virtual scale. Code that injects input without a
    /// window (tests, headless agents) therefore gets identical coordinates.
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_position = RenderVec2::new(x, y);
        self.mouse_ui_position = self.mouse_position;
    }

    /// Set the mouse position in UI (virtual-resolution) coordinates.
    pub fn set_mouse_ui_pos(&mut self, pos: RenderVec2) {
        self.mouse_ui_position = pos;
    }

    /// Update mouse scroll.
    pub fn handle_scroll(&mut self, delta: f32) {
        self.mouse_scroll_delta += delta;
    }

    /// Set world-space mouse position (computed using camera).
    pub fn set_mouse_world_pos(&mut self, pos: RenderVec2) {
        self.mouse_world_position = pos;
    }

    // ── Query API ──

    /// Key was just pressed this frame.
    pub fn pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Key is currently held down.
    pub fn held(&self, key: KeyCode) -> bool {
        self.keys_down.contains(&key)
    }

    /// Key was just released this frame.
    pub fn released(&self, key: KeyCode) -> bool {
        self.keys_released.contains(&key)
    }

    /// Mouse button was just pressed this frame.
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_pressed.contains(&button)
    }

    /// Mouse button is currently held down.
    pub fn mouse_held(&self, button: MouseButton) -> bool {
        self.mouse_buttons_down.contains(&button)
    }

    /// Mouse button was just released this frame.
    pub fn mouse_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons_released.contains(&button)
    }

    /// Mouse position in screen coordinates (physical window pixels).
    pub fn mouse_pos(&self) -> RenderVec2 {
        self.mouse_position
    }

    /// Mouse position in UI coordinates: the virtual-resolution pixels Pixel
    /// UI is laid out and drawn in. Hit-test widgets against this, not
    /// [`mouse_pos`](Self::mouse_pos) — with a 1280x720 window over a 480x270
    /// virtual resolution the two differ by a factor of 2.67.
    pub fn mouse_ui_pos(&self) -> RenderVec2 {
        self.mouse_ui_position
    }

    /// Mouse position in world coordinates.
    pub fn mouse_world_pos(&self) -> RenderVec2 {
        self.mouse_world_position
    }

    /// Mouse scroll delta this frame.
    pub fn scroll_delta(&self) -> f32 {
        self.mouse_scroll_delta
    }

    /// Record a typed character (from keyboard text input events).
    pub fn handle_text_input(&mut self, ch: char) {
        self.typed_chars.push(ch);
    }

    /// Characters typed this frame (for text fields).
    pub fn text_input(&self) -> &[char] {
        &self.typed_chars
    }

    /// The current state as data, for a replay.
    ///
    /// Keys [`key_name`] has no name for are left out: no binding can name
    /// them either.
    pub fn snapshot(&self) -> InputSnapshot {
        let keys = |set: &FxHashSet<KeyCode>| {
            let mut names: Vec<String> = set
                .iter()
                .filter_map(|&k| key_name(k))
                .map(str::to_string)
                .collect();
            names.sort_unstable();
            names
        };
        let buttons = |set: &FxHashSet<MouseButton>| {
            let mut codes: Vec<u32> = set.iter().map(|&b| mouse_button_code(b)).collect();
            codes.sort_unstable();
            codes
        };
        InputSnapshot {
            held: keys(&self.keys_down),
            pressed: keys(&self.keys_pressed),
            released: keys(&self.keys_released),
            mouse_held: buttons(&self.mouse_buttons_down),
            mouse_pressed: buttons(&self.mouse_buttons_pressed),
            mouse_released: buttons(&self.mouse_buttons_released),
            mouse: [self.mouse_position.x, self.mouse_position.y],
            mouse_ui: [self.mouse_ui_position.x, self.mouse_ui_position.y],
            mouse_world: [self.mouse_world_position.x, self.mouse_world_position.y],
            scroll: self.mouse_scroll_delta,
            text: self.typed_chars.iter().collect(),
        }
    }

    /// Replace the whole state with `snapshot`. Names [`key_from_name`] does
    /// not know are skipped.
    pub fn restore(&mut self, snapshot: &InputSnapshot) {
        let keys = |names: &[String]| names.iter().filter_map(|n| key_from_name(n)).collect();
        let buttons = |codes: &[u32]| codes.iter().map(|&c| mouse_button_from_code(c)).collect();
        let vec = |[x, y]: [f32; 2]| RenderVec2::new(x, y);
        self.keys_down = keys(&snapshot.held);
        self.keys_pressed = keys(&snapshot.pressed);
        self.keys_released = keys(&snapshot.released);
        self.mouse_buttons_down = buttons(&snapshot.mouse_held);
        self.mouse_buttons_pressed = buttons(&snapshot.mouse_pressed);
        self.mouse_buttons_released = buttons(&snapshot.mouse_released);
        self.mouse_position = vec(snapshot.mouse);
        self.mouse_ui_position = vec(snapshot.mouse_ui);
        self.mouse_world_position = vec(snapshot.mouse_world);
        self.mouse_scroll_delta = snapshot.scroll;
        self.typed_chars = snapshot.text.chars().collect();
    }
}

/// An [`InputState`] as data: what one tick saw from the keyboard and mouse.
///
/// Keys are stored by their [`key_name`], mouse buttons as numbers (0 left,
/// 1 right, 2 middle, as in [`InputBinding::MouseButton`], then 3 back,
/// 4 forward and 5 + n for other button n). Positions are `[x, y]` in
/// window, UI and world coordinates. Empty lists are left out when
/// serialized, so a tick with nothing pressed stays short.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub held: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pressed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub released: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mouse_held: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mouse_pressed: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mouse_released: Vec<u32>,
    #[serde(default)]
    pub mouse: [f32; 2],
    #[serde(default)]
    pub mouse_ui: [f32; 2],
    #[serde(default)]
    pub mouse_world: [f32; 2],
    #[serde(default, skip_serializing_if = "is_zero")]
    pub scroll: f32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// The number [`InputSnapshot`] stores for `button`.
pub fn mouse_button_code(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(n) => 5 + u32::from(n),
    }
}

/// The inverse of [`mouse_button_code`].
pub fn mouse_button_from_code(code: u32) -> MouseButton {
    match code {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        n => MouseButton::Other(u16::try_from(n - 5).unwrap_or(u16::MAX)),
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_round_trips_through_json() {
        let mut input = InputState::new();
        input.handle_key_event(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed);
        input.handle_key_event(PhysicalKey::Code(KeyCode::Space), ElementState::Pressed);
        input.begin_frame();
        input.handle_key_event(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed);
        input.handle_key_event(PhysicalKey::Code(KeyCode::Space), ElementState::Released);
        input.handle_mouse_button(MouseButton::Right, ElementState::Pressed);
        input.handle_mouse_button(MouseButton::Other(7), ElementState::Pressed);
        input.handle_mouse_move(100.25, 33.0);
        input.set_mouse_ui_pos(RenderVec2::new(37.59375, 12.375));
        input.set_mouse_world_pos(RenderVec2::new(-1.0e-7, 3.4e38));
        input.handle_scroll(-1.5);
        input.handle_text_input('ä');

        let snapshot = input.snapshot();
        assert_eq!(snapshot.held, ["ArrowUp", "KeyW"]);
        assert_eq!(snapshot.pressed, ["ArrowUp"]);
        assert_eq!(snapshot.released, ["Space"]);
        assert_eq!(snapshot.mouse_held, [1, 12]);

        let json = serde_json::to_string(&snapshot).unwrap();
        let back: InputSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, snapshot);

        let mut restored = InputState::new();
        restored.restore(&back);
        assert!(restored.held(KeyCode::KeyW) && restored.pressed(KeyCode::ArrowUp));
        assert!(restored.released(KeyCode::Space) && !restored.held(KeyCode::Space));
        assert!(restored.mouse_pressed(MouseButton::Other(7)));
        assert_eq!(restored.mouse_ui_pos().x.to_bits(), 37.59375f32.to_bits());
        assert_eq!(
            restored.mouse_world_pos().x.to_bits(),
            (-1.0e-7f32).to_bits()
        );
        assert_eq!(restored.mouse_world_pos().y.to_bits(), 3.4e38f32.to_bits());
        assert_eq!(restored.scroll_delta(), -1.5);
        assert_eq!(restored.text_input(), ['ä']);
        assert_eq!(restored.snapshot(), snapshot);
    }

    #[test]
    fn empty_snapshot_serializes_short() {
        let json = serde_json::to_string(&InputState::new().snapshot()).unwrap();
        assert_eq!(
            json,
            r#"{"mouse":[0.0,0.0],"mouse_ui":[0.0,0.0],"mouse_world":[0.0,0.0]}"#
        );
        assert_eq!(
            serde_json::from_str::<InputSnapshot>("{}").unwrap(),
            InputSnapshot::default()
        );
    }

    #[test]
    fn mouse_button_codes_round_trip() {
        for button in [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
            MouseButton::Other(0),
            MouseButton::Other(u16::MAX),
        ] {
            assert_eq!(mouse_button_from_code(mouse_button_code(button)), button);
        }
    }

    #[test]
    fn release_all_reports_held_inputs_as_released() {
        let mut input = InputState::new();
        input.handle_key_event(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed);
        input.handle_mouse_button(MouseButton::Left, ElementState::Pressed);
        input.begin_frame();

        input.release_all();

        assert!(!input.held(KeyCode::KeyW));
        assert!(input.released(KeyCode::KeyW));
        assert!(!input.mouse_held(MouseButton::Left));
        assert!(input.mouse_released(MouseButton::Left));

        // The release is a one-frame event like any other.
        input.begin_frame();
        assert!(!input.released(KeyCode::KeyW));
        // And pressing again registers as a fresh press.
        input.handle_key_event(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed);
        assert!(input.pressed(KeyCode::KeyW));
    }
}
