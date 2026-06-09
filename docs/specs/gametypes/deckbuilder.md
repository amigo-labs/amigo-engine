---
status: done
crate: amigo_core
depends_on: ["engine/core", "engine/procedural", "engine/save-load"]
last_updated: 2026-06-09
---

# Deckbuilder

## Purpose

Roguelike deckbuilders: the player fights card-based combats along a branching procedural map, earning new cards, relics, and gold between fights. The core loop is draw a hand, spend energy to play cards, end turn, watch telegraphed enemy intents resolve — then improve the deck at reward/shop/rest nodes and climb toward the boss. Deck shuffling and map layout are seed-deterministic.

Examples: Slay the Spire (energy + intents + relics), Monster Train (multi-floor combat), Inscryption (deckbuilding wrapped in narrative).

Implemented in `crates/amigo_core/src/deckbuilder.rs` (run/combat flow) and `crates/amigo_core/src/card.rs` (cards, deck, hand).

## Public API

### Cards (card.rs)

```rust
/// Unique card definition identifier.
pub struct CardId(pub u32);

pub enum Rarity { Common, Uncommon, Rare, Legendary }

/// How a card selects its targets. Random(n) = N random enemies.
pub enum TargetKind { Self_, SingleEnemy, AllEnemies, SingleAlly, AllAllies, Random(u8) }

/// An effect that a card applies when played.
pub enum CardEffect {
    Damage { amount: u32, damage_type: String },
    Block(u32), Heal(u32), DrawCards(u8), GainEnergy(u8),
    ApplyStatus { effect: String, magnitude: f32, duration: f32 },
    Custom(String),
}

/// Definition of a card.
pub struct CardDef {
    pub id: CardId,
    pub name: String,
    pub cost: u8,              // energy cost to play
    pub rarity: Rarity,
    pub effects: Vec<CardEffect>,
    pub tags: Vec<String>,     // synergy tags ("attack", "skill", "power")
    pub upgraded: bool,
    pub target: TargetKind,
}

/// Registry holding all card definitions.
pub struct CardRegistry { /* private map */ }

impl CardRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, def: CardDef);
    pub fn get(&self, id: CardId) -> Option<&CardDef>;
    pub fn by_rarity(&self, rarity: Rarity) -> Vec<&CardDef>;
    pub fn by_tag(&self, tag: &str) -> Vec<&CardDef>;
}

/// Events produced by the card system.
pub enum CardEvent {
    Drawn { card: CardId }, Played { card: CardId, target: TargetKind },
    Discarded { card: CardId }, Exhausted { card: CardId }, DeckShuffled,
}

/// A deck with draw, discard, and exhaust piles. Deterministic xorshift shuffle.
pub struct Deck { /* private: draw_pile, discard_pile, exhaust_pile, rng_state */ }

impl Deck {
    pub fn new(cards: Vec<CardId>, seed: u64) -> Self;  // shuffled with seed
    /// Draw N cards. If the draw pile runs out, the discard pile is shuffled back in.
    pub fn draw(&mut self, n: u8) -> Vec<CardId>;
    pub fn discard(&mut self, card: CardId);
    pub fn exhaust(&mut self, card: CardId);            // removed from the game
    pub fn shuffle_discard_into_draw(&mut self);
    pub fn remaining(&self) -> usize;                   // draw pile
    pub fn discard_count(&self) -> usize;
    pub fn exhaust_count(&self) -> usize;
    pub fn total(&self) -> usize;                       // all piles
    pub fn add_card(&mut self, card: CardId);           // to draw pile (e.g. rewards)
}

/// A player's hand of cards.
pub struct Hand { pub cards: Vec<CardId>, pub max_size: u8 }

impl Hand {
    pub fn new(max_size: u8) -> Self;
    /// Add a card. Returns Some(card) back if the hand is full (overflow).
    pub fn add(&mut self, card: CardId) -> Option<CardId>;
    pub fn remove(&mut self, index: usize) -> Option<CardId>;
    pub fn is_full(&self) -> bool;
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn discard_all(&mut self) -> Vec<CardId>;       // discard all, returning them
}
```

### Run Map, Relics, Rewards (deckbuilder.rs)

```rust
/// Kind of encounter at a map node.
pub enum NodeKind { Combat, Elite, Boss, Shop, Rest, Event, Treasure }

/// A node on the run map. connections = IDs of nodes on the next floor.
pub struct MapNode { pub id: u32, pub kind: NodeKind, pub connections: Vec<u32> }

/// Generate a procedural map. Floor 0 is always Combat, the last floor is a
/// single Boss node; intermediate kinds are seed-deterministic.
pub fn generate_map(floors: u32, nodes_per_floor: u32, seed: u64) -> Vec<Vec<MapNode>>;

pub struct RelicId(pub u32);

pub enum RelicTrigger { OnCombatStart, OnTurnStart, OnCardPlayed, OnDamageDealt, Passive }

pub enum RelicEffect {
    GainEnergy(u8), DrawCards(u8), GainBlock(u32),
    DamageBoost(u32), HealOnRest(u32), Custom(String),
}

pub struct RelicDef { pub id: RelicId, pub name: String, pub trigger: RelicTrigger, pub effect: RelicEffect }

/// A reward choice after combat.
pub enum RewardChoice { AddCard(CardId), RemoveCard, Gold(u32), Relic(RelicId), Heal(u32) }
```

### Run State and Combat

