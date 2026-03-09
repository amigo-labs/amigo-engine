use serde::{Deserialize, Serialize};

/// The current phase of the game.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GamePhase {
    Setup,
    Playing,
    Paused,
    BetweenWaves,
    GameOver,
    Victory,
}

/// Serializable state of a placed tower.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TowerState {
    pub id: u32,
    /// Identifies the tower archetype (maps to a TowerTypeId).
    pub tower_type: u32,
    pub tile_x: i32,
    pub tile_y: i32,
    pub level: u32,
    pub kills: u32,
    pub damage_dealt: u64,
}

/// Serializable state of an enemy.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EnemyState {
    pub id: u32,
    pub enemy_type: u32,
    /// Fixed-point x position stored as raw i64.
    pub x: i64,
    /// Fixed-point y position stored as raw i64.
    pub y: i64,
    pub health: i32,
    pub max_health: i32,
    pub path_index: u32,
    /// Fixed-point path progress stored as raw i64.
    pub path_progress: i64,
    /// Fixed-point speed stored as raw i64.
    pub speed: i64,
}

/// Serializable state of a projectile.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectileState {
    pub id: u32,
    pub projectile_type: u32,
    /// Fixed-point x position stored as raw i64.
    pub x: i64,
    /// Fixed-point y position stored as raw i64.
    pub y: i64,
    pub target_id: Option<u32>,
    pub damage: i32,
}

/// The complete, serializable state of a tower-defense game session.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameState {
    pub tick: u64,
    /// Seed for deterministic RNG (no external RNG crate dependency).
    pub rng_seed: u64,
    pub gold: i32,
    pub lives: i32,
    pub wave_number: u32,
    pub game_phase: GamePhase,
    pub score: u64,
    pub towers: Vec<TowerState>,
    pub enemies: Vec<EnemyState>,
    pub projectiles: Vec<ProjectileState>,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            tick: 0,
            rng_seed: 0,
            gold: 100,
            lives: 20,
            wave_number: 0,
            game_phase: GamePhase::Setup,
            score: 0,
            towers: Vec::new(),
            enemies: Vec::new(),
            projectiles: Vec::new(),
        }
    }
}

impl GameState {
    /// Create a new game state with the given starting resources.
    pub fn new(starting_gold: i32, starting_lives: i32) -> Self {
        Self {
            gold: starting_gold,
            lives: starting_lives,
            ..Self::default()
        }
    }

    /// Returns `true` if the game phase is `GameOver`.
    pub fn is_game_over(&self) -> bool {
        self.game_phase == GamePhase::GameOver
    }

    /// Returns `true` if the game phase is `Victory`.
    pub fn is_victory(&self) -> bool {
        self.game_phase == GamePhase::Victory
    }
}
