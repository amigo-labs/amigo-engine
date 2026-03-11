use crate::ecs::EntityId;
use crate::math::RenderVec2;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Item Rarity
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Rarity weight for drop probability.
    pub fn weight(self) -> f32 {
        match self {
            Rarity::Common => 60.0,
            Rarity::Uncommon => 25.0,
            Rarity::Rare => 10.0,
            Rarity::Epic => 4.0,
            Rarity::Legendary => 1.0,
        }
    }

    /// All rarities ordered from common to legendary.
    pub fn all() -> &'static [Rarity] {
        &[Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic, Rarity::Legendary]
    }
}

// ---------------------------------------------------------------------------
// Item definition
// ---------------------------------------------------------------------------

/// Item type categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemType {
    Weapon,
    Armor,
    Helmet,
    Boots,
    Ring,
    Amulet,
    Shield,
    Consumable,
    Material,
    Quest,
    Currency,
}

/// An item definition (template/prototype).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: u32,
    pub name: String,
    pub item_type: ItemType,
    pub rarity: Rarity,
    pub max_stack: u32,
    pub icon_name: String,
    /// Value in gold.
    pub value: u32,
}

/// An item instance (may have modifiers, durability, etc).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemInstance {
    pub def_id: u32,
    pub stack_count: u32,
    pub modifiers: Vec<ItemModifier>,
}

impl ItemInstance {
    pub fn new(def_id: u32) -> Self {
        Self {
            def_id,
            stack_count: 1,
            modifiers: Vec::new(),
        }
    }

    pub fn with_stack(def_id: u32, count: u32) -> Self {
        Self {
            def_id,
            stack_count: count,
            modifiers: Vec::new(),
        }
    }
}

/// A modifier on an item (e.g. +5 Attack Power).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemModifier {
    pub stat: String,
    pub value: f32,
}

// ---------------------------------------------------------------------------
// Drop table
// ---------------------------------------------------------------------------

/// A single entry in a drop table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DropEntry {
    pub item_def_id: u32,
    /// Probability weight (relative to other entries).
    pub weight: f32,
    /// Min-max stack count.
    pub min_count: u32,
    pub max_count: u32,
    /// Minimum rarity filter (only drop if rarity >= this).
    pub min_rarity: Option<Rarity>,
}

/// A drop table defining what an enemy can drop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DropTable {
    /// Individual entries.
    pub entries: Vec<DropEntry>,
    /// Number of rolls on this table (how many items can drop).
    pub rolls: u32,
    /// Chance per roll that something actually drops (0.0 - 1.0).
    pub drop_chance: f32,
    /// Guaranteed drops (always drop these).
    pub guaranteed: Vec<DropEntry>,
}

impl DropTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            rolls: 1,
            drop_chance: 0.5,
            guaranteed: Vec::new(),
        }
    }

    pub fn with_entry(mut self, item_id: u32, weight: f32) -> Self {
        self.entries.push(DropEntry {
            item_def_id: item_id,
            weight,
            min_count: 1,
            max_count: 1,
            min_rarity: None,
        });
        self
    }

    pub fn with_guaranteed(mut self, item_id: u32, min: u32, max: u32) -> Self {
        self.guaranteed.push(DropEntry {
            item_def_id: item_id,
            weight: 1.0,
            min_count: min,
            max_count: max,
            min_rarity: None,
        });
        self
    }

    pub fn with_rolls(mut self, rolls: u32) -> Self {
        self.rolls = rolls;
        self
    }

    pub fn with_drop_chance(mut self, chance: f32) -> Self {
        self.drop_chance = chance;
        self
    }
}

