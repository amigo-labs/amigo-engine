//! A complete small game in one file: move a player, collect coins, win.
//!
//! This sits between the generated `amigo new` template (an empty `Game`
//! impl) and the `starter` example (a multi-state game with menus). It shows
//! the patterns most games need first:
//!
//! * custom components stored in the ECS [`World`] (`insert_dynamic`)
//! * a movement "system" reading input and writing world state
//! * spawning and despawning entities (coin pickup)
//! * simple AABB overlap checks via [`Rect`]
//! * drawing rects and HUD text

use amigo_engine::prelude::*;

const VIRTUAL_W: f32 = 320.0;
const VIRTUAL_H: f32 = 180.0;
const PLAYER_SIZE: f32 = 10.0;
const COIN_SIZE: f32 = 6.0;
const PLAYER_SPEED: f32 = 90.0; // pixels per second
const COIN_COUNT: u32 = 12;

/// Marker + position component for coins, stored as a dynamic ECS component.
struct Coin {
    pos: RenderVec2,
}

struct BasicGame {
    world: World,
    player_pos: RenderVec2,
    score: u32,
}

impl BasicGame {
    fn new() -> Self {
        Self {
            world: World::new(),
            player_pos: RenderVec2 {
                x: VIRTUAL_W / 2.0,
                y: VIRTUAL_H / 2.0,
            },
            score: 0,
        }
    }

    fn spawn_coins(&mut self) {
        // Deterministic layout: a loose ring around the center.
        for i in 0..COIN_COUNT {
            let angle = i as f32 / COIN_COUNT as f32 * std::f32::consts::TAU;
            let radius = 50.0 + 18.0 * ((i % 3) as f32);
            let id = self.world.spawn();
            self.world.insert_dynamic(
                id,
                Coin {
                    pos: RenderVec2 {
                        x: VIRTUAL_W / 2.0 + angle.cos() * radius,
                        y: VIRTUAL_H / 2.0 + angle.sin() * radius,
                    },
                },
            );
        }
    }

    fn reset(&mut self) {
        let ids: Vec<EntityId> = self
            .world
            .dynamic::<Coin>()
            .map(|s| s.entities().to_vec())
            .unwrap_or_default();
        for id in ids {
            self.world.despawn(id);
        }
        self.player_pos = RenderVec2 {
            x: VIRTUAL_W / 2.0,
            y: VIRTUAL_H / 2.0,
        };
        self.score = 0;
        self.spawn_coins();
    }

    fn player_rect(&self) -> Rect {
        Rect::new(
            self.player_pos.x - PLAYER_SIZE / 2.0,
            self.player_pos.y - PLAYER_SIZE / 2.0,
            PLAYER_SIZE,
            PLAYER_SIZE,
        )
    }

    /// Movement system: WASD / arrow keys, clamped to the screen.
    fn move_player(&mut self, ctx: &GameContext) {
        let dt = ctx.time.dt;
        let mut dir = RenderVec2 { x: 0.0, y: 0.0 };
        if ctx.input.held(KeyCode::KeyW) || ctx.input.held(KeyCode::ArrowUp) {
            dir.y -= 1.0;
        }
        if ctx.input.held(KeyCode::KeyS) || ctx.input.held(KeyCode::ArrowDown) {
            dir.y += 1.0;
        }
        if ctx.input.held(KeyCode::KeyA) || ctx.input.held(KeyCode::ArrowLeft) {
            dir.x -= 1.0;
        }
        if ctx.input.held(KeyCode::KeyD) || ctx.input.held(KeyCode::ArrowRight) {
            dir.x += 1.0;
        }
        // Normalize so diagonal movement isn't faster.
        let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
        if len > 0.0 {
            self.player_pos.x += dir.x / len * PLAYER_SPEED * dt;
            self.player_pos.y += dir.y / len * PLAYER_SPEED * dt;
        }
        let half = PLAYER_SIZE / 2.0;
        self.player_pos.x = self.player_pos.x.clamp(half, VIRTUAL_W - half);
        self.player_pos.y = self.player_pos.y.clamp(half, VIRTUAL_H - half);
    }

    /// Pickup system: despawn coins overlapping the player, count score.
    fn collect_coins(&mut self) {
        let player = self.player_rect();
        let picked: Vec<EntityId> = self
            .world
            .dynamic::<Coin>()
            .map(|coins| {
                coins
                    .iter()
                    .filter(|(_, c)| {
                        let r = Rect::new(
                            c.pos.x - COIN_SIZE / 2.0,
                            c.pos.y - COIN_SIZE / 2.0,
                            COIN_SIZE,
                            COIN_SIZE,
                        );
                        player.overlaps(&r)
                    })
                    .map(|(id, _)| id)
                    .collect()
            })
            .unwrap_or_default();
        for id in picked {
            self.world.despawn(id);
            self.score += 1;
        }
    }

    fn coins_left(&self) -> usize {
        self.world.dynamic::<Coin>().map_or(0, |s| s.len())
    }
}

impl Game for BasicGame {
    fn init(&mut self, _ctx: &mut GameContext) {
        self.spawn_coins();
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        let won = self.coins_left() == 0;
        if won {
            if ctx.input.pressed(KeyCode::KeyR) {
                self.reset();
            }
            return SceneAction::Continue;
        }

        self.move_player(ctx);
        self.collect_coins();
        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        // Coins
        if let Some(coins) = self.world.dynamic::<Coin>() {
            for (_, c) in coins.iter() {
                ctx.draw_rect(
                    Rect::new(
                        c.pos.x - COIN_SIZE / 2.0,
                        c.pos.y - COIN_SIZE / 2.0,
                        COIN_SIZE,
                        COIN_SIZE,
                    ),
                    Color::new(1.0, 0.85, 0.2, 1.0),
                );
            }
        }

        // Player
        ctx.draw_rect(self.player_rect(), Color::new(0.3, 0.7, 1.0, 1.0));

        // HUD
        ctx.draw_text(
            &format!("Coins: {}/{}", self.score, COIN_COUNT),
            4.0,
            4.0,
            Color::WHITE,
        );
        if self.coins_left() == 0 {
            ctx.draw_text(
                "You win! Press R to play again",
                VIRTUAL_W / 2.0 - 90.0,
                VIRTUAL_H / 2.0,
                Color::new(0.4, 1.0, 0.4, 1.0),
            );
        } else {
            ctx.draw_text(
                "WASD / arrows to move",
                4.0,
                VIRTUAL_H - 14.0,
                Color::new(0.6, 0.6, 0.6, 1.0),
            );
        }
    }
}

fn main() {
    Engine::build()
        .title("Basic Game — Coin Collector")
        .virtual_resolution(VIRTUAL_W as u32, VIRTUAL_H as u32)
        .build()
        .run(BasicGame::new());
}
