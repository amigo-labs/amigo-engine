use super::bitset::BitSet;
use super::entity::EntityId;

const EMPTY: u32 = u32::MAX;

/// SparseSet storage for a single component type.
/// Dense array for cache-friendly iteration, sparse lookup for O(1) access by EntityId.
pub struct SparseSet<T> {
    sparse: Vec<u32>,
    dense_ids: Vec<EntityId>,
    dense_data: Vec<T>,
    changed: BitSet,
    added: BitSet,
    removed_ids: Vec<EntityId>,
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            sparse: Vec::new(),
            dense_ids: Vec::new(),
            dense_data: Vec::new(),
            changed: BitSet::new(),
            added: BitSet::new(),
            removed_ids: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            sparse: Vec::new(),
            dense_ids: Vec::with_capacity(capacity),
            dense_data: Vec::with_capacity(capacity),
            changed: BitSet::new(),
            added: BitSet::new(),
            removed_ids: Vec::new(),
        }
    }

    fn ensure_sparse(&mut self, index: u32) {
        let idx = index as usize;
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, EMPTY);
        }
    }

    /// Insert a component for the given entity.
    pub fn insert(&mut self, id: EntityId, data: T) {
        let idx = id.index;
        self.ensure_sparse(idx);

        if self.sparse[idx as usize] != EMPTY {
            // Entity already has this component - update it
            let dense_idx = self.sparse[idx as usize] as usize;
            self.dense_data[dense_idx] = data;
            self.changed.set(idx);
        } else {
            // New component
            let dense_idx = self.dense_data.len() as u32;
            self.sparse[idx as usize] = dense_idx;
            self.dense_ids.push(id);
            self.dense_data.push(data);
            self.added.set(idx);
        }
    }

    /// Remove the component for the given entity.
    pub fn remove(&mut self, id: EntityId) -> Option<T> {
        let idx = id.index as usize;
        if idx >= self.sparse.len() || self.sparse[idx] == EMPTY {
            return None;
        }

        let dense_idx = self.sparse[idx] as usize;
        self.sparse[idx] = EMPTY;

        // Swap-remove from dense arrays
        let last_dense = self.dense_data.len() - 1;
        if dense_idx != last_dense {
            let last_entity = self.dense_ids[last_dense];
            self.sparse[last_entity.index as usize] = dense_idx as u32;
        }
        self.dense_ids.swap_remove(dense_idx);
        let removed = self.dense_data.swap_remove(dense_idx);

        self.removed_ids.push(id);
        Some(removed)
    }

    /// Get an immutable reference to the component for the given entity.
    pub fn get(&self, id: EntityId) -> Option<&T> {
        let idx = id.index as usize;
        if idx >= self.sparse.len() || self.sparse[idx] == EMPTY {
            return None;
        }
        Some(&self.dense_data[self.sparse[idx] as usize])
    }

    /// Get a mutable reference to the component. Marks the entity as changed.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut T> {
        let idx = id.index as usize;
        if idx >= self.sparse.len() || self.sparse[idx] == EMPTY {
            return None;
        }
        self.changed.set(id.index);
        Some(&mut self.dense_data[self.sparse[idx] as usize])
    }

    /// Check if an entity has this component.
    pub fn contains(&self, id: EntityId) -> bool {
        let idx = id.index as usize;
        idx < self.sparse.len() && self.sparse[idx] != EMPTY
    }

    /// Number of entities with this component.
    pub fn len(&self) -> usize {
        self.dense_data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense_data.is_empty()
    }

    /// Iterate over all (EntityId, &T) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (EntityId, &T)> {
        self.dense_ids.iter().copied().zip(self.dense_data.iter())
    }

    /// Iterate over all (EntityId, &mut T) pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (EntityId, &mut T)> {
        self.dense_ids
            .iter()
            .copied()
            .zip(self.dense_data.iter_mut())
    }

    /// Iterate over entities that were added this tick.
    pub fn iter_added(&self) -> impl Iterator<Item = (EntityId, &T)> + '_ {
        self.added.iter_set().filter_map(|idx| {
            let sparse_idx = idx as usize;
            if sparse_idx < self.sparse.len() && self.sparse[sparse_idx] != EMPTY {
                let dense = self.sparse[sparse_idx] as usize;
                Some((self.dense_ids[dense], &self.dense_data[dense]))
            } else {
                None
            }
        })
    }

    /// Iterate over entities that were changed this tick.
    pub fn iter_changed(&self) -> impl Iterator<Item = (EntityId, &T)> + '_ {
        self.changed.iter_set().filter_map(|idx| {
            let sparse_idx = idx as usize;
            if sparse_idx < self.sparse.len() && self.sparse[sparse_idx] != EMPTY {
                let dense = self.sparse[sparse_idx] as usize;
                Some((self.dense_ids[dense], &self.dense_data[dense]))
            } else {
                None
            }
        })
    }

    /// Get removed entity IDs from this tick.
    pub fn removed(&self) -> &[EntityId] {
        &self.removed_ids
    }

    /// Clear change tracking (call at end of tick).
    pub fn flush(&mut self) {
        self.changed.clear();
        self.added.clear();
        self.removed_ids.clear();
    }

    /// Access dense data slice.
    pub fn data(&self) -> &[T] {
        &self.dense_data
    }

    /// Access dense entity IDs.
    pub fn entities(&self) -> &[EntityId] {
        &self.dense_ids
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
