# Example: Good Spec Excerpt (Inventory System)

This is an excerpt from a well-written spec. Note the specific signatures, documented edge cases, and clear behavior descriptions.

```yaml
---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---
```

## Purpose

Generic item and inventory system for Sandbox and RPG-style games. Provides an item registry for definitions, grid-based inventory with automatic stacking, equipment slots with type validation, and the data logic for containers (chests, shops, crafting output). Designed to be UI-agnostic: the engine handles data operations while the game layer renders the interface.

## Public API (excerpt)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    slots: Vec<InventorySlot>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self;

    /// Add an item, stacking with existing compatible items.
    /// Returns Some(remainder) if the inventory is full or stack limit is reached.
    pub fn add(&mut self, item: ItemInstance, registry: &ItemRegistry) -> Option<ItemInstance>;

    /// Remove item from a specific slot. Returns None if slot is empty.
    pub fn remove(&mut self, slot_index: usize) -> Option<ItemInstance>;

    /// Find first slot containing an item with the given def_id.
    pub fn find(&self, def_id: u32) -> Option<usize>;

    pub fn is_full(&self) -> bool;
    pub fn count(&self, def_id: u32) -> u32;
}
```

## Behavior (excerpt)

**Stacking:** When `add()` is called, the system first tries to stack with an existing item of the same `def_id`. Stacking respects `ItemDef.max_stack`. If the existing stack is full, it tries the next matching stack, then empty slots. If no space remains, the remainder is returned.

**Edge cases:**
- Items with `max_stack: 1` are never stacked — each occupies its own slot.
- `add()` on a full inventory returns the entire item as `Some(item)`.
- `remove()` with an out-of-bounds `slot_index` returns `None` (does not panic).
- `count()` sums across all stacks of the given `def_id`.

## Why This Is Good

1. **Complete signatures**: Every method has params, return type, and doc comment
2. **Edge cases documented**: Full inventory, max_stack: 1, out-of-bounds index
3. **Return values explicit**: "Returns Some(remainder)" not "returns the overflow"
4. **UI-agnostic stated upfront**: Clear boundary of what the module does NOT do
