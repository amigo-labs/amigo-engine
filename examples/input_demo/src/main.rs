use amigo_engine::prelude::*;

struct InputDemo {
    mouse_x: f32,
    mouse_y: f32,
    left_held: bool,
    right_held: bool,
    wasd: [bool; 4],
    arrows: [bool; 4],
    space_held: bool,
}

impl InputDemo {
    fn new() -> Self {
        Self {
            mouse_x: 0.0,
            mouse_y: 0.0,
            left_held: false,
            right_held: false,
            wasd: [false; 4],
            arrows: [false; 4],
            space_held: false,
        }
    }
}

impl Game for InputDemo {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        let mpos = ctx.input.mouse_world_pos();
        self.mouse_x = mpos.x;
        self.mouse_y = mpos.y;
        self.left_held = ctx.input.mouse_held(MouseButton::Left);
        self.right_held = ctx.input.mouse_held(MouseButton::Right);

        self.wasd = [
            ctx.input.held(KeyCode::KeyW),
            ctx.input.held(KeyCode::KeyA),
            ctx.input.held(KeyCode::KeyS),
            ctx.input.held(KeyCode::KeyD),
        ];
        self.arrows = [
            ctx.input.held(KeyCode::ArrowUp),
            ctx.input.held(KeyCode::ArrowDown),
            ctx.input.held(KeyCode::ArrowLeft),
            ctx.input.held(KeyCode::ArrowRight),
        ];
        self.space_held = ctx.input.held(KeyCode::Space);

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        // Mouse info
        ctx.draw_text(
            &format!("Mouse: ({:.0}, {:.0})", self.mouse_x, self.mouse_y),
            8.0, 8.0, Color::WHITE,
        );
        let left_str = if self.left_held { "held" } else { "released" };
        let right_str = if self.right_held { "held" } else { "released" };
        ctx.draw_text(&format!("Left: {left_str}"), 8.0, 22.0, Color::WHITE);
        ctx.draw_text(&format!("Right: {right_str}"), 8.0, 36.0, Color::WHITE);

        // Crosshair at mouse world position
        let (cx, cy) = (self.mouse_x, self.mouse_y);
        ctx.draw_rect(Rect::new(cx - 4.0, cy - 0.5, 8.0, 1.0), Color::GREEN);
        ctx.draw_rect(Rect::new(cx - 0.5, cy - 4.0, 1.0, 8.0), Color::GREEN);

        // WASD directional keys
        let labels = ["W", "A", "S", "D"];
        let mut move_str = String::new();
        for (i, &held) in self.wasd.iter().enumerate() {
            if held {
                if !move_str.is_empty() { move_str.push(' '); }
                move_str.push_str(labels[i]);
            }
        }
        if move_str.is_empty() { move_str.push_str("(none)"); }
        ctx.draw_text(&format!("Move keys: {move_str}"), 8.0, 56.0, Color::WHITE);

        // Arrow keys
        let arrow_labels = ["Up", "Down", "Left", "Right"];
        let mut arrow_str = String::new();
        for (i, &held) in self.arrows.iter().enumerate() {
            if held {
                if !arrow_str.is_empty() { arrow_str.push(' '); }
                arrow_str.push_str(arrow_labels[i]);
            }
        }
        if arrow_str.is_empty() { arrow_str.push_str("(none)"); }
        ctx.draw_text(&format!("Arrows: {arrow_str}"), 8.0, 70.0, Color::WHITE);

        // Jump (Space)
        let jump = if self.space_held { "ACTIVE" } else { "idle" };
        ctx.draw_text(&format!("Jump: {jump}"), 8.0, 84.0, Color::WHITE);

        // Action map legend
        ctx.draw_text(
            "Actions: Jump=Space, Move=WASD",
            8.0, 104.0, Color::new(180, 180, 180, 255),
        );
    }
}

fn main() {
    Engine::build()
        .title("Input Demo")
        .virtual_resolution(480, 270)
        .build()
        .run(InputDemo::new());
}
