---
status: spec
crate: amigo_reflect
depends_on: []
last_updated: 2026-03-20
---

# Reflection System

## Purpose

The reflection system provides runtime type introspection for ECS components and other game types. Today the editor (`crates/amigo_editor/src/egui_ui.rs`) must manually build UI for each component type -- the `draw_properties_panel` function (line 89) only shows level-level metadata (width, height, layer count) and has no ability to inspect or edit individual component fields on a selected entity. Similarly, the `World` struct (`crates/amigo_core/src/ecs/world.rs`, line 104) stores dynamic components behind `Box<dyn AnyStorage>` (line 115) which provides `as_any()`/`as_any_mut()` for downcasting but exposes no field names, types, or ranges.

The reflection system solves this by providing a `#[derive(Reflect)]` proc-macro that generates field metadata at compile time, a `Reflect` trait for runtime inspection and mutation, and a `TypeRegistry` that collects all reflected types for editor and serialization tools. This is gated behind the `reflect` feature flag.

## Public API

### Core trait and types (`amigo_reflect/src/lib.rs`)

```rust
/// Describes a single field on a reflected struct.
#[derive(Clone, Debug)]
pub struct FieldInfo {
    /// Field name as written in source code.
    pub name: &'static str,
    /// Fully qualified type name (e.g. "f32", "amigo_core::color::Color").
    pub type_name: &'static str,
    /// `std::any::TypeId` of the field type.
    pub type_id: std::any::TypeId,
    /// Byte offset from the start of the parent struct (for unsafe direct access).
    /// Only used internally; external consumers should use `field()` / `field_mut()`.
    pub offset: usize,
    /// Optional UI hints from #[reflect(...)] attributes.
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
    pub type_id: std::any::TypeId,
    /// Fields in declaration order.
    pub fields: &'static [FieldInfo],
}

/// A type-erased reference to a single field value.
/// Wraps `&dyn Any` with the field's metadata for convenience.
pub struct FieldRef<'a> {
    pub info: &'static FieldInfo,
    pub value: &'a dyn std::any::Any,
}

/// A type-erased mutable reference to a single field value.
pub struct FieldMut<'a> {
    pub info: &'static FieldInfo,
    pub value: &'a mut dyn std::any::Any,
}

/// Runtime reflection trait. Derive with `#[derive(Reflect)]`.
pub trait Reflect: std::any::Any + 'static {
    /// Static type metadata.
    fn type_info() -> &'static TypeInfo where Self: Sized;

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
    fields: Vec<(String, Box<dyn std::any::Any>)>,
}

impl ReflectPatch {
    pub fn new() -> Self;
    pub fn set<T: std::any::Any + Clone + 'static>(&mut self, name: &str, value: T);
    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn std::any::Any)>;
}
```

### TypeRegistry (`amigo_reflect/src/registry.rs`)

```rust
/// Global registry mapping TypeId -> TypeInfo + factory functions.
pub struct TypeRegistry {
    // ...
}

/// A registration entry for a single type.
pub struct TypeRegistration {
    pub info: &'static TypeInfo,
    /// Create a default instance (requires `Default`).
    pub default_fn: Option<fn() -> Box<dyn Reflect>>,
}

impl TypeRegistry {
    pub fn new() -> Self;

    /// Register a reflected type. Typically called via `registry.register::<Health>()`.
    pub fn register<T: Reflect + Default>(&mut self);

    /// Register without requiring Default (no default_fn).
    pub fn register_no_default<T: Reflect>(&mut self);

    /// Look up type info by TypeId.
    pub fn get(&self, type_id: std::any::TypeId) -> Option<&TypeRegistration>;

    /// Look up type info by short name (e.g. "Health").
    pub fn get_by_name(&self, name: &str) -> Option<&TypeRegistration>;

    /// Iterate over all registered types.
    pub fn iter(&self) -> impl Iterator<Item = &TypeRegistration>;

    /// Number of registered types.
    pub fn len(&self) -> usize;

