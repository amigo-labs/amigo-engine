//! Archetype-based ECS storage (ADR-0001).
//!
//! Groups entities that share the exact same set of component types into
//! contiguous "archetype tables." Multi-component iteration becomes a linear
//! scan with zero sparse lookups.

use super::entity::EntityId;
use rustc_hash::FxHashMap;
use std::alloc::{self, Layout};
use std::any::TypeId;
use std::ptr;

// ---------------------------------------------------------------------------
// IDs and locations
// ---------------------------------------------------------------------------

/// Identifies an archetype by its sorted set of component types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArchetypeId(u64);

impl ArchetypeId {
    /// Compute archetype ID from a sorted slice of TypeIds.
    pub fn from_type_ids(types: &[TypeId]) -> Self {
        // FNV-1a hash over the sorted TypeId bytes.
        let mut hash: u64 = 0xcbf29ce484222325;
        for tid in types {
            let bytes: [u8; std::mem::size_of::<TypeId>()] = unsafe { std::mem::transmute(*tid) };
            for b in bytes {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
        Self(hash)
    }

    /// The empty archetype (no components).
    pub fn empty() -> Self {
        Self(0xcbf29ce484222325) // FNV offset basis
    }
}

/// Where an entity lives inside the archetype storage.
#[derive(Clone, Copy, Debug)]
pub struct ArchetypeLocation {
    pub archetype_id: ArchetypeId,
    pub row: u32,
}

// ---------------------------------------------------------------------------
// Column — type-erased contiguous storage for one component type
// ---------------------------------------------------------------------------

/// Type-erased column storage for a single component type.
struct Column {
    type_id: TypeId,
    item_layout: Layout,
    drop_fn: Option<unsafe fn(*mut u8)>,
    data: *mut u8,
    capacity: usize,
    len: usize,
}

impl Column {
    fn new(type_id: TypeId, item_layout: Layout, drop_fn: Option<unsafe fn(*mut u8)>) -> Self {
        Self {
            type_id,
            item_layout,
            drop_fn,
            data: ptr::null_mut(),
            capacity: 0,
            len: 0,
        }
    }

    fn grow_if_needed(&mut self) {
        if self.len < self.capacity {
            return;
        }
        let new_cap = if self.capacity == 0 {
            8
        } else {
            self.capacity * 2
        };
        let size = self.item_layout.size();
        if size == 0 {
            self.capacity = new_cap;
            return;
        }
        let new_layout = Layout::from_size_align(size * new_cap, self.item_layout.align()).unwrap();
        let new_data = unsafe { alloc::alloc(new_layout) };
        if new_data.is_null() {
            alloc::handle_alloc_error(new_layout);
        }
        if !self.data.is_null() && self.len > 0 {
            unsafe { ptr::copy_nonoverlapping(self.data, new_data, size * self.len) };
            let old_layout =
                Layout::from_size_align(size * self.capacity, self.item_layout.align()).unwrap();
            unsafe { alloc::dealloc(self.data, old_layout) };
        }
        self.data = new_data;
        self.capacity = new_cap;
    }

    /// Push a value (caller provides raw bytes, must be correct type).
    unsafe fn push_raw(&mut self, src: *const u8) {
        self.grow_if_needed();
        let size = self.item_layout.size();
        if size > 0 {
            let dst = self.data.add(size * self.len);
            ptr::copy_nonoverlapping(src, dst, size);
        }
        self.len += 1;
    }

    /// Swap-remove a row. Returns whether a swap actually happened (vs. popping last).
    unsafe fn swap_remove(&mut self, row: usize) -> bool {
        let last = self.len - 1;
        let swapped = row != last;
        let size = self.item_layout.size();
        if size > 0 {
            let row_ptr = self.data.add(size * row);
            // Drop the removed element.
            if let Some(drop_fn) = self.drop_fn {
                drop_fn(row_ptr);
            }
            if swapped {
                let last_ptr = self.data.add(size * last);
                ptr::copy_nonoverlapping(last_ptr, row_ptr, size);
            }
        }
        self.len -= 1;
        swapped
    }

    /// Get pointer to row data.
    unsafe fn get_row(&self, row: usize) -> *const u8 {
        self.data.add(self.item_layout.size() * row)
    }

    /// Get mutable pointer to row data.
    #[allow(dead_code)]
    unsafe fn get_row_mut(&mut self, row: usize) -> *mut u8 {
        self.data.add(self.item_layout.size() * row)
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl Drop for Column {
    fn drop(&mut self) {
        let size = self.item_layout.size();
        if size > 0 && !self.data.is_null() {
            // Drop all remaining elements.
            if let Some(drop_fn) = self.drop_fn {
                for i in 0..self.len {
                    unsafe { drop_fn(self.data.add(size * i)) };
                }
            }
            let layout =
                Layout::from_size_align(size * self.capacity, self.item_layout.align()).unwrap();
            unsafe { alloc::dealloc(self.data, layout) };
        }
    }
}

// SAFETY: Column data is only accessed through &self/&mut self.
unsafe impl Send for Column {}
unsafe impl Sync for Column {}

// ---------------------------------------------------------------------------
// Archetype — a table of entities sharing the same component set
// ---------------------------------------------------------------------------

/// Component type descriptor for registration.
#[derive(Clone, Debug)]
pub struct ComponentDescriptor {
    pub type_id: TypeId,
    pub layout: Layout,
    pub drop_fn: Option<unsafe fn(*mut u8)>,
    pub name: &'static str,
}

/// A table of entities that all have the same set of components.
pub struct Archetype {
    id: ArchetypeId,
    /// Component type IDs in sorted order.
    component_types: Vec<TypeId>,
    /// Columns indexed parallel to component_types.
    columns: Vec<Column>,
    /// Entity IDs for each row.
    entities: Vec<EntityId>,
    /// TypeId → column index lookup.
    type_to_column: FxHashMap<TypeId, usize>,
}

impl Archetype {
    pub fn new(id: ArchetypeId, descriptors: &[ComponentDescriptor]) -> Self {
        let mut sorted_descs: Vec<_> = descriptors.to_vec();
        sorted_descs.sort_by_key(|d| d.type_id);

        let component_types: Vec<TypeId> = sorted_descs.iter().map(|d| d.type_id).collect();
        let columns: Vec<Column> = sorted_descs
            .iter()
            .map(|d| Column::new(d.type_id, d.layout, d.drop_fn))
            .collect();
        let type_to_column: FxHashMap<TypeId, usize> = component_types
            .iter()
            .enumerate()
            .map(|(i, &tid)| (tid, i))
            .collect();

        Self {
            id,
            component_types,
            columns,
            entities: Vec::new(),
            type_to_column,
        }
    }

    /// Archetype ID.
    pub fn id(&self) -> ArchetypeId {
        self.id
    }

    /// Component types in this archetype (sorted).
    pub fn component_types(&self) -> &[TypeId] {
        &self.component_types
    }

    /// Number of entities in this archetype.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Entity IDs.
    pub fn entities(&self) -> &[EntityId] {
        &self.entities
    }

    /// Check if this archetype contains a given component type.
    pub fn has_component(&self, type_id: TypeId) -> bool {
        self.type_to_column.contains_key(&type_id)
    }

    /// Push an entity with all component data. `components` must be in the
    /// same order as `component_types()` and each buffer must have the correct
    /// size/alignment for its type.
    ///
    /// Returns the row index.
    pub unsafe fn push_entity(&mut self, entity: EntityId, components: &[*const u8]) -> u32 {
        debug_assert_eq!(components.len(), self.columns.len());
        let row = self.entities.len() as u32;
        self.entities.push(entity);
        for (col, &src) in self.columns.iter_mut().zip(components.iter()) {
            col.push_raw(src);
        }
        row
    }

    /// Remove an entity by row index (swap-remove). Returns the entity that
    /// was swapped into this row (if any) so the caller can update its location.
    pub unsafe fn swap_remove(&mut self, row: u32) -> Option<EntityId> {
        let row = row as usize;
        let last = self.entities.len() - 1;
        let swapped_entity = if row != last {
            Some(self.entities[last])
        } else {
            None
        };

        self.entities.swap_remove(row);
        for col in &mut self.columns {
            col.swap_remove(row);
        }

        swapped_entity
    }

    /// Get a typed slice of a component column.
    ///
    /// # Safety
    /// T must match the actual stored type for that TypeId.
    pub unsafe fn column_slice<T: 'static>(&self) -> Option<&[T]> {
        let col_idx = self.type_to_column.get(&TypeId::of::<T>())?;
        let col = &self.columns[*col_idx];
        if col.len() == 0 {
            return Some(&[]);
        }
        Some(std::slice::from_raw_parts(col.data as *const T, col.len()))
    }

    /// Get a typed mutable slice of a component column.
    ///
    /// # Safety
    /// T must match the actual stored type for that TypeId.
    pub unsafe fn column_slice_mut<T: 'static>(&mut self) -> Option<&mut [T]> {
        let col_idx = self.type_to_column.get(&TypeId::of::<T>())?;
        let col = &mut self.columns[*col_idx];
        if col.len() == 0 {
            return Some(&mut []);
        }
        Some(std::slice::from_raw_parts_mut(
            col.data as *mut T,
            col.len(),
        ))
    }

    /// Get raw pointer to a component column row.
    pub unsafe fn get_component_raw(&self, type_id: TypeId, row: u32) -> Option<*const u8> {
        let col_idx = self.type_to_column.get(&type_id)?;
        Some(self.columns[*col_idx].get_row(row as usize))
    }
}

// ---------------------------------------------------------------------------
// ArchetypeMap — collection of all archetypes + entity location tracking
// ---------------------------------------------------------------------------

/// Manages all archetypes and entity-to-archetype mappings.
pub struct ArchetypeMap {
    archetypes: FxHashMap<ArchetypeId, Archetype>,
    /// Entity index → archetype location. None if entity not tracked here.
    locations: Vec<Option<ArchetypeLocation>>,
    /// Edge cache: (archetype_id, added_type) → target archetype_id.
    add_edges: FxHashMap<(ArchetypeId, TypeId), ArchetypeId>,
    /// Edge cache: (archetype_id, removed_type) → target archetype_id.
    remove_edges: FxHashMap<(ArchetypeId, TypeId), ArchetypeId>,
}

impl ArchetypeMap {
    pub fn new() -> Self {
        Self {
            archetypes: FxHashMap::default(),
            locations: Vec::new(),
            add_edges: FxHashMap::default(),
            remove_edges: FxHashMap::default(),
        }
    }

    /// Get or create an archetype for the given component set.
    pub fn get_or_create(&mut self, descriptors: &[ComponentDescriptor]) -> ArchetypeId {
        let mut sorted: Vec<TypeId> = descriptors.iter().map(|d| d.type_id).collect();
        sorted.sort();
        let id = ArchetypeId::from_type_ids(&sorted);

        if !self.archetypes.contains_key(&id) {
            self.archetypes.insert(id, Archetype::new(id, descriptors));
        }
        id
    }

    /// Get archetype by ID.
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(&id)
    }

    /// Get mutable archetype by ID.
    pub fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.archetypes.get_mut(&id)
    }

