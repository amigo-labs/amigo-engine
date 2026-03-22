use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique card definition identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardId(pub u32);

/// Card rarity tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
}

// ---------------------------------------------------------------------------
// Card targeting and effects
// ---------------------------------------------------------------------------

/// How a card selects its targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetKind {
    /// Targets the card player themselves.
    Self_,
    /// Targets a single enemy (player must choose).
    SingleEnemy,
    /// Targets all enemies.
    AllEnemies,
    /// Targets a single ally.
    SingleAlly,
    /// Targets all allies.
    AllAllies,
    /// Targets N random enemies.
    Random(u8),
}

/// An effect that a card applies when played.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CardEffect {
    Damage {
        amount: u32,
        damage_type: String,
    },
    Block(u32),
    Heal(u32),
    DrawCards(u8),
    GainEnergy(u8),
    ApplyStatus {
        effect: String,
        magnitude: f32,
        duration: f32,
    },
    Custom(String),
}

// ---------------------------------------------------------------------------
// Card definition
// ---------------------------------------------------------------------------

/// Definition of a card.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardDef {
    pub id: CardId,
    pub name: String,
    /// Energy cost to play.
    pub cost: u8,
    pub rarity: Rarity,
    /// Effects applied when the card is played.
    pub effects: Vec<CardEffect>,
    /// Tags for synergy detection (e.g. "attack", "skill", "power").
    pub tags: Vec<String>,
    /// Whether this is an upgraded version of the card.
    pub upgraded: bool,
    /// Targeting mode.
    pub target: TargetKind,
}

// ---------------------------------------------------------------------------
// Card registry
// ---------------------------------------------------------------------------

/// Registry holding all card definitions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CardRegistry {
    defs: FxHashMap<CardId, CardDef>,
}

impl CardRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: CardDef) {
        self.defs.insert(def.id, def);
    }

    pub fn get(&self, id: CardId) -> Option<&CardDef> {
        self.defs.get(&id)
    }

    pub fn by_rarity(&self, rarity: Rarity) -> Vec<&CardDef> {
        self.defs.values().filter(|d| d.rarity == rarity).collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&CardDef> {
        self.defs
            .values()
            .filter(|d| d.tags.iter().any(|t| t == tag))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Deck — draw, discard, exhaust piles
// ---------------------------------------------------------------------------

/// A deck with draw, discard, and exhaust piles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deck {
    draw_pile: Vec<CardId>,
    discard_pile: Vec<CardId>,
    exhaust_pile: Vec<CardId>,
    /// Simple xorshift RNG state for deterministic shuffling.
    rng_state: u64,
}

impl Deck {
    /// Create a new deck with the given cards in the draw pile.
    /// The deck is shuffled using the provided seed.
    pub fn new(cards: Vec<CardId>, seed: u64) -> Self {
        let mut deck = Self {
            draw_pile: cards,
            discard_pile: Vec::new(),
            exhaust_pile: Vec::new(),
            rng_state: seed,
        };
        deck.shuffle_draw_pile();
        deck
    }

    /// Draw N cards from the draw pile. If the draw pile runs out,
    /// the discard pile is shuffled back in.
    pub fn draw(&mut self, n: u8) -> Vec<CardId> {
        let mut drawn = Vec::with_capacity(n as usize);
        for _ in 0..n {
            if self.draw_pile.is_empty() {
                if self.discard_pile.is_empty() {
                    break;
                }
                self.shuffle_discard_into_draw();
            }
            if let Some(card) = self.draw_pile.pop() {
                drawn.push(card);
            }
        }
        drawn
    }

    /// Move a card to the discard pile.
    pub fn discard(&mut self, card: CardId) {
        self.discard_pile.push(card);
    }

    /// Move a card to the exhaust pile (removed from game).
    pub fn exhaust(&mut self, card: CardId) {
        self.exhaust_pile.push(card);
    }

    /// Shuffle the discard pile into the draw pile.
    pub fn shuffle_discard_into_draw(&mut self) {
        self.draw_pile.append(&mut self.discard_pile);
        self.shuffle_draw_pile();
    }

    /// Cards remaining in the draw pile.
    pub fn remaining(&self) -> usize {
        self.draw_pile.len()
    }

    /// Cards in the discard pile.
    pub fn discard_count(&self) -> usize {
        self.discard_pile.len()
    }

    /// Cards in the exhaust pile.
    pub fn exhaust_count(&self) -> usize {
        self.exhaust_pile.len()
    }

    /// Total cards across all piles.
    pub fn total(&self) -> usize {
        self.draw_pile.len() + self.discard_pile.len() + self.exhaust_pile.len()
    }

    /// Add a card to the draw pile (e.g. rewards).
    pub fn add_card(&mut self, card: CardId) {
        self.draw_pile.push(card);
    }

    fn shuffle_draw_pile(&mut self) {
        // Fisher-Yates shuffle with xorshift64.
        let len = self.draw_pile.len();
        for i in (1..len).rev() {
            self.rng_state = xorshift64(self.rng_state);
            let j = (self.rng_state as usize) % (i + 1);
            self.draw_pile.swap(i, j);
        }
    }
}

// ---------------------------------------------------------------------------
// Hand
// ---------------------------------------------------------------------------

/// A player's hand of cards.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Hand {
    pub cards: Vec<CardId>,
    pub max_size: u8,
}

