use crate::wizard::{NewProjectWizard, WizardStep};
use amigo_core::game_preset::ScenePreset;
use amigo_core::{Color, Rect};
use amigo_input::InputState;
use amigo_ui::UiContext;

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

const WIZ_BG: Color = Color { r: 0.10, g: 0.10, b: 0.13, a: 0.97 };
const WIZ_HEADER: Color = Color { r: 0.85, g: 0.92, b: 1.0, a: 1.0 };
const WIZ_ACCENT: Color = Color { r: 0.35, g: 0.55, b: 0.85, a: 1.0 };
const WIZ_DIM: Color = Color { r: 0.50, g: 0.50, b: 0.55, a: 1.0 };
const WIZ_SELECTED_BG: Color = Color { r: 0.25, g: 0.38, b: 0.60, a: 0.95 };
const WIZ_ITEM_BG: Color = Color { r: 0.18, g: 0.18, b: 0.22, a: 0.95 };
const WIZ_ITEM_HOVER: Color = Color { r: 0.25, g: 0.28, b: 0.35, a: 0.95 };

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Draw the project wizard overlay. Returns `true` when the wizard is done
/// (either completed or cancelled).
pub fn draw_wizard_ui(
    ui: &mut UiContext,
    wizard: &mut NewProjectWizard,
    input: &InputState,
    screen_w: f32,
    screen_h: f32,
) -> bool {
    if !wizard.is_active() {
        return true;
    }

    // Full-screen dimmed backdrop
    ui.filled_rect(
        Rect::new(0.0, 0.0, screen_w, screen_h),
        Color::new(0.0, 0.0, 0.0, 0.65),
    );

    // Centered wizard panel
    let panel_w = 260.0;
    let panel_h = 200.0;
    let px = (screen_w - panel_w) * 0.5;
    let py = (screen_h - panel_h) * 0.5;

    ui.panel(Rect::new(px, py, panel_w, panel_h), WIZ_BG);
    ui.rect_outline(
        Rect::new(px, py, panel_w, panel_h),
        Color::new(0.4, 0.4, 0.5, 0.8),
    );

    // Header with step progress
    let step_text = format!(
        "New Project  ({}/{})",
        wizard.step_number(),
        wizard.total_steps()
    );
    ui.pixel_text(&step_text, px + 6.0, py + 5.0, WIZ_HEADER);

    // Progress bar
    let progress = wizard.step_number() as f32 / wizard.total_steps() as f32;
    ui.progress_bar(
        Rect::new(px + 6.0, py + 18.0, panel_w - 12.0, 4.0),
        progress,
        WIZ_ACCENT,
    );

    ui.separator(px + 4.0, py + 26.0, panel_w - 8.0);

    // Content area starts below header
    let content_y = py + 32.0;
    let content_h = panel_h - 56.0; // room for nav buttons at bottom

    match wizard.step {
        WizardStep::ChooseTemplate => {
            draw_step_template(ui, wizard, input, px, content_y, panel_w, content_h);
        }
        WizardStep::ProjectSettings => {
            draw_step_settings(ui, wizard, input, px, content_y, panel_w);
        }
        WizardStep::AddScenes => {
            draw_step_scenes(ui, wizard, input, px, content_y, panel_w, content_h);
        }
        WizardStep::Review => {
            draw_step_review(ui, wizard, px, content_y, panel_w, content_h);
        }
        WizardStep::Done | WizardStep::Cancelled => {}
    }

    // Navigation buttons at bottom
    draw_nav_buttons(ui, wizard, input, px, py + panel_h - 22.0, panel_w);

    // Handle Escape to cancel
    if input.pressed(winit::keyboard::KeyCode::Escape) {
        wizard.cancel();
    }

    !wizard.is_active()
}

// ---------------------------------------------------------------------------
// Step 1: Choose template
// ---------------------------------------------------------------------------

