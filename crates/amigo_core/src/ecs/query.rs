//! Generic query and join API for the ECS.
//!
//! # Usage
//!
//! ```ignore
//! use amigo_core::ecs::query::join;
//!
//! // Iterate entities with both Position and Health:
//! for (id, pos, health) in join(&world.positions, &world.healths) {
//!     println!("{id}: pos={pos:?}, health={health:?}");
//! }
//!
//! // Three-component join:
//! for (id, pos, vel, health) in join3(&world.positions, &world.velocities, &world.healths) {
//!     // ...
//! }
//!
//! // With filter (entity must also have SpriteComp):
//! for (id, pos, health) in join(&world.positions, &world.healths).with(&world.sprites) {
//!     // Only entities that have all three components
//! }
//! ```

use super::entity::EntityId;
use super::sparse_set::SparseSet;

// ---------------------------------------------------------------------------
// Join2: iterate entities present in two SparseSets
// ---------------------------------------------------------------------------

/// Iterator over entities that exist in both `A` and `B` SparseSets.
#[allow(dead_code)]
pub struct Join2<'a, A, B> {
    /// We iterate the smaller set and look up in the larger one.
    small_ids: &'a [EntityId],
    small_data: &'a [A],
    other: &'a SparseSet<B>,
    cursor: usize,
    swapped: bool,
    // If swapped, we need to look up A from B's perspective
    other_a: Option<&'a SparseSet<A>>,
}

/// Create a join iterator over two SparseSets.
/// Returns `(EntityId, &A, &B)` for each entity present in both.
pub fn join<'a, A, B>(a: &'a SparseSet<A>, b: &'a SparseSet<B>) -> JoinIter2<'a, A, B> {
    // Drive from the smaller set for efficiency
    if a.len() <= b.len() {
        JoinIter2 {
            drive_ids: a.entities(),
            drive_idx: 0,
            a,
            b,
            swapped: false,
        }
    } else {
        JoinIter2 {
            drive_ids: b.entities(),
            drive_idx: 0,
            a,
            b,
            swapped: true,
        }
    }
}

pub struct JoinIter2<'a, A, B> {
    drive_ids: &'a [EntityId],
    drive_idx: usize,
    a: &'a SparseSet<A>,
    b: &'a SparseSet<B>,
    #[allow(dead_code)]
    swapped: bool,
}

impl<'a, A, B> Iterator for JoinIter2<'a, A, B> {
    type Item = (EntityId, &'a A, &'a B);

    fn next(&mut self) -> Option<Self::Item> {
        while self.drive_idx < self.drive_ids.len() {
            let id = self.drive_ids[self.drive_idx];
            self.drive_idx += 1;

            if let (Some(a), Some(b)) = (self.a.get(id), self.b.get(id)) {
                return Some((id, a, b));
            }
        }
        None
    }
}

impl<'a, A, B> JoinIter2<'a, A, B> {
    /// Filter: only yield entities that also have component `C`.
    pub fn with<C>(self, filter: &'a SparseSet<C>) -> JoinIter2With<'a, A, B, C> {
        JoinIter2With {
            inner: self,
            filter,
        }
    }
}

pub struct JoinIter2With<'a, A, B, C> {
    inner: JoinIter2<'a, A, B>,
    filter: &'a SparseSet<C>,
}

impl<'a, A, B, C> Iterator for JoinIter2With<'a, A, B, C> {
    type Item = (EntityId, &'a A, &'a B);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, a, b) = self.inner.next()?;
            if self.filter.contains(id) {
                return Some((id, a, b));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Join3: iterate entities present in three SparseSets
// ---------------------------------------------------------------------------

pub struct JoinIter3<'a, A, B, C> {
    drive_ids: &'a [EntityId],
    drive_idx: usize,
    a: &'a SparseSet<A>,
    b: &'a SparseSet<B>,
    c: &'a SparseSet<C>,
}