    /// Get entity location.
    pub fn entity_location(&self, entity: EntityId) -> Option<ArchetypeLocation> {
        self.locations
            .get(entity.index() as usize)
            .and_then(|loc| *loc)
    }

    /// Set entity location.
    pub fn set_entity_location(&mut self, entity: EntityId, location: Option<ArchetypeLocation>) {
        let idx = entity.index() as usize;
        if idx >= self.locations.len() {
            self.locations.resize(idx + 1, None);
        }
        self.locations[idx] = location;
    }

    /// Insert an entity into an archetype. Returns the row.
    pub unsafe fn insert_entity(
        &mut self,
        archetype_id: ArchetypeId,
        entity: EntityId,
        components: &[*const u8],
    ) -> u32 {
        let archetype = self.archetypes.get_mut(&archetype_id).unwrap();
        let row = archetype.push_entity(entity, components);
        self.set_entity_location(entity, Some(ArchetypeLocation { archetype_id, row }));
        row
    }

    /// Remove an entity from its archetype.
    pub unsafe fn remove_entity(&mut self, entity: EntityId) {
        if let Some(loc) = self.entity_location(entity) {
            let swapped = {
                let archetype = self.archetypes.get_mut(&loc.archetype_id).unwrap();
                archetype.swap_remove(loc.row)
            };
            // Update the swapped entity's location.
            if let Some(swapped_entity) = swapped {
                self.set_entity_location(
                    swapped_entity,
                    Some(ArchetypeLocation {
                        archetype_id: loc.archetype_id,
                        row: loc.row,
                    }),
                );
            }
            self.set_entity_location(entity, None);
        }
    }

