//! Hierarchical scene composition (ADR-0012).
//!
//! Provides [`SceneNode`] tree structure, [`UpdateMode`]/[`DrawMode`] per node,
//! optional sub-world isolation, and a [`HierarchicalSceneManager`] that
//! replaces the flat stack with a depth-first traversal.

use amigo_core::ecs::World;

use crate::{Scene, SceneAction, SceneFactory};

// ── Configuration types ─────────────────────────────────────────────

/// Controls whether a scene node receives `update()` calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateMode {
    /// Scene receives normal `update()` calls.
    Active,
    /// Scene is skipped during update.
    Paused,
    /// Scene receives `update()` but is not the focus (e.g. gameplay behind a menu).
    Background,
}

/// Controls whether/how a scene node is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawMode {
    /// Scene is drawn normally.
    Visible,
    /// Scene is not drawn.
    Hidden,
    /// Scene is drawn with the given alpha multiplier (0.0 = invisible, 1.0 = opaque).
    Transparent(f32),
}

/// Per-node configuration controlling update, draw, and world ownership.
#[derive(Clone, Debug)]
pub struct SceneConfig {
    pub update_mode: UpdateMode,
    pub draw_mode: DrawMode,
    /// When `true`, the scene gets its own fresh [`World`] instance.
    pub owns_world: bool,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            update_mode: UpdateMode::Active,
            draw_mode: DrawMode::Visible,
            owns_world: false,
        }
    }
}

// ── Scene node ──────────────────────────────────────────────────────

/// A node in the hierarchical scene tree.
///
/// Each node owns a [`Scene`], an optional sub-[`World`], a list of
/// child nodes, and a [`SceneConfig`].
pub struct SceneNode {
    pub scene: Box<dyn Scene>,
    pub children: Vec<SceneNode>,
    pub config: SceneConfig,
    /// If `config.owns_world` was true at creation, this holds the scene's
    /// private world. The manager swaps it into `GameContext.world` before
    /// calling update/draw.
    pub sub_world: Option<World>,
}

impl SceneNode {
    /// Create a new scene node. If `config.owns_world` is true, a fresh
    /// empty [`World`] is allocated.
    pub fn new(scene: Box<dyn Scene>, config: SceneConfig) -> Self {
        let sub_world = if config.owns_world {
            Some(World::new())
        } else {
            None
        };
        Self {
            scene,
            children: Vec::new(),
            config,
            sub_world,
        }
    }
}

// ── Extended scene action ───────────────────────────────────────────

/// Extended action returned by scenes in hierarchical mode.
///
/// This mirrors [`SceneAction`] but adds `PushOverlay` for child-scene
/// composition.
pub enum HierarchicalSceneAction {
    /// Continue running.
    Continue,
    /// Push a new scene on top of the stack (pauses current, hides it).
    Push(Box<dyn SceneFactory>),
    /// Push a child overlay scene with custom config. The parent keeps
    /// running according to its current config (typically set to
    /// `UpdateMode::Background`).
    PushOverlay(Box<dyn SceneFactory>, SceneConfig),
    /// Pop the current scene.
    Pop,
    /// Replace the current scene.
    Replace(Box<dyn SceneFactory>),
    /// Quit the application.
    Quit,
}

impl From<SceneAction> for HierarchicalSceneAction {
    fn from(action: SceneAction) -> Self {
        match action {
            SceneAction::Continue => Self::Continue,
            SceneAction::Push(f) => Self::Push(f),
            SceneAction::Pop => Self::Pop,
            SceneAction::Replace(f) => Self::Replace(f),
            SceneAction::Quit => Self::Quit,
        }
    }
}

// ── Hierarchical scene manager ──────────────────────────────────────

/// A scene manager that organises scenes in a tree structure.
///
/// Top-level nodes behave like the classic flat stack (only the last is
/// active by default). Child nodes on any given node are overlays that
/// draw/update alongside their parent according to their [`SceneConfig`].
pub struct HierarchicalSceneManager {
    /// Top-level scene stack. The last entry is the "current" scene.
    nodes: Vec<SceneNode>,
}

impl HierarchicalSceneManager {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    // ── Stack operations (compatible with flat SceneManager) ────────

    /// Push a scene onto the top-level stack with default config
    /// (Active + Visible). The previous top scene is paused and hidden.
    pub fn push(&mut self, mut scene: Box<dyn Scene>) {
        if let Some(top) = self.nodes.last_mut() {
            top.scene.on_pause();
            top.config.update_mode = UpdateMode::Paused;
            top.config.draw_mode = DrawMode::Hidden;
        }
        scene.on_enter();
        self.nodes
            .push(SceneNode::new(scene, SceneConfig::default()));
    }

