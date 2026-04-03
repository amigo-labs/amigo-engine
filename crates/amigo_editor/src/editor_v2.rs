//! Editor V2: Reflection-based inspector with entity selection and undo/redo.
//!
//! Gated behind the `editor_v2` feature flag.
//! See ADR-0010 for design rationale.

use amigo_core::EntityId;
use amigo_core::World;
use amigo_reflect::TypeRegistry;
use std::any::Any;

use crate::inspector;
use crate::{AmigoLevel, EditorState};

// ---------------------------------------------------------------------------
// Undo / Redo (simple Vec-based command history)
// ---------------------------------------------------------------------------

/// A single recorded field change for undo/redo.
struct FieldChange {
    /// Human-readable label (field name).
    label: String,
    /// The previous value (type-erased).
    _old_value: Box<dyn Any>,
    /// The new value (type-erased).
    _new_value: Box<dyn Any>,
}

/// Simple Vec-based undo/redo history for component field edits.
pub struct UndoStack {
    undo: Vec<FieldChange>,
    redo: Vec<FieldChange>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Record a field change. Clears the redo stack (timeline diverged).
    pub fn record_change<T: Any + Clone + 'static>(
        &mut self,
        label: &str,
        old_value: T,
        new_value: T,
    ) {
        self.redo.clear();
        self.undo.push(FieldChange {
            label: label.to_owned(),
            _old_value: Box::new(old_value),
            _new_value: Box::new(new_value),
        });
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Pop the most recent change from the undo stack and push to redo.
    /// Returns the label of the undone change.
    pub fn undo(&mut self) -> Option<String> {
        let change = self.undo.pop()?;
        let label = change.label.clone();
        self.redo.push(change);
        Some(label)
    }

    /// Pop from the redo stack and push back to undo.
    /// Returns the label of the redone change.
    pub fn redo(&mut self) -> Option<String> {
        let change = self.redo.pop()?;
        let label = change.label.clone();
        self.undo.push(change);
        Some(label)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// EditorContext — bundles all state needed by editor V2 panels
// ---------------------------------------------------------------------------

/// Context passed to editor V2 panels, providing access to the World,
/// TypeRegistry, selection state, and undo history.
pub struct EditorContext<'a> {
    pub state: &'a mut EditorState,
    pub level: &'a AmigoLevel,
    pub world: &'a mut World,
    pub registry: &'a TypeRegistry,
    pub selected_entity: Option<EntityId>,
    pub undo_stack: UndoStack,
}

impl<'a> EditorContext<'a> {
    pub fn new(
        state: &'a mut EditorState,
        level: &'a AmigoLevel,
        world: &'a mut World,
        registry: &'a TypeRegistry,
    ) -> Self {
        Self {
            state,
            level,
            world,
            registry,
            selected_entity: None,
            undo_stack: UndoStack::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Editor V2 panels
// ---------------------------------------------------------------------------

/// Draw the complete editor V2 UI. Drop-in replacement for `draw_editor_panels`
/// when the `editor_v2` feature is active.
pub fn draw_editor_v2_panels(egui_ctx: &egui::Context, ctx: &mut EditorContext) {
    draw_entity_list_panel(egui_ctx, ctx);
    draw_inspector_panel(egui_ctx, ctx);
    draw_v2_status_bar(egui_ctx, ctx);
}

/// Left panel: entity list for selection.
fn draw_entity_list_panel(egui_ctx: &egui::Context, ctx: &mut EditorContext) {
    egui::SidePanel::left("editor_v2_entity_list")
        .default_width(160.0)
        .resizable(true)
        .show(egui_ctx, |ui| {
            ui.heading("Entities");
            ui.separator();

            // Collect entity IDs first to avoid borrow issues.
            let entities: Vec<EntityId> = ctx.world.iter_entities().collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for entity in &entities {
                    let is_selected = ctx.selected_entity == Some(*entity);
                    let label = format!("{}", entity);
                    if ui.selectable_label(is_selected, &label).clicked() {
                        ctx.selected_entity = if is_selected { None } else { Some(*entity) };
                    }
                }
            });

            ui.separator();
            ui.label(format!("{} entities", entities.len()));
        });
}

/// Right panel: reflection-based inspector for the selected entity.
fn draw_inspector_panel(egui_ctx: &egui::Context, ctx: &mut EditorContext) {
    egui::SidePanel::right("editor_v2_inspector")
        .default_width(220.0)
        .resizable(true)
        .show(egui_ctx, |ui| {
            ui.heading("Inspector");
            ui.separator();

            inspector::draw_entity_inspector(ui, ctx);

            // Undo/Redo buttons
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(ctx.undo_stack.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    ctx.undo_stack.undo();
                }
                if ui
                    .add_enabled(ctx.undo_stack.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    ctx.undo_stack.redo();
                }
            });
        });
}

/// Bottom status bar for editor V2.
fn draw_v2_status_bar(egui_ctx: &egui::Context, ctx: &EditorContext) {
    egui::TopBottomPanel::bottom("editor_v2_status").show(egui_ctx, |ui| {
        ui.horizontal(|ui| {
            if let Some(entity) = ctx.selected_entity {
                ui.label(format!("Selected: {}", entity));
            } else {
                ui.label("No selection");
            }

            ui.separator();

            ui.label(format!(
                "Undo: {} | Redo: {}",
                ctx.undo_stack.undo_len(),
                ctx.undo_stack.redo_len()
            ));
        });
    });
}
