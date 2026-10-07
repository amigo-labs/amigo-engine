use amigo_animation::AnimPlayer;
use amigo_assets::AssetManager;
use amigo_core::events::EventHub;
use amigo_core::level_loader::LoadedLevel;
use amigo_core::resources::Resources;
use amigo_core::save::{SaveConfig, SaveManager};
use amigo_core::scheduler::TickScheduler;
use amigo_core::{Color, Rect, RenderVec2, SimRng, TimeInfo, World};
use amigo_input::{ActionBindings, ActionState, GamepadState, InputState};
use amigo_render::camera::Camera;
use amigo_render::font::{FontId, FontManager};
use amigo_render::lighting::LightingState;
use amigo_render::particles::ParticleSystem;
use amigo_render::post_process::PostEffect;
use amigo_render::sprite_batcher::SpriteInstance;
use amigo_render::texture::TextureId;
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
    // Texture mapping for sprites (name -> TextureId + dimensions)
    sprite_textures: Vec<(String, TextureId, u32, u32)>,
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
            #[cfg(feature = "audio")]
            audio: AudioManager::new(assets_path),
            #[cfg(feature = "async_tasks")]
            tasks: amigo_core::tasks::TaskPool::new(),
            sprite_textures: Vec::new(),
            seed: 0,
            replay: Default::default(),
            net: Default::default(),
        }
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

    /// Load a TTF/OTF font at the given pixel size. Returns a FontId handle.
    pub fn load_font(&mut self, data: &[u8], px: f32) -> Result<FontId, String> {
        self.fonts.load_font(data, px)
    }

    /// Register (or replace) the texture backing a sprite name. Replacing
    /// an existing entry keeps hot reload working: draws by name pick up
    /// the new texture on the next frame.
    pub fn register_sprite_texture(
        &mut self,
        name: String,
        texture_id: TextureId,
        width: u32,
        height: u32,
    ) {
        if let Some(entry) = self
            .sprite_textures
            .iter_mut()
            .find(|(n, _, _, _)| *n == name)
        {
            *entry = (name, texture_id, width, height);
        } else {
            self.sprite_textures.push((name, texture_id, width, height));
        }
    }

    pub fn find_sprite_texture(&self, name: &str) -> Option<(TextureId, u32, u32)> {
        self.sprite_textures
            .iter()
            .find(|(n, _, _, _)| n == name)
            .map(|(_, id, w, h)| (*id, *w, *h))
    }
}

