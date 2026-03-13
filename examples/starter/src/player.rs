use amigo_engine::prelude::*;
use amigo_core::ecs::world::{Position, Velocity, SpriteComp, StateScoped};
use crate::game::AppState;

pub struct Player {
    entity: Option<EntityId>,
    speed: f32,
    anim_timer: f32,
    anim_frame: u32,
    facing_left: bool,
    /// Cached position from last update (for rendering without world access).
    pub pos_x: f32,
    pub pos_y: f32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            entity: None,
            speed: 80.0,
            anim_timer: 0.0,
            anim_frame: 0,
            facing_left: false,
            pos_x: 0.0,
            pos_y: 0.0,
        }
    }

    pub fn spawn(&mut self, ctx: &mut GameContext, x: f32, y: f32) {
        let id = ctx.world.spawn();
        ctx.world.positions.insert(id, Position(SimVec2::from_f32(x, y)));
        ctx.world.velocities.insert(id, Velocity(SimVec2::ZERO));
        ctx.world.sprites.insert(id, SpriteComp::new("player"));
        ctx.world.state_scoped.insert(id, StateScoped(AppState::Playing as u32));
        self.entity = Some(id);
        self.pos_x = x;
        self.pos_y = y;
    }

    pub fn update(&mut self, ctx: &mut GameContext, tiles: &[u8], map_w: u32) {
        let id = match self.entity {
            Some(id) if ctx.world.is_alive(id) => id,
            _ => return,
        };

        // Input -> velocity
        let mut vx: f32 = 0.0;
        let mut vy: f32 = 0.0;
        if ctx.input.held(KeyCode::KeyW) || ctx.input.held(KeyCode::ArrowUp) { vy -= 1.0; }
        if ctx.input.held(KeyCode::KeyS) || ctx.input.held(KeyCode::ArrowDown) { vy += 1.0; }
        if ctx.input.held(KeyCode::KeyA) || ctx.input.held(KeyCode::ArrowLeft) { vx -= 1.0; }
        if ctx.input.held(KeyCode::KeyD) || ctx.input.held(KeyCode::ArrowRight) { vx += 1.0; }

        // Normalize diagonal movement
        let len = (vx * vx + vy * vy).sqrt();
        if len > 0.0 {
            vx = vx / len * self.speed;
            vy = vy / len * self.speed;
            if vx < 0.0 { self.facing_left = true; }
            if vx > 0.0 { self.facing_left = false; }
        }

        // Apply velocity with tile collision (separate X and Y for sliding)
        let dt = 1.0 / 60.0;
        let new_x = self.pos_x + vx * dt;
        let new_y = self.pos_y + vy * dt;

        if !self.tile_blocked(new_x, self.pos_y, tiles, map_w) { self.pos_x = new_x; }
        if !self.tile_blocked(self.pos_x, new_y, tiles, map_w) { self.pos_y = new_y; }

        // Write back to ECS
        ctx.world.positions.insert(id, Position(SimVec2::from_f32(self.pos_x, self.pos_y)));

        // Animation
        if len > 0.0 {
            self.anim_timer += dt;
            if self.anim_timer >= 0.15 {
                self.anim_timer = 0.0;
                self.anim_frame = (self.anim_frame + 1) % 4;
            }
        } else {
            self.anim_frame = 0;
            self.anim_timer = 0.0;
        }

        // Update sprite flip
        if let Some(sprite) = ctx.world.sprites.get_mut(id) {
            sprite.flip_x = self.facing_left;
        }
    }

    /// Check if a position collides with a solid tile (id == 1) or water (id == 2).
    fn tile_blocked(&self, x: f32, y: f32, tiles: &[u8], map_w: u32) -> bool {
        let tile_size = 16.0;
        // Check four corners of a 12x12 hitbox (centered in 16x16 cell)
        let offsets = [(2.0, 2.0), (13.0, 2.0), (2.0, 13.0), (13.0, 13.0)];
        for (ox, oy) in offsets {
            let tx = ((x + ox) / tile_size) as i32;
            let ty = ((y + oy) / tile_size) as i32;
            if tx < 0 || ty < 0 || tx >= map_w as i32 {
                return true;
            }
            let idx = ty as usize * map_w as usize + tx as usize;
            if idx >= tiles.len() {
                return true;
            }
            if tiles[idx] == 1 || tiles[idx] == 2 {
                return true;
            }
        }
        false
    }

    pub fn draw(&self, ctx: &mut DrawContext) {
        let x = self.pos_x;
        let y = self.pos_y;

        // Body
        ctx.draw_rect(
            Rect::new(x + 1.0, y + 2.0, 14.0, 14.0),
            Color::rgb(0.3, 0.6, 1.0),
        );

        // Head (lighter)
        ctx.draw_rect(
            Rect::new(x + 3.0, y, 10.0, 6.0),
            Color::rgb(0.5, 0.8, 1.0),
        );

        // Walk animation: alternate feet position
        let foot_offset = match self.anim_frame {
            1 => 1.0,
            3 => -1.0,
            _ => 0.0,
        };
        let foot_color = Color::rgb(0.2, 0.3, 0.6);
        ctx.draw_rect(Rect::new(x + 3.0 + foot_offset, y + 14.0, 4.0, 2.0), foot_color);
        ctx.draw_rect(Rect::new(x + 9.0 - foot_offset, y + 14.0, 4.0, 2.0), foot_color);
    }
}
