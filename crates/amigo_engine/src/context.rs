use amigo_animation::AnimPlayer;
use amigo_assets::{AssetManager, SpriteFrame};
use amigo_core::events::EventHub;
use amigo_core::level_loader::LoadedLevel;
use amigo_core::resources::Resources;
use amigo_core::save::{SaveConfig, SaveManager};
use amigo_core::scheduler::TickScheduler;
use amigo_core::{Color, Rect, RenderVec2, SimRng, TimeInfo, World};
use amigo_input::{ActionBindings, ActionState, GamepadState, InputState};
use amigo_render::camera::Camera;
use amigo_render::font::{FontAtlas, FontId, FontManager, TextMetrics, TextStyle};
use amigo_render::lighting::LightingState;
use amigo_render::particles::ParticleSystem;
use amigo_render::post_process::PostEffect;
use amigo_render::post_shader::{PostShaderRegistry, ShaderError};
use amigo_render::shapes::{self, ConvexShape};
use amigo_render::sprite_batcher::SpriteInstance;
use amigo_render::texture::TextureId;
use amigo_render::{ArtStyle, ScaleMode, ViewportInfo};
use amigo_tilemap::{TileId, TileLayer};
use amigo_ui::UiContext;

#[cfg(feature = "audio")]
use amigo_audio::AudioManager;

/// A level file under `assets/levels/` changed: the editor saved it, or it
/// was edited on disk while hot reload is on.
///
/// Read it like any event; [`GameContext::level_reloaded`] does that for one
/// level by name. It may arrive more than once for one save (the editor and
/// the file watcher both report it), so reloading should be idempotent.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelReloaded {
    /// File name without `.amigo`, as [`GameContext::load_level`] takes it.
    pub name: String,
    pub path: std::path::PathBuf,
}

/// Context passed to Game::update() with access to all engine systems.
pub struct GameContext {
    pub world: World,
    pub input: InputState,
    /// Connected gamepads. The engine polls them once per frame; like
    /// `input`, presses and releases are visible to exactly one tick.
    ///
    /// A context built directly (in a test, say) has no gamepad backend and
    /// reports none connected, so constructing one touches no OS device API.
    pub gamepad: GamepadState,
    /// Named actions ("jump", "pause") bound to keys, mouse buttons and
    /// gamepad buttons. The engine loads them from `input.ron` (the path is
    /// `[input] bindings` in `amigo.toml`) over the defaults given to
    /// `EngineBuilder::input_bindings`; a game can also edit them at runtime,
    /// e.g. from a rebinding menu.
    pub bindings: ActionBindings,
    /// Which bound actions are pressed, held or released this tick, refreshed
    /// from `input`, `gamepad` and `bindings` before every `Game::update`.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(ctx: &mut GameContext) {
    /// if ctx.actions.pressed("jump") {
    ///     // ...
    /// }
    /// # }
    /// ```
    pub actions: ActionState,
    pub time: TimeInfo,
    /// Random numbers for gameplay. Draw from this rather than an RNG of
    /// your own: the engine seeds it (`[dev] seed` in `amigo.toml`,
    /// `AMIGO_SEED`, or the clock; see [`seed`](Self::seed)) and replays
    /// restore it, so a recorded session draws the same numbers again.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(ctx: &mut GameContext) {
    /// let damage = ctx.rng.range(3, 7);
    /// # }
    /// ```
    pub rng: SimRng,
    pub camera: Camera,
    pub save: SaveManager,
    pub scheduler: TickScheduler,
    pub particles: ParticleSystem,
    pub fonts: FontManager,
    /// Double-buffered typed event system.
    pub events: EventHub,
    /// Typed resource storage for game-specific singletons.
    pub resources: Resources,
    /// Immediate-mode UI for this frame.
    ///
    /// Call `ui.begin()` at the start of `update`, build widgets, and the engine
    /// renders the resulting draw commands in a screen-space pass after
    /// post-processing. Widget input is read from `ctx.input`.
    ///
    /// Before this was wired, `UiContext` existed and produced draw commands that
    /// nothing consumed, so every widget drew nothing.
    pub ui: UiContext,
    /// Ambient and point lights for this frame.
    ///
    /// The lighting composite runs between the sprite pass and post-processing.
    /// It is skipped entirely while this is neutral (white ambient at full
    /// intensity, no point lights), which is the default.
    ///
    /// Before this was wired, `LightingState` could collect lights and pack them
    /// for the GPU, but there was no shader or pass to consume the bytes.
    pub lighting: LightingState,
    /// Post-processing effects to run this frame, applied in order.
    ///
    /// The engine copies these into the renderer when they change, so a game can
    /// set them from `update` without touching the renderer. An empty stack means
    /// the scene draws straight to the surface with no extra pass.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(ctx: &mut GameContext) {
    /// ctx.post_effects = vec![PostEffect::Vignette { intensity: 0.4, smoothness: 0.5 }];
    /// # }
    /// ```
    pub post_effects: Vec<PostEffect>,
    /// Post shaders registered with [`register_post_shader`](Self::register_post_shader).
    post_shaders: PostShaderRegistry,
    /// Loaded assets: sprites, and `load_ron` for game data.
    ///
    /// This used to live in the engine's private state, so game code could not
    /// reach `load_ron` at all and had to fall back to `std::fs` with hand-built
    /// paths.
    pub assets: AssetManager,
    /// The engine opens the output device at startup; a context built
    /// directly (in a test, say) opens it on its first sound.
    #[cfg(feature = "audio")]
    pub audio: AudioManager,
    #[cfg(feature = "async_tasks")]
    pub tasks: amigo_core::tasks::TaskPool,
    /// The window's layout, refreshed by the engine.
    viewport_info: ViewportInfo,
    /// Sprites the game can draw by name: texture, size and frames.
    sprites: std::collections::HashMap<String, SpriteEntry>,
    seed: u64,
    pub(crate) replay: crate::replay::ReplayDriver,
    pub(crate) net: crate::net::NetDriver,
}

