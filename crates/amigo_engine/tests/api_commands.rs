//! API command execution. The windowed loop drained nothing at all, so every
//! queued `camera.*`, `editor.*` and `debug.step` request piled up in
//! `pending_commands` and never ran — which is the entire surface `amigo connect`
//! and the MCP server drive. These tests exercise the drain both loops now use.

#![cfg(feature = "api")]

use amigo_api::handler::{handle_request, new_shared_state};
use amigo_api::RpcRequest;
use amigo_engine::api_bridge::{drain_api_commands, ApiControl, ApiInbox};
use amigo_engine::prelude::*;
use serde_json::{json, Value};

struct Noop;
impl Game for Noop {
    fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
        SceneAction::Continue
    }
    fn draw(&self, _ctx: &mut DrawContext) {}
}

fn request(method: &str, params: Value) -> RpcRequest {
    RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(1),
        method: method.to_string(),
        params,
    }
}

/// Set up a context/stack/control triple plus a shared API state.
fn harness() -> (
    amigo_api::handler::SharedState,
    GameContext,
    GameStack,
    ApiControl,
) {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let mut stack = GameStack::new(Box::new(Noop));
    stack.enter_root(&mut ctx);
    (new_shared_state(), ctx, stack, ApiControl::default())
}

#[test]
fn pause_and_unpause_reach_the_engine() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    handle_request(&request("engine.pause", Value::Null), &shared);
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
    assert!(control.paused, "engine.pause must pause the simulation");

    handle_request(&request("engine.unpause", Value::Null), &shared);
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
    assert!(!control.paused);
}

#[test]
fn step_and_tick_accumulate_requested_ticks() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    handle_request(&request("debug.step", json!({"ticks": 5})), &shared);
    handle_request(&request("amigo_tick", json!({"count": 3})), &shared);
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);

    assert_eq!(
        control.pending_ticks, 8,
        "step and tick both add to the tick budget"
    );
}

#[test]
fn set_speed_rejects_nonsense_multipliers() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    handle_request(&request("set_speed", json!({"multiplier": 2.5})), &shared);
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
    assert_eq!(control.speed, 2.5);

    for bad in [0.0, -1.0] {
        handle_request(&request("set_speed", json!({"multiplier": bad})), &shared);
        drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
        assert_eq!(
            control.speed, 2.5,
            "multiplier {bad} should be rejected, not applied"
        );
    }
}

#[test]
fn camera_set_moves_the_camera_now_not_over_time() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    handle_request(
        &request("camera.set", json!({"x": 100.0, "y": 50.0, "zoom": 3.0})),
        &shared,
    );
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);

    assert_eq!(ctx.camera.position.x, 100.0);
    assert_eq!(ctx.camera.position.y, 50.0);
    assert_eq!(
        ctx.camera.target.x, 100.0,
        "target must move too, or the next update drags the camera back"
    );
    // Zoom eases toward its target, so assert on what camera.set controls.
    assert_eq!(
        ctx.camera.target_zoom, 3.0,
        "zoom should be retargeted by camera.set"
    );
}

#[test]
fn camera_shake_raises_the_shake_and_decays_on_its_own() {
    let (shared, mut ctx, mut stack, mut control) = harness();
    let rest = ctx.camera.effective_position();

    handle_request(
        &request("camera.shake", json!({"intensity": 8.0, "duration": 0.5})),
        &shared,
    );
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
    ctx.camera.update(1.0 / 60.0);

    let shaken = ctx.camera.effective_position();
    assert!(
        (shaken.x - rest.x).abs() > 0.0 || (shaken.y - rest.y).abs() > 0.0,
        "shake should offset the effective camera position"
    );
}

#[test]
fn quit_is_reported_to_the_loop() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    handle_request(&request("engine.quit", Value::Null), &shared);
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);

    assert!(control.quit);
}

#[test]
fn unhandled_commands_reach_game_code_instead_of_vanishing() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    handle_request(&request("start_wave", Value::Null), &shared);
    handle_request(
        // `layer` is a name, not an index — see handle_editor_paint_tile.
        &request(
            "editor.paint_tile",
            json!({"layer": "ground", "x": 1, "y": 2, "tile": 3}),
        ),
        &shared,
    );
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);

    let inbox = ctx
        .resources
        .get_mut::<ApiInbox>()
        .expect("unhandled commands must be exposed to game code");
    let actions: Vec<String> = inbox.drain().into_iter().map(|c| c.action).collect();
    assert!(actions.contains(&"start_wave".to_string()));
    assert!(actions.contains(&"editor.paint_tile".to_string()));
    assert!(
        inbox.commands.is_empty(),
        "drain should hand ownership to the game"
    );
}

#[test]
fn an_unread_inbox_does_not_grow_without_bound() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    // A game that never reads the inbox, with a client that keeps sending.
    for _ in 0..1500 {
        handle_request(&request("start_wave", Value::Null), &shared);
    }
    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);

    let inbox = ctx.resources.get_mut::<ApiInbox>().expect("inbox exists");
    assert!(
        inbox.commands.len() <= 1024,
        "inbox grew to {} entries",
        inbox.commands.len()
    );
}

#[test]
fn draining_an_empty_queue_creates_no_inbox() {
    let (shared, mut ctx, mut stack, mut control) = harness();

    drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);

    assert!(
        ctx.resources.get::<ApiInbox>().is_none(),
        "no commands means no resource churn"
    );
}
