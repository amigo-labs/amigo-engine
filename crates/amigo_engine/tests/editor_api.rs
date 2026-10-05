//! `editor.*` over the JSON-RPC API, against the engine's own editor session.
//!
//! The commands used to land in `ApiInbox` (or, before that, nowhere), so an
//! agent could "paint" a level all day without a single tile changing.

#![cfg(all(feature = "api", feature = "editor"))]

use amigo_api::RpcRequest;
use amigo_api::handler::{SharedState, handle_request, new_shared_state};
use amigo_editor::{AmigoLevel, EditorSession};
use amigo_engine::api_bridge::{ApiControl, ApiInbox, drain_api_commands, publish_snapshot};
use amigo_engine::prelude::*;
use serde_json::{Value, json};
use std::path::PathBuf;

struct Noop;
impl Game for Noop {
    fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
        SceneAction::Continue
    }
    fn draw(&self, _ctx: &mut DrawContext) {}
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("amigo_editor_api_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Harness {
    shared: SharedState,
    ctx: GameContext,
    stack: GameStack,
    control: ApiControl,
}

impl Harness {
    fn new(dir: &std::path::Path) -> Self {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.resources.insert(EditorSession::new(
            AmigoLevel::new("api", 10, 6, 16),
            dir.join("api.amigo"),
        ));
        let mut stack = GameStack::new(Box::new(Noop));
        stack.enter_root(&mut ctx);
        Self {
            shared: new_shared_state(),
            ctx,
            stack,
            control: ApiControl::default(),
        }
    }

    fn send(&mut self, method: &str, params: Value) {
        let response = handle_request(
            &RpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(1),
                method: method.to_string(),
                params,
            },
            &self.shared,
        );
        assert!(response.error.is_none(), "{method}: {:?}", response.error);
        drain_api_commands(
            &self.shared,
            &mut self.ctx,
            &mut self.stack,
            &mut self.control,
        );
    }

    fn session(&self) -> &EditorSession {
        self.ctx.resources.get::<EditorSession>().unwrap()
    }
}

#[test]
fn editor_commands_edit_the_engines_level() {
    let dir = temp_dir("edit");
    let mut h = Harness::new(&dir);

    h.send(
        "editor.paint_tile",
        json!({"layer": "ground", "x": 2, "y": 3, "tile": 5}),
    );
    h.send(
        "editor.fill_rect",
        json!({"layer": "ground", "x": 0, "y": 0, "w": 4, "h": 1, "tile": 1}),
    );
    h.send(
        "editor.place_entity",
        json!({"type": "chest", "x": 32.0, "y": 16.0}),
    );
    h.send(
        "editor.add_path",
        json!({"points": [[0, 0], [16, 0], [16, 16]]}),
    );

    let level = &h.session().level;
    assert_eq!(level.tile(0, 2, 3), Some(5));
    assert!((0..4).all(|x| level.tile(0, x, 0) == Some(1)));
    assert_eq!(level.entities.len(), 1);
    assert_eq!(level.paths[0].points.len(), 3);
    assert_eq!(
        h.session().state.undo_stack.len(),
        4,
        "fill_rect is one step"
    );

    h.send("editor.undo", Value::Null);
    h.send("editor.undo", Value::Null);
    assert!(h.session().level.entities.is_empty());
    h.send("editor.redo", Value::Null);
    assert_eq!(h.session().level.entities.len(), 1);

    // Nothing fell through to the game's inbox.
    assert!(
        h.ctx
            .resources
            .get::<ApiInbox>()
            .is_none_or(|i| i.commands.is_empty())
    );

    // `engine.get_property {"key": "editor"}` shows what happened.
    publish_snapshot(&h.shared, &h.ctx, &h.control);
    let state = amigo_api::lock_or_recover(&h.shared);
    let editor = &state.snapshot.custom["editor"];
    assert_eq!(editor["entities"], 1);
    assert_eq!(editor["dirty"], true);
}

#[test]
fn saving_writes_the_file_and_tells_the_game() {
    let dir = temp_dir("save");
    let mut h = Harness::new(&dir);
    h.send(
        "editor.paint_tile",
        json!({"layer": "ground", "x": 1, "y": 1, "tile": 3}),
    );
    let target = dir.join("caves.amigo");
    h.send("editor.save", json!({"path": target.to_str().unwrap()}));

    let saved = amigo_core::level::load_level(&target).expect("saved level loads");
    assert_eq!(saved.tile(0, 1, 1), Some(3));
    assert!(!h.session().dirty);

    // Events become readable after the tick that emitted them flushes.
    h.ctx.events.flush();
    assert!(h.ctx.level_reloaded("caves"));
    assert!(!h.ctx.level_reloaded("api"));

    // Loading reads it back into the session.
    h.send("editor.new_level", json!({"width": 4, "height": 4}));
    assert_eq!(h.session().level.width, 4);
    h.send("editor.load", json!({"path": target.to_str().unwrap()}));
    assert_eq!(h.session().level.width, 10);
    assert_eq!(h.session().level.tile(0, 1, 1), Some(3));
}

#[test]
fn a_refused_command_changes_nothing() {
    let dir = temp_dir("refused");
    let mut h = Harness::new(&dir);
    h.send(
        "editor.paint_tile",
        json!({"layer": "sky", "x": 1, "y": 1, "tile": 3}),
    );
    h.send(
        "editor.paint_tile",
        json!({"layer": "ground", "x": 50, "y": 1, "tile": 3}),
    );
    assert!(!h.session().dirty);
    assert!(h.session().state.undo_stack.is_empty());
}