/// Create a join iterator over three SparseSets.
/// Returns `(EntityId, &A, &B, &C)` for each entity present in all three.
pub fn join3<'a, A, B, C>(
    a: &'a SparseSet<A>,
    b: &'a SparseSet<B>,
    c: &'a SparseSet<C>,
) -> JoinIter3<'a, A, B, C> {
    // Drive from the smallest set
    let min_len = a.len().min(b.len()).min(c.len());
    let drive_ids = if a.len() == min_len {
        a.entities()
    } else if b.len() == min_len {
        b.entities()
    } else {
        c.entities()
    };

    JoinIter3 {
        drive_ids,
        drive_idx: 0,
        a,
        b,
        c,
    }
}

impl<'a, A, B, C> Iterator for JoinIter3<'a, A, B, C> {
    type Item = (EntityId, &'a A, &'a B, &'a C);

    fn next(&mut self) -> Option<Self::Item> {
        while self.drive_idx < self.drive_ids.len() {
            let id = self.drive_ids[self.drive_idx];
            self.drive_idx += 1;

            if let (Some(a), Some(b), Some(c)) = (self.a.get(id), self.b.get(id), self.c.get(id)) {
                return Some((id, a, b, c));
            }
        }
        None
    }
}

impl<'a, A, B, C> JoinIter3<'a, A, B, C> {
    /// Filter: only yield entities that also have component `D`.
    pub fn with<D>(self, filter: &'a SparseSet<D>) -> JoinIter3With<'a, A, B, C, D> {
        JoinIter3With {
            inner: self,
            filter,
        }
    }
}

pub struct JoinIter3With<'a, A, B, C, D> {
    inner: JoinIter3<'a, A, B, C>,
    filter: &'a SparseSet<D>,
}

impl<'a, A, B, C, D> Iterator for JoinIter3With<'a, A, B, C, D> {
    type Item = (EntityId, &'a A, &'a B, &'a C);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, a, b, c) = self.inner.next()?;
            if self.filter.contains(id) {
                return Some((id, a, b, c));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Join4: iterate entities present in four SparseSets
// ---------------------------------------------------------------------------

pub struct JoinIter4<'a, A, B, C, D> {
    drive_ids: &'a [EntityId],
    drive_idx: usize,
    a: &'a SparseSet<A>,
    b: &'a SparseSet<B>,
    c: &'a SparseSet<C>,
    d: &'a SparseSet<D>,
}

/// Create a join iterator over four SparseSets.
pub fn join4<'a, A, B, C, D>(
    a: &'a SparseSet<A>,
    b: &'a SparseSet<B>,
    c: &'a SparseSet<C>,
    d: &'a SparseSet<D>,
) -> JoinIter4<'a, A, B, C, D> {
    let min_len = a.len().min(b.len()).min(c.len()).min(d.len());
    let drive_ids = if a.len() == min_len {
        a.entities()
    } else if b.len() == min_len {
        b.entities()
    } else if c.len() == min_len {
        c.entities()
    } else {
        d.entities()
    };

    JoinIter4 {
        drive_ids,
        drive_idx: 0,
        a,
        b,
        c,
        d,
    }
}

impl<'a, A, B, C, D> Iterator for JoinIter4<'a, A, B, C, D> {
    type Item = (EntityId, &'a A, &'a B, &'a C, &'a D);

