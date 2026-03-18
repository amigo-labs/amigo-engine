# Review Specs — Umsetzungsplan für Partial & Gaps

> Abgeleitet aus [STATUS.md](STATUS.md). Jeder Eintrag ist eine konkrete Spec zur Umsetzung.
>
> **Priorität:** P0 = Blocker, P1 = Nächster Sprint, P2 = Mittelfristig, P3 = Langfristig

---

## RS-01 — ArcadeShooter Game Type (NEU)

**Status:** Neu | **Priorität:** P1 | **Betrifft:** `amigo_core/game_preset.rs`, `docs/genre-modules.md`

### Beschreibung

Neuer `ScenePreset::ArcadeShooter` für klassische Arcade-Shooter (Contra, Metal Slug, Gradius, Space Invaders). Unterscheidet sich von `BulletHell` durch:

- **Scrolling-Richtung:** Horizontal oder vertikal (konfigurierbar), nicht nur vertikal
- **Spieler-Waffen:** Weapon-Upgrade-System mit Power-Ups, nicht reines Dodge-Gameplay
- **Feind-Waves:** Scripted-Wave-System mit Formations-Patterns
- **Score-System:** High-Score-Tracking mit Multipliern und Combos
- **Lives-System:** Klassisches Leben-System mit Continues

### Aktivierte Systeme

```
bullet_pattern, projectile, collision, physics, economy, waves, combat
```

### Akzeptanzkriterien

- [ ] `ScenePreset::ArcadeShooter` Enum-Variant existiert
- [ ] `display_name()` → "Arcade Shooter"
- [ ] `description()` → "Scrolling shooter with power-ups and waves. Contra, Gradius, Space Invaders."
- [ ] `default_systems()` gibt `["bullet_pattern", "projectile", "collision", "physics", "economy", "waves", "combat"]` zurück
- [ ] In `all()` und `gameplay_presets()` enthalten
- [ ] `ProjectTemplate` "Arcade Shooter" mit Resolution 320×240
- [ ] Alle bestehenden Tests grün, neue Tests für ArcadeShooter-Preset

---

## RS-02 — 7-Stage Render Pipeline

**Status:** 🔧 Partial → Roadmap | **Priorität:** P1 | **Betrifft:** `amigo_render`

### Beschreibung

Spec §5/A.6 definiert 7 Render-Stages. Aktuell: Single-Pass + Post-Processing.

### Zielarchitektur

```
Stage 1: Background    — Parallax-Layer, Sky
Stage 2: Tilemap       — Tile-Layer mit Z-Sorting
Stage 3: Entities      — Sprites, sortiert nach Y oder custom Z
Stage 4: Particles     — Partikel-System
Stage 5: Lighting      — Light-Map Compositing
Stage 6: PostProcess   — Bloom, CRT, Vignette, ChromAb, ColorGrade
Stage 7: UI            — HUD-Overlay (immer on top)
```

### Akzeptanzkriterien

- [ ] `RenderStage` Enum mit 7 Varianten
- [ ] Jede Stage hat eigenen Render-Pass oder Sub-Pass
- [ ] Sprites können einem Stage zugewiesen werden
- [ ] Reihenfolge ist garantiert (Background immer zuerst, UI immer zuletzt)
- [ ] Bestehender Single-Pass-Code migriert auf neue Pipeline
- [ ] Kein Performance-Regression (Benchmark vorher/nachher)

---

## RS-03 — Per-Sprite Shaders

**Status:** 🗓 Roadmap | **Priorität:** P2 | **Betrifft:** `amigo_render`

### Beschreibung

Spec §5 listet 6 Shader-Effekte für einzelne Sprites.

### Shader-Liste

| Shader | Beschreibung | Parameter |
|--------|-------------|-----------|
| `flash` | Sprite blinkt weiß (Treffer-Feedback) | `duration_ms: u32`, `color: Color` |
| `outline` | Farbiger Pixel-Rand um Sprite | `color: Color`, `width: u8` |
| `dissolve` | Sprite löst sich in Pixel auf | `progress: f32 (0..1)`, `seed: u32` |
| `palette_swap` | Farb-Palette austauschen | `source_palette: [Color]`, `target_palette: [Color]` |
| `silhouette` | Sprite als einfarbige Silhouette | `color: Color` |
| `wave` | Sinuswellen-Verzerrung | `amplitude: f32`, `frequency: f32`, `speed: f32` |

### Akzeptanzkriterien

