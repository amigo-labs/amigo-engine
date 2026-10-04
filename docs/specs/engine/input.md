---
status: done
crate: amigo_input
depends_on: ["engine/core"]
last_updated: 2026-10-04
---

# Input System

## Purpose

Unified input abstraction supporting keyboard, mouse, and gamepad with action mapping.

## Public API

All on `GameContext` (`crates/amigo_engine/src/context.rs`):

- `input: InputState` — keyboard and mouse: `pressed()`, `held()`, `released()`, `mouse_pressed()`/`mouse_held()`/`mouse_released()`, `mouse_pos()` (window pixels), `mouse_ui_pos()` (virtual pixels, through the renderer's viewport), `mouse_world_pos()`, `scroll_delta()`.
- `gamepad: GamepadState` — gilrs-backed: `pressed/held/released(id, GamepadButton)`, `axis()`, `left_stick()`/`right_stick()`, `left_trigger()`/`right_trigger()`, hot-plug via `just_connected()`/`just_disconnected()`, `rumble()`.
- `bindings: ActionBindings` and `actions: ActionState` — named actions bound to keys, mouse buttons and gamepad buttons: `ctx.actions.pressed("jump")`, `held`, `released`.

## Behavior

- **Per-tick edges.** Presses and releases (keys, mouse, gamepad) are visible to exactly one fixed-timestep tick; a frame with zero ticks keeps them for the next tick. Gamepads are polled once per frame, before the ticks.
- **Actions** are recomputed from `input`, `gamepad` and `bindings` before every `Game::update`. A gamepad binding fires on any connected pad.
- **Bindings** come from `EngineBuilder::input_bindings` (defaults in code), replaced at startup by the RON file named in `[input] bindings` (`input.ron` by default, relative to the working directory) when it exists. A file that fails to parse is logged and ignored. Bindings that can never fire (unknown key names, mouse buttons > 2, unknown gamepad buttons) are logged as warnings. The file is read at startup only; a game can reload it with `ActionBindings::load`.
- **Binding file format:**

  ```ron
  (
      bindings: {
          "jump":  [Key("Space"), Key("W"), GamepadButton("South")],
          "shoot": [MouseButton(0), GamepadButton("RightTrigger")],
      },
  )
  ```

  Keys take winit names (`"KeyW"`, `"ArrowUp"`, `"Escape"`) or short forms (`"W"`, `"Up"`, `"Esc"`, `"Shift"`); see `amigo_input::action_map::key_from_name`. Gamepad buttons take a name (`"South"`/`"A"`/`"Cross"`, `"DPadUp"`, `"Start"`, ...) or an index 0–11.
- **Focus loss** releases every held key and mouse button (`InputState::release_all`), since releases while another window has focus never arrive. With the editor overlay, the release of a key or button the game saw pressed reaches the game even when egui consumed it.
- **No backend, no device access.** `GameContext::new` creates `GamepadState::disabled()`; the windowed engine replaces it with a live gilrs state. Headless runs have no gamepad.