/// Context passed to Game::draw() for rendering.
pub struct DrawContext<'a> {
    pub sprites: &'a mut Vec<SpriteInstance>,
    pub camera_pos: RenderVec2,
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub alpha: f32,
    game_ctx: &'a GameContext,
    white_texture: TextureId,
    view: Rect,
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
        }
    }

    /// Set the visible world rectangle (zoom and shake included) that
    /// culling uses, e.g. `Camera::view_rect()`.
    pub fn with_view(mut self, view: Rect) -> Self {
        self.view = view;
        self
    }

    /// The visible world rectangle. Draws outside it are wasted work.
    pub fn view_rect(&self) -> Rect {
        self.view
    }

    /// Draw a sprite at a position.
    pub fn draw_sprite(&mut self, name: &str, pos: RenderVec2) {
        if let Some((tex_id, w, h)) = self.game_ctx.find_sprite_texture(name) {
            self.sprites.push(SpriteInstance {
                ..SpriteInstance::new(tex_id, pos.x, pos.y, w as f32, h as f32)
            });
        }
    }

    /// Draw a sprite with extended options.
    pub fn draw_sprite_ex<F>(&mut self, name: &str, pos: RenderVec2, f: F)
    where
        F: FnOnce(&mut SpriteInstance),
    {
        if let Some((tex_id, w, h)) = self.game_ctx.find_sprite_texture(name) {
            let mut instance = SpriteInstance {
                ..SpriteInstance::new(tex_id, pos.x, pos.y, w as f32, h as f32)
            };
            f(&mut instance);
            self.sprites.push(instance);
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
        let Some((tex_id, w, h)) = self.game_ctx.find_sprite_texture(sprite) else {
            return;
        };
        let Some(animation) = self.game_ctx.assets.animation(&player.current_animation) else {
            return;
        };
        let uv = player.current_uv(animation);
        let mut instance = SpriteInstance {
            uv_x: uv.x,
            uv_y: uv.y,
            uv_w: uv.w,
            uv_h: uv.h,
            ..SpriteInstance::new(tex_id, pos.x, pos.y, w as f32 * uv.w, h as f32 * uv.h)
        };
        f(&mut instance);
        self.sprites.push(instance);
    }

    /// Draw a colored rectangle.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.sprites.push(SpriteInstance {
            tint: color,
            ..SpriteInstance::new(self.white_texture, rect.x, rect.y, rect.w, rect.h)
        });
    }

    // -----------------------------------------------------------------------
    // Text rendering (TTF via fontdue)
    // -----------------------------------------------------------------------

    /// Draw text using the default loaded font.
    ///
    /// The font must have been loaded via `GameContext::load_font()` before
    /// calling this. If no font is loaded, this is a no-op.
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, color: Color) {
        let Some(font) = self.game_ctx.fonts.default_font() else {
            return;
        };
        let Some(tex_id) = font.texture_id else {
            return;
        };
        let px = font.px;

        let mut cx = x;
        for ch in text.chars() {
            if let Some(glyph) = font.glyph_cached(ch) {
                if glyph.width > 0.0 && glyph.height > 0.0 {
                    self.sprites.push(SpriteInstance {
                        uv_x: glyph.uv_x,
                        uv_y: glyph.uv_y,
                        uv_w: glyph.uv_w,
                        uv_h: glyph.uv_h,
                        tint: color,
                        z_order: 100,
                        ..SpriteInstance::new(
                            tex_id,
                            cx + glyph.offset_x,
                            y + px - glyph.height - glyph.offset_y,
                            glyph.width,
                            glyph.height,
                        )
                    });
                }
                cx += glyph.advance;
            }
        }
    }

    /// Draw text with the default font, scaled about its top-left corner.
    ///
    /// Glyphs are rasterized at the font's own pixel size and the quads are
    /// scaled, so large factors get blocky — which is what pixel-art UI wants.
    /// `scale` of 1.0 is identical to [`DrawContext::draw_text`].
    pub fn draw_text_scaled(&mut self, text: &str, x: f32, y: f32, color: Color, scale: f32) {
        let Some(font) = self.game_ctx.fonts.default_font() else {
            return;
        };
        let Some(tex_id) = font.texture_id else {
            return;
        };
        let px = font.px;
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };

        let mut cx = x;
        for ch in text.chars() {
            if let Some(glyph) = font.glyph_cached(ch) {
                if glyph.width > 0.0 && glyph.height > 0.0 {
                    self.sprites.push(SpriteInstance {
                        uv_x: glyph.uv_x,
                        uv_y: glyph.uv_y,
                        uv_w: glyph.uv_w,
                        uv_h: glyph.uv_h,
                        tint: color,
                        z_order: 100,
                        ..SpriteInstance::new(
                            tex_id,
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
    }

    /// Draw text using a specific font by FontId.
    pub fn draw_text_font(&mut self, font_id: FontId, text: &str, x: f32, y: f32, color: Color) {
        let Some(font) = self.game_ctx.fonts.get(font_id) else {
            return;
        };
        let Some(tex_id) = font.texture_id else {
            return;
        };
        let px = font.px;

        let mut cx = x;
        for ch in text.chars() {
            if let Some(glyph) = font.glyph_cached(ch) {
                if glyph.width > 0.0 && glyph.height > 0.0 {
                    self.sprites.push(SpriteInstance {
                        uv_x: glyph.uv_x,
                        uv_y: glyph.uv_y,
                        uv_w: glyph.uv_w,
                        uv_h: glyph.uv_h,
                        tint: color,
                        z_order: 100,
                        ..SpriteInstance::new(
                            tex_id,
                            cx + glyph.offset_x,
                            y + px - glyph.height - glyph.offset_y,
                            glyph.width,
                            glyph.height,
                        )
                    });
                }
                cx += glyph.advance;
            }
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
        let Some((xs, ys)) = self.visible_tiles(layer, tile_w, tile_h) else {
            return;
        };
        for y in ys {
            for x in xs.clone() {
                let tile_id = layer.get(x, y);
                if let Some(color) = color_fn(tile_id) {
                    self.draw_rect(
                        Rect::new(x as f32 * tile_w, y as f32 * tile_h, tile_w, tile_h),
                        color,
                    );
                }
            }
        }
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

                self.sprites.push(SpriteInstance {
                    uv_x: col as f32 * uv_tile_w,
                    uv_y: row as f32 * uv_tile_h,
                    uv_w: uv_tile_w,
                    uv_h: uv_tile_h,
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
        let view = self.view;
        Some((
            span(view.x, view.w, tile_w, layer.width)?,
            span(view.y, view.h, tile_h, layer.height)?,
        ))
    }
}
