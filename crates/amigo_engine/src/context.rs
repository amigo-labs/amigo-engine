use amigo_core::events::EventHub;
use amigo_core::resources::Resources;
use amigo_core::save::{SaveConfig, SaveManager};
use amigo_core::scheduler::TickScheduler;
use amigo_core::{Color, Rect, RenderVec2, TimeInfo, World};
use amigo_input::InputState;
use amigo_render::camera::Camera;
use amigo_render::font::{FontId, FontManager};
use amigo_render::particles::ParticleSystem;
use amigo_render::sprite_batcher::SpriteInstance;
use amigo_render::texture::TextureId;
use amigo_tilemap::{TileId, TileLayer};

#[cfg(feature = "audio")]
use amigo_audio::AudioManager;

/// Context passed to Game::update() with access to all engine systems.
pub struct GameContext {
    pub world: World,
    pub input: InputState,
    pub time: TimeInfo,
    pub camera: Camera,
    pub save: SaveManager,
    pub scheduler: TickScheduler,
    pub particles: ParticleSystem,
    pub fonts: FontManager,
    /// Double-buffered typed event system.
    pub events: EventHub,
    /// Typed resource storage for game-specific singletons.
    pub resources: Resources,
    #[cfg(feature = "audio")]
    pub audio: AudioManager,
    // Texture mapping for sprites (name -> TextureId + dimensions)
    sprite_textures: Vec<(String, TextureId, u32, u32)>,
}

impl GameContext {
    pub fn new(virtual_width: f32, virtual_height: f32, assets_path: &str) -> Self {
        Self {
            world: World::new(),
            input: InputState::new(),
            time: TimeInfo::new(),
            camera: Camera::new(virtual_width, virtual_height),
            save: SaveManager::new(SaveConfig {
                max_slots: 10,
                autosave_slots: 3,
                autosave_interval_secs: 300.0,
                app_name: "amigo_game".to_string(),
                compression: false,
            }),
            scheduler: TickScheduler::new(),
            particles: ParticleSystem::new(),
            fonts: FontManager::new(),
            events: EventHub::new(),
            resources: Resources::new(),
            #[cfg(feature = "audio")]
            audio: AudioManager::new(assets_path),
            sprite_textures: Vec::new(),
        }
    }

    /// Load a TTF/OTF font at the given pixel size. Returns a FontId handle.
    pub fn load_font(&mut self, data: &[u8], px: f32) -> Result<FontId, String> {
        self.fonts.load_font(data, px)
    }

    pub fn register_sprite_texture(
        &mut self,
        name: String,
        texture_id: TextureId,
        width: u32,
        height: u32,
    ) {
        self.sprite_textures.push((name, texture_id, width, height));
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
        }
    }

    /// Draw a sprite at a position.
    pub fn draw_sprite(&mut self, name: &str, pos: RenderVec2) {
        if let Some((tex_id, w, h)) = self.game_ctx.find_sprite_texture(name) {
            self.sprites.push(SpriteInstance {
                texture_id: tex_id,
                x: pos.x,
                y: pos.y,
                width: w as f32,
                height: h as f32,
                uv_x: 0.0,
                uv_y: 0.0,
                uv_w: 1.0,
                uv_h: 1.0,
                tint: Color::WHITE,
                flip_x: false,
                flip_y: false,
                z_order: 0,
                shaders: Vec::new(),
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
                texture_id: tex_id,
                x: pos.x,
                y: pos.y,
                width: w as f32,
                height: h as f32,
                uv_x: 0.0,
                uv_y: 0.0,
                uv_w: 1.0,
                uv_h: 1.0,
                tint: Color::WHITE,
                flip_x: false,
                flip_y: false,
                z_order: 0,
                shaders: Vec::new(),
            };
            f(&mut instance);
            self.sprites.push(instance);
        }
    }

    /// Draw a colored rectangle.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.sprites.push(SpriteInstance {
            texture_id: self.white_texture,
            x: rect.x,
            y: rect.y,
            width: rect.w,
            height: rect.h,
            uv_x: 0.0,
            uv_y: 0.0,
            uv_w: 1.0,
            uv_h: 1.0,
            tint: color,
            flip_x: false,
            flip_y: false,
            z_order: 0,
            shaders: Vec::new(),
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
                        texture_id: tex_id,
                        x: cx + glyph.offset_x,
                        y: y + px - glyph.height - glyph.offset_y,
                        width: glyph.width,
                        height: glyph.height,
                        uv_x: glyph.uv_x,
                        uv_y: glyph.uv_y,
                        uv_w: glyph.uv_w,
                        uv_h: glyph.uv_h,
                        tint: color,
                        flip_x: false,
                        flip_y: false,
                        z_order: 100,
                        shaders: Vec::new(),
                    });
                }
                cx += glyph.advance;
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
                        texture_id: tex_id,
                        x: cx + glyph.offset_x,
                        y: y + px - glyph.height - glyph.offset_y,
                        width: glyph.width,
                        height: glyph.height,
                        uv_x: glyph.uv_x,
                        uv_y: glyph.uv_y,
                        uv_w: glyph.uv_w,
                        uv_h: glyph.uv_h,
                        tint: color,
                        flip_x: false,
                        flip_y: false,
                        z_order: 100,
                        shaders: Vec::new(),
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
        for y in 0..layer.height {
            for x in 0..layer.width {
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
    /// `columns` is how many tile columns the tileset texture has.
    /// Tile IDs map to tileset positions: column = id % columns, row = id / columns.
    /// TileId(0) is skipped (empty).
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
        let uv_tile_w = tile_w / tex_w as f32;
        let uv_tile_h = tile_h / tex_h as f32;

        for y in 0..layer.height {
            for x in 0..layer.width {
                let tile_id = layer.get(x, y);
                if tile_id.is_empty() {
                    continue;
                }
                let tid = tile_id.0 - 1; // TileId(1) = first tile in tileset
                let col = tid % columns;
                let row = tid / columns;

                self.sprites.push(SpriteInstance {
                    texture_id: tex_id,
                    x: x as f32 * tile_w,
                    y: y as f32 * tile_h,
                    width: tile_w,
                    height: tile_h,
                    uv_x: col as f32 * uv_tile_w,
                    uv_y: row as f32 * uv_tile_h,
                    uv_w: uv_tile_w,
                    uv_h: uv_tile_h,
                    tint: Color::WHITE,
                    flip_x: false,
                    flip_y: false,
                    z_order: 0,
                    shaders: Vec::new(),
                });
            }
        }
    }
}
