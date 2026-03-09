pub mod gamepad;

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
    mouse_world_position: RenderVec2,
    mouse_scroll_delta: f32,
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
            mouse_world_position: RenderVec2::ZERO,
            mouse_scroll_delta: 0.0,
        }
    }

    /// Call at the start of each frame to clear per-frame events.
    pub fn begin_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.mouse_buttons_pressed.clear();
        self.mouse_buttons_released.clear();
        self.mouse_scroll_delta = 0.0;
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
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_position = RenderVec2::new(x, y);
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

    /// Mouse position in screen coordinates.
    pub fn mouse_pos(&self) -> RenderVec2 {
        self.mouse_position
    }

    /// Mouse position in world coordinates.
    pub fn mouse_world_pos(&self) -> RenderVec2 {
        self.mouse_world_position
    }

    /// Mouse scroll delta this frame.
    pub fn scroll_delta(&self) -> f32 {
        self.mouse_scroll_delta
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}
