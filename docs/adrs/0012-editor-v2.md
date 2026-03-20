---
number: "0012"
title: Editor V2 — Reflection-Based Inspector and Live Editing
status: proposed
date: 2026-03-20
---

# ADR-0012: Editor V2 — Reflection-Based Inspector and Live Editing

## Status

proposed

## Context

The current editor lives in `crates/amigo_editor/` and is built on egui (gated behind `#[cfg(feature = "egui")]` in `lib.rs`, line 17). The egui integration is set up in `crates/amigo_engine/src/engine.rs` (lines 503-508 for `EguiRenderer` creation, lines 819-868 for the render pass), with `EditorState` and `AmigoLevel` stored directly on `EngineState` (lines 388-390).

The editor today is a **level editor** -- it manipulates tile layers, entity placements, and paths in an `AmigoLevel` struct (`lib.rs`, lines 255-264). The `draw_properties_panel` function in `egui_ui.rs` (line 89) only shows static level metadata (size, tile count, layer count) and has no ability to inspect or edit ECS component data on individual entities at runtime.

Undo/redo is implemented via `EditorCommand` variants (`lib.rs`, lines 28-53) that are specific to level operations (PaintTile, PlaceEntity, RemoveEntity, MovePath). Each command stores old/new values and provides an `inverse()` method (line 57). There is no generic undo mechanism that works with arbitrary component mutations.

Meanwhile, the `RewindBuffer` in `crates/amigo_core/src/state_rewind.rs` provides frame-by-frame game state recording with delta compression (`CompressionMode::Delta`, line 19, keyframe interval 30). It stores full serialized snapshots and byte-level diffs, supporting `step_back()` / `step_forward()` / `rewind_to(tick)`. The `RewindController` (line 449) wraps this with play/rewind/pause modes. This infrastructure already exists for gameplay rewind but is not connected to the editor.

The egui render path in `engine.rs` (lines 819-855) renders editor panels on top of the game frame via `EguiRenderer::render()`. The editor panels (`draw_editor_panels` in `egui_ui.rs`, line 7) receive `&mut EditorState` and `&AmigoLevel` but have no access to `World` or the `TypeRegistry` (AP-11).

## Decision

Introduce **Editor V2** behind the `editor_v2` feature flag (implies `editor` + `reflect`). The core changes are:

1. **Reflection-based auto-inspector**: Replace the static `draw_properties_panel` with a dynamic inspector that uses `TypeRegistry` and `Reflect` (AP-11) to enumerate components on a selected entity and render egui widgets per field. Each reflected `FieldInfo` maps to a widget: `i32`/`f32` -> `DragValue` (with range from `FieldAttrs::range`), `bool` -> checkbox, `String` -> text edit, `Color` -> color picker, `SimVec2` -> two drag values. Unknown types show a read-only debug representation.

2. **Entity selection and live editing**: Add an entity picker (click in viewport -> raycast to entity via position + sprite bounds). The selected entity's components appear in the inspector panel. Editing a field immediately writes to the `World` via `get_reflected_mut()`.

3. **Undo/redo via RewindBuffer**: Instead of extending `EditorCommand` per component type, leverage the existing `RewindBuffer<WorldSnapshot>` where `WorldSnapshot` is a serialized capture of all component data. Before each editor mutation, `record()` a snapshot. Undo calls `step_back()`, redo calls `step_forward()`. The `RewindBuffer` already handles ring-buffer eviction (capacity-limited), delta compression, and timeline forking (`truncate_after_current()` in `state_rewind.rs`, line 335). The level-editing `EditorCommand` undo stack remains for tile/path operations; the RewindBuffer handles component mutations.

4. **Editor context expansion**: `draw_editor_panels` gains access to `&mut World` and `&TypeRegistry` via a new `EditorContext` struct passed from the engine loop, replacing the current `(&mut EditorState, &AmigoLevel)` pair.

### Alternatives Considered

1. **Per-field undo commands**: Generate an `EditorCommand::SetField { entity, component_type, field_name, old_value, new_value }` for each edit, using `ReflectPatch` to store type-erased before/after values. Rejected because: (a) it requires serializing arbitrary `Box<dyn Any>` values for the undo stack, (b) compound operations (e.g., dragging an entity changes Position.x and Position.y) would need command grouping, and (c) the `RewindBuffer` already solves this more simply at the whole-state level with delta compression keeping memory manageable.