    /// Cache an "add component" edge.
    pub fn cache_add_edge(&mut self, from: ArchetypeId, component: TypeId, to: ArchetypeId) {
        self.add_edges.insert((from, component), to);
    }

    /// Cache a "remove component" edge.
    #[allow(dead_code)]
    pub fn cache_remove_edge(&mut self, from: ArchetypeId, component: TypeId, to: ArchetypeId) {
        self.remove_edges.insert((from, component), to);
    }

    /// Look up a cached add edge.
    pub fn lookup_add_edge(&self, from: ArchetypeId, component: TypeId) -> Option<ArchetypeId> {
        self.add_edges.get(&(from, component)).copied()
    }

    /// Look up a cached remove edge.
    #[allow(dead_code)]
    pub fn lookup_remove_edge(&self, from: ArchetypeId, component: TypeId) -> Option<ArchetypeId> {
        self.remove_edges.get(&(from, component)).copied()
    }

    /// Iterate over all archetypes.
    pub fn iter(&self) -> impl Iterator<Item = (&ArchetypeId, &Archetype)> {
        self.archetypes.iter()
    }

    /// Iterate over all archetypes that contain all specified component types.
    pub fn matching_archetypes<'a>(
        &'a self,
        required: &'a [TypeId],
    ) -> impl Iterator<Item = &'a Archetype> + 'a {
        self.archetypes
            .values()
            .filter(move |arch| required.iter().all(|tid| arch.has_component(*tid)))
    }
}

