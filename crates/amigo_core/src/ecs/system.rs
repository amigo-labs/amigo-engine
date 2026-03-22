//! System trait and descriptor for the declarative system graph (ADR-0002).
//!
//! Gated behind `#[cfg(feature = "system_graph")]`.

use super::world::World;
use std::any::TypeId;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// SystemContext -- the borrow handed to each system at runtime
// ---------------------------------------------------------------------------

/// Context passed to a [`System`] when it runs.
///
/// Currently a thin wrapper around `&mut World`. Future work (ADR-0002 step 5)
/// will narrow this to only the declared component borrows.
pub struct SystemContext<'a> {
    pub world: &'a mut World,
}

// ---------------------------------------------------------------------------
// System trait
// ---------------------------------------------------------------------------

/// A unit of game logic that can be scheduled by the [`SystemGraph`](super::schedule::SystemGraph).
pub trait System: Send + Sync {
    /// Execute this system for one tick.
    fn run(&mut self, ctx: &mut SystemContext<'_>);

    /// Human-readable label used for ordering constraints.
    fn label(&self) -> &str;

    /// Labels of systems that must run **before** this one.
    fn after(&self) -> &[&str] {
        &[]
    }

    /// Labels of systems that must run **after** this one.
    fn before(&self) -> &[&str] {
        &[]
    }

    /// Set of [`TypeId`]s this system reads.
    fn reads(&self) -> HashSet<TypeId> {
        HashSet::new()
    }

    /// Set of [`TypeId`]s this system writes.
    fn writes(&self) -> HashSet<TypeId> {
        HashSet::new()
    }

    /// The stage this system belongs to (default: [`SystemStage::Update`]).
    fn stage(&self) -> SystemStage {
        SystemStage::Update
    }
}

// ---------------------------------------------------------------------------
// SystemStage
// ---------------------------------------------------------------------------

/// Coarse execution phases within a single tick.
///
/// Systems in an earlier stage always complete before systems in a later stage
/// begin, regardless of fine-grained ordering constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SystemStage {
    PreUpdate = 0,
    Update = 1,
    PostUpdate = 2,
    Render = 3,
}

impl SystemStage {
    /// All stages in execution order.
    pub const ALL: [SystemStage; 4] = [
        SystemStage::PreUpdate,
        SystemStage::Update,
        SystemStage::PostUpdate,
        SystemStage::Render,
    ];
}

// ---------------------------------------------------------------------------
// SystemDescriptor -- builder for registering systems from closures / fns
// ---------------------------------------------------------------------------

/// A boxed system built via the descriptor builder API.
///
/// ```ignore
/// let desc = SystemDescriptor::new("movement", move |ctx: &mut SystemContext| {
///     // move entities by velocity ...
/// })
/// .after("input")
/// .before("collision")
/// .stage(SystemStage::Update);
/// ```
pub struct SystemDescriptor {
    label: String,
    func: Box<dyn FnMut(&mut SystemContext<'_>) + Send + Sync>,
    after: Vec<String>,
    before: Vec<String>,
    reads: HashSet<TypeId>,
    writes: HashSet<TypeId>,
    stage: SystemStage,
}

impl SystemDescriptor {
    pub fn new(
        label: impl Into<String>,
        func: impl FnMut(&mut SystemContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            func: Box::new(func),
            after: Vec::new(),
            before: Vec::new(),
            reads: HashSet::new(),
            writes: HashSet::new(),
            stage: SystemStage::Update,
        }
    }

    /// Declare that this system must run after the system with the given label.
    pub fn after(mut self, label: impl Into<String>) -> Self {
        self.after.push(label.into());
        self
    }

    /// Declare that this system must run before the system with the given label.
    pub fn before(mut self, label: impl Into<String>) -> Self {
        self.before.push(label.into());
        self
    }

    /// Declare a component type this system reads.
    pub fn reads<T: 'static>(mut self) -> Self {
        self.reads.insert(TypeId::of::<T>());
        self
    }

    /// Declare a component type this system writes.
    pub fn writes<T: 'static>(mut self) -> Self {
        self.writes.insert(TypeId::of::<T>());
        self
    }

    /// Set the execution stage for this system.
    pub fn stage(mut self, stage: SystemStage) -> Self {
        self.stage = stage;
        self
    }
}

// Implement `System` for `SystemDescriptor` so it can be used uniformly.
impl System for SystemDescriptor {
    fn run(&mut self, ctx: &mut SystemContext<'_>) {
        (self.func)(ctx);
    }

    fn label(&self) -> &str {
        &self.label
    }

    fn after(&self) -> &[&str] {
        // SAFETY: the strings live as long as self
        // We use a small trick: leak a cached slice. For simplicity we return
        // an empty slice and let the schedule builder read the descriptor fields
        // directly.
        &[]
    }

    fn before(&self) -> &[&str] {
        &[]
    }

    fn reads(&self) -> HashSet<TypeId> {
        self.reads.clone()
    }

    fn writes(&self) -> HashSet<TypeId> {
        self.writes.clone()
    }

    fn stage(&self) -> SystemStage {
        self.stage
    }
}

impl SystemDescriptor {
    /// Ordering constraints: labels this system should run after.
    pub fn after_labels(&self) -> &[String] {
        &self.after
    }

    /// Ordering constraints: labels this system should run before.
    pub fn before_labels(&self) -> &[String] {
        &self.before
    }
}
