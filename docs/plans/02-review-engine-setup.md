# Spec: Engine Setup & Developer Experience Improvements

**Status:** In Progress
**Date:** 2025-03-15
**Scope:** Feature Gaps

---

## Verbleibende Tasks

### 1. egui Editor in Engine-Loop

**Prio:** Mittel | **Aufwand:** M

**Ziel:** Das `editor`-Feature-Flag aktiviert ein egui-Overlay über dem Spielfenster.

**Änderungen:**

1. `crates/amigo_render/src/lib.rs` — Neues Modul `egui_pass` (feature-gated):
   - `EguiRenderer` struct mit `egui_winit::State` + `egui_wgpu::Renderer`
   - `begin_frame(&mut self, window: &Window)` — leitet winit-Events an egui weiter
   - `end_frame(&mut self, encoder: &mut CommandEncoder, view: &TextureView)` — rendert egui über den Game-Frame

2. `crates/amigo_engine/src/engine.rs` — Im Render-Loop:
   ```rust
   #[cfg(feature = "editor")]
   {
       egui_renderer.begin_frame(&window);
       editor.ui(&egui_renderer.context(), &mut game_context);
       egui_renderer.end_frame(&mut encoder, &view);
   }
   ```

3. `crates/amigo_editor/src/lib.rs` — `Editor::ui()` Methode, die bestehende Panels (`tile_painter`, `wave_editor`, etc.) als egui-Windows rendert.

**Akzeptanzkriterien:**
- [ ] `cargo run --features editor` zeigt egui-Overlay
- [ ] Editor-Panels via Menü ein-/ausblendbar
- [ ] Kein Performance-Impact wenn Feature deaktiviert

---

### 2. Asset Packing (`amigo pack`)

**Prio:** Mittel | **Aufwand:** M

**Ziel:** `amigo pack` erzeugt eine einzelne `game.pak`-Datei für Release-Builds.

**Format:**
```
[4 bytes: magic "AMPK"]
[4 bytes: version u32]
[4 bytes: entry_count u32]
[entry_count × Entry]:
    [4 bytes: path_len u32]
    [path_len bytes: UTF-8 path]
    [8 bytes: offset u64]
    [8 bytes: size u64]
[raw file data, concatenated]
```

**Änderungen:**
1. `tools/amigo_cli/src/main.rs` — Subcommand `pack` hinzufügen
2. Neues Modul `tools/amigo_cli/src/pack.rs` — Traversiert `assets/`, schreibt `game.pak`
3. `crates/amigo_assets/src/pak.rs` — `PakReader` struct mit `open()`, `read_file(path) -> Vec<u8>`
4. `crates/amigo_assets/src/manager.rs` — Im Release-Modus `PakReader` statt Filesystem nutzen

**Akzeptanzkriterien:**
- [ ] `amigo pack` erzeugt `game.pak` aus `assets/`
- [ ] `cargo run --release` lädt Assets aus `game.pak`
- [ ] Fallback auf Filesystem wenn `game.pak` fehlt
