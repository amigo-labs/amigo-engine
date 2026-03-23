#![allow(missing_docs)]
// ---------------------------------------------------------------------------
// Public API stability markers (ADR-0013)
// ---------------------------------------------------------------------------
// Types and functions re-exported through `amigo_engine::prelude` are
// considered **stable public API**.  Breaking changes to these require a
// major version bump.
//
// Items behind `#[doc(hidden)]` or in modules *not* listed in the prelude
// are **internal** and may change without notice between minor versions.
//
// Feature-gated modules (`audio`, `editor`, `api`) follow the same contract
// once enabled — their public surface is stable within a major version.
// ---------------------------------------------------------------------------

//! # Amigo Engine
//!
//! A modern 2D pixel art game engine in Rust, built on fixed-point math,
//! ECS, and wgpu rendering.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use amigo_engine::prelude::*;
//!
//! struct MyGame;
//!
//! impl Game for MyGame {
//!     fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
//!         SceneAction::Continue
//!     }
//!     fn draw(&self, ctx: &mut DrawContext) {
//!         ctx.draw_text("Hello!", 10.0, 10.0, Color::WHITE);
//!     }
//! }
//!
//! fn main() {
//!     Engine::build()
//!         .title("My Game")
//!         .virtual_resolution(480, 270)
//!         .build()
//!         .run(MyGame);
//! }
//! ```
//!
//! ## Feature Flags
//!
//! | Flag | Description |
//! |------|-------------|
//! | `audio` | Audio playback via kira (default) |
//! | `editor` | egui editor overlay |
//! | `api` | JSON-RPC API + headless mode |
//! | `tracy` | Tracy profiler integration |
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

pub mod config;
pub mod context;
pub mod engine;
pub mod splash;

// Re-export all sub-crates for convenient access
pub use amigo_animation;
pub use amigo_assets;
pub use amigo_core;
pub use amigo_debug;
pub use amigo_input;
pub use amigo_net;
pub use amigo_render;
pub use amigo_scene;
pub use amigo_tilemap;
pub use amigo_ui;

#[cfg(feature = "audio")]
pub use amigo_audio;

pub use config::EngineConfig;
pub use context::{DrawContext, GameContext};
pub use engine::{Engine, EngineBuilder, Plugin, PluginContext};

/// The Game trait that all games implement.
pub trait Game: 'static {
    /// Called once when the game starts.
    fn init(&mut self, _ctx: &mut GameContext) {}

    /// Update game logic (called at fixed timestep, 60 ticks/sec).
    fn update(&mut self, ctx: &mut GameContext) -> amigo_scene::SceneAction;

    /// Render the game (called every frame, with interpolation alpha).
    fn draw(&self, ctx: &mut DrawContext);

    /// Called by `amigo dev` before recompile to capture game-specific state.
    /// Return a JSON blob that will be passed to `on_dev_restore` after restart.
    /// Default returns null (no game-specific state to save).
    fn on_dev_snapshot(&self, _ctx: &GameContext) -> serde_json::Value {
        serde_json::Value::Null
    }

    /// Called by `amigo dev` after recompile to restore game-specific state.
    /// The `state` parameter is whatever `on_dev_snapshot` returned previously.
    fn on_dev_restore(&mut self, _ctx: &mut GameContext, _state: &serde_json::Value) {}
}

/// Prelude with commonly used types.
pub mod prelude {
    pub use crate::{
        DrawContext, Engine, EngineBuilder, EngineConfig, Game, GameContext, Plugin, PluginContext,
    };
    pub use amigo_animation::*;
    pub use amigo_assets::{AssetError, AssetHandle, AssetState, HandleAllocator};
    pub use amigo_core::ecs::{self, join, join3, join4, join_mut, Component, SparseSet};
    pub use amigo_core::events::EventHub;
    pub use amigo_core::math::{vec2, IVec2};
    pub use amigo_core::resources::Resources;
    pub use amigo_core::save::{SaveConfig, SaveError, SaveManager, SlotInfo};
    pub use amigo_core::scheduler::{CallbackId, TickScheduler};
    pub use amigo_core::{
        find_path, CollisionShape, CollisionWorld, FlowField, PathFollower, PathRequest,
        SpatialHash, Walkable, WaypointPath,
    };
    pub use amigo_core::{Color, EntityId, Fix, Rect, RenderVec2, SimVec2, TimeInfo, World};
    pub use amigo_core::{WorldContext, Biome, Era, Mood, Palette, MusicStyle, VisualStyle};
    pub use amigo_debug::DebugOverlay;
    pub use amigo_input::InputState;
    pub use amigo_net::checksum::StateHasher;
    pub use amigo_net::lobby::{LobbyManager, Room, RoomConfig, RoomId, RoomPhase};
    pub use amigo_net::stats::{ConnectionQuality, NetStats};
    pub use amigo_net::{LocalTransport, PlayerId, Transport};
    pub use amigo_render::lighting::{AmbientLight, LightingState, PointLight};
    pub use amigo_render::particles::{EmitterConfig, EmitterShape, ParticleSystem};
    pub use amigo_render::post_process::{PostEffect, PostProcessPipeline};
    pub use amigo_render::{
        ArtStyle, Camera, CameraMode, Easing, FontId, FontManager, SamplerMode,
    };
    pub use amigo_scene::SceneAction;
    pub use amigo_tilemap::*;
    pub use amigo_ui::{UiContext, UiDrawCommand};
    pub use winit::event::MouseButton;
    pub use winit::keyboard::KeyCode;

    #[cfg(feature = "audio")]
    pub use amigo_audio::AudioManager;
}