    fn next(&mut self) -> Option<Self::Item> {
        while self.drive_idx < self.drive_ids.len() {
            let id = self.drive_ids[self.drive_idx];
            self.drive_idx += 1;

            if let (Some(a), Some(b), Some(c), Some(d)) = (
                self.a.get(id),
                self.b.get(id),
                self.c.get(id),
                self.d.get(id),
            ) {
                return Some((id, a, b, c, d));
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// JoinMut: mutable iteration helpers
// ---------------------------------------------------------------------------

/// Collect entity IDs matching two SparseSets for mutable iteration.
///
/// Returns a Vec of EntityIds present in both sets. Use this to iterate
/// and mutate components safely:
///
/// ```ignore
/// for id in join_ids(&world.positions, &world.healths) {
///     let pos = world.positions.get(id).unwrap();
///     let health = world.healths.get_mut(id).unwrap();
///     // ...
/// }
/// ```
pub fn join_ids<A, B>(a: &SparseSet<A>, b: &SparseSet<B>) -> Vec<EntityId> {
    let (smaller, larger_check): (&[EntityId], &SparseSet<B>) = if a.len() <= b.len() {
        (a.entities(), b)
    } else {
        // When b is smaller, we iterate b's entities and check a
        // but we need to return the right type... just iterate a for simplicity
        (a.entities(), b)
    };
    smaller
        .iter()
        .copied()
        .filter(|id| larger_check.contains(*id))
        .collect()
}

/// Apply a closure to each entity present in both SparseSets, with mutable
/// access to the second set.
///
/// ```ignore
/// join_mut(&world.positions, &mut world.healths, |id, pos, health| {
///     health.current -= 1;
/// });
/// ```
pub fn join_mut<A, B, F>(a: &SparseSet<A>, b: &mut SparseSet<B>, mut f: F)
where
    F: FnMut(EntityId, &A, &mut B),
{
    let ids: Vec<EntityId> = a
        .entities()
        .iter()
        .copied()
        .filter(|id| b.contains(*id))
        .collect();
    for id in ids {
        if let Some(a_val) = a.get(id) {
            if let Some(b_val) = b.get_mut(id) {
                f(id, a_val, b_val);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Generic component access on World
// ---------------------------------------------------------------------------

/// Marker trait for component types that have static storage in World.
///
/// Implemented for the 5 built-in component types. Game-specific types should
/// use `world.dynamic::<T>()` / `world.insert_dynamic()` instead.
pub trait Component: Sized + 'static {
    /// Get the SparseSet for this component from the World.
    fn storage(world: &super::world::World) -> &SparseSet<Self>;
    /// Get the mutable SparseSet for this component from the World.
    fn storage_mut(world: &mut super::world::World) -> &mut SparseSet<Self>;
}

macro_rules! impl_component {
    ($ty:ty, $field:ident) => {
        impl Component for $ty {
            fn storage(world: &super::world::World) -> &SparseSet<Self> {
                &world.$field
            }
            fn storage_mut(world: &mut super::world::World) -> &mut SparseSet<Self> {
                &mut world.$field
            }
        }
    };
}

impl_component!(super::world::Position, positions);
impl_component!(super::world::Velocity, velocities);
impl_component!(super::world::Health, healths);
impl_component!(super::world::SpriteComp, sprites);
impl_component!(super::world::StateScoped, state_scoped);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::world::*;

    #[test]
    fn test_join2() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();

        world.positions.insert(a, Position(crate::SimVec2::ZERO));
        world.positions.insert(b, Position(crate::SimVec2::ZERO));
        world.positions.insert(c, Position(crate::SimVec2::ZERO));

        world.healths.insert(a, Health::new(100));
        // b has no health
        world.healths.insert(c, Health::new(50));

        let results: Vec<_> = join(&world.positions, &world.healths).collect();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_join2_with_filter() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();

        world.positions.insert(a, Position(crate::SimVec2::ZERO));
        world.positions.insert(b, Position(crate::SimVec2::ZERO));
        world.healths.insert(a, Health::new(100));
        world.healths.insert(b, Health::new(50));

        // Only a has a sprite
        world.sprites.insert(a, SpriteComp::new("test"));

        let results: Vec<_> = join(&world.positions, &world.healths)
            .with(&world.sprites)
            .collect();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_join3() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();

        world.positions.insert(a, Position(crate::SimVec2::ZERO));
        world.positions.insert(b, Position(crate::SimVec2::ZERO));
        world.velocities.insert(a, Velocity(crate::SimVec2::ZERO));
        world.velocities.insert(b, Velocity(crate::SimVec2::ZERO));
        world.healths.insert(a, Health::new(100));
        // b has no health

        let results: Vec<_> = join3(&world.positions, &world.velocities, &world.healths).collect();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_component_trait() {
        let mut world = World::new();
        let id = world.spawn();

        // Generic add
        Position::storage_mut(&mut world).insert(id, Position(crate::SimVec2::ZERO));

        // Generic get
        assert!(Position::storage(&world).get(id).is_some());
    }
}
