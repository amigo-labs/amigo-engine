use amigo_engine::prelude::*;

struct Position(RenderVec2);
struct Velocity(RenderVec2);
struct Tint(Color);

impl Component for Position {}
impl Component for Velocity {}
impl Component for Tint {}

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
            self.world.insert(id, Position(RenderVec2 { x: cx, y: cy }));
            self.world.insert(
                id,
                Velocity(RenderVec2 {
                    x: angle.cos() * speed,
                    y: angle.sin() * speed,
                }),
            );
            self.world
                .insert(id, Tint(Self::color_for(self.next_color)));
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

        // Bounce off screen edges
        let positions = self.world.storage_mut::<Position>();
        let velocities = self.world.storage_mut::<Velocity>();
        for (_id, pos, vel) in join_mut!(positions, velocities) {
            pos.0.x += vel.0.x * dt;
            pos.0.y += vel.0.y * dt;

            if pos.0.x < 0.0 {
                pos.0.x = 0.0;
                vel.0.x = vel.0.x.abs();
            } else if pos.0.x > W - SIZE {
                pos.0.x = W - SIZE;
                vel.0.x = -vel.0.x.abs();
            }
            if pos.0.y < 0.0 {
                pos.0.y = 0.0;
                vel.0.y = vel.0.y.abs();
            } else if pos.0.y > H - SIZE {
                pos.0.y = H - SIZE;
                vel.0.y = -vel.0.y.abs();
            }
        }

        // Click spawns 50 entities
        if ctx.input.mouse_pressed(MouseButton::Left) {
            let pos = ctx.input.mouse_world_pos();
            self.spawn_entities(50, pos.x, pos.y);
        }

        // D key despawns oldest 50
        if ctx.input.pressed(KeyCode::KeyD) {
            let ids: Vec<EntityId> = self.world.storage::<Position>().ids().take(50).collect();
            for id in ids {
                self.world.despawn(id);
            }
        }

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        let positions = self.world.storage::<Position>();
        let tints = self.world.storage::<Tint>();
        for (_id, pos, tint) in join!(positions, tints) {
            ctx.draw_rect(Rect::new(pos.0.x, pos.0.y, SIZE, SIZE), tint.0);
        }

        let count = self.world.storage::<Position>().len();
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