impl GameContext {
    pub fn new(virtual_width: f32, virtual_height: f32, assets_path: &str) -> Self {
        let mut events = EventHub::new();
        events.register::<LevelReloaded>();
        events.register::<crate::net::NetEvent>();
        Self {
            world: World::new(),
            input: InputState::new(),
            gamepad: GamepadState::disabled(),
            bindings: ActionBindings::new(),
            actions: ActionState::new(),
            time: TimeInfo::new(),
            rng: SimRng::new(0),
            camera: Camera::new(virtual_width, virtual_height),
            save: SaveManager::new(SaveConfig {
                max_slots: 10,
                autosave_slots: 3,
                autosave_interval_secs: 300.0,
                app_name: "amigo_game".to_string(),
            }),
            scheduler: TickScheduler::new(),
            particles: ParticleSystem::new(),
            fonts: FontManager::new(),
            events,
            resources: Resources::new(),
            assets: AssetManager::new(assets_path),
            ui: UiContext::new(),
            lighting: LightingState::new(),
            post_effects: Vec::new(),
            post_shaders: PostShaderRegistry::new(),
            #[cfg(feature = "audio")]
            audio: AudioManager::new(assets_path),
            #[cfg(feature = "async_tasks")]
            tasks: amigo_core::tasks::TaskPool::new(),
            viewport_info: ViewportInfo::compute(
                ScaleMode::PixelPerfect,
                ArtStyle::PixelArt,
                (virtual_width as u32, virtual_height as u32),
                (virtual_width as u32, virtual_height as u32),
                1.0,
            ),
            sprites: std::collections::HashMap::new(),
            seed: 0,
            replay: Default::default(),
            net: Default::default(),
        }
    }

    /// What the game needs to lay out for its window: window size, where the
    /// scene lands, the current virtual resolution (which changes under
    /// `scale_mode = "expand"`), the render scale and the DPI factor.
    /// Refreshed by the engine before every `update` and on every resize;
    /// before the first window event it describes the configured window.
    pub fn viewport_info(&self) -> ViewportInfo {
        self.viewport_info
    }

    pub(crate) fn set_viewport_info(&mut self, info: ViewportInfo) {
        self.viewport_info = info;
    }

    /// The seed [`rng`](Self::rng) started from: `[dev] seed` in
    /// `amigo.toml`, `AMIGO_SEED`, or one taken from the clock. The engine
    /// logs it at startup, so a session can be rerun with the same numbers.
    /// A context built directly starts from seed 0.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Restart [`rng`](Self::rng) from `seed`.
    pub fn reseed(&mut self, seed: u64) {
        self.seed = seed;
        self.rng = SimRng::new(seed);
    }

    /// Whether a replay is being recorded or played, and how far it got.
    pub fn replay_status(&self) -> crate::replay::ReplayStatus {
        self.replay.status()
    }

    /// The players in this game: player 0 alone, or players 0 and 1 in a
    /// network game (`amigo run --host` / `--join`). Write gameplay against
    /// these and the same code runs alone and with two.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(ctx: &mut GameContext, pos: &mut [SimVec2; 2]) {
    /// for p in ctx.players() {
    ///     if ctx.player_actions(p).held("right") {
    ///         pos[p.0 as usize].x += Fix::ONE;
    ///     }
    /// }
    /// # }
    /// ```
    pub fn players(&self) -> impl Iterator<Item = amigo_net::PlayerId> + use<> {
        let count = if self.net.is_playing() { 2 } else { 1 };
        (0..count).map(amigo_net::PlayerId)
    }

    /// The player at this machine: 0 alone or as host, 1 as guest.
    pub fn local_player(&self) -> amigo_net::PlayerId {
        self.net.local_player()
    }

    /// `player`'s bound actions this tick. In a network game they arrive
    /// `input_delay` ticks after they were pressed, the local player's too,
    /// so both machines see the same thing; alone they are
    /// [`actions`](Self::actions). A player not in the game holds nothing.
    pub fn player_actions(&self, player: amigo_net::PlayerId) -> &ActionState {
        static NOBODY: std::sync::OnceLock<ActionState> = std::sync::OnceLock::new();
        match self.net.player_actions(player) {
            Some(actions) => actions,
            None if player.0 == 0 => &self.actions,
            None => NOBODY.get_or_init(ActionState::new),
        }
    }

    /// `player`'s cursor in world coordinates this tick, in fixed point.
    /// Alone it is the mouse's world position.
    pub fn player_cursor(&self, player: amigo_net::PlayerId) -> Option<amigo_core::SimVec2> {
        if self.net.is_playing() {
            return self.net.player_cursor(player);
        }
        (player.0 == 0).then(|| {
            let m = self.input.mouse_world_pos();
            amigo_core::SimVec2::new(
                amigo_core::Fix::saturating_from_num(m.x),
                amigo_core::Fix::saturating_from_num(m.y),
            )
        })
    }

    /// Whether a network game is connecting, running or over, and how it
    /// goes.
    pub fn net_status(&self) -> crate::net::NetStatus {
        self.net.status()
    }

    /// Refresh [`actions`](Self::actions) from the current input. The engine
    /// calls this before every tick (a playing replay sets the recorded
    /// actions instead); call it after injecting input yourself, e.g. in a
    /// test.
    pub fn update_actions(&mut self) {
        self.actions
            .update(&self.input, &self.bindings, Some(&self.gamepad));
    }

    /// Clear the one-tick input events (presses, releases, scroll, typed
    /// text, gamepad hot-plug) once a tick has consumed them.
    pub(crate) fn end_tick_input(&mut self) {
        self.input.begin_frame();
        self.gamepad.begin_frame();
    }

