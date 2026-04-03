//! Incremental, tick-based change detection for the ECS (ADR-0003).
//!
//! Instead of per-frame BitSets that are cleared every tick, each component
//! slot stores a `Tick` (monotonic `u32` counter) recording when it was last
//! added or mutated. Systems compare their own `last_run` tick against these
//! stamps to determine what changed since they last executed.
//!
//! # Key types
//!
//! - [`Tick`] -- monotonic u32 counter.
//! - [`ComponentTicks`] -- per-component-slot pair of (added, changed) ticks.
//! - [`Ticks`] -- references to a slot's ticks plus the current world tick,
//!   used internally by [`Mut`].
//! - [`Mut<T>`] -- smart pointer returned by mutable component access. Sets
//!   the changed tick only when `DerefMut` is invoked (i.e. actual writes).
//! - [`Added<T>`] / [`Changed<T>`] -- query-filter markers that restrict
//!   iteration to entities whose component was added / changed since the
//!   system's `last_run` tick.

use std::ops::{Deref, DerefMut};

// ── Tick ────────────────────────────────────────────────────────────────

/// Monotonically increasing counter representing a point in time.
///
/// The world advances this once per frame (or per schedule run).
/// Wrapping is handled via `is_newer_than`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub u32);

impl Tick {
    /// The initial tick value (before any frame has run).
    pub const ZERO: Self = Self(0);

    /// Create a new tick.
    #[inline]
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Get the raw value.
    #[inline]
    pub fn get(self) -> u32 {
        self.0
    }

    /// Check whether `self` is strictly newer than `other`, handling u32
    /// wrapping by treating differences > `u32::MAX / 2` as "older".
    #[inline]
    pub fn is_newer_than(self, other: Tick) -> bool {
        // Wrapping subtraction: if self is newer, the difference will be
        // a small positive number; if older after wrap, it will be large.
        let diff = self.0.wrapping_sub(other.0);
        diff > 0 && diff < u32::MAX / 2
    }
}

// ── ComponentTicks ──────────────────────────────────────────────────────

/// Per-component-slot tick pair: when it was added, when it was last changed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ComponentTicks {
    pub added: Tick,
    pub changed: Tick,
}

impl ComponentTicks {
    /// Create ticks for a freshly added component at the given world tick.
    #[inline]
    pub fn new(tick: Tick) -> Self {
        Self {
            added: tick,
            changed: tick,
        }
    }

    /// Was this component added after `last_run`?
    #[inline]
    pub fn is_added(&self, last_run: Tick) -> bool {
        self.added.is_newer_than(last_run)
    }

    /// Was this component changed (including added) after `last_run`?
    #[inline]
    pub fn is_changed(&self, last_run: Tick) -> bool {
        self.changed.is_newer_than(last_run)
    }

    /// Mark as changed at the given tick.
    #[inline]
    pub fn set_changed(&mut self, tick: Tick) {
        self.changed = tick;
    }
}

// ── Ticks (borrowed view used by Mut<T>) ────────────────────────────────

/// Borrowed references to a component slot's ticks plus the current world
/// tick. Carried inside [`Mut<T>`] so that `DerefMut` can stamp the change.
pub struct Ticks<'a> {
    pub component_ticks: &'a mut ComponentTicks,
    pub world_tick: Tick,
}

// ── Mut<T> ──────────────────────────────────────────────────────────────

/// Smart-pointer wrapper for mutable component access.
///
/// `Deref` gives `&T` *without* marking the component as changed.
/// `DerefMut` gives `&mut T` *and* stamps `changed_tick = world_tick`.
///
/// This eliminates false positives from code that borrows mutably but
/// only reads.
pub struct Mut<'a, T> {
    value: &'a mut T,
    ticks: Ticks<'a>,
}

impl<'a, T> Mut<'a, T> {
    /// Create a new `Mut` wrapper.
    #[inline]
    pub fn new(
        value: &'a mut T,
        component_ticks: &'a mut ComponentTicks,
        world_tick: Tick,
    ) -> Self {
        Self {
            value,
            ticks: Ticks {
                component_ticks,
                world_tick,
            },
        }
    }

    /// Was this component added since `last_run`?
    #[inline]
    pub fn is_added(&self, last_run: Tick) -> bool {
        self.ticks.component_ticks.is_added(last_run)
    }

