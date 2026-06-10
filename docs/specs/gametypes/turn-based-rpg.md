---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-06-09
---

# Turn-Based RPG

## Purpose

Template for classic turn-based RPG combat: speed-ordered rounds, a menu of actions (attack, skill, item, defend, flee, switch), elemental type effectiveness, status effects that tick at end of turn, and weighted random encounters on the world map. The battle is a step-driven state machine — the game calls `step()` once per UI/animation beat and renders the returned `BattleEffect`s.

Examples: Pokemon (type chart, switch, flee, status conditions), Dragon Quest (speed-ordered rounds, defend), Final Fantasy (parties, MP-cost skills), Earthbound (weighted area encounters).

Implementation: `crates/amigo_core/src/turn_combat.rs`. The generic real-time combat module (`combat.rs`: `DamageType`, `CombatStats`, `Cooldown`) is *not* used here — turn combat is self-contained with its own `Element` system.

## Public API

### Actions and Effects

```rust
/// An action a combatant can take during their turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TurnAction {
    Attack { target: usize },
    Skill { skill_id: u32, target: usize },
    UseItem { item_id: u32, target: usize },
    Defend,                     // 1.5x defense until next turn.
    Flee,                       // Speed-based escape chance.
    Switch { slot: usize },     // Party member switch (Pokemon-style).
    Skip,                       // Stunned, asleep, etc.
}

/// Result of executing an action (actor, action, effects).
pub struct ActionResult {
    pub actor: usize,
    pub action: TurnAction,
    pub effects: Vec<BattleEffect>,
}

/// An effect that happened during battle (consumed by animation/UI).
#[derive(Clone, Debug)]
pub enum BattleEffect {
    Damage { target: usize, amount: i32, is_critical: bool, element: Element },
    Heal { target: usize, amount: i32 },
    StatusApplied { target: usize, status: StatusEffect },
    StatusRemoved { target: usize, status: StatusType },
    Fainted { target: usize },
    Miss { target: usize },
    Fled,
    FleeBlocked,
    Switched { slot: usize },
    LevelUp { combatant: usize, new_level: u32 },
    ExpGained { amount: u32 },
}
```

### Elements and Type Effectiveness

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Element {
    Normal, Fire, Water, Grass, Electric, Ice,
    Earth, Wind, Light, Dark, Poison,
}

/// Type effectiveness multiplier: 2.0 (super effective), 0.5 (resisted),
/// 0.0 (immune, e.g. Electric vs Earth), 1.0 otherwise. Built-in chart
/// covers the classic Fire/Water/Grass triangle and extensions.
pub fn type_effectiveness(attack: Element, defend: Element) -> f32;
```

### Status Effects

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusType {
    Poison, Burn, Freeze, Paralyze, Sleep, Confused,
    AttackUp, AttackDown, DefenseUp, DefenseDown,
    SpeedUp, SpeedDown, Regen, Shield,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusEffect {
    pub status_type: StatusType,
    pub turns_remaining: u32,   // 0 = permanent until cured.
    pub magnitude: i32,         // DoT damage / stat modifier amount.
}

impl StatusEffect {
    pub fn new(status_type: StatusType, turns: u32, magnitude: i32) -> Self;
    pub fn is_debuff(&self) -> bool;
    /// Freeze, Paralyze, Sleep prevent acting.
    pub fn prevents_action(&self) -> bool;
}
```

### Skills

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SkillTarget { SingleEnemy, AllEnemies, SingleAlly, AllAllies, SelfOnly }

