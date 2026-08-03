//! The scene stack: does a `SceneAction` returned from `Game::update` actually
//! move the stack, and in the documented lifecycle order?
//!
//! Before the stack existed the engine matched only `SceneAction::Quit`, so
//! `Push`/`Pop`/`Replace` compiled and silently did nothing. These tests exist
//! so that cannot regress unnoticed.

use amigo_engine::prelude::*;
use std::sync::{Arc, Mutex};

/// Shared hook log, so assertions can look at ordering across instances.
type Log = Arc<Mutex<Vec<String>>>;

fn record(log: &Log, entry: impl Into<String>) {
    log.lock().expect("log poisoned").push(entry.into());
}

/// A game that records every lifecycle hook and returns a scripted action.
struct Recorder {
    name: &'static str,
    log: Log,
    /// Actions returned from `update`, one per tick, then `Continue` forever.
    script: Vec<SceneAction>,
}

impl Recorder {
    fn new(name: &'static str, log: Log, script: Vec<SceneAction>) -> Self {
        // Reverse so `pop` yields them in the written order.
        let mut script = script;
        script.reverse();
        Self { name, log, script }
    }
}

impl Game for Recorder {
    fn init(&mut self, _ctx: &mut GameContext) {
        record(&self.log, format!("{}:init", self.name));
    }
    fn on_enter(&mut self, _ctx: &mut GameContext) {
        record(&self.log, format!("{}:enter", self.name));
    }
    fn on_pause(&mut self, _ctx: &mut GameContext) {
        record(&self.log, format!("{}:pause", self.name));
    }
    fn on_resume(&mut self, _ctx: &mut GameContext) {
        record(&self.log, format!("{}:resume", self.name));
    }
    fn on_exit(&mut self, _ctx: &mut GameContext) {
        record(&self.log, format!("{}:exit", self.name));
    }
    fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
        record(&self.log, format!("{}:update", self.name));
        self.script.pop().unwrap_or(SceneAction::Continue)
    }
    fn draw(&self, _ctx: &mut DrawContext) {}
}

/// Run the stack like the engine loop does: update the top, apply its action.
/// Returns the number of ticks that ran before the stack asked to stop.
fn run(stack: &mut GameStack, ctx: &mut GameContext, max_ticks: usize) -> usize {
    for tick in 0..max_ticks {
        let Some(active) = stack.top_mut() else {
            return tick;
        };
        let action = active.update(ctx);
        if !stack.apply(action, ctx) {
            return tick + 1;
        }
    }
    max_ticks
}

fn ctx() -> GameContext {
    GameContext::new(320.0, 180.0, "assets")
}

#[test]
fn push_pauses_the_game_below_and_pop_resumes_it() {
    let log: Log = Arc::default();
    let mut ctx = ctx();

    let overlay_log = log.clone();
    let mut stack = GameStack::new(Box::new(Recorder::new(
        "menu",
        log.clone(),
        vec![SceneAction::Push(Box::new(move || {
            Box::new(Recorder::new(
                "play",
                overlay_log.clone(),
                vec![SceneAction::Pop],
            )) as Box<dyn Game>
        }))],
    )));
    stack.enter_root(&mut ctx);

    // menu pushes play (tick 1), play pops itself (tick 2), menu continues.
    let ticks = run(&mut stack, &mut ctx, 3);

    assert_eq!(ticks, 3, "the stack should still be running");
    assert_eq!(stack.depth(), 1, "back to just the menu");
    assert_eq!(
        log.lock().unwrap().as_slice(),
        [
            "menu:init",
            "menu:enter",
            "menu:update",
            "menu:pause",
            "play:init",
            "play:enter",
            "play:update",
            "play:exit",
            "menu:resume",
            "menu:update",
        ]
    );
}

#[test]
fn replace_exits_the_old_game_without_resuming_anything() {
    let log: Log = Arc::default();
    let mut ctx = ctx();

    let next_log = log.clone();
    let mut stack = GameStack::new(Box::new(Recorder::new(
        "loading",
        log.clone(),
        vec![SceneAction::Replace(Box::new(move || {
            Box::new(Recorder::new("menu", next_log.clone(), vec![])) as Box<dyn Game>
        }))],
    )));
    stack.enter_root(&mut ctx);

    run(&mut stack, &mut ctx, 2);

    assert_eq!(stack.depth(), 1, "replace must not grow the stack");
    assert_eq!(
        log.lock().unwrap().as_slice(),
        [
            "loading:init",
            "loading:enter",
            "loading:update",
            "loading:exit",
            "menu:init",
            "menu:enter",
            "menu:update",
        ],
        "no resume hook fires: nothing below was paused"
    );
}

