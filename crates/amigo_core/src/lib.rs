#![allow(missing_docs)]

pub mod accessibility;
pub mod achievements;
pub mod agents;
pub mod ai;
pub mod behavior_tree;
pub mod broad_phase;
pub mod bullet_pattern;
pub mod city_builder;
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
pub mod fog_of_war;
pub mod frame_arena;
pub mod game_preset;
pub mod inventory;
pub mod level_loader;
pub mod localization;
pub mod loot;
pub mod math;
pub mod metroidvania;
pub mod navigation;
pub mod pathfinding;
pub mod physics;
pub mod platformer;
pub mod procgen;
pub mod projectile;
pub mod puzzle;
pub mod raycast;
pub mod rect;
pub mod resources;
pub mod roguelike;
pub mod rts;
pub mod save;
pub mod scheduler;
pub mod shmup;
pub mod simulation;
pub mod spline;
pub mod state_rewind;
pub mod status_effect;
pub mod time;
pub mod timeline;
pub mod turn_combat;
pub mod tween;
pub mod visual_novel;
pub mod world_context;

// -- New building blocks -----------------------------------------------------
pub mod card;
pub mod door;
pub mod task_system;
pub mod vision_ray;
pub mod voting;

// -- New genre modules -------------------------------------------------------
pub mod auto_battler;
pub mod deckbuilder;
pub mod idle;
pub mod social_deduction;

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
pub use accessibility::{
    AccessibilityConfig, AccessibilityFeature, AccessibilityManager, ColorBlindFilter,
    ColorBlindMode, HighContrastTheme, InputAssistSettings, ShakeSettings, Subtitle,
    SubtitleCategory, SubtitleDirection, SubtitleManager, TextScaleSettings,
};
pub use achievements::{
    AchievementCondition, AchievementDef, AchievementError, AchievementProgress,
    AchievementSaveData, AchievementToastRenderer, AchievementTracker,
};
pub use broad_phase::{Aabb, BroadPhase, CollisionPair, CpuBroadPhase, GpuBroadPhase};
pub use collision::{
    CapsuleShape, CollisionShape, CollisionWorld, ContactInfo, SpatialHash, SweptContact,
    TriggerEvent, TriggerZone,
};
pub use collision_events::{CollisionEvent, CollisionPhase, ContactTracker};
pub use color::Color;
pub use command::{CommandLog, CommandQueue};
pub use economy::Economy;
pub use ecs::{EntityId, SparseSet, World};
pub use events::EventHub;
pub use fog_of_war::{update_visibility, FogOfWarGrid, TileVisibility};
pub use frame_arena::FrameArena;
pub use localization::{
    LocaleError, LocaleId, LocaleManager, PluralCategory, PluralRuleFn, StringEntry,
};
pub use math::{Fix, RenderVec2, SimVec2};
pub use metroidvania::{
    Ability, AbilityGate, AbilitySet, BacktrackMarker, BossData, BossId, BossMovement, BossPhase,
    BossRoomSystem, CheckpointData, CheckpointSystem, ExplorationGraph, MapPin, MapRevealer,
    PatternSequence, PhaseConfig, RoomConnection, RoomId, RoomNode, RoomTransitionSystem,
    SkillUnlockSystem, TransitionResult, ZoneId,
};
pub use pathfinding::{find_path, FlowField, PathFollower, PathRequest, Walkable, WaypointPath};
pub use physics::{
    sync_ecs_to_physics, sync_physics_to_ecs, BodyType, PhysicsContact, PhysicsWorld, RigidBody,
};
pub use projectile::ProjectileManager;
pub use raycast::{raycast, raycast_bodies, raycast_tiles, sensor, RayHit, TileBlock, TileQuery};
pub use rect::Rect;
pub use resources::Resources;
pub use save::{SaveConfig, SaveError, SaveManager, SlotInfo};
pub use scheduler::{CallbackId, TickScheduler};
pub use spline::{CatmullRomSpline, CubicBezier};
pub use status_effect::{EffectType, StatusEffect, StatusEffects};
pub use time::TimeInfo;
pub use tween::{
    EasingFn, RepeatCount, Tween, TweenHandle, TweenManager, TweenSequence, Tweenable,
};
pub use visual_novel::{
    AutoRead, BacklogEntry, BacklogSystem, BgTransition, BranchingSystem, CharacterDisplay,
    ChoiceMenu, ChoiceOption, ChoiceRecord, Emotion, NameLabelConfig, SlideDir, SlotPosition,
    TextboxBackground, TextboxConfig, TextboxMode, TextboxState, TypewriterEffect, VnScene,
};
pub use world_context::{
    ArtType, Biome, Era, Lighting, Mood, MusicGenre, MusicKey, MusicStyle, Palette, VisualStyle,
    Weather, WorldContext,
};

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
