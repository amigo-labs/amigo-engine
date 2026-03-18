use crate::ecs::EntityId;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Turn-based combat: Actions, Combatants, Turn Order
// ---------------------------------------------------------------------------

/// An action a combatant can take during their turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TurnAction {
    /// Attack a single target.
    Attack { target: usize },
    /// Use an ability/skill on a target (or self).
    Skill { skill_id: u32, target: usize },
    /// Use an item from inventory.
    UseItem { item_id: u32, target: usize },
    /// Defend (reduce incoming damage this round).
    Defend,
    /// Flee from battle.
    Flee,
    /// Switch party member (Pokemon-style).
    Switch { slot: usize },
    /// Do nothing (stunned, asleep, etc).
    Skip,
}

/// Result of executing an action.
#[derive(Clone, Debug)]
pub struct ActionResult {
    pub actor: usize,
    pub action: TurnAction,
    pub effects: Vec<BattleEffect>,
}

/// An effect that happened during battle (for animation/UI).
#[derive(Clone, Debug)]
pub enum BattleEffect {
    Damage {
        target: usize,
        amount: i32,
        is_critical: bool,
        element: Element,
    },
    Heal {
        target: usize,
        amount: i32,
    },
    StatusApplied {
        target: usize,
        status: StatusEffect,
    },
    StatusRemoved {
        target: usize,
        status: StatusType,
    },
    Fainted {
        target: usize,
    },
    Miss {
        target: usize,
    },
    Fled,
    FleeBlocked,
    Switched {
        slot: usize,
    },
    LevelUp {
        combatant: usize,
        new_level: u32,
    },
    ExpGained {
        amount: u32,
    },
}

// ---------------------------------------------------------------------------
// Elements (type effectiveness)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Element {
    Normal,
    Fire,
    Water,
    Grass,
    Electric,
    Ice,
    Earth,
    Wind,
    Light,
    Dark,
    Poison,
}

/// Type effectiveness multiplier.
pub fn type_effectiveness(attack: Element, defend: Element) -> f32 {
    match (attack, defend) {
        // Classic rock-paper-scissors triangle
        (Element::Fire, Element::Grass) => 2.0,
        (Element::Fire, Element::Ice) => 2.0,
        (Element::Fire, Element::Water) => 0.5,
        (Element::Fire, Element::Earth) => 0.5,

        (Element::Water, Element::Fire) => 2.0,
        (Element::Water, Element::Earth) => 2.0,
        (Element::Water, Element::Grass) => 0.5,
        (Element::Water, Element::Electric) => 0.5,

        (Element::Grass, Element::Water) => 2.0,
        (Element::Grass, Element::Earth) => 2.0,
        (Element::Grass, Element::Fire) => 0.5,
        (Element::Grass, Element::Ice) => 0.5,

        (Element::Electric, Element::Water) => 2.0,
        (Element::Electric, Element::Wind) => 2.0,
        (Element::Electric, Element::Earth) => 0.0, // immune
        (Element::Electric, Element::Grass) => 0.5,

        (Element::Ice, Element::Grass) => 2.0,
        (Element::Ice, Element::Earth) => 2.0,
        (Element::Ice, Element::Wind) => 2.0,
        (Element::Ice, Element::Fire) => 0.5,
        (Element::Ice, Element::Water) => 0.5,

        (Element::Earth, Element::Fire) => 2.0,
        (Element::Earth, Element::Electric) => 2.0,
        (Element::Earth, Element::Poison) => 2.0,
        (Element::Earth, Element::Wind) => 0.0, // immune (flying)

        (Element::Wind, Element::Grass) => 2.0,
        (Element::Wind, Element::Earth) => 2.0,
        (Element::Wind, Element::Electric) => 0.5,
        (Element::Wind, Element::Ice) => 0.5,

        (Element::Light, Element::Dark) => 2.0,
        (Element::Light, Element::Light) => 0.5,
        (Element::Dark, Element::Light) => 2.0,
        (Element::Dark, Element::Dark) => 0.5,

        (Element::Poison, Element::Grass) => 2.0,
        (Element::Poison, Element::Earth) => 0.5,
        (Element::Poison, Element::Poison) => 0.5,

        _ => 1.0,
    }
}