    /// Load `assets/levels/{name}.amigo`, the format the editor saves and
    /// `amigo new` writes.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(ctx: &mut GameContext) -> Result<(), String> {
    /// let level = ctx.load_level("level_01")?;
    /// let walls = level.collision_from_layer("ground");
    /// # Ok(()) }
    /// ```
    pub fn load_level(&self, name: &str) -> Result<LoadedLevel, String> {
        let path = self
            .assets
            .base_path()
            .join("levels")
            .join(format!("{name}.amigo"));
        amigo_core::level_loader::load_level_from_file(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Whether the level `name` changed on disk since the last tick (see
    /// [`LevelReloaded`]). Call [`load_level`](Self::load_level) again when it
    /// did.
    pub fn level_reloaded(&self, name: &str) -> bool {
        self.events
            .read::<LevelReloaded>()
            .iter()
            .any(|e| e.name == name)
    }

    /// Announce that the level file at `path` changed.
    pub(crate) fn emit_level_reloaded(&mut self, path: &std::path::Path) {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.events.emit(LevelReloaded {
            name,
            path: path.to_path_buf(),
        });
    }

    /// Register (or replace) the post shader `name`, for
    /// `PostEffect::Custom { shader: name, .. }`. Its source is the fragment
    /// stage only; the engine prepends the prelude
    /// (`amigo_render::post_shader::POST_PRELUDE`). Parsed and validated now;
    /// compiled for the GPU at the start of the next frame.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(ctx: &mut GameContext) -> Result<(), ShaderError> {
    /// ctx.register_post_shader("tint", r#"
    /// @fragment
    /// fn fs_main(in: PostVertexOutput) -> @location(0) vec4<f32> {
    ///     return textureSample(scene, scene_sampler, in.uv) * post.params[0];
    /// }
    /// "#)?;
    /// let mut params = [0.0; 16];
    /// params[..4].copy_from_slice(&[1.0, 0.8, 0.8, 1.0]);
    /// ctx.post_effects = vec![PostEffect::Custom { shader: "tint".into(), params }];
    /// # Ok(()) }
    /// ```
    pub fn register_post_shader(&mut self, name: &str, wgsl: &str) -> Result<(), ShaderError> {
        self.post_shaders.register(name, wgsl)
    }

    /// The post shaders registered so far.
    pub fn post_shaders(&self) -> &PostShaderRegistry {
        &self.post_shaders
    }

    /// Load a TTF/OTF font at the given pixel size. Returns a FontId handle.
    pub fn load_font(&mut self, data: &[u8], px: f32) -> Result<FontId, String> {
        self.fonts.load_font(data, px)
    }

    /// Register (or replace) the texture backing a sprite name, as one frame
    /// covering the texture. Replacing an existing entry keeps hot reload
    /// working: draws by name pick up the new texture on the next frame.
    pub fn register_sprite_texture(
        &mut self,
        name: String,
        texture_id: TextureId,
        width: u32,
        height: u32,
    ) {
        self.register_sprite(
            name,
            SpriteEntry {
                texture: texture_id,
                size: (width, height),
                frames: vec![SpriteFrame::whole(width, height)],
                atlas: false,
            },
        );
    }

    /// Register (or replace) a sprite with its frames.
    pub fn register_sprite(&mut self, name: String, entry: SpriteEntry) {
        self.sprites.insert(name, entry);
    }

    /// A registered sprite.
    pub fn sprite_entry(&self, name: &str) -> Option<&SpriteEntry> {
        self.sprites.get(name)
    }

    /// The texture of a sprite and the texture's size.
    pub fn find_sprite_texture(&self, name: &str) -> Option<(TextureId, u32, u32)> {
        self.sprites
            .get(name)
            .map(|e| (e.texture, e.size.0, e.size.1))
    }
}

/// A sprite the engine can draw by name.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteEntry {
    /// The texture it lives on: its own, or its atlas sheet's.
    pub texture: TextureId,
    /// Size of that texture in pixels.
    pub size: (u32, u32),
    /// At least one frame. A PNG has one covering the texture; an Aseprite
    /// strip one per cell; an atlas sprite the manifest's.
    pub frames: Vec<SpriteFrame>,
    /// The sprite lives on an atlas sheet: `draw_sprite` draws its first
    /// frame, and `draw_animated` draws through `draw_frame`.
    pub atlas: bool,
}

/// The corners of `rect`, top-left first, clockwise on screen.
fn rect_corners(rect: Rect) -> [RenderVec2; 4] {
    [
        RenderVec2::new(rect.x, rect.y),
        RenderVec2::new(rect.x + rect.w, rect.y),
        RenderVec2::new(rect.x + rect.w, rect.y + rect.h),
        RenderVec2::new(rect.x, rect.y + rect.h),
    ]
}

/// Which coordinate space [`DrawContext`] draws into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawSpace {
    /// World coordinates, through the camera, lighting and post-processing.
    #[default]
    World,
    /// Virtual-resolution screen coordinates, origin top-left, no camera,
    /// drawn in the UI pass after post-processing (conventions A.6).
    Screen,
}

/// What `Game::draw` asked of this frame's camera. The engine applies it to
/// the projection after `draw` returns; it never reaches `GameContext::camera`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FrameOverrides {
    pub camera_position: Option<RenderVec2>,
    pub camera_offset: RenderVec2,
}

impl FrameOverrides {
    /// Point `camera` at the overridden position for this frame and return
    /// the position to restore afterwards.
    pub(crate) fn apply(&self, camera: &mut Camera) -> RenderVec2 {
        let saved = camera.position;
        let base = self.camera_position.unwrap_or(saved);
        camera.position =
            RenderVec2::new(base.x + self.camera_offset.x, base.y + self.camera_offset.y);
        saved
    }
}

/// Context passed to Game::draw() for rendering.
pub struct DrawContext<'a> {
    pub sprites: &'a mut Vec<SpriteInstance>,
    /// The camera position this frame renders with: shake, a draw-time
    /// override and offset included (see [`camera_position`](Self::camera_position)).
    pub camera_pos: RenderVec2,
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub alpha: f32,
    game_ctx: &'a GameContext,
    white_texture: TextureId,
    /// The visible world rectangle, before any parallax shift.
    view: Rect,
    screen: Option<&'a mut Vec<SpriteInstance>>,
    warned_no_screen: bool,
    space: DrawSpace,
    z: i32,
    parallax: (f32, f32),
    /// The camera's position as of `update`, without shake.
    camera_base: RenderVec2,
    /// The camera's built-in shake this frame.
    camera_shake: RenderVec2,
    overrides: FrameOverrides,
    render_scale: f32,
    art_style: ArtStyle,
}

