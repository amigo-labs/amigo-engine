use crate::session::tile_preview_color;
use crate::{
    AmigoLevel, EditorAction, EditorCommand, EditorSession, EditorState, EditorTool,
    EntityPlacement, ZoneDef,
};

/// Draw the complete editor UI using egui.
///
/// Call this from within the engine's egui render closure with the root
/// [`egui::Ui`] (egui 0.36 lays panels out inside a `Ui`, not a `Context`).
/// Edits act on the session's level directly, through its undo history; the
/// file actions are returned for the engine to run, since saving also tells
/// the game.
pub fn draw_editor_panels(ui: &mut egui::Ui, session: &mut EditorSession) -> Option<EditorAction> {
    let mut action = draw_menu_bar(ui, session);
    draw_tools_panel(ui, &mut session.state);
    draw_properties_panel(ui, session, &mut action);
    draw_status_bar(ui, session);
    let ctx = ui.ctx().clone();
    action = action.or_else(|| draw_dialogs(&ctx, session));
    import_dropped_files(&ctx, session);
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
                if ui.button("Open…").clicked() {
                    session.ui.open_dialog = true;
                    ui.close();
                }
                if ui.button("Save (Ctrl+S)").clicked() {
                    action = Some(EditorAction::Save);
                    ui.close();
                }
                if ui.button("Save As…").clicked() {
                    let name = session
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    session.ui.save_as = Some(name);
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
                if ui.button("Delete Selection (Del)").clicked() {
                    session.delete_selection();
                    ui.close();
                }
            });

            ui.menu_button("Tools", |ui| {
                if ui.button("Auto-decorate…").clicked() {
                    let world = session
                        .level
                        .metadata
                        .get("world")
                        .cloned()
                        .unwrap_or_default();
                    let tiles = session
                        .decor_tiles(&world)
                        .iter()
                        .map(u16::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    session.ui.decorate = Some((tiles, 0.1));
                    ui.close();
                }
                let picked: Vec<&EntityPlacement> = session
                    .state
                    .selected_entities
                    .iter()
                    .filter_map(|&i| session.level.entities.get(i))
                    .collect();
                let ts = session.level.tile_size as f32 / 2.0;
                let ends = (picked.len() == 2).then(|| {
                    (
                        (picked[0].x + ts, picked[0].y + ts),
                        (picked[1].x + ts, picked[1].y + ts),
                    )
                });
                if ui
                    .add_enabled(
                        ends.is_some(),
                        egui::Button::new("Path between the two selected entities"),
                    )
                    .clicked()
                    && let Some((from, to)) = ends
                {
                    let layer = session.level.layer_index("collision").unwrap_or(0);
                    if let Err(e) = session.auto_path(from, to, layer) {
                        session.status = Some(e);
                    }
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

            for (tool, label) in TOOLS {
                let selected = state.tool == tool;
                if ui.selectable_label(selected, label).clicked() {
                    state.tool = tool;
                }
            }

            ui.separator();
            ui.label("Shortcuts:");
            ui.small("S=Select P=Paint");
            ui.small("E=Erase F=Fill");
            ui.small("N=Entity R=Path");
            ui.small("Z=Zone G=Grid");
            ui.small("[ ] tile, Tab layer");
            ui.small("Shift+click adds");
            ui.small("Del deletes, Esc");
            ui.small("Ctrl+Z/Y/S");
            ui.small("F9 closes");
        });
}

const TOOLS: [(EditorTool, &str); 7] = [
    (EditorTool::Select, "Select"),
    (EditorTool::PaintTile, "Paint"),
    (EditorTool::Erase, "Erase"),
    (EditorTool::Fill, "Fill"),
    (EditorTool::PlaceEntity, "Entity"),
    (EditorTool::PathEdit, "Path"),
    (EditorTool::Zone, "Zone"),
];

fn tool_name(tool: EditorTool) -> &'static str {
    TOOLS
        .iter()
        .find(|(t, _)| *t == tool)
        .map_or("?", |(_, name)| name)
}

