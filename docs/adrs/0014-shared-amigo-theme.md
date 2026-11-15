# ADR-0014: The Editor Uses the Shared amigo App Theme

- Status: accepted
- Date: 2026-11-15

## Context

The editor is an egui app and ran with egui's default dark visuals. The other
amigo-labs apps (Pincel, Krake, Sofarr, Blinzel) share one theme: the same
surfaces, text colours, status colours, radii and control sizes, with one
accent per app. That theme lives in amigo-labs/website (`theme/`) and is
generated into each app; a CI check fails when an app's copy is stale. An
editor that looks like a different product next to them is a cost with no
benefit.

## Decision

The editor takes its look from the shared theme.

- `crates/amigo_render/src/theme.rs` is the Rust module amigo-labs/website
  generates (`node scripts/theme.ts default --rust …`): the palettes `DARK`
  and `LIGHT`, text styles, spacing, control sizes and radii. It is never
  edited by hand. CI regenerates it from the website's `main` and fails on a
  difference.
- The engine has no brand colour of its own (it has no page on
  amigo-labs.dev), so the accent is the theme's default, amigo yellow.
- `amigo_render::egui_theme` turns a palette into `egui::Visuals` and a
  `egui::Style`. Dark is the default; the editor's View menu switches to
  light.
- Game rendering is untouched: the theme styles the editor's chrome, never
  what a game draws. Colours that are content (tile swatches, heatmaps, debug
  overlays) stay as they are.

## Consequences

- No new dependency: the theme is a generated source file.
- The generated module sits behind the `editor` feature, like the egui
  integration it serves.
- A change of the shared theme reaches the editor by regenerating one file;
  the CI check says when that is due.
- The website repository becomes a build-time input of one CI job (not of the
  build itself).