- [ ] `SpriteShader` Enum mit 6 Varianten
- [ ] Shader per Sprite oder per Entity setzbar
- [ ] Shader-Stack (mehrere Shader auf einem Sprite kombinierbar)
- [ ] WGSL-Shader-Code für jeden Effekt
- [ ] Funktioniert mit der 7-Stage-Pipeline (RS-02)

---

## RS-04 — Bumpalo Arena Allocator

**Status:** 🗓 Roadmap | **Priorität:** P3 | **Betrifft:** `amigo_core`, `Cargo.toml`

### Beschreibung

Spec §6 definiert bumpalo als Per-Frame Temp-Allocator für kurzlebige Daten.

### Akzeptanzkriterien

- [ ] `bumpalo` Dependency in `amigo_core/Cargo.toml`
- [ ] `FrameArena` Wrapper-Struct mit `reset()` pro Frame
- [ ] Integration in Game-Loop: Arena wird am Frame-Start ge-reset
- [ ] Mindestens 1 System nutzt Arena (z.B. Collision-Pairs, Spatial-Query-Results)
- [ ] Benchmark: Messbare Reduktion der Heap-Allokationen pro Frame

---

## RS-05 — UDP/Laminar Transport

**Status:** 🗓 Roadmap | **Priorität:** P2 | **Betrifft:** `amigo_net`

### Beschreibung

Spec §8 definiert laminar-basierte UDP-Transport für echtes Netzwerk-Multiplayer. Aktuell nur `LocalTransport`.

### Akzeptanzkriterien

- [ ] `UdpTransport` implementiert `Transport` Trait
- [ ] Server-Modus: Listen auf Port, akzeptiert Clients
- [ ] Client-Modus: Connect zu Server-IP
- [ ] Reliable + Unreliable Channels
- [ ] Integration mit bestehendem Lockstep-System
- [ ] Desync-Detection über Netzwerk
- [ ] Mindestens 2-Spieler LAN-Test funktioniert
- [ ] Feature-Flag `net-udp`

---

## RS-06 — Tier 2 Pixel UI Widgets

**Status:** 🗓 Roadmap | **Priorität:** P2 | **Betrifft:** `amigo_ui`

### Beschreibung

Spec §21 definiert Tier-2-Widgets als Pixel-UI (nicht egui). Editor nutzt egui — aber In-Game-UI soll Pixel-native Widgets haben.

### Widget-Liste

| Widget | Beschreibung |
|--------|-------------|
| `TextInput` | Einzeiliges Textfeld mit Cursor |
| `Slider` | Horizontaler Schieberegler |
| `Dropdown` | Ausklappbare Auswahlliste |
| `ColorPicker` | Einfacher Farb-Wähler (Palette-basiert) |
| `ScrollableList` | Scrollbarer Container für Listen |
| `TreeView` | Hierarchische Baumansicht |

### Akzeptanzkriterien

- [ ] Alle 6 Widgets als Pixel-UI implementiert (kein egui)
- [ ] Pixel-perfektes Rendering in virtueller Auflösung
- [ ] Keyboard- und Maus-Navigation
- [ ] Gamepad-Navigation (D-Pad)
- [ ] Konsistentes Theming (Farben, Schriftgröße konfigurierbar)

---

## RS-07 — Isometric Tilemap Rendering

**Status:** 🔧 Partial | **Priorität:** P2 | **Betrifft:** `amigo_tilemap`

### Beschreibung

`GridMode::Isometric` existiert als Enum-Variant, aber Rendering und Koordinaten-Konvertierung sind nicht getestet/implementiert.

### Akzeptanzkriterien

- [ ] `world_to_iso()` und `iso_to_world()` Konvertierungen korrekt
- [ ] Isometrisches Tile-Rendering mit korrektem Z-Sorting
- [ ] Maus-Picking auf isometrischem Grid funktioniert
- [ ] Auto-Tiling funktioniert im Isometric-Modus
- [ ] Editor unterstützt Isometric-Painting
- [ ] Mindestens 1 Beispiel/Test mit isometrischem Grid

---

## RS-08 — Chunk Streaming Tilemap

**Status:** 🗓 Roadmap | **Priorität:** P2 | **Betrifft:** `amigo_tilemap`

### Beschreibung

Spec §10 beschreibt `ChunkedTilemap` mit dynamischem Laden/Entladen basierend auf Kamera-Position.

### Akzeptanzkriterien

