//! System graph: topological sorting, parallel dispatch, and stage grouping (ADR-0002).
//!
//! Gated behind `#[cfg(feature = "system_graph")]`.

use super::system::{System, SystemContext, SystemDescriptor, SystemStage};
use std::collections::{HashMap, VecDeque};

/// A raw pointer wrapper that is `Send`/`Sync` when the pointee is.
///
/// The bounds matter: a blanket `unsafe impl<T> Send` would let the scheduler
/// move thread-affine data (an `Rc` inside a component, say) onto a rayon
/// worker. Requiring `T: Send` pushes that check back onto the pointee, so
/// `World` has to actually be `Send` for parallel dispatch to compile — which
/// is why every component storage carries a `Send + Sync` bound (see
/// `AnyStorage` in [`world`](super::world)).
///
/// # Safety
/// The caller must still guarantee that access through this pointer is free of
/// data races; the bounds only rule out thread-affine pointees, not aliasing.
/// The system schedule ensures disjoint access.
#[cfg(feature = "system_graph")]
struct SendPtr<T>(*mut T);

#[cfg(feature = "system_graph")]
impl<T> Clone for SendPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[cfg(feature = "system_graph")]
impl<T> Copy for SendPtr<T> {}

#[cfg(feature = "system_graph")]
impl<T> SendPtr<T> {
    fn ptr(self) -> *mut T {
        self.0
    }
}

// SAFETY: sending the pointer is sound whenever sending the pointee would be;
// the aliasing discipline is enforced by the schedule, not by these impls.
#[cfg(feature = "system_graph")]
unsafe impl<T: Send> Send for SendPtr<T> {}
// SAFETY: as for Send: sharing the pointer is sound whenever sharing the
// pointee would be.
#[cfg(feature = "system_graph")]
unsafe impl<T: Sync> Sync for SendPtr<T> {}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur when building or running a [`SystemGraph`].
#[derive(Debug, Clone)]
pub enum ScheduleError {
    /// A dependency cycle was detected. The `Vec<String>` contains the labels
    /// forming the cycle.
    CycleDetected(Vec<String>),
    /// A system references a label that does not exist.
    UnknownLabel(String),
    /// Duplicate system label.
    DuplicateLabel(String),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CycleDetected(labels) => write!(f, "dependency cycle: {}", labels.join(" -> ")),
            Self::UnknownLabel(l) => write!(f, "unknown system label: {l}"),
            Self::DuplicateLabel(l) => write!(f, "duplicate system label: {l}"),
        }
    }
}

impl std::error::Error for ScheduleError {}

// ---------------------------------------------------------------------------
// SystemGraph
// ---------------------------------------------------------------------------

/// A resolved execution schedule for systems within a single [`SystemStage`].
///
/// Each *step* is a vec of system indices that may execute in parallel (they
/// have no ordering constraints and no data conflicts).
struct StageSchedule {
    /// Ordered list of parallel batches. Each batch is a vec of indices into
    /// `SystemGraph::systems`.
    steps: Vec<Vec<usize>>,
}

/// Collects [`SystemDescriptor`]s, resolves ordering, detects cycles, and
/// dispatches systems each tick.
pub struct SystemGraph {
    systems: Vec<SystemDescriptor>,
    /// Built schedule per stage, populated by [`build`].
    schedules: HashMap<SystemStage, StageSchedule>,
    built: bool,
}

