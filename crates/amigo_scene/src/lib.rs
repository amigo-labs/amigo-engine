#![allow(missing_docs)]

pub mod transition;

/// Action returned by a scene's update method to control scene transitions.
pub enum SceneAction {
    /// Continue running the current scene.
    Continue,
    /// Push a new scene on top (overlay).
    Push(Box<dyn SceneFactory>),
    /// Pop the current scene (go back).
    Pop,
    /// Replace the current scene with a new one.
    Replace(Box<dyn SceneFactory>),
    /// Quit the application.
    Quit,
}

/// Trait for creating scenes. Used with Push/Replace to defer construction.
pub trait SceneFactory: Send + 'static {
    fn create(&self) -> Box<dyn Scene>;
}

/// Implement SceneFactory for closures.
impl<F> SceneFactory for F
where
    F: Fn() -> Box<dyn Scene> + Send + 'static,
{
    fn create(&self) -> Box<dyn Scene> {
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
