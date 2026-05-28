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

    /// Build the emitter configuration and spawn shape for this preset.
    fn emitter(self) -> (EmitterConfig, EmitterShape) {
        match self {
            Preset::Fire => (
                EmitterConfig {
                    max_particles: 80,
                    emission_rate: 120.0,
                    lifetime_min: 0.6,
                    lifetime_max: 0.8,
                    speed_min: 40.0,
                    speed_max: 60.0,
                    direction: -std::f32::consts::FRAC_PI_2, // upward
                    spread: 0.4,
                    gravity: -30.0,
                    color_start: Color::new(1.0, 0.6, 0.0, 1.0),
                    color_end: Color::new(1.0, 0.0, 0.0, 0.0),
                    size_start: 3.0,
                    size_end: 1.0,
                    ..Default::default()
                },
                EmitterShape::Circle { radius: 6.0 },
            ),
            Preset::Snow => (
                EmitterConfig {
                    max_particles: 120,
                    emission_rate: 40.0,
                    lifetime_min: 2.5,
                    lifetime_max: 3.0,
                    speed_min: 15.0,
                    speed_max: 25.0,
                    direction: std::f32::consts::FRAC_PI_2, // downward
                    spread: 0.3,
                    gravity: 10.0,
                    color_start: Color::WHITE,
                    color_end: Color::new(0.8, 0.9, 1.0, 0.0),
                    size_start: 2.0,
                    size_end: 2.0,
                    ..Default::default()
                },
                EmitterShape::Circle { radius: 30.0 },
            ),
            Preset::Sparks => (
                EmitterConfig {
                    max_particles: 60,
                    emission_rate: f32::MAX,
                    lifetime_min: 0.3,
                    lifetime_max: 0.5,
                    speed_min: 120.0,
                    speed_max: 180.0,
                    direction: 0.0,
                    spread: std::f32::consts::PI, // full circle
                    gravity: 120.0,
                    color_start: Color::new(1.0, 1.0, 0.5, 1.0),
                    color_end: Color::new(1.0, 0.8, 0.0, 0.0),
                    size_start: 2.0,
                    size_end: 1.0,
                    burst: true,
                    ..Default::default()
                },
                EmitterShape::Point,
            ),
            Preset::Smoke => (
                EmitterConfig {
                    max_particles: 60,
                    emission_rate: 30.0,
                    lifetime_min: 1.5,
                    lifetime_max: 2.0,
                    speed_min: 10.0,
                    speed_max: 20.0,
                    direction: -std::f32::consts::FRAC_PI_2, // upward
                    spread: 0.6,
                    gravity: -10.0,
                    color_start: Color::new(0.5, 0.5, 0.5, 0.6),
                    color_end: Color::new(0.3, 0.3, 0.3, 0.0),
                    size_start: 4.0,
                    size_end: 8.0,
                    ..Default::default()
                },
                EmitterShape::Circle { radius: 10.0 },
            ),
        }
    }
}

struct ParticleDemo {
    current_preset: Preset,
    particle_count: usize,
}

impl ParticleDemo {
    fn new() -> Self {
        Self {
            current_preset: Preset::Fire,
            particle_count: 0,
        }
    }
}

impl Game for ParticleDemo {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        if ctx.input.pressed(KeyCode::Space) {
            self.current_preset = self.current_preset.next();
        }

        if ctx.input.mouse_pressed(MouseButton::Left) {
            let pos = ctx.input.mouse_world_pos();
            let (config, shape) = self.current_preset.emitter();
            // The engine updates and draws `ctx.particles` automatically.
            ctx.particles
                .spawn(self.current_preset.name(), config, shape, pos.x, pos.y);
        }

        self.particle_count = ctx.particles.particle_count();

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        let status = format!(
            "Preset: {} | Particles: {} | Click to spawn, Space to cycle",
            self.current_preset.name(),
            self.particle_count,
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