impl Default for DropTable {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Drop resolution
// ---------------------------------------------------------------------------

/// A resolved drop (ready to spawn as a ground item).
#[derive(Clone, Debug)]
pub struct ResolvedDrop {
    pub item_def_id: u32,
    pub count: u32,
}

struct LootRng(u64);

impl LootRng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xCAFE_BABE } else { seed })
    }

    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 & 0x00FF_FFFF) as f32 / 16_777_216.0
    }

    fn range_u32(&mut self, min: u32, max: u32) -> u32 {
        if min >= max { return min; }
        min + ((self.next_f32() * (max - min + 1) as f32) as u32).min(max - min)
    }
}

/// Roll a drop table and return resolved items.
pub fn roll_drops(table: &DropTable, seed: u64) -> Vec<ResolvedDrop> {
    let mut rng = LootRng::new(seed);
    let mut drops = Vec::new();

    // Guaranteed drops
    for entry in &table.guaranteed {
        let count = rng.range_u32(entry.min_count, entry.max_count);
        drops.push(ResolvedDrop {
            item_def_id: entry.item_def_id,
            count,
        });
    }

    // Random rolls
    let total_weight: f32 = table.entries.iter().map(|e| e.weight).sum();
    if total_weight <= 0.0 {
        return drops;
    }

    for _ in 0..table.rolls {
        // Check if this roll drops anything
        if rng.next_f32() > table.drop_chance {
            continue;
        }

        // Weighted random selection
        let mut roll = rng.next_f32() * total_weight;
        for entry in &table.entries {
            roll -= entry.weight;
            if roll <= 0.0 {
                let count = rng.range_u32(entry.min_count, entry.max_count);
                drops.push(ResolvedDrop {
                    item_def_id: entry.item_def_id,
                    count,
                });
                break;
            }
        }
    }

    drops
}

// ---------------------------------------------------------------------------
// Ground item (dropped loot in the world)
// ---------------------------------------------------------------------------

/// A loot item lying on the ground, waiting to be picked up.
#[derive(Clone, Debug)]
pub struct GroundItem {
    pub item: ItemInstance,
    pub position: RenderVec2,
    pub spawn_time: f64,
    /// Time in seconds before the item can be picked up (anti-ninja).
    pub pickup_delay: f32,
    /// Lifetime in seconds before despawn (0 = never).
    pub lifetime: f32,
    /// Owner who has pickup priority (e.g. the killer).
    pub owner: Option<EntityId>,
    /// Magnetic attraction toward player when close.
    pub magnet_range: f32,
}

impl GroundItem {
    pub fn new(item: ItemInstance, position: RenderVec2, time: f64) -> Self {
        Self {
            item,
            position,
            spawn_time: time,
            pickup_delay: 0.5,
            lifetime: 120.0,
            owner: None,
            magnet_range: 32.0,
        }
    }

    /// Check if the item can be picked up.
    pub fn can_pickup(&self, current_time: f64) -> bool {
        current_time - self.spawn_time >= self.pickup_delay as f64
    }

    /// Check if the item has expired.
    pub fn is_expired(&self, current_time: f64) -> bool {
        self.lifetime > 0.0 && current_time - self.spawn_time >= self.lifetime as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_table_rolls() {
        let table = DropTable::new()
            .with_entry(1, 10.0)
            .with_entry(2, 5.0)
            .with_entry(3, 1.0)
            .with_rolls(3)
            .with_drop_chance(1.0)
            .with_guaranteed(100, 5, 10); // guaranteed gold

        let drops = roll_drops(&table, 12345);
        // At least the guaranteed drop
        assert!(!drops.is_empty());
        assert!(drops.iter().any(|d| d.item_def_id == 100));
    }

    #[test]
    fn ground_item_pickup_delay() {
        let item = GroundItem::new(ItemInstance::new(1), RenderVec2::ZERO, 0.0);
        assert!(!item.can_pickup(0.3));
        assert!(item.can_pickup(0.6));
    }

    #[test]
    fn rarity_weights() {
        let total: f32 = Rarity::all().iter().map(|r| r.weight()).sum();
        assert_eq!(total, 100.0);
    }
}