fn draw_properties_panel(
    ui: &mut egui::Ui,
    session: &mut EditorSession,
    action: &mut Option<EditorAction>,
) {
    egui::Panel::right("editor_properties")
        .default_size(200.0)
        .resizable(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Properties");
                ui.separator();
                level_info(ui, &session.level);
                ui.separator();
                tileset_field(ui, session);
                ui.separator();

                // Layer that painting goes to
                ui.label("Layer:");
                let mut picked = None;
                for (i, layer) in session.level.layers.iter().enumerate() {
                    if ui
                        .selectable_label(session.state.selected_layer == i, &layer.name)
                        .clicked()
                    {
                        picked = Some(i);
                    }
                }
                if let Some(i) = picked {
                    session.state.selected_layer = i;
                }

                if session.state.tool == EditorTool::PlaceEntity {
                    ui.separator();
                    ui.label("Entity type:");
                    ui.text_edit_singleline(&mut session.state.selected_entity_type);
                    ui.horizontal(|ui| {
                        for preset in ["spawn", "goal", "light", "emitter"] {
                            if ui.small_button(preset).clicked() {
                                session.state.selected_entity_type = preset.to_string();
                            }
                        }
                    });
                }

                entity_inspector(ui, session, action);
                zone_inspector(ui, session);
                path_inspector(ui, session);
                playtest_panel(ui, session);

                // Tile palette when paint tool is active
                if matches!(session.state.tool, EditorTool::PaintTile | EditorTool::Fill) {
                    ui.separator();
                    tile_palette(ui, &mut session.state);
                }
            });
        });
}

fn level_info(ui: &mut egui::Ui, level: &AmigoLevel) {
    egui::Grid::new("level_info").show(ui, |ui| {
        for (label, value) in [
            ("Size:", format!("{}x{}", level.width, level.height)),
            ("Tile:", format!("{}px", level.tile_size)),
            ("Layers:", level.layers.len().to_string()),
            ("Entities:", level.entities.len().to_string()),
            ("Paths:", level.paths.len().to_string()),
            ("Zones:", level.zones.len().to_string()),
        ] {
            ui.label(label);
            ui.label(value);
            ui.end_row();
        }
    });
}

/// The sprite the level's tiles are drawn with (`metadata["tileset"]`).
fn tileset_field(ui: &mut egui::Ui, session: &mut EditorSession) {
    ui.label("Tileset sprite:");
    let current = session
        .level
        .metadata
        .get("tileset")
        .cloned()
        .unwrap_or_default();
    let text = session.ui.tileset.get_or_insert_with(|| current.clone());
    let response = ui.text_edit_singleline(text);
    if response.lost_focus() && *text != current {
        let value = Some(text.trim().to_string()).filter(|v| !v.is_empty());
        if let Some(cmd) = EditorCommand::set_metadata(&session.level, "tileset", value) {
            let _ = session.execute(cmd);
        }
        session.ui.tileset = None;
    } else if !response.has_focus() && *text != current {
        // The level changed underneath (undo, load): show what it has.
        session.ui.tileset = Some(current);
    }
}

/// Type, position and properties of the selected entity, or the size of a
/// multiple selection.
fn entity_inspector(
    ui: &mut egui::Ui,
    session: &mut EditorSession,
    action: &mut Option<EditorAction>,
) {
    let selected = session.state.selected_entities.clone();
    match selected.as_slice() {
        [] => {
            session.ui.entity_edit = None;
        }
        [index] => {
            let Some(entity) = session.level.entities.get(*index).cloned() else {
                return;
            };
            ui.separator();
            ui.heading(format!("Entity #{index}"));
            let edit = match &mut session.ui.entity_edit {
                Some((i, e)) if *i == *index => e,
                slot => {
                    *slot = Some((*index, entity.clone()));
                    match slot {
                        Some((_, e)) => e,
                        None => return,
                    }
                }
            };
            egui::Grid::new("entity_fields").show(ui, |ui| {
                ui.label("Type:");
                ui.text_edit_singleline(&mut edit.entity_type);
                ui.end_row();
                ui.label("X:");
                ui.add(egui::DragValue::new(&mut edit.x).speed(1.0));
                ui.end_row();
                ui.label("Y:");
                ui.add(egui::DragValue::new(&mut edit.y).speed(1.0));
                ui.end_row();
            });
            properties_editor(
                ui,
                "entity_props",
                &mut edit.properties,
                &mut session.ui.new_property,
            );
            let edited = edit.clone();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(edited != entity, egui::Button::new("Apply"))
                    .clicked()
                    && let Some(cmd) =
                        EditorCommand::set_entity(&session.level, *index, edited.clone())
                {
                    let _ = session.execute(cmd);
                }
                if ui.button("Delete").clicked() {
                    session.delete_selection();
                }
                if entity.entity_type == "emitter" && ui.button("Save .emitter.ron").clicked() {
                    *action = Some(EditorAction::SaveEmitter(*index));
                }
            });
        }
        many => {
            ui.separator();
            ui.heading(format!("{} entities", many.len()));
            if ui.button("Delete all").clicked() {
                session.delete_selection();
            }
        }
    }
}