impl<'a> DrawContext<'a> {
    pub fn new(
        sprites: &'a mut Vec<SpriteInstance>,
        game_ctx: &'a GameContext,
        camera_pos: RenderVec2,
        virtual_width: f32,
        virtual_height: f32,
        alpha: f32,
        white_texture: TextureId,
    ) -> Self {
        Self {
            sprites,
            camera_pos,
            virtual_width,
            virtual_height,
            alpha,
            game_ctx,
            white_texture,
            // Unzoomed default; the engine passes the camera's real view.
            view: Rect::new(
                camera_pos.x - virtual_width / 2.0,
                camera_pos.y - virtual_height / 2.0,
                virtual_width,
                virtual_height,
            ),
            screen: None,
            warned_no_screen: false,
            space: DrawSpace::World,
            z: 0,
            parallax: (1.0, 1.0),
            camera_base: camera_pos,
            camera_shake: RenderVec2::ZERO,
            overrides: FrameOverrides::default(),
            render_scale: 1.0,
            art_style: ArtStyle::PixelArt,
        }
    }

    /// Take position, shake, zoom and view from `camera`, as the engine does
    /// for every frame.
    pub fn with_camera(mut self, camera: &Camera) -> Self {
        self.camera_base = camera.position;
        self.camera_shake = camera.shake_offset();
        self.camera_pos = camera.effective_position();
        self.virtual_width = camera.virtual_width;
        self.virtual_height = camera.virtual_height;
        self.view = camera.view_rect();
        self
    }

    /// Set the visible world rectangle (zoom and shake included) that
    /// culling uses, e.g. `Camera::view_rect()`.
    pub fn with_view(mut self, view: Rect) -> Self {
        self.view = view;
        self
    }

    /// Attach the list that screen-space draws go into. The engine always
    /// attaches one; a `DrawContext` built without it (in a test, say) drops
    /// screen-space draws and logs one warning.
    pub fn with_screen_list(mut self, screen: &'a mut Vec<SpriteInstance>) -> Self {
        self.screen = Some(screen);
        self
    }