impl Default for ArchetypeMap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Velocity {
        dx: f32,
        dy: f32,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Health(i32);

    fn pos_desc() -> ComponentDescriptor {
        ComponentDescriptor {
            type_id: TypeId::of::<Position>(),
            layout: Layout::new::<Position>(),
            drop_fn: None,
            name: "Position",
        }
    }

    fn vel_desc() -> ComponentDescriptor {
        ComponentDescriptor {
            type_id: TypeId::of::<Velocity>(),
            layout: Layout::new::<Velocity>(),
            drop_fn: None,
            name: "Velocity",
        }
    }

    fn health_desc() -> ComponentDescriptor {
        ComponentDescriptor {
            type_id: TypeId::of::<Health>(),
            layout: Layout::new::<Health>(),
            drop_fn: None,
            name: "Health",
        }
    }

    #[test]
    fn archetype_insert_and_read() {
        let descs = vec![pos_desc(), vel_desc()];
        let mut sorted_types: Vec<TypeId> = descs.iter().map(|d| d.type_id).collect();
        sorted_types.sort();
        let id = ArchetypeId::from_type_ids(&sorted_types);
        let mut arch = Archetype::new(id, &descs);

        let entity = EntityId::from_raw(0, 0);
        let pos = Position { x: 1.0, y: 2.0 };
        let vel = Velocity { dx: 3.0, dy: 4.0 };

        // Components must be in sorted TypeId order.
        let mut components_by_type: Vec<(TypeId, *const u8)> = vec![
            (TypeId::of::<Position>(), &pos as *const _ as *const u8),
            (TypeId::of::<Velocity>(), &vel as *const _ as *const u8),
        ];
        components_by_type.sort_by_key(|(tid, _)| *tid);
        let ptrs: Vec<*const u8> = components_by_type.iter().map(|(_, p)| *p).collect();

        let row = unsafe { arch.push_entity(entity, &ptrs) };
        assert_eq!(row, 0);
        assert_eq!(arch.len(), 1);

        unsafe {
            let positions = arch.column_slice::<Position>().unwrap();
            assert_eq!(positions[0], pos);
            let velocities = arch.column_slice::<Velocity>().unwrap();
            assert_eq!(velocities[0], vel);
        }
    }

    #[test]
    fn archetype_swap_remove() {
        let descs = vec![pos_desc()];
        let sorted_types = vec![TypeId::of::<Position>()];
        let id = ArchetypeId::from_type_ids(&sorted_types);
        let mut arch = Archetype::new(id, &descs);

        let e0 = EntityId::from_raw(0, 0);
        let e1 = EntityId::from_raw(1, 0);
        let e2 = EntityId::from_raw(2, 0);

        let p0 = Position { x: 0.0, y: 0.0 };
        let p1 = Position { x: 1.0, y: 1.0 };
        let p2 = Position { x: 2.0, y: 2.0 };

        unsafe {
            arch.push_entity(e0, &[&p0 as *const _ as *const u8]);
            arch.push_entity(e1, &[&p1 as *const _ as *const u8]);
            arch.push_entity(e2, &[&p2 as *const _ as *const u8]);
        }

        // Remove middle element (row 1) — e2 should swap into row 1.
        let swapped = unsafe { arch.swap_remove(1) };
        assert_eq!(swapped, Some(e2));
        assert_eq!(arch.len(), 2);

        unsafe {
            let positions = arch.column_slice::<Position>().unwrap();
            assert_eq!(positions[0], p0);
            assert_eq!(positions[1], p2); // e2 swapped into row 1
        }
    }