fn zone_inspector(ui: &mut egui::Ui, session: &mut EditorSession) {
    let Some(index) = session.state.selected_zone else {
        session.ui.zone_edit = None;
        return;
    };
    let Some(zone) = session.level.zones.get(index).cloned() else {
        session.state.selected_zone = None;
        return;
    };
    ui.separator();
    ui.heading(format!("Zone #{index}"));
    let edit: &mut ZoneDef = match &mut session.ui.zone_edit {
        Some((i, z)) if *i == index => z,
        slot => {
            *slot = Some((index, zone.clone()));
            match slot {
                Some((_, z)) => z,
                None => return,
            }
        }
    };
    egui::Grid::new("zone_fields").show(ui, |ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut edit.name);
        ui.end_row();
        for (label, value) in [
            ("X:", &mut edit.x),
            ("Y:", &mut edit.y),
            ("W:", &mut edit.w),
            ("H:", &mut edit.h),
        ] {
            ui.label(label);
            ui.add(egui::DragValue::new(value).speed(1.0));
            ui.end_row();
        }
    });
    properties_editor(
        ui,
        "zone_props",
        &mut edit.properties,
        &mut session.ui.new_property,
    );
    let edited = edit.clone();
    ui.horizontal(|ui| {
        let valid = edited.w > 0.0 && edited.h > 0.0;
        if ui
            .add_enabled(valid && edited != zone, egui::Button::new("Apply"))
            .clicked()
            && let Some(cmd) = EditorCommand::set_zone(&session.level, index, edited.clone())
        {
            let _ = session.execute(cmd);
        }
        if ui.button("Delete").clicked() {
            session.state.selected_entities.clear();
            session.delete_selection();
        }
    });
}

fn path_inspector(ui: &mut egui::Ui, session: &mut EditorSession) {
    let Some(index) = session.state.selected_path else {
        return;
    };
    let Some(path) = session.level.paths.get(index).cloned() else {
        session.state.selected_path = None;
        return;
    };
    ui.separator();
    ui.heading(format!("Path '{}'", path.name));
    ui.label(format!("{} point(s)", path.points.len()));
    let mut closed = path.closed;
    if ui.checkbox(&mut closed, "Closed loop").changed() {
        let mut new = path.clone();
        new.closed = closed;
        let _ = session.execute(EditorCommand::Batch(vec![
            EditorCommand::RemovePath {
                index,
                path: path.clone(),
            },
            EditorCommand::AddPath { index, path: new },
        ]));
    }
    ui.horizontal(|ui| {
        if ui.button("Deselect").clicked() {
            session.state.selected_path = None;
            session.state.selected_point = None;
        }
        if ui.button("Delete path").clicked() {
            session.state.selected_path = None;
            session.state.selected_point = None;
            let _ = session.execute(EditorCommand::RemovePath { index, path });
        }
    });
}

/// Key/value properties with a row to add one and a button per row to
/// remove it.
fn properties_editor(
    ui: &mut egui::Ui,
    id: &str,
    properties: &mut std::collections::HashMap<String, String>,
    new_key: &mut String,
) {
    ui.label("Properties:");
    let mut keys: Vec<String> = properties.keys().cloned().collect();
    keys.sort();
    let mut remove = None;
    egui::Grid::new(id).show(ui, |ui| {
        for key in &keys {
            ui.label(key);
            if let Some(value) = properties.get_mut(key) {
                ui.text_edit_singleline(value);
            }
            if ui.small_button("x").clicked() {
                remove = Some(key.clone());
            }
            ui.end_row();
        }
    });
    if let Some(key) = remove {
        properties.remove(&key);
    }
    ui.horizontal(|ui| {
        ui.text_edit_singleline(new_key);
        let key = new_key.trim().to_string();
        if ui
            .add_enabled(
                !key.is_empty() && !properties.contains_key(&key),
                egui::Button::new("Add"),
            )
            .clicked()
        {
            properties.insert(key, String::new());
            new_key.clear();
        }
    });
}

