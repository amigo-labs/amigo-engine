use amigo_core::{Color, Rect, RenderVec2, TimeInfo, World};
use amigo_input::InputState;
use amigo_render::sprite_batcher::SpriteInstance;
use amigo_render::texture::TextureId;

/// Context passed to Game::update() with access to all engine systems.
pub struct GameContext {
    pub world: World,
    pub input: InputState,
    pub time: TimeInfo,
    // Texture mapping for sprites (name -> TextureId + dimensions)
    sprite_textures: Vec<(String, TextureId, u32, u32)>,
}

impl GameContext {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            input: InputState::new(),
            time: TimeInfo::new(),
            sprite_textures: Vec::new(),
        }
    }

    pub fn register_sprite_texture(&mut self, name: String, texture_id: TextureId, width: u32, height: u32) {
        self.sprite_textures.push((name, texture_id, width, height));
    }

    pub fn find_sprite_texture(&self, name: &str) -> Option<(TextureId, u32, u32)> {
        self.sprite_textures
            .iter()
            .find(|(n, _, _, _)| n == name)
            .map(|(_, id, w, h)| (*id, *w, *h))
    }
}

impl Default for GameContext {
    fn default() -> Self {
        Self::new()
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
        });
    }
}