2. **Separate editor world**: Run the editor in its own `World` that mirrors the game world, so edits do not affect the running simulation until "committed". Rejected because it doubles memory usage and creates synchronization complexity. Instead, editor edits apply directly to the live world, and the RewindBuffer provides the safety net.

## Migration Path

1. **Create `EditorContext` and thread it through the render path** -- Define `pub struct EditorContext<'a> { pub state: &'a mut EditorState, pub level: &'a AmigoLevel, pub world: &'a mut World, pub registry: &'a TypeRegistry, pub rewind: &'a mut RewindBuffer<Vec<u8>> }` in `crates/amigo_editor/src/lib.rs`. Update `draw_editor_panels` in `egui_ui.rs` to accept `&mut EditorContext` instead of `(&mut EditorState, &AmigoLevel)`. In `crates/amigo_engine/src/engine.rs`, add `TypeRegistry` and `RewindBuffer<Vec<u8>>` to `EngineState` (alongside `editor_state` at line 388), construct `EditorContext` before calling `draw_editor_panels`, and pass it through the egui closure (lines 845-851). Verify: `cargo build --features editor_v2` compiles; existing editor panels render unchanged.

2. **Implement the auto-inspector panel** -- Add `fn draw_entity_inspector(ui: &mut egui::Ui, ctx: &mut EditorContext)` in a new file `crates/amigo_editor/src/inspector.rs`. For the selected entity (stored in `EditorState`), call `ctx.world.component_types(entity)` to get all component `TypeId`s, look each up in `ctx.registry`, then for each component call `ctx.world.get_reflected_mut(entity, type_id, ctx.registry)` and render widgets per `FieldInfo`. Use `egui::CollapsingHeader` per component. Wire this into `draw_properties_panel` as a new collapsible section below the existing level info. Verify: select an entity with `Health { current: 80, max: 100 }` -- inspector shows two drag-value fields labeled "current" (editable, range 0..1000) and "max" (read-only). Editing "current" to 50 is immediately visible in-game.

3. (rough) Add entity selection via viewport click. On left-click when Select tool is active, iterate `join(&world.positions, &world.sprites)` and hit-test against sprite bounds. Store selected `EntityId` in `EditorState`.

4. (rough) Integrate `RewindBuffer` for undo. Before each field mutation in the inspector, call `rewind.record(&snapshot)`. Wire Ctrl+Z to `rewind.step_back()` and apply the reconstructed snapshot back to the World. Wire Ctrl+Shift+Z to `step_forward()`.

5. (rough) Add a timeline scrubber widget at the bottom of the editor (horizontal slider over `rewind.oldest_tick()..=rewind.newest_tick()`), allowing the user to drag to any recorded state.

6. (rough) Performance profiling: measure egui render time for an inspector with 10 components x 8 fields (80 widgets). If exceeding the abort threshold, add field batching or lazy rendering.

## Abort Criteria

- If rendering the reflection-based inspector for a single entity with 10 components (each having up to 8 fields) causes the egui pass to exceed 2ms per frame (preventing 60fps at 16.6ms budget with rendering taking ~12ms), abandon the auto-inspector approach and revert to hand-coded component panels.
- If the `RewindBuffer` undo approach requires more than 64 MB of memory for 300 undo steps of a world with 1,000 entities, switch to per-field `EditorCommand` undo instead.

## Consequences

### Positive
- Adding a new component type with `#[derive(Reflect)]` automatically makes it editable in the inspector -- zero editor code needed per component.
- Undo/redo for component edits reuses the battle-tested `RewindBuffer` with delta compression, rather than requiring new command types per component.
- Live editing with immediate feedback enables rapid iteration during development.

### Negative / Trade-offs
- The `editor_v2` feature flag pulls in `amigo_reflect` and its proc-macro crate, increasing compile time for editor builds.
- `RewindBuffer`-based undo records the entire world state per edit, which is heavier than per-field undo for single-field changes. Delta compression mitigates this but does not eliminate it.
- `draw_editor_panels` now borrows `&mut World` during the egui pass, which means game systems cannot run concurrently with editor rendering. This is already the case (editor renders after simulation in the frame), but the borrow scope is larger.
- The existing level-editor undo (`EditorCommand` stack) and the new component-editor undo (`RewindBuffer`) are two separate undo histories, which could confuse users. A future unification pass may be needed.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