/// Data definition for a skill (RON-serializable).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: u32,
    pub name: String,
    pub element: Element,
    pub power: i32,
    pub accuracy: u32,
    pub cost: i32,              // MP cost.
    pub target: SkillTarget,
    /// (status type, chance %, turns) applied on hit.
    pub status_chance: Option<(StatusType, u32, u32)>,
    pub description: String,
}
```

### Combatants

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatantStats {
    pub hp: i32, pub max_hp: i32,
    pub mp: i32, pub max_mp: i32,
    pub attack: i32, pub defense: i32, pub speed: i32,
    pub level: u32, pub exp: u32,
    pub element: Element,
}

impl CombatantStats {
    pub fn is_alive(&self) -> bool;
    pub fn hp_fraction(&self) -> f32;
}

/// A combatant in battle.
#[derive(Clone, Debug)]
pub struct Combatant {
    pub name: String,
    pub stats: CombatantStats,
    pub skills: Vec<u32>,               // Skill ids this combatant knows.
    pub statuses: Vec<StatusEffect>,
    pub entity: Option<EntityId>,       // Optional ECS link for rendering.
    pub is_defending: bool,
    pub team: u8,                       // 0 = player party, 1 = enemy party.
}

impl Combatant {
    pub fn new(name: impl Into<String>, stats: CombatantStats, team: u8) -> Self;
    pub fn with_skills(self, skills: Vec<u32>) -> Self;
    pub fn is_alive(&self) -> bool;
    pub fn has_status(&self, status_type: StatusType) -> bool;
    /// Alive and not Frozen/Paralyzed/Asleep.
    pub fn can_act(&self) -> bool;
    // Stats with status modifiers applied (floor 1).
    // Defending multiplies defense by 1.5.
    pub fn effective_attack(&self) -> i32;
    pub fn effective_defense(&self) -> i32;
    pub fn effective_speed(&self) -> i32;
}
```

### Battle State Machine

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattlePhase {
    Setup,
    TurnOrder,                              // Sort by effective speed.
    WaitingForAction { combatant: usize },  // Needs submit_action.
    Executing,
    EndOfTurn,                              // Status ticks, defend reset.
    CheckResult,
    Victory, Defeat, Fled,
}

/// The battle system managing a turn-based encounter.
pub struct Battle {
    pub combatants: Vec<Combatant>,     // Player party then enemy party.
    pub phase: BattlePhase,
    pub turn_order: Vec<usize>,
    pub current_turn: usize,
    pub round: u32,
    pub exp_pool: u32,                  // Filled on Victory.
}

impl Battle {
    pub fn new(player_party: Vec<Combatant>, enemy_party: Vec<Combatant>) -> Self;
    pub fn start(&mut self);
    /// Submit an action for the combatant in WaitingForAction.
    pub fn submit_action(&mut self, action: TurnAction);
    /// Advance the battle by one step. Returns effects that occurred.
    pub fn step(&mut self) -> Vec<BattleEffect>;
    // Queries:
    pub fn alive_on_team(&self, team: u8) -> Vec<usize>;
    pub fn is_over(&self) -> bool;      // Victory | Defeat | Fled.
    pub fn current_combatant(&self) -> Option<usize>;
    /// True if the waiting combatant is on team 0.
    pub fn needs_player_input(&self) -> bool;
}
```

### Random Encounters

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncounterEntry {
    pub enemy_group_id: u32,
    pub weight: f32,
    pub min_level: u32,
    pub max_level: u32,
}

/// Encounter table for a map area. Chance rises with steps since the
/// last encounter (no long dry spells).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncounterTable {
    pub entries: Vec<EncounterEntry>,
    pub base_rate: f32,     // Base chance per step (0.0-1.0).
    pub steps: u32,
}

impl EncounterTable {
    pub fn new(base_rate: f32) -> Self;
    pub fn with_entry(self, group_id: u32, weight: f32, min_level: u32, max_level: u32) -> Self;
    /// Call on each player step. Returns the enemy_group_id if an
    /// encounter triggers (weighted pick among level-eligible entries).
    pub fn check(&mut self, player_level: u32, seed: u64) -> Option<u32>;
}
```

## Behavior