- [ ] `ChunkedTilemap` Struct mit konfigurierbarer Chunk-Größe
- [ ] Chunks werden automatisch geladen wenn Kamera sich nähert
- [ ] Chunks werden entladen wenn außerhalb des Load-Radius
- [ ] Async Chunk-Loading (nicht blockierend im Game-Loop)
- [ ] Chunk-Cache mit LRU-Eviction
- [ ] Nahtloses Rendering über Chunk-Grenzen
- [ ] Pathfinding über Chunk-Grenzen funktioniert

---

## RS-09 — Skeleton Animation

**Status:** 🗓 Roadmap | **Priorität:** P3 | **Betrifft:** `amigo_animation`

### Beschreibung

Spec §12 erwähnt Skelett-Animation für große Bosse und komplexe Charaktere.

### Akzeptanzkriterien

- [ ] `Skeleton` Struct: Bone-Hierarchy mit Transforms
- [ ] `SkeletonAnimation`: Keyframe-basierte Bone-Animation
- [ ] Blending zwischen Skeleton-Animationen
- [ ] Sprite-Attachment an Bones
- [ ] Import aus Spine/DragonBones JSON oder eigenem Format
- [ ] Integration mit bestehendem Animation-State-Machine

---

## RS-10 — Spatial SFX

**Status:** 🗓 Roadmap | **Priorität:** P2 | **Betrifft:** `amigo_audio`

### Beschreibung

Spec §15.1 erwähnt positionsbasierte Lautstärke-Absenkung.

### Akzeptanzkriterien

- [ ] `play_sfx_at(sfx, position)` API
- [ ] Lautstärke fällt mit Entfernung zum Listener ab (linear oder logarithmisch, konfigurierbar)
- [ ] Stereo-Panning basierend auf X-Position relativ zum Listener
- [ ] Max-Distanz: kein Sound jenseits konfigurierbarer Grenze
- [ ] Listener-Position folgt Kamera oder ist separat setzbar
- [ ] Feature-Flag `spatial-audio`

---

## RS-11 — MusicTransition: StingerThen & LayerSwap

**Status:** 🗓 Roadmap | **Priorität:** P1 | **Betrifft:** `amigo_audio`

### Beschreibung

3 von 5 `MusicTransition`-Varianten implementiert. Fehlen: `StingerThen` und `LayerSwap`.

### Akzeptanzkriterien

- [ ] `MusicTransition::StingerThen` — Spielt Stinger-Sample, dann Transition zur nächsten Section
- [ ] `MusicTransition::LayerSwap` — Tauscht einzelne Layer (Stems) einer Section aus, ohne Unterbrechung
- [ ] Beide quantisiert auf Beat/Bar (wie bestehende Transitions)
- [ ] Tests mit Mock-Audio

---

## RS-12 — Plugin::update() Lifecycle

**Status:** 🗓 Roadmap | **Priorität:** P1 | **Betrifft:** `amigo_engine`

### Beschreibung

Plugin Trait hat nur `build()` und `init()`. Spec zeigt `update()` für per-Frame Plugin-Logik.

### Akzeptanzkriterien

- [ ] `Plugin` Trait erhält `fn update(&mut self, ctx: &mut GameContext)` Methode (mit Default-Impl)
- [ ] Engine ruft `update()` für alle Plugins pro Frame auf
- [ ] Definierte Reihenfolge: Plugins laufen nach Engine-Systems, vor Render
- [ ] Bestehende Plugins kompilieren weiterhin (Default-Impl = kein Breaking Change)

---

## RS-13 — Debug F5–F8 Keys

**Status:** 🗓 Roadmap | **Priorität:** P1 | **Betrifft:** `amigo_debug`

### Beschreibung

F1–F4 sind implementiert. Fehlen: F5–F8.

### Tasten-Belegung

| Taste | Overlay | Beschreibung |
|-------|---------|-------------|
| F5 | Entity IDs | Zeigt Entity-IDs über jedem Sprite |
| F6 | Tile IDs | Zeigt Tile-IDs/Koordinaten auf der Tilemap |
| F7 | Audio Debug | Aktuell spielende Sounds, Musik-Section, Layer-States |
| F8 | Network Debug | Ping, Packet-Loss, Desync-Status, Replay-Position |

### Akzeptanzkriterien

- [ ] Alle 4 Debug-Overlays implementiert
- [ ] Toggle-Verhalten wie F1–F4 (an/aus)
- [ ] Overlays sind nur im Debug-Build verfügbar
- [ ] Kein Performance-Impact wenn deaktiviert

