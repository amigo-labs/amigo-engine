use crate::config::EngineConfig;
use crate::context::{DrawContext, GameContext};
use crate::splash::{self, SplashState};
use crate::Game;
use amigo_assets::{AssetManager, HotReloader};
use amigo_debug::DebugOverlay;
use amigo_render::renderer::Renderer;
use amigo_render::sprite_batcher::SpriteInstance;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::{Window, WindowId};

/// A plugin that can register systems, events, and resources with the engine.
pub trait Plugin: 'static {
    /// Called once during engine build to register events, resources, etc.
    fn build(&self, ctx: &mut PluginContext);
    /// Called once after the window and renderer are initialized.
    fn init(&self, _ctx: &mut GameContext) {}
}

/// Context passed to Plugin::build() for registration.
pub struct PluginContext {
    /// Event registrations to apply when GameContext is created.
    pub(crate) event_registrations: Vec<Box<dyn FnOnce(&mut amigo_core::events::EventHub)>>,
    /// Resource insertions to apply when GameContext is created.
    pub(crate) resource_insertions: Vec<Box<dyn FnOnce(&mut amigo_core::resources::Resources)>>,
}

impl PluginContext {
    fn new() -> Self {
        Self {
            event_registrations: Vec::new(),
            resource_insertions: Vec::new(),
        }
    }

    /// Register an event type so it can be emitted and read.
    pub fn register_event<T: 'static>(&mut self) {
        self.event_registrations
            .push(Box::new(|hub: &mut amigo_core::events::EventHub| {
                hub.register::<T>();
            }));
    }

    /// Insert a resource that will be available in GameContext.
    pub fn insert_resource<T: 'static>(&mut self, resource: T) {
        self.resource_insertions
            .push(Box::new(move |res: &mut amigo_core::resources::Resources| {
                res.insert(resource);
            }));
    }
}

/// Builder for configuring and launching the engine.
pub struct EngineBuilder {
    config: EngineConfig,
    assets_path: String,
    plugins: Vec<Box<dyn Plugin>>,
    plugin_ctx: PluginContext,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self {
            config: EngineConfig::load(),
            assets_path: "assets".to_string(),
            plugins: Vec::new(),
            plugin_ctx: PluginContext::new(),
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.config.window.title = title.to_string();
        self
    }

