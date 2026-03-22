---
number: "0011"
title: Deklarative UI mit Retained-Mode Layout
status: done
date: 2026-03-20
---

# ADR-0011: Deklarative UI mit Retained-Mode Layout

## Status

done

## Context

All in-game UI is rendered through the immediate-mode `UiContext` in `crates/amigo_ui/src/lib.rs`. `UiContext` stores a flat `Vec<UiDrawCommand>` (line 7) that is rebuilt every frame via `begin()` (line 58, clears the command list) and `end()` (line 64, returns the slice). Every widget is positioned with absolute pixel coordinates -- for example `pixel_text(&mut self, text: &str, x: f32, y: f32, color: Color)` (line 69) and `filled_rect(&mut self, rect: Rect, color: Color)` (line 100).

This works well for HUD overlays where elements are placed at known screen positions, but breaks down for complex menu systems:

1. **No layout engine**: Building an inventory grid, a settings menu with tabs, or a dialogue box with word-wrapping requires the game developer to compute every element position manually. The `scrollable_list` widget (line 488) hardcodes `item_h = 16.0` and `visible_count` as parameters, with manual scroll offset tracking.

2. **No hierarchy**: The `UiDrawCommand` enum (line 14) is flat -- there is no concept of parent/child containers, padding, or margin. The `tree_view` widget (line 570) fakes hierarchy by tracking `(label, depth, expanded)` tuples and computing indentation manually (`indent = 16.0`, line 581).

3. **No responsive layout**: If the virtual resolution changes or the game window is resized, all absolute positions break. There are no percentage-based sizes or flex properties.

4. **Widget count**: The existing widgets (text, rect, sprite, progress bar, checkbox, slider, dropdown, text_input, color_picker, scrollable_list, tree_view, text_button, tooltip, separator, label_panel) total 16 widgets using ~650 lines of manual layout code. Adding a new complex widget (e.g., tabbed panel, grid layout) requires duplicating this pattern.

The `UiDrawCommand` enum ultimately produces `SpriteInstance` geometry for the GPU via the renderer. Any retained-mode system must output the same `UiDrawCommand` list so the existing render path is unchanged.

## Decision

Add a **retained-mode layout system** behind the `ui_v2` feature flag in a new module `crates/amigo_ui/src/layout.rs`. The immediate-mode `UiContext` remains the primary interface for HUD rendering (Tier 1). The retained-mode system (Tier 2) provides a declarative node tree with Flexbox-like layout, which resolves to absolute positions and emits `UiDrawCommand`s into the existing `UiContext`.

The layout model uses a **Taffy**-inspired approach (simplified for 2D games):

- A `UiTree` holds a flat `Vec<UiNode>` arena with parent/child indices.
- Each `UiNode` has a `Style` (width, height, padding, margin, gap, flex_direction, justify_content, align_items, flex_grow, flex_shrink) and a `Content` (None, Text, Sprite, Widget callback).
- `UiTree::layout(available_width, available_height)` runs a single-pass flexbox algorithm that computes `Rect` for each node.
- `UiTree::render(&mut UiContext)` walks the tree in depth-first order and emits `UiDrawCommand`s at the computed positions.

The immediate-mode API is untouched. Game code that uses `UiContext::pixel_text()` continues to work. For complex menus, game code builds a `UiTree` once (or rebuilds it when the menu state changes) and calls `tree.render(&mut ui_ctx)` each frame.

### Alternatives Considered

1. **Replace UiContext with a full retained-mode framework (e.g., embed egui for in-game UI)**: Rejected because egui's styling is desktop-oriented and difficult to theme for pixel-art games. It also adds a large dependency for all game builds, not just editor builds. The current `UiContext` is lightweight (~650 lines) and produces pixel-perfect output for HUDs.

2. **Immediate-mode layout helpers (auto-layout UiContext extensions)**: Add `begin_row()` / `end_row()` / `begin_column()` / `end_column()` methods to `UiContext` that track a cursor and auto-advance positions. Rejected because this still requires every frame to recompute layout, and nested flex layouts with `flex_grow` / `justify_content` cannot be resolved in a single forward pass without a retained tree structure.

