use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for an auto-battler game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutoBattlerConfig {
    pub grid_width: u8,
    pub grid_height: u8,
    pub bench_size: u8,
    pub shop_size: u8,
    pub starting_gold: u32,
    pub gold_per_round: u32,
    pub interest_rate: f32,
    pub max_interest: u32,
    pub reroll_cost: u32,
    pub seed: u64,
}

impl Default for AutoBattlerConfig {
    fn default() -> Self {
        Self {
            grid_width: 7,
            grid_height: 4,
            bench_size: 8,
            shop_size: 5,
            starting_gold: 10,
            gold_per_round: 5,
            interest_rate: 0.1,
            max_interest: 5,
            reroll_cost: 2,
            seed: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Unit definitions
// ---------------------------------------------------------------------------

/// Unique unit type identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnitId(pub u32);

/// Unique trait identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraitId(pub u32);

/// Base stats for a unit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitStats {
    pub hp: i32,
    pub attack: i32,
    pub attack_speed: f32,
    pub range: u8,
    pub armor: i32,
}

/// Definition of a unit type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitDef {
    pub id: UnitId,
    pub name: String,
    pub tier: u8,
    pub traits: Vec<TraitId>,
    pub base_stats: UnitStats,
    pub cost: u32,
}

/// A live unit on the board or bench.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnitInstance {
    pub def_id: UnitId,
    /// 1, 2, or 3 stars.
    pub star_level: u8,
    /// Grid position (None if on bench).
    pub position: Option<(u8, u8)>,
    /// Current stats (scaled by star level).
    pub stats: UnitStats,
    /// Current HP in combat.
    pub current_hp: i32,
}

// ---------------------------------------------------------------------------
// Traits / synergies
// ---------------------------------------------------------------------------

/// A bonus granted by a trait at a specific threshold.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraitBonus {
    pub attack_bonus: i32,
    pub hp_bonus: i32,
    pub armor_bonus: i32,
}

/// Definition of a synergy trait.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraitDef {
    pub id: TraitId,
    pub name: String,
    /// (count_required, bonus) pairs, sorted ascending.
    pub thresholds: Vec<(u8, TraitBonus)>,
}

// ---------------------------------------------------------------------------
// Shop
// ---------------------------------------------------------------------------

/// The shop that appears between rounds.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Shop {
    pub slots: Vec<Option<UnitId>>,
    pub frozen: bool,
}

impl Shop {
    pub fn new(size: u8) -> Self {
        Self {
            slots: vec![None; size as usize],
            frozen: false,
        }
    }

    /// Reroll the shop. Returns false if not enough gold.
    pub fn reroll(&mut self, gold: &mut u32, cost: u32, pool: &[UnitId], rng: &mut u64) -> bool {
        if *gold < cost {
            return false;
        }
        *gold -= cost;
        for slot in &mut self.slots {
            *rng = xorshift64(*rng);
            if pool.is_empty() {
                *slot = None;
            } else {
                *slot = Some(pool[(*rng as usize) % pool.len()]);
            }
        }
        true
    }

    /// Buy a unit from a slot. Returns the UnitId if successful.
    pub fn buy(&mut self, slot: usize, gold: &mut u32, unit_cost: u32) -> Option<UnitId> {
        if slot >= self.slots.len() {
            return None;
        }
        let unit = self.slots[slot].take()?;
        if *gold < unit_cost {
            self.slots[slot] = Some(unit);
            return None;
        }
        *gold -= unit_cost;
        Some(unit)
    }

    /// Toggle freeze on the shop.
    pub fn toggle_freeze(&mut self) {
        self.frozen = !self.frozen;
    }
}

// ---------------------------------------------------------------------------
// Phases and state
// ---------------------------------------------------------------------------

/// Phases of an auto-battler round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoundPhase {
    Shop,
    Place,
    Combat,
    Results,
}

/// Result of a combat round.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatResult {
    pub won: bool,
    pub damage_taken: u32,
    pub surviving_units: u32,
}

/// Top-level auto-battler state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbState {
    pub config: AutoBattlerConfig,
    pub phase: RoundPhase,
    pub round: u32,
    pub gold: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub board: Vec<UnitInstance>,
    pub bench: Vec<UnitInstance>,
    pub shop: Shop,
    pub win_streak: u32,
    pub loss_streak: u32,
    pub rng_state: u64,
}

