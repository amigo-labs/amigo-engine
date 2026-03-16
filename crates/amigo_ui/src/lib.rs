use amigo_core::{Color, Rect};
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
    pub fn sprite_button(
        &mut self,
        name: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        input: &InputState,
    ) -> bool {
        let _id = self.gen_id();
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

// ---------------------------------------------------------------------------
// Extended widgets
// ---------------------------------------------------------------------------

/// A label + checkbox widget. Returns the new checked state.
impl UiContext {
    pub fn checkbox(
        &mut self,
        label: &str,
        x: f32,
        y: f32,
        checked: bool,
        input: &InputState,
    ) -> bool {
        let _id = self.gen_id();
        let box_size = 12.0;
        let rect = Rect::new(x, y, box_size, box_size);

        // Draw box outline
        self.rect_outline(rect, Color::WHITE);

        // Draw check mark if checked
        if checked {
            let inner = Rect::new(x + 2.0, y + 2.0, box_size - 4.0, box_size - 4.0);
            self.filled_rect(inner, Color::WHITE);
        }

        // Draw label
        self.pixel_text(label, x + box_size + 4.0, y + 2.0, Color::WHITE);

        // Handle click
        let mouse = input.mouse_pos();
        let click_area = Rect::new(x, y, box_size + 4.0 + label.len() as f32 * 8.0, box_size);
        if click_area.contains(mouse.x, mouse.y)
            && input.mouse_pressed(winit::event::MouseButton::Left)
        {
            return !checked;
        }
        checked
    }

    /// A horizontal slider. Returns the new value (0.0 to 1.0).
    pub fn slider(&mut self, x: f32, y: f32, width: f32, value: f32, input: &InputState) -> f32 {
        let _id = self.gen_id();
        let height = 8.0;
        let handle_w = 6.0;
        let track = Rect::new(x, y + 2.0, width, height - 4.0);
        let val = value.clamp(0.0, 1.0);

        // Draw track
        self.filled_rect(track, Color::new(0.3, 0.3, 0.3, 0.8));

        // Draw filled portion
        let filled = Rect::new(x, y + 2.0, width * val, height - 4.0);
        self.filled_rect(filled, Color::new(0.4, 0.7, 1.0, 1.0));

        // Draw handle
        let handle_x = x + width * val - handle_w / 2.0;
        let handle = Rect::new(handle_x, y, handle_w, height);
        self.filled_rect(handle, Color::WHITE);

        // Handle drag
        let mouse = input.mouse_pos();
        let interact = Rect::new(x - 4.0, y - 4.0, width + 8.0, height + 8.0);
        if interact.contains(mouse.x, mouse.y) && input.mouse_held(winit::event::MouseButton::Left)
        {
            let new_val = ((mouse.x - x) / width).clamp(0.0, 1.0);
            return new_val;
        }
        val
    }

    /// A dropdown / select widget. Returns the new selected index.
    pub fn dropdown(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        options: &[&str],
        selected: usize,
        open: &mut bool,
        input: &InputState,
    ) -> usize {
        let _id = self.gen_id();
        let item_h = 16.0;

        // Draw current selection
        let header = Rect::new(x, y, width, item_h);
        self.filled_rect(header, Color::new(0.25, 0.25, 0.25, 0.9));
        self.rect_outline(header, Color::new(0.5, 0.5, 0.5, 1.0));
        let label = options.get(selected).copied().unwrap_or("—");
        self.pixel_text(label, x + 4.0, y + 4.0, Color::WHITE);

        // Arrow indicator
        let arrow = if *open { "v" } else { ">" };
        self.pixel_text(arrow, x + width - 12.0, y + 4.0, Color::WHITE);

        let mouse = input.mouse_pos();
        let clicked = input.mouse_pressed(winit::event::MouseButton::Left);

        // Toggle dropdown on header click
        if header.contains(mouse.x, mouse.y) && clicked {
            *open = !*open;
            return selected;
        }

        // Draw open dropdown list
        if *open {
            for (i, option) in options.iter().enumerate() {
                let oy = y + item_h * (i + 1) as f32;
                let item_rect = Rect::new(x, oy, width, item_h);

                let bg_color = if item_rect.contains(mouse.x, mouse.y) {
                    Color::new(0.4, 0.5, 0.7, 0.9)
                } else if i == selected {
                    Color::new(0.3, 0.3, 0.4, 0.9)
                } else {
                    Color::new(0.2, 0.2, 0.2, 0.9)
                };
                self.filled_rect(item_rect, bg_color);
                self.pixel_text(option, x + 4.0, oy + 4.0, Color::WHITE);

                if item_rect.contains(mouse.x, mouse.y) && clicked {
                    *open = false;
                    return i;
                }
            }
        }

        selected
    }

    /// A tooltip that appears near a position. Call this after the widget it describes.
    pub fn tooltip(&mut self, text: &str, x: f32, y: f32) {
        let padding = 4.0;
        let w = text.len() as f32 * 7.0 + padding * 2.0;
        let h = 14.0;
        let bg = Rect::new(x, y - h - 2.0, w, h);
        self.filled_rect(bg, Color::new(0.1, 0.1, 0.1, 0.95));
        self.rect_outline(bg, Color::new(0.5, 0.5, 0.5, 1.0));
        self.pixel_text(text, x + padding, y - h + 2.0, Color::WHITE);
    }

    /// A simple text label with a background panel.
    pub fn label_panel(&mut self, text: &str, x: f32, y: f32, color: Color) {
        let padding = 4.0;
        let w = text.len() as f32 * 7.0 + padding * 2.0;
        let h = 14.0;
        self.filled_rect(Rect::new(x, y, w, h), Color::new(0.0, 0.0, 0.0, 0.6));
        self.pixel_text(text, x + padding, y + 3.0, color);
    }

    /// A text button. Returns true if clicked.
    pub fn text_button(&mut self, label: &str, x: f32, y: f32, input: &InputState) -> bool {
        let _id = self.gen_id();
        let padding = 6.0;
        let w = label.len() as f32 * 7.0 + padding * 2.0;
        let h = 16.0;
        let rect = Rect::new(x, y, w, h);

        let mouse = input.mouse_pos();
        let hovering = rect.contains(mouse.x, mouse.y);

        let bg = if hovering {
            Color::new(0.4, 0.5, 0.7, 0.9)
        } else {
            Color::new(0.3, 0.3, 0.3, 0.9)
        };
        self.filled_rect(rect, bg);
        self.rect_outline(rect, Color::new(0.6, 0.6, 0.6, 1.0));
        self.pixel_text(label, x + padding, y + 4.0, Color::WHITE);

        hovering && input.mouse_pressed(winit::event::MouseButton::Left)
    }

    /// A separator line.
    pub fn separator(&mut self, x: f32, y: f32, width: f32) {
        self.filled_rect(Rect::new(x, y, width, 1.0), Color::new(0.5, 0.5, 0.5, 0.5));
    }
}