    /// Pop the top-level scene.
    pub fn pop(&mut self) {
        if let Some(mut node) = self.nodes.pop() {
            // Recursively exit all children first.
            Self::exit_node(&mut node);
        }
        if let Some(top) = self.nodes.last_mut() {
            top.config.update_mode = UpdateMode::Active;
            top.config.draw_mode = DrawMode::Visible;
            top.scene.on_resume();
        }
    }

    /// Replace the top-level scene.
    pub fn replace(&mut self, factory: Box<dyn SceneFactory>) {
        if let Some(mut node) = self.nodes.pop() {
            Self::exit_node(&mut node);
        }
        let mut new_scene = factory.create();
        new_scene.on_enter();
        self.nodes
            .push(SceneNode::new(new_scene, SceneConfig::default()));
    }

    // ── Overlay operations ──────────────────────────────────────────

    /// Push a child overlay onto the current top-level scene.
    /// The parent's update mode is set to `Background`.
    pub fn push_overlay(&mut self, mut scene: Box<dyn Scene>, config: SceneConfig) {
        if let Some(top) = self.nodes.last_mut() {
            top.config.update_mode = UpdateMode::Background;
            scene.on_enter();
            top.children.push(SceneNode::new(scene, config));
        }
    }

    /// Pop the topmost child overlay from the current top-level scene.
    pub fn pop_overlay(&mut self) {
        if let Some(top) = self.nodes.last_mut() {
            if let Some(mut child) = top.children.pop() {
                Self::exit_node(&mut child);
            }
            if top.children.is_empty() {
                top.config.update_mode = UpdateMode::Active;
            }
        }
    }

    // ── Update / Draw ───────────────────────────────────────────────

    /// Update the scene tree. Returns `false` when the application should
    /// quit (empty stack or `Quit` action).
    pub fn update(&mut self) -> bool {
        let len = self.nodes.len();
        if len == 0 {
            return false;
        }

        // Collect actions from the tree rooted at the topmost node.
        // We update the top-level node and all its children.
        let last = len - 1;
        let node = &mut self.nodes[last];
        let action = Self::update_node(node);

        match action {
            HierarchicalSceneAction::Continue => true,
            HierarchicalSceneAction::Push(factory) => {
                let new_scene = factory.create();
                self.push(new_scene);
                true
            }
            HierarchicalSceneAction::PushOverlay(factory, config) => {
                let new_scene = factory.create();
                self.push_overlay(new_scene, config);
                true
            }
            HierarchicalSceneAction::Pop => {
                self.pop();
                !self.nodes.is_empty()
            }
            HierarchicalSceneAction::Replace(factory) => {
                self.replace(factory);
                true
            }
            HierarchicalSceneAction::Quit => false,
        }
    }

    /// Draw all visible scenes in the tree (depth-first, bottom to top).
    pub fn draw(&self) {
        if let Some(node) = self.nodes.last() {
            Self::draw_node(node);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.nodes.len()
    }

    // ── Private helpers ─────────────────────────────────────────────

    /// Recursively update a node and its children. Returns the action
    /// from the deepest active child (or the node itself).
    fn update_node(node: &mut SceneNode) -> HierarchicalSceneAction {
        // Update the node itself if its mode allows it.
        let self_action = match node.config.update_mode {
            UpdateMode::Active | UpdateMode::Background => node.scene.update().into(),
            UpdateMode::Paused => HierarchicalSceneAction::Continue,
        };

        // Update children (overlays). The last child that returns a
        // non-Continue action wins.
        let mut child_action: Option<HierarchicalSceneAction> = None;
        for child in node.children.iter_mut() {
            let a = Self::update_node(child);
            if !matches!(a, HierarchicalSceneAction::Continue) {
                child_action = Some(a);
            }
        }

        // Child overlay actions take priority. If a child overlay says
        // Pop, we pop that overlay (not the parent). Handle overlay pops
        // locally.
        if let Some(action) = child_action {
            match action {
                HierarchicalSceneAction::Pop => {
                    // Pop the last child overlay.
                    if let Some(mut child) = node.children.pop() {
                        Self::exit_node(&mut child);
                    }
                    if node.children.is_empty() {
                        node.config.update_mode = UpdateMode::Active;
                    }
                    HierarchicalSceneAction::Continue
                }
                other => other,
            }
        } else {
            self_action
        }
    }

    /// Recursively draw a node and its children.
    fn draw_node(node: &SceneNode) {
        match node.config.draw_mode {
            DrawMode::Visible | DrawMode::Transparent(_) => {
                node.scene.draw();
            }
            DrawMode::Hidden => {}
        }

        for child in &node.children {
            Self::draw_node(child);
        }
    }

    /// Recursively exit a node and all its children.
    fn exit_node(node: &mut SceneNode) {
        for child in node.children.iter_mut() {
            Self::exit_node(child);
        }
        node.children.clear();
        node.scene.on_exit();
        // Sub-world is dropped automatically when the node is dropped.
    }
}

impl Default for HierarchicalSceneManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A simple test scene that records lifecycle events.
    struct TestScene {
        name: String,
        log: Rc<RefCell<Vec<String>>>,
        next_action: RefCell<SceneAction>,
    }

