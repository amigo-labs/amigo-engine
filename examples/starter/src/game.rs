use crate::states::{LoadingState, MenuState, PlayingState};
use amigo_engine::prelude::*;

/// Top-level game state enum.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Loading,
    Menu,
    Playing,
}

pub struct StarterGame {
    state: AppState,
    loading: LoadingState,
    menu: MenuState,
    playing: PlayingState,
}

impl StarterGame {
    pub fn new() -> Self {
        Self {
            state: AppState::Loading,
            loading: LoadingState::new(),
            menu: MenuState::new(),
            playing: PlayingState::new(),
        }
    }

    fn transition(&mut self, ctx: &mut GameContext, next: AppState) {
        // Clean up entities tagged with the old state
        ctx.world.cleanup_state(self.state as u32);
        ctx.world.flush();

        self.state = next;
        if next == AppState::Playing {
            self.playing.enter(ctx);
        }
    }
}

impl Game for StarterGame {
    fn init(&mut self, _ctx: &mut GameContext) {
        // Assets are loaded synchronously by the engine before this is called.
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        match self.state {
            AppState::Loading => {
                if self.loading.update(ctx) {
                    self.transition(ctx, AppState::Menu);
                }
            }
            AppState::Menu => {
                if self.menu.update(ctx) {
                    self.transition(ctx, AppState::Playing);
                }
            }
            AppState::Playing => {
                if self.playing.update(ctx) {
                    self.transition(ctx, AppState::Menu);
                }
            }
        }
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        match self.state {
            AppState::Loading => self.loading.draw(ctx),
            AppState::Menu => self.menu.draw(ctx),
            AppState::Playing => self.playing.draw(ctx),
        }
    }
}
