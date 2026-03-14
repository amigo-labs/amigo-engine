//! # amigo_engine
//!
//! A 2D game engine built on fixed-point math, ECS, and wgpu rendering.
//! amigo_engine is the top-level crate that ties together all sub-crates
//! into a single, batteries-included framework for building 2D games.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use amigo_engine::prelude::*;
//!
//! struct MyGame;
//!
//! impl Game for MyGame {
//!     fn update(&mut self, _ctx: &mut GameContext) -> SceneAction {
//!         SceneAction::Continue
//!     }
//!
//!     fn draw(&self, _ctx: &mut DrawContext) {}
//! }
//!
//! fn main() {
//!     EngineBuilder::new()
//!         .build()
//!         .run(MyGame);
//! }
//! ```
//!
//! ## Sub-crates / modules
//!
//! | Crate               | Purpose                                         |
//! |---------------------|-------------------------------------------------|
//! | [`amigo_core`]      | Fixed-point math, ECS, save system, scheduling  |
//! | [`amigo_render`]    | wgpu renderer, camera, particles, lighting      |
//! | [`amigo_input`]     | Keyboard, mouse, and gamepad input              |
//! | [`amigo_assets`]    | Asset loading, Aseprite import, hot-reloading   |
//! | [`amigo_tilemap`]   | Tilemap data structures and utilities           |
//! | [`amigo_animation`] | Sprite animation state machine                  |
//! | [`amigo_scene`]     | Scene stack and transitions                     |
//! | [`amigo_ui`]        | Immediate-mode pixel UI (HUD, menus)            |
//! | [`amigo_net`]       | Networking and multiplayer transport             |
//! | [`amigo_debug`]     | Debug overlay, FPS counter, system profiling     |
//! | [`amigo_audio`]     | Audio playback (behind `audio` feature flag)     |
//!
//! The [`engine`], [`config`], and [`context`] modules live in this crate
//! and provide the main loop, configuration, and per-frame contexts.

pub mod engine;
pub mod config;
pub mod context;

// Re-export all sub-crates for convenient access
pub use amigo_core;
pub use amigo_render;
pub use amigo_input;
pub use amigo_assets;
pub use amigo_tilemap;
pub use amigo_animation;
pub use amigo_scene;
pub use amigo_ui;
pub use amigo_net;
pub use amigo_debug;

#[cfg(feature = "audio")]
pub use amigo_audio;

pub use engine::{Engine, EngineBuilder, Plugin, PluginContext};
pub use context::{GameContext, DrawContext};
pub use config::EngineConfig;

/// The Game trait that all games implement.
pub trait Game: 'static {
    /// Called once when the game starts.
    fn init(&mut self, _ctx: &mut GameContext) {}

    /// Update game logic (called at fixed timestep, 60 ticks/sec).
    fn update(&mut self, ctx: &mut GameContext) -> amigo_scene::SceneAction;

    /// Render the game (called every frame, with interpolation alpha).
    fn draw(&self, ctx: &mut DrawContext);
}

/// Prelude with commonly used types.
pub mod prelude {
    pub use crate::{Game, Engine, EngineBuilder, Plugin, PluginContext, GameContext, DrawContext, EngineConfig};
    pub use amigo_core::events::EventHub;
    pub use amigo_core::resources::Resources;
    pub use amigo_core::{Fix, SimVec2, RenderVec2, Color, Rect, EntityId, World, TimeInfo};
    pub use amigo_core::math::{vec2, IVec2};
    pub use amigo_core::ecs::{self, SparseSet, Component, join, join3, join4, join_mut};
    pub use amigo_core::save::{SaveManager, SaveConfig, SlotInfo, SaveError};
    pub use amigo_core::scheduler::{TickScheduler, CallbackId};
    pub use amigo_scene::SceneAction;
    pub use amigo_input::InputState;
    pub use amigo_render::{Camera, CameraMode, Easing, FontId, FontManager, ArtStyle, SamplerMode};
    pub use amigo_render::particles::{ParticleSystem, EmitterConfig, EmitterShape};
    pub use amigo_render::lighting::{LightingState, PointLight, AmbientLight};
    pub use amigo_render::post_process::{PostProcessPipeline, PostEffect};
    pub use amigo_tilemap::*;
    pub use amigo_animation::*;
    pub use amigo_net::{PlayerId, Transport, LocalTransport};
    pub use amigo_net::checksum::StateHasher;
    pub use amigo_net::lobby::{LobbyManager, Room, RoomId, RoomConfig, RoomPhase};
    pub use amigo_net::stats::{NetStats, ConnectionQuality};
    pub use amigo_ui::{UiContext, UiDrawCommand};
    pub use amigo_debug::DebugOverlay;
    pub use amigo_assets::AssetError;
    pub use winit::keyboard::KeyCode;
    pub use winit::event::MouseButton;

    #[cfg(feature = "audio")]
    pub use amigo_audio::AudioManager;
}
