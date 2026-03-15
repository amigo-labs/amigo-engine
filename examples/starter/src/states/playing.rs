use amigo_engine::prelude::*;
use amigo_core::ecs::world::{Position, SpriteComp, StateScoped};
use crate::game::AppState;
use crate::player::Player;

/// The main gameplay state: tilemap, player, decorations, HUD.
pub struct PlayingState {
    player: Player,
    /// Simple tilemap stored as a flat grid of tile IDs.
    tiles: Vec<u8>,
    map_w: u32,
    map_h: u32,
}

impl PlayingState {
    pub fn new() -> Self {
        Self {
            player: Player::new(),
            tiles: Vec::new(),
            map_w: 20,
            map_h: 12,
        }
    }

    /// Set up the level: load tilemap, spawn entities.
    pub fn enter(&mut self, ctx: &mut GameContext) {
        self.load_tilemap();
        self.player.spawn(ctx, 64.0, 64.0);
        self.spawn_decorations(ctx);
    }

    fn load_tilemap(&mut self) {
        // 20x12 tile grid. 0=grass, 1=stone(wall), 2=water
        // Border of stone, some water in the middle, rest grass.
        let w = self.map_w as usize;
        let h = self.map_h as usize;
        self.tiles = vec![0u8; w * h];

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                // Walls around the border
                if x == 0 || x == w - 1 || y == 0 || y == h - 1 {
                    self.tiles[idx] = 1;
                }
                // Small water pond
                if (8..12).contains(&x) && (5..7).contains(&y) {
                    self.tiles[idx] = 2;
                }
            }
        }
    }

    fn spawn_decorations(&self, ctx: &mut GameContext) {
        // Spawn a few decoration entities tagged with Playing state
        let positions = [(160.0, 80.0), (240.0, 120.0), (96.0, 144.0)];
        for (x, y) in positions {
            let id = ctx.world.spawn();
            ctx.world.positions.insert(id, Position(SimVec2::from_f32(x, y)));
            ctx.world.sprites.insert(id, SpriteComp::new("decoration"));
            ctx.world.state_scoped.insert(id, StateScoped(AppState::Playing as u32));
        }
    }

    /// Returns true when player presses Escape (back to menu).
    pub fn update(&mut self, ctx: &mut GameContext) -> bool {
        self.player.update(ctx, &self.tiles, self.map_w);

        // Camera follows the player
        let target = RenderVec2 { x: self.player.pos_x, y: self.player.pos_y };
        ctx.camera.set_target(target);

        ctx.input.pressed(KeyCode::Escape)
    }

    pub fn draw(&self, ctx: &mut DrawContext) {
        let vw = ctx.virtual_width;
        let vh = ctx.virtual_height;

        // Clear
        ctx.draw_rect(Rect::new(0.0, 0.0, vw, vh), Color::rgb(0.15, 0.18, 0.12));

        // Draw tilemap
        let tile_size = 16.0;
        for y in 0..self.map_h {
            for x in 0..self.map_w {
                let idx = (y * self.map_w + x) as usize;
                let tile = self.tiles[idx];
                let color = match tile {
                    0 => Color::rgb(0.25, 0.55, 0.20), // grass
                    1 => Color::rgb(0.45, 0.40, 0.35), // stone
                    2 => Color::rgb(0.20, 0.35, 0.60), // water
                    _ => Color::MAGENTA,
                };
                ctx.draw_rect(
                    Rect::new(x as f32 * tile_size, y as f32 * tile_size, tile_size, tile_size),
                    color,
                );
            }
        }

        // Draw player
        self.player.draw(ctx);

        // HUD: position text
        let px = self.player.pos_x;
        let py = self.player.pos_y;
        let pos_text = format!("X:{:.0} Y:{:.0}", px, py);
        ctx.draw_text(&pos_text, 4.0, 4.0, Color::rgb(0.9, 0.9, 0.8).with_alpha(0.7));

        // Hint bar at bottom
        ctx.draw_rect(Rect::new(0.0, vh - 12.0, vw, 12.0), Color::BLACK.with_alpha(0.5));
        ctx.draw_text("WASD:Move  ESC:Menu", 4.0, vh - 10.0, Color::rgb(0.7, 0.7, 0.6));
    }
}