/// Reported playtests: difficulty, balance and wave-curve suggestions, and
/// the recorded heatmaps.
fn playtest_panel(ui: &mut egui::Ui, session: &mut EditorSession) {
    if session.playtest.runs.is_empty() && session.heatmaps.names().is_empty() {
        return;
    }
    ui.separator();
    ui.heading("Playtests");
    let results = &session.playtest;
    if !results.runs.is_empty() {
        ui.label(format!(
            "{} run(s), win rate {:.0}%, kill ratio {:.0}%",
            results.runs.len(),
            results.win_rate() * 100.0,
            results.avg_kill_ratio() * 100.0
        ));
        ui.label(format!("Difficulty: {:?}", results.difficulty_assessment()));
        for suggestion in session.balance_suggestions() {
            ui.label(format!(
                "• [{:?}] {}",
                suggestion.severity, suggestion.description
            ));
        }
    }
    let names: Vec<String> = session
        .heatmaps
        .names()
        .iter()
        .map(|n| n.to_string())
        .collect();
    if !names.is_empty() {
        ui.label("Heatmap:");
        if ui
            .selectable_label(session.shown_heatmap.is_none(), "none")
            .clicked()
        {
            session.shown_heatmap = None;
        }
        for name in names {
            let shown = session.shown_heatmap.as_deref() == Some(name.as_str());
            if ui.selectable_label(shown, &name).clicked() {
                session.shown_heatmap = Some(name);
            }
        }
    }
}

fn tile_palette(ui: &mut egui::Ui, state: &mut EditorState) {
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
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(tile_size, tile_size), egui::Sense::click());
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

/// The Open, Save As and Auto-decorate windows.
fn draw_dialogs(ctx: &egui::Context, session: &mut EditorSession) -> Option<EditorAction> {
    let mut action = None;
    if session.ui.open_dialog {
        let mut open = true;
        egui::Window::new("Open Level")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                let files = session.level_files();
                if files.is_empty() {
                    ui.label(format!(
                        "No .amigo files in {}",
                        session.levels_dir.display()
                    ));
                }
                for file in files {
                    let name = file
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let current = file == session.path;
                    if ui.selectable_label(current, name).clicked() {
                        action = Some(EditorAction::Open(file));
                    }
                }
            });
        session.ui.open_dialog = open && action.is_none();
    }

    if let Some(name) = &mut session.ui.save_as {
        let mut open = true;
        let mut save = false;
        egui::Window::new("Save Level As")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(format!("In {}:", session.levels_dir.display()));
                let response = ui.text_edit_singleline(name);
                let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Save").clicked() || enter {
                    save = true;
                }
            });
        if save && !name.trim().is_empty() {
            action = Some(EditorAction::SaveAs(name.trim().into()));
            session.ui.save_as = None;
        } else if !open {
            session.ui.save_as = None;
        }
    }

    if let Some((tiles, density)) = &mut session.ui.decorate {
        let mut open = true;
        let mut run = false;
        egui::Window::new("Auto-decorate")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Decoration tile ids (comma separated):");
                ui.text_edit_singleline(tiles);
                ui.add(egui::Slider::new(density, 0.0..=1.0).text("density"));
                ui.small(
                    "Fills empty cells of the 'decoration' layer where the first layer has ground.",
                );
                run = ui.button("Decorate").clicked();
            });
        if run {
            let ids: Vec<u16> = tiles
                .split(',')
                .filter_map(|t| t.trim().parse().ok())
                .collect();
            let density = *density;
            let seed = session.level.name.bytes().map(u64::from).sum();
            if let Err(e) = session.auto_decorate("decoration", &ids, density, seed) {
                session.status = Some(e);
            }
            session.ui.decorate = None;
        } else if !open {
            session.ui.decorate = None;
        }
    }
    action
}

/// Sprite import: image and Aseprite files dropped on the window are copied
/// into `assets/sprites/`, where hot reload picks them up.
fn import_dropped_files(ctx: &egui::Context, session: &mut EditorSession) {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    for file in dropped {
        match session.import_sprite(file.path()) {
            Ok(to) => session.status = Some(format!("Imported {}", to.display())),
            Err(e) => session.status = Some(e),
        }
    }
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

            ui.label(format!(
                "Tool: {} | Tile #{}",
                tool_name(state.tool),
                state.selected_tile
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