## Migration Path

1. **Add `UiNode`, `Style`, and `UiTree` types** -- Create `crates/amigo_ui/src/layout.rs` with the node arena, style struct, and tree builder API. `Style` fields: `width: Size` (Fixed, Percent, Auto), `height: Size`, `min_width`, `max_width`, `padding: Edges`, `margin: Edges`, `gap: f32`, `flex_direction: FlexDirection` (Row, Column), `justify_content: JustifyContent` (Start, Center, End, SpaceBetween, SpaceAround), `align_items: AlignItems` (Start, Center, End, Stretch), `flex_grow: f32`, `flex_shrink: f32`, `background: Option<Color>`, `border: Option<(Color, f32)>`. Implement `UiTree::add_node(parent, style, content) -> NodeId`. Verify: unit test creates a tree with 1 root + 3 children, calls `layout(320.0, 240.0)`, asserts each child rect is within the root bounds and non-overlapping.

2. **Implement the flexbox layout algorithm** -- Single-pass flex resolution in `UiTree::layout()`: (a) resolve fixed/percent sizes in a top-down pass, (b) distribute remaining space to `flex_grow` children, (c) position children according to `justify_content` and `align_items`, (d) recurse into children. Verify: benchmark layout of 100 nodes in a 3-level hierarchy. Must complete in under 1ms (abort criterion). Verify with `cargo bench`.

3. (rough) Implement `UiTree::render(&mut UiContext)` -- Walk nodes in depth-first order. For each node with `background`, emit `UiDrawCommand::Rect`. For `Content::Text`, emit `UiDrawCommand::Text` at the node's computed position plus padding. For `Content::Sprite`, emit `UiDrawCommand::Sprite`.

4. (rough) Add convenience builder API: `UiTree::column(style, |builder| { builder.text("Hello"); builder.row(style, |b| { b.button("OK"); b.button("Cancel"); }); })`.

5. (rough) Port the `dropdown` widget to use retained-mode layout internally as a proof-of-concept, comparing code complexity and visual output with the existing manual implementation (`lib.rs`, lines 246-303).

6. (rough) Add `Content::Widget(Box<dyn FnMut(&mut UiContext, Rect)>)` for integrating immediate-mode widgets into the layout tree (e.g., embedding a slider at a layout-computed position).

## Abort Criteria

- If layout calculation for 100 UI nodes in a 3-level hierarchy exceeds 1ms on the target hardware (measured via `std::time::Instant` in a benchmark), abandon the built-in layout engine and evaluate integrating the `taffy` crate directly.
- If the retained-mode `render()` pass adds more than 0.5ms overhead compared to the equivalent hand-coded immediate-mode UI (measured on a menu with 50 visible elements), simplify to a flat auto-layout helper instead of a full tree.

## Consequences

### Positive
- Complex menus (inventory grids, settings panels, dialogue boxes) can be built declaratively without manual pixel math.
- Flex layout responds to different virtual resolutions and aspect ratios automatically.
- The immediate-mode `UiContext` is unchanged -- zero migration cost for existing HUD code.
- Layout is computed only when the tree changes, not every frame, which is cheaper than immediate-mode for static menus.

### Negative / Trade-offs
- Two UI paradigms (immediate-mode for HUDs, retained-mode for menus) may confuse new users. Clear documentation and examples are needed to guide when to use which.
- The layout algorithm adds ~500-800 lines of code to `amigo_ui`. If the full flexbox spec is not needed, this may be over-engineered for simple pixel-art games.
- `UiTree` allocates a `Vec<UiNode>` on the heap. For menus that rebuild every frame (e.g., dynamically changing item lists), this allocation pattern differs from the existing zero-allocation `UiContext::begin()/end()` cycle. Mitigation: `UiTree` can be retained across frames and mutated incrementally.
- Styling for the retained-mode tree must be defined separately from immediate-mode widget styling, potentially leading to visual inconsistencies if not carefully managed.

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