#[test]
fn quit_stops_the_loop() {
    let log: Log = Arc::default();
    let mut ctx = ctx();
    let mut stack = GameStack::new(Box::new(Recorder::new(
        "game",
        log.clone(),
        vec![SceneAction::Quit],
    )));
    stack.enter_root(&mut ctx);

    let ticks = run(&mut stack, &mut ctx, 10);

    assert_eq!(ticks, 1, "Quit ends the loop on the first tick");
}

#[test]
fn popping_the_last_game_stops_the_loop() {
    let log: Log = Arc::default();
    let mut ctx = ctx();
    let mut stack = GameStack::new(Box::new(Recorder::new(
        "only",
        log.clone(),
        vec![SceneAction::Pop],
    )));
    stack.enter_root(&mut ctx);

    let ticks = run(&mut stack, &mut ctx, 10);

    assert_eq!(ticks, 1);
    assert!(stack.is_empty());
    assert!(stack.top().is_none());
    assert!(
        log.lock().unwrap().contains(&"only:exit".to_string()),
        "the popped game still gets its exit hook"
    );
}

#[test]
fn each_pushed_instance_is_initialized_exactly_once() {
    let log: Log = Arc::default();
    let mut ctx = ctx();

    // The menu pushes an overlay that immediately pops, twice over. The factory
    // runs twice, so there are two instances and two `init` calls.
    let overlay_log = log.clone();
    let factory = move || {
        Box::new(Recorder::new(
            "overlay",
            overlay_log.clone(),
            vec![SceneAction::Pop],
        )) as Box<dyn Game>
    };
    let f1 = factory.clone();
    let mut stack = GameStack::new(Box::new(Recorder::new(
        "menu",
        log.clone(),
        vec![
            SceneAction::Push(Box::new(factory)),
            SceneAction::Continue,
            SceneAction::Push(Box::new(f1)),
        ],
    )));
    stack.enter_root(&mut ctx);

    run(&mut stack, &mut ctx, 8);

    let entries = log.lock().unwrap();
    assert_eq!(
        entries.iter().filter(|e| *e == "overlay:init").count(),
        2,
        "one init per instance"
    );
    assert_eq!(
        entries.iter().filter(|e| *e == "menu:init").count(),
        1,
        "the root is initialized once, not on every resume"
    );
}

/// A game that quits after N ticks, driven the way the headless loop drives it.
///
/// The headless loop cannot be started in a test (it owns the process's Ctrl+C
/// handler and blocks), so this exercises the same stack-and-quit contract it
/// relies on: update, apply, stop when apply returns false.
#[test]
fn a_game_that_quits_after_n_ticks_stops_the_loop_there() {
    struct QuitAfter {
        remaining: u32,
        ticks: u32,
    }
    impl Game for QuitAfter {
        fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
            self.ticks += 1;
            if self.remaining == 0 {
                return SceneAction::Quit;
            }
            self.remaining -= 1;
            SceneAction::Continue
        }
        fn draw(&self, _ctx: &mut DrawContext) {}
    }

    let mut ctx = ctx();
    let mut stack = GameStack::new(Box::new(QuitAfter {
        remaining: 5,
        ticks: 0,
    }));
    stack.enter_root(&mut ctx);

    let ticks = run(&mut stack, &mut ctx, 100);

    // Five Continues, then the sixth update returns Quit.
    assert_eq!(ticks, 6, "the loop must stop on Quit, not run to the cap");
}

/// The world and event buffers have to be flushed per tick, or despawns and
/// events pile up — the headless loop does this between updates.
#[test]
fn flushing_between_ticks_applies_despawns() {
    struct Spawner {
        spawned: Vec<EntityId>,
    }
    impl Game for Spawner {
        fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
            if let Some(id) = self.spawned.pop() {
                ctx.world.despawn(id);
            }
            SceneAction::Continue
        }
        fn draw(&self, _ctx: &mut DrawContext) {}
    }

    let mut ctx = ctx();
    let ids: Vec<EntityId> = (0..3).map(|_| ctx.world.spawn()).collect();
    ctx.world.flush();
    assert_eq!(ctx.world.entity_count(), 3);

    let mut stack = GameStack::new(Box::new(Spawner {
        spawned: ids.clone(),
    }));
    stack.enter_root(&mut ctx);

    for _ in 0..3 {
        let action = stack.top_mut().unwrap().update(&mut ctx);
        stack.apply(action, &mut ctx);
        ctx.world.flush();
    }

    assert_eq!(
        ctx.world.entity_count(),
        0,
        "despawns only take effect on flush"
    );
}