    pub fn is_empty(&self) -> bool;
}
```

### Derive macro (`amigo_reflect_derive/src/lib.rs`)

```rust
/// Derive macro that implements `Reflect` for a struct.
///
/// Supports named-field structs only. Tuple structs and enums are non-goals (v1).
///
/// # Field attributes
///
/// - `#[reflect(skip)]` — exclude from reflection
/// - `#[reflect(read_only)]` — visible but not editable in inspector
/// - `#[reflect(label = "Display Name")]` — custom display label
/// - `#[reflect(range = 0.0..=100.0)]` — numeric range hint for sliders
///
/// # Example
///
/// ```ignore
/// #[derive(Reflect)]
/// struct Health {
///     #[reflect(range = 0.0..=1000.0)]
///     pub current: i32,
///     #[reflect(read_only)]
///     pub max: i32,
/// }
/// ```
#[proc_macro_derive(Reflect, attributes(reflect))]
pub fn derive_reflect(input: TokenStream) -> TokenStream;
```

### World integration (`amigo_core::ecs::world`, behind `#[cfg(feature = "reflect")]`)

```rust
impl World {
    /// Get a type-erased reflected reference to a dynamic component on an entity.
    /// Returns `None` if the entity does not have the component or if the type
    /// is not registered in the provided TypeRegistry.
    pub fn get_reflected(
        &self,
        entity: EntityId,
        type_id: std::any::TypeId,
        registry: &amigo_reflect::TypeRegistry,
    ) -> Option<&dyn amigo_reflect::Reflect>;

    /// Get a mutable reflected reference to a dynamic component.
    pub fn get_reflected_mut(
        &mut self,
        entity: EntityId,
        type_id: std::any::TypeId,
        registry: &amigo_reflect::TypeRegistry,
    ) -> Option<&mut dyn amigo_reflect::Reflect>;

    /// List all component TypeIds present on an entity.
    /// Includes both built-in (static) and dynamic components.
    pub fn component_types(&self, entity: EntityId) -> Vec<std::any::TypeId>;
}
```

## Behavior

### Normal Flow

1. At startup, the game or editor registers reflected types:
   ```rust
   let mut registry = TypeRegistry::new();
   registry.register::<Position>();
   registry.register::<Health>();
   registry.register::<SpriteComp>();
   ```
2. The derive macro generates `Reflect` implementations at compile time. Each generated impl constructs a `static TypeInfo` with field metadata computed via `std::mem::offset_of!` (stabilized in Rust 1.77).
3. The editor iterates `world.component_types(entity)` for the selected entity, looks up each `TypeId` in the `TypeRegistry`, and calls `world.get_reflected(entity, type_id, &registry)` to obtain a `&dyn Reflect`. It then calls `reflected.fields()` to iterate and render UI widgets per field.
4. When the user edits a field, the editor calls `world.get_reflected_mut()` then `reflected.field_mut("current")` to obtain a `FieldMut`, downcasts `value` to `&mut i32`, and writes the new value.

### Edge Cases

- **Unknown field name**: `field("nonexistent")` returns `None`. Callers must handle gracefully.
- **Type mismatch on downcast**: If `FieldMut::value.downcast_mut::<T>()` returns `None`, the edit is silently ignored. The editor should log a warning.
- **`#[reflect(skip)]` fields**: Not included in `fields()` / `fields_mut()` iteration, and `field("skipped_name")` returns `None`.
- **Zero-sized types**: `FieldInfo::offset` is valid but the `Any` reference points to a ZST. Reads succeed; writes are no-ops.
- **Unregistered type in `get_reflected`**: Returns `None`. The editor displays "unregistered component" in the inspector.
- **`apply_patch` partial failure**: If a patch contains 3 fields but only 2 names match, those 2 are applied and the return value is `2`. The unmatched field is ignored.

### Ordering Guarantees

- `fields()` and `fields_mut()` always return fields in source declaration order.
- `TypeRegistry::iter()` returns types in registration order.

