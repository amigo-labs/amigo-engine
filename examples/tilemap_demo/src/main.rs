use amigo_engine::prelude::*;

const MAP_W: u32 = 32;
const MAP_H: u32 = 32;
const TILE_SIZE: f32 = 16.0;
const SCROLL_SPEED: f32 = 2.0;

/// Tile IDs used in the demo map.
const GRASS: TileId = TileId(1);
const PATH: TileId = TileId(2);
const WATER: TileId = TileId(3);
const WALL: TileId = TileId(4);

struct TilemapDemo {
    tilemap: TileMap,
    camera_x: f32,
    camera_y: f32,
    show_collision: bool,
}

impl TilemapDemo {
    fn new() -> Self {
        Self {
            tilemap: TileMap::new(GridMode::default(), MAP_W, MAP_H),
            camera_x: 0.0,
            camera_y: 0.0,
            show_collision: false,
        }
    }

    fn build_map(&mut self) {
        let terrain = self.tilemap.layer_mut("terrain").unwrap();

        // Fill everything with grass.
        terrain.fill_rect(0, 0, MAP_W, MAP_H, GRASS);

        // Walls around the border.
        for x in 0..MAP_W {
            terrain.set(x, 0, WALL);
            terrain.set(x, MAP_H - 1, WALL);
        }
        for y in 0..MAP_H {
            terrain.set(0, y, WALL);
            terrain.set(MAP_W - 1, y, WALL);
        }

        // Horizontal path across the middle.
        for x in 1..MAP_W - 1 {
            terrain.set(x, MAP_H / 2, PATH);
            terrain.set(x, MAP_H / 2 + 1, PATH);
        }

        // Vertical path crossing it.
        for y in 1..MAP_H - 1 {
            terrain.set(MAP_W / 2, y, PATH);
            terrain.set(MAP_W / 2 + 1, y, PATH);
        }

        // Water pond in the top-right quadrant.
        let terrain = self.tilemap.layer_mut("terrain").unwrap();
        terrain.fill_rect(20, 4, 6, 4, WATER);

        // Mark walls and water as solid in the collision layer.
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let tile = self.tilemap.layer("terrain").unwrap().get(x, y);
                if tile == WALL || tile == WATER {
                    self.tilemap
                        .collision
                        .set(x, y, CollisionType::Solid);
                }
            }
        }
    }

    fn tile_color(id: TileId) -> Option<Color> {
        match id {
            t if t == GRASS => Some(Color::rgb(0.25, 0.55, 0.20)),
            t if t == PATH  => Some(Color::rgb(0.60, 0.55, 0.40)),
            t if t == WATER => Some(Color::rgb(0.20, 0.35, 0.65)),
            t if t == WALL  => Some(Color::rgb(0.45, 0.40, 0.35)),
            _ => None,
        }
    }
}

impl Game for TilemapDemo {
    fn init(&mut self, _ctx: &mut GameContext) {
        self.build_map();
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        // WASD / Arrow key scrolling.
        if ctx.input.held(KeyCode::KeyW) || ctx.input.held(KeyCode::ArrowUp) {
            self.camera_y -= SCROLL_SPEED;
        }
        if ctx.input.held(KeyCode::KeyS) || ctx.input.held(KeyCode::ArrowDown) {
            self.camera_y += SCROLL_SPEED;
        }
        if ctx.input.held(KeyCode::KeyA) || ctx.input.held(KeyCode::ArrowLeft) {
            self.camera_x -= SCROLL_SPEED;
        }
        if ctx.input.held(KeyCode::KeyD) || ctx.input.held(KeyCode::ArrowRight) {
            self.camera_x += SCROLL_SPEED;
        }

        // Toggle collision overlay with F3.
        if ctx.input.pressed(KeyCode::F3) {
            self.show_collision = !self.show_collision;
        }

        ctx.camera.set_target(RenderVec2 {
            x: self.camera_x,
            y: self.camera_y,
        });

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        let vw = ctx.virtual_width;
        let vh = ctx.virtual_height;

        // Background clear.
        ctx.draw_rect(Rect::new(0.0, 0.0, vw, vh), Color::rgb(0.08, 0.08, 0.12));

        // Draw the terrain layer.
        let terrain = self.tilemap.layer("terrain").unwrap();
        ctx.draw_tilemap_colored(terrain, TILE_SIZE, TILE_SIZE, Self::tile_color);

        // Collision overlay.
        if self.show_collision {
            let col = &self.tilemap.collision;
            let overlay = Color::rgb(1.0, 0.2, 0.2).with_alpha(0.35);
            for y in 0..col.height {
                for x in 0..col.width {
                    if col.is_solid(x as i32, y as i32) {
                        ctx.draw_rect(
                            Rect::new(
                                x as f32 * TILE_SIZE,
                                y as f32 * TILE_SIZE,
                                TILE_SIZE,
                                TILE_SIZE,
                            ),
                            overlay,
                        );
                    }
                }
            }
        }

        // HUD (screen-space).
        ctx.draw_text("WASD/Arrows: Scroll  F3: Collision", 4.0, 4.0, Color::WHITE);
        let status = if self.show_collision {
            "Collision: ON"
        } else {
            "Collision: OFF"
        };
        ctx.draw_text(status, 4.0, 16.0, Color::rgb(0.8, 0.8, 0.6));
    }
}

fn main() {
    Engine::build()
        .title("Tilemap Demo")
        .virtual_resolution(480, 270)
        .window_size(960, 540)
        .build()
        .run(TilemapDemo::new());
}
