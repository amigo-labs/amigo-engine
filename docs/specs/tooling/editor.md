---
status: partial
crate: amigo_editor
depends_on: ["engine/core", "engine/ui"]
last_updated: 2026-10-05
---

# Integrated Level Editor

## Purpose

In-engine level editor, enabled via the `editor` feature flag, with zero overhead
in release builds since neither the editor nor egui is linked without it.

## Public API

The editor is wired by the `editor` feature rather than registered as a plugin:
the engine constructs `amigo_editor::EditorState` and an egui renderer in
`EngineApp::resumed` when the flag is on. There is no `EditorPlugin` type, and no
`Tab` toggle is bound — earlier revisions of this spec described both.

```sh
# Run a game with the editor overlay
amigo editor
# equivalently
cargo run --features amigo_engine/editor
```

**F9** opens and closes the editor (F1–F8 belong to the debug overlay). While it
is open, keys and mouse buttons go to the editor instead of the game; the
simulation keeps running.

The engine keeps one `amigo_editor::EditorSession` in `ctx.resources`: the level
being edited, its file, the undo history and the active tool. On startup it opens
the first `assets/levels/*.amigo` by name, or starts `level_01.amigo` when there
is none. The same session serves the window, the headless loop and the
`editor.*` API commands.

The level format is `amigo_core::level::AmigoLevel` (re-exported by
`amigo_editor`). Games read it with `ctx.load_level("level_01")`, which returns
an `amigo_core::level_loader::LoadedLevel`, and react to edits with
`ctx.level_reloaded("level_01")`: saving in the editor, over the API, or
changing the file on disk with hot reload on emits `amigo_engine::LevelReloaded`.
The platformer and free-roam templates do both.

### What works (2026-10)

| Feature | How |
|---|---|
| Paint, erase, flood fill | Click or drag in the viewport; one stroke is one undo step |
| Layer select | Properties panel or Tab |
| Place and remove entities | Entity tool (N) with a type name; Erase on an entity removes it |
| Undo / redo | Edit menu, Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z, `editor.undo/redo` |
| Save, revert, new level | File menu, Ctrl+S, `editor.save/load/new_level`; saves are atomic |
| Grid, cursor, tile preview | Drawn over the game; tiles show as one colour per id |
| `editor.*` API | All commands below except `auto_decorate`; `engine.get_property {"key": "editor"}` reports the result |

### Not implemented yet

- Drawing the level with its tileset: tiles are previewed as coloured squares,
  since a level does not name its tileset.
- The path tool in the viewport (paths can be added and moved over the API only).
- Zones (kept when saving, not editable), entity properties, multi-select.
- A file dialog: Save writes the open file, New picks the next `level_NN.amigo`.
- `editor.auto_decorate`, wave balancing, AI playtesting (Phase 3 below).
- `EditorRuntime` (`plugin.rs`) is a separate game-side helper with its own
  level and F5–F7 keys, which clash with the debug overlay; the engine does not
  use it.

### Editor UI Widgets (Tier 2, behind `editor` feature flag)

Builds on Tier 1 Game HUD. Added: text input, sliders, dropdowns, color pickers, scrollable containers, tree views. Editor look is consistent with the game's pixel art aesthetic -- not a generic desktop UI.

```rust
#[cfg(feature = "editor")]
{
    ui.text_input("tower_name", &mut name);
    ui.slider("range", &mut range, 1.0..=20.0);
    ui.dropdown("type", &mut tower_type, &["Archer", "Mage", "Cannon"]);
    ui.color_picker("tint", &mut color);
    ui.scrollable_list("entities", &entity_list, |item, ui| { ... });
    ui.tree_view("hierarchy", &tree, |node, ui| { ... });
}
```

## Behavior

### Phase 1: Core Features

- Tile painter (paint, erase, fill, layer select)
- Entity placement + property inspector
- Path editor with visual preview
- Undo/Redo (Command Pattern)
- `.amigo` format save/load (RON-based)

### Phase 2: Live Preview

- Edit-while-playing (game simulation continues during editing)
- Changes take effect immediately
- Tower ranges, enemy paths, spawn points visualized

### Phase 2.5: Data & Visual Editors

- **Property Inspector**: Edit arbitrary component data (tower stats, enemy HP, speed, etc.) via reflection. Any `Serialize + Deserialize` component can be inspected and modified.
- **Light Placement Tool**: Place point/area lights on the tilemap, adjust radius, color, intensity. Preview light propagation in real-time.
- **Particle Emitter Editor**: Select emitter shape, configure rate, lifetime, speed, gravity, color gradient. Live preview on the tilemap. Save as `.emitter.ron`.
- **Sprite Import**: Drag-and-drop artgen output → auto-register in atlas manifest, generate `.sprite.ron`, preview in editor viewport.

