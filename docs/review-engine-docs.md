# Review: Amigo Engine — Docs, Specs, Architecture & Splashscreen

## Context

Umfassende Überprüfung der Amigo Engine auf: Spec-Vollständigkeit, Developer Experience, AI-Integration, Service-Kommunikation und Lizenzstrategie. Ergebnis: Dokumentation aufbessern + Default-Splashscreen einbauen.

## Entscheidungen
- **Lizenz:** MIT OR Apache-2.0 bleibt
- **Splashscreen:** Default "Powered by Amigo Engine", deaktivierbar via `EngineBuilder`

---

## 1. Spec-Implementierungs-Status

### Implementiert (mit echtem Code, ~34.000 LOC Rust)

| Crate | LOC | Status | Bemerkung |
|-------|-----|--------|-----------|
| `amigo_core` | 15.795 | **Solide** | ECS (SparseSet + Change Tracking), Fixed-Point Math, Pathfinding, Collision, Physics, Save System, Scheduler, Commands, 10+ Genre-Module (TD, Platformer, Roguelike, Fighting, Farming, Puzzle, Bullet-Hell...) |
| `amigo_render` | 3.695 | **Solide** | wgpu Renderer, Sprite Batcher, Camera, Particles, Lighting, Post-Processing, Atmosphaere, Font-Rendering |
| `amigo_animation` | 1.935 | **Solide** | Sprite-Animation State Machine, Aseprite-Integration |
| `amigo_net` | 2.169 | **Solide** | Protocol, Client/Server, Replay, Lobby, Stats, Checksum |
| `amigo_editor` | 4.829 | **Solide** | Tile-Painter, Wave-Editor, Collision-Editor, Playtest, Heatmap, Visual Scripting, Auto-Path, Wizard |
| `amigo_input` | 931 | **OK** | Keyboard, Mouse, Gamepad, Action-Maps |
| `amigo_audio` | 1.112 | **OK** | kira Wrapper, Audio Manager |
| `amigo_api` | 1.198 | **OK** | JSON-RPC 2.0 Server + Handler |
| `amigo_assets` | 647 | **OK** | Asset Manager, Hot Reload, Aseprite, Handles |
| `amigo_tilemap` | 762 | **OK** | TileLayer, Auto-Tiling |
| `amigo_scene` | 361 | **OK** | Scene Stack, Transitions |
| `amigo_ui` | 361 | **Minimal** | Immediate-Mode Pixel UI (Basis vorhanden) |
| `amigo_debug` | 274 | **Minimal** | FPS Overlay, Toggle-System |
| `amigo_engine` | 1.002 | **Solide** | Game Loop, EngineBuilder, Plugin System, GameContext, DrawContext |

### Tools (separate Binaries)

| Tool | LOC | Status |
|------|-----|--------|
| `amigo_cli` | 949 | **OK** -- Projekt-Scaffolding, 10 Templates |
| `amigo_mcp` | 902 | **OK** -- MCP-Bridge zu amigo_api |
| `amigo_artgen` | 2.468 | **OK** -- ComfyUI-Integration, Post-Processing |
| `amigo_audiogen` | 1.762 | **OK** -- ACE-Step + AudioGen Integration |

### Spec vs. Implementierung -- Luecken

| Spec-Feature | Status | Anmerkung |
|-------------|--------|-----------|
| Sektionen 10-27 der Spec-v2 | **Nur Verweis** | Die unified spec enthielt Sektionen 10-27 nur als Verweise. Jetzt vervollstaendigt. |
| Tracy Profiling | **Nicht verbunden** | `amigo_debug` hat kein Tracy-Integration, nur FPS-Overlay |
| Spatial Hash / Flow Fields | **Nicht gefunden** | Pathfinding ja, aber Spatial Hash Broad-Phase und Flow Fields fehlen im Code |
| egui Editor-UI | **Nicht integriert** | Editor-Code existiert, aber egui-Rendering-Pipeline fehlt im Engine-Loop |
| Asset Packing (`game.pak`) | **Nicht implementiert** | CLI hat Scaffolding, aber kein `amigo pack` Command |
| Headless Simulation | **Nicht implementiert** | Spec beschreibt headless tick-forward fuer AI, Code fehlt |
| Screenshot API | **Nicht implementiert** | `amigo_screenshot` MCP-Tool beschrieben, aber nicht gebaut |
| Splashscreen | **Implementiert** | Default "Powered by Amigo Engine", deaktivierbar |

