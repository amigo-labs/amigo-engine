use amigo_engine::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preset {
    Fire,
    Snow,
    Sparks,
    Smoke,
}

impl Preset {
    const ALL: [Preset; 4] = [Preset::Fire, Preset::Snow, Preset::Sparks, Preset::Smoke];

    fn name(self) -> &'static str {
        match self {
            Preset::Fire => "Fire",
            Preset::Snow => "Snow",
            Preset::Sparks => "Sparks",
            Preset::Smoke => "Smoke",
        }
    }

    fn next(self) -> Preset {
        let idx = Preset::ALL.iter().position(|&p| p == self).unwrap();
        Preset::ALL[(idx + 1) % Preset::ALL.len()]
    }

    fn emitter_config(self, position: (f32, f32)) -> EmitterConfig {
        let mut config = EmitterConfig {
            position,
            ..Default::default()
        };

        match self {
            Preset::Fire => {
                config.lifetime = 0.8;
                config.speed = 60.0;
                config.count = 40;
                config.color_start = Color::new(1.0, 0.6, 0.0, 1.0);
                config.color_end = Color::new(1.0, 0.0, 0.0, 0.0);
                config.shape = EmitterShape::Circle { radius: 6.0 };
            }
            Preset::Snow => {
                config.lifetime = 3.0;
                config.speed = 20.0;
                config.count = 20;
                config.color_start = Color::WHITE;
                config.color_end = Color::new(0.8, 0.9, 1.0, 0.0);
                config.shape = EmitterShape::Circle { radius: 30.0 };
            }
            Preset::Sparks => {
                config.lifetime = 0.4;
                config.speed = 150.0;
                config.count = 60;
                config.color_start = Color::new(1.0, 1.0, 0.5, 1.0);
                config.color_end = Color::new(1.0, 0.8, 0.0, 0.0);
                config.shape = EmitterShape::Point;
            }
            Preset::Smoke => {
                config.lifetime = 2.0;
                config.speed = 15.0;
                config.count = 15;
                config.color_start = Color::new(0.5, 0.5, 0.5, 0.6);
                config.color_end = Color::new(0.3, 0.3, 0.3, 0.0);
                config.shape = EmitterShape::Circle { radius: 10.0 };
            }
        }

        config
    }
}

struct ParticleDemo {
    particles: ParticleSystem,
    current_preset: Preset,
}

impl ParticleDemo {
    fn new() -> Self {
        Self {
            particles: ParticleSystem::new(),
            current_preset: Preset::Fire,
        }
    }
}

impl Game for ParticleDemo {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        if ctx.input.pressed(KeyCode::Space) {
            self.current_preset = self.current_preset.next();
        }

        if ctx.input.pressed_mouse(MouseButton::Left) {
            let pos = ctx.input.mouse_world_pos();
            let config = self.current_preset.emitter_config(pos);
            self.particles.add_emitter(config);
        }

        self.particles.update(ctx.dt);

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        self.particles.draw(ctx);

        let status = format!(
            "Preset: {} | Particles: {} | Click to spawn, Space to cycle",
            self.current_preset.name(),
            self.particles.particle_count(),
        );
        ctx.draw_text(&status, 4.0, 4.0, Color::WHITE);
    }
}

fn main() {
    let game = ParticleDemo::new();
    Engine::build()
        .title("Particles Demo")
        .virtual_resolution(480, 270)
        .build()
        .run(game);
}