// ---------------------------------------------------------------------------
// Status effects
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusType {
    Poison,
    Burn,
    Freeze,
    Paralyze,
    Sleep,
    Confused,
    AttackUp,
    AttackDown,
    DefenseUp,
    DefenseDown,
    SpeedUp,
    SpeedDown,
    Regen,
    Shield,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusEffect {
    pub status_type: StatusType,
    /// Turns remaining (0 = permanent until cured).
    pub turns_remaining: u32,
    /// Magnitude (e.g. poison damage per turn, stat modifier amount).
    pub magnitude: i32,
}

impl StatusEffect {
    pub fn new(status_type: StatusType, turns: u32, magnitude: i32) -> Self {
        Self {
            status_type,
            turns_remaining: turns,
            magnitude,
        }
    }

    pub fn is_debuff(&self) -> bool {
        matches!(
            self.status_type,
            StatusType::Poison
                | StatusType::Burn
                | StatusType::Freeze
                | StatusType::Paralyze
                | StatusType::Sleep
                | StatusType::Confused
                | StatusType::AttackDown
                | StatusType::DefenseDown
                | StatusType::SpeedDown
        )
    }

    pub fn prevents_action(&self) -> bool {
        matches!(
            self.status_type,
            StatusType::Freeze | StatusType::Paralyze | StatusType::Sleep
        )
    }
}

// ---------------------------------------------------------------------------
// Skill definition
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SkillTarget {
    /// Single enemy.
    SingleEnemy,
    /// All enemies.
    AllEnemies,
    /// Single ally.
    SingleAlly,
    /// All allies.
    AllAllies,
    /// Self only.
    SelfOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillDef {
    pub id: u32,
    pub name: String,
    pub element: Element,
    pub power: i32,
    pub accuracy: u32,
    pub cost: i32,
    pub target: SkillTarget,
    pub status_chance: Option<(StatusType, u32, u32)>, // (type, chance%, turns)
    pub description: String,
}

// ---------------------------------------------------------------------------
// Combatant
// ---------------------------------------------------------------------------

/// Stats for a turn-based combatant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatantStats {
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub level: u32,
    pub exp: u32,
    pub element: Element,
}

impl CombatantStats {
    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    pub fn hp_fraction(&self) -> f32 {
        if self.max_hp <= 0 {
            0.0
        } else {
            self.hp as f32 / self.max_hp as f32
        }
    }
}

/// A combatant in battle.
#[derive(Clone, Debug)]
pub struct Combatant {
    pub name: String,
    pub stats: CombatantStats,
    pub skills: Vec<u32>,
    pub statuses: Vec<StatusEffect>,
    pub entity: Option<EntityId>,
    pub is_defending: bool,
    /// Team index (0 = player party, 1 = enemy party).
    pub team: u8,
}

impl Combatant {
    pub fn new(name: impl Into<String>, stats: CombatantStats, team: u8) -> Self {
        Self {
            name: name.into(),
            stats,
            skills: Vec::new(),
            statuses: Vec::new(),
            entity: None,
            is_defending: false,
            team,
        }
    }

    pub fn with_skills(mut self, skills: Vec<u32>) -> Self {
        self.skills = skills;
        self
    }

    pub fn is_alive(&self) -> bool {
        self.stats.is_alive()
    }

    pub fn has_status(&self, status_type: StatusType) -> bool {
        self.statuses.iter().any(|s| s.status_type == status_type)
    }

    pub fn can_act(&self) -> bool {
        self.is_alive() && !self.statuses.iter().any(|s| s.prevents_action())
    }

    /// Get effective stat with status modifiers.
    pub fn effective_attack(&self) -> i32 {
        let mut val = self.stats.attack;
        for s in &self.statuses {
            match s.status_type {
                StatusType::AttackUp => val += s.magnitude,
                StatusType::AttackDown => val -= s.magnitude,
                _ => {}
            }
        }
        val.max(1)
    }

    pub fn effective_defense(&self) -> i32 {
        let mut val = self.stats.defense;
        if self.is_defending {
            val = (val as f32 * 1.5) as i32;
        }
        for s in &self.statuses {
            match s.status_type {
                StatusType::DefenseUp => val += s.magnitude,
                StatusType::DefenseDown => val -= s.magnitude,
                _ => {}
            }
        }
        val.max(1)
    }

    pub fn effective_speed(&self) -> i32 {
        let mut val = self.stats.speed;
        for s in &self.statuses {
            match s.status_type {
                StatusType::SpeedUp => val += s.magnitude,
                StatusType::SpeedDown => val -= s.magnitude,
                _ => {}
            }
        }
        val.max(1)
    }
}

// ---------------------------------------------------------------------------
// Battle state machine
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattlePhase {
    /// Setting up the battle.
    Setup,
    /// Determining turn order for this round.
    TurnOrder,
    /// Waiting for input for a specific combatant.
    WaitingForAction { combatant: usize },
    /// Executing an action (for animation).
    Executing,
    /// Processing end-of-turn effects (status ticks, etc).
    EndOfTurn,
    /// Checking win/lose conditions.
    CheckResult,
    /// Battle won by player.
    Victory,
    /// Battle lost by player.
    Defeat,
    /// Player fled successfully.
    Fled,
}

/// The battle system managing a turn-based encounter.
pub struct Battle {
    pub combatants: Vec<Combatant>,
    pub phase: BattlePhase,
    pub turn_order: Vec<usize>,
    pub current_turn: usize,
    pub round: u32,
    pub exp_pool: u32,
    pending_action: Option<TurnAction>,
    rng_seed: u64,
}

impl Battle {
    pub fn new(player_party: Vec<Combatant>, enemy_party: Vec<Combatant>) -> Self {
        let mut combatants = Vec::new();
        combatants.extend(player_party);
        combatants.extend(enemy_party);

        Self {
            combatants,
            phase: BattlePhase::Setup,
            turn_order: Vec::new(),
            current_turn: 0,
            round: 0,
            exp_pool: 0,
            pending_action: None,
            rng_seed: 12345,
        }
    }

    fn next_rng(&mut self) -> f32 {
        self.rng_seed ^= self.rng_seed << 13;
        self.rng_seed ^= self.rng_seed >> 7;
        self.rng_seed ^= self.rng_seed << 17;
        (self.rng_seed & 0x00FF_FFFF) as f32 / 16_777_216.0
    }

    /// Start the battle.
    pub fn start(&mut self) {
        self.phase = BattlePhase::TurnOrder;
    }

    /// Submit an action for the current combatant.
    pub fn submit_action(&mut self, action: TurnAction) {
        if matches!(self.phase, BattlePhase::WaitingForAction { .. }) {
            self.pending_action = Some(action);
        }
    }

    /// Advance the battle by one step. Returns effects that occurred.
    pub fn step(&mut self) -> Vec<BattleEffect> {
        let mut effects = Vec::new();

        match self.phase {
            BattlePhase::Setup => {
                self.phase = BattlePhase::TurnOrder;
            }

            BattlePhase::TurnOrder => {
                self.round += 1;
                self.calculate_turn_order();
                self.current_turn = 0;
                self.advance_to_next_alive();
            }

            BattlePhase::WaitingForAction { combatant } => {
                if let Some(action) = self.pending_action.take() {
                    effects = self.execute_action(combatant, &action);
                    self.phase = BattlePhase::EndOfTurn;
                }
            }

            BattlePhase::Executing => {
                self.phase = BattlePhase::EndOfTurn;
            }

            BattlePhase::EndOfTurn => {
                let idx = if self.current_turn > 0 && self.current_turn <= self.turn_order.len() {
                    self.turn_order[self.current_turn - 1]
                } else if !self.turn_order.is_empty() {
                    self.turn_order[0]
                } else {
                    0
                };

                // Tick status effects
                effects.extend(self.tick_statuses(idx));

                // Reset defend
                if idx < self.combatants.len() {
                    self.combatants[idx].is_defending = false;
                }

                // Next turn or next round
                self.current_turn += 1;
                self.advance_to_next_alive();
            }

            BattlePhase::CheckResult => {
                let players_alive = self.combatants.iter().any(|c| c.team == 0 && c.is_alive());
                let enemies_alive = self.combatants.iter().any(|c| c.team == 1 && c.is_alive());

                if !enemies_alive {
                    self.phase = BattlePhase::Victory;
                    let exp: u32 = self
                        .combatants
                        .iter()
                        .filter(|c| c.team == 1)
                        .map(|c| c.stats.level * 10 + 5)
                        .sum();
                    self.exp_pool = exp;
                    effects.push(BattleEffect::ExpGained { amount: exp });
                } else if !players_alive {
                    self.phase = BattlePhase::Defeat;
                } else {
                    self.phase = BattlePhase::TurnOrder;
                }
            }

            BattlePhase::Victory | BattlePhase::Defeat | BattlePhase::Fled => {}
        }

        effects
    }

    fn advance_to_next_alive(&mut self) {
        while self.current_turn < self.turn_order.len() {
            let idx = self.turn_order[self.current_turn];
            if idx < self.combatants.len() && self.combatants[idx].is_alive() {
                if self.combatants[idx].can_act() {
                    self.phase = BattlePhase::WaitingForAction { combatant: idx };
                } else {
                    // Can't act (stunned etc), skip
                    self.current_turn += 1;
                    continue;
                }
                return;
            }
            self.current_turn += 1;
        }
        // All turns done → check result
        self.phase = BattlePhase::CheckResult;
    }

    fn calculate_turn_order(&mut self) {
        let mut order: Vec<(usize, i32)> = self
            .combatants
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_alive())
            .map(|(i, c)| (i, c.effective_speed()))
            .collect();
        order.sort_by(|a, b| b.1.cmp(&a.1));
        self.turn_order = order.into_iter().map(|(i, _)| i).collect();
    }

    fn execute_action(&mut self, actor: usize, action: &TurnAction) -> Vec<BattleEffect> {
        let mut effects = Vec::new();

        match action {
            TurnAction::Attack { target } => {
                let target = *target;
                if target >= self.combatants.len() || !self.combatants[target].is_alive() {
                    return effects;
                }

                let atk = self.combatants[actor].effective_attack();
                let def = self.combatants[target].effective_defense();
                let element = self.combatants[actor].stats.element;
                let target_element = self.combatants[target].stats.element;

                let effectiveness = type_effectiveness(element, target_element);
                let is_crit = self.next_rng() < 0.1;
                let crit_mult = if is_crit { 1.5 } else { 1.0 };

                let raw = ((atk as f32 * 2.0 - def as f32) * effectiveness * crit_mult).max(1.0);
                let damage = raw as i32;

                self.combatants[target].stats.hp -= damage;
                effects.push(BattleEffect::Damage {
                    target,
                    amount: damage,
                    is_critical: is_crit,
                    element,
                });

                if self.combatants[target].stats.hp <= 0 {
                    self.combatants[target].stats.hp = 0;
                    effects.push(BattleEffect::Fainted { target });
                }
            }

            TurnAction::Defend => {
                self.combatants[actor].is_defending = true;
            }

            TurnAction::Flee => {
                let player_speed: i32 = self
                    .combatants
                    .iter()
                    .filter(|c| c.team == 0 && c.is_alive())
                    .map(|c| c.effective_speed())
                    .max()
                    .unwrap_or(0);
                let enemy_speed: i32 = self
                    .combatants
                    .iter()
                    .filter(|c| c.team == 1 && c.is_alive())
                    .map(|c| c.effective_speed())
                    .max()
                    .unwrap_or(0);

                let flee_chance = 0.5 + (player_speed - enemy_speed) as f32 * 0.05;
                if self.next_rng() < flee_chance.clamp(0.1, 0.95) {
                    self.phase = BattlePhase::Fled;
                    effects.push(BattleEffect::Fled);
                } else {
                    effects.push(BattleEffect::FleeBlocked);
                }
            }

            TurnAction::Switch { slot } => {
                effects.push(BattleEffect::Switched { slot: *slot });
            }

            TurnAction::Skill {
                skill_id: _,
                target,
            } => {
                // Placeholder: games implement skill lookup + execution
                // For now treat as a basic attack
                let target = *target;
                if target < self.combatants.len() && self.combatants[target].is_alive() {
                    let atk = self.combatants[actor].effective_attack();
                    let def = self.combatants[target].effective_defense();
                    let damage = (atk - def / 2).max(1);
                    self.combatants[target].stats.hp -= damage;
                    effects.push(BattleEffect::Damage {
                        target,
                        amount: damage,
                        is_critical: false,
                        element: self.combatants[actor].stats.element,
                    });
                    if self.combatants[target].stats.hp <= 0 {
                        self.combatants[target].stats.hp = 0;
                        effects.push(BattleEffect::Fainted { target });
                    }
                }
            }

            TurnAction::UseItem { item_id: _, target } => {
                // Placeholder: games handle item effects
                let target = *target;
                if target < self.combatants.len() {
                    let heal = 30;
                    self.combatants[target].stats.hp = (self.combatants[target].stats.hp + heal)
                        .min(self.combatants[target].stats.max_hp);
                    effects.push(BattleEffect::Heal {
                        target,
                        amount: heal,
                    });
                }
            }

            TurnAction::Skip => {}
        }

        effects
    }

    fn tick_statuses(&mut self, combatant: usize) -> Vec<BattleEffect> {
        let mut effects = Vec::new();
        if combatant >= self.combatants.len() {
            return effects;
        }

        let mut i = 0;
        while i < self.combatants[combatant].statuses.len() {
            // Read status fields into locals to avoid overlapping borrows
            let status_type = self.combatants[combatant].statuses[i].status_type;
            let magnitude = self.combatants[combatant].statuses[i].magnitude;

            // Apply per-turn effects
            match status_type {
                StatusType::Poison => {
                    let dmg = magnitude.max(1);
                    self.combatants[combatant].stats.hp -= dmg;
                    effects.push(BattleEffect::Damage {
                        target: combatant,
                        amount: dmg,
                        is_critical: false,
                        element: Element::Poison,
                    });
                }
                StatusType::Burn => {
                    let dmg = magnitude.max(1);
                    self.combatants[combatant].stats.hp -= dmg;
                    effects.push(BattleEffect::Damage {
                        target: combatant,
                        amount: dmg,
                        is_critical: false,
                        element: Element::Fire,
                    });
                }
                StatusType::Regen => {
                    let heal = magnitude.max(1);
                    let max_hp = self.combatants[combatant].stats.max_hp;
                    self.combatants[combatant].stats.hp =
                        (self.combatants[combatant].stats.hp + heal).min(max_hp);
                    effects.push(BattleEffect::Heal {
                        target: combatant,
                        amount: heal,
                    });
                }
                _ => {}
            }

            // Decrement duration
            let turns_remaining = &mut self.combatants[combatant].statuses[i].turns_remaining;
            if *turns_remaining > 0 {
                *turns_remaining -= 1;
                if *turns_remaining == 0 {
                    self.combatants[combatant].statuses.remove(i);
                    effects.push(BattleEffect::StatusRemoved {
                        target: combatant,
                        status: status_type,
                    });
                    continue;
                }
            }
            i += 1;
        }

        if self.combatants[combatant].stats.hp <= 0 {
            self.combatants[combatant].stats.hp = 0;
            effects.push(BattleEffect::Fainted { target: combatant });
        }

        effects
    }

    // -----------------------------------------------------------------------
    // Query helpers
    // -----------------------------------------------------------------------

    /// Get all alive combatants on a team.
    pub fn alive_on_team(&self, team: u8) -> Vec<usize> {
        self.combatants
            .iter()
            .enumerate()
            .filter(|(_, c)| c.team == team && c.is_alive())
            .map(|(i, _)| i)
            .collect()
    }

    /// Check if battle is over.
    pub fn is_over(&self) -> bool {
        matches!(
            self.phase,
            BattlePhase::Victory | BattlePhase::Defeat | BattlePhase::Fled
        )
    }

    /// Get the index of the combatant whose turn it is.
    pub fn current_combatant(&self) -> Option<usize> {
        match self.phase {
            BattlePhase::WaitingForAction { combatant } => Some(combatant),
            _ => None,
        }
    }

    /// Is the current combatant player-controlled?
    pub fn needs_player_input(&self) -> bool {
        match self.phase {
            BattlePhase::WaitingForAction { combatant } => {
                combatant < self.combatants.len() && self.combatants[combatant].team == 0
            }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Simple encounter table
// ---------------------------------------------------------------------------

/// Random encounter definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncounterEntry {
    pub enemy_group_id: u32,
    pub weight: f32,
    pub min_level: u32,
    pub max_level: u32,
}

/// Encounter table for a map area.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncounterTable {
    pub entries: Vec<EncounterEntry>,
    /// Base chance per step (0.0-1.0).
    pub base_rate: f32,
    /// Steps since last encounter.
    pub steps: u32,
}

impl EncounterTable {
    pub fn new(base_rate: f32) -> Self {
        Self {
            entries: Vec::new(),
            base_rate,
            steps: 0,
        }
    }

    pub fn with_entry(
        mut self,
        group_id: u32,
        weight: f32,
        min_level: u32,
        max_level: u32,
    ) -> Self {
        self.entries.push(EncounterEntry {
            enemy_group_id: group_id,
            weight,
            min_level,
            max_level,
        });
        self
    }

    /// Check if an encounter should happen. Call on each step.
    /// Returns the enemy_group_id if an encounter triggers.
    pub fn check(&mut self, player_level: u32, seed: u64) -> Option<u32> {
        self.steps += 1;
        // Increasing chance with steps (prevents long dry spells)
        let chance = self.base_rate * (1.0 + self.steps as f32 * 0.02);

        let mut rng_state = seed ^ (self.steps as u64 * 0x9E37_79B9);
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        let roll = (rng_state & 0x00FF_FFFF) as f32 / 16_777_216.0;

        if roll >= chance.clamp(0.0, 0.9) {
            return None;
        }

        self.steps = 0;

        // Select encounter
        let eligible: Vec<&EncounterEntry> = self
            .entries
            .iter()
            .filter(|e| player_level >= e.min_level && player_level <= e.max_level)
            .collect();

        if eligible.is_empty() {
            return None;
        }

        let total_weight: f32 = eligible.iter().map(|e| e.weight).sum();
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        let mut pick = (rng_state & 0x00FF_FFFF) as f32 / 16_777_216.0 * total_weight;

        for entry in &eligible {
            pick -= entry.weight;
            if pick <= 0.0 {
                return Some(entry.enemy_group_id);
            }
        }

        eligible.last().map(|e| e.enemy_group_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_combatant(name: &str, hp: i32, atk: i32, def: i32, spd: i32, team: u8) -> Combatant {
        Combatant::new(
            name,
            CombatantStats {
                hp,
                max_hp: hp,
                mp: 10,
                max_mp: 10,
                attack: atk,
                defense: def,
                speed: spd,
                level: 5,
                exp: 0,
                element: Element::Normal,
            },
            team,
        )
    }

    // ── Battle flow ─────────────────────────────────────────

    #[test]
    fn basic_battle_flow() {
        let player = make_combatant("Hero", 100, 20, 10, 15, 0);
        let enemy = make_combatant("Slime", 30, 8, 5, 5, 1);

        let mut battle = Battle::new(vec![player], vec![enemy]);
        battle.start();

        // Step to determine turn order
        battle.step();

        // Hero should go first (speed 15 > 5)
        assert!(battle.needs_player_input());
        assert_eq!(battle.current_combatant(), Some(0));

        // Attack the enemy
        battle.submit_action(TurnAction::Attack { target: 1 });
        let effects = battle.step();
        assert!(effects
            .iter()
            .any(|e| matches!(e, BattleEffect::Damage { target: 1, .. })));

        // Process end of turn, then slime's turn or check result
        loop {
            let effects = battle.step();
            if battle.is_over() {
                break;
            }
            // If slime gets a turn, auto-attack
            if battle.needs_player_input() {
                break;
            }
            if let BattlePhase::WaitingForAction { combatant } = battle.phase {
                if battle.combatants[combatant].team == 1 {
                    battle.submit_action(TurnAction::Attack { target: 0 });
                }
            }
            if effects.is_empty() && battle.phase == BattlePhase::TurnOrder {
                battle.step(); // next round
            }
        }
    }

    // ── Type effectiveness ──────────────────────────────────

    #[test]
    fn type_effectiveness_basics() {
        assert_eq!(type_effectiveness(Element::Fire, Element::Grass), 2.0);
        assert_eq!(type_effectiveness(Element::Fire, Element::Water), 0.5);
        assert_eq!(type_effectiveness(Element::Electric, Element::Earth), 0.0);
        assert_eq!(type_effectiveness(Element::Normal, Element::Normal), 1.0);
    }

    // ── Status effects and defense ──────────────────────────

    #[test]
    fn status_effects_tick() {
        let mut player = make_combatant("Hero", 100, 20, 10, 15, 0);
        player
            .statuses
            .push(StatusEffect::new(StatusType::Poison, 2, 5));

        let enemy = make_combatant("Slime", 999, 1, 1, 1, 1);
        let mut battle = Battle::new(vec![player], vec![enemy]);
        battle.start();
        battle.step(); // turn order

        // Hero's turn → defend
        battle.submit_action(TurnAction::Defend);
        battle.step(); // execute

        // End of turn → poison ticks
        let effects = battle.step();
        let poison_dmg = effects
            .iter()
            .any(|e| matches!(e, BattleEffect::Damage { amount: 5, .. }));
        assert!(poison_dmg);
    }

    #[test]
    fn defend_reduces_damage() {
        let player = make_combatant("Hero", 100, 20, 10, 15, 0);
        let enemy = make_combatant("Slime", 999, 20, 5, 20, 1); // enemy faster

        let mut battle = Battle::new(vec![player], vec![enemy]);
        battle.start();
        battle.step(); // turn order

        // Enemy goes first (speed 20) → attack player
        // But we need to handle: if enemy goes first, player hasn't defended yet
        // Just verify defending flag works
        let hero = &battle.combatants[0];
        let normal_def = hero.effective_defense();
        assert_eq!(normal_def, 10);

        // Simulate defend
        let mut hero_clone = battle.combatants[0].clone();
        hero_clone.is_defending = true;
        assert_eq!(hero_clone.effective_defense(), 15); // 10 * 1.5
    }
}
