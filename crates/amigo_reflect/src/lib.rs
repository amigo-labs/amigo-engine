//! Runtime reflection system for the Amigo engine.
//!
//! Provides the [`Reflect`] trait for runtime type introspection, [`TypeInfo`] and [`FieldInfo`]
//! for structural metadata, and [`TypeRegistry`] for collecting all reflected types.
//!
//! Use `#[derive(Reflect)]` from `amigo_reflect_derive` to auto-implement reflection for
//! named-field structs.

// Allow the derive macro to reference `amigo_reflect::` types when used within this crate.
extern crate self as amigo_reflect;

mod registry;

pub use registry::{TypeRegistration, TypeRegistry};

// Re-export derive macro so users can write `use amigo_reflect::Reflect;`
pub use amigo_reflect_derive::Reflect;

use std::any::{Any, TypeId};

/// Describes a single field on a reflected struct.
#[derive(Clone, Debug)]
pub struct FieldInfo {
    /// Field name as written in source code.
    pub name: &'static str,
    /// Fully qualified type name (e.g. "f32", "amigo_core::color::Color").
    pub type_name: &'static str,
    /// `std::any::TypeId` of the field type.
    pub type_id: TypeId,
    /// Byte offset from the start of the parent struct (for unsafe direct access).
    /// Only used internally; external consumers should use `field()` / `field_mut()`.
    pub offset: usize,
    /// Optional UI hints from `#[reflect(...)]` attributes.
    pub attrs: FieldAttrs,
}

/// Optional per-field attributes for editor/UI hints.
#[derive(Clone, Debug, Default)]
pub struct FieldAttrs {
    /// If set, display this label instead of the field name.
    pub label: Option<&'static str>,
    /// Numeric range hint: `#[reflect(range = 0.0..=1.0)]`.
    pub range: Option<(f64, f64)>,
    /// If true, field is read-only in the editor: `#[reflect(read_only)]`.
    pub read_only: bool,
    /// If true, field is hidden from the editor: `#[reflect(skip)]`.
    pub skip: bool,
}

/// Type-level metadata for a reflected type.
#[derive(Clone, Debug)]
pub struct TypeInfo {
    /// Short type name (e.g. "Health").
    pub short_name: &'static str,
    /// Fully qualified type path (e.g. "amigo_core::ecs::world::Health").
    pub type_path: &'static str,
    /// `std::any::TypeId` of this type.
    pub type_id: TypeId,
    /// Fields in declaration order.
    pub fields: &'static [FieldInfo],
}

/// A type-erased reference to a single field value.
/// Wraps `&dyn Any` with the field's metadata for convenience.
pub struct FieldRef<'a> {
    pub info: &'static FieldInfo,
    pub value: &'a dyn Any,
}

/// A type-erased mutable reference to a single field value.
pub struct FieldMut<'a> {
    pub info: &'static FieldInfo,
    pub value: &'a mut dyn Any,
}

/// Runtime reflection trait. Derive with `#[derive(Reflect)]`.
pub trait Reflect: Any + 'static {
    /// Static type metadata.
    fn type_info() -> &'static TypeInfo
    where
        Self: Sized;

    /// Instance-level type info (for trait objects).
    fn reflected_type_info(&self) -> &'static TypeInfo;

    /// Get a field reference by name. Returns `None` if the name is unknown.
    fn field(&self, name: &str) -> Option<FieldRef<'_>>;

    /// Get a mutable field reference by name. Returns `None` if the name is unknown.
    fn field_mut(&mut self, name: &str) -> Option<FieldMut<'_>>;

    /// Iterate over all fields.
    fn fields(&self) -> Vec<FieldRef<'_>>;

    /// Iterate over all fields mutably.
    fn fields_mut(&mut self) -> Vec<FieldMut<'_>>;

    /// Apply a patch: set all fields that match keys in the map.
    /// Returns the number of fields successfully patched.
    fn apply_patch(&mut self, patch: &ReflectPatch) -> usize;

    /// Clone this value into a `Box<dyn Reflect>`.
    fn clone_reflect(&self) -> Box<dyn Reflect>;
}

/// A key-value patch map for bulk field updates (used by undo/redo).
/// Values are type-erased via `Box<dyn Any>`.
pub struct ReflectPatch {
    fields: Vec<(String, Box<dyn Any>)>,
}

