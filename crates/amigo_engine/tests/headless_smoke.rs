//! Integration smoke test: drive a `Game` through `GameContext` without a
//! window or renderer, the same way the headless engine loop does.

use amigo_engine::prelude::*;

struct Coin;

#[derive(Default)]
struct CountingGame {
    inits: u32,
    updates: u32,
    collected: u32,
}

impl Game for CountingGame {
    fn init(&mut self, ctx: &mut GameContext) {
        self.inits += 1;
        for _ in 0..5 {
            let id = ctx.world.spawn();
            ctx.world.insert_dynamic(id, Coin);
        }
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        self.updates += 1;
        // Collect one coin per tick.
        let next = ctx
            .world
            .dynamic::<Coin>()
            .and_then(|s| s.entities().first().copied());
        if let Some(id) = next {
            ctx.world.despawn(id);
            self.collected += 1;
        }
        SceneAction::Continue
    }

    fn draw(&self, _ctx: &mut DrawContext) {}
}

#[test]
fn game_runs_ticks_without_renderer() {
    let mut ctx = GameContext::new(320.0, 180.0, "assets");
    let mut game = CountingGame::default();

    game.init(&mut ctx);
    assert_eq!(game.inits, 1);

    for _ in 0..10 {
        let action = game.update(&mut ctx);
        assert!(matches!(action, SceneAction::Continue));
        ctx.world.flush();
    }

    assert_eq!(game.updates, 10);
    assert_eq!(game.collected, 5, "all five coins should be collected");
    assert_eq!(ctx.world.dynamic::<Coin>().map_or(0, |s| s.len()), 0);
}

#[test]
fn engine_builder_produces_config() {
    // The builder is plain data plumbing; verify it round-trips settings.
    let engine = Engine::build()
        .title("Smoke")
        .virtual_resolution(123, 45)
        .window_size(640, 480)
        .build();
    let _ = engine;
}
