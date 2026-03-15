use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Economy events
// ---------------------------------------------------------------------------

/// An economy transaction that was processed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub kind: TransactionKind,
    pub amount: i32,
    pub balance_after: i32,
    pub tick: u64,
}

/// What caused the transaction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TransactionKind {
    /// Starting gold.
    Initial,
    /// Bounty from killing an enemy.
    EnemyBounty { enemy_type: u32 },
    /// Bonus for completing a wave.
    WaveBonus { wave: usize },
    /// Tower placement cost.
    TowerPlace { tower_type: u32 },
    /// Tower upgrade cost.
    TowerUpgrade { tower_id: u32 },
    /// Tower sell refund.
    TowerSell { tower_id: u32 },
    /// Interest on current gold (e.g. 1% per wave).
    Interest,
    /// Generic / custom.
    Custom { tag: String },
}

// ---------------------------------------------------------------------------
// Economy manager
// ---------------------------------------------------------------------------

/// Manages player gold, lives, and score for a tower defense game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Economy {
    pub gold: i32,
    pub lives: i32,
    pub max_lives: i32,
    pub score: u64,

    /// Interest rate applied per wave (0.0 = disabled).
    pub interest_rate: f32,
    /// Maximum gold that earns interest.
    pub interest_cap: i32,

    /// Transaction history (for replay, undo, and UI).
    history: Vec<Transaction>,
    /// Current game tick (set externally via `set_tick`).
    tick: u64,
}

impl Economy {
    pub fn new(starting_gold: i32, starting_lives: i32) -> Self {
        let mut eco = Self {
            gold: 0,
            lives: starting_lives,
            max_lives: starting_lives,
            score: 0,
            interest_rate: 0.0,
            interest_cap: 0,
            history: Vec::new(),
            tick: 0,
        };
        eco.add_gold(starting_gold, TransactionKind::Initial);
        eco
    }

    /// Set the current game tick (call each frame/tick before processing).
    pub fn set_tick(&mut self, tick: u64) {
        self.tick = tick;
    }

    // -- Gold ---------------------------------------------------------------

    /// Add gold and record the transaction. Returns new balance.
    pub fn add_gold(&mut self, amount: i32, kind: TransactionKind) -> i32 {
        self.gold += amount;
        self.history.push(Transaction {
            kind,
            amount,
            balance_after: self.gold,
            tick: self.tick,
        });
        self.gold
    }

    /// Try to spend gold. Returns `true` if the purchase succeeded.
    pub fn try_spend(&mut self, cost: i32, kind: TransactionKind) -> bool {
        if cost <= 0 || self.gold >= cost {
            self.gold -= cost;
            self.history.push(Transaction {
                kind,
                amount: -cost,
                balance_after: self.gold,
                tick: self.tick,
            });
            true
        } else {
            false
        }
    }

    /// Check if the player can afford a given cost.
    pub fn can_afford(&self, cost: i32) -> bool {
        self.gold >= cost
    }

    /// Apply interest (call at the end of each wave).
    pub fn apply_interest(&mut self) {
        if self.interest_rate <= 0.0 {
            return;
        }
        let base = if self.interest_cap > 0 {
            self.gold.min(self.interest_cap)
        } else {
            self.gold
        };
        let bonus = (base as f32 * self.interest_rate) as i32;
        if bonus > 0 {
            self.add_gold(bonus, TransactionKind::Interest);
        }
    }

    // -- Lives --------------------------------------------------------------

    /// Lose lives (enemy reached exit). Returns remaining lives.
    pub fn lose_lives(&mut self, amount: i32) -> i32 {
        self.lives = (self.lives - amount).max(0);
        self.lives
    }

    /// Heal lives (bonus). Capped at max_lives.
    pub fn heal_lives(&mut self, amount: i32) -> i32 {
        self.lives = (self.lives + amount).min(self.max_lives);
        self.lives
    }

    /// Check if the player is defeated.
    pub fn is_defeated(&self) -> bool {
        self.lives <= 0
    }

    // -- Score --------------------------------------------------------------

    /// Add to score.
    pub fn add_score(&mut self, points: u64) {
        self.score += points;
    }

    // -- History ------------------------------------------------------------

    /// Transaction history (most recent last).
    pub fn history(&self) -> &[Transaction] {
        &self.history
    }

    /// Last N transactions.
    pub fn recent_transactions(&self, n: usize) -> &[Transaction] {
        let start = self.history.len().saturating_sub(n);
        &self.history[start..]
    }

    /// Clear transaction history (e.g. on new game).
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_economy() {
        let mut eco = Economy::new(100, 20);
        assert_eq!(eco.gold, 100);
        assert_eq!(eco.lives, 20);

        // Spend gold
        assert!(eco.try_spend(50, TransactionKind::TowerPlace { tower_type: 1 }));
        assert_eq!(eco.gold, 50);

        // Can't overspend
        assert!(!eco.try_spend(100, TransactionKind::TowerPlace { tower_type: 2 }));
        assert_eq!(eco.gold, 50);

        // Earn bounty
        eco.add_gold(25, TransactionKind::EnemyBounty { enemy_type: 1 });
        assert_eq!(eco.gold, 75);
    }

    #[test]
    fn lives_system() {
        let mut eco = Economy::new(0, 20);
        eco.lose_lives(5);
        assert_eq!(eco.lives, 15);
        assert!(!eco.is_defeated());

        eco.lose_lives(15);
        assert!(eco.is_defeated());

        // Can't go below 0
        eco.lose_lives(10);
        assert_eq!(eco.lives, 0);
    }

    #[test]
    fn interest() {
        let mut eco = Economy::new(100, 20);
        eco.interest_rate = 0.1;
        eco.interest_cap = 200;

        eco.apply_interest();
        assert_eq!(eco.gold, 110);
    }

    #[test]
    fn transaction_history() {
        let mut eco = Economy::new(100, 20);
        eco.try_spend(30, TransactionKind::TowerPlace { tower_type: 1 });
        eco.add_gold(10, TransactionKind::EnemyBounty { enemy_type: 1 });

        // Initial + spend + bounty = 3
        assert_eq!(eco.history().len(), 3);

        let recent = eco.recent_transactions(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].amount, -30);
        assert_eq!(recent[1].amount, 10);
    }

    #[test]
    fn heal_lives_capped() {
        let mut eco = Economy::new(0, 20);
        eco.lose_lives(10);
        eco.heal_lives(100);
        assert_eq!(eco.lives, 20); // capped at max
    }
}