    /// The visible rectangle in the coordinates of the current draw space:
    /// the screen `(0, 0, virtual_w, virtual_h)` in [`DrawSpace::Screen`], the
    /// world view (shifted against the parallax factor) in [`DrawSpace::World`].
    /// Draws outside it are wasted work.
    pub fn view_rect(&self) -> Rect {
        match self.space {
            DrawSpace::Screen => Rect::new(0.0, 0.0, self.virtual_width, self.virtual_height),
            DrawSpace::World => {
                let (dx, dy) = self.parallax_shift();
                Rect::new(self.view.x - dx, self.view.y - dy, self.view.w, self.view.h)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Draw space, z and parallax
    // -----------------------------------------------------------------------

    /// Route every following draw call into `space`.
    pub fn set_space(&mut self, space: DrawSpace) {
        self.space = space;
    }

    /// The current draw space.
    pub fn space(&self) -> DrawSpace {
        self.space
    }

    /// Run `f` with `space` active, then restore the previous space.
    pub fn in_space<R>(&mut self, space: DrawSpace, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.space;
        self.space = space;
        let result = f(self);
        self.space = previous;
        result
    }

    /// Default z-order for the following draws that do not take one
    /// explicitly (`draw_sprite`, `draw_animated`, `draw_rect`, tilemaps and
    /// shapes). Starts at 0. Text keeps its own z.
    pub fn set_z(&mut self, z_order: i32) {
        self.z = z_order;
    }

    /// The current default z-order.
    pub fn z(&self) -> i32 {
        self.z
    }

    /// World draws after this call move at `fx`/`fy` times the camera's
    /// speed: 1.0 is the world (default), 0.0 is fixed to the screen, 0.25 a
    /// distant layer. Ignored in [`DrawSpace::Screen`]. A non-finite factor is
    /// treated as 1.0.
    pub fn set_parallax(&mut self, fx: f32, fy: f32) {
        let finite = |f: f32| if f.is_finite() { f } else { 1.0 };
        self.parallax = (finite(fx), finite(fy));
    }

    /// The current parallax factor.
    pub fn parallax(&self) -> (f32, f32) {
        self.parallax
    }

    /// How far a world draw is shifted at the current parallax factor.
    fn parallax_shift(&self) -> (f32, f32) {
        let (fx, fy) = self.parallax;
        let cam = self.camera_pos;
        (cam.x * (1.0 - fx), cam.y * (1.0 - fy))
    }

    /// Queue one sprite in the current space, applying parallax in the world.
    /// Every draw call goes through here.
    pub fn push(&mut self, mut sprite: SpriteInstance) {
        match self.space {
            DrawSpace::World => {
                let (dx, dy) = self.parallax_shift();
                if dx != 0.0 || dy != 0.0 {
                    sprite.x += dx;
                    sprite.y += dy;
                    if let Some(geometry) = &mut sprite.geometry {
                        for corner in &mut geometry.corners {
                            corner[0] += dx;
                            corner[1] += dy;
                        }
                    }
                }
                self.sprites.push(sprite);
            }
            DrawSpace::Screen => match &mut self.screen {
                Some(screen) => screen.push(sprite),
                None => {
                    if !self.warned_no_screen {
                        tracing::warn!(
                            "DrawContext has no screen list: screen-space draws are dropped"
                        );
                        self.warned_no_screen = true;
                    }
                }
            },
        }
    }

    // -----------------------------------------------------------------------
    // Camera from draw()
    // -----------------------------------------------------------------------

    /// Render this frame with the camera centred on `center` instead of the
    /// position the camera reached in `update`. Affects only this frame's
    /// projection; `GameContext::camera` is not changed. Call it before
    /// drawing anything, so culling and parallax see the new camera.
    pub fn set_camera_position(&mut self, center: RenderVec2) {
        self.overrides.camera_position = Some(center);
        self.refresh_camera();
    }

    /// Add `offset` to this frame's camera position, e.g. a game's own shake.
    /// Added after `set_camera_position` and after the camera's built-in shake.
    pub fn set_camera_offset(&mut self, offset: RenderVec2) {
        self.overrides.camera_offset = offset;
        self.refresh_camera();
    }

    /// The camera position this frame renders with (shake and offset
    /// included), after any override.
    pub fn camera_position(&self) -> RenderVec2 {
        let base = self.overrides.camera_position.unwrap_or(self.camera_base);
        RenderVec2::new(
            base.x + self.camera_shake.x + self.overrides.camera_offset.x,
            base.y + self.camera_shake.y + self.overrides.camera_offset.y,
        )
    }

    /// What `draw` asked of the camera, for the engine to apply.
    pub(crate) fn frame_overrides(&self) -> FrameOverrides {
        self.overrides
    }

    fn refresh_camera(&mut self) {
        self.camera_pos = self.camera_position();
        self.view = Rect::new(
            self.camera_pos.x - self.view.w / 2.0,
            self.camera_pos.y - self.view.h / 2.0,
            self.view.w,
            self.view.h,
        );
    }

    /// Draw a sprite at a position: the whole image, or the first frame of
    /// an atlas sprite with its pivot at `pos`.
    pub fn draw_sprite(&mut self, name: &str, pos: RenderVec2) {
        self.draw_sprite_ex(name, pos, |_| {});
    }

    /// Draw a sprite with extended options.
    pub fn draw_sprite_ex<F>(&mut self, name: &str, pos: RenderVec2, f: F)
    where
        F: FnOnce(&mut SpriteInstance),
    {
        let Some(entry) = self.game_ctx.sprite_entry(name) else {
            return;
        };
        let mut instance = if entry.atlas {
            self.frame_instance(entry, 0, pos)
        } else {
            let (w, h) = entry.size;
            SpriteInstance {
                z_order: self.z,
                ..SpriteInstance::new(entry.texture, pos.x, pos.y, w as f32, h as f32)
            }
        };
        f(&mut instance);
        self.push(instance);
    }

    /// Draw frame `frame` of `sprite` with its pivot at `pos`. Frames past the
    /// last one draw the last frame. A PNG sprite has one frame with its pivot
    /// at the top-left; an Aseprite sprite one per frame of its strip; an
    /// atlas sprite the frames and origins of its manifest.
    pub fn draw_frame(&mut self, sprite: &str, frame: usize, pos: RenderVec2) {
        self.draw_frame_ex(sprite, frame, pos, |_| {});
    }

    /// [`draw_frame`](Self::draw_frame) with extended options.
    pub fn draw_frame_ex<F>(&mut self, sprite: &str, frame: usize, pos: RenderVec2, f: F)
    where
        F: FnOnce(&mut SpriteInstance),
    {
        let Some(entry) = self.game_ctx.sprite_entry(sprite) else {
            return;
        };
        let mut instance = self.frame_instance(entry, frame, pos);
        f(&mut instance);
        self.push(instance);
    }

    /// Number of frames of `sprite`; 0 when the name is unknown.
    pub fn frame_count(&self, sprite: &str) -> usize {
        self.game_ctx
            .sprite_entry(sprite)
            .map_or(0, |e| e.frames.len())
    }

    fn frame_instance(&self, entry: &SpriteEntry, frame: usize, pos: RenderVec2) -> SpriteInstance {
        let index = frame.min(entry.frames.len().saturating_sub(1));
        let Some(f) = entry.frames.get(index) else {
            return SpriteInstance::new(entry.texture, pos.x, pos.y, 0.0, 0.0);
        };
        SpriteInstance {
            uv_x: f.uv.x,
            uv_y: f.uv.y,
            uv_w: f.uv.w,
            uv_h: f.uv.h,
            origin: f.origin,
            z_order: self.z,
            ..SpriteInstance::new(entry.texture, pos.x, pos.y, f.w as f32, f.h as f32)
        }
    }

    /// Draw the current frame of `player`'s animation, from the sprite
    /// `sprite`, at `pos`. The quad is one frame in size.
    ///
    /// Animations come from Aseprite files: `sprites/hero.aseprite` registers
    /// the sprite `"hero"` and an animation `"hero/<tag>"` per tag.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(ctx: &mut GameContext, draw: &mut DrawContext, hero: &mut AnimPlayer) {
    /// // In update:
    /// hero.play("hero/walk", PlayMode::Loop);
    /// hero.advance(ctx.assets.animations());
    /// // In draw:
    /// draw.draw_animated("hero", hero, RenderVec2::new(100.0, 80.0));
    /// # }
    /// ```
    ///
    /// Draws nothing when the sprite or the animation is unknown.
    pub fn draw_animated(&mut self, sprite: &str, player: &AnimPlayer, pos: RenderVec2) {
        self.draw_animated_ex(sprite, player, pos, |_| {});
    }

    /// [`draw_animated`](Self::draw_animated) with extended options (flip,
    /// tint, z-order), like [`draw_sprite_ex`](Self::draw_sprite_ex).
    pub fn draw_animated_ex<F>(&mut self, sprite: &str, player: &AnimPlayer, pos: RenderVec2, f: F)
    where
        F: FnOnce(&mut SpriteInstance),
    {
        let Some(entry) = self.game_ctx.sprite_entry(sprite) else {
            return;
        };
        // An atlas sprite's animation steps through its own frames, origins
        // included.
        if entry.atlas {
            let mut instance = self.frame_instance(entry, player.frame_index, pos);
            f(&mut instance);
            self.push(instance);
            return;
        }
        let (tex_id, (w, h)) = (entry.texture, entry.size);
        let Some(animation) = self.game_ctx.assets.animation(&player.current_animation) else {
            return;
        };
        let uv = player.current_uv(animation);
        let mut instance = SpriteInstance {
            uv_x: uv.x,
            uv_y: uv.y,
            uv_w: uv.w,
            uv_h: uv.h,
            z_order: self.z,
            ..SpriteInstance::new(tex_id, pos.x, pos.y, w as f32 * uv.w, h as f32 * uv.h)
        };
        f(&mut instance);
        self.push(instance);
    }

    /// Draw a colored rectangle.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.push(SpriteInstance {
            tint: color,
            z_order: self.z,
            ..SpriteInstance::new(self.white_texture, rect.x, rect.y, rect.w, rect.h)
        });
    }

