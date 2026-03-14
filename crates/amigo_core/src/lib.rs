pub mod math;
pub mod ecs;
pub mod color;
pub mod rect;
pub mod time;
pub mod scheduler;
pub mod pathfinding;
pub mod collision;
pub mod save;
pub mod command;
pub mod physics;
pub mod collision_events;
pub mod navigation;
pub mod ai;
pub mod combat;
pub mod loot;
pub mod inventory;
pub mod turn_combat;
pub mod dialog;
pub mod crafting;
pub mod procgen;
pub mod roguelike;
pub mod fighting;
pub mod platformer;
pub mod farming;
pub mod bullet_pattern;
pub mod puzzle;
pub mod events;
pub mod resources;
pub mod economy;
pub mod projectile;
pub mod status_effect;
pub mod game_preset;
pub mod level_loader;

// -- Tower Defense genre modules (feature-gated) ----------------------------
#[cfg(feature = "td")]
pub mod tower;
#[cfg(feature = "td")]
pub mod waves;
#[cfg(feature = "td")]
pub mod enemy;
#[cfg(feature = "td")]
pub mod game_state;
#[cfg(feature = "td")]
pub mod td_systems;

// -- Core re-exports (always available) -------------------------------------
pub use math::{Fix, SimVec2, RenderVec2};
pub use color::Color;
pub use rect::Rect;
pub use ecs::{EntityId, World, SparseSet};
pub use time::TimeInfo;
pub use scheduler::{TickScheduler, CallbackId};
pub use save::{SaveManager, SaveConfig, SlotInfo, SaveError};
pub use command::{CommandQueue, CommandLog};
pub use physics::{PhysicsWorld, RigidBody, BodyType, PhysicsContact};
pub use collision_events::{ContactTracker, CollisionEvent, CollisionPhase};
pub use events::EventHub;
pub use resources::Resources;
pub use economy::Economy;
pub use projectile::ProjectileManager;
pub use status_effect::{StatusEffects, StatusEffect, EffectType};

// -- Tower Defense re-exports (feature-gated) -------------------------------
#[cfg(feature = "td")]
pub use tower::{TowerDef, TowerInstance, TowerTier, TowerAttackType, TargetingStrategy, PlacementGrid};
#[cfg(feature = "td")]
pub use waves::{WaveDef, WaveSpawner, WavePhase, SpawnGroup, SpawnEvent};
#[cfg(feature = "td")]
pub use enemy::{EnemyDef, EnemyInstance, EnemyManager};
#[cfg(feature = "td")]
pub use game_state::{TdGameState, GameCommand, GamePhase, CommandResult};
#[cfg(feature = "td")]
pub use td_systems::td_tick;
