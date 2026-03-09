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

pub use engine::{Engine, EngineBuilder};
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
    pub use crate::{Game, Engine, EngineBuilder, GameContext, DrawContext, EngineConfig};
    pub use amigo_core::{Fix, SimVec2, RenderVec2, Color, Rect, EntityId, World, TimeInfo};
    pub use amigo_core::math::{vec2, IVec2};
    pub use amigo_core::ecs::{self, SparseSet};
    pub use amigo_core::save::{SaveManager, SaveConfig, SlotInfo, SaveError};
    pub use amigo_core::scheduler::{TickScheduler, CallbackId};
    pub use amigo_scene::SceneAction;
    pub use amigo_input::InputState;
    pub use amigo_render::{Camera, CameraMode, Easing};
    pub use amigo_render::particles::{ParticleSystem, EmitterConfig, EmitterShape};
    pub use amigo_render::lighting::{LightingState, PointLight, AmbientLight};
    pub use amigo_render::post_process::{PostProcessPipeline, PostEffect};
    pub use amigo_tilemap::*;
    pub use amigo_animation::*;
    pub use amigo_net::{PlayerId, Transport, LocalTransport};
    pub use winit::keyboard::KeyCode;
    pub use winit::event::MouseButton;
}
