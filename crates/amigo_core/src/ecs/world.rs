use super::entity::{EntityId, GenerationalArena};
use super::sparse_set::SparseSet;
use crate::color::Color;
use crate::math::SimVec2;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};

// ── Core Components (statically typed for zero-overhead) ──

/// Position component (simulation space).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Position(pub SimVec2);

/// Velocity component (simulation space).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Velocity(pub SimVec2);

/// Health component.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

impl Health {
    pub fn new(max: i32) -> Self {
        Self { current: max, max }
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0
    }

    pub fn fraction(&self) -> f32 {
        if self.max == 0 {
            0.0
        } else {
            self.current as f32 / self.max as f32
        }
    }
}

/// Sprite component for rendering.
#[derive(Clone, Debug)]
pub struct SpriteComp {
    pub name: String,
    pub flip_x: bool,
    pub flip_y: bool,
    pub tint: Color,
    pub z_order: i32,
    pub visible: bool,
}

impl SpriteComp {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            flip_x: false,
            flip_y: false,
            tint: Color::WHITE,
            z_order: 0,
            visible: true,
        }
    }
}

/// Tag for state-scoped entity cleanup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StateScoped(pub u32);

/// Stored accessor function pointers for reflection on dynamic components.
#[cfg(feature = "reflect")]
#[derive(Clone)]
struct ReflectAccessor {
    get_fn: fn(&dyn AnyStorage, EntityId) -> Option<*const dyn amigo_reflect::Reflect>,
    get_mut_fn: fn(&mut dyn AnyStorage, EntityId) -> Option<*mut dyn amigo_reflect::Reflect>,
    contains_fn: fn(&dyn AnyStorage, EntityId) -> bool,
}

/// Trait for type-erased SparseSet storage.
trait AnyStorage: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove_entity(&mut self, id: EntityId);
    fn flush(&mut self);
    #[allow(dead_code)]
    fn len(&self) -> usize;
}

impl<T: 'static> AnyStorage for SparseSet<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn remove_entity(&mut self, id: EntityId) {
        self.remove(id);
    }

    fn flush(&mut self) {
        self.flush();
    }

    fn len(&self) -> usize {
        self.len()
    }
}

/// The World holds all entity and component data.
pub struct World {
    entities: GenerationalArena,

    // Core components (static, zero-overhead access)
    pub positions: SparseSet<Position>,
    pub velocities: SparseSet<Velocity>,
    pub healths: SparseSet<Health>,
    pub sprites: SparseSet<SpriteComp>,
    pub state_scoped: SparseSet<StateScoped>,

    // Dynamic components (game-specific)
    dynamic: FxHashMap<TypeId, Box<dyn AnyStorage>>,

    // Pending despawns (processed at flush)
    pending_despawn: Vec<EntityId>,

    // Reflection accessor functions for dynamic components (behind `reflect` feature)
    #[cfg(feature = "reflect")]
    reflect_accessors: FxHashMap<TypeId, ReflectAccessor>,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: GenerationalArena::new(),
            positions: SparseSet::new(),
            velocities: SparseSet::new(),
            healths: SparseSet::new(),
            sprites: SparseSet::new(),
            state_scoped: SparseSet::new(),
            dynamic: FxHashMap::default(),
            pending_despawn: Vec::new(),
            #[cfg(feature = "reflect")]
            reflect_accessors: FxHashMap::default(),
        }
    }

    /// Spawn a new entity.
    pub fn spawn(&mut self) -> EntityId {
        self.entities.spawn()
    }

    /// Queue an entity for despawn (processed at flush).
    pub fn despawn(&mut self, id: EntityId) {
        self.pending_despawn.push(id);
    }

    /// Check if an entity is alive.
    pub fn is_alive(&self, id: EntityId) -> bool {
        self.entities.is_alive(id)
    }

    /// Number of alive entities.
    pub fn entity_count(&self) -> usize {
        self.entities.count()
    }

    /// Iterate over all alive entity IDs.
    pub fn iter_entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.entities.iter_alive()
    }

    // ── Dynamic Component Access ──

    /// Register a dynamic component type with optional capacity hint.
    pub fn register_dynamic<T: 'static>(&mut self, capacity: usize) {
        let type_id = TypeId::of::<T>();
        self.dynamic
            .entry(type_id)
            .or_insert_with(|| Box::new(SparseSet::<T>::with_capacity(capacity)));
    }

    /// Get the SparseSet for a dynamic component type.
    pub fn dynamic<T: 'static>(&self) -> Option<&SparseSet<T>> {
        self.dynamic
            .get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref())
    }

    /// Get mutable SparseSet for a dynamic component type.
    pub fn dynamic_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        self.dynamic
            .get_mut(&TypeId::of::<T>())
            .and_then(|s| s.as_any_mut().downcast_mut())
    }

    /// Insert a dynamic component. Registers the type if not already registered.
    pub fn insert_dynamic<T: 'static>(&mut self, id: EntityId, data: T) {
        let type_id = TypeId::of::<T>();
        let storage = self
            .dynamic
            .entry(type_id)
            .or_insert_with(|| Box::new(SparseSet::<T>::new()));
        storage
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()
            .unwrap()
            .insert(id, data);
    }

    /// Get a dynamic component by reference.
    pub fn get_dynamic<T: 'static>(&self, id: EntityId) -> Option<&T> {
        self.dynamic::<T>()?.get(id)
    }

    /// Get a dynamic component by mutable reference (marks changed).
    pub fn get_dynamic_mut<T: 'static>(&mut self, id: EntityId) -> Option<&mut T> {
        self.dynamic_mut::<T>()?.get_mut(id)
    }

    // ── State-Scoped Cleanup ──

    /// Despawn all entities tagged with the given state.
    pub fn cleanup_state(&mut self, state: u32) {
        let to_despawn: Vec<EntityId> = self
            .state_scoped
            .iter()
            .filter(|(_, s)| s.0 == state)
            .map(|(id, _)| id)
            .collect();

        for id in to_despawn {
            self.despawn(id);
        }
    }

    // ── End of Tick ──

    /// Process pending despawns and clear change tracking.
    pub fn flush(&mut self) {
        // Process despawns
        let despawns = std::mem::take(&mut self.pending_despawn);
        for id in &despawns {
            self.positions.remove(*id);
            self.velocities.remove(*id);
            self.healths.remove(*id);
            self.sprites.remove(*id);
            self.state_scoped.remove(*id);

            for storage in self.dynamic.values_mut() {
                storage.remove_entity(*id);
            }

            self.entities.despawn(*id);
        }

        // Clear change tracking
        self.positions.flush();
        self.velocities.flush();
        self.healths.flush();
        self.sprites.flush();
        self.state_scoped.flush();

        for storage in self.dynamic.values_mut() {
            storage.flush();
        }
    }
}

