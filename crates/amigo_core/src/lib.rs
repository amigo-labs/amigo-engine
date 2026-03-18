#![warn(missing_docs)]

pub mod agents;
pub mod ai;
pub mod bullet_pattern;
pub mod collision;
pub mod collision_events;
pub mod color;
pub mod combat;
pub mod command;
pub mod crafting;
pub mod dialog;
pub mod economy;
pub mod ecs;
pub mod events;
pub mod farming;
pub mod fighting;
pub mod frame_arena;
pub mod game_preset;
pub mod inventory;
pub mod level_loader;
pub mod loot;
pub mod math;
pub mod navigation;
pub mod pathfinding;
pub mod physics;
pub mod platformer;
pub mod procgen;
pub mod projectile;
pub mod puzzle;
pub mod rect;
pub mod resources;
pub mod roguelike;
pub mod save;
pub mod scheduler;
pub mod simulation;
pub mod status_effect;
pub mod time;
pub mod turn_combat;

// -- Tower Defense genre modules (feature-gated) ----------------------------
#[cfg(feature = "td")]
pub mod enemy;
#[cfg(feature = "td")]
pub mod game_state;
#[cfg(feature = "td")]
pub mod td_systems;
#[cfg(feature = "td")]
pub mod tower;
#[cfg(feature = "td")]
pub mod waves;

// -- Core re-exports (always available) -------------------------------------
pub use collision::{
    CollisionShape, CollisionWorld, ContactInfo, SpatialHash, TriggerEvent, TriggerZone,
};
pub use collision_events::{CollisionEvent, CollisionPhase, ContactTracker};
pub use color::Color;
pub use command::{CommandLog, CommandQueue};
pub use economy::Economy;
pub use ecs::{EntityId, SparseSet, World};
pub use events::EventHub;
pub use math::{Fix, RenderVec2, SimVec2};
pub use pathfinding::{find_path, FlowField, PathFollower, PathRequest, Walkable, WaypointPath};
pub use physics::{BodyType, PhysicsContact, PhysicsWorld, RigidBody};
pub use projectile::ProjectileManager;
pub use rect::Rect;
pub use resources::Resources;
pub use save::{SaveConfig, SaveError, SaveManager, SlotInfo};
pub use scheduler::{CallbackId, TickScheduler};
pub use status_effect::{EffectType, StatusEffect, StatusEffects};
pub use time::TimeInfo;
pub use frame_arena::FrameArena;

// -- Tower Defense re-exports (feature-gated) -------------------------------
#[cfg(feature = "td")]
pub use enemy::{EnemyDef, EnemyInstance, EnemyManager};
#[cfg(feature = "td")]
pub use game_state::{CommandResult, GameCommand, GamePhase, TdGameState};
#[cfg(feature = "td")]
pub use td_systems::td_tick;
#[cfg(feature = "td")]
pub use tower::{
    PlacementGrid, TargetingStrategy, TowerAttackType, TowerDef, TowerInstance, TowerTier,
};
#[cfg(feature = "td")]
pub use waves::{SpawnEvent, SpawnGroup, WaveDef, WavePhase, WaveSpawner};
