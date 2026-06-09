---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/dialogue", "engine/inventory", "engine/tilemap"]
last_updated: 2026-06-09
---

# Top-Down Adventure

## Purpose

Top-down exploration games with real-time combat, NPC conversations, item collection, and gated world traversal. The core loop is: explore the overworld, talk to NPCs, accept quests via dialog flags, fight enemies with abilities and projectiles, collect loot into a slot inventory, and unlock doors to reach new areas.

Examples: The Legend of Zelda (top-down dungeons, keys and locked doors), Secret of Mana (real-time combat with NPC towns), Stardew Valley's mine floors (combat + inventory), Undertale (dialog-driven encounters).

Backed by `amigo_core::combat`, `amigo_core::dialog`, `amigo_core::inventory`, and `amigo_core::door`.

## Public API

### Combat (`amigo_core::combat`)

```rust
/// Damage type for element/resist systems.
pub enum DamageType { Physical, Fire, Ice, Lightning, Poison, Holy, Shadow, Pure /* ignores armor/resists */ }

/// Combat stats for an entity.
pub struct CombatStats {
    pub attack_power: i32,
    pub defense: i32,
    pub attack_speed: f32,
    pub critical_chance: f32,
    pub critical_multiplier: f32,
    pub attack_range: f32,
    pub resistances: Resistances, // per-DamageType f32; 0.0 = none, 1.0 = immune, negative = vulnerable
}

/// An ability/skill definition with constructors for common archetypes.
pub struct Ability {
    pub name: String,
    pub damage_type: DamageType,
    pub base_damage: i32,
    pub cooldown_duration: f32,
    pub range: f32,
    pub aoe: Option<AoeShape>,            // Circle / Cone / Line / Rect
    pub projectile_speed: Option<f32>,
    pub mana_cost: i32,
}
impl Ability {
    pub fn basic_attack(damage: i32, range: f32) -> Self;
    pub fn fireball(damage: i32, range: f32, radius: f32) -> Self;
}

/// A cooldown timer for abilities / attacks:
/// trigger() / update(dt) / is_ready() / fraction_remaining() for UI rings.
pub struct Cooldown { pub duration: f32, pub remaining: f32 }

/// Calculate final damage after crit roll, defense, and resistance.
pub fn calculate_damage(base: i32, damage_type: DamageType, attacker: &CombatStats,
                        defender: &CombatStats, seed: u64) -> DamageResult;
pub struct DamageResult { pub final_amount: i32, pub is_critical: bool, pub was_resisted: bool }

/// A projectile in flight (supports homing, piercing, AoE-on-hit).
pub struct Projectile { /* owner, position, velocity, damage, max_range, pierce_count, ... */ }
impl Projectile {
    pub fn new(owner: Option<EntityId>, start: RenderVec2, direction: RenderVec2,
               speed: f32, damage: i32, damage_type: DamageType, max_range: f32) -> Self;
    pub fn homing(owner: Option<EntityId>, start: RenderVec2, target: EntityId, ...) -> Self;
    pub fn update(&mut self, dt: f32, target_pos: Option<RenderVec2>);
    pub fn on_hit(&mut self, target: EntityId) -> bool; // false = destroy projectile
}

/// AoE hit-testing helper for melee swings and spell impacts.
pub fn point_in_aoe(point: RenderVec2, origin: RenderVec2, direction: RenderVec2, shape: &AoeShape) -> bool;
```

The combat module also emits plain event structs — `DamageEvent`, `HealEvent`, `KillEvent` — for the game to route to UI (damage numbers), audio, and quest logic.

### Dialog (`amigo_core::dialog`)

```rust
/// A complete conversation: a graph of nodes keyed by DialogId.
pub struct DialogTree { pub id: u32, pub name: String, pub entry_point: DialogId,
                        pub nodes: FxHashMap<DialogId, DialogNode> }

/// One "screen" of dialog with speaker, portrait, text, choices, conditions, effects.
pub struct DialogNode { /* id, speaker, portrait, text, next, choices, conditions, effects */ }
// Builder API: DialogNode::new(id, speaker, text).with_next(n).with_choice(text, next)
//   .with_conditional_choice(text, next, cond).with_effect(e).with_condition(c)

/// Conditions: FlagSet/FlagNotSet/FlagEquals/FlagGreaterThan/FlagLessThan/HasItem/And/Or/Not.
pub enum DialogCondition { /* ... */ }
/// Effects: SetFlag/ClearFlag/IncrementFlag/GiveItem/TakeItem/GiveExp/Heal/StartBattle/PlaySound/Custom.
pub enum DialogEffect { /* ... */ }

/// Persistent, saveable flag store (quest state): set_flag/get_flag/has_flag/
/// increment_flag, plus check_condition() and apply_effect() for flag effects.
pub struct DialogState { /* flags: FxHashMap<String, i32> */ }

/// Runtime state machine: tracks current node, filters choices by condition,
/// and queues non-flag effects (items, exp, battles) as DialogGameEffect for the game.
pub struct DialogRunner { /* ... */ }
impl DialogRunner {
    pub fn start(&mut self, tree: &DialogTree, state: &mut DialogState);
    pub fn advance(&mut self, tree: &DialogTree, state: &mut DialogState);
    pub fn choose(&mut self, choice_index: usize, tree: &DialogTree, state: &mut DialogState);
    pub fn is_active(&self) -> bool;
    pub fn current_node<'a>(&self, tree: &'a DialogTree) -> Option<&'a DialogNode>;
    pub fn available_choices(&self) -> &[usize];
    pub fn take_effects(&mut self) -> Vec<DialogGameEffect>;
    pub fn stop(&mut self);
}

/// Central registry for all dialog trees (one per NPC/cutscene).
pub struct DialogRegistry { pub fn register(&mut self, tree: DialogTree); pub fn get(&self, id: u32) -> Option<&DialogTree>; }
```