    /// Was this component changed since `last_run`?
    #[inline]
    pub fn is_changed(&self, last_run: Tick) -> bool {
        self.ticks.component_ticks.is_changed(last_run)
    }
}

impl<T> Deref for Mut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T> DerefMut for Mut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.ticks
            .component_ticks
            .set_changed(self.ticks.world_tick);
        self.value
    }
}

// ── Query Filters ───────────────────────────────────────────────────────

/// Marker type for query filters: only yield entities whose `T` was added
/// after the system's `last_run` tick.
///
/// Usage (conceptual):
/// ```ignore
/// // In a system with access to last_run tick:
/// for (id, val) in Added::<Position>::filter(&sparse_set, &ticks_storage, last_run) {
///     // `val` was added since this system last ran
/// }
/// ```
pub struct Added<T> {
    _marker: std::marker::PhantomData<T>,
}

/// Marker type for query filters: only yield entities whose `T` was changed
/// (or added) after the system's `last_run` tick.
pub struct Changed<T> {
    _marker: std::marker::PhantomData<T>,
}

// ── TickStorage: parallel Vec<ComponentTicks> alongside SparseSet ──────

/// Parallel tick storage that mirrors a `SparseSet<T>`.
///
/// Indexed identically to the dense arrays in the SparseSet: dense index `i`
/// in the SparseSet corresponds to `ticks[i]` here.
#[derive(Clone, Debug, Default)]
pub struct TickStorage {
    ticks: Vec<ComponentTicks>,
}

impl TickStorage {
    pub fn new() -> Self {
        Self { ticks: Vec::new() }
    }

    /// Push ticks for a newly added component (at the end of dense storage).
    #[inline]
    pub fn push_added(&mut self, tick: Tick) {
        self.ticks.push(ComponentTicks::new(tick));
    }

    /// Get the ticks for a dense index.
    #[inline]
    pub fn get(&self, dense_index: usize) -> Option<&ComponentTicks> {
        self.ticks.get(dense_index)
    }

    /// Get mutable ticks for a dense index.
    #[inline]
    pub fn get_mut(&mut self, dense_index: usize) -> Option<&mut ComponentTicks> {
        self.ticks.get_mut(dense_index)
    }

    /// Swap-remove to mirror SparseSet's swap-remove.
    #[inline]
    pub fn swap_remove(&mut self, dense_index: usize) -> ComponentTicks {
        self.ticks.swap_remove(dense_index)
    }

    /// Number of entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.ticks.len()
    }

    /// Is empty?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ticks.is_empty()
    }

    /// Iterate all ticks.
    pub fn iter(&self) -> impl Iterator<Item = &ComponentTicks> {
        self.ticks.iter()
    }
}

// ── Filter iterators ────────────────────────────────────────────────────

use super::entity::EntityId;
use super::sparse_set::SparseSet;

impl<T> Added<T> {
    /// Iterate entities whose component `T` was added after `last_run`.
    pub fn filter<'a>(
        set: &'a SparseSet<T>,
        tick_storage: &'a TickStorage,
        last_run: Tick,
    ) -> impl Iterator<Item = (EntityId, &'a T)> + 'a {
        set.entities()
            .iter()
            .copied()
            .zip(set.data().iter())
            .zip(tick_storage.iter())
            .filter_map(move |((id, data), ct)| {
                if ct.is_added(last_run) {
                    Some((id, data))
                } else {
                    None
                }
            })
    }
}

