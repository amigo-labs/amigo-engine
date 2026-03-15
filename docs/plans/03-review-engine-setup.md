# Spec: Engine Setup & Developer Experience Improvements

**Status:** Proposed
**Date:** 2025-03-15
**Scope:** Documentation, Examples, Feature Gaps, API Docs

---

## 1. Rust-Installationsanleitung in getting-started.md ergänzen

### Problem

`docs/getting-started.md` setzt eine funktionierende Rust-Toolchain voraus, erklärt aber nicht, wie man sie installiert. Neue Entwickler (besonders Pixel-Art-Künstler, die erstmals Rust nutzen) scheitern am ersten Schritt.

### Spec

Einen neuen Abschnitt **"Prerequisites"** vor dem bestehenden "Installation"-Abschnitt einfügen:

```markdown
## Prerequisites

### Rust Toolchain

Amigo Engine requires Rust (stable, edition 2021). Install via [rustup](https://rustup.rs):

**Linux / macOS:**
```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

**Windows:**
Download and run [rustup-init.exe](https://win.rustup.rs/x86_64).

**Verify installation:**
```sh
rustc --version   # should print 1.xx.x
cargo --version   # should print 1.xx.x
```

### System Dependencies

| Platform | Packages |
|----------|----------|
| Ubuntu/Debian | `sudo apt install pkg-config libx11-dev libxi-dev libxcursor-dev libxrandr-dev libvulkan-dev libasound2-dev libudev-dev` |
| Fedora | `sudo dnf install pkgconfig libX11-devel libXi-devel libXcursor-devel libXrandr-devel vulkan-loader-devel alsa-lib-devel libudev-devel` |
| Arch | `sudo pacman -S pkgconf libx11 libxi libxcursor libxrandr vulkan-icd-loader alsa-lib` |
| macOS | Xcode Command Line Tools: `xcode-select --install` |
| Windows | Visual Studio Build Tools (C++ workload) — installiert automatisch mit rustup |

### Empfohlene Extras (optional)

| Tool | Zweck | Installation |
|------|-------|-------------|
| `mold` | Schnellerer Linker (dev builds 2-3× schneller) | `sudo apt install mold` / `brew install mold` |
| Tracy | GPU/CPU-Profiler | [github.com/wolfpld/tracy](https://github.com/wolfpld/tracy) |
| Aseprite | Pixel-Art-Editor (`.ase`-Dateien) | [aseprite.org](https://www.aseprite.org) |
```

### Akzeptanzkriterien

- [ ] Neuer "Prerequisites"-Abschnitt steht **vor** "Installation"
- [ ] Linux (Ubuntu, Fedora, Arch), macOS, Windows abgedeckt
- [ ] System-Dependencies für wgpu/winit/kira/gilrs dokumentiert
- [ ] `mold`-Linker als optionaler Speed-Tipp erwähnt
- [ ] Externe Tools (Tracy, Aseprite) verlinkt

---

## 2. Mehr Beispiel-Projekte anlegen

### Problem

Es existiert nur ein einziges Beispiel (`examples/starter/`). Entwickler brauchen fokussierte, minimale Beispiele für einzelne Engine-Features, um schnell zu verstehen, wie Particles, Tilemaps, Audio etc. funktionieren.

### Spec

Sieben neue Beispiel-Projekte unter `examples/` anlegen. Jedes ist ein eigenständiges Cargo-Binary im Workspace.

#### 2.1 `examples/particles/`

**Zweck:** Particle-System demonstrieren (Emitter, Lifetime, Farben, Blending).

**Struktur:**
```
examples/particles/
├── Cargo.toml          # amigo_engine + amigo_render deps
├── src/
│   └── main.rs         # ~80 LOC
└── assets/
    └── sprites/
        └── particle.png  # 4×4 px weißer Punkt
```

**Inhalt von `main.rs`:**
- `ParticleDemo` struct mit `Game` trait
- Mausklick spawnt Emitter an Cursorposition
- Leertaste wechselt zwischen Preset-Konfigurationen (Feuer, Schnee, Funken, Rauch)
- Zeigt `draw_text` mit aktuellem Preset-Namen und Particle-Count

**Akzeptanzkriterien:**
- [ ] `cargo run -p amigo_particles` startet ohne Fehler
- [ ] Mindestens 4 Particle-Presets demonstriert
- [ ] Unter 120 LOC (nur `main.rs`, keine Module)

#### 2.2 `examples/tilemap/`

**Zweck:** Tilemap-Rendering, Auto-Tiling, Kamera-Scrolling.

**Struktur:**
```
examples/tilemap/
├── Cargo.toml
├── src/
│   └── main.rs         # ~100 LOC
└── assets/
    ├── sprites/
    │   └── tiles.png   # 16×16-Tileset (min. 8 Tiles)
    └── maps/
        └── demo.ron    # Vordefinierte Map-Daten
```

**Inhalt:**
- Lädt Tileset und Map aus RON-Datei
- WASD/Pfeiltasten bewegen die Kamera
- Zeigt Auto-Tiling (Terrain-Kanten)
- F3 zeigt Collision-Layer-Overlay

**Akzeptanzkriterien:**
- [ ] `cargo run -p amigo_tilemap_demo` startet
- [ ] Map mindestens 32×32 Tiles groß
- [ ] Auto-Tiling sichtbar aktiv
- [ ] Kamera scrollt smooth

#### 2.3 `examples/audio/`

**Zweck:** Audio-System demonstrieren (SFX, Musik, Lautstärke, Crossfade).

**Struktur:**
```
examples/audio/
├── Cargo.toml          # amigo_engine + amigo_audio deps
├── src/
│   └── main.rs         # ~90 LOC
└── assets/
    └── audio/
        ├── music_a.ogg   # ~15s Loop
        ├── music_b.ogg   # ~15s Loop (für Crossfade)
        └── sfx_click.ogg # Kurzer Click-Sound
```

**Inhalt:**
- Tastensteuerung: `1` = Musik A, `2` = Musik B (Crossfade), `Space` = SFX
- `Up/Down` = Master-Lautstärke
- HUD zeigt: aktueller Track, Lautstärke, "Press 1/2/Space"

**Akzeptanzkriterien:**
- [ ] `cargo run -p amigo_audio_demo` startet
- [ ] Musik-Crossfade hörbar
- [ ] SFX überlagert Musik korrekt
- [ ] Lautstärke-Änderung funktioniert

#### 2.4 `examples/ecs/`

**Zweck:** ECS-System demonstrieren (Spawn, Despawn, Queries, Change Tracking).

**Struktur:**
```
examples/ecs/
├── Cargo.toml
└── src/
    └── main.rs         # ~100 LOC
```

**Inhalt:**
- Spawnt 500 bunte Rechtecke mit `Position` + `Velocity` + `Color`-Komponenten
- Bouncing-Balls-Physik an Screen-Rändern
- Klick spawnt 50 neue Entities, `D` despawnt die ältesten 50
- HUD: Entity-Count, Changed-Count pro Tick

**Akzeptanzkriterien:**
- [ ] `cargo run -p amigo_ecs_demo` startet
- [ ] Spawn/Despawn visuell sichtbar
- [ ] Change Tracking im HUD angezeigt

#### 2.5 `examples/input/`

**Zweck:** Input-System demonstrieren (Keyboard, Maus, Gamepad, Action Maps).

**Struktur:**
```
examples/input/
├── Cargo.toml
├── src/
│   └── main.rs         # ~80 LOC
└── input.ron           # Action-Map-Konfiguration
```

**Inhalt:**
- Zeigt Live-Darstellung aller gedrückten Tasten
- Mausposition + Buttons visuell angezeigt
- Gamepad-Stick + Buttons wenn angeschlossen
- Action-Map: "Jump" → Space/Gamepad-A, "Move" → WASD/Left-Stick

**Akzeptanzkriterien:**
- [ ] `cargo run -p amigo_input_demo` startet
- [ ] Keyboard-Input visuell dargestellt
- [ ] Gamepad wird erkannt (wenn vorhanden)

#### 2.6 `examples/animation/`

**Zweck:** Sprite-Animation-System (Aseprite-Integration, State Machine).

**Struktur:**
```
examples/animation/
├── Cargo.toml
├── src/
│   └── main.rs         # ~90 LOC
└── assets/
    └── sprites/
        ├── character.png  # Spritesheet (idle 4f, walk 6f, jump 2f)
        └── character.ron  # Animation-Definitionen
```

**Inhalt:**
- Character mit Idle/Walk/Jump-Animationen
- Pfeiltasten = Walk-Animation, Space = Jump-Animation
- Automatischer Übergang: Jump → Idle, Walk-Stop → Idle
- HUD: Aktueller State, Frame-Index, Animation-Speed

**Akzeptanzkriterien:**
- [ ] `cargo run -p amigo_animation_demo` startet
- [ ] Mindestens 3 Animation-States sichtbar
- [ ] State-Übergänge korrekt

#### 2.7 `examples/pathfinding/`

**Zweck:** A*-Pathfinding + Flow Fields demonstrieren.

**Struktur:**
```
examples/pathfinding/
├── Cargo.toml
├── src/
│   └── main.rs         # ~120 LOC
└── assets/
    └── sprites/
        └── tiles.png
```

**Inhalt:**
- Grid-basierte Map mit Hindernissen
- Linksklick = Start, Rechtsklick = Ziel → A*-Pfad wird gezeichnet
- `F` = Flow-Field-Overlay (Richtungspfeile pro Zelle)
- `Space` = Toggle Hindernisse malen/löschen

**Akzeptanzkriterien:**
- [ ] `cargo run -p amigo_pathfinding_demo` startet
- [ ] A*-Pfad visuell korrekt
- [ ] Flow-Field-Overlay zeigt Richtungspfeile

### Workspace-Integration

Alle neuen Beispiele müssen in `/Cargo.toml` unter `[workspace] members` eingetragen werden:

```toml
members = [
    # ... existing ...
    "examples/particles",
    "examples/tilemap_demo",
    "examples/audio_demo",
    "examples/ecs_demo",
    "examples/input_demo",
    "examples/animation_demo",
    "examples/pathfinding_demo",
]
```

---

## 3. Fehlende Features — Implementierungsstatus korrigieren & echte Lücken schließen

### Korrektur: Bereits implementiert

Die folgenden Features sind im Code **vorhanden**, obwohl `review-engine-docs.md` sie als "Not found" / "Not connected" listet:

| Feature | Tatsächlicher Status | Fundort |
|---------|---------------------|---------|
| **Spatial Hash** | Implementiert, getestet | `crates/amigo_core/src/collision.rs:66` — `SpatialHash` struct mit `insert`, `remove`, `query_aabb`, `query_point`, `query_circle`, `clear`. 7 Unit-Tests. Integriert in `CollisionWorld` und `PhysicsWorld`. |
| **Flow Fields** | Implementiert, getestet | `crates/amigo_core/src/pathfinding.rs:153` — `FlowField::compute()` mit Dijkstra. 5 Unit-Tests (goal cost, direction, unreachable, out-of-bounds). |
| **Tracy Integration** | Implementiert (feature-gated) | `crates/amigo_debug/src/lib.rs:229` — `frame_mark()`, `tracy_enabled()`, `init_logging()` mit Tracy-Layer. Feature `tracy` in Cargo.toml. `tracing-tracy` + `tracy-client` als Dependencies. Frame-mark-Aufruf in Engine-Loop (`engine.rs:836`). |

**Aktion:** `docs/review-engine-docs.md` Tabelle "Spec vs. Implementation — Gaps" aktualisieren. Diese drei Einträge von "Not found" / "Not connected" auf "Implemented" ändern.

### Echte verbleibende Lücken

| Feature | Prio | Aufwand | Spec |
|---------|------|---------|------|
| **egui Editor in Engine-Loop** | Mittel | M | egui-Rendering-Pass in `amigo_render` integrieren. `amigo_editor` Panels über `egui::Context` in den Engine-Frame-Loop einbinden. Behind `editor` feature flag. egui-winit Events im Input-System durchreichen. |
| **Asset Packing (`amigo pack`)** | Mittel | M | `amigo_cli` um Subcommand `pack` erweitern. Alle Dateien aus `assets/` in ein `game.pak` (tar-ähnlich, unkomprimiert, mit Index-Header) packen. `amigo_assets` um `PakReader` erweitern, der im Release-Modus `game.pak` statt Filesystem nutzt. |
| **Headless Simulation (`amigo_tick`)** | Hoch | S | In `amigo_api` neuen JSON-RPC-Handler `amigo_tick(count: u32)` hinzufügen. Ruft `Game::update()` N-mal ohne Rendering auf. Für AI-Playtesting und Balancing. |
| **Screenshot API** | Hoch | S | In `amigo_api` neuen Handler `amigo_screenshot()` hinzufügen. Liest den aktuellen wgpu-Framebuffer, kodiert als PNG, gibt Base64 zurück. Benötigt `wgpu::Buffer` mit `MAP_READ` Usage. |

### Detailspecs für die echten Lücken

#### 3.1 egui Editor in Engine-Loop

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

#### 3.2 Asset Packing

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

#### 3.3 Headless Simulation

**Ziel:** AI-Agents können die Simulation ohne Rendering vorspulen.

**Änderungen:**
1. `crates/amigo_api/src/handler.rs` — Neuer Handler:
   ```rust
   "amigo_tick" => {
       let count: u32 = params["count"].as_u64().unwrap_or(1) as u32;
       for _ in 0..count {
           game.update(&mut context);
           context.world.flush();
       }
       Ok(json!({ "ticks_advanced": count, "total_tick": context.tick_count }))
   }
   ```

**Akzeptanzkriterien:**
- [ ] `amigo_tick(100)` führt 100 Update-Zyklen aus
- [ ] Kein Rendering während der Ticks
- [ ] Rückgabe enthält `ticks_advanced` und `total_tick`

#### 3.4 Screenshot API

**Ziel:** AI-Agents können den aktuellen Frame als Bild abrufen.

**Änderungen:**
1. `crates/amigo_render/src/screenshot.rs` — `capture_frame(device, queue, texture) -> Vec<u8>` (PNG bytes)
2. `crates/amigo_api/src/handler.rs` — Neuer Handler:
   ```rust
   "amigo_screenshot" => {
       let png_bytes = renderer.capture_frame();
       let b64 = base64::encode(&png_bytes);
       Ok(json!({ "image": b64, "width": w, "height": h, "format": "png" }))
   }
   ```

**Akzeptanzkriterien:**
- [ ] `amigo_screenshot()` gibt Base64-PNG zurück
- [ ] Auflösung entspricht `virtual_resolution`
- [ ] Funktioniert im Headless-Modus (offscreen rendering)

---

## 4. API-Docs generieren (cargo doc Setup)

### Problem

Es gibt keine `cargo doc`-Konfiguration. Entwickler können keine API-Referenz browsen. Kein CI-Job prüft, ob Doc-Comments kompilieren.

### Spec

#### 4.1 Workspace-Level Doc-Konfiguration

`Cargo.toml` um `[workspace.metadata.docs]` erweitern (optional) und ein Makefile/Justfile-Target anlegen:

**Neues Target in CI (`ci.yml`):**
```yaml
  docs:
    name: Documentation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build docs
        run: cargo doc --workspace --no-deps --all-features
        env:
          RUSTDOCFLAGS: "-D warnings"
      - name: Upload docs artifact
        uses: actions/upload-artifact@v4
        with:
          name: api-docs
          path: target/doc/
```

#### 4.2 Doc-Tests als CI-Check

Zum bestehenden `test`-Job hinzufügen:
```yaml
      - name: Doc tests
        run: cargo test --workspace --doc
```

#### 4.3 Workspace-Root `lib.rs` Doc-Comment

`crates/amigo_engine/src/lib.rs` sollte einen umfassenden Crate-Level-Doc-Comment haben:

```rust
//! # Amigo Engine
//!
//! A modern 2D pixel art game engine in Rust.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use amigo_engine::prelude::*;
//!
//! struct MyGame;
//!
//! impl Game for MyGame {
//!     fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
//!         SceneAction::Continue
//!     }
//!     fn draw(&self, ctx: &mut DrawContext) {
//!         ctx.draw_text("Hello!", 10.0, 10.0, Color::WHITE);
//!     }
//! }
//!
//! fn main() {
//!     Engine::build()
//!         .title("My Game")
//!         .virtual_resolution(480, 270)
//!         .build()
//!         .run(MyGame);
//! }
//! ```
//!
//! ## Feature Flags
//!
//! | Flag | Description |
//! |------|-------------|
//! | `audio` | Audio playback via kira (default) |
//! | `editor` | egui editor overlay |
//! | `api` | JSON-RPC API + headless mode |
//! | `tracy` | Tracy profiler integration |
```

#### 4.4 `#![warn(missing_docs)]` für Public Crates

Folgende Crates sollten `#![warn(missing_docs)]` in ihrer `lib.rs` erhalten:

- `amigo_engine` (public API)
- `amigo_core` (ECS, math, types)
- `amigo_render` (renderer API)
- `amigo_input` (input API)
- `amigo_audio` (audio API)
- `amigo_scene` (scene management)

**Nicht** für interne Crates wie `amigo_editor`, `amigo_api`, `amigo_debug` (noch zu instabil).

#### 4.5 Lokales Doc-Building

Entwickler können Docs lokal bauen:

```sh
# Alle Docs bauen und öffnen
cargo doc --workspace --no-deps --open

# Mit allen Features (inkl. editor, api, tracy)
cargo doc --workspace --no-deps --all-features --open
```

Diesen Hinweis in `CONTRIBUTING.md` unter einem neuen Abschnitt "## API Documentation" dokumentieren.

### Akzeptanzkriterien

- [ ] `cargo doc --workspace --no-deps --all-features` baut ohne Warnings (`-D warnings`)
- [ ] CI-Job "docs" existiert und läuft bei jedem PR
- [ ] Doc-Tests laufen im CI (`cargo test --doc`)
- [ ] `amigo_engine` hat Crate-Level-Doc-Comment mit Beispiel
- [ ] `CONTRIBUTING.md` dokumentiert `cargo doc`-Workflow

---

## Priorisierung & Reihenfolge

| # | Task | Prio | Aufwand | Abhängigkeit |
|---|------|------|---------|-------------|
| 1 | Getting-Started Prerequisites | Hoch | XS | Keine |
| 2 | Review-Doc Gap-Tabelle korrigieren | Hoch | XS | Keine |
| 3 | Cargo-Doc CI-Job + `#![warn(missing_docs)]` | Hoch | S | Keine |
| 4 | Headless Simulation (`amigo_tick`) | Hoch | S | Keine |
| 5 | Screenshot API | Hoch | S | Keine |
| 6 | Beispiel-Projekte (7 Stück) | Mittel | L | Keine |
| 7 | egui Editor Integration | Mittel | M | Keine |
| 8 | Asset Packing | Mittel | M | Keine |
