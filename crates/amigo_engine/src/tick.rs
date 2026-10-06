//! One simulation tick, shared by the windowed and the headless loop.
//!
//! The two loops used to carry their own copies of this sequence, and they
//! had drifted: headless never ran `Plugin::update` or cleared the UI, and
//! the windowed loop handed every tick of a multi-tick frame the whole
//! frame's `dt`.

use crate::engine::Plugin;
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
    plugins: &mut [Box<dyn Plugin>],
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
            for plugin in plugins.iter_mut() {
                plugin.update(ctx);
            }
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
