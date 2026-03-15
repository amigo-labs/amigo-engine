use crate::ecs::EntityId;
use crate::economy::Economy;
use crate::math::RenderVec2;
use crate::tower::{TargetingStrategy, TowerDef, TowerInstance};
use crate::waves::{WaveDef, WavePhase, WaveSpawner};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Game Commands (network-safe, serializable)
// ---------------------------------------------------------------------------

/// Tower upgrade path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradePath {
    /// Default linear upgrade.
    Main,
    /// Alternate path A.
    PathA,
    /// Alternate path B.
    PathB,
}

/// A serializable game command. All player actions go through this enum
/// so they can be transmitted over the network and logged for replays.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GameCommand {
    /// Place a tower at a grid position.
    PlaceTower { pos: (u32, u32), tower_type: u32 },
    /// Sell an existing tower.
    SellTower { tower_id: EntityId },
    /// Upgrade a tower along a path.
    UpgradeTower { tower_id: EntityId, path: UpgradePath },
    /// Change a tower's targeting strategy.
    SetTargeting { tower_id: EntityId, strategy: TargetingStrategy },
    /// Start the next wave manually.
    StartWave,
    /// Pause the game.
    Pause,
    /// Unpause the game.
    Unpause,
    /// Set game speed multiplier.
    SetSpeed { multiplier: f32 },
}

// ---------------------------------------------------------------------------
// Game phase
// ---------------------------------------------------------------------------

/// High-level game phase.
/// High-level game phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    /// Between waves — player can build/upgrade.
    Build,
    /// Waves in progress.
    Combat,
    /// Player won (all waves cleared).
    Victory,
    /// Player lost (lives reached 0).
    Defeat,
    /// Game paused (stores what phase to return to).
    Paused { previous: Box<GamePhase> },
}

impl GamePhase {
    pub fn is_paused(&self) -> bool {
        matches!(self, GamePhase::Paused { .. })
    }

    pub fn is_over(&self) -> bool {
        matches!(self, GamePhase::Victory | GamePhase::Defeat)
    }
}

// ---------------------------------------------------------------------------
// Game state snapshot (for serialization / API)
// ---------------------------------------------------------------------------

/// A lightweight snapshot of game state for save/load and API queries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameSnapshot {
    pub tick: u64,
    pub gold: i32,
    pub lives: i32,
    pub score: u64,
    pub wave: usize,
    pub total_waves: usize,
    pub wave_phase: String,
    pub phase: String,
    pub tower_count: usize,
    pub enemy_count: usize,
    pub projectile_count: usize,
}

// ---------------------------------------------------------------------------
// Tower Defense game state
// ---------------------------------------------------------------------------

/// Unified tower defense game state tying all systems together.
pub struct TdGameState {
    pub tick: u64,
    pub economy: Economy,
    pub spawner: WaveSpawner,
    pub phase: GamePhase,
    pub speed_multiplier: f32,

    /// Game time in seconds (accounting for speed multiplier).
    pub game_time: f64,
}

impl TdGameState {
    pub fn new(
        starting_gold: i32,
        starting_lives: i32,
        waves: Vec<WaveDef>,
        spawn_points: Vec<RenderVec2>,
    ) -> Self {
        Self {
            tick: 0,
            economy: Economy::new(starting_gold, starting_lives),
            spawner: WaveSpawner::new(waves, spawn_points),
            phase: GamePhase::Build,
            speed_multiplier: 1.0,
            game_time: 0.0,
        }
    }

