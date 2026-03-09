use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use rustc_hash::{FxHashMap, FxHashSet};

/// Per-gamepad state tracking buttons and axes.
struct PadState {
    buttons_down: FxHashSet<Button>,
    buttons_pressed: FxHashSet<Button>,
    buttons_released: FxHashSet<Button>,
    axis_values: FxHashMap<Axis, f32>,
}

impl PadState {
    fn new() -> Self {
        Self {
            buttons_down: FxHashSet::default(),
            buttons_pressed: FxHashSet::default(),
            buttons_released: FxHashSet::default(),
            axis_values: FxHashMap::default(),
        }
    }

    fn begin_frame(&mut self) {
        self.buttons_pressed.clear();
        self.buttons_released.clear();
    }
}

/// Tracks all connected gamepads, their button/axis state, and hot-plug events.
pub struct GamepadState {
    gilrs: Gilrs,
    pads: FxHashMap<GamepadId, PadState>,
    connected_this_frame: Vec<GamepadId>,
    disconnected_this_frame: Vec<GamepadId>,
}

impl GamepadState {
    /// Create a new `GamepadState`, initialising the gilrs backend.
    ///
    /// Panics if the platform gamepad subsystem cannot be initialised.
    pub fn new() -> Self {
        let gilrs = Gilrs::new().expect("failed to initialise gilrs");

        // Register any gamepads that are already connected at startup.
        let mut pads = FxHashMap::default();
        for (id, gp) in gilrs.gamepads() {
            if gp.is_connected() {
                pads.insert(id, PadState::new());
            }
        }

        Self {
            gilrs,
            pads,
            connected_this_frame: Vec::new(),
            disconnected_this_frame: Vec::new(),
        }
    }

    /// Call at the start of each frame before polling events.
    /// Clears per-frame pressed/released sets and hot-plug lists.
    pub fn begin_frame(&mut self) {
        self.connected_this_frame.clear();
        self.disconnected_this_frame.clear();
        for pad in self.pads.values_mut() {
            pad.begin_frame();
        }
    }

    /// Poll all pending gilrs events and update internal state.
    /// Call once per frame after `begin_frame`.
    pub fn update(&mut self) {
        while let Some(event) = self.gilrs.next_event() {
            let id = event.id;
            match event.event {
                EventType::Connected => {
                    self.pads.entry(id).or_insert_with(PadState::new);
                    self.connected_this_frame.push(id);
                }
                EventType::Disconnected => {
                    self.pads.remove(&id);
                    self.disconnected_this_frame.push(id);
                }
                EventType::ButtonPressed(button, _) => {
                    if let Some(pad) = self.pads.get_mut(&id) {
                        pad.buttons_down.insert(button);
                        pad.buttons_pressed.insert(button);
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(pad) = self.pads.get_mut(&id) {
                        pad.buttons_down.remove(&button);
                        pad.buttons_released.insert(button);
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    if let Some(pad) = self.pads.get_mut(&id) {
                        pad.axis_values.insert(axis, value);
                    }
                }
                _ => {}
            }
        }

        // Synchronise held-button and axis state from gilrs for accuracy,
        // so that state stays correct even if events were coalesced.
        for (&id, pad) in &mut self.pads {
            let gp = self.gilrs.gamepad(id);
            for &btn in &ALL_BUTTONS {
                if gp.is_pressed(btn) {
                    pad.buttons_down.insert(btn);
                } else {
                    pad.buttons_down.remove(&btn);
                }
            }
            for &axis in &ALL_AXES {
                pad.axis_values.insert(axis, gp.value(axis));
            }
        }
    }

    // ── Query API ──────────────────────────────────────────────────────

    /// Returns `true` if the button was just pressed this frame on the given gamepad.
    pub fn pressed(&self, id: GamepadId, button: Button) -> bool {
        self.pads
            .get(&id)
            .map_or(false, |p| p.buttons_pressed.contains(&button))
    }

    /// Returns `true` if the button is currently held down on the given gamepad.
    pub fn held(&self, id: GamepadId, button: Button) -> bool {
        self.pads
            .get(&id)
            .map_or(false, |p| p.buttons_down.contains(&button))
    }

    /// Returns `true` if the button was just released this frame on the given gamepad.
    pub fn released(&self, id: GamepadId, button: Button) -> bool {
        self.pads
            .get(&id)
            .map_or(false, |p| p.buttons_released.contains(&button))
    }

    /// Get the current value of an axis on the given gamepad.
    /// Returns `0.0` if the gamepad or axis is not found.
    pub fn axis(&self, id: GamepadId, axis: Axis) -> f32 {
        self.pads
            .get(&id)
            .and_then(|p| p.axis_values.get(&axis).copied())
            .unwrap_or(0.0)
    }

    /// Left stick as (x, y), each in -1.0..1.0.
    pub fn left_stick(&self, id: GamepadId) -> (f32, f32) {
        (
            self.axis(id, Axis::LeftStickX),
            self.axis(id, Axis::LeftStickY),
        )
    }

    /// Right stick as (x, y), each in -1.0..1.0.
    pub fn right_stick(&self, id: GamepadId) -> (f32, f32) {
        (
            self.axis(id, Axis::RightStickX),
            self.axis(id, Axis::RightStickY),
        )
    }

    /// Left trigger value, 0.0 (released) to 1.0 (fully pressed).
    pub fn left_trigger(&self, id: GamepadId) -> f32 {
        self.axis(id, Axis::LeftZ)
    }

    /// Right trigger value, 0.0 (released) to 1.0 (fully pressed).
    pub fn right_trigger(&self, id: GamepadId) -> f32 {
        self.axis(id, Axis::RightZ)
    }

    // ── Hot-plug queries ───────────────────────────────────────────────

    /// Gamepad IDs that connected this frame.
    pub fn just_connected(&self) -> &[GamepadId] {
        &self.connected_this_frame
    }

    /// Gamepad IDs that disconnected this frame.
    pub fn just_disconnected(&self) -> &[GamepadId] {
        &self.disconnected_this_frame
    }

    /// Iterator over all currently connected gamepad IDs.
    pub fn connected_ids(&self) -> impl Iterator<Item = GamepadId> + '_ {
        self.pads.keys().copied()
    }

    /// Number of currently connected gamepads.
    pub fn connected_count(&self) -> usize {
        self.pads.len()
    }

    /// Returns `true` if at least one gamepad is connected.
    pub fn any_connected(&self) -> bool {
        !self.pads.is_empty()
    }
}

impl Default for GamepadState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Constants ──────────────────────────────────────────────────────────────

const ALL_BUTTONS: [Button; 12] = [
    Button::South,
    Button::East,
    Button::North,
    Button::West,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::Start,
    Button::Select,
];

const ALL_AXES: [Axis; 4] = [
    Axis::LeftStickX,
    Axis::LeftStickY,
    Axis::RightStickX,
    Axis::RightStickY,
];
