use amigo_engine::prelude::*;

pub struct MenuState;

impl MenuState {
    pub fn new() -> Self {
        Self
    }

    /// Returns true when player presses Space/Enter to start.
    pub fn update(&mut self, ctx: &mut GameContext) -> bool {
        ctx.input.pressed(KeyCode::Space) || ctx.input.pressed(KeyCode::Enter)
    }

    pub fn draw(&self, ctx: &mut DrawContext) {
        let vw = ctx.virtual_width;
        let vh = ctx.virtual_height;

        // Background
        ctx.draw_rect(Rect::new(0.0, 0.0, vw, vh), Color::rgb(0.08, 0.06, 0.12));

        // Title text
        let title = "AMIGO STARTER";
        let (tw, _) = ctx.measure_text(title);
        ctx.draw_text(title, (vw - tw) * 0.5, vh * 0.35, Color::rgb(0.94, 0.86, 0.70));

        // "Press SPACE" prompt — blinking text
        let blink = ((ctx.alpha * 60.0) as u32 / 30) % 2 == 0;
        if blink {
            let prompt = "Press SPACE to start";
            let (pw, _) = ctx.measure_text(prompt);
            ctx.draw_text(prompt, (vw - pw) * 0.5, vh * 0.55, Color::rgb(0.6, 0.55, 0.5));
        }
    }
}
