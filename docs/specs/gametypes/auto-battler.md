---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/simulation"]
last_updated: 2026-06-09
---

# Auto-Battler

## Purpose

Round-based games where the player builds an army but never controls it in combat. The loop per round: receive gold (plus streak bonus and interest), buy units from a rerollable shop, place them on a grid, merge triples into star-upgraded units, then watch a deterministic auto-resolved fight against an opponent's board. Losing costs player HP; the last player standing wins. Strategy lives in economy management, unit positioning, and trait synergies.

Examples: Teamfight Tactics (traits + interest economy), Dota Underlords, Super Auto Pets (simplified lane battler).

Implemented in `crates/amigo_core/src/auto_battler.rs`.

## Public API

### Configuration

```rust
/// Configuration for an auto-battler game.
/// Defaults: 7x4 grid, bench 8, shop 5, 10 starting gold, 5 gold/round,
/// 10% interest capped at 5, reroll cost 2.
pub struct AutoBattlerConfig {
    pub grid_width: u8, pub grid_height: u8,
    pub bench_size: u8, pub shop_size: u8,
    pub starting_gold: u32, pub gold_per_round: u32,
    pub interest_rate: f32, pub max_interest: u32,
    pub reroll_cost: u32,
    pub seed: u64,
}
```

### Units and Traits

```rust
/// Unique unit type identifier.
pub struct UnitId(pub u32);
/// Unique trait identifier.
pub struct TraitId(pub u32);

/// Base stats for a unit.
pub struct UnitStats { pub hp: i32, pub attack: i32, pub attack_speed: f32, pub range: u8, pub armor: i32 }

/// Definition of a unit type.
pub struct UnitDef {
    pub id: UnitId,
    pub name: String,
    pub tier: u8,
    pub traits: Vec<TraitId>,
    pub base_stats: UnitStats,
    pub cost: u32,
}

/// A live unit on the board or bench.
pub struct UnitInstance {
    pub def_id: UnitId,
    pub star_level: u8,                 // 1, 2, or 3 stars
    pub position: Option<(u8, u8)>,     // grid cell; None if on bench
    pub stats: UnitStats,               // current stats (scaled by star level)
    pub current_hp: i32,                // current HP in combat
}

/// A bonus granted by a trait at a specific threshold.
pub struct TraitBonus { pub attack_bonus: i32, pub hp_bonus: i32, pub armor_bonus: i32 }

/// Definition of a synergy trait.
pub struct TraitDef {
    pub id: TraitId,
    pub name: String,
    /// (count_required, bonus) pairs, sorted ascending.
    pub thresholds: Vec<(u8, TraitBonus)>,
}
```

### Shop

```rust
/// The shop that appears between rounds.
pub struct Shop {
    pub slots: Vec<Option<UnitId>>,
    pub frozen: bool,
}

impl Shop {
    pub fn new(size: u8) -> Self;
    /// Reroll all slots from the unit pool. Returns false if not enough gold.
    pub fn reroll(&mut self, gold: &mut u32, cost: u32, pool: &[UnitId], rng: &mut u64) -> bool;
    /// Buy a unit from a slot. Refunds nothing on failure; the slot is
    /// restored if gold is insufficient.
    pub fn buy(&mut self, slot: usize, gold: &mut u32, unit_cost: u32) -> Option<UnitId>;
    /// Toggle freeze (keep current offerings next round).
    pub fn toggle_freeze(&mut self);
}
```

### Phases, State, Events, System Functions

