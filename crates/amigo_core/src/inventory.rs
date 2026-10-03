use crate::loot::{ItemDef, ItemInstance, ItemType};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Item Registry
// ---------------------------------------------------------------------------

/// Central registry of all item definitions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ItemRegistry {
    items: FxHashMap<u32, ItemDef>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        Self {
            items: FxHashMap::default(),
        }
    }

    pub fn register(&mut self, def: ItemDef) {
        self.items.insert(def.id, def);
    }

    pub fn get(&self, id: u32) -> Option<&ItemDef> {
        self.items.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u32, &ItemDef)> {
        self.items.iter()
    }
}

// ---------------------------------------------------------------------------
// Inventory Slot
// ---------------------------------------------------------------------------

/// A single inventory slot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventorySlot {
    pub item: Option<ItemInstance>,
}

impl InventorySlot {
    pub fn empty() -> Self {
        Self { item: None }
    }

    pub fn is_empty(&self) -> bool {
        self.item.is_none()
    }

    pub fn take(&mut self) -> Option<ItemInstance> {
        self.item.take()
    }
}

impl Default for InventorySlot {
    fn default() -> Self {
        Self::empty()
    }
}

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

/// Grid-based inventory (like Diablo).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    slots: Vec<InventorySlot>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        slots.resize_with(capacity, InventorySlot::empty);
        Self { slots, capacity }
    }

    /// Try to add an item, stacking if possible. Returns leftover if inventory full.
    ///
    /// Existing stacks of the same item are topped up first; the rest goes
    /// into empty slots, at most `max_stack` per slot, so an incoming stack
    /// larger than `max_stack` (a 150-gold drop with `max_stack = 99`) is
    /// split rather than stored over the limit.
    pub fn add(&mut self, mut item: ItemInstance, registry: &ItemRegistry) -> Option<ItemInstance> {
        let max_stack = registry
            .get(item.def_id)
            .map(|d| d.max_stack)
            .unwrap_or(1)
            .max(1);

        if max_stack > 1 {
            for existing in self.slots.iter_mut().filter_map(|s| s.item.as_mut()) {
                if item.stack_count == 0 {
                    return None;
                }
                if existing.def_id == item.def_id {
                    // Saturating: a stack already over the limit (e.g. from
                    // a save made under a larger max_stack) has no room.
                    let moved = max_stack
                        .saturating_sub(existing.stack_count)
                        .min(item.stack_count);
                    existing.stack_count += moved;
                    item.stack_count -= moved;
                }
            }
        }

        for slot in &mut self.slots {
            if item.stack_count == 0 {
                return None;
            }
            if slot.is_empty() {
                let mut part = item.clone();
                part.stack_count = item.stack_count.min(max_stack);
                item.stack_count -= part.stack_count;
                slot.item = Some(part);
            }
        }

        (item.stack_count > 0).then_some(item)
    }

    /// Remove an item at a specific slot index. Returns the removed item.
    pub fn remove(&mut self, index: usize) -> Option<ItemInstance> {
        self.slots.get_mut(index)?.take()
    }

    /// Remove a specific count of an item by def_id. Returns actual removed count.
    pub fn remove_by_id(&mut self, def_id: u32, count: u32) -> u32 {
        let mut remaining = count;
        for slot in &mut self.slots {
            if remaining == 0 {
                break;
            }
            if let Some(item) = &mut slot.item
                && item.def_id == def_id
            {
                if item.stack_count <= remaining {
                    remaining -= item.stack_count;
                    slot.item = None;
                } else {
                    item.stack_count -= remaining;
                    remaining = 0;
                }
            }
        }
        count - remaining
    }

    /// Count total items with a given def_id.
    pub fn count(&self, def_id: u32) -> u32 {
        self.slots
            .iter()
            .filter_map(|s| s.item.as_ref())
            .filter(|i| i.def_id == def_id)
            .map(|i| i.stack_count)
            .sum()
    }

    /// Check if inventory has at least `count` of an item.
    pub fn has(&self, def_id: u32, count: u32) -> bool {
        self.count(def_id) >= count
    }

    /// Get a reference to a slot.
    pub fn slot(&self, index: usize) -> Option<&InventorySlot> {
        self.slots.get(index)
    }

    /// Get a mutable reference to a slot.
    pub fn slot_mut(&mut self, index: usize) -> Option<&mut InventorySlot> {
        self.slots.get_mut(index)
    }

    /// Swap two slots.
    pub fn swap(&mut self, a: usize, b: usize) {
        if a < self.slots.len() && b < self.slots.len() {
            self.slots.swap(a, b);
        }
    }

    /// Number of free slots.
    pub fn free_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_empty()).count()
    }

    /// Check if inventory is full.
    pub fn is_full(&self) -> bool {
        self.free_slots() == 0
    }

    /// Iterator over non-empty slots with their indices.
    pub fn iter_items(&self) -> impl Iterator<Item = (usize, &ItemInstance)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.item.as_ref().map(|item| (i, item)))
    }

    /// Get all slots.
    pub fn slots(&self) -> &[InventorySlot] {
        &self.slots
    }
}