impl ReflectPatch {
    /// Create an empty patch.
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Set a field value in the patch.
    pub fn set<T: Any + Clone + 'static>(&mut self, name: &str, value: T) {
        self.fields.push((name.to_owned(), Box::new(value)));
    }

    /// Iterate over the patch entries.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn Any)> {
        self.fields
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_ref()))
    }
}

impl Default for ReflectPatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, Reflect)]
    struct Health {
        #[reflect(range = 0.0..=1000.0)]
        pub current: i32,
        #[reflect(read_only)]
        pub max: i32,
    }

    #[derive(Clone, Debug, Default, Reflect)]
    struct Stats {
        #[reflect(label = "Player Name")]
        pub name: String,
        pub speed: f32,
        #[reflect(skip)]
        pub internal_id: u64,
        pub active: bool,
    }

    #[test]
    fn type_info_has_correct_name() {
        let info = Health::type_info();
        assert_eq!(info.short_name, "Health");
        assert_eq!(info.type_id, TypeId::of::<Health>());
    }

    #[test]
    fn field_count_excludes_skipped() {
        let info = Stats::type_info();
        // internal_id is skipped, so 3 fields: name, speed, active
        assert_eq!(info.fields.len(), 3);
        assert_eq!(info.fields[0].name, "name");
        assert_eq!(info.fields[1].name, "speed");
        assert_eq!(info.fields[2].name, "active");
    }

    #[test]
    fn field_attrs_range() {
        let info = Health::type_info();
        let current_field = &info.fields[0];
        assert_eq!(current_field.name, "current");
        let (lo, hi) = current_field.attrs.range.expect("should have range");
        assert!((lo - 0.0).abs() < f64::EPSILON);
        assert!((hi - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn field_attrs_read_only() {
        let info = Health::type_info();
        let max_field = &info.fields[1];
        assert_eq!(max_field.name, "max");
        assert!(max_field.attrs.read_only);
    }

    #[test]
    fn field_attrs_label() {
        let info = Stats::type_info();
        let name_field = &info.fields[0];
        assert_eq!(name_field.attrs.label, Some("Player Name"));
    }

    #[test]
    fn field_by_name() {
        let h = Health { current: 50, max: 100 };
        let field_ref = h.field("current").expect("should find current");
        assert_eq!(field_ref.info.name, "current");
        let val = field_ref.value.downcast_ref::<i32>().expect("should be i32");
        assert_eq!(*val, 50);
    }

    #[test]
    fn field_by_name_unknown_returns_none() {
        let h = Health { current: 50, max: 100 };
        assert!(h.field("nonexistent").is_none());
    }

    #[test]
    fn field_by_name_skipped_returns_none() {
        let s = Stats {
            name: "test".into(),
            speed: 1.0,
            internal_id: 42,
            active: true,
        };
        assert!(s.field("internal_id").is_none());
    }

    #[test]
    fn field_mut_by_name() {
        let mut h = Health { current: 50, max: 100 };
        {
            let field_mut = h.field_mut("current").expect("should find current");
            let val = field_mut.value.downcast_mut::<i32>().expect("should be i32");
            *val = 75;
        }
        assert_eq!(h.current, 75);
    }

    #[test]
    fn field_mut_type_mismatch() {
        let mut h = Health { current: 50, max: 100 };
        let field_mut = h.field_mut("current").expect("should find current");
        // Try to downcast to wrong type
        assert!(field_mut.value.downcast_mut::<f32>().is_none());
    }

    #[test]
    fn fields_returns_all_non_skipped_in_order() {
        let h = Health { current: 10, max: 20 };
        let fields = h.fields();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].info.name, "current");
        assert_eq!(fields[1].info.name, "max");
    }

    #[test]
    fn fields_mut_returns_all_non_skipped_in_order() {
        let mut s = Stats {
            name: "alice".into(),
            speed: 5.0,
            internal_id: 99,
            active: false,
        };
        let fields = s.fields_mut();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].info.name, "name");
        assert_eq!(fields[1].info.name, "speed");
        assert_eq!(fields[2].info.name, "active");
    }

    #[test]
    fn apply_patch_updates_matching_fields() {
        let mut h = Health { current: 50, max: 100 };
        let mut patch = ReflectPatch::new();
        patch.set("current", 75i32);
        let count = h.apply_patch(&patch);
        assert_eq!(count, 1);
        assert_eq!(h.current, 75);
    }

    #[test]
    fn apply_patch_skips_read_only() {
        let mut h = Health { current: 50, max: 100 };
        let mut patch = ReflectPatch::new();
        patch.set("max", 200i32);
        let count = h.apply_patch(&patch);
        // max is read_only, so should not be patched
        assert_eq!(count, 0);
        assert_eq!(h.max, 100);
    }

    #[test]
    fn apply_patch_partial_match() {
        let mut h = Health { current: 50, max: 100 };
        let mut patch = ReflectPatch::new();
        patch.set("current", 75i32);
        patch.set("nonexistent", 42i32);
        patch.set("another_missing", true);
        let count = h.apply_patch(&patch);
        assert_eq!(count, 1);
        assert_eq!(h.current, 75);
    }

    #[test]
    fn apply_patch_no_match_returns_zero() {
        let mut h = Health { current: 50, max: 100 };
        let mut patch = ReflectPatch::new();
        patch.set("foo", 1i32);
        patch.set("bar", 2i32);
        let count = h.apply_patch(&patch);
        assert_eq!(count, 0);
    }

    #[test]
    fn clone_reflect_creates_independent_copy() {
        let h = Health { current: 50, max: 100 };
        let cloned = h.clone_reflect();
        let info = cloned.reflected_type_info();
        assert_eq!(info.short_name, "Health");

        let current_ref = cloned.field("current").expect("should find current");
        let val = current_ref.value.downcast_ref::<i32>().expect("should be i32");
        assert_eq!(*val, 50);
    }

    #[test]
    fn reflected_type_info_matches_type_info() {
        let h = Health { current: 10, max: 20 };
        let info_static = Health::type_info();
        let info_instance = h.reflected_type_info();
        assert_eq!(info_static.short_name, info_instance.short_name);
        assert_eq!(info_static.type_id, info_instance.type_id);
    }

    #[test]
    fn registry_register_and_get() {
        let mut registry = TypeRegistry::new();
        registry.register::<Health>();
        let reg = registry.get(TypeId::of::<Health>()).expect("should be registered");
        assert_eq!(reg.info.short_name, "Health");
        assert!(reg.default_fn.is_some());
    }

    #[test]
    fn registry_get_by_name() {
        let mut registry = TypeRegistry::new();
        registry.register::<Health>();
        let reg = registry.get_by_name("Health").expect("should find by name");
        assert_eq!(reg.info.short_name, "Health");
    }

    #[test]
    fn registry_get_missing_returns_none() {
        let registry = TypeRegistry::new();
        assert!(registry.get(TypeId::of::<Health>()).is_none());
        assert!(registry.get_by_name("Health").is_none());
    }

    #[test]
    fn registry_register_no_default() {
        #[derive(Clone, Reflect)]
        struct NoDefault {
            pub value: i32,
        }
        let mut registry = TypeRegistry::new();
        registry.register_no_default::<NoDefault>();
        let reg = registry.get(TypeId::of::<NoDefault>()).expect("should be registered");
        assert!(reg.default_fn.is_none());
    }

    #[test]
    fn registry_iter_in_registration_order() {
        let mut registry = TypeRegistry::new();
        registry.register::<Health>();
        registry.register::<Stats>();
        let names: Vec<&str> = registry.iter().map(|r| r.info.short_name).collect();
        assert_eq!(names, vec!["Health", "Stats"]);
    }

    #[test]
    fn registry_len_and_is_empty() {
        let mut registry = TypeRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        registry.register::<Health>();
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn registry_default_fn_creates_instance() {
        let mut registry = TypeRegistry::new();
        registry.register::<Health>();
        let reg = registry.get(TypeId::of::<Health>()).unwrap();
        let default_fn = reg.default_fn.unwrap();
        let instance = default_fn();
        let info = instance.reflected_type_info();
        assert_eq!(info.short_name, "Health");
    }

    #[test]
    fn registry_duplicate_register_is_idempotent() {
        let mut registry = TypeRegistry::new();
        registry.register::<Health>();
        registry.register::<Health>();
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn reflect_patch_iter() {
        let mut patch = ReflectPatch::new();
        patch.set("a", 1i32);
        patch.set("b", 2.0f64);
        let entries: Vec<_> = patch.iter().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "a");
        assert_eq!(entries[1].0, "b");
    }
}