    // -----------------------------------------------------------------------
    // Text rendering (TTF via fontdue)
    // -----------------------------------------------------------------------

    /// Draw text using the default font (AmigoPixel unless
    /// `FontManager::set_default_font` chose another), at the font's load
    /// size, with `y` the top of the line. Any `char` can be drawn; one the
    /// font lacks draws its `.notdef` glyph. No kerning, so pixel-font output
    /// is unchanged. If no font is loaded, this is a no-op.
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, color: Color) {
        let game_ctx = self.game_ctx;
        if let Some(font) = game_ctx.fonts.default_font() {
            self.draw_text_legacy(font, text, x, y, color, 1.0);
        }
    }

    /// Draw text with the default font, scaled about its top-left corner.
    ///
    /// Glyphs are rasterized at the font's own pixel size and the quads are
    /// scaled, so large factors get blocky — which is what pixel-art UI wants.
    /// `scale` of 1.0 is identical to [`DrawContext::draw_text`].
    pub fn draw_text_scaled(&mut self, text: &str, x: f32, y: f32, color: Color, scale: f32) {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let game_ctx = self.game_ctx;
        if let Some(font) = game_ctx.fonts.default_font() {
            self.draw_text_legacy(font, text, x, y, color, scale);
        }
    }

    /// Draw text using a specific font by FontId.
    pub fn draw_text_font(&mut self, font_id: FontId, text: &str, x: f32, y: f32, color: Color) {
        let game_ctx = self.game_ctx;
        if let Some(font) = game_ctx.fonts.get(font_id) {
            self.draw_text_legacy(font, text, x, y, color, 1.0);
        }
    }

    /// The layout `draw_text` has always had: advances only, the line's top
    /// at `y` and its baseline at `y + px`, z 100.
    fn draw_text_legacy(
        &mut self,
        font: &FontAtlas,
        text: &str,
        x: f32,
        y: f32,
        color: Color,
        scale: f32,
    ) {
        let px = font.px;
        let mut cx = x;
        for ch in text.chars() {
            let Some(glyph) = font.glyph(ch) else {
                continue;
            };
            if glyph.width > 0.0 && glyph.height > 0.0 {
                self.push(SpriteInstance {
                    uv_x: glyph.uv_x,
                    uv_y: glyph.uv_y,
                    uv_w: glyph.uv_w,
                    uv_h: glyph.uv_h,
                    tint: color,
                    z_order: 100,
                    ..SpriteInstance::new(
                        glyph.texture_id,
                        cx + glyph.offset_x * scale,
                        y + (px - glyph.height - glyph.offset_y) * scale,
                        glyph.width * scale,
                        glyph.height * scale,
                    )
                });
            }
            cx += glyph.advance * scale;
        }
    }

    /// Measure text dimensions using the default font.
    /// Returns (width, height) in pixels.
    pub fn measure_text(&self, text: &str) -> (f32, f32) {
        if let Some(font) = self.game_ctx.fonts.default_font() {
            font.measure(text)
        } else {
            (0.0, 0.0)
        }
    }

    /// Measure text dimensions using a specific font.
    pub fn measure_text_font(&self, font_id: FontId, text: &str) -> (f32, f32) {
        if let Some(font) = self.game_ctx.fonts.get(font_id) {
            font.measure(text)
        } else {
            (0.0, 0.0)
        }
    }

    /// Draw one line of text. `pos.y` is the top of the line box; the
    /// baseline sits at `pos.y + ascent`. Kerning and `letter_spacing` apply,
    /// and the line is anchored per `style.align`. Glyphs are rasterised at the
    /// output resolution (`size_px * render_scale`), so raster-art text stays
    /// crisp. Returns the drawn line's bounds.
    ///
    /// ```no_run
    /// # use amigo_engine::prelude::*;
    /// # fn f(draw: &mut DrawContext) {
    /// let style = TextStyle { size_px: Some(14.0), align: TextAlign::Center, ..Default::default() };
    /// draw.draw_text_ex("Grüße!", RenderVec2::new(160.0, 20.0), &style);
    /// # }
    /// ```
    pub fn draw_text_ex(&mut self, text: &str, pos: RenderVec2, style: &TextStyle) -> Rect {
        let line = self
            .game_ctx
            .fonts
            .layout_line(text, pos, style, self.render_scale);
        for quad in &line.quads {
            self.push(SpriteInstance {
                uv_x: quad.uv[0],
                uv_y: quad.uv[1],
                uv_w: quad.uv[2],
                uv_h: quad.uv[3],
                tint: style.color,
                z_order: style.z_order,
                blend: style.blend,
                ..SpriteInstance::new(quad.texture_id, quad.x, quad.y, quad.width, quad.height)
            });
        }
        line.bounds
    }

    /// Measure one line exactly as [`draw_text_ex`](Self::draw_text_ex) would
    /// lay it out.
    pub fn measure_text_ex(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.game_ctx.fonts.measure_line(text, style)
    }

    /// The window's layout this frame; see [`GameContext::viewport_info`].
    pub fn viewport_info(&self) -> ViewportInfo {
        self.game_ctx.viewport_info()
    }

    /// Scene-target pixels per virtual pixel this frame: 1.0 for pixel art,
    /// the viewport scale for raster art.
    pub fn render_scale(&self) -> f32 {
        self.render_scale
    }

    /// Set the render scale text and shapes are rasterised for. The engine
    /// sets the renderer's.
    pub fn with_render_scale(mut self, render_scale: f32) -> Self {
        if render_scale.is_finite() && render_scale > 0.0 {
            self.render_scale = render_scale;
        }
        self
    }

    // -----------------------------------------------------------------------
    // Shapes
    // -----------------------------------------------------------------------

    /// The art style shapes are drawn for: under raster art every shape gets
    /// a one-scene-pixel feather on its outer edges; under pixel art edges
    /// are hard. The engine sets the renderer's.
    pub fn with_art_style(mut self, art_style: ArtStyle) -> Self {
        self.art_style = art_style;
        self
    }

    /// Width of the anti-aliasing feather, in virtual pixels; 0 for hard edges.
    fn feather(&self) -> f32 {
        if self.art_style == ArtStyle::RasterArt {
            1.0 / self.render_scale
        } else {
            0.0
        }
    }

    /// Tessellate `shape` and queue it on the white texture.
    fn push_shape(&mut self, shape: &ConvexShape) {
        let mut quads = Vec::new();
        shape.tessellate(self.feather(), &mut quads);
        for geometry in quads {
            self.push(SpriteInstance {
                geometry: Some(geometry),
                z_order: self.z,
                ..SpriteInstance::new(self.white_texture, 0.0, 0.0, 0.0, 0.0)
            });
        }
    }

    /// Any convex quad, corners in order TL, TR, BR, BL (or any consistent
    /// winding).
    pub fn draw_quad(&mut self, corners: [RenderVec2; 4], color: Color) {
        self.draw_quad_colors(corners, [color; 4]);
    }

    /// Like [`draw_quad`](Self::draw_quad) with one colour per corner,
    /// interpolated across.
    pub fn draw_quad_colors(&mut self, corners: [RenderVec2; 4], colors: [Color; 4]) {
        let shape = ConvexShape {
            points: corners.iter().map(|c| [c.x, c.y]).collect(),
            colors: colors.to_vec(),
            feather_edges: vec![true; 4],
        };
        self.push_shape(&shape);
    }

    /// Vertical gradient from `top` to `bottom`.
    pub fn draw_gradient_rect(&mut self, rect: Rect, top: Color, bottom: Color) {
        if !shapes::usable(rect.w) || !shapes::usable(rect.h) {
            return;
        }
        self.draw_quad_colors(rect_corners(rect), [top, top, bottom, bottom]);
    }

    /// A segment of `thickness` virtual pixels, centred on the line from `a`
    /// to `b`, with square ends.
    pub fn draw_line(&mut self, a: RenderVec2, b: RenderVec2, thickness: f32, color: Color) {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt();
        if !shapes::usable(thickness) || !shapes::usable(len) {
            return;
        }
        let (nx, ny) = (-dy / len * thickness / 2.0, dx / len * thickness / 2.0);
        self.push_shape(&ConvexShape::filled(
            vec![
                [a.x + nx, a.y + ny],
                [b.x + nx, b.y + ny],
                [b.x - nx, b.y - ny],
                [a.x - nx, a.y - ny],
            ],
            color,
        ));
    }

    /// The outline of `rect`, `thickness` pixels wide, inside the rect.
    pub fn draw_rect_outline(&mut self, rect: Rect, thickness: f32, color: Color) {
        if !shapes::usable(rect.w) || !shapes::usable(rect.h) || !shapes::usable(thickness) {
            return;
        }
        let t = thickness.min(rect.w / 2.0).min(rect.h / 2.0);
        let (x0, y0, x1, y1) = (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h);
        let strip = |l: f32, t_: f32, r: f32, b: f32, feather: [bool; 4]| ConvexShape {
            points: vec![[l, t_], [r, t_], [r, b], [l, b]],
            colors: vec![color; 4],
            feather_edges: feather.to_vec(),
        };
        // Top and bottom span the width; the sides sit between them, so no
        // pixel is covered twice. Edges shared by two strips get no feather.
        self.push_shape(&strip(x0, y0, x1, y0 + t, [true; 4]));
        if rect.h > 2.0 * t {
            self.push_shape(&strip(x0, y1 - t, x1, y1, [true; 4]));
            self.push_shape(&strip(
                x0,
                y0 + t,
                x0 + t,
                y1 - t,
                [false, true, false, true],
            ));
            self.push_shape(&strip(
                x1 - t,
                y0 + t,
                x1,
                y1 - t,
                [false, true, false, true],
            ));
        }
    }

    /// A filled circle.
    pub fn draw_circle(&mut self, center: RenderVec2, radius: f32, color: Color) {
        if !shapes::usable(radius) {
            return;
        }
        let segments = shapes::circle_segments(radius, self.render_scale);
        let points = shapes::circle_points([center.x, center.y], radius, segments);
        self.push_shape(&ConvexShape::filled(points, color));
    }

    /// A filled rectangle with rounded corners. `radius` is clamped to half
    /// the shorter side; `radius <= 0` draws a plain rectangle.
    pub fn draw_rounded_rect(&mut self, rect: Rect, radius: f32, color: Color) {
        if !shapes::usable(rect.w) || !shapes::usable(rect.h) {
            return;
        }
        let radius = if radius.is_finite() {
            radius.min(rect.w / 2.0).min(rect.h / 2.0)
        } else {
            0.0
        };
        let points = if radius > 0.0 {
            let segments = shapes::circle_segments(radius, self.render_scale);
            shapes::rounded_rect_points(rect, radius, segments)
        } else {
            rect_corners(rect).iter().map(|c| [c.x, c.y]).collect()
        };
        self.push_shape(&ConvexShape::filled(points, color));
    }

    /// A convex polygon with at least 3 points, in either winding. A
    /// non-convex list draws a triangle fan from the first point.
    pub fn draw_convex_polygon(&mut self, points: &[RenderVec2], color: Color) {
        let points = points.iter().map(|p| [p.x, p.y]).collect();
        self.push_shape(&ConvexShape::filled(points, color));
    }

    // -----------------------------------------------------------------------
    // Tilemap rendering
    // -----------------------------------------------------------------------

    /// Draw a tilemap layer using colored rectangles.
    ///
    /// The `color_fn` maps a `TileId` to an optional `Color`. Return `None`
    /// for tiles that should be skipped (transparent / drawn by sprites).
    pub fn draw_tilemap_colored<F>(
        &mut self,
        layer: &TileLayer,
        tile_w: f32,
        tile_h: f32,
        color_fn: F,
    ) where
        F: Fn(TileId) -> Option<Color>,
    {
        self.with_layer_parallax(layer, |draw| {
            let Some((xs, ys)) = draw.visible_tiles(layer, tile_w, tile_h) else {
                return;
            };
            for y in ys {
                for x in xs.clone() {
                    let tile_id = layer.get(x, y);
                    if let Some(color) = color_fn(tile_id) {
                        draw.draw_rect(
                            Rect::new(x as f32 * tile_w, y as f32 * tile_h, tile_w, tile_h),
                            color,
                        );
                    }
                }
            }
        });
    }

    /// Run `f` at the layer's own `scroll_factor_x`/`scroll_factor_y` in
    /// place of the context's parallax factor.
    fn with_layer_parallax(&mut self, layer: &TileLayer, f: impl FnOnce(&mut Self)) {
        let previous = self.parallax;
        self.set_parallax(layer.scroll_factor_x, layer.scroll_factor_y);
        f(self);
        self.parallax = previous;
    }

    /// Draw a tilemap layer using sprites from a tileset texture.
    ///
    /// `tileset_name` is the sprite name registered with the engine.
    /// `columns` is how many tile columns the tileset texture has; 0 derives
    /// it from the texture width. Tile IDs map to tileset positions:
    /// column = (id - 1) % columns, row = (id - 1) / columns.
    /// TileId(0) is skipped (empty).
    ///
    /// Like [`draw_tilemap_colored`](Self::draw_tilemap_colored), only tiles
    /// inside [`view_rect`](Self::view_rect) are drawn, and a hidden layer
    /// draws nothing.
    pub fn draw_tilemap_sprite(
        &mut self,
        layer: &TileLayer,
        tile_w: f32,
        tile_h: f32,
        tileset_name: &str,
        columns: u32,
    ) {
        self.with_layer_parallax(layer, |draw| {
            draw.draw_tileset_tiles(layer, tile_w, tile_h, tileset_name, columns);
        });
    }

    fn draw_tileset_tiles(
        &mut self,
        layer: &TileLayer,
        tile_w: f32,
        tile_h: f32,
        tileset_name: &str,
        columns: u32,
    ) {
        let Some((tex_id, tex_w, tex_h)) = self.game_ctx.find_sprite_texture(tileset_name) else {
            return;
        };
        let Some((xs, ys)) = self.visible_tiles(layer, tile_w, tile_h) else {
            return;
        };
        // `columns == 0` divided by zero below and panicked the frame.
        let columns = if columns > 0 {
            columns
        } else {
            (tex_w as f32 / tile_w) as u32
        };
        if columns == 0 {
            return;
        }
        let uv_tile_w = tile_w / tex_w as f32;
        let uv_tile_h = tile_h / tex_h as f32;

        for y in ys {
            for x in xs.clone() {
                let tile_id = layer.get(x, y);
                if tile_id.is_empty() {
                    continue;
                }
                let tid = tile_id.0 - 1; // TileId(1) = first tile in tileset
                let col = tid % columns;
                let row = tid / columns;

                self.push(SpriteInstance {
                    uv_x: col as f32 * uv_tile_w,
                    uv_y: row as f32 * uv_tile_h,
                    uv_w: uv_tile_w,
                    uv_h: uv_tile_h,
                    z_order: self.z,
                    ..SpriteInstance::new(
                        tex_id,
                        x as f32 * tile_w,
                        y as f32 * tile_h,
                        tile_w,
                        tile_h,
                    )
                });
            }
        }
    }

    /// The column and row ranges of `layer` that overlap the view, with one
    /// tile of margin for the camera's pixel snapping. `None` when nothing
    /// can be drawn: a hidden layer, a non-positive tile size, or a layer
    /// entirely off screen. Both draw functions used to walk every tile of
    /// the layer each frame, visible or not.
    fn visible_tiles(
        &self,
        layer: &TileLayer,
        tile_w: f32,
        tile_h: f32,
    ) -> Option<(std::ops::Range<u32>, std::ops::Range<u32>)> {
        // `is_nan` too: NaN would pass a `<= 0.0` check.
        let usable = |size: f32| size > 0.0 && !size.is_nan();
        if !layer.visible || !usable(tile_w) || !usable(tile_h) {
            return None;
        }
        let span = |start: f32, len: f32, tile: f32, count: u32| {
            let first = ((start / tile).floor() - 1.0).max(0.0);
            let last = ((start + len) / tile).ceil() + 1.0;
            let last = last.min(count as f32).max(0.0);
            (first < last).then_some(first as u32..last as u32)
        };
        let view = self.view_rect();
        Some((
            span(view.x, view.w, tile_w, layer.width)?,
            span(view.y, view.h, tile_h, layer.height)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_overrides_move_the_camera_and_hand_back_its_position() {
        let mut camera = Camera::new(320.0, 180.0);
        camera.position = RenderVec2::new(10.0, 20.0);

        let none = FrameOverrides::default();
        assert_eq!(none.apply(&mut camera), RenderVec2::new(10.0, 20.0));
        assert_eq!(camera.position, RenderVec2::new(10.0, 20.0));

        let overrides = FrameOverrides {
            camera_position: Some(RenderVec2::new(100.0, 50.0)),
            camera_offset: RenderVec2::new(1.0, 2.0),
        };
        let saved = overrides.apply(&mut camera);
        assert_eq!(camera.position, RenderVec2::new(101.0, 52.0));
        camera.position = saved;
        assert_eq!(camera.position, RenderVec2::new(10.0, 20.0));
    }
}