- **Round loop**: `start()` → `step()` enters `TurnOrder` (alive combatants sorted by `effective_speed`, descending) → for each combatant in order, `WaitingForAction`; combatants that `!can_act()` (Freeze/Paralyze/Sleep) are skipped automatically. The game shows the menu when `needs_player_input()`, runs enemy AI otherwise, then calls `submit_action` + `step()` to execute. After each action `EndOfTurn` ticks that actor's statuses and clears `is_defending`. When the order is exhausted, `CheckResult` either ends the battle or starts the next round.
- **Damage formula** (basic attack): `(attack * 2 - defense) * type_effectiveness * crit`, minimum 1; crits are 10% for 1.5x via the battle's internal deterministic xorshift RNG. `Fainted` is emitted when HP reaches 0.
- **Defend / Flee**: Defend sets `is_defending` (1.5x effective defense) until the actor's next end-of-turn. Flee compares max alive player speed vs. max alive enemy speed: `0.5 + (player - enemy) * 0.05`, clamped to 10–95%; failure emits `FleeBlocked` and wastes the turn.
- **Status ticks** (end of the actor's turn): Poison/Burn deal `magnitude` damage, Regen heals `magnitude`; durations decrement and expired statuses emit `StatusRemoved`. Stat buffs/debuffs apply continuously through the `effective_*` accessors.
- **Victory rewards**: on Victory, `exp_pool` is set to the sum of `level * 10 + 5` over enemies and `ExpGained` is emitted; distribution and level-ups are game code (`BattleEffect::LevelUp` exists for the UI).
- **Encounters**: `EncounterTable::check` per overworld step; the trigger chance grows 2% per dry step, resets on trigger, then picks a weighted entry whose `[min_level, max_level]` contains the player level.

## Internal Design

- `Battle::step` is intentionally single-step so the UI can animate every `BattleEffect` between calls; no internal timers, no engine coupling — fully headless-testable (see tests in `turn_combat.rs`).
- RNG is an internal xorshift64 seeded with a constant (12345); battles are deterministic given the same action sequence. Encounter rolls derive their state from `seed ^ steps * 0x9E37_79B9` so the caller controls world-seed determinism.
- Combatant references are plain indices into `Battle::combatants` (player party first, then enemies); `team` distinguishes sides, which keeps `TurnAction` serializable.
- `Combatant::entity` optionally links to an ECS entity for battle sprites; the battle itself never touches the ECS.

## Future Work

- **Skill execution is a placeholder**: `TurnAction::Skill` currently ignores `skill_id` and resolves as a basic attack; wiring `SkillDef` (element, power, accuracy, MP cost, `status_chance`, multi-target via `SkillTarget`) into `execute_action` is the main gap.
- **Item effects are a placeholder**: `UseItem` heals a flat 30 HP regardless of `item_id`; should integrate with the inventory system.
- **Switch is a stub**: it only emits `BattleEffect::Switched`; bench/party slot management is not implemented.
- Accuracy/evasion rolls (`BattleEffect::Miss` exists but nothing emits it) and `StatusType::Shield`/`Confused` behavior.
- Exp distribution and level-up curves (only the pooled total is computed).

## Open Questions

- Should the type chart be data-driven (RON matrix) instead of the hard-coded `type_effectiveness` match?
- Should `Battle` expose a seedable RNG constructor for replay-exact battles across runs?
- ATB/CTB variants (per-combatant charge gauges) — same state machine with a timed `TurnOrder`, or a separate template?

## Referenzen

- [engine/core](../engine/core.md) — `EntityId` for optional sprite links
- [engine/dialogue](../engine/dialogue.md) — Battle text and menus
- [engine/save-load](../engine/save-load.md) — Party stats persistence between battles
- [gametypes/roguelike](roguelike.md) — Alternative turn model (energy-based, grid)
- Pokemon — Type chart, switch/flee, status conditions
- Dragon Quest — Speed-ordered rounds, defend command
- Earthbound — Step-based weighted encounter tables
