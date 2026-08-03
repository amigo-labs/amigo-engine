use crate::player::Player;
use amigo_core::ecs::world::{Position, StateScoped};
use amigo_engine::prelude::*;

/// Tag for entities this state owns, so `on_exit` can despawn exactly those and
/// leave anything the menu or a future overlay spawned alone.
const STATE_SCOPE: u32 = 1;

/// Game-specific marker component. Engine-core components are typed fields on
/// `World`; anything a game invents goes into dynamic storage instead.
struct Decoration;

/// The main gameplay state: tilemap, player, decorations, HUD.
pub struct PlayingState {
    player: Player,
    /// Simple tilemap stored as a flat grid of tile IDs.
    tiles: Vec<u8>,
    map_w: u32,
    map_h: u32,
    /// Decoration positions read out of the ECS during `update`.
    ///
    /// `draw` only gets a [`DrawContext`], which does not expose the world, so
    /// anything drawn from ECS state has to be collected while the game still
    /// has `&mut GameContext`.
    decorations: Vec<RenderVec2>,
}

impl PlayingState {
    pub fn new() -> Self {
        Self {
            player: Player::new(),
            tiles: Vec::new(),
            map_w: 20,
            map_h: 12,
            decorations: Vec::new(),
        }
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
        // Decorations live in the ECS and are tagged with this state's scope, so
        // `on_exit` cleans them up without tracking the ids by hand.
        let positions = [(160.0, 80.0), (240.0, 120.0), (96.0, 144.0)];
        for (x, y) in positions {
            let id = ctx.world.spawn();
            ctx.world
                .positions
                .insert(id, Position(SimVec2::from_f32(x, y)));
            ctx.world.state_scoped.insert(id, StateScoped(STATE_SCOPE));
            ctx.world.insert_dynamic(id, Decoration);
        }
    }

    /// Collect decoration positions for `draw`.
    ///
    /// Filtering on the `Decoration` marker rather than the state scope matters:
    /// the player carries the same scope, and would otherwise be drawn twice.
    fn collect_decorations(&mut self, ctx: &GameContext) {
        self.decorations.clear();
        let Some(markers) = ctx.world.dynamic::<Decoration>() else {
            return;
        };
        for (id, _) in markers.iter() {
            if let Some(pos) = ctx.world.positions.get(id) {
                self.decorations.push(RenderVec2 {
                    x: pos.0.x.to_num::<f32>(),
                    y: pos.0.y.to_num::<f32>(),
                });
            }
        }
    }
}

impl Default for PlayingState {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for PlayingState {
    /// Set up the level: load tilemap, spawn entities.
    fn on_enter(&mut self, ctx: &mut GameContext) {
        self.load_tilemap();
        self.player.spawn(ctx, 64.0, 64.0, STATE_SCOPE);
        self.spawn_decorations(ctx);
        ctx.world.flush();
        self.collect_decorations(ctx);

        // Dusk: a dim blue ambient with a warm light on the player. Neutral
        // lighting skips the composite pass entirely, so this is also what makes
        // the stage run at all.
        ctx.lighting.set_ambient(Color::rgb(0.55, 0.6, 0.85), 0.55);
        ctx.lighting.add_light(PointLight {
            position: (64.0, 64.0),
            color: Color::rgb(1.0, 0.85, 0.6),
            intensity: 1.4,
            radius: 90.0,
            falloff: 1.6,
        });
    }

    /// Drop this state's lights, so the menu is not left in gameplay's dusk.
    fn on_pause(&mut self, ctx: &mut GameContext) {
        ctx.lighting.clear_lights();
        ctx.lighting.set_ambient(Color::WHITE, 1.0);
    }

    /// Despawn everything this state spawned, so returning to the menu does not
    /// leak entities into the next round of gameplay.
    fn on_exit(&mut self, ctx: &mut GameContext) {
        ctx.world.cleanup_state(STATE_SCOPE);
        ctx.world.flush();
        ctx.lighting.clear_lights();
        ctx.lighting.set_ambient(Color::WHITE, 1.0);
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        self.player.update(ctx, &self.tiles, self.map_w);

        // Camera follows the player
        let target = RenderVec2 {
            x: self.player.pos_x,
            y: self.player.pos_y,
        };
        ctx.camera.set_target(target);

        // Keep the torch on the player.
        if let Some(light) = ctx.lighting.lights.first_mut() {
            light.position = (self.player.pos_x + 8.0, self.player.pos_y + 8.0);
        }

        // HUD through the Pixel UI. These widgets are drawn in a screen-space
        // pass after post-processing, so they neither scroll with the camera nor
        // get bent by post effects — unlike anything drawn from `draw`.
        let vw = ctx.camera.virtual_width;
        let vh = ctx.camera.virtual_height;
        ctx.ui.panel(
            Rect::new(0.0, vh - 14.0, vw, 14.0),
            Color::BLACK.with_alpha(0.6),
        );
        ctx.ui.pixel_text(
            "WASD:Move  ESC:Menu",
            4.0,
            vh - 11.0,
            Color::rgb(0.7, 0.7, 0.6),
        );
        // Distance from spawn, as a stand-in for a real resource bar.
        let travelled =
            ((self.player.pos_x - 64.0).abs() + (self.player.pos_y - 64.0).abs()) / 400.0;
        ctx.ui.progress_bar(
            Rect::new(vw - 64.0, 4.0, 60.0, 6.0),
            travelled,
            Color::rgb(0.4, 0.8, 1.0),
        );

        if ctx.input.pressed(KeyCode::Escape) {
            // Pop back to the menu that pushed us.
            return SceneAction::Pop;
        }
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
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
                    Rect::new(
                        x as f32 * tile_size,
                        y as f32 * tile_size,
                        tile_size,
                        tile_size,
                    ),
                    color,
                );
            }
        }

        // Decorations, from the positions collected out of the ECS in `update`
        for pos in &self.decorations {
            ctx.draw_rect(
                Rect::new(pos.x, pos.y, 8.0, 8.0),
                Color::rgb(0.75, 0.65, 0.30),
            );
        }

        // Draw player
        self.player.draw(ctx);

        // HUD: position text
        let px = self.player.pos_x;
        let py = self.player.pos_y;
        let pos_text = format!("X:{:.0} Y:{:.0}", px, py);
        ctx.draw_text(
            &pos_text,
            4.0,
            4.0,
            Color::rgb(0.9, 0.9, 0.8).with_alpha(0.7),
        );

        // The hint bar and the progress bar are UI now (see `update`), so they
        // live in the screen-space pass instead of being drawn into the world.
    }
}
