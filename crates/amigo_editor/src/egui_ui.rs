use crate::session::tile_preview_color;
use crate::{AmigoLevel, EditorAction, EditorSession, EditorState, EditorTool};

/// Draw the complete editor UI using egui.
///
/// Call this from within the engine's egui render closure with the root
/// [`egui::Ui`] (egui 0.36 lays panels out inside a `Ui`, not a `Context`).
/// Undo and redo act on the session's level directly; the file actions are
/// returned for the engine to run, since saving also tells the game.
pub fn draw_editor_panels(ui: &mut egui::Ui, session: &mut EditorSession) -> Option<EditorAction> {
    let action = draw_menu_bar(ui, session);
    draw_tools_panel(ui, &mut session.state);
    draw_properties_panel(ui, &mut session.state, &session.level);
    draw_status_bar(ui, session);
    action
}

fn draw_menu_bar(ui: &mut egui::Ui, session: &mut EditorSession) -> Option<EditorAction> {
    let mut action = None;
    egui::Panel::top("editor_menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Level").clicked() {
                    action = Some(EditorAction::NewLevel);
                    ui.close();
                }
                if ui.button("Save (Ctrl+S)").clicked() {
                    action = Some(EditorAction::Save);
                    ui.close();
                }
                if ui.button("Revert to Saved").clicked() {
                    action = Some(EditorAction::Reload);
                    ui.close();
                }
            });

            ui.menu_button("Edit", |ui| {
                let state = &session.state;
                let undo_label = format!("Undo ({})", state.undo_stack.len());
                if ui
                    .add_enabled(state.can_undo(), egui::Button::new(undo_label))
                    .clicked()
                {
                    session.undo();
                    ui.close();
                }
                let state = &session.state;
                let redo_label = format!("Redo ({})", state.redo_stack.len());
                if ui
                    .add_enabled(state.can_redo(), egui::Button::new(redo_label))
                    .clicked()
                {
                    session.redo();
                    ui.close();
                }
            });

            let state = &mut session.state;
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut state.grid_visible, "Grid");
                ui.checkbox(&mut state.show_collision, "Collision");
                ui.checkbox(&mut state.show_paths, "Paths");
            });
        });
    });
    action
}

fn draw_tools_panel(ui: &mut egui::Ui, state: &mut EditorState) {
    egui::Panel::left("editor_tools")
        .default_size(100.0)
        .resizable(false)
        .show(ui, |ui| {
            ui.heading("Tools");
            ui.separator();

            let tools = [
                (EditorTool::Select, "Select"),
                (EditorTool::PaintTile, "Paint"),
                (EditorTool::Erase, "Erase"),
                (EditorTool::Fill, "Fill"),
                (EditorTool::PlaceEntity, "Entity"),
                (EditorTool::PathEdit, "Path"),
            ];

            for (tool, label) in &tools {
                let selected = state.tool == *tool;
                if ui.selectable_label(selected, *label).clicked() {
                    state.tool = *tool;
                }
            }

            ui.separator();
            ui.label("Shortcuts:");
            ui.small("S=Select P=Paint");
            ui.small("E=Erase F=Fill");
            ui.small("N=Entity G=Grid");
            ui.small("[ ] tile, Tab layer");
            ui.small("Ctrl+Z/Y/S");
            ui.small("F9 closes");
        });
}

fn draw_properties_panel(ui: &mut egui::Ui, state: &mut EditorState, level: &AmigoLevel) {
    egui::Panel::right("editor_properties")
        .default_size(160.0)
        .resizable(true)
        .show(ui, |ui| {
            ui.heading("Properties");
            ui.separator();

            // Level info
            egui::Grid::new("level_info").show(ui, |ui| {
                ui.label("Size:");
                ui.label(format!("{}x{}", level.width, level.height));
                ui.end_row();

                ui.label("Tile:");
                ui.label(format!("{}px", level.tile_size));
                ui.end_row();

                ui.label("Layers:");
                ui.label(format!("{}", level.layers.len()));
                ui.end_row();

                ui.label("Entities:");
                ui.label(format!("{}", level.entities.len()));
                ui.end_row();

                ui.label("Paths:");
                ui.label(format!("{}", level.paths.len()));
                ui.end_row();
            });

            ui.separator();

            // Layer that painting goes to
            ui.label("Layer:");
            for (i, layer) in level.layers.iter().enumerate() {
                if ui
                    .selectable_label(state.selected_layer == i, &layer.name)
                    .clicked()
                {
                    state.selected_layer = i;
                }
            }

            if state.tool == EditorTool::PlaceEntity {
                ui.separator();
                ui.label("Entity type:");
                ui.text_edit_singleline(&mut state.selected_entity_type);
            }

            ui.separator();

            // Toggles
            ui.checkbox(&mut state.grid_visible, "Show Grid");
            ui.checkbox(&mut state.show_collision, "Show Collision");
            ui.checkbox(&mut state.show_paths, "Show Paths");

            // Tile palette when paint tool is active
            if matches!(state.tool, EditorTool::PaintTile | EditorTool::Fill) {
                ui.separator();
                ui.heading("Tile Palette");

                let tiles_per_row = 8;
                let tile_size = 16.0;
                let total_tiles = 32u16;

                egui::Grid::new("tile_palette")
                    .spacing([2.0, 2.0])
                    .show(ui, |ui| {
                        for i in 0..total_tiles {
                            let selected = state.selected_tile == i;
                            // The same colour the viewport preview uses.
                            let [r, g, b] = tile_preview_color(i);
                            let color = if i == 0 {
                                egui::Color32::from_rgb(40, 40, 40)
                            } else {
                                egui::Color32::from_rgb(r, g, b)
                            };
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(tile_size, tile_size),
                                egui::Sense::click(),
                            );
                            ui.painter().rect_filled(rect, 0.0, color);
                            if selected {
                                ui.painter().rect_stroke(
                                    rect,
                                    0.0,
                                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("{}", i),
                                egui::FontId::proportional(8.0),
                                egui::Color32::WHITE,
                            );
                            if response.clicked() {
                                state.selected_tile = i;
                            }
                            if (i + 1) % tiles_per_row == 0 {
                                ui.end_row();
                            }
                        }
                    });
            }
        });
}

fn draw_status_bar(ui: &mut egui::Ui, session: &EditorSession) {
    let state = &session.state;
    egui::Panel::bottom("editor_status").show(ui, |ui| {
        ui.horizontal(|ui| {
            let file = session
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            ui.label(format!("{file}{}", if session.dirty { " *" } else { "" }));
            ui.separator();

            let tool_name = match state.tool {
                EditorTool::Select => "Select",
                EditorTool::PaintTile => "Paint",
                EditorTool::Erase => "Erase",
                EditorTool::Fill => "Fill",
                EditorTool::PlaceEntity => "Entity",
                EditorTool::PathEdit => "Path",
            };
            ui.label(format!(
                "Tool: {} | Tile #{}",
                tool_name, state.selected_tile
            ));

            ui.separator();

            if let Some((tx, ty)) = state.cursor_tile {
                ui.label(format!("Cursor: ({}, {})", tx, ty));
                ui.separator();
            }

            ui.label(format!(
                "Undo: {} | Redo: {}",
                state.undo_stack.len(),
                state.redo_stack.len()
            ));

            if let Some(status) = &session.status {
                ui.separator();
                ui.label(status);
            }
        });
    });
}