---

## RS-14 — Event Streaming (WebSocket)

**Status:** 🗓 Roadmap | **Priorität:** P3 | **Betrifft:** `amigo_api`

### Beschreibung

`subscribe`/`poll_events` existieren, aber Events werden gepollt statt gestreamt.

### Akzeptanzkriterien

- [ ] WebSocket-Endpoint neben JSON-RPC
- [ ] `subscribe` gibt WebSocket-URL zurück
- [ ] Events werden in Echtzeit gepusht (kein Polling nötig)
- [ ] Filter: Client kann Event-Typen abonnieren
- [ ] Reconnect-Logik bei Verbindungsabbruch
- [ ] Feature-Flag `api-websocket`

---

## RS-15 — ComfyUI HTTP Integration

**Status:** 🔧 Partial (Stubs) | **Priorität:** P2 | **Betrifft:** `amigo_artgen`

### Beschreibung

Workflow-Builder und Post-Processing funktionieren. HTTP-Client-Aufrufe zu ComfyUI sind Stubs.

### Akzeptanzkriterien

- [ ] `reqwest` oder `ureq` Dependency
- [ ] `POST /prompt` — Workflow an ComfyUI senden
- [ ] `GET /history/{prompt_id}` — Status abfragen
- [ ] `GET /view?filename=...` — Ergebnis-Bild herunterladen
- [ ] Polling-Loop mit Timeout für Generation-Status
- [ ] Fehlerbehandlung: ComfyUI offline, Queue voll, Generation fehlgeschlagen
- [ ] Alle 12 MCP-Tools liefern echte Ergebnisse statt Placeholder

---

## RS-16 — ACE-Step / AudioGen HTTP Integration

**Status:** 🔧 Partial (Stubs) | **Priorität:** P2 | **Betrifft:** `amigo_audiogen`

### Beschreibung

Client-Structs und Prompt-Building existieren. HTTP-Calls sind Stubs.

### Akzeptanzkriterien

- [ ] HTTP-Client für ACE-Step API (generate, status, download)
- [ ] HTTP-Client für AudioGen API
- [ ] Polling/Callback für lange Generationen
- [ ] Download und Speicherung der generierten Audio-Dateien
- [ ] Fehlerbehandlung: Server offline, GPU busy, Generation fehlgeschlagen

---

## RS-17 — 13 fehlende AudioGen MCP Tools

**Status:** 🗓 Roadmap | **Priorität:** P2 | **Betrifft:** `amigo_audiogen`

### Beschreibung

6 von 19 Spec-Tools implementiert. 13 fehlen.

### Fehlende Tools

| Tool | Beschreibung |
|------|-------------|
| `generate_core_melody` | Kern-Melodie für Clean-Mode Stem-Workflow |
| `generate_stem` | Einzelnen Stem generieren (Bass, Drums, etc.) |
| `generate_variation` | Variation eines bestehenden Tracks |
| `extend_track` | Track verlängern (Continuation) |
| `remix` | Track mit anderen Parametern neu generieren |
| `generate_ambient` | Ambient/Atmosphäre-Loop generieren |
| `loop_trim` | Audio auf Loop-Punkt trimmen |
| `normalize` | Audio normalisieren |
| `convert` | Format-Konvertierung (WAV→OGG, FLAC, etc.) |
| `preview` | Kurze Vorschau generieren |
| `generate_track` → `generate_music` | Naming-Fix (Spec sagt `generate_track`) |
| `list_models` | Verfügbare Modelle auflisten |
| `queue_status` | Generierungs-Queue Status |

### Akzeptanzkriterien

- [ ] Alle 13 Tools als MCP-Tools registriert
- [ ] Input-Validation für alle Parameter
- [ ] Korrekte Integration mit HTTP-Client (RS-16)
- [ ] Fehlerbehandlung pro Tool

---

## RS-18 — Style RON Files on Disk

**Status:** 🗓 Roadmap | **Priorität:** P2 | **Betrifft:** `amigo_artgen`, `amigo_audiogen`

### Beschreibung

6 Art-Styles und 6 Audio-Styles sind hardcoded. Spec definiert RON-Dateien auf Disk.

### Akzeptanzkriterien