fn draw_step_template(
    ui: &mut UiContext,
    wizard: &mut NewProjectWizard,
    input: &InputState,
    px: f32,
    start_y: f32,
    panel_w: f32,
    content_h: f32,
) {
    ui.pixel_text("Choose a template:", px + 6.0, start_y, WIZ_DIM);
    let mut y = start_y + 14.0;
    let item_h = 14.0;
    let mouse = input.mouse_pos();
    let clicked = input.mouse_pressed(winit::event::MouseButton::Left);

    let max_visible = ((content_h - 14.0) / item_h) as usize;
    let mut clicked_idx = None;

    let template_count = wizard.templates.len().min(max_visible);
    for i in 0..template_count {
        let template = &wizard.templates[i];
        let item_rect = Rect::new(px + 4.0, y, panel_w - 8.0, item_h - 1.0);
        let hovering = item_rect.contains(mouse.x, mouse.y);

        let bg = if i == wizard.selected_template {
            WIZ_SELECTED_BG
        } else if hovering {
            WIZ_ITEM_HOVER
        } else {
            WIZ_ITEM_BG
        };

        ui.filled_rect(item_rect, bg);
        ui.pixel_text(template.name, px + 8.0, y + 2.0, Color::WHITE);

        // Short description on the right
        let desc_x = px + 100.0;
        let desc_w = panel_w - 108.0;
        let truncated: String = template.description.chars().take((desc_w / 6.0) as usize).collect();
        ui.pixel_text(&truncated, desc_x, y + 2.0, WIZ_DIM);

        if hovering && clicked {
            clicked_idx = Some(i);
        }

        y += item_h;
    }

    if let Some(idx) = clicked_idx {
        wizard.select_template(idx);
    }
}

// ---------------------------------------------------------------------------
// Step 2: Project settings
// ---------------------------------------------------------------------------

fn draw_step_settings(
    ui: &mut UiContext,
    wizard: &mut NewProjectWizard,
    input: &InputState,
    px: f32,
    start_y: f32,
    panel_w: f32,
) {
    let mut y = start_y;

    // Project name
    ui.pixel_text("Project Name:", px + 6.0, y, WIZ_DIM);
    y += 14.0;

    // Text input box
    let input_rect = Rect::new(px + 6.0, y, panel_w - 12.0, 16.0);
    ui.filled_rect(input_rect, Color::new(0.12, 0.12, 0.15, 1.0));
    ui.rect_outline(input_rect, WIZ_ACCENT);

    // Show name with cursor
    let display = format!("{}|", wizard.project_name);
    ui.pixel_text(&display, px + 10.0, y + 4.0, Color::WHITE);
    y += 24.0;

    // Handle text input via key presses
    handle_text_input(wizard, input);


    // Resolution
    ui.pixel_text("Resolution:", px + 6.0, y, WIZ_DIM);
    y += 14.0;

    let res_options: [(u32, u32, &str); 4] = [
        (320, 180, "320x180 (16:9)"),
        (480, 270, "480x270 (16:9)"),
        (256, 224, "256x224 (SNES)"),
        (240, 320, "240x320 (Portrait)"),
    ];

    let mouse = input.mouse_pos();
    let clicked = input.mouse_pressed(winit::event::MouseButton::Left);

    for (w, h, label) in &res_options {
        let item_rect = Rect::new(px + 6.0, y, panel_w - 12.0, 13.0);
        let is_selected = wizard.custom_width == *w && wizard.custom_height == *h;
        let hovering = item_rect.contains(mouse.x, mouse.y);

        let bg = if is_selected {
            WIZ_SELECTED_BG
        } else if hovering {
            WIZ_ITEM_HOVER
        } else {
            WIZ_ITEM_BG
        };

        ui.filled_rect(item_rect, bg);

        let marker = if is_selected { "> " } else { "  " };
        ui.pixel_text(&format!("{}{}", marker, label), px + 8.0, y + 2.0, Color::WHITE);

        if hovering && clicked {
            wizard.custom_width = *w;
            wizard.custom_height = *h;
        }

        y += 14.0;
    }

    y += 8.0;
    let template = &wizard.templates[wizard.selected_template];
    ui.pixel_text(
        &format!("Template: {}", template.name),
        px + 6.0,
        y,
        WIZ_DIM,
    );
}

// ---------------------------------------------------------------------------
// Step 3: Add extra scenes
// ---------------------------------------------------------------------------