### Inventory (`amigo_core::inventory`)

```rust
/// Central registry of all item definitions (from amigo_core::loot::ItemDef).
pub struct ItemRegistry { pub fn register(&mut self, def: ItemDef); pub fn get(&self, id: u32) -> Option<&ItemDef>; }

/// Slot-based inventory with automatic stacking.
pub struct Inventory { pub capacity: usize, /* slots: Vec<InventorySlot> */ }
impl Inventory {
    pub fn new(capacity: usize) -> Self;
    /// Stacks if possible; returns the leftover ItemInstance if full.
    pub fn add(&mut self, item: ItemInstance, registry: &ItemRegistry) -> Option<ItemInstance>;
    pub fn remove(&mut self, index: usize) -> Option<ItemInstance>;
    pub fn remove_by_id(&mut self, def_id: u32, count: u32) -> u32;
    pub fn count(&self, def_id: u32) -> u32;
    pub fn has(&self, def_id: u32, count: u32) -> bool;  // quest item checks
    pub fn swap(&mut self, a: usize, b: usize);          // drag & drop UI
    // also: free_slots(), is_full(), iter_items(), slot(), slot_mut(), slots()
}

/// Named equipment slots (MainHand, OffHand, Head, Chest, Legs, Boots, Gloves, Ring1/2, Amulet)
/// with per-slot ItemType validation via EquipSlot::accepts().
pub struct Equipment { /* ... */ }
impl Equipment {
    pub fn equip(&mut self, slot: EquipSlot, item: ItemInstance) -> Option<ItemInstance>; // returns previous
    pub fn unequip(&mut self, slot: EquipSlot) -> Option<ItemInstance>;
    pub fn total_modifier(&self, stat: &str) -> f32; // sum of ItemModifier values across equipped items
}
```

### Doors (`amigo_core::door`)

```rust
pub struct DoorId(pub u32);
pub enum DoorState { Open, Closed, Locked }
pub enum DoorAccess { Public, TeamOnly(u8), SystemOnly }

/// Definition: maps a door to a TriggerZone id in the collision layer,
/// with initial state, access control, vent flag, optional auto-lock timer.
pub struct DoorDef { /* id, zone_id, initial_state, access, is_vent, auto_lock_duration */ }

/// Manages all doors in the current map.
pub struct DoorManager { /* doors: Vec<DoorInstance> */ }
impl DoorManager {
    pub fn spawn_doors(&mut self, defs: &[DoorDef]);
    pub fn open(&mut self, door_id: DoorId, team: u8) -> Option<DoorEvent>;
    pub fn close(&mut self, door_id: DoorId, team: u8) -> Option<DoorEvent>;
    pub fn lock(&mut self, door_id: DoorId, duration: f32) -> Option<DoorEvent>;   // system-level
    pub fn unlock(&mut self, door_id: DoorId) -> Option<DoorEvent>;                // system-level
    pub fn update(&mut self, dt: f32) -> Vec<DoorEvent>;  // ticks auto-lock timers
    pub fn state(&self, door_id: DoorId) -> Option<DoorState>;
    pub fn is_passable(&self, zone_id: u32) -> bool;      // collision query
}
```

## Behavior

- **Exploration loop**: The player moves in real time on a tilemap. `DoorManager::is_passable()` is consulted by the collision layer for door trigger zones; opening a locked door is gated by game logic (e.g. `Inventory::has(key_id, 1)` then `DoorManager::unlock()` and `Inventory::remove_by_id()`).
- **NPC dialog**: Interacting with an NPC starts its `DialogTree` via `DialogRunner::start()`. Nodes whose conditions fail are skipped automatically. After each step the game drains `take_effects()` and applies `GiveItem`/`TakeItem` against the `Inventory`, `StartBattle` against the combat layer, etc. Quest progress lives entirely in `DialogState` flags, which serialize with the save game.
- **Combat**: Attacks roll `calculate_damage()` (crit via seeded xorshift RNG, defense with diminishing returns `def/(def+100)`, resists clamped to [-0.5, 0.9], minimum 1 damage). Melee swings query targets with `point_in_aoe()` (Cone for sword arcs); ranged abilities spawn `Projectile`s, updated per frame with optional homing. `Cooldown` gates ability reuse. Resulting `DamageEvent`/`KillEvent`s drive HP, drops, and quest flags.
- **Items**: `Equipment::total_modifier("attack_power")` feeds back into `CombatStats` when gear changes. The `HasItem` dialog condition is resolved by the game against the `Inventory` (the engine-side default returns true since `DialogState` does not own the inventory).

## Future Work

- No quest-log system beyond raw dialog flags (no structured quest defs, objectives, or journal UI).
- No NPC daily schedules or overworld NPC pathing in these modules (see `engine/agents` / `engine/pathfinding` for building blocks).
- `DoorState` has no "key item" field; key-to-door mapping is game code.
- No mana/HP pool type — `Ability.mana_cost` exists but resource pools are game-defined components.

## Referenzen

- [engine/dialogue](../engine/dialogue.md) — dialog presentation layer (text boxes, portraits)
- [engine/inventory](../engine/inventory.md) — inventory UI conventions
- [engine/tilemap](../engine/tilemap.md) — TriggerZone ids referenced by `DoorDef.zone_id`
- [engine/save-load](../engine/save-load.md) — `DialogState`, `Inventory`, `DoorManager` are all Serde-serializable
- The Legend of Zelda — locked doors, keys, top-down combat
- Secret of Mana — real-time abilities with cooldowns
