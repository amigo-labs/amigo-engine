use crate::card::{CardId, CardRegistry, Deck, Hand};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a deckbuilder run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeckbuilderConfig {
    pub starting_energy: u8,
    pub max_energy: u8,
    pub hand_size: u8,
    pub starting_deck: Vec<CardId>,
    pub starting_hp: i32,
    pub starting_max_hp: i32,
    pub seed: u64,
}

impl Default for DeckbuilderConfig {
    fn default() -> Self {
        Self {
            starting_energy: 3,
            max_energy: 3,
            hand_size: 5,
            starting_deck: Vec::new(),
            starting_hp: 50,
            starting_max_hp: 50,
            seed: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Map / node system
// ---------------------------------------------------------------------------

/// Kind of encounter at a map node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Combat,
    Elite,
    Boss,
    Shop,
    Rest,
    Event,
    Treasure,
}

/// A node on the run map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapNode {
    pub id: u32,
    pub kind: NodeKind,
    /// Indices of connected nodes on the next floor.
    pub connections: Vec<u32>,
}

/// Generate a procedural map with the given number of floors.
pub fn generate_map(floors: u32, nodes_per_floor: u32, seed: u64) -> Vec<Vec<MapNode>> {
    let mut map = Vec::new();
    let mut rng = seed;
    let mut next_id = 0u32;

    for floor in 0..floors {
        let mut nodes = Vec::new();
        let count = if floor == floors - 1 {
            1 // Boss floor
        } else {
            nodes_per_floor
        };

        for _ in 0..count {
            rng = xorshift64(rng);
            let kind = if floor == floors - 1 {
                NodeKind::Boss
            } else if floor == 0 {
                NodeKind::Combat
            } else {
                match rng % 7 {
                    0 => NodeKind::Combat,
                    1 => NodeKind::Combat,
                    2 => NodeKind::Elite,
                    3 => NodeKind::Shop,
                    4 => NodeKind::Rest,
                    5 => NodeKind::Event,
                    6 => NodeKind::Treasure,
                    _ => NodeKind::Combat,
                }
            };

            nodes.push(MapNode {
                id: next_id,
                kind,
                connections: Vec::new(),
            });
            next_id += 1;
        }
        map.push(nodes);
    }

    // Connect nodes between floors.
    for floor in 0..map.len().saturating_sub(1) {
        // Collect next floor IDs to avoid borrow conflict.
        let next_ids: Vec<u32> = map[floor + 1].iter().map(|n| n.id).collect();
        for node in &mut map[floor] {
            rng = xorshift64(rng);
            let conn_count = 1 + (rng % 2) as u32;
            for _ in 0..conn_count.min(next_ids.len() as u32) {
                rng = xorshift64(rng);
                let target_id = next_ids[rng as usize % next_ids.len()];
                if !node.connections.contains(&target_id) {
                    node.connections.push(target_id);
                }
            }
        }
    }

    map
}

// ---------------------------------------------------------------------------
// Relics
// ---------------------------------------------------------------------------

/// Unique relic identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelicId(pub u32);

/// When a relic triggers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelicTrigger {
    OnCombatStart,
    OnTurnStart,
    OnCardPlayed,
    OnDamageDealt,
    Passive,
}

/// What a relic does.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RelicEffect {
    GainEnergy(u8),
    DrawCards(u8),
    GainBlock(u32),
    DamageBoost(u32),
    HealOnRest(u32),
    Custom(String),
}

/// Definition of a relic.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelicDef {
    pub id: RelicId,
    pub name: String,
    pub trigger: RelicTrigger,
    pub effect: RelicEffect,
}

// ---------------------------------------------------------------------------
// Rewards
// ---------------------------------------------------------------------------

/// A reward choice after combat.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RewardChoice {
    AddCard(CardId),
    RemoveCard,
    Gold(u32),
    Relic(RelicId),
    Heal(u32),
}

// ---------------------------------------------------------------------------
// Phases and state
// ---------------------------------------------------------------------------

/// Phases of a deckbuilder run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DbPhase {
    MapSelect,
    Combat,
    Reward,
    Shop,
    Rest,
    Event,
    GameOver,
}

/// Combat state within a deckbuilder.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatState {
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub player_block: u32,
    pub energy: u8,
    pub max_energy: u8,
    pub enemies: Vec<EnemyState>,
    pub turn: u32,
}

/// A combat enemy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnemyState {
    pub id: u32,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub block: u32,
    /// Next intended action (for intent display).
    pub intent: EnemyIntent,
}

/// What the enemy plans to do next turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EnemyIntent {
    Attack(u32),
    Block(u32),
    Buff,
    Debuff,
    Unknown,
}

/// Top-level deckbuilder state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DbState {
    pub config: DeckbuilderConfig,
    pub phase: DbPhase,
    pub deck: Deck,
    pub hand: Hand,
    pub combat: Option<CombatState>,
    pub gold: u32,
    pub floor: u32,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub relics: Vec<RelicId>,
    pub map: Vec<Vec<MapNode>>,
    pub reward_choices: Vec<RewardChoice>,
}