fn draw_step_scenes(
    ui: &mut UiContext,
    wizard: &mut NewProjectWizard,
    input: &InputState,
    px: f32,
    start_y: f32,
    panel_w: f32,
    content_h: f32,
) {
    let mut y = start_y;

    // Default scenes (read-only)
    ui.pixel_text("Default scenes:", px + 6.0, y, WIZ_DIM);
    y += 14.0;

    let template = &wizard.templates[wizard.selected_template];
    let defaults = [
        ("Title Menu", "Menu"),
        ("Gameplay", template.primary_preset.display_name()),
        ("Pause Menu", "Menu"),
    ];

    for (name, preset) in &defaults {
        ui.pixel_text(
            &format!("  {} ({})", name, preset),
            px + 6.0,
            y,
            Color::new(0.6, 0.7, 0.6, 1.0),
        );
        y += 12.0;
    }

    y += 4.0;
    ui.separator(px + 4.0, y, panel_w - 8.0);
    y += 6.0;

    // Extra scenes the user added
    if !wizard.extra_scenes.is_empty() {
        ui.pixel_text("Extra scenes:", px + 6.0, y, WIZ_DIM);
        y += 14.0;

        let mut remove_idx = None;

        for (i, scene) in wizard.extra_scenes.iter().enumerate() {
            let row_rect = Rect::new(px + 6.0, y, panel_w - 50.0, 13.0);
            ui.filled_rect(row_rect, WIZ_ITEM_BG);
            ui.pixel_text(
                &format!("  {} ({})", scene.name, scene.preset.display_name()),
                px + 6.0,
                y + 2.0,
                Color::WHITE,
            );

            // Remove button
            let rm_x = px + panel_w - 40.0;
            if ui.text_button("[x]", rm_x, y, input) {
                remove_idx = Some(i);
            }

            y += 15.0;
        }

        if let Some(idx) = remove_idx {
            wizard.remove_extra_scene(idx);
        }

        y += 4.0;
    }

    // "Add Scene" button — adds a scene with the next gameplay preset
    if y < start_y + content_h - 20.0 {
        let presets = ScenePreset::gameplay_presets();
        let next_preset = presets[wizard.extra_scenes.len() % presets.len()];
        let btn_label = format!("+ Add {} Scene", next_preset.display_name());

        if ui.text_button(&btn_label, px + 6.0, y, input) {
            let name = format!("{} Scene", next_preset.display_name());
            wizard.add_extra_scene(name, next_preset);
        }
    }
}

// ---------------------------------------------------------------------------
// Step 4: Review
// ---------------------------------------------------------------------------

fn draw_step_review(
    ui: &mut UiContext,
    wizard: &NewProjectWizard,
    px: f32,
    start_y: f32,
    _panel_w: f32,
    content_h: f32,
) {
    let lines = wizard.review_summary();
    let mut y = start_y;
    let max_y = start_y + content_h;

    for line in &lines {
        if y >= max_y {
            break;
        }

        if line.is_empty() {
            y += 6.0;
            continue;
        }

        let color = if line.starts_with("  -") {
            Color::new(0.7, 0.8, 0.7, 1.0)
        } else if line.starts_with("Systems:") || line.starts_with("Scenes:") {
            WIZ_HEADER
        } else {
            Color::WHITE
        };

        ui.pixel_text(line, px + 6.0, y, color);
        y += 12.0;
    }
}

// ---------------------------------------------------------------------------
// Navigation buttons (Back / Next / Cancel)
// ---------------------------------------------------------------------------

fn draw_nav_buttons(
    ui: &mut UiContext,
    wizard: &mut NewProjectWizard,
    input: &InputState,
    px: f32,
    y: f32,
    panel_w: f32,
) {
    // Cancel (left)
    if ui.text_button("Cancel", px + 6.0, y, input) {
        wizard.cancel();
    }

    // Back (center-left) — not on first step
    if wizard.step != WizardStep::ChooseTemplate {
        if ui.text_button("< Back", px + 70.0, y, input) {
            wizard.back();
        }
    }

    // Next / Create (right)
    let is_final = wizard.step == WizardStep::Review;
    let next_label = if is_final { "Create!" } else { "Next >" };
    let next_x = px + panel_w - next_label.len() as f32 * 7.0 - 18.0;

    if ui.text_button(next_label, next_x, y, input) {
        wizard.next();
    }

    // Enter key also advances
    if input.pressed(winit::keyboard::KeyCode::Enter) {
        wizard.next();
    }
}