### Phase 3: AI-Assisted Features

- Auto-pathing (algorithmic path generation from start/end)
- Wave balancing (difficulty curve analysis)
- Auto-decoration (themed tile filling per world)
- AI playtesting (simulation + heatmaps + balancing suggestions)

### Remote Editor Control (via AI API)

The editor can be controlled remotely through the AI Agent Interface (amigo_api). Claude Code uses MCP tools to interact with the editor:

**Editor MCP Tools:**

- `amigo_editor_new_level(world, width, height)`
- `amigo_editor_paint_tile(layer, x, y, tile)`
- `amigo_editor_fill_rect(layer, x, y, w, h, tile)`
- `amigo_editor_place_entity(type, x, y)`
- `amigo_editor_add_path(points)`
- `amigo_editor_move_path_point(path, point, new_pos)`
- `amigo_editor_auto_decorate(world)`
- `amigo_editor_save(path)` / `amigo_editor_load(path)`
- `amigo_editor_undo()` / `amigo_editor_redo()`

**JSON-RPC Protocol (underlying):**

```jsonc
// Create new level
{"method": "editor.new_level", "params": {"world": "caribbean", "width": 30, "height": 20}}

// Paint tiles
{"method": "editor.paint_tile", "params": {"layer": "terrain", "x": 5, "y": 3, "tile": 42}}

// Fill rectangle
{"method": "editor.fill_rect", "params": {"layer": "terrain", "x": 0, "y": 0, "w": 10, "h": 5, "tile": 1}}

// Place entity marker
{"method": "editor.place_entity", "params": {"type": "spawn_point", "x": 0, "y": 10}}

// Define path
{"method": "editor.add_path", "params": {"points": [[0,10], [5,10], [5,5], [15,5], [15,15], [29,15]]}}

// Modify path point
{"method": "editor.move_path_point", "params": {"path": 0, "point": 2, "new_pos": [7, 7]}}

// Auto-decorate (fill non-gameplay tiles with themed decoration)
{"method": "editor.auto_decorate", "params": {"world": "caribbean"}}

// Save level
{"method": "editor.save", "params": {"path": "levels/caribbean/level_02.amigo"}}

// Load level
{"method": "editor.load", "params": {"path": "levels/caribbean/level_01.amigo"}}

// Undo / Redo
{"method": "editor.undo"}
{"method": "editor.redo"}
```

### Example: Claude Code Building a Level

```
Claude Code calls MCP tools natively:

1. amigo_editor_new_level(world="dune", width=40, height=25)
2. amigo_editor_fill_rect(layer="terrain", x=0, y=0, w=40, h=25, tile="sand")
3. amigo_editor_add_path(points=[[0,12],[10,12],[10,5],[20,5],[20,20],[39,20]])
4. amigo_editor_auto_decorate(world="dune")
5. amigo_screenshot(path="/tmp/level_draft.png", overlays=["paths","grid"])
   -> Claude SEES the image, analyzes layout
6. "Path needs more curves"
7. amigo_editor_move_path_point(path=0, point=2, new_pos=[12,7])
8. amigo_screenshot(path="/tmp/level_v2.png")
   -> "Better. Now testing playability..."
9. amigo_editor_save(path="levels/dune/level_02.amigo")
```

## Internal Design

The editor is a Plugin with its own update/draw cycle. It renders using the same Pixel UI system as the game HUD (Tier 1), extended with Tier 2 editor widgets. The editor's command pattern (undo/redo) is separate from the game's command system.

The editor uses the `.amigo` format (RON-based) for level serialization, which includes tilemap data, entity placements, path definitions, and metadata.

## Non-Goals

- Standalone editor application (always in-engine)
- Standalone desktop application — the editor is always an overlay on a running
  game
- 3D editing

Superseded non-goal: this said "Desktop UI toolkit (uses own Pixel UI, not egui or
similar)". The editor **is** an egui application —
`crates/amigo_editor/src/{egui_ui,editor_v2,inspector}.rs` and
`crates/amigo_render/src/egui_integration.rs`, and the engine's `editor` feature
enables `amigo_editor/egui`. egui was chosen for the editor because editor widgets
(dockable panels, scrolling property grids, text fields, colour pickers) are a
solved problem there and rebuilding them in the pixel UI would trade months of work
for aesthetic consistency in a tool that never ships to players. The game-facing UI
is still the engine's own pixel UI; the split is deliberate, and `index.md`'s
"No egui" key decision is superseded to say so.
- 3D editing capabilities
- Runtime editor in release builds

## Open Questions

- Exact `.amigo` format specification
- Maximum undo history depth
- Whether Phase 3 AI features require a separate training step or work with general LLM reasoning