```rust
/// Configuration for a deckbuilder run. Defaults: 3 energy, hand size 5, 50 HP.
pub struct DeckbuilderConfig {
    pub starting_energy: u8, pub max_energy: u8, pub hand_size: u8,
    pub starting_deck: Vec<CardId>,
    pub starting_hp: i32, pub starting_max_hp: i32,
    pub seed: u64,
}

pub enum DbPhase { MapSelect, Combat, Reward, Shop, Rest, Event, GameOver }

/// Combat state within a deckbuilder.
pub struct CombatState {
    pub player_hp: i32, pub player_max_hp: i32, pub player_block: u32,
    pub energy: u8, pub max_energy: u8,
    pub enemies: Vec<EnemyState>,
    pub turn: u32,
}

/// A combat enemy with a telegraphed next action (intent display).
pub struct EnemyState {
    pub id: u32, pub name: String,
    pub hp: i32, pub max_hp: i32, pub block: u32,
    pub intent: EnemyIntent,
}

pub enum EnemyIntent { Attack(u32), Block(u32), Buff, Debuff, Unknown }

/// Top-level deckbuilder state. player_hp/max_hp persist across combats.
pub struct DbState {
    pub config: DeckbuilderConfig,
    pub phase: DbPhase,
    pub deck: Deck, pub hand: Hand,
    pub combat: Option<CombatState>,
    pub gold: u32, pub floor: u32,
    pub player_hp: i32, pub player_max_hp: i32,
    pub relics: Vec<RelicId>,
    pub map: Vec<Vec<MapNode>>,
    pub reward_choices: Vec<RewardChoice>,
}

impl DbState {
    /// Creates the deck/hand from config and generates a 15-floor, 3-wide map.
    pub fn new(config: DeckbuilderConfig) -> Self;
}
```

### System Functions and Events

```rust
/// Start combat: snapshots persistent HP into CombatState, sets energy,
/// draws the starting hand.
pub fn start_combat(state: &mut DbState, enemies: Vec<EnemyState>) -> Vec<DbEvent>;

/// Play a card from hand by index. Validates energy cost via the registry,
/// discards the card, and checks for combat victory.
pub fn play_card(state: &mut DbState, hand_index: usize, target: Option<u32>, registry: &CardRegistry) -> Vec<DbEvent>;

/// End the player's turn: discard hand, emit enemy actions (intents),
/// refill energy, clear block, draw a new hand.
pub fn end_turn(state: &mut DbState) -> Vec<DbEvent>;

pub enum DbEvent {
    PhaseChanged { from: DbPhase, to: DbPhase },
    CombatStarted,
    TurnStarted { turn: u32, energy: u8 },
    CardPlayed { card: CardId, target: Option<u32> },
    EnemyAction { enemy_id: u32, intent: EnemyIntent },
    EnemyDefeated { enemy_id: u32 },
    CombatWon, PlayerDefeated,
    RewardOffered { choices: Vec<RewardChoice> },
    RelicObtained { relic: RelicId },
    FloorAdvanced { floor: u32 },
}
```

## Behavior

- **Combat Loop**: `start_combat()` enters `DbPhase::Combat`, copies run HP into a fresh `CombatState`, and draws `hand_size` cards. `play_card()` rejects unaffordable cards silently (no events), otherwise spends energy, moves the card to the discard pile, and emits `CardPlayed`. When all enemies have `hp <= 0`, combat HP (current and max) is persisted back into `DbState` and the phase becomes `Reward`.
- **Turn Cycle**: `end_turn()` discards the whole hand into the deck, emits one `EnemyAction` per living enemy (the game layer applies the intent's effects), then increments the turn, refills energy to `max_energy`, resets block to 0, and draws a fresh hand.
- **Deck Cycling**: `Deck::draw()` automatically reshuffles the discard pile into the draw pile when empty. Exhausted cards never return. Shuffles are Fisher-Yates with xorshift64, so the same seed yields the same draw order.
- **Map**: `generate_map()` is deterministic per seed; each node connects to 1-2 nodes on the next floor. The final floor is always a single `Boss` node, floor 0 is always plain `Combat`.
- **Persistence**: All types are `Serialize`/`Deserialize`; a whole run (`DbState`) can be saved mid-combat.

## Internal Design

- `card.rs` is a self-contained, genre-agnostic card toolkit (registry, piles, hand); `deckbuilder.rs` layers the run structure (map, phases, relics, rewards) on top. Other genres can reuse `card.rs` alone.
- System functions are free functions `fn(&mut DbState, ...) -> Vec<DbEvent>`; the game layer interprets events for rendering, audio, and applying `CardEffect`/`EnemyIntent` to actual HP values.
- `Deck` keeps its own `rng_state` so deck shuffles do not perturb other seeded systems.

## Future Work

- Card effect resolution: `CardEffect` and `EnemyIntent` are data only — `play_card()` does not yet apply damage/block to `CombatState`; the game layer does this today.
- Relic trigger dispatch: `RelicDef` is data only; there is no engine-side hook that fires `RelicTrigger`s.
- Shop/Rest/Event phase logic (`DbPhase` variants exist, transitions are caller-driven), reward generation, and `EnemyDefeated`/`PlayerDefeated` emission.
- Status effects on enemies (integration with `status_effect.rs`).

## Referenzen

- Slay the Spire: energy economy, intents, exhaust pile, branching map
- Monster Train / Inscryption: deck reuse across genre variants
- [engine/core](../engine/core.md) → event flow conventions
- [engine/procedural](../engine/procedural.md) → seeded generation philosophy
- [engine/save-load](../engine/save-load.md) → mid-run save of DbState
