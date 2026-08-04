//! Dev snapshots: `Game::on_dev_snapshot` / `on_dev_restore` had no callers at
//! all, so the documented "state survives a recompile" loop did nothing. These
//! tests drive the round trip the way the engine does.

#![cfg(feature = "api")]

use amigo_engine::api_bridge::{
    apply_dev_snapshot, build_dev_snapshot, load_dev_snapshot, ApiControl,
};
use amigo_engine::prelude::*;
use serde_json::json;

/// A game whose whole state is one counter, persisted through the snapshot blob.
#[derive(Default)]
struct Counter {
    hits: u32,
}

impl Game for Counter {
    fn scene_id(&self) -> &'static str {
        "counter"
    }

    fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
        self.hits += 1;
        SceneAction::Continue
    }

    fn draw(&self, _ctx: &mut DrawContext) {}

    fn on_dev_snapshot(&self, _ctx: &GameContext) -> serde_json::Value {
        json!({ "hits": self.hits })
    }

    fn on_dev_restore(&mut self, _ctx: &mut GameContext, state: &serde_json::Value) {
        if let Some(hits) = state.get("hits").and_then(|v| v.as_u64()) {
            self.hits = hits as u32;
        }
    }
}

fn fresh_ctx() -> GameContext {
    GameContext::new(320.0, 180.0, "assets")
}

#[test]
fn snapshot_captures_engine_and_game_state() {
    let mut ctx = fresh_ctx();
    let mut stack = GameStack::new(Box::new(Counter::default()));
    stack.enter_root(&mut ctx);

    // Advance the game and move the engine off its defaults.
    for _ in 0..7 {
        let action = stack.top_mut().unwrap().update(&mut ctx);
        assert!(stack.apply(action, &mut ctx));
    }
    ctx.time.tick = 4200;
    ctx.camera.position = RenderVec2 { x: 12.0, y: 34.0 };
    ctx.camera.set_zoom_immediate(2.0);
    let control = ApiControl {
        paused: true,
        speed: 0.5,
        ..ApiControl::default()
    };

    let snapshot = build_dev_snapshot(&ctx, &stack, &control);

    assert_eq!(snapshot.tick, 4200);
    assert_eq!(snapshot.camera_zoom, 2.0);
    assert!(snapshot.paused);
    assert_eq!(snapshot.speed_multiplier, 0.5);
    assert_eq!(snapshot.scene_id, "counter");
    assert_eq!(
        snapshot.game_state.get("hits").and_then(|v| v.as_u64()),
        Some(7),
        "the game's own blob has to come from on_dev_snapshot"
    );
}

#[test]
fn restoring_reinstates_tick_camera_and_game_blob() {
    // Snapshot from one session...
    let mut ctx = fresh_ctx();
    let mut stack = GameStack::new(Box::new(Counter::default()));
    stack.enter_root(&mut ctx);
    for _ in 0..3 {
        let action = stack.top_mut().unwrap().update(&mut ctx);
        stack.apply(action, &mut ctx);
    }
    ctx.time.tick = 99;
    ctx.camera.position = RenderVec2 { x: 5.0, y: 6.0 };
    let snapshot = build_dev_snapshot(&ctx, &stack, &ApiControl::default());

    // ...restored into a fresh one, as if the process had restarted.
    let mut ctx2 = fresh_ctx();
    let mut stack2 = GameStack::new(Box::new(Counter::default()));
    stack2.enter_root(&mut ctx2);
    let mut control2 = ApiControl::default();

    apply_dev_snapshot(&snapshot, &mut ctx2, &mut stack2, &mut control2);

    assert_eq!(ctx2.time.tick, 99, "tick must survive the restart");
    assert_eq!(ctx2.camera.position.x, 5.0);
    assert_eq!(ctx2.camera.position.y, 6.0);
    // The blob reached the game: it saw 3 updates in the previous session.
    let snapshot_after = build_dev_snapshot(&ctx2, &stack2, &control2);
    assert_eq!(
        snapshot_after
            .game_state
            .get("hits")
            .and_then(|v| v.as_u64()),
        Some(3),
        "on_dev_restore must have been called with the saved blob"
    );
}

#[test]
fn snapshot_survives_a_round_trip_through_ron_on_disk() {
    let mut ctx = fresh_ctx();
    let mut stack = GameStack::new(Box::new(Counter::default()));
    stack.enter_root(&mut ctx);
    let action = stack.top_mut().unwrap().update(&mut ctx);
    stack.apply(action, &mut ctx);
    ctx.time.tick = 1234;

    let snapshot = build_dev_snapshot(&ctx, &stack, &ApiControl::default());

    // Mirror what save_dev_snapshot writes, then read it back.
    let dir = std::env::temp_dir().join("amigo_dev_snapshot_test");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("snapshot.ron");
    let serialized = ron::ser::to_string_pretty(&snapshot, ron::ser::PrettyConfig::default())
        .expect("serialize snapshot");
    std::fs::write(&path, serialized).expect("write snapshot");

    let loaded = load_dev_snapshot(&path).expect("snapshot must parse back");

    assert_eq!(loaded.tick, 1234);
    assert_eq!(loaded.scene_id, "counter");
    assert_eq!(
        loaded.game_state.get("hits").and_then(|v| v.as_u64()),
        Some(1)
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn restoring_a_missing_file_is_an_error_not_a_panic() {
    let path = std::env::temp_dir().join("amigo_dev_snapshot_absent.ron");
    let _ = std::fs::remove_file(&path);

    assert!(load_dev_snapshot(&path).is_err());
}

#[test]
fn scene_id_defaults_to_the_type_name() {
    struct Unnamed;
    impl Game for Unnamed {
        fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
            SceneAction::Continue
        }
        fn draw(&self, _ctx: &mut DrawContext) {}
    }

    let game = Unnamed;
    assert!(
        game.scene_id().contains("Unnamed"),
        "default scene_id should name the type, got {}",
        game.scene_id()
    );
}