    /// Process a game command. Returns a result describing what happened.
    pub fn execute_command(
        &mut self,
        cmd: &GameCommand,
        tower_defs: &[TowerDef],
        towers: &mut Vec<(EntityId, TowerInstance)>,
    ) -> CommandResult {
        match cmd {
            GameCommand::PlaceTower { pos, tower_type } => {
                let def = tower_defs.iter().find(|d| d.id == *tower_type);
                let def = match def {
                    Some(d) => d,
                    None => return CommandResult::Failed("unknown tower type".into()),
                };

                let tier = &def.tiers[0];
                let cost = tier.cost as i32;

                if !self.economy.can_afford(cost) {
                    return CommandResult::Failed("not enough gold".into());
                }

                self.economy.try_spend(
                    cost,
                    crate::economy::TransactionKind::TowerPlace { tower_type: *tower_type },
                );

                // Tower instance creation is left to the caller — we just validate and deduct gold.

                CommandResult::TowerPlaced {
                    gold_remaining: self.economy.gold,
                    position: *pos,
                }
            }

            GameCommand::SellTower { tower_id } => {
                if let Some(idx) = towers.iter().position(|(eid, _)| eid == tower_id) {
                    let (_, instance) = &towers[idx];
                    if let Some(def) = tower_defs.iter().find(|d| d.id == instance.def_id) {
                        let refund = instance.sell_value(def) as i32;
                        self.economy.add_gold(
                            refund,
                            crate::economy::TransactionKind::TowerSell { tower_id: tower_id.index() },
                        );
                        towers.swap_remove(idx);
                        CommandResult::TowerSold {
                            refund,
                            gold_remaining: self.economy.gold,
                        }
                    } else {
                        CommandResult::Failed("tower def not found".into())
                    }
                } else {
                    CommandResult::Failed("tower not found".into())
                }
            }

            GameCommand::UpgradeTower { tower_id, path: _ } => {
                if let Some((_eid, instance)) = towers.iter_mut().find(|(eid, _)| eid == tower_id) {
                    if let Some(def) = tower_defs.iter().find(|d| d.id == instance.def_id) {
                        if !instance.can_upgrade(def) {
                            return CommandResult::Failed("max tier".into());
                        }
                        let next_tier = instance.current_tier + 1;
                        let cost = def.tiers[next_tier].cost as i32;
                        if !self.economy.try_spend(
                            cost,
                            crate::economy::TransactionKind::TowerUpgrade { tower_id: tower_id.index() },
                        ) {
                            return CommandResult::Failed("not enough gold".into());
                        }
                        instance.upgrade(def);
                        CommandResult::TowerUpgraded {
                            new_tier: instance.current_tier,
                            gold_remaining: self.economy.gold,
                        }
                    } else {
                        CommandResult::Failed("tower def not found".into())
                    }
                } else {
                    CommandResult::Failed("tower not found".into())
                }
            }

            GameCommand::SetTargeting { tower_id, strategy } => {
                if let Some((_eid, instance)) = towers.iter_mut().find(|(eid, _)| eid == tower_id) {
                    instance.targeting = *strategy;
                    CommandResult::Ok
                } else {
                    CommandResult::Failed("tower not found".into())
                }
            }

            GameCommand::StartWave => {
                if self.phase == GamePhase::Build {
                    self.spawner.start_next_wave();
                    self.phase = GamePhase::Combat;
                    CommandResult::WaveStarted {
                        wave: self.spawner.wave_number(),
                    }
                } else {
                    CommandResult::Failed("not in build phase".into())
                }
            }

            GameCommand::Pause => {
                if !self.phase.is_paused() && !self.phase.is_over() {
                    let prev = std::mem::replace(&mut self.phase, GamePhase::Build);
                    self.phase = GamePhase::Paused {
                        previous: Box::new(prev),
                    };
                    CommandResult::Ok
                } else {
                    CommandResult::Failed("can't pause".into())
                }
            }

            GameCommand::Unpause => {
                if let GamePhase::Paused { previous } = &self.phase {
                    self.phase = *previous.clone();
                    CommandResult::Ok
                } else {
                    CommandResult::Failed("not paused".into())
                }
            }

            GameCommand::SetSpeed { multiplier } => {
                self.speed_multiplier = multiplier.clamp(0.5, 3.0);
                CommandResult::Ok
            }
        }
    }

    /// Advance game state by one tick.
    pub fn update(&mut self, dt: f32) {
        if self.phase.is_paused() || self.phase.is_over() {
            return;
        }

        let scaled_dt = dt * self.speed_multiplier;
        self.tick += 1;
        self.game_time += scaled_dt as f64;
        self.economy.set_tick(self.tick);

        // Check defeat
        if self.economy.is_defeated() {
            self.phase = GamePhase::Defeat;
            return;
        }

        // Update wave spawner during combat
        if self.phase == GamePhase::Combat {
            if self.spawner.phase == WavePhase::Complete {
                // Wave done — apply interest and go back to build
                self.economy.apply_interest();
                self.phase = GamePhase::Build;
            } else if self.spawner.phase == WavePhase::Victory {
                self.phase = GamePhase::Victory;
            }
        }
    }

    /// Notify that an enemy was killed (awards bounty and updates spawner).
    pub fn on_enemy_killed(&mut self, enemy_type: u32, bounty: i32, score: u64) {
        self.economy.add_gold(
            bounty,
            crate::economy::TransactionKind::EnemyBounty { enemy_type },
        );
        self.economy.add_score(score);
        self.spawner.on_enemy_killed();
    }

    /// Notify that an enemy reached the exit.
    pub fn on_enemy_leaked(&mut self, lives_cost: i32) {
        self.economy.lose_lives(lives_cost);
        self.spawner.on_enemy_killed(); // still counts as "removed"
    }

    /// Create a snapshot of current state for API/save.
    pub fn snapshot(&self, tower_count: usize, enemy_count: usize, projectile_count: usize) -> GameSnapshot {
        GameSnapshot {
            tick: self.tick,
            gold: self.economy.gold,
            lives: self.economy.lives,
            score: self.economy.score,
            wave: self.spawner.wave_number(),
            total_waves: self.spawner.total_waves(),
            wave_phase: format!("{:?}", self.spawner.phase),
            phase: format!("{:?}", self.phase),
            tower_count,
            enemy_count,
            projectile_count,
        }
    }
}