---

## 2. Developer Experience -- Kann ein Entwickler ein neues Spiel bauen?

### Was gut ist
- **Quick Start ist klar:** `cargo install --path tools/amigo_cli && amigo new my_game && cargo run`
- **10 Genre-Templates** im CLI (platformer, topdown-rpg, roguelike, tower-defense, etc.)
- **Minimal Example** in README und lib.rs doc-comment
- **Game Trait** ist simpel: `init()`, `update()`, `draw()` -- kein Macro-Magic
- **Starter Template** (`examples/starter/`) mit Player-Logic, Asset-Loading, States
- **Prelude** exportiert alle wichtigen Typen

### Was fehlt / verbessert
1. **Getting Started Guide** -- `docs/getting-started.md` (NEU)
2. **API-Referenz** -- `cargo doc --workspace --no-deps` generieren und hosten
3. **AI Integration Guide** -- `docs/ai-integration.md` (NEU)
4. **Architecture Diagramm** -- `docs/architecture.md` (NEU)

---

## 3. AI-Integration -- Voraussetzungen beschrieben?

### Was dokumentiert ist
- **Two-Layer Architecture:** `amigo_api` (JSON-RPC IPC) + `amigo_mcp` (MCP Bridge)
- **MCP Tools** vollstaendig spezifiziert: Screenshot, State, Entities, Perf, Simulation Control, Editor
- **Art Generation Pipeline:** ComfyUI Integration mit Style-Definitions, Post-Processing
- **Audio Generation Pipeline:** ACE-Step + AudioGen mit MCP-Tools
- **AI Integration Guide:** `docs/ai-integration.md` (NEU)

### Was noch fehlt (Implementierung)
1. **Headless Mode** -- `amigo_tick(count)` fuer Fast-Forward
2. **Screenshot API** -- `amigo_screenshot()` MCP-Tool
3. **Hardware-Requirements fuer AI** -- GPU fuer ComfyUI, ACE-Step

---

## 4. Service-Kommunikation

Siehe `docs/architecture.md` fuer das vollstaendige Mermaid-Diagramm.

### Kommunikationsprotokolle

| Verbindung | Protokoll | Format | Richtung |
|-----------|-----------|--------|----------|
| Claude Code <-> MCP Servers | MCP (stdio) | JSON-RPC 2.0 | Bidirektional |
| amigo_mcp <-> amigo_api | TCP Socket | JSON-RPC 2.0 | Bidirektional |
| amigo_artgen <-> ComfyUI | HTTP REST | JSON + Binary | Request/Response |
| amigo_audiogen <-> ACE-Step | HTTP REST | JSON + WAV | Request/Response |
| Engine <-> Assets | Filesystem | PNG/ASE/WAV/RON | Watch + Reload |
| Multiplayer Clients <-> Server | UDP (laminar) | Serialized Commands | Lockstep |

---

## 5. Lizenz & Splashscreen

### Lizenz: MIT OR Apache-2.0 (bleibt)

### Splashscreen: Default "Powered by Amigo Engine"

Der Splashscreen ist als **Default-Verhalten** in die Engine eingebaut -- nicht lizenzerzwungen, aber standardmaessig aktiv. Entwickler koennen ihn per `EngineBuilder::splash(false)` deaktivieren.

**Technische Umsetzung:**
- Modul: `crates/amigo_engine/src/splash.rs`
- Zeigt "Powered by Amigo Engine" fuer 2 Sekunden beim Start
- Text wird als Pixel-Font gerendert (kein externes Asset noetig)
- `EngineBuilder::splash(bool)` zum De-/Aktivieren
- `EngineConfig.splash.enabled` in `amigo.toml`
- Default: `true`
