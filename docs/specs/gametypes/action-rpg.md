---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/pathfinding", "engine/inventory"]
last_updated: 2026-06-09
---

# Action RPG

## Purpose

Diablo-style action RPGs: click-to-move through monster-filled zones, real-time combat with skills on cooldowns, status effects (burns, slows, stuns), and a loot fountain — weighted drop tables, rarity tiers, ground items with pickup magnetism.

Examples: Diablo II/III (rarity-tiered loot, skill cooldowns), Path of Exile (drop tables, DoT stacking), Torchlight (ground loot with pickup delay), Hades (ability-based real-time combat).

Backed by `amigo_core::loot`, `amigo_core::combat`, and `amigo_core::status_effect`. Click-to-move itself is composed from `engine/pathfinding` (A* to the clicked tile) plus `engine/steering`; the modules here provide everything that happens once the player arrives.

## Public API

### Skills and Damage (`amigo_core::combat`)

```rust
/// Skill definition: element, damage, cooldown, range, optional AoE and projectile.
pub struct Ability {
    pub name: String,
    pub damage_type: DamageType,    // Physical/Fire/Ice/Lightning/Poison/Holy/Shadow/Pure
    pub base_damage: i32,
    pub cooldown_duration: f32,
    pub range: f32,
    pub aoe: Option<AoeShape>,      // Circle { radius } | Cone { radius, half_angle }
                                    // | Line { width, length } | Rect { width, height }
    pub projectile_speed: Option<f32>,
    pub mana_cost: i32,
}
impl Ability {
    pub fn basic_attack(damage: i32, range: f32) -> Self;
    pub fn fireball(damage: i32, range: f32, radius: f32) -> Self; // AoE projectile preset
}

/// Per-skill cooldown timer (trigger/update/is_ready/fraction_remaining for UI).
pub struct Cooldown { pub duration: f32, pub remaining: f32 }

/// Attacker/defender stat block used by the damage formula.
pub struct CombatStats {
    pub attack_power: i32, pub defense: i32, pub attack_speed: f32,
    pub critical_chance: f32, pub critical_multiplier: f32, pub attack_range: f32,
    pub resistances: Resistances, // per-element f32, Resistances::get(DamageType) -> f32
}

/// Deterministic damage roll: crit chance, attack-power scaling (+0.5/point),
/// defense with diminishing returns, elemental resists (clamped -0.5..0.9), min 1.
pub fn calculate_damage(base: i32, damage_type: DamageType, attacker: &CombatStats,
                        defender: &CombatStats, seed: u64) -> DamageResult;
pub struct DamageResult { pub final_amount: i32, pub is_critical: bool, pub was_resisted: bool }

/// Projectile with homing, pierce counting, per-hit dedup, max range expiry.
pub struct Projectile {
    pub pierce_count: u32, pub hit_entities: Vec<EntityId>,
    pub aoe_on_hit: Option<AoeShape>, pub target_entity: Option<EntityId>, /* ... */
}
impl Projectile {
    pub fn new(/* owner, start, direction, speed, damage, damage_type, max_range */) -> Self;
    pub fn homing(/* owner, start, target: EntityId, ... */) -> Self;
    pub fn update(&mut self, dt: f32, target_pos: Option<RenderVec2>);
    pub fn on_hit(&mut self, target: EntityId) -> bool; // pierces or dies
}

/// Hit-test for AoE skills (nova, cleave, beam).
pub fn point_in_aoe(point: RenderVec2, origin: RenderVec2, direction: RenderVec2, shape: &AoeShape) -> bool;
```

Combat emits `DamageEvent` (with `position` for floating damage numbers and `is_critical`), `HealEvent`, and `KillEvent { killer, victim, position }` — the kill event is the hook that triggers a loot roll at the victim's position.

### Status Effects (`amigo_core::status_effect`)

```rust
/// Slow (% move speed), Stun, Burn (DPS), Poison (stacking DPS),
/// ArmorBreak (flat armor reduction), Vulnerable (% extra damage taken).
pub enum EffectType { Slow, Stun, Burn, Poison, ArmorBreak, Vulnerable }

pub struct StatusEffect {
    pub effect_type: EffectType,
    pub magnitude: f32,    // interpretation depends on type
    pub remaining: f32,
    pub duration: f32,
    pub source_id: Option<u32>, // kill attribution for DoT kills
}
impl StatusEffect {
    pub fn new(effect_type: EffectType, magnitude: f32, duration: f32) -> Self;
    pub fn with_source(self, source: u32) -> Self;
    pub fn progress(&self) -> f32; // 0.0 fresh .. 1.0 expired (UI bars)
}

/// Per-entity container with refresh-vs-stack semantics.
pub struct StatusEffects { /* effects: Vec<StatusEffect> */ }
impl StatusEffects {
    /// Same-type effects refresh: stronger magnitude wins, longer duration wins.
    pub fn apply(&mut self, effect: StatusEffect);
    /// Always adds a new instance (Poison stacks).
    pub fn apply_stacking(&mut self, effect: StatusEffect);
    pub fn update(&mut self, dt: f32);                 // tick + drop expired
    pub fn speed_multiplier(&self) -> f32;             // 0.0 stunned .. 1.0; strongest slow wins
    pub fn damage_per_second(&self) -> f32;            // sum of Burn + Poison
    pub fn armor_reduction(&self) -> f32;              // sum of ArmorBreak
    pub fn damage_taken_multiplier(&self) -> f32;      // 1.0 + sum of Vulnerable %
    pub fn is_stunned(&self) -> bool;
    pub fn has(&self, effect_type: EffectType) -> bool;
}
```