- [ ] `styles/` Verzeichnis mit `.style.ron` Dateien für Art
- [ ] `styles/` Verzeichnis mit `.audio_style.ron` Dateien für Audio
- [ ] RON-Loader lädt und parsed Style-Definitionen vom Dateisystem
- [ ] Builtins als Default-Fallback wenn keine Dateien vorhanden
- [ ] Custom Styles können per Datei hinzugefügt werden
- [ ] Hot-Reload: Style-Änderungen werden ohne Neustart übernommen

---

## RS-19 — Clean Mode Stem Workflow

**Status:** 🗓 Roadmap | **Priorität:** P3 | **Betrifft:** `amigo_audiogen`

### Beschreibung

Quick Mode (generate + Demucs split) ist scaffolded. Clean Mode fehlt.

### Clean Mode Flow

```
1. generate_core_melody(prompt, key, bpm) → melody.wav
2. generate_stem("bass", melody_ref, prompt) → bass.wav
3. generate_stem("drums", melody_ref, prompt) → drums.wav
4. generate_stem("harmony", melody_ref, prompt) → harmony.wav
5. Mix → final_track.wav
```

### Akzeptanzkriterien

- [ ] `CleanModePipeline` Struct mit Step-Tracking
- [ ] Jeder Stem wird individuell konditioniert auf Core-Melody
- [ ] Mix-Down mit konfigurierbaren Stem-Volumes
- [ ] Fortschritts-Reporting (Step 2/5, etc.)
- [ ] Fallback auf Quick Mode wenn Clean Mode fehlschlägt

---

## RS-20 — Asset Format TOML Descriptors

**Status:** 🗓 Roadmap | **Priorität:** P3 | **Betrifft:** `amigo_assets`, neue Tool-Crate

### Beschreibung

Spec 03 definiert TOML-Deskriptoren als Zwischenschicht zwischen Raw-Assets und Engine.

### Deskriptor-Typen

| Datei | Zweck |
|-------|-------|
| `.sprite.toml` | Sprite-Metadaten (Animations, Hitbox, Origin) |
| `.tileset.toml` | Tileset-Metadaten (Tile-Size, Auto-Tile-Rules) |
| `.map.toml` | Level-Daten (Layers, Entities, Triggers) |
| `.entity.toml` | Entity-Prefabs (Components, Defaults) |
| `.palette.toml` | Farbpaletten (Hex-Colors, Ramps) |

### Akzeptanzkriterien

- [ ] TOML-Parser für jeden Deskriptor-Typ
- [ ] Loader-Integration: Engine kann `.sprite.toml` statt raw `.aseprite` laden
- [ ] Fallback: Ohne `.toml` funktioniert direktes Asset-Loading weiterhin
- [ ] `amigo build` generiert `.toml` aus vorhandenen Assets
- [ ] Dokumentation der TOML-Formate

---

## RS-21 — Runtime-Formate (.ait, WebP, OGG)

**Status:** 🗓 Roadmap | **Priorität:** P3 | **Betrifft:** `amigo_assets`, `amigo_cli`

### Beschreibung

Engine nutzt PNG + WAV direkt. Spec definiert optimierte Runtime-Formate.

### Akzeptanzkriterien

- [ ] `.ait` Format: Amigo Image Tile — vorgepackte Atlas-Tiles
- [ ] PNG → WebP Konvertierung in `amigo build`
- [ ] WAV → OGG Konvertierung in `amigo build`
- [ ] Engine lädt WebP und OGG nativ
- [ ] Größenvergleich: Messbarer Vorteil gegenüber Raw-Formaten
- [ ] Backward-Compatibility: PNG/WAV funktioniert weiterhin

---

## RS-22 — Audio Post-Processing Pipeline

**Status:** 🔧 Partial | **Priorität:** P2 | **Betrifft:** `amigo_audiogen`

### Beschreibung

`apply_loop_crossfade()` und Normalisierung existieren. Volle Pipeline fehlt.

### Akzeptanzkriterien

- [ ] BPM-Detection (Autocorrelation oder FFT-basiert)
- [ ] Bar-Snap: Audio auf nächsten Bar-Beginn trimmen
- [ ] Spectral Validation: Prüfung ob generiertes Audio zum Style passt
- [ ] Pipeline-Reihenfolge: Generate → Normalize → BPM Detect → Bar Snap → Loop Crossfade → Validate
- [ ] Fehler-Reporting wenn Validation fehlschlägt

---

## RS-23 — Amigo.toml Manifest Alignment

**Status:** 🔧 Partial | **Priorität:** P1 | **Betrifft:** `amigo_cli`

### Beschreibung

`Amigo.toml` existiert, aber Schema weicht von Spec 03 §2 ab.

