use amigo_engine::prelude::*;

/// Minimal loading state. In a real game this would show a loading bar
/// while assets stream in. Here we just wait one tick to demonstrate
/// the state transition.
pub struct LoadingState {
    frames_waited: u32,
}

impl LoadingState {
    pub fn new() -> Self {
        Self { frames_waited: 0 }
    }

    /// Returns true when loading is "done" (after 1 tick).
    pub fn update(&mut self, _ctx: &mut GameContext) -> bool {
        self.frames_waited += 1;
        self.frames_waited > 1
    }

    pub fn draw(&self, ctx: &mut DrawContext) {
        // Dark background
        ctx.draw_rect(
            Rect::new(0.0, 0.0, ctx.virtual_width, ctx.virtual_height),
            Color::rgb(0.05, 0.05, 0.08),
        );

        // "Loading..." text
        let (tw, _) = ctx.measure_text("Loading...");
        ctx.draw_text(
            "Loading...",
            (ctx.virtual_width - tw) * 0.5,
            ctx.virtual_height * 0.45,
            Color::WHITE,
        );

        // Simple "loading" indicator bar
        let bar_w = 120.0;
        let bar_h = 8.0;
        let x = (ctx.virtual_width - bar_w) * 0.5;
        let y = ctx.virtual_height * 0.55;
        ctx.draw_rect(Rect::new(x, y, bar_w, bar_h), Color::rgb(0.2, 0.2, 0.25));
        ctx.draw_rect(
            Rect::new(x, y, bar_w * 0.6, bar_h),
            Color::rgb(0.4, 0.7, 1.0),
        );
    }
}