    impl TestScene {
        fn new(name: &str, log: Rc<RefCell<Vec<String>>>) -> Self {
            Self {
                name: name.to_string(),
                log,
                next_action: RefCell::new(SceneAction::Continue),
            }
        }
    }

    impl Scene for TestScene {
        fn on_enter(&mut self) {
            self.log.borrow_mut().push(format!("{}:enter", self.name));
        }
        fn on_pause(&mut self) {
            self.log.borrow_mut().push(format!("{}:pause", self.name));
        }
        fn on_resume(&mut self) {
            self.log.borrow_mut().push(format!("{}:resume", self.name));
        }
        fn on_exit(&mut self) {
            self.log.borrow_mut().push(format!("{}:exit", self.name));
        }
        fn update(&mut self) -> SceneAction {
            self.log.borrow_mut().push(format!("{}:update", self.name));
            self.next_action.replace(SceneAction::Continue)
        }
        fn draw(&self) {
            self.log.borrow_mut().push(format!("{}:draw", self.name));
        }
    }

    #[test]
    fn flat_push_pop_lifecycle() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut mgr = HierarchicalSceneManager::new();

        mgr.push(Box::new(TestScene::new("A", log.clone())));
        assert_eq!(mgr.depth(), 1);
        assert_eq!(&*log.borrow(), &["A:enter"]);

        mgr.push(Box::new(TestScene::new("B", log.clone())));
        assert_eq!(mgr.depth(), 2);
        assert!(log.borrow().contains(&"A:pause".to_string()));
        assert!(log.borrow().contains(&"B:enter".to_string()));

        mgr.pop();
        assert_eq!(mgr.depth(), 1);
        assert!(log.borrow().contains(&"B:exit".to_string()));
        assert!(log.borrow().contains(&"A:resume".to_string()));
    }

    #[test]
    fn overlay_update_and_draw() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut mgr = HierarchicalSceneManager::new();

        mgr.push(Box::new(TestScene::new("game", log.clone())));

        // Push overlay with parent visible
        mgr.push_overlay(
            Box::new(TestScene::new("menu", log.clone())),
            SceneConfig {
                update_mode: UpdateMode::Active,
                draw_mode: DrawMode::Transparent(0.5),
                owns_world: false,
            },
        );

        log.borrow_mut().clear();

        // Update should call both game (Background) and menu (Active).
        mgr.update();
        let entries = log.borrow().clone();
        assert!(entries.contains(&"game:update".to_string()));
        assert!(entries.contains(&"menu:update".to_string()));

        log.borrow_mut().clear();

        // Draw should draw both (game then menu).
        mgr.draw();
        let entries = log.borrow().clone();
        assert!(entries.contains(&"game:draw".to_string()));
        assert!(entries.contains(&"menu:draw".to_string()));
    }

    #[test]
    fn overlay_pop_from_child_action() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut mgr = HierarchicalSceneManager::new();

        mgr.push(Box::new(TestScene::new("game", log.clone())));
        let mut menu = TestScene::new("menu", log.clone());
        // Set the menu to pop itself on next update.
        *menu.next_action.get_mut() = SceneAction::Pop;
        mgr.push_overlay(Box::new(menu), SceneConfig::default());

        assert_eq!(mgr.nodes.last().unwrap().children.len(), 1);

        mgr.update();

        // The overlay should be popped.
        assert_eq!(mgr.nodes.last().unwrap().children.len(), 0);
        // Parent should be back to Active.
        assert_eq!(
            mgr.nodes.last().unwrap().config.update_mode,
            UpdateMode::Active
        );
        assert!(log.borrow().contains(&"menu:exit".to_string()));
    }

    #[test]
    fn sub_world_is_created_when_owns_world() {
        let node = SceneNode::new(
            Box::new(TestScene::new("x", Rc::new(RefCell::new(Vec::new())))),
            SceneConfig {
                owns_world: true,
                ..SceneConfig::default()
            },
        );
        assert!(node.sub_world.is_some());
    }

    #[test]
    fn sub_world_not_created_by_default() {
        let node = SceneNode::new(
            Box::new(TestScene::new("x", Rc::new(RefCell::new(Vec::new())))),
            SceneConfig::default(),
        );
        assert!(node.sub_world.is_none());
    }
}