impl World {
    // ── Generic Component Access ──

    /// Add a component to an entity using the `Component` trait for static routing.
    pub fn add<T: super::query::Component>(&mut self, id: EntityId, data: T) {
        T::storage_mut(self).insert(id, data);
    }

    /// Get a component reference via the `Component` trait.
    pub fn get<T: super::query::Component>(&self, id: EntityId) -> Option<&T> {
        T::storage(self).get(id)
    }

    /// Get a mutable component reference via the `Component` trait. Marks as changed.
    pub fn get_mut_comp<T: super::query::Component>(&mut self, id: EntityId) -> Option<&mut T> {
        T::storage_mut(self).get_mut(id)
    }

    /// Remove a component via the `Component` trait.
    pub fn remove_comp<T: super::query::Component>(&mut self, id: EntityId) -> Option<T> {
        T::storage_mut(self).remove(id)
    }

    /// Check if an entity has a component via the `Component` trait.
    pub fn has<T: super::query::Component>(&self, id: EntityId) -> bool {
        T::storage(self).contains(id)
    }

    /// Get the SparseSet for a component type.
    pub fn storage<T: super::query::Component>(&self) -> &SparseSet<T> {
        T::storage(self)
    }

    /// Get the mutable SparseSet for a component type.
    pub fn storage_mut<T: super::query::Component>(&mut self) -> &mut SparseSet<T> {
        T::storage_mut(self)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

// ── Reflection Integration (behind `reflect` feature) ──

#[cfg(feature = "reflect")]
mod reflect_impl {
    use super::*;
    use amigo_reflect::{Reflect, TypeRegistry};

    /// Helper: try to get a `&dyn Reflect` from a `&dyn AnyStorage` for a given entity,
    /// knowing the concrete type `T` at registration time.
    fn get_reflect_from_storage<T: Reflect + 'static>(
        storage: &dyn AnyStorage,
        entity: EntityId,
    ) -> Option<*const dyn Reflect> {
        let sparse = storage.as_any().downcast_ref::<SparseSet<T>>()?;
        let val = sparse.get(entity)?;
        Some(val as &dyn Reflect as *const dyn Reflect)
    }

    /// Helper: try to get a `&mut dyn Reflect` from a `&mut dyn AnyStorage` for a given entity.
    fn get_reflect_mut_from_storage<T: Reflect + 'static>(
        storage: &mut dyn AnyStorage,
        entity: EntityId,
    ) -> Option<*mut dyn Reflect> {
        let sparse = storage.as_any_mut().downcast_mut::<SparseSet<T>>()?;
        let val = sparse.get_mut(entity)?;
        Some(val as &mut dyn Reflect as *mut dyn Reflect)
    }

