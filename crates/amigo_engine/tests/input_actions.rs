//! `ActionBindings`, `ActionState` and `GamepadState` existed in amigo_input
//! but nothing in the engine created or updated them: `examples/starter`
//! shipped an `input.ron` that was never read, and no game could see a
//! gamepad. `GameContext` now carries all three.

use amigo_engine::prelude::*;
use winit::event::ElementState;
use winit::keyboard::PhysicalKey;

#[test]
fn bound_keys_drive_actions_through_the_context() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    ctx.bindings.bind_key("jump", "Space");
    ctx.bindings.bind_gamepad("jump", "South");

    ctx.input
        .handle_key_event(PhysicalKey::Code(KeyCode::Space), ElementState::Pressed);
    ctx.update_actions();
    assert!(ctx.actions.pressed("jump"));
    assert!(ctx.actions.held("jump"));
    assert!(!ctx.actions.held("dash"), "unbound actions never fire");

    // The next tick: still held, no longer a fresh press.
    ctx.input.begin_frame();
    ctx.update_actions();
    assert!(!ctx.actions.pressed("jump"));
    assert!(ctx.actions.held("jump"));
}

#[test]
fn a_directly_built_context_has_no_gamepad_backend() {
    // Tests and headless runs must not open OS device handles.
    let ctx = GameContext::new(320.0, 180.0, "assets");
    assert!(!ctx.gamepad.backend_available());
    assert!(!ctx.gamepad.any_connected());
}

#[test]
fn losing_focus_releases_held_actions() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    ctx.bindings.bind_key("move_right", "D");
    ctx.input
        .handle_key_event(PhysicalKey::Code(KeyCode::KeyD), ElementState::Pressed);
    ctx.update_actions();
    assert!(ctx.actions.held("move_right"));

    // What the engine does on WindowEvent::Focused(false).
    ctx.input.release_all();
    ctx.update_actions();
    assert!(!ctx.actions.held("move_right"));
    assert!(ctx.actions.released("move_right"));
}
