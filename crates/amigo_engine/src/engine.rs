use crate::config::EngineConfig;
use crate::context::{DrawContext, GameContext};
use crate::splash::{self, SplashState};
use crate::stack::GameStack;
use crate::Game;
use amigo_assets::{AssetManager, HotReloader};
use amigo_debug::DebugOverlay;
use amigo_render::renderer::Renderer;
use amigo_render::sprite_batcher::SpriteInstance;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, info_span, warn};
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
    /// Called every tick after engine systems but before rendering.
    /// Default implementation is a no-op, so existing plugins are unaffected.
    fn update(&mut self, _ctx: &mut GameContext) {}
}

/// Context passed to Plugin::build() for registration.
type EventRegistration = Box<dyn FnOnce(&mut amigo_core::events::EventHub)>;
type ResourceInsertion = Box<dyn FnOnce(&mut amigo_core::resources::Resources)>;

pub struct PluginContext {
    /// Event registrations to apply when GameContext is created.
    pub(crate) event_registrations: Vec<EventRegistration>,
    /// Resource insertions to apply when GameContext is created.
    pub(crate) resource_insertions: Vec<ResourceInsertion>,
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
        self.resource_insertions.push(Box::new(
            move |res: &mut amigo_core::resources::Resources| {
                res.insert(resource);
            },
        ));
    }
}