    impl World {
        /// Get a type-erased reflected reference to a component on an entity.
        /// Returns `None` if the entity does not have the component or if the type
        /// is not registered in the provided `TypeRegistry`.
        pub fn get_reflected(
            &self,
            entity: EntityId,
            type_id: TypeId,
            registry: &TypeRegistry,
        ) -> Option<&dyn Reflect> {
            // Check that the type is registered
            let _reg = registry.get(type_id)?;

            // Check built-in sparse sets
            if type_id == TypeId::of::<Health>() {
                return self.healths.get(entity).map(|v| v as &dyn Reflect);
            }

            // Check dynamic components -- we need the concrete type to downcast
            // Since we don't know the concrete type at this point, we iterate
            // through dynamic storage and attempt downcast via the registry.
            if let Some(storage) = self.dynamic.get(&type_id) {
                // We need to try downcasting. The only way to do this generically
                // is to use the as_any() + downcast pattern, but we need to know T.
                // For now, use the AnyStorage::as_any() and try known types.
                // This is a limitation -- dynamic reflection requires a registration
                // step that stores accessor functions.
                //
                // Use a helper stored at registration time via register_reflected.
                if let Some(accessor) = self.reflect_accessors.get(&type_id) {
                    // SAFETY: The accessor returns a pointer that borrows from storage,
                    // which itself borrows from self. We return a reference with
                    // the lifetime of self.
                    unsafe {
                        let ptr = (accessor.get_fn)(storage.as_ref(), entity)?;
                        return Some(&*ptr);
                    }
                }
            }

            None
        }

        /// Get a mutable reflected reference to a component on an entity.
        pub fn get_reflected_mut(
            &mut self,
            entity: EntityId,
            type_id: TypeId,
            registry: &TypeRegistry,
        ) -> Option<&mut dyn Reflect> {
            let _reg = registry.get(type_id)?;

            if type_id == TypeId::of::<Health>() {
                return self.healths.get_mut(entity).map(|v| v as &mut dyn Reflect);
            }

            // For dynamic components, use stored accessor
            if let Some(storage) = self.dynamic.get_mut(&type_id) {
                let accessor = self.reflect_accessors.get(&type_id).cloned();
                if let Some(accessor) = accessor {
                    // SAFETY: The accessor returns a pointer that borrows from storage,
                    // which borrows from self. We return a &mut with the lifetime of self.
                    unsafe {
                        let ptr = (accessor.get_mut_fn)(storage.as_mut(), entity)?;
                        return Some(&mut *ptr);
                    }
                }
            }

            None
        }

        /// List all component TypeIds present on an entity.
        /// Includes both built-in (static) and dynamic components.
        pub fn component_types(&self, entity: EntityId) -> Vec<TypeId> {
            let mut types = Vec::new();

            if self.positions.contains(entity) {
                types.push(TypeId::of::<Position>());
            }
            if self.velocities.contains(entity) {
                types.push(TypeId::of::<Velocity>());
            }
            if self.healths.contains(entity) {
                types.push(TypeId::of::<Health>());
            }
            if self.sprites.contains(entity) {
                types.push(TypeId::of::<SpriteComp>());
            }
            if self.state_scoped.contains(entity) {
                types.push(TypeId::of::<StateScoped>());
            }

            for (type_id, storage) in &self.dynamic {
                // Check if the entity has this component by trying to access it
                // We need a contains method on AnyStorage
                if storage.as_any().downcast_ref::<SparseSet<()>>().is_none() {
                    // Can't check directly without knowing the type, but we stored
                    // it as a SparseSet<T>. Use the reflect_accessors to check.
                    if let Some(accessor) = self.reflect_accessors.get(type_id) {
                        if (accessor.contains_fn)(storage.as_ref(), entity) {
                            types.push(*type_id);
                        }
                    } else {
                        // No accessor -- can't inspect. Skip.
                    }
                }
            }

            types
        }

        /// Register a dynamic component type for reflection.
        /// This stores accessor functions that allow `get_reflected` and `get_reflected_mut`
        /// to work with this component type.
        pub fn register_reflected<T: Reflect + 'static>(&mut self, capacity: usize) {
            let type_id = TypeId::of::<T>();
            self.dynamic
                .entry(type_id)
                .or_insert_with(|| Box::new(SparseSet::<T>::with_capacity(capacity)));
            self.reflect_accessors.insert(
                type_id,
                ReflectAccessor {
                    get_fn: |storage, entity| get_reflect_from_storage::<T>(storage, entity),
                    get_mut_fn: |storage, entity| get_reflect_mut_from_storage::<T>(storage, entity),
                    contains_fn: |storage, entity| {
                        storage
                            .as_any()
                            .downcast_ref::<SparseSet<T>>()
                            .is_some_and(|s| s.contains(entity))
                    },
                },
            );
        }
    }

    /// Stored accessor function pointers for reflection on dynamic components.
    #[derive(Clone)]
    pub(super) struct ReflectAccessor {
        pub get_fn: fn(&dyn AnyStorage, EntityId) -> Option<*const dyn Reflect>,
        pub get_mut_fn: fn(&mut dyn AnyStorage, EntityId) -> Option<*mut dyn Reflect>,
        pub contains_fn: fn(&dyn AnyStorage, EntityId) -> bool,
    }
}

// Add the reflect_accessors field to World when the reflect feature is on

