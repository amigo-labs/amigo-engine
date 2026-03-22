//! Reflection-based entity inspector for Editor V2.
//!
//! Uses the `Reflect` trait and `TypeRegistry` to dynamically display and edit
//! any component on a selected entity.

use crate::editor_v2::EditorContext;
use amigo_reflect::{FieldMut, TypeInfo};
use std::any::TypeId;

/// Draw the entity inspector panel. Shows all reflected components on the
/// currently selected entity with auto-generated widgets per field.
pub fn draw_entity_inspector(ui: &mut egui::Ui, ctx: &mut EditorContext<'_>) {
    let entity = match ctx.selected_entity {
        Some(e) => e,
        None => {
            ui.label("No entity selected");
            return;
        }
    };

    if !ctx.world.is_alive(entity) {
        ui.label("Selected entity no longer alive");
        ctx.selected_entity = None;
        return;
    }

    ui.label(format!("Entity: {}", entity));
    ui.separator();

    // Get component type IDs for this entity.
    let type_ids = ctx.world.component_types(entity);

    if type_ids.is_empty() {
        ui.label("(no reflected components)");
        return;
    }

    for type_id in &type_ids {
        // Look up type info from the registry.
        let type_info = match ctx.registry.get(*type_id) {
            Some(reg) => reg.info,
            None => continue, // Not registered in the reflect registry; skip.
        };

        let header_name = type_info.short_name;

        egui::CollapsingHeader::new(header_name)
            .default_open(true)
            .show(ui, |ui| {
                draw_component_fields(ui, ctx, entity, *type_id, type_info);
            });
    }
}

/// Draw editable widgets for each field of a reflected component.
fn draw_component_fields(
    ui: &mut egui::Ui,
    ctx: &mut EditorContext,
    entity: amigo_core::EntityId,
    type_id: TypeId,
    _type_info: &'static TypeInfo,
) {
    // Get mutable reflected component.
    let component = match ctx.world.get_reflected_mut(entity, type_id, ctx.registry) {
        Some(c) => c,
        None => {
            ui.label("(unavailable)");
            return;
        }
    };

    let fields = component.fields_mut();

    for field in fields {
        draw_field_widget(ui, field, &mut ctx.undo_stack);
    }
}

/// Draw a single field widget based on its type.
fn draw_field_widget(
    ui: &mut egui::Ui,
    field: FieldMut<'_>,
    undo_stack: &mut crate::editor_v2::UndoStack,
) {
    let label = field.info.attrs.label.unwrap_or(field.info.name);
    let read_only = field.info.attrs.read_only;

    ui.horizontal(|ui| {
        ui.label(label);

        if read_only {
            // Show read-only representation.
            draw_field_read_only(ui, &field);
            return;
        }

        let type_id = field.info.type_id;

        if type_id == TypeId::of::<i32>() {
            if let Some(val) = field.value.downcast_mut::<i32>() {
                let old = *val;
                let mut drag = egui::DragValue::new(val);
                if let Some((lo, hi)) = field.info.attrs.range {
                    drag = drag.range(lo as i32..=hi as i32);
                }
                let response = ui.add(drag);
                if response.changed() {
                    undo_stack.record_change(label, old, *val);
                }
            }
        } else if type_id == TypeId::of::<f32>() {
            if let Some(val) = field.value.downcast_mut::<f32>() {
                let old = *val;
                let mut drag = egui::DragValue::new(val).speed(0.1);
                if let Some((lo, hi)) = field.info.attrs.range {
                    drag = drag.range(lo as f32..=hi as f32);
                }
                let response = ui.add(drag);
                if response.changed() {
                    undo_stack.record_change(label, old, *val);
                }
            }
        } else if type_id == TypeId::of::<f64>() {
            if let Some(val) = field.value.downcast_mut::<f64>() {
                let old = *val;
                let mut drag = egui::DragValue::new(val).speed(0.1);
                if let Some((lo, hi)) = field.info.attrs.range {
                    drag = drag.range(lo..=hi);
                }
                let response = ui.add(drag);
                if response.changed() {
                    undo_stack.record_change(label, old, *val);
                }
            }
        } else if type_id == TypeId::of::<bool>() {
            if let Some(val) = field.value.downcast_mut::<bool>() {
                let old = *val;
                if ui.checkbox(val, "").changed() {
                    undo_stack.record_change(label, old, *val);
                }
            }
        } else if type_id == TypeId::of::<String>() {
            if let Some(val) = field.value.downcast_mut::<String>() {
                let old = val.clone();
                if ui.text_edit_singleline(val).changed() {
                    undo_stack.record_change(label, old, val.clone());
                }
            }
        } else if type_id == TypeId::of::<u32>() {
            if let Some(val) = field.value.downcast_mut::<u32>() {
                let old = *val;
                let mut as_i64 = *val as i64;
                let drag = egui::DragValue::new(&mut as_i64).range(0..=u32::MAX as i64);
                if ui.add(drag).changed() {
                    *val = as_i64 as u32;
                    undo_stack.record_change(label, old, *val);
                }
            }
        } else {
            // Unknown type: show read-only debug.
            draw_field_read_only(ui, &field);
        }
    });
}

/// Display a read-only representation of a field value.
fn draw_field_read_only(ui: &mut egui::Ui, field: &FieldMut<'_>) {
    let type_id = field.info.type_id;
    if type_id == TypeId::of::<i32>() {
        if let Some(val) = field.value.downcast_ref::<i32>() {
            ui.label(format!("{}", val));
            return;
        }
    } else if type_id == TypeId::of::<f32>() {
        if let Some(val) = field.value.downcast_ref::<f32>() {
            ui.label(format!("{:.3}", val));
            return;
        }
    } else if type_id == TypeId::of::<bool>() {
        if let Some(val) = field.value.downcast_ref::<bool>() {
            ui.label(format!("{}", val));
            return;
        }
    } else if type_id == TypeId::of::<String>() {
        if let Some(val) = field.value.downcast_ref::<String>() {
            ui.label(val.as_str());
            return;
        }
    }
    ui.label(format!("({}: ?)", field.info.type_name));
}
