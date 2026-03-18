---
status: done
crate: amigo_editor
depends_on: ["engine/core", "engine/ui"]
last_updated: 2026-03-16
---

# Integrated Level Editor

## Purpose

In-engine level editor using the engine's own Pixel UI system. Enabled via `--features editor` feature flag. Zero overhead in release builds. Toggle with `Tab` between Play and Edit mode.

## Public API

```rust
// Plugin registration
Engine::build()
    #[cfg(feature = "editor")]
    .add_plugin(EditorPlugin)
    .build()
    .run(MyGame);
```

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
- Desktop UI toolkit (uses own Pixel UI, not egui or similar)
- 3D editing capabilities
- Runtime editor in release builds

## Open Questions

- Exact `.amigo` format specification
- Maximum undo history depth
- Whether Phase 3 AI features require a separate training step or work with general LLM reasoning