impl SystemGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            schedules: HashMap::new(),
            built: false,
        }
    }

    /// Add a system to the graph. Must call [`build`](Self::build) before
    /// [`run`](Self::run).
    pub fn add_system(&mut self, desc: SystemDescriptor) {
        self.built = false;
        self.systems.push(desc);
    }

    /// Resolve the execution schedule. Returns an error if cycles or unknown
    /// labels are detected.
    pub fn build(&mut self) -> Result<(), ScheduleError> {
        // -- label -> index map -------------------------------------------
        let mut label_to_idx: HashMap<&str, usize> = HashMap::new();
        for (i, sys) in self.systems.iter().enumerate() {
            let label = sys.label();
            if label_to_idx.contains_key(label) {
                return Err(ScheduleError::DuplicateLabel(label.to_string()));
            }
            label_to_idx.insert(label, i);
        }

        // -- group by stage -----------------------------------------------
        let mut stage_indices: HashMap<SystemStage, Vec<usize>> = HashMap::new();
        for (i, sys) in self.systems.iter().enumerate() {
            stage_indices.entry(sys.stage()).or_default().push(i);
        }

        // -- build adjacency + in-degree per stage, then toposort ----------
        self.schedules.clear();

        for stage in SystemStage::ALL {
            let indices = match stage_indices.get(&stage) {
                Some(v) => v,
                None => continue,
            };

            // Local index within this stage -> global index in self.systems
            let local_to_global: Vec<usize> = indices.clone();
            let global_to_local: HashMap<usize, usize> = local_to_global
                .iter()
                .enumerate()
                .map(|(l, &g)| (g, l))
                .collect();
            let n = local_to_global.len();

            // adjacency[a] contains b means a must run before b (a -> b)
            let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut in_degree: Vec<usize> = vec![0; n];

            for (local_i, &global_i) in local_to_global.iter().enumerate() {
                let sys = &self.systems[global_i];

                // "after" constraints: for each label L in sys.after_labels(),
                // L -> this system (L must run before this one).
                for after_label in sys.after_labels() {
                    if let Some(&dep_global) = label_to_idx.get(after_label.as_str()) {
                        if let Some(&dep_local) = global_to_local.get(&dep_global) {
                            adj[dep_local].push(local_i);
                            in_degree[local_i] += 1;
                        }
                        // If the dependency is in a different stage, the stage
                        // ordering already guarantees it runs first -- skip.
                    } else {
                        return Err(ScheduleError::UnknownLabel(after_label.clone()));
                    }
                }

                // "before" constraints: for each label L in sys.before_labels(),
                // this system -> L (this one must run before L).
                for before_label in sys.before_labels() {
                    if let Some(&dep_global) = label_to_idx.get(before_label.as_str()) {
                        if let Some(&dep_local) = global_to_local.get(&dep_global) {
                            adj[local_i].push(dep_local);
                            in_degree[dep_local] += 1;
                        }
                    } else {
                        return Err(ScheduleError::UnknownLabel(before_label.clone()));
                    }
                }
            }

            // Kahn's algorithm with batched layers for parallelism
            let mut queue: VecDeque<usize> = VecDeque::new();
            for (i, &deg) in in_degree.iter().enumerate() {
                if deg == 0 {
                    queue.push_back(i);
                }
            }

            let mut steps: Vec<Vec<usize>> = Vec::new();
            let mut visited = 0usize;

            while !queue.is_empty() {
                // All nodes currently in the queue have no remaining
                // dependencies -- they form one parallelisable batch.
                let batch_size = queue.len();
                let batch_locals: Vec<usize> = queue.drain(..batch_size).collect();
                visited += batch_locals.len();

                // Within a batch, further split by data conflicts.
                let batch_globals: Vec<usize> =
                    batch_locals.iter().map(|&l| local_to_global[l]).collect();
                let parallel_groups = split_by_conflicts(&self.systems, &batch_globals);

                for group in parallel_groups {
                    steps.push(group);
                }

                // Decrease in-degree for successors
                for &local_i in &batch_locals {
                    for &succ in &adj[local_i] {
                        in_degree[succ] -= 1;
                        if in_degree[succ] == 0 {
                            queue.push_back(succ);
                        }
                    }
                }
            }

            if visited != n {
                // Cycle -- collect labels of unvisited nodes for diagnostics
                let cycle_labels: Vec<String> = (0..n)
                    .filter(|i| in_degree[*i] > 0)
                    .map(|i| self.systems[local_to_global[i]].label().to_string())
                    .collect();
                return Err(ScheduleError::CycleDetected(cycle_labels));
            }

            self.schedules.insert(stage, StageSchedule { steps });
        }

        self.built = true;
        Ok(())
    }

    /// Execute all systems for one tick in schedule order, one at a time.
    ///
    /// Systems the schedule groups into a batch still run sequentially here,
    /// in batch order, so this is sound whatever the systems touch — including
    /// components they did not declare and structural changes. Use
    /// [`run_parallel`](Self::run_parallel) to spread batches across threads.
    ///
    /// # Panics
    /// Panics if [`build`](Self::build) has not been called (or was invalidated
    /// by adding new systems).
    pub fn run(&mut self, world: &mut super::world::World) {
        assert!(
            self.built,
            "SystemGraph::build() must be called before run()"
        );

        let Self {
            systems, schedules, ..
        } = self;
        for stage in SystemStage::ALL {
            let Some(schedule) = schedules.get(&stage) else {
                continue;
            };
            for &idx in schedule.steps.iter().flatten() {
                let mut ctx = SystemContext { world };
                systems[idx].run(&mut ctx);
            }
        }
    }

    /// Execute all systems for one tick, dispatching each batch of systems
    /// with pairwise-disjoint declared access in parallel via rayon.
    ///
    /// Every batch member receives its own `&mut World`, so the borrow checker
    /// cannot see what each one touches; the declarations stand in for it.
    ///
    /// # Safety
    /// For every system that declares access with `.reads::<T>()` /
    /// `.writes::<T>()` (systems declaring nothing always run alone and are
    /// exempt), the caller guarantees that the system:
    ///
    /// - reads and writes only the component storages it declared, and
    /// - makes no structural change to the world: no `spawn`, `despawn` or
    ///   `flush`, and no `insert_dynamic` of a type that is not registered yet
    ///   (registration can rehash the dynamic-storage map under a sibling
    ///   system; call [`World::register_dynamic`](super::world::World::register_dynamic)
    ///   up front).
    ///
    /// Breaking either is a data race. Debug builds compare a structural
    /// fingerprint around each batch and panic on a detected structural
    /// change, but by then the race has already happened.
    ///
    /// # Panics
    /// Panics if [`build`](Self::build) has not been called.
    pub unsafe fn run_parallel(&mut self, world: &mut super::world::World) {
        assert!(
            self.built,
            "SystemGraph::build() must be called before run_parallel()"
        );

        let Self {
            systems, schedules, ..
        } = self;
        for stage in SystemStage::ALL {
            let Some(schedule) = schedules.get(&stage) else {
                continue;
            };
            for step in &schedule.steps {
                if let [idx] = step[..] {
                    let mut ctx = SystemContext { world };
                    systems[idx].run(&mut ctx);
                } else {
                    // SAFETY: forwarded from this function's contract.
                    unsafe { Self::dispatch_parallel(systems, step, world) };
                }
            }
        }
    }

    /// Dispatch a batch of non-conflicting systems in parallel using rayon.
    ///
    /// # Safety
    /// `indices` must be distinct and in bounds, and every system they name
    /// must satisfy the contract of [`run_parallel`](Self::run_parallel).
    unsafe fn dispatch_parallel(
        systems: &mut [SystemDescriptor],
        indices: &[usize],
        world: &mut super::world::World,
    ) {
        let fingerprint_before = world.structural_fingerprint();

        let world_ptr = SendPtr(world as *mut super::world::World);
        let systems_ptr = SendPtr(systems.as_mut_ptr());

        rayon::scope(move |s| {
            for &idx in indices {
                let wp = world_ptr;
                let sp = systems_ptr;
                s.spawn(move |_| {
                    // SAFETY: `idx` is in bounds and distinct from every other
                    // index in this batch (`build` puts each system in exactly
                    // one step), so each task gets the only `&mut` to its
                    // descriptor.
                    let sys = unsafe { &mut *sp.ptr().add(idx) };
                    // SAFETY: the tasks alias `*world`; the caller guarantees
                    // their accesses are disjoint and non-structural, so no
                    // two of them touch the same memory with a write.
                    let w = unsafe { &mut *wp.ptr() };
                    let mut ctx = SystemContext { world: w };
                    sys.run(&mut ctx);
                });
            }
        });

        debug_assert_eq!(
            fingerprint_before,
            world.structural_fingerprint(),
            "a system in a parallel batch performed a structural change \
             (spawn, despawn, or dynamic component registration), which \
             SystemGraph::run_parallel's safety contract forbids. Move the \
             structural work into a system that declares no access (those \
             always run alone), register dynamic types before the run, or use \
             SystemGraph::run."
        );
    }

    /// Returns the number of registered systems.
    pub fn system_count(&self) -> usize {
        self.systems.len()
    }

    /// Returns true if the schedule has been built.
    pub fn is_built(&self) -> bool {
        self.built
    }
}