    #[test]
    fn archetype_map_insert_and_query() {
        let mut map = ArchetypeMap::new();
        let descs = vec![pos_desc(), vel_desc(), health_desc()];
        let arch_id = map.get_or_create(&descs);

        let e0 = EntityId::from_raw(0, 0);
        let pos = Position { x: 10.0, y: 20.0 };
        let vel = Velocity { dx: 1.0, dy: 2.0 };
        let hp = Health(100);

        // Build component pointers in sorted TypeId order.
        let mut typed_ptrs: Vec<(TypeId, *const u8)> = vec![
            (TypeId::of::<Position>(), &pos as *const _ as *const u8),
            (TypeId::of::<Velocity>(), &vel as *const _ as *const u8),
            (TypeId::of::<Health>(), &hp as *const _ as *const u8),
        ];
        typed_ptrs.sort_by_key(|(tid, _)| *tid);
        let ptrs: Vec<*const u8> = typed_ptrs.iter().map(|(_, p)| *p).collect();

        unsafe { map.insert_entity(arch_id, e0, &ptrs) };

        let loc = map.entity_location(e0).unwrap();
        assert_eq!(loc.archetype_id, arch_id);
        assert_eq!(loc.row, 0);

        // Query matching archetypes.
        let required = vec![TypeId::of::<Position>(), TypeId::of::<Velocity>()];
        let matching: Vec<_> = map.matching_archetypes(&required).collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].len(), 1);
    }

    #[test]
    fn archetype_map_remove_entity() {
        let mut map = ArchetypeMap::new();
        let descs = vec![pos_desc()];
        let arch_id = map.get_or_create(&descs);

        let e0 = EntityId::from_raw(0, 0);
        let e1 = EntityId::from_raw(1, 0);
        let p0 = Position { x: 0.0, y: 0.0 };
        let p1 = Position { x: 1.0, y: 1.0 };

        unsafe {
            map.insert_entity(arch_id, e0, &[&p0 as *const _ as *const u8]);
            map.insert_entity(arch_id, e1, &[&p1 as *const _ as *const u8]);
        }

        unsafe { map.remove_entity(e0) };

        assert!(map.entity_location(e0).is_none());
        let loc1 = map.entity_location(e1).unwrap();
        // e1 was swapped into row 0
        assert_eq!(loc1.row, 0);
    }

    #[test]
    fn archetype_edge_caching() {
        let mut map = ArchetypeMap::new();
        let from = ArchetypeId::empty();
        let to = ArchetypeId::from_type_ids(&[TypeId::of::<Position>()]);

        assert!(map
            .lookup_add_edge(from, TypeId::of::<Position>())
            .is_none());
        map.cache_add_edge(from, TypeId::of::<Position>(), to);
        assert_eq!(
            map.lookup_add_edge(from, TypeId::of::<Position>()),
            Some(to)
        );
    }

    #[test]
    fn many_entities_linear_iteration() {
        let mut map = ArchetypeMap::new();
        let descs = vec![pos_desc(), vel_desc()];
        let arch_id = map.get_or_create(&descs);

        // Sort component pointers by TypeId.
        let pos_tid = TypeId::of::<Position>();
        let vel_tid = TypeId::of::<Velocity>();
        let pos_first = pos_tid < vel_tid;

        for i in 0..10_000u32 {
            let entity = EntityId::from_raw(i, 0);
            let pos = Position {
                x: i as f32,
                y: i as f32 * 2.0,
            };
            let vel = Velocity {
                dx: i as f32 * 0.1,
                dy: 0.0,
            };

            let ptrs = if pos_first {
                vec![&pos as *const _ as *const u8, &vel as *const _ as *const u8]
            } else {
                vec![&vel as *const _ as *const u8, &pos as *const _ as *const u8]
            };
            unsafe { map.insert_entity(arch_id, entity, &ptrs) };
        }

        let arch = map.get(arch_id).unwrap();
        assert_eq!(arch.len(), 10_000);

        // Linear iteration over positions.
        let positions = unsafe { arch.column_slice::<Position>().unwrap() };
        let velocities = unsafe { arch.column_slice::<Velocity>().unwrap() };
        assert_eq!(positions.len(), 10_000);
        assert_eq!(velocities.len(), 10_000);

        // Spot check.
        assert_eq!(positions[42].x, 42.0);
        assert!((velocities[42].dx - 4.2).abs() < 0.01);
    }
}
