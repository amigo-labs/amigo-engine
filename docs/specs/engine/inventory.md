---
status: draft
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Inventory & Items

## Purpose

Generic item and inventory system for Sandbox and RPG-style games. Provides an item registry for definitions, grid-based inventory with automatic stacking, equipment slots with type validation, and the data logic for containers (chests, shops, crafting output). Designed to be UI-agnostic: the engine handles data operations while the game layer renders the interface.

## Public API

Existing implementation in `crates/amigo_core/src/inventory.rs`. Item definitions come from `crates/amigo_core/src/loot.rs`.

### ItemRegistry

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ItemRegistry {
    items: FxHashMap<u32, ItemDef>,
}

impl ItemRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, def: ItemDef);
    pub fn get(&self, id: u32) -> Option<&ItemDef>;
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &ItemDef)>;
}
```

### InventorySlot

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventorySlot {
    pub item: Option<ItemInstance>,
}

impl InventorySlot {
    pub fn empty() -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn take(&mut self) -> Option<ItemInstance>;
}
```

### Inventory

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    slots: Vec<InventorySlot>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self;
    pub fn add(&mut self, item: ItemInstance, registry: &ItemRegistry) -> Option<ItemInstance>;
    pub fn remove(&mut self, index: usize) -> Option<ItemInstance>;
    pub fn remove_by_id(&mut self, def_id: u32, count: u32) -> u32;
    pub fn count(&self, def_id: u32) -> u32;
    pub fn has(&self, def_id: u32, count: u32) -> bool;
    pub fn slot(&self, index: usize) -> Option<&InventorySlot>;
    pub fn slot_mut(&mut self, index: usize) -> Option<&mut InventorySlot>;
    pub fn swap(&mut self, a: usize, b: usize);
    pub fn free_slots(&self) -> usize;
    pub fn is_full(&self) -> bool;
    pub fn iter_items(&self) -> impl Iterator<Item = (usize, &ItemInstance)>;
    pub fn slots(&self) -> &[InventorySlot];
}
```

### EquipSlot & Equipment

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    MainHand, OffHand, Head, Chest, Legs, Boots, Gloves, Ring1, Ring2, Amulet,
}

impl EquipSlot {
    pub fn all() -> &'static [EquipSlot];
    pub fn accepts(&self, item_type: ItemType) -> bool;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Equipment {
    slots: FxHashMap<EquipSlot, ItemInstance>,
}

impl Equipment {
    pub fn new() -> Self;
    pub fn equip(&mut self, slot: EquipSlot, item: ItemInstance) -> Option<ItemInstance>;
    pub fn unequip(&mut self, slot: EquipSlot) -> Option<ItemInstance>;
    pub fn get(&self, slot: EquipSlot) -> Option<&ItemInstance>;
    pub fn total_modifier(&self, stat: &str) -> f32;
    pub fn iter(&self) -> impl Iterator<Item = (&EquipSlot, &ItemInstance)>;
}
```

## Behavior

- **Stacking:** `add()` first attempts to merge into existing stacks of the same `def_id` up to `max_stack` (from `ItemDef`). Remaining items go into the first empty slot. If no space remains, the leftover `ItemInstance` is returned to the caller.
- **Remove by ID:** `remove_by_id()` drains items across multiple slots if needed, returning the actual count removed. Partially depleted stacks are kept.
- **Swap:** `swap()` exchanges two slots by index. Out-of-bounds indices are silently ignored.
- **Equipment validation:** `EquipSlot::accepts()` enforces which `ItemType` values can go in each slot (e.g., only `Weapon` in `MainHand`, only `Ring` in `Ring1`/`Ring2`).
- **Modifier aggregation:** `Equipment::total_modifier()` sums a named stat modifier across all equipped items for easy stat calculation.
- **Serialization:** Both `Inventory` and `Equipment` derive `Serialize`/`Deserialize` for [save/load](save-load.md) integration.

## Internal Design

- `Inventory` uses a fixed-size `Vec<InventorySlot>` initialized at construction. Capacity does not grow.
- `Equipment` uses `FxHashMap<EquipSlot, ItemInstance>` for sparse storage (most slots empty at any time).
- Item definitions (`ItemDef`) live in the `loot` module and are referenced by `def_id: u32`.

## Non-Goals

- **UI rendering.** Drag-and-drop, tooltips, and slot highlighting are game-layer responsibilities.
- **Item generation.** Loot tables and random item generation are in the `loot` module, not here.
- **Container entities.** Chests and shops use `Inventory` instances but their entity/component wiring is game-specific.

## Open Questions

- Should `Inventory` support dynamic resizing (e.g., backpack upgrades)?
- Should there be a `Container` trait abstracting over `Inventory`, `Equipment`, and future container types?
- How should item tooltips and description text be stored -- in `ItemDef` or in a separate localization table?