// ---------------------------------------------------------------------------
// Equipment slots
// ---------------------------------------------------------------------------

/// Named equipment slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    MainHand,
    OffHand,
    Head,
    Chest,
    Legs,
    Boots,
    Gloves,
    Ring1,
    Ring2,
    Amulet,
}

impl EquipSlot {
    /// All equipment slots.
    pub fn all() -> &'static [EquipSlot] {
        &[
            EquipSlot::MainHand,
            EquipSlot::OffHand,
            EquipSlot::Head,
            EquipSlot::Chest,
            EquipSlot::Legs,
            EquipSlot::Boots,
            EquipSlot::Gloves,
            EquipSlot::Ring1,
            EquipSlot::Ring2,
            EquipSlot::Amulet,
        ]
    }

    /// Which item types can go in this slot.
    pub fn accepts(&self, item_type: ItemType) -> bool {
        match self {
            EquipSlot::MainHand => matches!(item_type, ItemType::Weapon),
            EquipSlot::OffHand => matches!(item_type, ItemType::Shield | ItemType::Weapon),
            EquipSlot::Head => matches!(item_type, ItemType::Helmet),
            EquipSlot::Chest | EquipSlot::Legs | EquipSlot::Gloves => {
                matches!(item_type, ItemType::Armor)
            }
            EquipSlot::Boots => matches!(item_type, ItemType::Boots),
            EquipSlot::Ring1 | EquipSlot::Ring2 => matches!(item_type, ItemType::Ring),
            EquipSlot::Amulet => matches!(item_type, ItemType::Amulet),
        }
    }
}

/// Equipment container.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Equipment {
    slots: FxHashMap<EquipSlot, ItemInstance>,
}

impl Equipment {
    pub fn new() -> Self {
        Self {
            slots: FxHashMap::default(),
        }
    }

    /// Equip an item. Returns the previously equipped item (if any).
    pub fn equip(&mut self, slot: EquipSlot, item: ItemInstance) -> Option<ItemInstance> {
        let prev = self.slots.remove(&slot);
        self.slots.insert(slot, item);
        prev
    }

    /// Unequip an item from a slot.
    pub fn unequip(&mut self, slot: EquipSlot) -> Option<ItemInstance> {
        self.slots.remove(&slot)
    }

    /// Get the item in a slot.
    pub fn get(&self, slot: EquipSlot) -> Option<&ItemInstance> {
        self.slots.get(&slot)
    }

    /// Sum a modifier stat across all equipped items.
    pub fn total_modifier(&self, stat: &str) -> f32 {
        self.slots
            .values()
            .flat_map(|item| item.modifiers.iter())
            .filter(|m| m.stat == stat)
            .map(|m| m.value)
            .sum()
    }