### Loot (`amigo_core::loot`)

```rust
/// Common/Uncommon/Rare/Epic/Legendary with built-in drop weights (60/25/10/4/1).
pub enum Rarity { Common, Uncommon, Rare, Epic, Legendary }
impl Rarity { pub fn weight(self) -> f32; pub fn all() -> &'static [Rarity]; }

/// Item template: id, name, type (Weapon/Armor/.../Consumable/Material/Quest/Currency),
/// rarity, max_stack, icon_name, gold value.
pub struct ItemDef { pub id: u32, pub name: String, pub item_type: ItemType,
                     pub rarity: Rarity, pub max_stack: u32, pub icon_name: String, pub value: u32 }

/// Concrete instance with rolled affixes.
pub struct ItemInstance { pub def_id: u32, pub stack_count: u32, pub modifiers: Vec<ItemModifier> }
pub struct ItemModifier { pub stat: String, pub value: f32 } // e.g. ("attack_power", 5.0)

/// Per-monster drop table: weighted entries, roll count, per-roll drop chance, guaranteed drops.
pub struct DropTable {
    pub entries: Vec<DropEntry>,    // { item_def_id, weight, min_count, max_count, min_rarity }
    pub rolls: u32,
    pub drop_chance: f32,           // 0.0 - 1.0 per roll
    pub guaranteed: Vec<DropEntry>, // e.g. gold
}
// Builder: DropTable::new().with_entry(id, weight).with_guaranteed(id, min, max)
//          .with_rolls(n).with_drop_chance(c)

/// Deterministic roll (seeded xorshift): guaranteed drops first, then weighted rolls.
pub fn roll_drops(table: &DropTable, seed: u64) -> Vec<ResolvedDrop>;
pub struct ResolvedDrop { pub item_def_id: u32, pub count: u32 }

/// Loot on the ground awaiting pickup.
pub struct GroundItem {
    pub item: ItemInstance, pub position: RenderVec2, pub spawn_time: f64,
    pub pickup_delay: f32,   // anti-ninja, default 0.5s
    pub lifetime: f32,       // despawn, default 120s (0 = never)
    pub owner: Option<EntityId>, // pickup priority for the killer
    pub magnet_range: f32,   // attraction radius, default 32.0
}
impl GroundItem {
    pub fn new(item: ItemInstance, position: RenderVec2, time: f64) -> Self;
    pub fn can_pickup(&self, current_time: f64) -> bool;
    pub fn is_expired(&self, current_time: f64) -> bool;
}
```

## Behavior

- **Click-to-move**: A click raycasts to a tile; A* (`engine/pathfinding`) produces a path and steering follows it. Clicking a monster sets it as the attack target; the entity advances until `CombatStats.attack_range` is reached, then attacks on the `Cooldown` of the selected `Ability`.
- **Skill use**: On cast, either an instant AoE check (`point_in_aoe` over nearby enemies) or a spawned `Projectile`. Effective movement speed = base speed × `StatusEffects::speed_multiplier()`; incoming damage = `calculate_damage(...).final_amount` × `damage_taken_multiplier()`, with `armor_reduction()` subtracted from the defender's `defense` before the roll.
- **DoT ticking**: Each tick applies `damage_per_second() * dt` to the entity; `StatusEffect.source_id` attributes the kill if a DoT finishes the target.
- **Loot drop**: On `KillEvent`, the monster's `DropTable` is rolled with a seed derived from the world RNG; each `ResolvedDrop` spawns a `GroundItem` scattered around `KillEvent.position`. Items magnetize toward the player inside `magnet_range` once `can_pickup()` is true, then go into the `Inventory` (`amigo_core::inventory`, see the top-down-adventure spec) and `Equipment` for stat modifiers.

## Future Work

- No affix-generation tables — `ItemInstance.modifiers` exists, but rolling random affixes per rarity is game code.
- No skill tree / character progression module (XP, levels, skill points).
- No monster AI in these modules — compose Utility AI or FSM from `engine/agents`.
- `min_rarity` on `DropEntry` is stored but filtering against an `ItemRegistry` is left to the game.

## Referenzen

- [engine/pathfinding](../engine/pathfinding.md) — A* for click-to-move
- [engine/steering](../engine/steering.md) — path following and separation in monster packs
- [engine/agents](../engine/agents.md) — monster behavior
- [gametypes/roguelike](roguelike.md) — shares Rarity tiers and run-scoped item concepts
- Diablo II — drop tables, rarity weights, ground loot
- Path of Exile — stacking poison, armor break, vulnerability mechanics
