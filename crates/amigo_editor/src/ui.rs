use crate::{AmigoLevel, EditorState, EditorTool, EditorCommand};
use amigo_core::{Color, Rect};
use amigo_ui::UiContext;
use amigo_input::InputState;

/// Colors for the editor UI.
const PANEL_BG: Color = Color { r: 0.15, g: 0.15, b: 0.18, a: 0.95 };
const TOOL_ACTIVE: Color = Color { r: 0.3, g: 0.5, b: 0.8, a: 1.0 };
const TOOL_INACTIVE: Color = Color { r: 0.25, g: 0.25, b: 0.28, a: 1.0 };
const HEADER_COLOR: Color = Color { r: 0.8, g: 0.9, b: 1.0, a: 1.0 };

/// Draw the complete editor UI. Returns any command produced by user interaction.
pub fn draw_editor_ui(
    ui: &mut UiContext,
    state: &mut EditorState,
    level: &AmigoLevel,
    input: &InputState,
    screen_w: f32,
    screen_h: f32,
) -> Option<EditorCommand> {
    let mut result_cmd = None;

    // Left toolbar (tools)
    draw_toolbar(ui, state, input);

    // Right panel (properties / tile palette)
    draw_properties_panel(ui, state, level, input, screen_w, screen_h);

    // Bottom status bar
    draw_status_bar(ui, state, level, screen_w, screen_h);

    // Handle keyboard shortcuts
    result_cmd = result_cmd.or_else(|| handle_shortcuts(state, input));

    result_cmd
}

fn draw_toolbar(ui: &mut UiContext, state: &mut EditorState, input: &InputState) {
    let x = 2.0;
    let mut y = 2.0;
    let w = 80.0;

    // Background panel
    ui.panel(Rect::new(x, y, w, 130.0), PANEL_BG);
    y += 4.0;
    ui.pixel_text("Tools", x + 4.0, y, HEADER_COLOR);
    y += 14.0;
    ui.separator(x + 2.0, y, w - 4.0);
    y += 4.0;

    let tools = [
        (EditorTool::Select, "Select"),
        (EditorTool::PaintTile, "Paint"),
        (EditorTool::Erase, "Erase"),
        (EditorTool::Fill, "Fill"),
        (EditorTool::PlaceEntity, "Entity"),
        (EditorTool::PathEdit, "Path"),
    ];

    for (tool, label) in &tools {
        let is_active = state.tool == *tool;
        let bg = if is_active { TOOL_ACTIVE } else { TOOL_INACTIVE };
        let btn_rect = Rect::new(x + 4.0, y, w - 8.0, 14.0);
        ui.filled_rect(btn_rect, bg);
        ui.pixel_text(label, x + 8.0, y + 3.0, Color::WHITE);

        let mouse = input.mouse_pos();
        if btn_rect.contains(mouse.x, mouse.y)
            && input.mouse_pressed(winit::event::MouseButton::Left)
        {
            state.tool = *tool;
        }
        y += 16.0;
    }
}

