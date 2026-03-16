use amigo_engine::prelude::*;

struct AudioDemo {
    current_track: &'static str,
    volume: f32,
}

impl AudioDemo {
    fn new() -> Self {
        Self {
            current_track: "None",
            volume: 0.7,
        }
    }

    fn volume_percent(&self) -> u32 {
        (self.volume * 100.0).round() as u32
    }
}

impl Game for AudioDemo {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        // Key 1: play music track A
        if ctx.input.pressed(KeyCode::Digit1) {
            if self.current_track == "Track A" {
                ctx.audio.play_music("track_a");
            } else {
                ctx.audio.crossfade("track_a", 1.0);
            }
            self.current_track = "Track A";
        }

        // Key 2: play music track B (crossfade from current)
        if ctx.input.pressed(KeyCode::Digit2) {
            if self.current_track == "Track B" {
                ctx.audio.play_music("track_b");
            } else {
                ctx.audio.crossfade("track_b", 1.0);
            }
            self.current_track = "Track B";
        }

        // Space: play SFX click
        if ctx.input.pressed(KeyCode::Space) {
            ctx.audio.play_sfx("click");
        }

        // Up arrow: increase volume
        if ctx.input.pressed(KeyCode::ArrowUp) {
            self.volume = (self.volume + 0.1).min(1.0);
            ctx.audio.set_master_volume(self.volume);
        }

        // Down arrow: decrease volume
        if ctx.input.pressed(KeyCode::ArrowDown) {
            self.volume = (self.volume - 0.1).max(0.0);
            ctx.audio.set_master_volume(self.volume);
        }

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        ctx.draw_text("=== Audio Demo ===", 20.0, 20.0, Color::WHITE);

        let track_text = format!("Current Track: {}", self.current_track);
        ctx.draw_text(&track_text, 20.0, 50.0, Color::WHITE);

        let volume_text = format!("Volume: {}%", self.volume_percent());
        ctx.draw_text(&volume_text, 20.0, 80.0, Color::WHITE);

        ctx.draw_text("Controls:", 20.0, 120.0, Color::WHITE);
        ctx.draw_text("[1] Play Track A", 30.0, 145.0, Color::WHITE);
        ctx.draw_text("[2] Play Track B", 30.0, 165.0, Color::WHITE);
        ctx.draw_text("[Space] Play SFX Click", 30.0, 185.0, Color::WHITE);
        ctx.draw_text("[Up/Down] Adjust Volume", 30.0, 205.0, Color::WHITE);
    }
}

fn main() {
    let game = AudioDemo::new();

    Engine::build()
        .title("Audio Demo")
        .virtual_resolution(480, 270)
        .build()
        .run(game);
}
