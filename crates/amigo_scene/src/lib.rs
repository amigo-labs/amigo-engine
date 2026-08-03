#![allow(missing_docs)]

#[cfg(feature = "hierarchical_scenes")]
pub mod hierarchical;

pub mod transition;

/// Action returned by a scene's update method to control scene transitions.
///
/// `S` is the trait object the pushed/replaced scene is constructed as. It
/// defaults to [`Scene`] for the context-free [`SceneManager`] in this crate;
/// `amigo_engine` uses `SceneAction<dyn Game>` so that a game's own `update`
/// can drive the stack while still receiving the engine contexts.
pub enum SceneAction<S: ?Sized = dyn Scene> {
    /// Continue running the current scene.
    Continue,
    /// Push a new scene on top (overlay).
    Push(Box<dyn SceneFactory<S>>),
    /// Pop the current scene (go back).
    Pop,
    /// Replace the current scene with a new one.
    Replace(Box<dyn SceneFactory<S>>),
    /// Quit the application.
    Quit,
}

/// Trait for creating scenes. Used with Push/Replace to defer construction.
///
/// Construction is deferred because the transition is decided mid-update, when
/// the outgoing scene still owns the stack.
pub trait SceneFactory<S: ?Sized = dyn Scene>: Send + 'static {
    fn create(&self) -> Box<S>;
}

/// Implement SceneFactory for closures.
impl<S: ?Sized, F> SceneFactory<S> for F
where
    F: Fn() -> Box<S> + Send + 'static,
{
    fn create(&self) -> Box<S> {
        (self)()
    }
}

/// Trait that all game scenes implement.
pub trait Scene: 'static {
    /// Called when this scene becomes the active scene.
    fn on_enter(&mut self) {}

    /// Called when this scene is no longer the active scene (but still on stack).
    fn on_pause(&mut self) {}

    /// Called when this scene becomes active again after being paused.
    fn on_resume(&mut self) {}

    /// Called when this scene is removed from the stack.
    fn on_exit(&mut self) {}

    /// Update game logic. Returns a SceneAction to control transitions.
    fn update(&mut self) -> SceneAction;

    /// Render the scene.
    fn draw(&self);
}

/// Scene stack manager.
pub struct SceneManager {
    stack: Vec<Box<dyn Scene>>,
}

impl SceneManager {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, mut scene: Box<dyn Scene>) {
        if let Some(current) = self.stack.last_mut() {
            current.on_pause();
        }
        scene.on_enter();
        self.stack.push(scene);
    }

    pub fn pop(&mut self) {
        if let Some(mut scene) = self.stack.pop() {
            scene.on_exit();
        }
        if let Some(current) = self.stack.last_mut() {
            current.on_resume();
        }
    }

    pub fn replace(&mut self, factory: Box<dyn SceneFactory>) {
        if let Some(mut scene) = self.stack.pop() {
            scene.on_exit();
        }
        let mut new_scene = factory.create();
        new_scene.on_enter();
        self.stack.push(new_scene);
    }

    pub fn update(&mut self) -> bool {
        if let Some(scene) = self.stack.last_mut() {
            match scene.update() {
                SceneAction::Continue => true,
                SceneAction::Push(factory) => {
                    let new_scene = factory.create();
                    self.push(new_scene);
                    true
                }
                SceneAction::Pop => {
                    self.pop();
                    !self.stack.is_empty()
                }
                SceneAction::Replace(factory) => {
                    self.replace(factory);
                    true
                }
                SceneAction::Quit => false,
            }
        } else {
            false
        }
    }

    pub fn draw(&self) {
        if let Some(scene) = self.stack.last() {
            scene.draw();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Default for SceneManager {
    fn default() -> Self {
        Self::new()
    }
}