## Internal Design

### Proc-macro crate (`amigo_reflect_derive`)

A separate crate is required because Rust proc-macros must be compiled as `proc-macro = true` dylibs. The crate parses the input struct with `syn`, iterates named fields, extracts `#[reflect(...)]` attributes, and generates:

1. A `static FIELDS: [FieldInfo; N]` array using `std::mem::offset_of!(StructName, field_name)` for each non-skipped field.
2. A `static TYPE_INFO: TypeInfo` referencing `FIELDS`.
3. The `Reflect` trait implementation where `field(name)` is a `match name { "x" => ..., "y" => ..., _ => None }` chain. For structs with more than 12 fields, a `phf` perfect-hash map could be generated instead, but the match-chain is sufficient for typical ECS components (2-8 fields).

### TypeRegistry internals

- Backed by `rustc_hash::FxHashMap<TypeId, TypeRegistration>` for O(1) lookup by `TypeId`.
- A secondary `FxHashMap<&'static str, TypeId>` for name-based lookup.
- `register::<T>()` stores `default_fn: Some(|| Box::new(T::default()))` which allows the editor to spawn components of a given type.

### World integration

`get_reflected` works with both built-in and dynamic components:
1. Check each built-in SparseSet (`positions`, `healths`, etc.) for the given `TypeId`. If the component is found and registered, return a `&dyn Reflect` trait object (requires that built-in components derive `Reflect`).
2. Fall back to `self.dynamic.get(&type_id)` and downcast the `AnyStorage` to the concrete `SparseSet<T>`. Since the concrete type `T` is erased, this requires a stored downcast function pointer in the registry entry (a `fn(&dyn AnyStorage, EntityId) -> Option<&dyn Reflect>`). This function pointer is generated at `register()` time via a generic closure.

### Performance

- `Reflect::field()` is a match-chain on string literals -- not performance-critical since it is only called in editor code, not simulation.
- `fields()` allocates a `Vec<FieldRef>` per call. For the editor inspector running at 60fps with 5 components of 4 fields each, this is 5 small allocations per frame -- negligible.
- The derive macro adds zero runtime cost for game builds that do not enable the `reflect` feature.

## Non-Goals

- **Enum reflection**: v1 only supports named-field structs. Enums can be added later with a `ReflectEnum` trait if needed.
- **Tuple struct reflection**: Tuple structs (e.g., `Position(SimVec2)`) would require unnamed field access. These can implement `Reflect` manually for v1.
- **Serialization via reflection**: The reflection system is for inspection and editing, not for save/load. Serialization continues to use `serde`. A future `ReflectSerialize` adapter could bridge the two.
- **Automatic registration**: Types are not auto-registered by the derive macro. The game must call `registry.register::<T>()` explicitly for each type it wants inspectable. Inventory/linkme-based auto-registration is a v2 concern.
- **Nested struct drilling**: v1 exposes fields as flat `&dyn Any`. The editor renders leaf fields (i32, f32, bool, String, Color) directly. For composite fields like `SimVec2`, the editor can check if the field type itself implements `Reflect` and recurse, but this is editor logic, not part of the `Reflect` trait contract.

## Open Questions

- **Should `Reflect` require `Clone`?** `clone_reflect()` requires cloning for undo snapshots. This is already satisfied by most components (`Position`, `Health`, `SpriteComp` all derive `Clone`). Proposal: yes, require `Clone` as a supertrait.
- **Should the built-in components (`Position`, `Velocity`, etc.) derive `Reflect` directly, or should we provide manual impls?** `Position(pub SimVec2)` is a tuple struct, which the derive macro does not support in v1. Manual impls for the 5 built-in types are low effort.
- **Interaction with archetype storage (ADR-0001)**: If archetypes land first, `get_reflected` must also probe archetype columns. The `AnyColumn` trait from ADR-0001 would need a similar downcast-function-pointer mechanism. Resolve: implement reflection against the current SparseSet storage; archetype support is additive.