/// Builder for configuring and launching the engine.
pub struct EngineBuilder {
    config: EngineConfig,
    assets_path: String,
    plugins: Vec<Box<dyn Plugin>>,
    plugin_ctx: PluginContext,
    restore_snapshot: Option<std::path::PathBuf>,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self {
            config: EngineConfig::load(),
            assets_path: "assets".to_string(),
            plugins: Vec::new(),
            plugin_ctx: PluginContext::new(),
            // `amigo run --restore-snapshot` and `amigo dev` pass the path this
            // way: the engine never parses argv, since those arguments belong to
            // the game binary.
            restore_snapshot: std::env::var("AMIGO_RESTORE_SNAPSHOT")
                .ok()
                .filter(|p| !p.is_empty())
                .map(std::path::PathBuf::from),
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

    /// Enable headless mode (no window/renderer). The engine runs simulation
    /// only, controlled via the JSON-RPC API. Implies `api_server: true`.
    pub fn headless(mut self, enabled: bool) -> Self {
        self.config.dev.headless = enabled;
        if enabled {
            self.config.dev.api_server = true;
        }
        self
    }

    pub fn config(mut self, config: EngineConfig) -> Self {
        self.config = config;
        self
    }

    /// Restore a dev snapshot on startup: skip the splash, jump to the recorded
    /// tick and camera, and hand the game blob to `Game::on_dev_restore`.
    ///
    /// This is what `amigo run --restore-snapshot <path>` sets, and what makes
    /// `amigo dev` preserve state across a recompile.
    pub fn restore_snapshot(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.restore_snapshot = Some(path.into());
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
            restore_snapshot: self.restore_snapshot,
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
    restore_snapshot: Option<std::path::PathBuf>,
}

impl Engine {
    pub fn build() -> EngineBuilder {
        EngineBuilder::new()
    }

    pub fn run<G: Game>(self, game: G) {
        amigo_debug::init_logging();
        info!("Amigo Engine starting: {}", self.config.window.title);

        #[cfg(feature = "api")]
        if self.config.dev.headless {
            self.run_headless(game);
            return;
        }

        #[cfg(not(feature = "api"))]
        if self.config.dev.headless {
            error!("Headless mode requires the 'api' feature. Enable it with: cargo run --features api");
            return;
        }

        let event_loop = match EventLoop::new() {
            Ok(el) => el,
            Err(e) => {
                error!("Failed to create a window event loop: {e}");
                error!("No display seems to be available. On a headless machine, run with `amigo run --headless` (requires the 'api' feature).");
                return;
            }
        };
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = EngineApp {
            config: self.config,
            assets_path: self.assets_path,
            stack: GameStack::new(Box::new(game)),
            plugins: self.plugins,
            plugin_ctx: Some(self.plugin_ctx),
            state: None,
            restore_snapshot: self.restore_snapshot,
        };

        event_loop.run_app(&mut app).expect("Event loop failed");
    }

    /// Run the engine in headless mode: simulation only, no window or renderer.
    /// Controlled entirely via the JSON-RPC API server.
    #[cfg(feature = "api")]
    fn run_headless<G: Game>(self, game: G) {
        use amigo_api::handler::new_shared_state;
        use amigo_api::server::ApiServer;
        use std::sync::atomic::{AtomicBool, Ordering};

        info!("Starting in HEADLESS mode (no window/renderer)");
        info!("API server on port {}", self.config.dev.api_port);

        let vw = self.config.render.virtual_width as f32;
        let vh = self.config.render.virtual_height as f32;
        let mut game_ctx = GameContext::new(vw, vh, &self.assets_path);

        // Apply plugin registrations
        let plugin_ctx = self.plugin_ctx;
        for reg in plugin_ctx.event_registrations {
            reg(&mut game_ctx.events);
        }
        for ins in plugin_ctx.resource_insertions {
            ins(&mut game_ctx.resources);
        }

        // Initialize plugins
        for plugin in &self.plugins {
            plugin.init(&mut game_ctx);
        }

        // Initialize the game (there is no splash in headless mode)
        let mut stack = GameStack::new(Box::new(game));
        stack.enter_root(&mut game_ctx);

        let mut control = crate::api_bridge::ApiControl::default();
        if let Some(path) = &self.restore_snapshot {
            match crate::api_bridge::load_dev_snapshot(path) {
                Ok(snapshot) => crate::api_bridge::apply_dev_snapshot(
                    &snapshot,
                    &mut game_ctx,
                    &mut stack,
                    &mut control,
                ),
                Err(e) => warn!(
                    "Could not restore dev snapshot from {}: {e}. Starting fresh.",
                    path.display()
                ),
            }
        }

        // Start the API server
        let shared_state = new_shared_state();
        let _api_server = match ApiServer::start(self.config.dev.api_port, shared_state.clone()) {
            Ok(server) => server,
            Err(e) => {
                error!("Failed to start API server: {}", e);
                return;
            }
        };

        info!("Headless engine ready. Waiting for commands via JSON-RPC...");

        // Signal handling for graceful shutdown
        let running = Arc::new(AtomicBool::new(true));
        {
            let running = running.clone();
            // A failed registration means Ctrl+C will kill the process
            // outright instead of shutting down cleanly — worth saying so.
            if let Err(e) = ctrlc::set_handler(move || {
                running.store(false, Ordering::Relaxed);
            }) {
                warn!("Could not install the Ctrl+C handler, shutdown will not be graceful: {e}");
            }
        }

        let tick_duration = amigo_core::TimeInfo::TICK_DURATION;

        // Main headless loop: process API commands, run ticks on demand
        while running.load(Ordering::Relaxed) {
            crate::api_bridge::drain_api_commands(
                &shared_state,
                &mut game_ctx,
                &mut stack,
                &mut control,
            );

            if control.quit {
                info!("Headless: quit command received");
                break;
            }

            // Headless only advances when asked to; `paused` additionally
            // suppresses those requests so a client can freeze the simulation
            // without racing its own pending ticks.
            let ticks_requested = if control.paused {
                0
            } else {
                std::mem::take(&mut control.pending_ticks)
            };
            let mut quit = false;

            // Execute requested ticks at max CPU speed
            if ticks_requested > 0 {
                let start = Instant::now();
                for _ in 0..ticks_requested {
                    game_ctx.time.dt = tick_duration as f32;
                    game_ctx.time.elapsed += tick_duration;

                    let Some(active) = stack.top_mut() else {
                        quit = true;
                        break;
                    };
                    let action = active.update(&mut game_ctx);
                    game_ctx.time.tick += 1;

                    if !stack.apply(action, &mut game_ctx) {
                        quit = true;
                        break;
                    }

                    game_ctx.world.flush();
                    game_ctx.events.flush();
                    game_ctx.particles.update(tick_duration as f32);
                    // Clear edge-detected input AFTER the update consumed
                    // it (clearing before update would hide injected
                    // just-pressed state from the game).
                    game_ctx.input.begin_frame();
                }
                let elapsed = start.elapsed();
                info!(
                    "Headless: executed {} ticks in {:.1}ms ({:.0} ticks/sec)",
                    ticks_requested,
                    elapsed.as_secs_f64() * 1000.0,
                    ticks_requested as f64 / elapsed.as_secs_f64().max(0.000001),
                );

                if quit {
                    break;
                }
            }

            // Handle screenshot requests (no GPU in headless — return error)
            {
                let mut state = amigo_api::lock_or_recover(&shared_state);
                let requests = state.drain_screenshot_requests();
                for req in &requests {
                    state.screenshot_results.push(serde_json::json!({
                        "ok": false,
                        "error": "Screenshots not available in headless mode (no GPU renderer)",
                        "path": req.path,
                    }));
                }
            }

            // Update shared state snapshot for API queries
            {
                let mut state = amigo_api::lock_or_recover(&shared_state);
                state.snapshot.tick = game_ctx.time.tick;
                state.snapshot.entity_count = game_ctx.world.entity_count();
            }
            crate::api_bridge::publish_snapshot(&shared_state, &game_ctx, &control);

            // If no ticks were requested, sleep briefly to avoid busy-waiting
            if ticks_requested == 0 {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        info!("Headless engine shutting down");
    }
}

/// Load sprites into `assets`, preferring a packed archive over loose files.
///
/// `amigo pack` writes `<assets>/packed/game.pak`, but nothing ever read it back:
/// `AssetManager::load_from_pak` had no caller, so a packed release build still
/// needed the loose `assets/` tree beside the binary. Returns whether the pak was
/// used, which also decides whether hot reload can run.
pub fn load_assets(assets: &mut AssetManager, assets_path: &str) -> bool {
    let pak_path = std::path::Path::new(assets_path)
        .join("packed")
        .join("game.pak");

    if pak_path.exists() {
        match assets.load_from_pak(&pak_path) {
            Ok(_reader) => {
                info!(
                    "Loaded {} sprite(s) from {}",
                    assets.sprite_names().len(),
                    pak_path.display()
                );
                // The reader also holds audio, data, levels and fonts. Those
                // paths are not wired to the pak yet, so it is dropped here
                // rather than pretending otherwise.
                return true;
            }
            Err(e) => {
                warn!(
                    "Could not read {}: {e}. Falling back to loose files.",
                    pak_path.display()
                );
            }
        }
    }

    if let Err(e) = assets.load_sprites() {
        error!("Failed to load sprites: {}", e);
    }
    false
}

/// Upload any dirty font atlas textures to the GPU.
/// Handle one hot-reload file change: PNGs under `<assets>/sprites/` are
/// re-read, re-uploaded to the GPU, and re-registered under their sprite
/// name (the old texture stays resident until shutdown — acceptable for
/// dev mode). Other asset types are not live-reloadable yet and get a
/// visible warning instead of being silently ignored.
fn reload_changed_asset(
    path: &std::path::Path,
    renderer: &mut Renderer,
    game_ctx: &mut GameContext,
) {
    // Reload first, then register: the borrow of `game_ctx.assets` has to end
    // before `register_sprite_texture` takes `&mut game_ctx`.
    let reloaded = game_ctx.assets.reload_sprite(path).map(|sprite| {
        (
            sprite.name.clone(),
            sprite.image.clone(),
            sprite.width,
            sprite.height,
        )
    });
    if let Some((name, image, width, height)) = reloaded {
        let tex_id = renderer.load_texture(&image, &name);
        game_ctx.register_sprite_texture(name.clone(), tex_id, width, height);
        info!("Hot reload: sprite '{}' reloaded", name);
    } else {
        warn!(
            "Hot reload: '{}' changed but is not a reloadable sprite (restart to apply)",
            path.display()
        );
    }
}

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
    hot_reloader: Option<HotReloader>,
    sprite_draw_list: Vec<SpriteInstance>,
    last_frame: Instant,
    accumulator: f64,
    splash: Option<SplashState>,
    #[cfg(feature = "api")]
    api_state: Option<ApiEngineState>,
    #[cfg(feature = "api")]
    api_control: crate::api_bridge::ApiControl,
    #[cfg(feature = "editor")]
    egui: amigo_render::egui_integration::EguiRenderer,
    #[cfg(feature = "editor")]
    editor_state: amigo_editor::EditorState,
    #[cfg(feature = "editor")]
    editor_level: amigo_editor::AmigoLevel,
}

#[cfg(feature = "api")]
struct ApiEngineState {
    shared_state: amigo_api::handler::SharedState,
    _server: amigo_api::server::ApiServer,
}

struct EngineApp {
    config: EngineConfig,
    assets_path: String,
    stack: GameStack,
    plugins: Vec<Box<dyn Plugin>>,
    plugin_ctx: Option<PluginContext>,
    state: Option<EngineState>,
    /// Dev snapshot to restore once the contexts exist, from
    /// `EngineBuilder::restore_snapshot`.
    restore_snapshot: Option<std::path::PathBuf>,
}

impl ApplicationHandler for EngineApp {
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

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let mut renderer = pollster::block_on(Renderer::new(
            window.clone(),
            self.config.render.virtual_width,
            self.config.render.virtual_height,
        ));

        let vw = self.config.render.virtual_width as f32;
        let vh = self.config.render.virtual_height as f32;
        let mut game_ctx = GameContext::new(vw, vh, &self.assets_path);
        let packed = load_assets(&mut game_ctx.assets, &self.assets_path);

        // Hot reload only makes sense against loose files: a pak is a build
        // artifact, and watching it would reload the whole archive per write.
        let hot_reloader = if self.config.dev.hot_reload && !packed {
            HotReloader::new(std::path::PathBuf::from(&self.assets_path))
        } else {
            if packed && self.config.dev.hot_reload {
                info!("Hot reload disabled: assets are served from game.pak");
            }
            None
        };

        // Load built-in pixel font at 7px (native size)
        if let Err(e) = game_ctx.fonts.load_builtin(7.0) {
            error!("Failed to load built-in font: {}", e);
        }

        // Upload loaded sprites to GPU and register them so games can draw
        // them by name via DrawContext::draw_sprite.
        for name in game_ctx.assets.sprite_names().to_vec() {
            let uploaded = game_ctx.assets.sprite(&name).map(|sprite| {
                (
                    renderer.load_texture(&sprite.image, &name),
                    sprite.width,
                    sprite.height,
                )
            });
            if let Some((tex_id, w, h)) = uploaded {
                game_ctx.register_sprite_texture(name, tex_id, w, h);
            }
        }

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

        // A restored dev session skips the splash: `amigo dev` restarts the
        // process on every source change, and sitting through the logo each time
        // is the opposite of what the dev loop is for.
        let skip_splash = self.restore_snapshot.is_some();
        let splash = if self.config.splash.enabled && !skip_splash {
            Some(SplashState::new())
        } else {
            // No splash — init game immediately
            self.stack.enter_root(&mut game_ctx);
            None
        };

        // Start API server if configured
        #[cfg(feature = "api")]
        let api_state = if self.config.dev.api_server {
            let shared = amigo_api::handler::new_shared_state();
            match amigo_api::server::ApiServer::start(self.config.dev.api_port, shared.clone()) {
                Ok(server) => {
                    info!("API server started on port {}", self.config.dev.api_port);
                    Some(ApiEngineState {
                        shared_state: shared,
                        _server: server,
                    })
                }
                Err(e) => {
                    error!("Failed to start API server: {}", e);
                    None
                }
            }
        } else {
            None
        };

        #[cfg(feature = "editor")]
        let egui = amigo_render::egui_integration::EguiRenderer::new(
            &renderer.device,
            renderer.surface_config.format,
            &window,
        );

        info!("Engine initialized successfully");

        self.state = Some(EngineState {
            window,
            renderer,
            game_ctx,
            debug: DebugOverlay::new(),
            hot_reloader,
            sprite_draw_list: Vec::new(),
            last_frame: Instant::now(),
            accumulator: 0.0,
            splash,
            #[cfg(feature = "api")]
            api_state,
            #[cfg(feature = "api")]
            api_control: crate::api_bridge::ApiControl::default(),
            #[cfg(feature = "editor")]
            egui,
            #[cfg(feature = "editor")]
            editor_state: amigo_editor::EditorState::new(),
            #[cfg(feature = "editor")]
            editor_level: amigo_editor::AmigoLevel {
                name: "Untitled".to_string(),
                width: 30,
                height: 20,
                tile_size: 16,
                layers: vec![amigo_editor::LayerData {
                    name: "ground".to_string(),
                    tiles: vec![0; 600],
                    visible: true,
                }],
                entities: Vec::new(),
                paths: Vec::new(),
                metadata: std::collections::HashMap::new(),
            },
        });

        // Restore a dev snapshot, now that the game has been initialized (the
        // splash was skipped above, so the root game's init already ran).
        #[cfg(feature = "api")]
        if let Some(path) = self.restore_snapshot.take() {
            let Some(state) = &mut self.state else { return };
            match crate::api_bridge::load_dev_snapshot(&path) {
                Ok(snapshot) => crate::api_bridge::apply_dev_snapshot(
                    &snapshot,
                    &mut state.game_ctx,
                    &mut self.stack,
                    &mut state.api_control,
                ),
                Err(e) => warn!(
                    "Could not restore dev snapshot from {}: {e}. Starting fresh.",
                    path.display()
                ),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return };

        // Forward events to egui first; if consumed, skip game input
        #[cfg(feature = "editor")]
        let egui_consumed = state.egui.handle_window_event(&state.window, &event);
        #[cfg(not(feature = "editor"))]
        let egui_consumed = false;

        match event {
            WindowEvent::CloseRequested => {
                info!("Window close requested");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if !egui_consumed {
                    state
                        .game_ctx
                        .input
                        .handle_key_event(event.physical_key, event.state);
                }

                // Debug overlay toggle (always active, even when egui has focus)
                if event.state == ElementState::Pressed {
                    if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                        match code {
                            KeyCode::F1 => state.debug.toggle(),
                            KeyCode::F2 => state.debug.show_grid = !state.debug.show_grid,
                            KeyCode::F3 => state.debug.show_collision = !state.debug.show_collision,
                            KeyCode::F4 => state.debug.show_paths = !state.debug.show_paths,
                            KeyCode::F5 => {
                                state.debug.show_entity_ids = !state.debug.show_entity_ids
                            }
                            KeyCode::F6 => state.debug.show_tile_ids = !state.debug.show_tile_ids,
                            KeyCode::F7 => {
                                state.debug.show_audio_debug = !state.debug.show_audio_debug
                            }
                            KeyCode::F8 => {
                                state.debug.show_network_debug = !state.debug.show_network_debug
                            }
                            _ => {}
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                if !egui_consumed {
                    state
                        .game_ctx
                        .input
                        .handle_mouse_move(position.x as f32, position.y as f32);

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
            }

            WindowEvent::MouseInput {
                state: btn_state,
                button,
                ..
            } => {
                if !egui_consumed {
                    state.game_ctx.input.handle_mouse_button(button, btn_state);
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if !egui_consumed {
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 / 120.0,
                    };
                    state.game_ctx.input.handle_scroll(scroll);
                }
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
                        self.stack.enter_root(&mut state.game_ctx);
                    }
                    return;
                }

                // ── Normal game loop ─────────────────────────────────
                let _frame_span = info_span!("frame").entered();

                // Execute queued API commands. This has to happen while the
                // camera still lives on the GameContext (it is swapped into the
                // renderer further down) and before the tick, so a client's
                // pause/step lands on this frame rather than the next.
                #[cfg(feature = "api")]
                if let Some(ref api) = state.api_state {
                    crate::api_bridge::drain_api_commands(
                        &api.shared_state,
                        &mut state.game_ctx,
                        &mut self.stack,
                        &mut state.api_control,
                    );
                    if state.api_control.quit {
                        info!("Quit requested over the API");
                        event_loop.exit();
                        return;
                    }
                }

                // Simulation time control from the API: `set_speed` scales the
                // accumulator, `pause` stops feeding it, and `tick`/`debug.step`
                // request ticks that run even while paused.
                #[cfg(feature = "api")]
                let (sim_paused, sim_speed, forced_ticks) = {
                    let c = &mut state.api_control;
                    (c.paused, c.speed, std::mem::take(&mut c.pending_ticks))
                };
                #[cfg(not(feature = "api"))]
                let (sim_paused, sim_speed, forced_ticks) = (false, 1.0f32, 0u64);

                if !sim_paused {
                    state.accumulator += dt * sim_speed as f64;
                }

                state.game_ctx.time.dt = dt as f32;
                state.game_ctx.time.elapsed += dt;

                // Hot reload: re-upload changed sprite textures.
                if let Some(reloader) = &state.hot_reloader {
                    for path in reloader.poll_changes() {
                        reload_changed_asset(&path, &mut state.renderer, &mut state.game_ctx);
                    }
                }

                // Fixed timestep simulation. Ticks come from two sources: the
                // accumulator (real time) and `forced_ticks` (an API step
                // request, which must advance even while paused).
                let tick_duration = amigo_core::TimeInfo::TICK_DURATION;
                let mut ticks_ran = false;
                let mut forced_remaining = forced_ticks;
                while state.accumulator >= tick_duration || forced_remaining > 0 {
                    let _tick_span = info_span!("tick").entered();
                    ticks_ran = true;
                    // A forced tick does not consume accumulated real time.
                    let forced = state.accumulator < tick_duration;
                    if forced {
                        forced_remaining -= 1;
                    }

                    let Some(active) = self.stack.top_mut() else {
                        event_loop.exit();
                        return;
                    };
                    let action = {
                        let _update_span = info_span!("game_update").entered();
                        active.update(&mut state.game_ctx)
                    };
                    state.game_ctx.time.tick += 1;

                    // Push/Pop/Replace run the stack's lifecycle hooks; a
                    // `false` return means Quit, or the last game popped
                    // itself off and there is nothing left to run.
                    if !self.stack.apply(action, &mut state.game_ctx) {
                        event_loop.exit();
                        return;
                    }

                    // Plugin per-frame update (after game systems, before render)
                    {
                        let _plugin_span = info_span!("plugin_update").entered();
                        for plugin in &mut self.plugins {
                            plugin.update(&mut state.game_ctx);
                        }
                    }

                    {
                        let _flush_span = info_span!("ecs_flush").entered();
                        state.game_ctx.world.flush();
                        state.game_ctx.events.flush();
                    }
                    state.game_ctx.particles.update(tick_duration as f32);
                    if !forced {
                        state.accumulator -= tick_duration;
                    }
                }

                // Clear edge-detected input (just pressed/released) only
                // after the simulation consumed it, and only if a tick
                // actually ran this frame. Clearing at tick START would wipe
                // the events winit delivered before this redraw, so
                // `just_pressed` would never be observable; clearing on
                // zero-tick frames would drop presses that arrive between
                // ticks.
                if ticks_ran {
                    state.game_ctx.input.begin_frame();
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
                    let _draw_span = info_span!("game_draw").entered();
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
                    if let Some(active) = self.stack.top() {
                        active.draw(&mut draw_ctx);
                    }
                }

                // Collect particle sprites
                let white_tex = state.renderer.white_texture_id;
                state
                    .game_ctx
                    .particles
                    .collect_sprites(&mut state.sprite_draw_list, white_tex);

                // Push sprites to batcher
                for sprite in &state.sprite_draw_list {
                    state.renderer.batcher.push(sprite.clone());
                }

                // Process screenshot requests from API (before render clears batcher)
                #[cfg(feature = "api")]
                if let Some(ref api) = state.api_state {
                    let requests = {
                        let mut s = amigo_api::lock_or_recover(&api.shared_state);
                        s.drain_screenshot_requests()
                    };
                    for req in &requests {
                        let result = state.renderer.capture_screenshot(&req.path);
                        let mut s = amigo_api::lock_or_recover(&api.shared_state);
                        match result {
                            Ok(()) => {
                                s.screenshot_results.push(serde_json::json!({
                                    "ok": true,
                                    "path": req.path,
                                }));
                            }
                            Err(e) => {
                                s.screenshot_results.push(serde_json::json!({
                                    "ok": false,
                                    "error": e,
                                }));
                            }
                        }
                    }
                }

                // Update debug overlay
                state.debug.update(
                    dt,
                    state.game_ctx.world.entity_count(),
                    state.renderer.draw_call_count(),
                );

                // Render frame
                let _render_span = info_span!("gpu_render").entered();

                #[cfg(feature = "editor")]
                {
                    // Render sprites first, then overlay egui on top
                    match state.renderer.begin_frame() {
                        Ok(frame) => {
                            // Submit sprite pass
                            state
                                .renderer
                                .queue
                                .submit(std::iter::once(frame.encoder.finish()));

                            // Render egui overlay on top
                            let (sw, sh) = state.renderer.window_size();
                            let screen_desc =
                                amigo_render::egui_integration::egui_wgpu::ScreenDescriptor {
                                    size_in_pixels: [sw, sh],
                                    pixels_per_point: state.window.scale_factor() as f32,
                                };
                            let editor_state = &mut state.editor_state;
                            let editor_level = &state.editor_level;
                            state.egui.render(
                                &state.renderer.device,
                                &state.renderer.queue,
                                &state.window,
                                &frame.view,
                                screen_desc,
                                |ctx| {
                                    amigo_editor::egui_ui::draw_editor_panels(
                                        ctx,
                                        editor_state,
                                        editor_level,
                                    );
                                },
                            );

                            frame.output.present();
                            state.renderer.batcher.clear();
                        }
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
                }

                #[cfg(not(feature = "editor"))]
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

                // Update API snapshot after frame
                #[cfg(feature = "api")]
                if let Some(ref api) = state.api_state {
                    let mut s = amigo_api::lock_or_recover(&api.shared_state);
                    s.snapshot.tick = state.game_ctx.time.tick;
                    s.snapshot.fps = state.debug.fps() as f32;
                    s.snapshot.entity_count = state.game_ctx.world.entity_count();
                    s.snapshot.draw_calls = state.renderer.draw_call_count();
                }

                // Swap camera back to GameContext so game code can read updated state
                std::mem::swap(&mut state.game_ctx.camera, &mut state.renderer.camera);

                // Publish pause/speed/camera for `engine.status` and `camera.get`.
                // After the swap-back, so the camera reported is the updated one.
                #[cfg(feature = "api")]
                if let Some(ref api) = state.api_state {
                    crate::api_bridge::publish_snapshot(
                        &api.shared_state,
                        &state.game_ctx,
                        &state.api_control,
                    );
                }

                // Mark frame end for Tracy profiler
                amigo_debug::frame_mark();
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