// ---------------------------------------------------------------------------
// Text input helper (maps key presses to characters)
// ---------------------------------------------------------------------------

fn handle_text_input(wizard: &mut NewProjectWizard, input: &InputState) {
    use winit::keyboard::KeyCode;

    if input.pressed(KeyCode::Backspace) {
        wizard.backspace();
    }

    let shift = input.held(KeyCode::ShiftLeft) || input.held(KeyCode::ShiftRight);

    // Letter keys
    let letters = [
        (KeyCode::KeyA, 'a'), (KeyCode::KeyB, 'b'), (KeyCode::KeyC, 'c'),
        (KeyCode::KeyD, 'd'), (KeyCode::KeyE, 'e'), (KeyCode::KeyF, 'f'),
        (KeyCode::KeyG, 'g'), (KeyCode::KeyH, 'h'), (KeyCode::KeyI, 'i'),
        (KeyCode::KeyJ, 'j'), (KeyCode::KeyK, 'k'), (KeyCode::KeyL, 'l'),
        (KeyCode::KeyM, 'm'), (KeyCode::KeyN, 'n'), (KeyCode::KeyO, 'o'),
        (KeyCode::KeyP, 'p'), (KeyCode::KeyQ, 'q'), (KeyCode::KeyR, 'r'),
        (KeyCode::KeyS, 's'), (KeyCode::KeyT, 't'), (KeyCode::KeyU, 'u'),
        (KeyCode::KeyV, 'v'), (KeyCode::KeyW, 'w'), (KeyCode::KeyX, 'x'),
        (KeyCode::KeyY, 'y'), (KeyCode::KeyZ, 'z'),
    ];

    for (code, ch) in &letters {
        if input.pressed(*code) {
            let c = if shift { ch.to_ascii_uppercase() } else { *ch };
            wizard.type_char(c);
        }
    }

    // Number keys
    let numbers = [
        (KeyCode::Digit0, '0'), (KeyCode::Digit1, '1'), (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'), (KeyCode::Digit4, '4'), (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'), (KeyCode::Digit7, '7'), (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'),
    ];

    for (code, ch) in &numbers {
        if input.pressed(*code) {
            wizard.type_char(*ch);
        }
    }

    // Space
    if input.pressed(KeyCode::Space) {
        wizard.type_char(' ');
    }

    // Common punctuation
    if input.pressed(KeyCode::Minus) {
        wizard.type_char(if shift { '_' } else { '-' });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wizard::NewProjectWizard;

    #[test]
    fn wizard_ui_runs_without_panic() {
        let mut ui = UiContext::new();
        let mut wizard = NewProjectWizard::new();
        let input = InputState::new();

        ui.begin();
        let done = draw_wizard_ui(&mut ui, &mut wizard, &input, 320.0, 180.0);
        assert!(!done, "Wizard should still be active");

        let cmds = ui.end();
        assert!(!cmds.is_empty(), "Should produce draw commands");
    }

    #[test]
    fn wizard_ui_done_returns_true() {
        let mut ui = UiContext::new();
        let mut wizard = NewProjectWizard::new();
        let input = InputState::new();

        // Advance to done
        wizard.next(); // → settings
        wizard.next(); // → scenes
        wizard.next(); // → review
        wizard.next(); // → done

        ui.begin();
        let done = draw_wizard_ui(&mut ui, &mut wizard, &input, 320.0, 180.0);
        assert!(done, "Wizard should be done");
    }

    #[test]
    fn wizard_ui_cancelled_returns_true() {
        let mut ui = UiContext::new();
        let mut wizard = NewProjectWizard::new();
        let input = InputState::new();

        wizard.cancel();

        ui.begin();
        let done = draw_wizard_ui(&mut ui, &mut wizard, &input, 320.0, 180.0);
        assert!(done, "Cancelled wizard should report done");
    }
}