impl DbState {
    pub fn new(config: DeckbuilderConfig) -> Self {
        let deck = Deck::new(config.starting_deck.clone(), config.seed);
        let hand = Hand::new(config.hand_size);
        let map = generate_map(15, 3, config.seed);

        Self {
            player_hp: config.starting_hp,
            player_max_hp: config.starting_max_hp,
            phase: DbPhase::MapSelect,
            deck,
            hand,
            combat: None,
            gold: 0,
            floor: 0,
            relics: Vec::new(),
            map,
            reward_choices: Vec::new(),
            config,
        }
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events from the deckbuilder system.
#[derive(Clone, Debug)]
pub enum DbEvent {
    PhaseChanged { from: DbPhase, to: DbPhase },
    CombatStarted,
    TurnStarted { turn: u32, energy: u8 },
    CardPlayed { card: CardId, target: Option<u32> },
    EnemyAction { enemy_id: u32, intent: EnemyIntent },
    EnemyDefeated { enemy_id: u32 },
    CombatWon,
    PlayerDefeated,
    RewardOffered { choices: Vec<RewardChoice> },
    RelicObtained { relic: RelicId },
    FloorAdvanced { floor: u32 },
}

// ---------------------------------------------------------------------------
// System functions
// ---------------------------------------------------------------------------

/// Start combat with the given enemies.
pub fn start_combat(state: &mut DbState, enemies: Vec<EnemyState>) -> Vec<DbEvent> {
    let mut events = Vec::new();
    let old = state.phase.clone();
    state.phase = DbPhase::Combat;

    state.combat = Some(CombatState {
        player_hp: state.player_hp,
        player_max_hp: state.player_max_hp,
        player_block: 0,
        energy: state.config.starting_energy,
        max_energy: state.config.max_energy,
        enemies,
        turn: 1,
    });

    // Draw starting hand.
    let drawn = state.deck.draw(state.config.hand_size);
    for card in drawn {
        state.hand.add(card);
    }

    events.push(DbEvent::PhaseChanged {
        from: old,
        to: DbPhase::Combat,
    });
    events.push(DbEvent::CombatStarted);
    events.push(DbEvent::TurnStarted {
        turn: 1,
        energy: state.config.starting_energy,
    });
    events
}

/// Play a card from hand by index.
pub fn play_card(
    state: &mut DbState,
    hand_index: usize,
    target: Option<u32>,
    registry: &CardRegistry,
) -> Vec<DbEvent> {
    let mut events = Vec::new();

    let combat = match &mut state.combat {
        Some(c) => c,
        None => return events,
    };

    let card_id = match state.hand.cards.get(hand_index) {
        Some(&id) => id,
        None => return events,
    };

    let cost = registry.get(card_id).map(|d| d.cost).unwrap_or(0);
    if combat.energy < cost {
        return events;
    }

    combat.energy -= cost;
    state.hand.remove(hand_index);
    state.deck.discard(card_id);

    events.push(DbEvent::CardPlayed {
        card: card_id,
        target,
    });

    // Check if all enemies dead.
    if let Some(ref combat) = state.combat {
        if combat.enemies.iter().all(|e| e.hp <= 0) {
            // Persist HP back to run state (both current and max, in case
            // relics/upgrades changed max_hp during combat).
            state.player_hp = combat.player_hp;
            state.player_max_hp = combat.player_max_hp;
            events.push(DbEvent::CombatWon);
            state.phase = DbPhase::Reward;
        }
    }

    events
}

/// End the player's turn.
pub fn end_turn(state: &mut DbState) -> Vec<DbEvent> {
    let mut events = Vec::new();

    // Discard hand.
    let discarded = state.hand.discard_all();
    for card in discarded {
        state.deck.discard(card);
    }

    // Enemy actions.
    if let Some(ref mut combat) = state.combat {
        for enemy in &combat.enemies {
            if enemy.hp > 0 {
                events.push(DbEvent::EnemyAction {
                    enemy_id: enemy.id,
                    intent: enemy.intent.clone(),
                });
            }
        }

        // New turn.
        combat.turn += 1;
        combat.energy = combat.max_energy;
        combat.player_block = 0;
    }

    // Draw new hand.
    let drawn = state.deck.draw(state.config.hand_size);
    for card in drawn {
        state.hand.add(card);
    }

    if let Some(ref combat) = state.combat {
        events.push(DbEvent::TurnStarted {
            turn: combat.turn,
            energy: combat.energy,
        });
    }

    events
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn xorshift64(mut s: u64) -> u64 {
    if s == 0 {
        s = 1;
    }
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::CardDef;

    #[test]
    fn map_generation_deterministic() {
        let map_a = generate_map(10, 3, 42);
        let map_b = generate_map(10, 3, 42);
        assert_eq!(map_a.len(), map_b.len());
        for (floor_a, floor_b) in map_a.iter().zip(map_b.iter()) {
            assert_eq!(floor_a.len(), floor_b.len());
            for (node_a, node_b) in floor_a.iter().zip(floor_b.iter()) {
                assert_eq!(node_a.id, node_b.id);
                assert_eq!(node_a.kind, node_b.kind);
                assert_eq!(node_a.connections, node_b.connections);
            }
        }
    }

    #[test]
    fn map_boss_on_last_floor() {
        let map = generate_map(10, 3, 123);
        assert_eq!(map.last().unwrap().len(), 1);
        assert_eq!(map.last().unwrap()[0].kind, NodeKind::Boss);
    }

    #[test]
    fn combat_flow() {
        let config = DeckbuilderConfig {
            starting_deck: (0..10).map(CardId).collect(),
            hand_size: 5,
            starting_energy: 3,
            seed: 42,
            ..Default::default()
        };
        let mut state = DbState::new(config);

        let enemies = vec![EnemyState {
            id: 0,
            name: "Slime".into(),
            hp: 20,
            max_hp: 20,
            block: 0,
            intent: EnemyIntent::Attack(6),
        }];

        let events = start_combat(&mut state, enemies);
        assert!(events.iter().any(|e| matches!(e, DbEvent::CombatStarted)));
        assert_eq!(state.hand.len(), 5);
        assert_eq!(state.phase, DbPhase::Combat);

        // End turn.
        let events = end_turn(&mut state);
        assert!(events
            .iter()
            .any(|e| matches!(e, DbEvent::TurnStarted { turn: 2, .. })));
    }

    #[test]
    fn state_creation() {
        let state = DbState::new(DeckbuilderConfig::default());
        assert_eq!(state.phase, DbPhase::MapSelect);
        assert_eq!(state.floor, 0);
        assert_eq!(state.player_hp, 50);
        assert_eq!(state.player_max_hp, 50);
        assert!(!state.map.is_empty());
    }

    #[test]
    fn persistent_hp_across_combats() {
        let config = DeckbuilderConfig {
            starting_deck: (0..10).map(CardId).collect(),
            starting_hp: 40,
            starting_max_hp: 50,
            seed: 42,
            ..Default::default()
        };
        let mut state = DbState::new(config);
        assert_eq!(state.player_hp, 40);

        let enemies = vec![EnemyState {
            id: 0,
            name: "Slime".into(),
            hp: 20,
            max_hp: 20,
            block: 0,
            intent: EnemyIntent::Attack(6),
        }];
        start_combat(&mut state, enemies);

        // Combat should use the persistent HP.
        let combat = state.combat.as_ref().unwrap();
        assert_eq!(combat.player_hp, 40);
        assert_eq!(combat.player_max_hp, 50);
    }

    fn make_registry_with_cost(id: u32, cost: u8) -> CardRegistry {
        let mut reg = CardRegistry::new();
        reg.register(CardDef {
            id: CardId(id),
            name: format!("Card{}", id),
            cost,
            rarity: crate::card::Rarity::Common,
            effects: vec![],
            tags: vec![],
            upgraded: false,
            target: crate::card::TargetKind::SingleEnemy,
        });
        reg
    }

    #[test]
    fn play_card_insufficient_energy() {
        // Card costs 5, but starting energy is 3
        let registry = make_registry_with_cost(0, 5);
        let config = DeckbuilderConfig {
            starting_deck: vec![CardId(0)],
            hand_size: 5,
            starting_energy: 3,
            seed: 42,
            ..Default::default()
        };
        let mut state = DbState::new(config);

        let enemies = vec![EnemyState {
            id: 0,
            name: "Slime".into(),
            hp: 20,
            max_hp: 20,
            block: 0,
            intent: EnemyIntent::Attack(6),
        }];
        start_combat(&mut state, enemies);

        let hand_before = state.hand.cards.clone();
        let events = play_card(&mut state, 0, Some(0), &registry);

        // No events emitted (card too expensive)
        assert!(events.is_empty());
        // Hand unchanged
        assert_eq!(state.hand.cards, hand_before);
        // Energy unchanged
        assert_eq!(state.combat.as_ref().unwrap().energy, 3);
    }

    #[test]
    fn combat_won_persists_hp_and_max_hp() {
        // Card costs 0 so we can always play it
        let registry = make_registry_with_cost(0, 0);
        let config = DeckbuilderConfig {
            starting_deck: vec![CardId(0)],
            hand_size: 5,
            starting_energy: 3,
            starting_hp: 35,
            starting_max_hp: 45,
            seed: 42,
            ..Default::default()
        };
        let mut state = DbState::new(config);

        // Enemy already dead (hp = 0)
        let enemies = vec![EnemyState {
            id: 0,
            name: "Dead Slime".into(),
            hp: 0,
            max_hp: 20,
            block: 0,
            intent: EnemyIntent::Unknown,
        }];
        start_combat(&mut state, enemies);

        // Modify combat HP to simulate damage taken during combat
        state.combat.as_mut().unwrap().player_hp = 30;
        state.combat.as_mut().unwrap().player_max_hp = 45;

        // Playing a card triggers the "all enemies dead" check
        let events = play_card(&mut state, 0, None, &registry);

        assert!(events.iter().any(|e| matches!(e, DbEvent::CombatWon)));
        assert_eq!(state.phase, DbPhase::Reward);
        // HP persisted from combat back to run state
        assert_eq!(state.player_hp, 30);
        assert_eq!(state.player_max_hp, 45);
    }
}
