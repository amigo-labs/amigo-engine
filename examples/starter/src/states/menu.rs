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

        // Title block (centered white rectangle as placeholder for text)
        let title_w = 200.0;
        let title_h = 24.0;
        let tx = (vw - title_w) * 0.5;
        ctx.draw_rect(
            Rect::new(tx, vh * 0.35, title_w, title_h),
            Color::rgb(0.94, 0.86, 0.70),
        );

        // "Press SPACE" prompt — blinking rectangle
        let blink = ((ctx.alpha * 60.0) as u32 / 30) % 2 == 0;
        if blink {
            let prompt_w = 140.0;
            let prompt_h = 12.0;
            let px = (vw - prompt_w) * 0.5;
            ctx.draw_rect(
                Rect::new(px, vh * 0.55, prompt_w, prompt_h),
                Color::rgb(0.6, 0.55, 0.5),
            );
        }
    }
}