impl Default for SystemGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Conflict detection helpers
// ---------------------------------------------------------------------------

/// Returns true if two systems have conflicting component access (i.e. at
/// least one writes to a component the other reads or writes).
///
/// A system that declares no access at all (empty `reads` AND empty
/// `writes`) gets a full `&mut World` without any statement about what it
/// touches, so it must be treated as conflicting with everything. Otherwise
/// two undeclared systems would be scheduled into the same parallel batch
/// and race on whatever they actually access. Parallel execution is
/// strictly opt-in via `.reads::<T>()` / `.writes::<T>()` declarations.
fn has_conflict(a: &SystemDescriptor, b: &SystemDescriptor) -> bool {
    let a_writes = a.writes();
    let b_writes = b.writes();
    let a_reads = a.reads();
    let b_reads = b.reads();

    let a_undeclared = a_reads.is_empty() && a_writes.is_empty();
    let b_undeclared = b_reads.is_empty() && b_writes.is_empty();
    if a_undeclared || b_undeclared {
        return true;
    }

    // Conflict if a writes something b reads or writes, or vice-versa.
    !a_writes.is_disjoint(&b_reads)
        || !a_writes.is_disjoint(&b_writes)
        || !b_writes.is_disjoint(&a_reads)
}

/// Given a list of global system indices that have no ordering constraints
/// between them, split them into groups where systems within a group have
/// disjoint data access and can safely run in parallel.
fn split_by_conflicts(systems: &[SystemDescriptor], indices: &[usize]) -> Vec<Vec<usize>> {
    if indices.is_empty() {
        return Vec::new();
    }

    // Greedy graph colouring -- assign each system to the first group it
    // doesn't conflict with.
    let mut groups: Vec<Vec<usize>> = Vec::new();

    for &idx in indices {
        let mut placed = false;
        for group in groups.iter_mut() {
            let conflicts = group
                .iter()
                .any(|&existing| has_conflict(&systems[idx], &systems[existing]));
            if !conflicts {
                group.push(idx);
                placed = true;
                break;
            }
        }
        if !placed {
            groups.push(vec![idx]);
        }
    }

    groups
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_system(label: &str) -> SystemDescriptor {
        let l = label.to_string();
        SystemDescriptor::new(l, |_ctx: &mut SystemContext<'_>| {})
    }

    #[test]
    fn basic_topological_order() {
        let mut graph = SystemGraph::new();
        graph.add_system(noop_system("c").after("b"));
        graph.add_system(noop_system("a"));
        graph.add_system(noop_system("b").after("a"));

        graph.build().expect("should build without errors");
        assert!(graph.is_built());
    }

    #[test]
    fn cycle_detection() {
        let mut graph = SystemGraph::new();
        graph.add_system(noop_system("a").after("b"));
        graph.add_system(noop_system("b").after("a"));

        match graph.build() {
            Err(ScheduleError::CycleDetected(labels)) => {
                assert!(labels.contains(&"a".to_string()));
                assert!(labels.contains(&"b".to_string()));
            }
            other => panic!("expected CycleDetected, got {:?}", other),
        }
    }

    #[test]
    fn duplicate_label_detection() {
        let mut graph = SystemGraph::new();
        graph.add_system(noop_system("a"));
        graph.add_system(noop_system("a"));

        match graph.build() {
            Err(ScheduleError::DuplicateLabel(l)) => assert_eq!(l, "a"),
            other => panic!("expected DuplicateLabel, got {:?}", other),
        }
    }

    #[test]
    fn unknown_label_detection() {
        let mut graph = SystemGraph::new();
        graph.add_system(noop_system("a").after("nonexistent"));

        match graph.build() {
            Err(ScheduleError::UnknownLabel(l)) => assert_eq!(l, "nonexistent"),
            other => panic!("expected UnknownLabel, got {:?}", other),
        }
    }

    #[test]
    fn undeclared_systems_never_share_a_parallel_batch() {
        // Systems without declared reads/writes get a full &mut World, so
        // they must be serialized even without ordering constraints.
        let systems = vec![noop_system("a"), noop_system("b"), noop_system("c")];
        let groups = split_by_conflicts(&systems, &[0, 1, 2]);
        assert_eq!(groups.len(), 3);
        for group in &groups {
            assert_eq!(group.len(), 1);
        }
    }

    #[test]
    fn declared_disjoint_systems_share_a_batch() {
        struct CompA;
        struct CompB;
        let systems = vec![
            noop_system("a").writes::<CompA>(),
            noop_system("b").writes::<CompB>(),
            noop_system("c").reads::<CompA>(),
        ];
        let groups = split_by_conflicts(&systems, &[0, 1, 2]);
        // a+b are disjoint (parallel); c reads what a writes (own batch).
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![0, 1]);
        assert_eq!(groups[1], vec![2]);
    }

    #[test]
    fn stage_ordering() {
        let mut graph = SystemGraph::new();
        graph.add_system(noop_system("render_sys").stage(SystemStage::Render));
        graph.add_system(noop_system("pre_sys").stage(SystemStage::PreUpdate));
        graph.add_system(noop_system("update_sys").stage(SystemStage::Update));
        graph.add_system(noop_system("post_sys").stage(SystemStage::PostUpdate));

        graph.build().expect("should build");
        assert_eq!(graph.system_count(), 4);
    }

    #[test]
    fn before_constraint() {
        let mut graph = SystemGraph::new();
        graph.add_system(noop_system("a").before("b"));
        graph.add_system(noop_system("b"));

        graph.build().expect("should build with before constraint");
    }

    /// Guards against scheduling becoming super-linear. The ADR's target is
    /// < 0.5 ms to build 30 systems; the assertion allows 50 ms so a loaded CI
    /// runner does not fail the suite over scheduler-unrelated noise. The
    /// comment used to claim 0.5 ms while the assertion allowed 500 ms, which
    /// would not have caught a thousandfold regression.
    ///
    /// Ignored under Miri, where everything runs orders of magnitude slower and
    /// a wall-clock bound measures the interpreter rather than the code.
    #[test]
    #[cfg_attr(miri, ignore)]
    fn many_systems_builds_fast() {
        let mut graph = SystemGraph::new();
        for i in 0..30 {
            graph.add_system(noop_system(&format!("sys_{i}")));
        }
        let start = std::time::Instant::now();
        graph.build().expect("should build");
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 50,
            "scheduling 30 systems took {:?}, far past the 0.5ms target",
            elapsed
        );
    }

    #[test]
    fn conflict_detection_splits_batches() {
        struct Pos;
        struct Vel;

        let sys_a = noop_system("a").writes::<Pos>();
        let sys_b = noop_system("b").reads::<Pos>();
        let sys_c = noop_system("c").writes::<Vel>();

        let systems = vec![sys_a, sys_b, sys_c];
        let groups = split_by_conflicts(&systems, &[0, 1, 2]);

        // a and b conflict (both touch Pos), c is independent.
        // So we expect at least 2 groups.
        assert!(
            groups.len() >= 2,
            "expected >=2 groups due to Pos conflict, got {}",
            groups.len()
        );
    }

    /// `dispatch_parallel` moves a `*mut World` onto rayon workers, which is
    /// only sound if `World` is `Send`. `SendPtr`'s bounds enforce that at the
    /// type level; this pins it so a future component storage without a
    /// `Send + Sync` bound fails here instead of silently re-opening the hole.
    #[test]
    fn world_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::super::world::World>();
        assert_send_sync::<SystemDescriptor>();
        assert_send_sync::<SendPtr<super::super::world::World>>();
    }

    #[test]
    fn parallel_batch_runs_every_system() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CompA;
        struct CompB;

        let counter = Arc::new(AtomicUsize::new(0));
        let c1 = counter.clone();
        let c2 = counter.clone();

        let mut graph = SystemGraph::new();
        graph.add_system(
            SystemDescriptor::new("a", move |_ctx: &mut SystemContext<'_>| {
                c1.fetch_add(1, Ordering::Relaxed);
            })
            .writes::<CompA>(),
        );
        graph.add_system(
            SystemDescriptor::new("b", move |_ctx: &mut SystemContext<'_>| {
                c2.fetch_add(1, Ordering::Relaxed);
            })
            .writes::<CompB>(),
        );
        graph.build().expect("should build");

        let mut world = super::super::world::World::new();
        graph.run(&mut world);
        assert_eq!(counter.load(Ordering::Relaxed), 2);

        // SAFETY: neither system touches the world at all.
        unsafe { graph.run_parallel(&mut world) };
        assert_eq!(counter.load(Ordering::Relaxed), 4);
    }

    /// Two systems the schedule would batch together may still make
    /// structural changes under `run`, which executes the batch one system at
    /// a time. (Under `run_parallel` the same graph would break its safety
    /// contract.)
    #[test]
    fn structural_changes_in_a_batch_are_sound_under_run() {
        struct CompA;
        struct CompB;

        let mut graph = SystemGraph::new();
        graph.add_system(
            SystemDescriptor::new("spawner", |ctx: &mut SystemContext<'_>| {
                ctx.world.spawn();
            })
            .writes::<CompA>(),
        );
        graph.add_system(
            SystemDescriptor::new("registrar", |ctx: &mut SystemContext<'_>| {
                let e = ctx.world.spawn();
                ctx.world.insert_dynamic(e, CompB);
            })
            .writes::<CompB>(),
        );
        graph.build().expect("should build");

        let mut world = super::super::world::World::new();
        graph.run(&mut world);
        assert_eq!(world.entity_count(), 2);
        assert!(world.dynamic::<CompB>().is_some());
    }
}