### Akzeptanzkriterien

- [ ] Schema-Abgleich: Fehlende Felder aus Spec ergänzen
- [ ] Migration: Alte `Amigo.toml` werden automatisch auf neues Schema migriert
- [ ] Validierung: `amigo build` prüft Manifest gegen Schema
- [ ] Dokumentation des vollständigen Schemas

---

## RS-24 — .amigo-pak Format Alignment

**Status:** 🔧 Partial | **Priorität:** P2 | **Betrifft:** `amigo_assets/pak.rs`

### Beschreibung

PakWriter funktioniert, weicht aber vom Spec ab: keine Type-Tags, kein LZ4, keine Flags, kein SHA256.

### Akzeptanzkriterien

- [ ] Type-Tags (0x01–0x07) für Asset-Typen im Pak-Header
- [ ] LZ4 Block-Compression (Feature-Flag `pak-lz4`)
- [ ] Asset-Flags (compressed, encrypted placeholder)
- [ ] SHA256 Manifest-Hash für Integritätsprüfung
- [ ] Backward-Compatible: Alte Paks werden weiterhin gelesen
- [ ] `amigo pack --verify` prüft Pak-Integrität

---

## RS-25 — Import Pipeline (Tiled, LDTK, MML, ROM)

**Status:** 🗓 Roadmap | **Priorität:** P3 | **Betrifft:** `amigo_cli`, `amigo_assets`

### Beschreibung

Nur Aseprite-Import existiert. Spec definiert weitere Import-Formate.

### Akzeptanzkriterien

- [ ] `amigo import tiled <file.tmx>` → `.map.toml` + Tilesets
- [ ] `amigo import ldtk <file.ldtk>` → `.map.toml` + Tilesets
- [ ] `amigo import mml <file.mml>` → Audio-Pattern
- [ ] Jeder Importer als separates Modul
- [ ] Fehlerbehandlung bei inkompatiblen Features (z.B. Tiled-Features die Amigo nicht unterstützt)

---

## RS-26 — RON Music Config Loading

**Status:** 🗓 Roadmap | **Priorität:** P1 | **Betrifft:** `amigo_audio`

### Beschreibung

AdaptiveMusicEngine Runtime existiert, aber Konfiguration erfolgt nur per Code. Spec §12 definiert `.music.ron` und `.sequence.ron`.

### Akzeptanzkriterien

- [ ] `.music.ron` Parser: Definiert Sections, Layers, Transitions
- [ ] `.sequence.ron` Parser: Definiert Musik-Sequenzen und Trigger
- [ ] `AdaptiveMusicEngine::from_ron(path)` Loader
- [ ] Hot-Reload: RON-Änderungen werden live übernommen
- [ ] Dokumentation des RON-Formats mit Beispielen

---

## RS-27 — Post-Processing Pipeline Order Fix

**Status:** ⚠️ Inkonsistenz | **Priorität:** P1 | **Betrifft:** `amigo_artgen/postprocess.rs`

### Beschreibung

Spec-Reihenfolge: Downscale → Palette Clamp → AA Removal → Transparency → Outline → Tile Edge.
Code-Reihenfolge: Transparency → AA Removal → Palette Clamp → Outline.

### Akzeptanzkriterien

- [ ] Pipeline-Reihenfolge an Spec anpassen ODER Spec updaten mit Begründung
- [ ] `Downscale` Step vor Palette Clamp
- [ ] `Tile Edge Check` Step am Ende
- [ ] Dokumentierte Begründung für die gewählte Reihenfolge

---

## Zusammenfassung

| Priorität | Specs | Thema |
|-----------|-------|-------|
| **P0** | — | Keine Blocker |
| **P1** | RS-01, RS-02, RS-11, RS-12, RS-13, RS-23, RS-26, RS-27 | ArcadeShooter, Render Pipeline, Audio Transitions, Plugin Lifecycle, Debug Keys, Manifest, Music Config, Pipeline Order |
| **P2** | RS-03, RS-05, RS-06, RS-07, RS-08, RS-10, RS-15, RS-16, RS-17, RS-18, RS-22, RS-24 | Shaders, Networking, UI, Tilemap, Audio, Art/Audio Gen, Pak Format |
| **P3** | RS-04, RS-09, RS-14, RS-19, RS-20, RS-21, RS-25 | Arena Alloc, Skeleton Anim, WebSocket, Clean Mode, TOML Descriptors, Runtime Formats, Importers |
