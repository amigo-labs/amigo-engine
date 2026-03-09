pub mod math;
pub mod ecs;
pub mod color;
pub mod rect;
pub mod time;
pub mod scheduler;
pub mod pathfinding;
pub mod collision;
pub mod save;
pub mod game_state;
pub mod command;
pub mod tower_defense;

pub use math::{Fix, SimVec2, RenderVec2};
pub use color::Color;
pub use rect::Rect;
pub use ecs::{EntityId, World, SparseSet};
pub use time::TimeInfo;
pub use scheduler::{TickScheduler, CallbackId};
pub use save::{SaveManager, SaveConfig, SlotInfo, SaveError};
pub use game_state::{GameState, GamePhase, TowerState, EnemyState, ProjectileState};
pub use command::{GameCommand, CommandQueue, CommandLog, TowerTypeId, UpgradePath, IVec2};
pub use tower_defense::{
    TowerDefinition, TargetingPriority, TowerUpgrade,
    EnemyDefinition, WaveDefinition, WaveGroup, WaveManager,
    SpawnEvent, Economy,
};