    pub fn virtual_resolution(mut self, width: u32, height: u32) -> Self {
        self.config.render.virtual_width = width;
        self.config.render.virtual_height = height;
        self
    }

    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.config.window.width = width;
        self.config.window.height = height;
        self
    }

    pub fn assets_path(mut self, path: &str) -> Self {
        self.assets_path = path.to_string();
        self
    }

    /// Enable or disable the default "Powered by Amigo Engine" splash screen.
    /// Enabled by default.
    pub fn splash(mut self, enabled: bool) -> Self {
        self.config.splash.enabled = enabled;
        self
    }

    pub fn config(mut self, config: EngineConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a plugin to the engine.
    pub fn add_plugin(mut self, plugin: impl Plugin) -> Self {
        plugin.build(&mut self.plugin_ctx);
        self.plugins.push(Box::new(plugin));
        self
    }

    pub fn build(self) -> Engine {
        Engine {
            config: self.config,
            assets_path: self.assets_path,
            plugins: self.plugins,
            plugin_ctx: self.plugin_ctx,
        }
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// The main engine struct. Call `run()` with your Game implementation to start.
pub struct Engine {
    config: EngineConfig,
    assets_path: String,
    plugins: Vec<Box<dyn Plugin>>,
    plugin_ctx: PluginContext,
}

impl Engine {
    pub fn build() -> EngineBuilder {
        EngineBuilder::new()
    }

    pub fn run<G: Game>(self, game: G) {
        amigo_debug::init_logging();
        info!("Amigo Engine starting: {}", self.config.window.title);

        let event_loop = EventLoop::new().expect("Failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = EngineApp {
            config: self.config,
            assets_path: self.assets_path,
            game,
            plugins: self.plugins,
            plugin_ctx: Some(self.plugin_ctx),
            state: None,
        };

        event_loop.run_app(&mut app).expect("Event loop failed");
    }
}

/// Upload any dirty font atlas textures to the GPU.
fn upload_font_atlases(game_ctx: &mut GameContext, renderer: &mut Renderer) {
    for font_atlas in game_ctx.fonts.iter_mut() {
        if font_atlas.dirty || font_atlas.texture_id.is_none() {
            let image = font_atlas.to_rgba_image();
            let tex_id = renderer.load_texture(&image, &format!("font_{}", font_atlas.id.0));
            font_atlas.texture_id = Some(tex_id);
            font_atlas.dirty = false;
        }
    }
}

struct EngineState {
    window: Arc<Window>,
    renderer: Renderer,
    game_ctx: GameContext,
    debug: DebugOverlay,
    assets: AssetManager,
    hot_reloader: Option<HotReloader>,
    sprite_draw_list: Vec<SpriteInstance>,
    last_frame: Instant,
    accumulator: f64,
    splash: Option<SplashState>,
}

struct EngineApp<G: Game> {
    config: EngineConfig,
    assets_path: String,
    game: G,
    plugins: Vec<Box<dyn Plugin>>,
    plugin_ctx: Option<PluginContext>,
    state: Option<EngineState>,
}

impl<G: Game> ApplicationHandler for EngineApp<G> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.window.width,
                self.config.window.height,
            ));

        let window = Arc::new(event_loop.create_window(window_attrs).expect("Failed to create window"));

        let mut renderer = pollster::block_on(Renderer::new(
            window.clone(),
            self.config.render.virtual_width,
            self.config.render.virtual_height,
        ));

        let mut assets = AssetManager::new(&self.assets_path);
        if let Err(e) = assets.load_sprites() {
            error!("Failed to load sprites: {}", e);
        }

        let hot_reloader = if self.config.dev.hot_reload {
            HotReloader::new(std::path::PathBuf::from(&self.assets_path))
        } else {
            None
        };

        let vw = self.config.render.virtual_width as f32;
        let vh = self.config.render.virtual_height as f32;
        let mut game_ctx = GameContext::new(vw, vh, &self.assets_path);

        // Load built-in pixel font at 7px (native size)
        if let Err(e) = game_ctx.fonts.load_builtin(7.0) {
            error!("Failed to load built-in font: {}", e);
        }

        // Upload loaded sprites to GPU
        // (In a real implementation this would happen via the asset manager)

        // Apply plugin registrations (events, resources)
        if let Some(plugin_ctx) = self.plugin_ctx.take() {
            for reg in plugin_ctx.event_registrations {
                reg(&mut game_ctx.events);
            }
            for ins in plugin_ctx.resource_insertions {
                ins(&mut game_ctx.resources);
            }
        }

        // Initialize plugins
        for plugin in &self.plugins {
            plugin.init(&mut game_ctx);
        }

        // Upload font atlas textures to GPU
        upload_font_atlases(&mut game_ctx, &mut renderer);

        let splash = if self.config.splash.enabled {
            Some(SplashState::new())
        } else {
            // No splash — init game immediately
            self.game.init(&mut game_ctx);
            None
        };

        info!("Engine initialized successfully");

        self.state = Some(EngineState {
            window,
            renderer,
            game_ctx,
            debug: DebugOverlay::new(),
            assets,
            hot_reloader,
            sprite_draw_list: Vec::new(),
            last_frame: Instant::now(),
            accumulator: 0.0,
            splash,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };

        match event {
            WindowEvent::CloseRequested => {
                info!("Window close requested");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                state.game_ctx.input.handle_key_event(event.physical_key, event.state);

                // Debug overlay toggle
                if event.state == ElementState::Pressed {
                    if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                        match code {
                            KeyCode::F1 => state.debug.toggle(),
                            KeyCode::F2 => state.debug.show_grid = !state.debug.show_grid,
                            KeyCode::F3 => state.debug.show_collision = !state.debug.show_collision,
                            KeyCode::F4 => state.debug.show_paths = !state.debug.show_paths,
                            _ => {}
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.game_ctx.input.handle_mouse_move(position.x as f32, position.y as f32);

                // Update world-space mouse position
                let (ww, wh) = state.renderer.window_size();
                let world_pos = state.game_ctx.camera.screen_to_world(
                    position.x as f32,
                    position.y as f32,
                    ww as f32,
                    wh as f32,
                );
                state.game_ctx.input.set_mouse_world_pos(world_pos);
            }

            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                state.game_ctx.input.handle_mouse_button(button, btn_state);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 120.0,
                };
                state.game_ctx.input.handle_scroll(scroll);
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame).as_secs_f64();
                state.last_frame = now;

                // Cap dt to prevent spiral of death
                let dt = dt.min(0.25);

                // ── Splash screen phase ──────────────────────────────
                if let Some(ref mut splash_state) = state.splash {
                    let finished = splash_state.tick(dt);
                    let splash_alpha = splash_state.alpha();

                    state.sprite_draw_list.clear();
                    let vw = state.renderer.camera.virtual_width;
                    let vh = state.renderer.camera.virtual_height;
                    let white_tex = state.renderer.white_texture_id;
                    splash::render_splash(
                        &mut state.sprite_draw_list,
                        white_tex,
                        vw,
                        vh,
                        splash_alpha,
                    );

                    for sprite in &state.sprite_draw_list {
                        state.renderer.batcher.push(sprite.clone());
                    }

                    match state.renderer.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            let (w, h) = state.renderer.window_size();
                            state.renderer.resize(w, h);
                        }
                        Err(e) => {
                            error!("Render error during splash: {:?}", e);
                        }
                    }

                    if finished {
                        state.splash = None;
                        self.game.init(&mut state.game_ctx);
                    }
                    return;
                }

                // ── Normal game loop ─────────────────────────────────
                state.accumulator += dt;

                state.game_ctx.time.dt = dt as f32;
                state.game_ctx.time.elapsed += dt;

                // Check hot reload
                if let Some(reloader) = &state.hot_reloader {
                    let changes = reloader.poll_changes();
                    if !changes.is_empty() {
                        info!("Hot reload: {} files changed", changes.len());
                    }
                }

                // Fixed timestep simulation
                let tick_duration = amigo_core::TimeInfo::TICK_DURATION;
                while state.accumulator >= tick_duration {
                    state.game_ctx.input.begin_frame();

                    let action = self.game.update(&mut state.game_ctx);
                    state.game_ctx.time.tick += 1;

                    match action {
                        amigo_scene::SceneAction::Quit => {
                            event_loop.exit();
                            return;
                        }
                        _ => {}
                    }

                    state.game_ctx.world.flush();
                    state.game_ctx.events.flush();
                    state.game_ctx.particles.update(tick_duration as f32);
                    state.accumulator -= tick_duration;
                }

                state.game_ctx.time.alpha = (state.accumulator / tick_duration) as f32;

                // Re-upload dirty font atlases
                upload_font_atlases(&mut state.game_ctx, &mut state.renderer);

                // Camera: game code sets target/shake/zoom on GameContext.camera.
                // Swap it into the renderer for update + render, then swap back.
                std::mem::swap(&mut state.game_ctx.camera, &mut state.renderer.camera);
                state.renderer.camera.update(dt as f32);

                // Render
                state.sprite_draw_list.clear();
                {
                    let camera_pos = state.renderer.camera.effective_position();
                    let vw = state.renderer.camera.virtual_width;
                    let vh = state.renderer.camera.virtual_height;
                    let alpha = state.game_ctx.time.alpha;
                    let white_tex = state.renderer.white_texture_id;

                    let mut draw_ctx = DrawContext::new(
                        &mut state.sprite_draw_list,
                        &state.game_ctx,
                        camera_pos,
                        vw,
                        vh,
                        alpha,
                        white_tex,
                    );
                    self.game.draw(&mut draw_ctx);
                }

                // Collect particle sprites
                let white_tex = state.renderer.white_texture_id;
                state.game_ctx.particles.collect_sprites(&mut state.sprite_draw_list, white_tex);

                // Push sprites to batcher
                for sprite in &state.sprite_draw_list {
                    state.renderer.batcher.push(sprite.clone());
                }

                // Update debug overlay
                state.debug.update(
                    dt,
                    state.game_ctx.world.entity_count(),
                    state.renderer.draw_call_count(),
                );

                // Render frame
                match state.renderer.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        let (w, h) = state.renderer.window_size();
                        state.renderer.resize(w, h);
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        error!("GPU out of memory");
                        event_loop.exit();
                    }
                    Err(e) => {
                        error!("Render error: {:?}", e);
                    }
                }

                // Swap camera back to GameContext so game code can read updated state
                std::mem::swap(&mut state.game_ctx.camera, &mut state.renderer.camera);
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}
