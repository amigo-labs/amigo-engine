use gilrs::{ff, Axis, Button, EventType, GamepadId, Gilrs};
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
    /// Deadzone for analog sticks. Values below this threshold are treated as 0.
    deadzone: f32,
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
            deadzone: 0.15,
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

    /// Get the current value of an axis on the given gamepad, with deadzone applied.
    /// Returns `0.0` if the gamepad or axis is not found, or if the value is within
    /// the deadzone.
    pub fn axis(&self, id: GamepadId, axis: Axis) -> f32 {
        let raw = self.pads
            .get(&id)
            .and_then(|p| p.axis_values.get(&axis).copied())
            .unwrap_or(0.0);
        apply_deadzone(raw, self.deadzone)
    }

    /// Get the raw axis value without deadzone applied.
    pub fn axis_raw(&self, id: GamepadId, axis: Axis) -> f32 {
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

    // ── Deadzone ─────────────────────────────────────────────────────────

    /// Set the deadzone threshold for analog sticks (default 0.15).
    /// Values below this threshold are treated as 0.
    pub fn set_deadzone(&mut self, deadzone: f32) {
        self.deadzone = deadzone.abs().clamp(0.0, 0.95);
    }

    /// Get the current deadzone threshold.
    pub fn deadzone(&self) -> f32 {
        self.deadzone
    }

    // ── Rumble / Force Feedback ──────────────────────────────────────────

    /// Request a simple rumble effect on the given gamepad.
    ///
    /// `strong` controls the low-frequency motor (0.0–1.0).
    /// `weak` controls the high-frequency motor (0.0–1.0).
    /// `duration` is how long the effect plays.
    ///
    /// This is a best-effort API — not all platforms/gamepads support rumble.
    pub fn rumble(
        &mut self,
        id: GamepadId,
        strong: f32,
        weak: f32,
        duration: std::time::Duration,
    ) {
        let strong = strong.clamp(0.0, 1.0);
        let weak = weak.clamp(0.0, 1.0);
        if strong == 0.0 && weak == 0.0 {
            return;
        }
        let dur = ff::Ticks::from_ms(duration.as_millis().min(10_000) as u32);
        if let Ok(effect) = ff::EffectBuilder::new()
            .add_effect(ff::BaseEffect {
                kind: ff::BaseEffectType::Strong { magnitude: (strong * 65535.0) as u16 },
                scheduling: ff::Replay { play_for: dur, ..Default::default() },
                envelope: Default::default(),
            })
            .add_effect(ff::BaseEffect {
                kind: ff::BaseEffectType::Weak { magnitude: (weak * 65535.0) as u16 },
                scheduling: ff::Replay { play_for: dur, ..Default::default() },
                envelope: Default::default(),
            })
            .gamepads(&[id])
            .finish(&mut self.gilrs)
        {
            let _ = effect.play();
        }
    }

    // ── Button name mapping ──────────────────────────────────────────────

    /// Convert a button string name to a gilrs `Button`.
    pub fn str_to_button(name: &str) -> Option<Button> {
        str_to_button(name)
    }

    /// Convert a gilrs `Button` to its string name.
    pub fn button_to_str(button: Button) -> &'static str {
        button_to_str(button)
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

// ── Deadzone helper ─────────────────────────────────────────────────────────

/// Apply a radial deadzone to a single axis value.
fn apply_deadzone(value: f32, deadzone: f32) -> f32 {
    let abs = value.abs();
    if abs < deadzone {
        0.0
    } else {
        // Remap so that just past the deadzone is near 0, and 1.0 stays 1.0
        let remapped = (abs - deadzone) / (1.0 - deadzone);
        remapped.min(1.0) * value.signum()
    }
}

// ── Button name mapping ─────────────────────────────────────────────────────

/// Convert a string to a gilrs `Button`.
pub fn str_to_button(name: &str) -> Option<Button> {
    match name {
        "South" | "A" | "Cross" => Some(Button::South),
        "East" | "B" | "Circle" => Some(Button::East),
        "North" | "Y" | "Triangle" => Some(Button::North),
        "West" | "X" | "Square" => Some(Button::West),
        "DPadUp" => Some(Button::DPadUp),
        "DPadDown" => Some(Button::DPadDown),
        "DPadLeft" => Some(Button::DPadLeft),
        "DPadRight" => Some(Button::DPadRight),
        "LeftTrigger" | "L1" | "LB" => Some(Button::LeftTrigger),
        "RightTrigger" | "R1" | "RB" => Some(Button::RightTrigger),
        "Start" | "Options" | "Menu" => Some(Button::Start),
        "Select" | "Back" | "Share" => Some(Button::Select),
        _ => None,
    }
}

/// Convert a gilrs `Button` to its canonical string name.
pub fn button_to_str(button: Button) -> &'static str {
    match button {
        Button::South => "South",
        Button::East => "East",
        Button::North => "North",
        Button::West => "West",
        Button::DPadUp => "DPadUp",
        Button::DPadDown => "DPadDown",
        Button::DPadLeft => "DPadLeft",
        Button::DPadRight => "DPadRight",
        Button::LeftTrigger => "LeftTrigger",
        Button::RightTrigger => "RightTrigger",
        Button::Start => "Start",
        Button::Select => "Select",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_filters_small_values() {
        assert_eq!(apply_deadzone(0.1, 0.15), 0.0);
        assert_eq!(apply_deadzone(-0.1, 0.15), 0.0);
        assert_eq!(apply_deadzone(0.0, 0.15), 0.0);
    }

    #[test]
    fn deadzone_remaps_above_threshold() {
        let val = apply_deadzone(0.5, 0.15);
        // (0.5 - 0.15) / (1.0 - 0.15) ≈ 0.4118
        assert!((val - 0.4118).abs() < 0.01);
    }

    #[test]
    fn deadzone_preserves_sign() {
        let pos = apply_deadzone(0.8, 0.15);
        let neg = apply_deadzone(-0.8, 0.15);
        assert!(pos > 0.0);
        assert!(neg < 0.0);
        assert!((pos + neg).abs() < 0.001);
    }

    #[test]
    fn deadzone_clamps_at_one() {
        assert_eq!(apply_deadzone(1.0, 0.15), 1.0);
        assert_eq!(apply_deadzone(-1.0, 0.15), -1.0);
    }

    #[test]
    fn button_name_roundtrip() {
        for &btn in &ALL_BUTTONS {
            let name = button_to_str(btn);
            assert_eq!(str_to_button(name), Some(btn), "roundtrip failed for {name}");
        }
    }

    #[test]
    fn button_aliases() {
        assert_eq!(str_to_button("A"), Some(Button::South));
        assert_eq!(str_to_button("Cross"), Some(Button::South));
        assert_eq!(str_to_button("LB"), Some(Button::LeftTrigger));
        assert_eq!(str_to_button("Options"), Some(Button::Start));
    }
}