impl<T> Changed<T> {
    /// Iterate entities whose component `T` was changed (or added) after
    /// `last_run`.
    pub fn filter<'a>(
        set: &'a SparseSet<T>,
        tick_storage: &'a TickStorage,
        last_run: Tick,
    ) -> impl Iterator<Item = (EntityId, &'a T)> + 'a {
        set.entities()
            .iter()
            .copied()
            .zip(set.data().iter())
            .zip(tick_storage.iter())
            .filter_map(move |((id, data), ct)| {
                if ct.is_changed(last_run) {
                    Some((id, data))
                } else {
                    None
                }
            })
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::entity::EntityId;
    use crate::ecs::sparse_set::SparseSet;

    fn entity(index: u32) -> EntityId {
        EntityId::from_raw(index, 0)
    }

    #[test]
    fn tick_is_newer_than() {
        let t0 = Tick::new(0);
        let t1 = Tick::new(1);
        let t5 = Tick::new(5);

        assert!(!t0.is_newer_than(t0));
        assert!(t1.is_newer_than(t0));
        assert!(t5.is_newer_than(t1));
        assert!(!t1.is_newer_than(t5));
    }

    #[test]
    fn tick_wrapping() {
        let near_max = Tick::new(u32::MAX - 1);
        let wrapped = Tick::new(1);
        // After wrapping, 1 should be newer than MAX-1
        assert!(wrapped.is_newer_than(near_max));
    }

    #[test]
    fn component_ticks_added_and_changed() {
        let tick3 = Tick::new(3);
        let ct = ComponentTicks::new(tick3);

        // Both added and changed should be newer than tick 2
        assert!(ct.is_added(Tick::new(2)));
        assert!(ct.is_changed(Tick::new(2)));

        // But not newer than tick 3 (same tick)
        assert!(!ct.is_added(Tick::new(3)));
        assert!(!ct.is_changed(Tick::new(3)));
    }

    #[test]
    fn mut_deref_does_not_mark_changed() {
        let mut value = 42u32;
        let mut ticks = ComponentTicks::new(Tick::new(1));
        let world_tick = Tick::new(5);

        {
            let wrapper = Mut::new(&mut value, &mut ticks, world_tick);
            // Read through Deref -- should NOT update changed tick
            let _read: &u32 = &*wrapper;
        }

        // Changed tick should still be the original (1)
        assert_eq!(ticks.changed, Tick::new(1));
    }

    #[test]
    fn mut_deref_mut_marks_changed() {
        let mut value = 42u32;
        let mut ticks = ComponentTicks::new(Tick::new(1));
        let world_tick = Tick::new(5);

        {
            let mut wrapper = Mut::new(&mut value, &mut ticks, world_tick);
            // Write through DerefMut -- SHOULD update changed tick
            *wrapper = 99;
        }

        assert_eq!(ticks.changed, Tick::new(5));
        assert_eq!(value, 99);
    }

    #[test]
    fn added_filter() {
        let mut set = SparseSet::<f32>::new();
        let mut ts = TickStorage::new();

        let e0 = entity(0);
        let e1 = entity(1);
        let e2 = entity(2);

        // Insert at tick 1
        set.insert(e0, 1.0);
        ts.push_added(Tick::new(1));

        // Insert at tick 3
        set.insert(e1, 2.0);
        ts.push_added(Tick::new(3));

        // Insert at tick 5
        set.insert(e2, 3.0);
        ts.push_added(Tick::new(5));

        // A system that last ran at tick 2 should see e1 and e2
        let results: Vec<_> = Added::<f32>::filter(&set, &ts, Tick::new(2)).collect();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, e1);
        assert_eq!(results[1].0, e2);
    }

    #[test]
    fn changed_filter() {
        let mut set = SparseSet::<f32>::new();
        let mut ts = TickStorage::new();

        let e0 = entity(0);
        let e1 = entity(1);

        // Both inserted at tick 1
        set.insert(e0, 1.0);
        ts.push_added(Tick::new(1));
        set.insert(e1, 2.0);
        ts.push_added(Tick::new(1));

        // Mutate e1 at tick 4 (simulating Mut<T>::deref_mut)
        ts.get_mut(1).unwrap().set_changed(Tick::new(4));

        // System last ran at tick 2: only e1 should show as changed
        let results: Vec<_> = Changed::<f32>::filter(&set, &ts, Tick::new(2)).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e1);
    }

    #[test]
    fn tick_storage_swap_remove_mirrors_sparse_set() {
        let mut set = SparseSet::<&str>::new();
        let mut ts = TickStorage::new();

        let e0 = entity(0);
        let e1 = entity(1);
        let e2 = entity(2);

        set.insert(e0, "a");
        ts.push_added(Tick::new(1));
        set.insert(e1, "b");
        ts.push_added(Tick::new(2));
        set.insert(e2, "c");
        ts.push_added(Tick::new(3));

        // Remove e0 (dense index 0): last element swaps in
        set.remove(e0);
        ts.swap_remove(0);

        assert_eq!(ts.len(), 2);
        // The element that was at the end (tick 3 for e2) is now at index 0
        assert_eq!(ts.get(0).unwrap().added, Tick::new(3));
        assert_eq!(ts.get(1).unwrap().added, Tick::new(2));
    }
}
