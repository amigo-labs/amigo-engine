use amigo_engine::prelude::*;

// ---------------------------------------------------------------------------
// Animation state enum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CharState {
    Idle,
    Walk,
    Jump,
}

impl CharState {
    fn name(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Walk => "Walk",
            Self::Jump => "Jump",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Idle => Color::new(80, 180, 80, 255),
            Self::Walk => Color::new(80, 120, 220, 255),
            Self::Jump => Color::new(220, 160, 50, 255),
        }
    }

    fn frame_count(self) -> usize {
        match self {
            Self::Idle => 4,
            Self::Walk => 6,
            Self::Jump => 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Game
// ---------------------------------------------------------------------------

struct AnimationDemo {
    state: CharState,
    frame_index: usize,
    frame_timer: f32,
    jump_timer: f32,
    facing_left: bool,
}

const FRAME_DURATION: f32 = 0.12;
const JUMP_DURATION: f32 = 0.5;
const CHAR_W: f32 = 24.0;
const CHAR_H: f32 = 32.0;

impl AnimationDemo {
    fn new() -> Self {
        Self {
            state: CharState::Idle,
            frame_index: 0,
            frame_timer: 0.0,
            jump_timer: 0.0,
            facing_left: false,
        }
    }

    fn set_state(&mut self, new: CharState) {
        if self.state != new {
            self.state = new;
            self.frame_index = 0;
            self.frame_timer = 0.0;
        }
    }
}

impl Game for AnimationDemo {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        let dt = ctx.time.dt;

        // --- Input handling ---
        match self.state {
            CharState::Jump => {
                self.jump_timer -= dt;
                if self.jump_timer <= 0.0 {
                    self.set_state(CharState::Idle);
                }
            }
            _ => {
                if ctx.input.pressed(KeyCode::Space) {
                    self.set_state(CharState::Jump);
                    self.jump_timer = JUMP_DURATION;
                } else if ctx.input.held(KeyCode::ArrowLeft)
                    || ctx.input.held(KeyCode::ArrowRight)
                {
                    if ctx.input.held(KeyCode::ArrowLeft) {
                        self.facing_left = true;
                    }
                    if ctx.input.held(KeyCode::ArrowRight) {
                        self.facing_left = false;
                    }
                    self.set_state(CharState::Walk);
                } else {
                    self.set_state(CharState::Idle);
                }
            }
        }

        // --- Advance frame timer ---
        self.frame_timer += dt;
        if self.frame_timer >= FRAME_DURATION {
            self.frame_timer -= FRAME_DURATION;
            self.frame_index = (self.frame_index + 1) % self.state.frame_count();
        }

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        let cx = ctx.virtual_width / 2.0;
        let cy = ctx.virtual_height / 2.0;

        // Character placeholder rect
        let x = cx - CHAR_W / 2.0;
        let y = cy - CHAR_H / 2.0;
        ctx.draw_rect(Rect::new(x, y, CHAR_W, CHAR_H), self.state.color());

        // Direction indicator (small triangle-like bar)
        let dir_x = if self.facing_left { x - 6.0 } else { x + CHAR_W + 2.0 };
        ctx.draw_rect(Rect::new(dir_x, cy - 3.0, 4.0, 6.0), Color::WHITE);

        // HUD text
        let state_text = format!(
            "State: {}  Frame: {}/{}",
            self.state.name(),
            self.frame_index + 1,
            self.state.frame_count(),
        );
        ctx.draw_text(&state_text, 4.0, 4.0, Color::WHITE);
        ctx.draw_text("Arrows=Walk  Space=Jump", 4.0, 16.0, Color::new(180, 180, 180, 255));
    }
}

fn main() {
    Engine::build()
        .title("Animation Demo")
        .virtual_resolution(480, 270)
        .build()
        .run(AnimationDemo::new());
}
