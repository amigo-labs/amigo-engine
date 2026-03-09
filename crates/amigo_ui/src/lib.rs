use amigo_core::{Color, Rect, RenderVec2};
use amigo_input::InputState;

/// Pixel UI context for immediate-mode UI rendering.
/// Tier 1: Game HUD (always available).
pub struct UiContext {
    draw_commands: Vec<UiDrawCommand>,
    hot_id: Option<u64>,
    active_id: Option<u64>,
    next_id: u64,
}

/// Internal draw command for the UI system.
#[derive(Clone, Debug)]
pub enum UiDrawCommand {
    Text {
        text: String,
        x: f32,
        y: f32,
        color: Color,
        scale: f32,
    },
    Rect {
        rect: Rect,
        color: Color,
        filled: bool,
    },
    Sprite {
        name: String,
        x: f32,
        y: f32,
    },
    ProgressBar {
        rect: Rect,
        fraction: f32,
        color: Color,
        bg_color: Color,
    },
}

impl UiContext {
    pub fn new() -> Self {
        Self {
            draw_commands: Vec::new(),
            hot_id: None,
            active_id: None,
            next_id: 1,
        }
    }

    fn gen_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Begin a new UI frame.
    pub fn begin(&mut self) {
        self.draw_commands.clear();
        self.next_id = 1;
    }

    /// End the UI frame. Returns draw commands.
    pub fn end(&mut self) -> &[UiDrawCommand] {
        &self.draw_commands
    }

    /// Draw text at a position.
    pub fn pixel_text(&mut self, text: &str, x: f32, y: f32, color: Color) {
        self.draw_commands.push(UiDrawCommand::Text {
            text: text.to_string(),
            x,
            y,
            color,
            scale: 1.0,
        });
    }

    /// Draw text with custom scale.
    pub fn pixel_text_scaled(&mut self, text: &str, x: f32, y: f32, color: Color, scale: f32) {
        self.draw_commands.push(UiDrawCommand::Text {
            text: text.to_string(),
            x,
            y,
            color,
            scale,
        });
    }

    /// Draw a sprite by name.
    pub fn sprite(&mut self, name: &str, x: f32, y: f32) {
        self.draw_commands.push(UiDrawCommand::Sprite {
            name: name.to_string(),
            x,
            y,
        });
    }

    /// Draw a filled rectangle.
    pub fn filled_rect(&mut self, rect: Rect, color: Color) {
        self.draw_commands.push(UiDrawCommand::Rect {
            rect,
            color,
            filled: true,
        });
    }

    /// Draw a rectangle outline.
    pub fn rect_outline(&mut self, rect: Rect, color: Color) {
        self.draw_commands.push(UiDrawCommand::Rect {
            rect,
            color,
            filled: false,
        });
    }

    /// Draw a progress bar.
    pub fn progress_bar(&mut self, rect: Rect, fraction: f32, color: Color) {
        self.draw_commands.push(UiDrawCommand::ProgressBar {
            rect,
            fraction: fraction.clamp(0.0, 1.0),
            color,
            bg_color: Color::new(0.2, 0.2, 0.2, 0.8),
        });
    }

    /// A clickable sprite button. Returns true if clicked.
    pub fn sprite_button(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32, input: &InputState) -> bool {
        let id = self.gen_id();
        let rect = Rect::new(x, y, w, h);

        self.draw_commands.push(UiDrawCommand::Sprite {
            name: name.to_string(),
            x,
            y,
        });

        let mouse = input.mouse_pos();
        let hovering = rect.contains(mouse.x, mouse.y);

        if hovering && input.mouse_pressed(winit::event::MouseButton::Left) {
            return true;
        }

        false
    }

    /// A panel with background.
    pub fn panel(&mut self, rect: Rect, color: Color) {
        self.filled_rect(rect, color);
    }

    /// Get all draw commands for this frame.
    pub fn draw_commands(&self) -> &[UiDrawCommand] {
        &self.draw_commands
    }
}

impl Default for UiContext {
    fn default() -> Self {
        Self::new()
    }
}