```rust
/// Phases of an auto-battler round.
pub enum RoundPhase { Shop, Place, Combat, Results }

/// Result of a combat round.
pub struct CombatResult { pub won: bool, pub damage_taken: u32, pub surviving_units: u32 }

/// Top-level auto-battler state.
pub struct AbState {
    pub config: AutoBattlerConfig,
    pub phase: RoundPhase,
    pub round: u32,
    pub gold: u32,
    pub hp: i32, pub max_hp: i32,       // starts at 100
    pub board: Vec<UnitInstance>,
    pub bench: Vec<UnitInstance>,
    pub shop: Shop,
    pub win_streak: u32, pub loss_streak: u32,
    pub rng_state: u64,
}

impl AbState {
    pub fn new(config: AutoBattlerConfig) -> Self;
}

pub enum AbEvent {
    PhaseChanged { from: RoundPhase, to: RoundPhase },
    RoundStarted { round: u32 },
    GoldReceived { amount: u32, interest: u32 },
    UnitBought { unit: UnitId },
    UnitMerged { unit: UnitId, new_star: u8 },
    CombatResolved { result: CombatResult },
    PlayerDamaged { damage: u32, hp_remaining: i32 },
    PlayerEliminated,
    TraitActivated { trait_id: TraitId, count: u8 },
}

/// Calculate gold income for a new round: base + streak bonus (capped at 3)
/// + interest (floor(gold * rate), capped at max_interest).
/// Returns (total_income, interest).
pub fn calculate_income(state: &AbState) -> (u32, u32);

/// Start a new round (Shop phase): grants income and emits
/// PhaseChanged / RoundStarted / GoldReceived.
pub fn start_round(state: &mut AbState) -> Vec<AbEvent>;

/// Resolve combat between two teams deterministically (seeded xorshift,
/// capped at 100 exchange rounds; timeout counts as a draw).
pub fn resolve_combat(team_a: &mut [UnitInstance], team_b: &mut [UnitInstance], seed: u64) -> CombatResult;

/// Try to merge 3 copies of the same unit into a star upgrade
/// (3x 1-star → one 2-star, 3x 2-star → one 3-star; x1.8 HP, x1.5 attack).
pub fn try_merge(units: &mut Vec<UnitInstance>, def_id: UnitId) -> Option<u8>;

/// Calculate active synergies from living units on the board. Returns the
/// trait count and the highest reached threshold bonus (None if below all).
pub fn calculate_synergies(
    board: &[UnitInstance],
    unit_registry: &FxHashMap<UnitId, UnitDef>,
    trait_registry: &FxHashMap<TraitId, TraitDef>,
) -> Vec<(TraitId, u8, Option<TraitBonus>)>;
```

## Behavior

- **Economy**: `start_round()` grants `gold_per_round` + `max(win_streak, loss_streak).min(3)` + interest. Interest is `floor(gold * interest_rate)` capped at `max_interest`, rewarding banked gold like TFT's 10/50 rule.
- **Shop**: `Shop::reroll()` deducts `reroll_cost` and refills every slot with a uniformly random pick from the supplied unit pool (the pool encodes tier odds — duplicate entries increase weight). `buy()` takes the unit out of the slot and puts it back if gold is insufficient. `frozen` is a flag for the game layer to skip the automatic refresh between rounds.
- **Placement**: A `UnitInstance` is on the board grid when `position == Some((x, y))` (within `grid_width` x `grid_height`) and on the bench when `None`. Board/bench capacity enforcement is the caller's job (`bench_size` in config).
- **Merging**: After each purchase, the game layer calls `try_merge()` on the combined board+bench vector. Three matching 1-stars collapse into one 2-star (HP x1.8, attack x1.5); three 2-stars into one 3-star. 3-star units never merge further.
- **Auto-Combat**: `resolve_combat()` alternates full-team attack passes. Each living attacker picks a random living defender (seeded RNG) and deals `max(attack - armor, 1)` damage. The function mutates both teams' `current_hp`, so the caller can show end-of-combat board state. Same inputs + seed = identical result, enabling lockstep replay of fights on all clients.
- **Synergies**: `calculate_synergies()` counts traits across living board units and returns the highest satisfied threshold per trait; the game layer applies `TraitBonus` stats before combat and emits `TraitActivated`.

## Internal Design

- Pure data + free functions: there is no internal tick. The game layer drives `RoundPhase` transitions and calls `resolve_combat()` once when entering `Combat`.
- All randomness flows through xorshift64 streams (`AbState::rng_state` for the shop, an explicit seed parameter for combat), keeping shop rolls and combat independent and reproducible.
- `UnitInstance.stats` is a denormalized copy of `UnitDef::base_stats` scaled by star level, so combat needs no registry access.
- All types are `Serialize`/`Deserialize` for save games and network state sync.

## Future Work

- Positional combat: `position`, `range`, and `attack_speed` exist on unit data but `resolve_combat()` currently ignores them (random-target exchange model). A grid-aware simulation with movement and range is the natural next step.
- Player damage scaling per round: the internal `state_round_bonus()` stub always returns 1.
- Items/equipment, unit selling (gold refund), and opponent matchmaking rotation.
- Engine-side application of `TraitBonus` and emission of `UnitMerged`/`TraitActivated` (currently the game layer wires these).

## Referenzen

- Teamfight Tactics: interest economy, streaks, traits, 3x merge rule
- Super Auto Pets: deterministic seeded auto-combat as replay/network primitive
- [engine/core](../engine/core.md) → event flow conventions
- [engine/simulation](../engine/simulation.md) → determinism requirements for lockstep play
- [engine/save-load](../engine/save-load.md) → AbState is fully serializable