impl AbState {
    pub fn new(config: AutoBattlerConfig) -> Self {
        let shop = Shop::new(config.shop_size);
        let gold = config.starting_gold;
        let seed = config.seed;
        Self {
            phase: RoundPhase::Shop,
            round: 1,
            gold,
            hp: 100,
            max_hp: 100,
            board: Vec::new(),
            bench: Vec::new(),
            shop,
            win_streak: 0,
            loss_streak: 0,
            rng_state: seed,
            config,
        }
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events from the auto-battler system.
#[derive(Clone, Debug)]
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

// ---------------------------------------------------------------------------
// System functions
// ---------------------------------------------------------------------------

/// Calculate gold income for a new round.
pub fn calculate_income(state: &AbState) -> (u32, u32) {
    let base = state.config.gold_per_round;
    let streak_bonus = state.win_streak.max(state.loss_streak).min(3);
    let interest = ((state.gold as f32 * state.config.interest_rate).floor() as u32)
        .min(state.config.max_interest);
    (base + streak_bonus + interest, interest)
}

/// Start a new round (shop phase).
pub fn start_round(state: &mut AbState) -> Vec<AbEvent> {
    let mut events = Vec::new();
    let old = state.phase;
    state.phase = RoundPhase::Shop;

    let (income, interest) = calculate_income(state);
    state.gold += income;

    events.push(AbEvent::PhaseChanged {
        from: old,
        to: RoundPhase::Shop,
    });
    events.push(AbEvent::RoundStarted { round: state.round });
    events.push(AbEvent::GoldReceived {
        amount: income,
        interest,
    });
    events
}

/// Resolve combat between two teams deterministically.
pub fn resolve_combat(
    team_a: &mut [UnitInstance],
    team_b: &mut [UnitInstance],
    seed: u64,
) -> CombatResult {
    let mut rng = seed;

    // Simple simulation: units attack in order until one side is eliminated.
    let max_rounds = 100;
    for _ in 0..max_rounds {
        // Team A attacks: collect attacker stats first to avoid borrow issues.
        let attackers_a: Vec<(i32, bool)> = team_a
            .iter()
            .map(|u| (u.stats.attack, u.current_hp > 0))
            .collect();
        for (atk, alive) in &attackers_a {
            if !alive {
                continue;
            }
            rng = xorshift64(rng);
            let alive_b: Vec<usize> = team_b
                .iter()
                .enumerate()
                .filter(|(_, u)| u.current_hp > 0)
                .map(|(idx, _)| idx)
                .collect();
            if alive_b.is_empty() {
                break;
            }
            let target = alive_b[rng as usize % alive_b.len()];
            let damage = (*atk - team_b[target].stats.armor).max(1);
            team_b[target].current_hp -= damage;
        }

        if team_b.iter().all(|u| u.current_hp <= 0) {
            return CombatResult {
                won: true,
                damage_taken: 0,
                surviving_units: team_a.iter().filter(|u| u.current_hp > 0).count() as u32,
            };
        }

        // Team B attacks.
        let attackers_b: Vec<(i32, bool)> = team_b
            .iter()
            .map(|u| (u.stats.attack, u.current_hp > 0))
            .collect();
        for (atk, alive) in &attackers_b {
            if !alive {
                continue;
            }
            rng = xorshift64(rng);
            let alive_a: Vec<usize> = team_a
                .iter()
                .enumerate()
                .filter(|(_, u)| u.current_hp > 0)
                .map(|(idx, _)| idx)
                .collect();
            if alive_a.is_empty() {
                break;
            }
            let target = alive_a[rng as usize % alive_a.len()];
            let damage = (*atk - team_a[target].stats.armor).max(1);
            team_a[target].current_hp -= damage;
        }

        if team_a.iter().all(|u| u.current_hp <= 0) {
            let surviving_b = team_b.iter().filter(|u| u.current_hp > 0).count() as u32;
            return CombatResult {
                won: false,
                damage_taken: surviving_b + state_round_bonus(0),
                surviving_units: 0,
            };
        }
    }

    // Timeout = draw.
    CombatResult {
        won: false,
        damage_taken: 0,
        surviving_units: team_a.iter().filter(|u| u.current_hp > 0).count() as u32,
    }
}

/// Try to merge 3 copies of the same unit into a star upgrade.
pub fn try_merge(units: &mut Vec<UnitInstance>, def_id: UnitId) -> Option<u8> {
    let matching: Vec<usize> = units
        .iter()
        .enumerate()
        .filter(|(_, u)| u.def_id == def_id && u.star_level < 3)
        .map(|(i, _)| i)
        .collect();

    // Group by star level.
    let star1: Vec<usize> = matching
        .iter()
        .filter(|&&i| units[i].star_level == 1)
        .copied()
        .collect();
    let star2: Vec<usize> = matching
        .iter()
        .filter(|&&i| units[i].star_level == 2)
        .copied()
        .collect();

    if star1.len() >= 3 {
        // Remove 2, upgrade 1.
        let mut to_remove = vec![star1[1], star1[2]];
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            units.remove(idx);
        }
        // Adjust keep index if needed.
        if let Some(unit) = units
            .iter_mut()
            .find(|u| u.def_id == def_id && u.star_level == 1)
        {
            unit.star_level = 2;
            unit.stats.hp = (unit.stats.hp as f32 * 1.8) as i32;
            unit.stats.attack = (unit.stats.attack as f32 * 1.5) as i32;
            return Some(2);
        }
    }

    if star2.len() >= 3 {
        let mut to_remove = vec![star2[1], star2[2]];
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in to_remove {
            units.remove(idx);
        }
        if let Some(unit) = units
            .iter_mut()
            .find(|u| u.def_id == def_id && u.star_level == 2)
        {
            unit.star_level = 3;
            unit.stats.hp = (unit.stats.hp as f32 * 1.8) as i32;
            unit.stats.attack = (unit.stats.attack as f32 * 1.5) as i32;
            return Some(3);
        }
    }

    None
}

/// Calculate active synergies from units on the board.
pub fn calculate_synergies(
    board: &[UnitInstance],
    unit_registry: &FxHashMap<UnitId, UnitDef>,
    trait_registry: &FxHashMap<TraitId, TraitDef>,
) -> Vec<(TraitId, u8, Option<TraitBonus>)> {
    // Count traits.
    let mut trait_counts: FxHashMap<TraitId, u8> = FxHashMap::default();
    for unit in board {
        if unit.current_hp <= 0 {
            continue;
        }
        if let Some(def) = unit_registry.get(&unit.def_id) {
            for &trait_id in &def.traits {
                *trait_counts.entry(trait_id).or_insert(0) += 1;
            }
        }
    }

    // Find active bonuses.
    let mut result = Vec::new();
    for (trait_id, count) in &trait_counts {
        if let Some(trait_def) = trait_registry.get(trait_id) {
            let bonus = trait_def
                .thresholds
                .iter()
                .rev()
                .find(|(threshold, _)| count >= threshold)
                .map(|(_, bonus)| bonus.clone());
            result.push((*trait_id, *count, bonus));
        }
    }
    result
}

fn state_round_bonus(_round: u32) -> u32 {
    // In a full implementation, damage scales with round.
    1
}

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

