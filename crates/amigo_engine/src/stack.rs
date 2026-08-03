//! Scene stack for [`Game`] implementations.
//!
//! [`amigo_scene::SceneManager`] models the same stack, but its `Scene` trait
//! receives no contexts, so a scene there cannot touch the world, input, or the
//! renderer. `Game` already is the context-aware equivalent, so the stack is
//! built over `Box<dyn Game>` and reuses `SceneManager`'s semantics: pushing
//! pauses the game below, popping resumes it, replacing exits the current one.
//!
//! Lifecycle contract:
//!
//! * [`Game::init`] runs once per instance, the first (and only) time that
//!   instance becomes active. A popped game is dropped, so a factory that is
//!   used twice produces two instances and `init` runs for each.
//! * [`Game::on_enter`] runs immediately after `init`, when the game becomes
//!   the active top.
//! * [`Game::on_pause`] runs when another game is pushed above.
//! * [`Game::on_resume`] runs when the game above is popped.
//! * [`Game::on_exit`] runs when the game itself is popped or replaced.

use crate::{Game, GameContext};
use amigo_scene::SceneAction;

/// A stack of [`Game`] states, topmost last.
pub struct GameStack {
    stack: Vec<Box<dyn Game>>,
}

impl GameStack {
    /// Create a stack holding `root` without running any lifecycle hooks.
    ///
    /// The engine defers root initialization until after the splash screen, so
    /// call [`GameStack::enter_root`] once the contexts are ready.
    pub fn new(root: Box<dyn Game>) -> Self {
        Self { stack: vec![root] }
    }

    /// Run `init` + `on_enter` for the root game.
    ///
    /// Separate from [`GameStack::new`] because the engine builds the stack
    /// before the renderer and asset manager exist.
    pub fn enter_root(&mut self, ctx: &mut GameContext) {
        if let Some(root) = self.stack.last_mut() {
            root.init(ctx);
            root.on_enter(ctx);
        }
    }

    /// The active game, or `None` once the stack has run empty.
    pub fn top(&self) -> Option<&dyn Game> {
        self.stack.last().map(|g| g.as_ref())
    }

    /// The active game, mutably.
    pub fn top_mut(&mut self) -> Option<&mut (dyn Game + 'static)> {
        self.stack.last_mut().map(|g| g.as_mut())
    }

    /// Number of games on the stack.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Whether the stack has run empty (the engine then shuts down).
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Push `game` on top, pausing the one below.
    pub fn push(&mut self, mut game: Box<dyn Game>, ctx: &mut GameContext) {
        if let Some(current) = self.stack.last_mut() {
            current.on_pause(ctx);
        }
        game.init(ctx);
        game.on_enter(ctx);
        self.stack.push(game);
    }

    /// Pop the active game, resuming the one below.
    pub fn pop(&mut self, ctx: &mut GameContext) {
        if let Some(mut game) = self.stack.pop() {
            game.on_exit(ctx);
        }
        if let Some(current) = self.stack.last_mut() {
            current.on_resume(ctx);
        }
    }

    /// Replace the active game, without resuming anything below it.
    pub fn replace(&mut self, mut game: Box<dyn Game>, ctx: &mut GameContext) {
        if let Some(mut old) = self.stack.pop() {
            old.on_exit(ctx);
        }
        game.init(ctx);
        game.on_enter(ctx);
        self.stack.push(game);
    }

    /// Apply the action the active game returned from `update`.
    ///
    /// Returns `false` when the engine should shut down: either `Quit` was
    /// returned, or the last game popped itself off the stack.
    pub fn apply(&mut self, action: SceneAction<dyn Game>, ctx: &mut GameContext) -> bool {
        match action {
            SceneAction::Continue => true,
            SceneAction::Push(factory) => {
                self.push(factory.create(), ctx);
                true
            }
            SceneAction::Pop => {
                self.pop(ctx);
                !self.stack.is_empty()
            }
            SceneAction::Replace(factory) => {
                self.replace(factory.create(), ctx);
                true
            }
            SceneAction::Quit => false,
        }
    }
}
