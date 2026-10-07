//! One simulation tick, shared by the windowed and the headless loop.
//!
//! The two loops used to carry their own copies of this sequence, and they
//! had drifted: headless never ran `Plugin::update` or cleared the UI, and
//! the windowed loop handed every tick of a multi-tick frame the whole
//! frame's `dt`.

use crate::engine::{Plugins, SystemStage};
use crate::stack::GameStack;
use crate::{GameContext, net, replay};
use amigo_core::TimeInfo;
use tracing::info_span;

/// What [`run_tick`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TickOutcome {
    Ran,
    /// A network game is waiting for the other player's input; nothing ran.
    /// Try again next frame.
    Stalled,
    /// The game quit, or the stack ran empty: shut down.
    Quit,
}

/// Run one tick: input (live, replayed or from the network), `Game::update`,
/// the stack's scene change, plugins, ECS and event flush, particles, replay
/// and network bookkeeping, and the end of the tick's one-shot input.
pub(crate) fn run_tick(
    ctx: &mut GameContext,
    stack: &mut GameStack,
    plugins: &mut Plugins,
) -> TickOutcome {
    let _tick_span = info_span!("tick").entered();
    let tick_duration = TimeInfo::TICK_DURATION;
    ctx.time.dt = tick_duration as f32;

    let Some(active) = stack.top_mut() else {
        return TickOutcome::Quit;
    };
    if ctx.net.is_playing() {
        // Decided before anything of the tick happens, so a stalled tick
        // leaves no trace (the UI of the last tick stays on screen, and
        // this tick's presses stay pending for the retry).
        ctx.update_actions();
        if net::before_update(ctx) == net::Prepared::Stall {
            return TickOutcome::Stalled;
        }
    } else {
        replay::before_update(ctx);
    }
    // Immediate-mode UI: clear last tick's commands so a game can just build
    // widgets in `update` without bookkeeping. Calling `ui.begin()` again in
    // game code is harmless.
    ctx.ui.begin();
    {
        let _plugin_span = info_span!("plugin_pre_update").entered();
        plugins.run(SystemStage::PreUpdate, ctx);
    }
    let action = {
        let _update_span = info_span!("game_update").entered();
        active.update(ctx)
    };
    ctx.time.tick += 1;

    // Push/Pop/Replace run the stack's lifecycle hooks; `false` means Quit,
    // or the last game popped itself off and there is nothing left to run.
    let running = stack.apply(action, ctx);
    if running {
        {
            let _plugin_span = info_span!("plugin_update").entered();
            plugins.run(SystemStage::PostUpdate, ctx);
        }
        {
            let _flush_span = info_span!("ecs_flush").entered();
            ctx.world.flush();
            ctx.events.flush();
        }
        ctx.particles.update(tick_duration as f32);
    }
    replay::after_tick(ctx, stack);
    net::after_tick(ctx, stack);

    // Clear edge-detected input (just pressed/released) at the END of every
    // tick, so each press is seen by exactly one tick. Clearing once per
    // frame let every tick of a 2+-tick frame (30/50 Hz displays, a hitch,
    // an API `tick N`) see the same press: Esc opened a pause menu and the
    // menu saw it again and closed itself. Clearing at tick START would
    // instead wipe the events winit delivered before this redraw, and
    // zero-tick frames keep their presses for the next tick. Clearing before
    // `update` would hide input injected by tests or the API.
    ctx.end_tick_input();
    if running {
        TickOutcome::Ran
    } else {
        TickOutcome::Quit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Plugin, PluginContext};
    use crate::{DrawContext, Game, SceneAction};
    use std::sync::{Arc, Mutex};

    type Log = Arc<Mutex<Vec<&'static str>>>;

    fn push(log: &Log, entry: &'static str) {
        log.lock().unwrap_or_else(|e| e.into_inner()).push(entry);
    }

    struct Recorder(Log);

    impl Game for Recorder {
        fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
            push(&self.0, "game");
            SceneAction::Continue
        }
        fn draw(&self, _ctx: &mut DrawContext) {}
    }

    struct Hooks(Log);

    impl Plugin for Hooks {
        fn build(&self, ctx: &mut PluginContext) {
            let pre = self.0.clone();
            ctx.add_system(SystemStage::PreUpdate, move |_| push(&pre, "pre system"));
            let post = self.0.clone();
            ctx.add_system(SystemStage::PostUpdate, move |_| push(&post, "post system"));
        }
        fn pre_update(&mut self, ctx: &mut GameContext) {
            push(&self.0, "pre hook");
            // An input handler: the game never sees this press.
            ctx.input.release_all();
        }
        fn update(&mut self, _ctx: &mut GameContext) {
            push(&self.0, "post hook");
        }
    }

    struct Badge;

    impl Plugin for Badge {
        fn build(&self, _ctx: &mut PluginContext) {}
        fn draw(&self, draw: &mut DrawContext) {
            draw.draw_rect(
                amigo_core::Rect::new(1.0, 2.0, 3.0, 4.0),
                amigo_core::Color::RED,
            );
        }
    }

    #[test]
    fn plugins_draw_into_the_frame() {
        let engine = crate::Engine::build().add_plugin(Badge).build();
        let plugins = engine.into_plugins();
        let ctx = GameContext::new(320.0, 180.0, "assets");
        let mut sprites = Vec::new();
        let mut draw = DrawContext::new(
            &mut sprites,
            &ctx,
            amigo_core::RenderVec2::ZERO,
            320.0,
            180.0,
            0.0,
            amigo_render::TextureId(0),
        );
        plugins.draw(&mut draw);
        assert_eq!(sprites.len(), 1);
        assert_eq!(sprites[0].width, 3.0);
    }

    #[test]
    fn plugins_run_around_the_games_update_in_order() {
        let log = Log::default();
        let engine = crate::Engine::build()
            .add_plugin(Hooks(log.clone()))
            .build();
        let mut plugins = engine.into_plugins();
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        let mut stack = GameStack::new(Box::new(Recorder(log.clone())));
        stack.enter_root(&mut ctx);

        assert_eq!(
            run_tick(&mut ctx, &mut stack, &mut plugins),
            TickOutcome::Ran
        );
        assert_eq!(
            *log.lock().unwrap_or_else(|e| e.into_inner()),
            vec!["pre hook", "pre system", "game", "post hook", "post system"]
        );
    }
}
