---
status: done
crate: amigo_ui
depends_on: ["engine/core", "engine/rendering"]
last_updated: 2026-03-16
---

# UI System (Pixel-Native, Two Tiers)

> **How it reaches the screen.** `UiContext` emits `UiDrawCommand`s; the engine
> translates them into sprites (`crates/amigo_engine/src/ui_bridge.rs`) and draws
> them in a screen-space pass *after* post-processing, per conventions A.6. That
> pass has its own orthographic projection with no camera in it, so a HUD neither
> scrolls with the world nor scales with camera zoom, and post effects do not bend
> it. Until that bridge existed, every widget produced draw commands that nothing
> consumed — the whole widget set drew nothing.
>
> Call `ctx.ui` from `update`, not `draw`: the engine clears the command list at
> the start of each tick and reads it after.

## Purpose

One UI system, two complexity levels. All rendering through the engine's sprite batcher -- bitmap fonts, sprite-based widgets, pixel-perfect at virtual resolution.

## Public API

### Tier 1: Game HUD (always available)

```rust
ui.sprite("gold_icon", pos);
ui.pixel_text("Gold: 350", pos, Color::GOLD);
if ui.sprite_button("btn_archer", pos) { /* select tower */ }
ui.panel(rect, |ui| { /* nested content */ });
ui.progress_bar(rect, health / max_health, Color::RED);
```

Immediate mode, ~20-30 functions. Bitmap fonts, sprite buttons, panels, progress bars, text. Renders identically to game sprites -- no anti-aliasing, no style mismatch.

### Tier 2: Editor Widgets (behind `editor` feature flag)

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

Builds on Tier 1. Added: text input, sliders, dropdowns, color pickers, scrollable containers, tree views. Editor look is consistent with the game's pixel art aesthetic -- not a generic desktop UI.