impl Hand {
    pub fn new(max_size: u8) -> Self {
        Self {
            cards: Vec::new(),
            max_size,
        }
    }

    /// Add a card to the hand. Returns Some(card) if hand is full (overflow).
    pub fn add(&mut self, card: CardId) -> Option<CardId> {
        if self.cards.len() >= self.max_size as usize {
            return Some(card);
        }
        self.cards.push(card);
        None
    }

    /// Remove a card from the hand by index. Returns None if index invalid.
    pub fn remove(&mut self, index: usize) -> Option<CardId> {
        if index >= self.cards.len() {
            return None;
        }
        Some(self.cards.remove(index))
    }

    pub fn is_full(&self) -> bool {
        self.cards.len() >= self.max_size as usize
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Discard all cards, returning them.
    pub fn discard_all(&mut self) -> Vec<CardId> {
        std::mem::take(&mut self.cards)
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events produced by the card system.
#[derive(Clone, Debug)]
pub enum CardEvent {
    Drawn { card: CardId },
    Played { card: CardId, target: TargetKind },
    Discarded { card: CardId },
    Exhausted { card: CardId },
    DeckShuffled,
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn xorshift64(mut state: u64) -> u64 {
    if state == 0 {
        state = 1;
    }
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cards() -> Vec<CardId> {
        (0..10).map(CardId).collect()
    }

    #[test]
    fn deck_draw_and_discard() {
        let mut deck = Deck::new(test_cards(), 42);
        assert_eq!(deck.remaining(), 10);

        let drawn = deck.draw(3);
        assert_eq!(drawn.len(), 3);
        assert_eq!(deck.remaining(), 7);

        for card in drawn {
            deck.discard(card);
        }
        assert_eq!(deck.discard_count(), 3);
    }

    #[test]
    fn deck_reshuffles_on_empty() {
        let mut deck = Deck::new(vec![CardId(1), CardId(2)], 42);
        let _ = deck.draw(2);
        assert_eq!(deck.remaining(), 0);

        deck.discard(CardId(1));
        deck.discard(CardId(2));

        let drawn = deck.draw(1);
        assert_eq!(drawn.len(), 1);
        assert_eq!(deck.remaining(), 1); // 1 left in draw after reshuffle
    }

    #[test]
    fn deck_exhaust_removes_permanently() {
        let mut deck = Deck::new(vec![CardId(1), CardId(2), CardId(3)], 42);
        let drawn = deck.draw(3);
        deck.exhaust(drawn[0]);
        deck.discard(drawn[1]);
        deck.discard(drawn[2]);

        assert_eq!(deck.exhaust_count(), 1);
        assert_eq!(deck.total(), 3);

        deck.shuffle_discard_into_draw();
        assert_eq!(deck.remaining(), 2); // Only 2, exhausted card is gone
    }

    #[test]
    fn deck_deterministic_shuffle() {
        let cards = test_cards();
        let deck_a = Deck::new(cards.clone(), 123);
        let deck_b = Deck::new(cards, 123);
        assert_eq!(deck_a.draw_pile, deck_b.draw_pile);
    }

    #[test]
    fn hand_overflow() {
        let mut hand = Hand::new(3);
        assert!(hand.add(CardId(1)).is_none());
        assert!(hand.add(CardId(2)).is_none());
        assert!(hand.add(CardId(3)).is_none());
        // Overflow.
        assert_eq!(hand.add(CardId(4)), Some(CardId(4)));
        assert!(hand.is_full());
    }

    #[test]
    fn hand_remove() {
        let mut hand = Hand::new(5);
        hand.add(CardId(10));
        hand.add(CardId(20));
        hand.add(CardId(30));

        assert_eq!(hand.remove(1), Some(CardId(20)));
        assert_eq!(hand.len(), 2);
        assert!(hand.remove(5).is_none());
    }

    #[test]
    fn hand_discard_all() {
        let mut hand = Hand::new(5);
        hand.add(CardId(1));
        hand.add(CardId(2));
        let discarded = hand.discard_all();
        assert_eq!(discarded.len(), 2);
        assert!(hand.is_empty());
    }

    #[test]
    fn registry_by_rarity() {
        let mut reg = CardRegistry::new();
        reg.register(CardDef {
            id: CardId(1),
            name: "Strike".into(),
            cost: 1,
            rarity: Rarity::Common,
            effects: vec![CardEffect::Damage {
                amount: 6,
                damage_type: "physical".into(),
            }],
            tags: vec!["attack".into()],
            upgraded: false,
            target: TargetKind::SingleEnemy,
        });
        reg.register(CardDef {
            id: CardId(2),
            name: "Inferno".into(),
            cost: 3,
            rarity: Rarity::Rare,
            effects: vec![],
            tags: vec!["attack".into()],
            upgraded: false,
            target: TargetKind::AllEnemies,
        });

        assert_eq!(reg.by_rarity(Rarity::Common).len(), 1);
        assert_eq!(reg.by_tag("attack").len(), 2);
    }
}