/// Result of executing a game command.
#[derive(Clone, Debug)]
pub enum CommandResult {
    Ok,
    Failed(String),
    TowerPlaced { gold_remaining: i32, position: (u32, u32) },
    TowerSold { refund: i32, gold_remaining: i32 },
    TowerUpgraded { new_tier: usize, gold_remaining: i32 },
    WaveStarted { wave: usize },
}

impl CommandResult {
    pub fn is_ok(&self) -> bool {
        !matches!(self, CommandResult::Failed(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tower::{TowerAttackType, TowerTier};

    fn test_defs() -> Vec<TowerDef> {
        vec![TowerDef {
            id: 1,
            name: "Arrow".to_string(),
            tiers: vec![
                TowerTier {
                    damage: 10,
                    range: 100.0,
                    attack_speed: 1.0,
                    cost: 50,
                    attack_type: TowerAttackType::SingleTarget,
                    sprite_name: "tower_arrow_1".to_string(),
                },
                TowerTier {
                    damage: 20,
                    range: 120.0,
                    attack_speed: 1.5,
                    cost: 75,
                    attack_type: TowerAttackType::SingleTarget,
                    sprite_name: "tower_arrow_2".to_string(),
                },
            ],
            targeting: TargetingStrategy::First,
        }]
    }

    #[test]
    fn place_tower_costs_gold() {
        let mut state = TdGameState::new(100, 20, Vec::new(), Vec::new());
        let defs = test_defs();
        let mut towers = Vec::new();

        let result = state.execute_command(
            &GameCommand::PlaceTower { pos: (3, 4), tower_type: 1 },
            &defs,
            &mut towers,
        );
        assert!(result.is_ok());
        assert_eq!(state.economy.gold, 50);
    }

    #[test]
    fn cant_afford_tower() {
        let mut state = TdGameState::new(30, 20, Vec::new(), Vec::new());
        let defs = test_defs();
        let mut towers = Vec::new();

        let result = state.execute_command(
            &GameCommand::PlaceTower { pos: (0, 0), tower_type: 1 },
            &defs,
            &mut towers,
        );
        assert!(!result.is_ok());
        assert_eq!(state.economy.gold, 30);
    }

    #[test]
    fn sell_tower_refunds() {
        let mut state = TdGameState::new(200, 20, Vec::new(), Vec::new());
        let defs = test_defs();
        let eid = EntityId::from_raw(1, 0);
        let instance = TowerInstance::new(1, RenderVec2::ZERO, &defs[0].tiers[0], TargetingStrategy::First);
        let mut towers = vec![(eid, instance)];

        let result = state.execute_command(
            &GameCommand::SellTower { tower_id: eid },
            &defs,
            &mut towers,
        );
        assert!(result.is_ok());
        assert!(towers.is_empty());
        // Sell value = 70% of 50 = 35
        assert_eq!(state.economy.gold, 235);
    }

    #[test]
    fn enemy_killed_awards_bounty() {
        let mut state = TdGameState::new(100, 20, Vec::new(), Vec::new());
        state.on_enemy_killed(1, 25, 100);
        assert_eq!(state.economy.gold, 125);
        assert_eq!(state.economy.score, 100);
    }

    #[test]
    fn enemy_leak_costs_lives() {
        let mut state = TdGameState::new(100, 20, Vec::new(), Vec::new());
        state.on_enemy_leaked(3);
        assert_eq!(state.economy.lives, 17);
    }

    #[test]
    fn defeat_on_zero_lives() {
        let mut state = TdGameState::new(100, 1, Vec::new(), Vec::new());
        state.phase = GamePhase::Combat;
        state.on_enemy_leaked(1);
        state.update(0.016);
        assert_eq!(state.phase, GamePhase::Defeat);
    }

    #[test]
    fn pause_unpause() {
        let mut state = TdGameState::new(100, 20, Vec::new(), Vec::new());
        let defs = test_defs();
        let mut towers = Vec::new();

        state.execute_command(&GameCommand::Pause, &defs, &mut towers);
        assert!(state.phase.is_paused());

        state.execute_command(&GameCommand::Unpause, &defs, &mut towers);
        assert_eq!(state.phase, GamePhase::Build);
    }

    #[test]
    fn snapshot_works() {
        let state = TdGameState::new(100, 20, Vec::new(), Vec::new());
        let snap = state.snapshot(3, 10, 5);
        assert_eq!(snap.gold, 100);
        assert_eq!(snap.lives, 20);
        assert_eq!(snap.tower_count, 3);
    }
}
