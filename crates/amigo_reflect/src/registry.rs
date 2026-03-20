//! Type registry for collecting all reflected types.

use crate::{Reflect, TypeInfo};
use rustc_hash::FxHashMap;
use std::any::TypeId;

/// A registration entry for a single type.
pub struct TypeRegistration {
    /// Static type metadata.
    pub info: &'static TypeInfo,
    /// Create a default instance (requires `Default`).
    pub default_fn: Option<fn() -> Box<dyn Reflect>>,
}

/// Global registry mapping `TypeId` -> `TypeInfo` + factory functions.
pub struct TypeRegistry {
    by_type_id: FxHashMap<TypeId, TypeRegistration>,
    by_name: FxHashMap<&'static str, TypeId>,
    /// Insertion-ordered list of type IDs for deterministic iteration.
    order: Vec<TypeId>,
}

impl TypeRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            by_type_id: FxHashMap::default(),
            by_name: FxHashMap::default(),
            order: Vec::new(),
        }
    }

    /// Register a reflected type that also implements `Default`.
    /// The `default_fn` allows creating new instances (e.g., for adding components in the editor).
    pub fn register<T: Reflect + Default>(&mut self) {
        let info = T::type_info();
        let type_id = TypeId::of::<T>();

        if self.by_type_id.contains_key(&type_id) {
            return;
        }

        self.by_name.insert(info.short_name, type_id);
        self.order.push(type_id);
        self.by_type_id.insert(
            type_id,
            TypeRegistration {
                info,
                default_fn: Some(|| Box::new(T::default())),
            },
        );
    }

    /// Register a reflected type without requiring `Default` (no `default_fn`).
    pub fn register_no_default<T: Reflect>(&mut self) {
        let info = T::type_info();
        let type_id = TypeId::of::<T>();

        if self.by_type_id.contains_key(&type_id) {
            return;
        }

        self.by_name.insert(info.short_name, type_id);
        self.order.push(type_id);
        self.by_type_id.insert(
            type_id,
            TypeRegistration {
                info,
                default_fn: None,
            },
        );
    }

    /// Look up type info by `TypeId`.
    pub fn get(&self, type_id: TypeId) -> Option<&TypeRegistration> {
        self.by_type_id.get(&type_id)
    }

    /// Look up type info by short name (e.g. "Health").
    pub fn get_by_name(&self, name: &str) -> Option<&TypeRegistration> {
        let type_id = self.by_name.get(name)?;
        self.by_type_id.get(type_id)
    }

    /// Iterate over all registered types in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &TypeRegistration> {
        self.order
            .iter()
            .filter_map(move |id| self.by_type_id.get(id))
    }

    /// Number of registered types.
    pub fn len(&self) -> usize {
        self.by_type_id.len()
    }

    /// Returns `true` if no types have been registered.
    pub fn is_empty(&self) -> bool {
        self.by_type_id.is_empty()
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