    /// Iterate over all equipped items.
    pub fn iter(&self) -> impl Iterator<Item = (&EquipSlot, &ItemInstance)> {
        self.slots.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loot::Rarity;

    fn test_registry() -> ItemRegistry {
        let mut reg = ItemRegistry::new();
        reg.register(ItemDef {
            id: 1,
            name: "Health Potion".to_string(),
            item_type: ItemType::Consumable,
            rarity: Rarity::Common,
            max_stack: 20,
            icon_name: "potion_hp".to_string(),
            value: 10,
        });
        reg.register(ItemDef {
            id: 2,
            name: "Iron Sword".to_string(),
            item_type: ItemType::Weapon,
            rarity: Rarity::Common,
            max_stack: 1,
            icon_name: "sword_iron".to_string(),
            value: 50,
        });
        reg
    }

    // ── Inventory tests ─────────────────────────────────────────

    #[test]
    fn oversized_stacks_are_split_at_max_stack() {
        let reg = test_registry(); // potions stack to 20
        let mut inv = Inventory::new(3);

        // 45 potions in one instance: two full slots, one with 5.
        assert!(inv.add(ItemInstance::with_stack(1, 45), &reg).is_none());
        let counts: Vec<u32> = inv
            .slots
            .iter()
            .map(|s| s.item.as_ref().map_or(0, |i| i.stack_count))
            .collect();
        assert_eq!(counts, vec![20, 20, 5]);

        // Adding to a full-ish inventory tops up, then returns what is left.
        let leftover = inv.add(ItemInstance::with_stack(1, 30), &reg).unwrap();
        assert_eq!(leftover.stack_count, 15);
    }

    #[test]
    fn a_stack_already_over_the_limit_does_not_underflow() {
        let reg = test_registry();
        let mut inv = Inventory::new(2);
        inv.slots[0].item = Some(ItemInstance::with_stack(1, 25)); // over max 20
        // Used to compute 20 - 25 in u32: a debug panic, unbounded in release.
        assert!(inv.add(ItemInstance::with_stack(1, 1), &reg).is_none());
        assert_eq!(inv.slots[0].item.as_ref().unwrap().stack_count, 25);
        assert_eq!(inv.slots[1].item.as_ref().unwrap().stack_count, 1);
    }

    #[test]
    fn inventory_add_and_stack() {
        let reg = test_registry();
        let mut inv = Inventory::new(10);

        // Add 5 potions
        inv.add(ItemInstance::with_stack(1, 5), &reg);
        assert_eq!(inv.count(1), 5);

        // Add 3 more → should stack
        inv.add(ItemInstance::with_stack(1, 3), &reg);
        assert_eq!(inv.count(1), 8);
        assert_eq!(inv.free_slots(), 9); // only used 1 slot
    }

    #[test]
    fn inventory_remove_by_id() {
        let reg = test_registry();
        let mut inv = Inventory::new(10);
        inv.add(ItemInstance::with_stack(1, 10), &reg);

        let removed = inv.remove_by_id(1, 7);
        assert_eq!(removed, 7);
        assert_eq!(inv.count(1), 3);
    }

    // ── Equipment tests ─────────────────────────────────────────

    #[test]
    fn equipment_equip_unequip() {
        let mut equip = Equipment::new();
        let sword = ItemInstance::new(2);

        assert!(equip.get(EquipSlot::MainHand).is_none());
        equip.equip(EquipSlot::MainHand, sword);
        assert!(equip.get(EquipSlot::MainHand).is_some());

        let removed = equip.unequip(EquipSlot::MainHand);
        assert!(removed.is_some());
        assert!(equip.get(EquipSlot::MainHand).is_none());
    }

    #[test]
    fn equipment_swap() {
        let mut equip = Equipment::new();
        let sword1 = ItemInstance::new(2);
        let sword2 = ItemInstance::new(2);

        equip.equip(EquipSlot::MainHand, sword1);
        let prev = equip.equip(EquipSlot::MainHand, sword2);
        assert!(prev.is_some());
    }
}
