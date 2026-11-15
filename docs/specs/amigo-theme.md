---
status: todo
created: 2026-11-15
size: M
crates: [amigo_render, amigo_editor]
design: [tooling/editor.md, ../adrs/0014-shared-amigo-theme.md]
---

# Shared amigo theme for the editor

## Context

The editor is an egui app (`tooling/editor.md`) and runs with egui's default dark visuals. The other amigo-labs apps share one theme, generated from amigo-labs/website into each app and checked in CI (ADR-0014). This spec gives the editor that theme: its dark palette by default, the light one on request, the theme's radii, control sizes and text sizes, and amigo yellow as accent.

## Scope

### In scope

- The generated theme module in `amigo_render`, checked in CI against the website
- `egui::Visuals` and `egui::Style` built from the theme, applied when the egui context is created
- A Dark / Light choice in the editor's View menu, dark by default
- The editor's own interface colours that are not content

### Out of scope

- Remembering the choice between runs — because the editor has no preferences store this spec could use; backlog
- Following the system's appearance — because egui-winit reports it differently per platform and the editor is a developer tool; backlog
- Fonts — because egui ships its own and a bundled Inter is a new asset; the theme's text sizes are used
- Game rendering, the Pixel UI (`amigo_ui`), debug overlays — because the theme styles the editor's chrome, not what a game draws
- Colours that are content: tile swatches, heatmap ramps, collision and path overlays — because they encode data

## Acceptance criteria

- [ ] AC-01 (always): `crates/amigo_render/src/theme.rs` is the module amigo-labs/website generates with the default accent (`node scripts/theme.ts default --rust …`), unedited, compiled only with the `editor` feature, and `cargo fmt --all -- --check` passes with it.
- [ ] AC-02 (always, CI: theme): A CI job checks out amigo-labs/website and runs `node scripts/theme.ts default --check --rust crates/amigo_render/src/theme.rs`; a stale or edited copy fails it.
- [ ] AC-03 (always): `amigo_render::egui_theme::visuals(mode)` returns `egui::Visuals` whose colours all come from the palette of that mode: window and panel fill `surface`, the area behind panels `bg`, popups and menus `surface_raised`, strokes of windows and separators `border`, text `text`, weak text `text_muted`, hyperlinks `accent_ink`, warnings `warning_ink`, errors `danger_ink`.
- [ ] AC-04 (always): Widgets take their states from the palette: at rest `surface_raised` with a `border` stroke, hovered `hover`, pressed `active`, open (a menu button whose menu shows) `active`; text and icons on them are `text`; a text field's and a checkbox's outline is `border_strong`.
- [ ] AC-05 (always): The selection (selected rows and labels, text selection) is filled with `accent_subtle` and its text stays `text`; the keyboard-focus outline is `focus`; no colour of egui's default blue (`rgb(0, 92, 128)`, `rgb(90, 170, 255)`, `rgb(144, 209, 255)`) is left in either mode.
- [ ] AC-06 (always): Corner radii are the theme's: widgets `RADIUS_SM`, menus and popups `RADIUS_MD`, windows `RADIUS_LG`.
- [ ] AC-07 (always): `amigo_render::egui_theme::style(mode)` sets the text styles to the theme's sizes (heading `TEXT_TITLE`, body and button `TEXT_BODY`, small `TEXT_CAPTION`, monospace `TEXT_CODE`), the interactive height to `CONTROL_SM` and the item spacing to `SPACE_2`.
- [ ] AC-08 (always): In both modes text has a contrast of at least 4.5 : 1 on its ground (`text`, `text_muted`, `accent_ink`, `warning_ink`, `danger_ink` on `surface` and on `surface_raised`; `text` on `accent_subtle`), and `focus` and `border_strong` at least 3 : 1 on `surface`.
- [ ] AC-09 (always): `EguiRenderer::new` applies the dark theme, so an editor that never chooses is dark.
- [ ] AC-10 (event): When the user picks View ▸ Theme ▸ Light or Dark, then the whole editor switches at once; the entry of the active mode is marked.
- [ ] AC-11 (always): The editor draws no interface colour of its own: outside content colours (Scope) `crates/amigo_editor/src` holds no `Color32::from_rgb`, `from_gray` or hex literal; the empty tile swatch is `surface_raised` and the selected swatch's frame is `focus`.
- [ ] AC-12 (always): With the `editor` feature off, nothing of this is compiled: `cargo check -p amigo_render` and the default workspace build have no `egui` and no `theme` module.
- [ ] AC-13 (always, manual): The running editor looks right in both modes: menus, panels, inspector, tile palette, dialogs readable, the accent yellow, nothing left in egui's default look.

## Technical notes

- Generate with a checkout of amigo-labs/website: `node scripts/theme.ts default --out <scratch dir> --rust ../amigo-engine/crates/amigo_render/src/theme.rs`. Never edit the file.
- `egui_theme.rs` (new, `editor` feature): `pub enum ThemeMode { Dark, Light }`, `pub fn visuals(ThemeMode) -> egui::Visuals`, `pub fn style(ThemeMode) -> egui::Style`, `pub fn apply(&egui::Context, ThemeMode)`. Start from `egui::Visuals::dark()` / `light()` so fields this spec does not name keep sensible values, then overwrite every colour; a test walks the result for the default blues (AC-05).
- `egui::Visuals` cannot be exhaustively pattern-matched across egui versions; the tests assert the named fields and scan a `Debug` print of the visuals for the default blues.
- The editor's choice is state of the editor session (`amigo_editor`), read by the View menu and applied through `amigo_render::egui_theme::apply`; `amigo_editor` already depends on egui behind its `egui` feature.
- The CI job mirrors what the website's `validate-project` workflow does to run its scripts (checkout of amigo-labs/website, pnpm, Node 22, `pnpm install --frozen-lockfile`); it is a job of `ci.yml`, not a `just` recipe, because it needs the second repository.
- Tests need no GPU and no window: `egui::Context::default()` plus the functions above.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The editor is built without `editor_v2` | The theme still applies: it belongs to the egui context, not to one editor UI |
| A game sets its own `egui::Visuals` on the context | The game's call wins; the engine applies the theme once at creation and on a View ▸ Theme pick |
| The shared theme changes in the website | The CI job fails until `theme.rs` is regenerated |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |

## Decisions

- Accent: the theme's default yellow. The engine has no entry on amigo-labs.dev and so no brand colour; if it gets one, only the generator's argument changes.
- Dark and light, no "System": the editor is a developer tool started from a terminal; following the desktop adds per-platform code for little use.
- The choice is not stored: there is no editor preferences file yet, and inventing one is its own feature.
- `CONTROL_SM` (24) as interactive height, not `CONTROL_MD` (32): an editor is dense, and egui's default is 18.
