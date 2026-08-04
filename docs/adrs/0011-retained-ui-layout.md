# ADR-0011: Retained-Mode Layout Over the Immediate-Mode UI

- Status: accepted
- Date: 2026-08-03

## Context

`UiContext` is immediate-mode: every widget takes absolute coordinates and emits
draw commands for that frame. That is a good fit for a HUD, where positions are
known and few. It is a bad fit for anything that has to *fit*: a settings list
that grows, a dialogue box that wraps, an inventory grid that centres itself.
Computing those positions by hand in game code means recomputing them whenever
the virtual resolution or the content changes.

Replacing the immediate-mode API with a retained tree would be a breaking change
for every existing HUD, and would make the simple case harder.

## Decision

Add a retained layout tree *on top of* the immediate-mode context, behind the
`retained_layout` feature flag (`crates/amigo_ui/src/layout.rs`). A `UiTree` of
nodes with Flexbox-like sizing (`Size::Fixed`, `Size::Percent`, and grow) is
resolved to absolute rectangles, and those rectangles are then emitted as the
same `UiDrawCommand`s the immediate-mode widgets produce.

Both APIs therefore render through one path, and a game can use absolute
coordinates for its HUD and a tree for its menus without two renderers.

## Consequences

- Layout is opt-in and additive: existing code is untouched.
- There is one draw-command vocabulary, so the render bridge does not have to
  know which API produced a command.
- Two ways to build UI is a real cost in learnability. The split is drawn at
  "does this need to be measured": if not, use the immediate API.
- Layout resolution happens per frame. It is over a handful of nodes, not a
  document, so no caching layer exists yet — add one when a profile says to.