fn draw_properties_panel(
    ui: &mut UiContext,
    state: &mut EditorState,
    level: &AmigoLevel,
    input: &InputState,
    screen_w: f32,
    _screen_h: f32,
) {
    let panel_w = 120.0;
    let x = screen_w - panel_w - 2.0;
    let mut y = 2.0;

    // Background
    ui.panel(Rect::new(x, y, panel_w, 160.0), PANEL_BG);
    y += 4.0;
    ui.pixel_text("Properties", x + 4.0, y, HEADER_COLOR);
    y += 14.0;
    ui.separator(x + 2.0, y, panel_w - 4.0);
    y += 6.0;

    // Level info
    ui.pixel_text(
        &format!("{}x{}", level.width, level.height),
        x + 4.0,
        y,
        Color::WHITE,
    );
    y += 12.0;
    ui.pixel_text(
        &format!("Tile: {}px", level.tile_size),
        x + 4.0,
        y,
        Color::WHITE,
    );
    y += 12.0;
    ui.pixel_text(
        &format!("Layers: {}", level.layers.len()),
        x + 4.0,
        y,
        Color::WHITE,
    );
    y += 12.0;
    ui.pixel_text(
        &format!("Entities: {}", level.entities.len()),
        x + 4.0,
        y,
        Color::WHITE,
    );
    y += 16.0;

    // Toggle checkboxes
    state.grid_visible = ui.checkbox("Grid", x + 4.0, y, state.grid_visible, input);
    y += 16.0;
    state.show_collision = ui.checkbox("Collision", x + 4.0, y, state.show_collision, input);
    y += 16.0;
    state.show_paths = ui.checkbox("Paths", x + 4.0, y, state.show_paths, input);

    // Tile palette (when paint tool active)
    if state.tool == EditorTool::PaintTile {
        let palette_y = y + 24.0;
        ui.panel(Rect::new(x, palette_y, panel_w, 80.0), PANEL_BG);
        ui.pixel_text("Tile Palette", x + 4.0, palette_y + 4.0, HEADER_COLOR);
        ui.separator(x + 2.0, palette_y + 18.0, panel_w - 4.0);

        // Draw tile grid (8 tiles per row)
        let tiles_per_row = 8;
        let tile_preview_size = 12.0;
        let padding = 2.0;
        for i in 0..32u16 {
            let col = (i % tiles_per_row) as f32;
            let row = (i / tiles_per_row) as f32;
            let tx = x + 4.0 + col * (tile_preview_size + padding);
            let ty = palette_y + 22.0 + row * (tile_preview_size + padding);
            let rect = Rect::new(tx, ty, tile_preview_size, tile_preview_size);

            let bg = if i == state.selected_tile {
                TOOL_ACTIVE
            } else {
                Color::new(0.3, 0.3, 0.3, 0.8)
            };
            ui.filled_rect(rect, bg);

            let mouse = input.mouse_pos();
            if rect.contains(mouse.x, mouse.y)
                && input.mouse_pressed(winit::event::MouseButton::Left)
            {
                state.selected_tile = i;
            }
        }
    }
}

fn draw_status_bar(
    ui: &mut UiContext,
    state: &EditorState,
    _level: &AmigoLevel,
    screen_w: f32,
    screen_h: f32,
) {
    let bar_h = 16.0;
    let y = screen_h - bar_h;
    ui.filled_rect(Rect::new(0.0, y, screen_w, bar_h), PANEL_BG);

    let tool_name = match state.tool {
        EditorTool::Select => "Select",
        EditorTool::PaintTile => "Paint",
        EditorTool::Erase => "Erase",
        EditorTool::Fill => "Fill",
        EditorTool::PlaceEntity => "Entity",
        EditorTool::PathEdit => "Path",
    };

    ui.pixel_text(
        &format!("Tool: {} | Tile #{}", tool_name, state.selected_tile),
        4.0,
        y + 3.0,
        Color::WHITE,
    );

    if let Some((tx, ty)) = state.cursor_tile {
        ui.pixel_text(
            &format!("({}, {})", tx, ty),
            screen_w - 80.0,
            y + 3.0,
            Color::new(0.7, 0.7, 0.7, 1.0),
        );
    }

    // Undo/redo indicator
    let undo_text = format!(
        "Undo:{} Redo:{}",
        state.undo_stack.len(),
        state.redo_stack.len()
    );
    ui.pixel_text(
        &undo_text,
        screen_w * 0.5 - 40.0,
        y + 3.0,
        Color::new(0.6, 0.8, 0.6, 1.0),
    );
}

fn handle_shortcuts(state: &mut EditorState, input: &InputState) -> Option<EditorCommand> {
    use winit::keyboard::KeyCode;

    // Ctrl+Z: undo
    if input.held(KeyCode::ControlLeft) && input.pressed(KeyCode::KeyZ) {
        if input.held(KeyCode::ShiftLeft) {
            return state.redo();
        } else {
            return state.undo();
        }
    }

    // Tool shortcuts
    if input.pressed(KeyCode::KeyS) { state.tool = EditorTool::Select; }
    if input.pressed(KeyCode::KeyP) { state.tool = EditorTool::PaintTile; }
    if input.pressed(KeyCode::KeyE) { state.tool = EditorTool::Erase; }
    if input.pressed(KeyCode::KeyF) { state.tool = EditorTool::Fill; }
    if input.pressed(KeyCode::KeyG) { state.grid_visible = !state.grid_visible; }

    None
}