    fn test_unit(id: u32, hp: i32, attack: i32) -> UnitInstance {
        UnitInstance {
            def_id: UnitId(id),
            star_level: 1,
            position: Some((0, 0)),
            stats: UnitStats {
                hp,
                attack,
                attack_speed: 1.0,
                range: 1,
                armor: 0,
            },
            current_hp: hp,
        }
    }

    #[test]
    fn income_calculation() {
        let state = AbState::new(AutoBattlerConfig {
            gold_per_round: 5,
            interest_rate: 0.1,
            max_interest: 5,
            ..Default::default()
        });
        let (income, interest) = calculate_income(&state);
        assert_eq!(interest, 1); // 10 gold * 0.1 = 1
        assert_eq!(income, 5 + 0 + 1); // base + streak + interest
    }

    #[test]
    fn combat_deterministic() {
        let mut team_a = vec![test_unit(1, 50, 10)];
        let mut team_b = vec![test_unit(2, 30, 5)];
        let result = resolve_combat(&mut team_a, &mut team_b, 42);
        assert!(result.won);
    }

    #[test]
    fn merge_three_units() {
        let mut units = vec![
            test_unit(1, 50, 10),
            test_unit(1, 50, 10),
            test_unit(1, 50, 10),
        ];
        let result = try_merge(&mut units, UnitId(1));
        assert_eq!(result, Some(2));
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].star_level, 2);
    }

    #[test]
    fn shop_buy_and_reroll() {
        let pool = vec![UnitId(1), UnitId(2), UnitId(3)];
        let mut shop = Shop::new(3);
        let mut gold = 20u32;
        let mut rng = 42u64;

        assert!(shop.reroll(&mut gold, 2, &pool, &mut rng));
        assert_eq!(gold, 18);
        assert!(shop.slots.iter().any(|s| s.is_some()));

        // Buy from first non-empty slot.
        let slot = shop.slots.iter().position(|s| s.is_some()).unwrap();
        let bought = shop.buy(slot, &mut gold, 3);
        assert!(bought.is_some());
        assert_eq!(gold, 15);
    }

    #[test]
    fn synergy_calculation() {
        let mut unit_reg = FxHashMap::default();
        unit_reg.insert(
            UnitId(1),
            UnitDef {
                id: UnitId(1),
                name: "Warrior".into(),
                tier: 1,
                traits: vec![TraitId(10)],
                base_stats: UnitStats {
                    hp: 50,
                    attack: 10,
                    attack_speed: 1.0,
                    range: 1,
                    armor: 0,
                },
                cost: 1,
            },
        );

        let mut trait_reg = FxHashMap::default();
        trait_reg.insert(
            TraitId(10),
            TraitDef {
                id: TraitId(10),
                name: "Warrior".into(),
                thresholds: vec![
                    (
                        2,
                        TraitBonus {
                            attack_bonus: 5,
                            hp_bonus: 0,
                            armor_bonus: 0,
                        },
                    ),
                    (
                        4,
                        TraitBonus {
                            attack_bonus: 15,
                            hp_bonus: 0,
                            armor_bonus: 0,
                        },
                    ),
                ],
            },
        );

        let board = vec![
            test_unit(1, 50, 10),
            test_unit(1, 50, 10),
            test_unit(1, 50, 10),
        ];
        let synergies = calculate_synergies(&board, &unit_reg, &trait_reg);
        assert_eq!(synergies.len(), 1);
        assert_eq!(synergies[0].1, 3); // 3 warriors
        assert!(synergies[0].2.is_some()); // Threshold 2 activated
        assert_eq!(synergies[0].2.as_ref().unwrap().attack_bonus, 5);
    }
}
