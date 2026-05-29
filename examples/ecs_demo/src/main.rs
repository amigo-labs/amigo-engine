use amigo_engine::prelude::*;

// Game-specific component types. With the current ECS, the `Component` trait is
// reserved for the engine's built-in components; game types are stored as
// *dynamic* components via `world.insert_dynamic` / `world.dynamic`.
//
// Position and velocity are combined into a single `Particle` component so the
// integration step needs only one mutable borrow of the world.
struct Particle {
    pos: RenderVec2,
    vel: RenderVec2,
}
struct Tint(Color);

const W: f32 = 480.0;
const H: f32 = 270.0;
const SIZE: f32 = 4.0;

struct EcsDemo {
    world: World,
    next_color: u32,
}

impl EcsDemo {
    fn new() -> Self {
        Self {
            world: World::new(),
            next_color: 0,
        }
    }

    fn color_for(idx: u32) -> Color {
        let r = ((idx * 73 + 29) % 256) as f32 / 255.0;
        let g = ((idx * 137 + 43) % 256) as f32 / 255.0;
        let b = ((idx * 53 + 97) % 256) as f32 / 255.0;
        Color::new(r, g, b, 1.0)
    }

    fn spawn_entities(&mut self, count: u32, cx: f32, cy: f32) {
        for _ in 0..count {
            let id = self.world.spawn();
            let angle = (self.next_color as f32) * 0.618 * std::f32::consts::TAU;
            let speed = 30.0 + (self.next_color % 60) as f32;
            self.world.insert_dynamic(
                id,
                Particle {
                    pos: RenderVec2 { x: cx, y: cy },
                    vel: RenderVec2 {
                        x: angle.cos() * speed,
                        y: angle.sin() * speed,
                    },
                },
            );
            self.world
                .insert_dynamic(id, Tint(Self::color_for(self.next_color)));
            self.next_color += 1;
        }
    }
}

impl Game for EcsDemo {
    fn init(&mut self, _ctx: &mut GameContext) {
        self.spawn_entities(500, W / 2.0, H / 2.0);
    }

    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        let dt = ctx.time.dt;

        // Move and bounce off screen edges.
        if let Some(particles) = self.world.dynamic_mut::<Particle>() {
            for (_id, p) in particles.iter_mut() {
                p.pos.x += p.vel.x * dt;
                p.pos.y += p.vel.y * dt;

                if p.pos.x < 0.0 {
                    p.pos.x = 0.0;
                    p.vel.x = p.vel.x.abs();
                } else if p.pos.x > W - SIZE {
                    p.pos.x = W - SIZE;
                    p.vel.x = -p.vel.x.abs();
                }
                if p.pos.y < 0.0 {
                    p.pos.y = 0.0;
                    p.vel.y = p.vel.y.abs();
                } else if p.pos.y > H - SIZE {
                    p.pos.y = H - SIZE;
                    p.vel.y = -p.vel.y.abs();
                }
            }
        }

        // Click spawns 50 entities at the cursor.
        if ctx.input.mouse_pressed(MouseButton::Left) {
            let pos = ctx.input.mouse_world_pos();
            self.spawn_entities(50, pos.x, pos.y);
        }

        // D key despawns the oldest 50 entities.
        if ctx.input.pressed(KeyCode::KeyD) {
            let ids: Vec<EntityId> = self
                .world
                .dynamic::<Particle>()
                .map(|s| s.entities().iter().take(50).copied().collect())
                .unwrap_or_default();
            for id in ids {
                self.world.despawn(id);
            }
        }

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        if let (Some(particles), Some(tints)) = (
            self.world.dynamic::<Particle>(),
            self.world.dynamic::<Tint>(),
        ) {
            for (_id, p, tint) in join(particles, tints) {
                ctx.draw_rect(Rect::new(p.pos.x, p.pos.y, SIZE, SIZE), tint.0);
            }
        }

        let count = self.world.dynamic::<Particle>().map_or(0, |s| s.len());
        let hud = format!("Entities: {}  |  Click=spawn 50  D=despawn 50", count);
        ctx.draw_text(&hud, 4.0, 4.0, Color::WHITE);
    }
}

fn main() {
    Engine::build()
        .title("ECS Demo")
        .virtual_resolution(480, 270)
        .build()
        .run(EcsDemo::new());
}
