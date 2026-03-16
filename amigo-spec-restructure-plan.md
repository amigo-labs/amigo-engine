# Amigo Engine – Spec-Restrukturierung

## Ziel

Die bestehenden monolithischen Spec-Dateien in eine modulare Ordnerstruktur aufteilen. Jede Spec soll eigenständig lesbar und implementierbar sein, ohne dass der Agent 800+ Zeilen Kontext laden muss.

---

## Quell-Dateien

Die folgenden Dateien existieren aktuell und enthalten den gesamten Inhalt, der aufgeteilt werden soll:

| Datei | Inhalt |
|-------|--------|
| `amigo-engine-complete.md` | Engine-Spec v2.0, 27 Sections (Core, Rendering, Audio, Input, Tilemap, Pathfinding, Animation, Camera, UI, Networking, Memory, Plugins, Assets, Editor, CLI, Debug, AI Pipelines, Config, Starter Template) |
| `amigo-td-spec.md` | Tower Defense Game Design v2.0 (6 Welten, Towers, Enemies, Waves, Economy, Balance) |
| `amigo-asset-format-spec.md` | Asset-Formate (TOML-Specs, .amigo-pak, Import/Export, Audio Patterns, Instrument Banks, Build Pipeline) |

Falls weitere Spec-Dateien existieren (UI-Spec, Artgen-Spec, Audiogen-Spec), diese ebenfalls einordnen.

---

## Ziel-Struktur

```
docs/specs/
├── _index.md
├── _conventions.md
│
├── engine/
│   ├── core.md
│   ├── rendering.md
│   ├── audio.md
│   ├── input.md
│   ├── tilemap.md
│   ├── pathfinding.md
│   ├── animation.md
│   ├── camera.md
│   ├── ui.md
│   ├── networking.md
│   ├── memory-performance.md
│   └── plugin-system.md
│
├── assets/
│   ├── format.md
│   ├── pipeline.md
│   └── atlas.md
│
├── tooling/
│   ├── cli.md
│   ├── editor.md
│   └── debug.md
│
├── ai-pipelines/
│   ├── artgen.md
│   ├── audiogen.md
│   └── agent-api.md
│
├── games/
│   └── td/
│       ├── design.md
│       └── ui.md
│
└── config/
    ├── amigo-toml.md
    └── data-formats.md
```

---

## Mapping: Quelle → Ziel

### Aus `amigo-engine-complete.md`

| Section(s) in Quelle | Ziel-Datei | Hinweise |
|----------------------|------------|----------|
| §1 Vision & Philosophy | `_index.md` | Wird Teil der Übersicht |
| §2 Tech Stack | `_index.md` | Dependency-Übersicht |
| §3 Architecture Overview | `_index.md` | Workspace/Crate-Struktur, Dependency Graph |
| §4 Core Types, Math & ECS | `engine/core.md` | |
| §5 Rendering Pipeline | `engine/rendering.md` | |
| §6 Memory & Performance | `engine/memory-performance.md` | |
| §7 API Design (Game Trait, Builder) | `engine/core.md` | Gehört zum Core-Vertrag |
| §8 Command System & Networking | `engine/networking.md` | |
| §9 Asset Pipeline | `assets/pipeline.md` | |
| §10 Tilemap System | `engine/tilemap.md` | |
| §11 Pathfinding | `engine/pathfinding.md` | |
| §12 Animation System | `engine/animation.md` | |
| §13 Camera System | `engine/camera.md` | |
| §14 Input System | `engine/input.md` | |
| §15 Audio System | `engine/audio.md` | |
| §15b Audio Generation Pipeline | `ai-pipelines/audiogen.md` | |
| §16 Level Editor | `tooling/editor.md` | |
| §16b Art Studio | `tooling/editor.md` | Anhängen als Subsection |
| §16c Art Generation Pipeline | `ai-pipelines/artgen.md` | |
| §17 AI Agent Interface | `ai-pipelines/agent-api.md` | |
| §18 Debug & Profiling | `tooling/debug.md` | |
| §19 Build & Distribution | `tooling/cli.md` | |
| §20 Plugin System | `engine/plugin-system.md` | |
| §21 UI System | `engine/ui.md` | |
| §22 Error Handling & Logging | `_conventions.md` | Cross-cutting concern |
| §23 Configuration | `config/amigo-toml.md` + `config/data-formats.md` | Aufteilen nach Dateityp |
| §24 Starter Template | `tooling/cli.md` | Teil von `amigo new` |
| §25 Game-Specific Design | Entfällt | Sind nur Referenzen auf game specs |
| §26 Implementation Phases | `_index.md` | Roadmap-Abschnitt |
| §27 Key Decisions Summary | `_index.md` | Decisions-Tabelle |
| Appendix | `_conventions.md` oder inline | Design Rationale verteilen |

### Aus `amigo-td-spec.md`

| Inhalt | Ziel-Datei |
|--------|------------|
| Gesamte TD-Spec | `games/td/design.md` |
| Falls UI-Sections enthalten | `games/td/ui.md` extrahieren |

### Aus `amigo-asset-format-spec.md`

| Inhalt | Ziel-Datei |
|--------|------------|
| TOML-Format-Definitionen (Sprite, Tilemap, Palette, etc.) | `assets/format.md` |
| Audio-Formate (Pattern, Instrument Bank, Song) | `assets/format.md` (oder `engine/audio.md` referenzieren) |
| Import/Export (Tiled, Aseprite, LDTK, MML, VGM) | `assets/format.md` |
| Build Pipeline (.amigo-pak) | `assets/pipeline.md` |
| Atlas/Spritesheet-Packing | `assets/atlas.md` |

---

## Spec-Template

Jede neue Spec-Datei bekommt diesen Header:

```markdown
# [Modulname]

> Status: draft | ready | implementing | stable
> Crate: amigo_[name] (falls zutreffend)
> Depends on: [Liste anderer Specs, z.B. engine/core]
> Last updated: [Datum]

## Zweck

Was macht dieses Modul und warum existiert es.

## Public API

Traits, Structs, Enums – der Vertrag nach außen.
Code-Blöcke mit Rust-Signaturen.

## Verhalten

Wie verhält sich das Modul. Invarianten, Edge Cases, Lifecycles.

## Internes Design

Implementierungsdetails, Algorithmen, Datenstrukturen.
(Nur wenn relevant für Verständnis, nicht als Implementierungsvorgabe.)

## Nicht-Ziele

Was explizit nicht in Scope ist.

## Offene Fragen

Was noch geklärt werden muss.
```

---

## Sonderdateien

### `_index.md`

```markdown
# Amigo Engine – Spec-Übersicht

## Vision
[Aus §1 der Engine-Spec]

## Tech Stack
[Aus §2: Rust, wgpu, kira, egui, etc.]

## Architektur
[Aus §3: Workspace-Struktur, Crate-Graph]

## Dependency Graph
[Mermaid-Diagramm: welche Spec hängt von welcher ab]

## Status-Tabelle
| Spec | Status | Crate | Depends on |
|------|--------|-------|------------|
| engine/core | draft | amigo_core | – |
| engine/rendering | draft | amigo_render | engine/core |
| ... | | | |

## Implementation Phases
[Aus §26: Phase 0-8 Timeline]

## Key Decisions
[Aus §27: Entscheidungstabelle]
```

### `_conventions.md`

```markdown
# Amigo Engine – Konventionen

## Rust Patterns
- Error Handling: thiserror für Library-Errors, anyhow für Binaries
- Logging: tracing mit span-basiertem Context
- [Weitere Patterns aus §22 und Appendix]

## Spec-Konventionen
- Jede Spec nutzt das Template aus diesem Dokument
- Status-Flow: draft → ready → implementing → stable
- Eine Spec ist "ready" wenn alle Dependencies mindestens "ready" sind
- Public API Section ist der Vertrag: implementiere genau das, nicht mehr

## Naming
- Crates: snake_case mit amigo_ Prefix
- Traits: PascalCase, beschreibend (AudioMixer, TileRenderer)
- Config-Dateien: kebab-case.toml / kebab-case.ron
```

---

## Arbeitsanweisungen

### Reihenfolge

1. Ordnerstruktur anlegen
2. `_index.md` und `_conventions.md` als erstes erstellen
3. Engine-Specs extrahieren (engine/ Ordner) – größter Block
4. Asset-Specs extrahieren (assets/ Ordner)
5. Tooling-Specs extrahieren (tooling/ Ordner)
6. AI-Pipeline-Specs extrahieren (ai-pipelines/ Ordner)
7. Game-Specs verschieben (games/ Ordner)
8. Config-Specs extrahieren (config/ Ordner)
9. Querverweise prüfen und aktualisieren

### Regeln

- **Kein Inhalt geht verloren.** Jeder Satz aus den Quell-Dateien muss in genau einer Ziel-Datei landen.
- **Keine Duplikation.** Wenn etwas in mehreren Specs relevant ist, in einer Spec definieren und aus den anderen referenzieren: `→ Siehe [engine/core](../engine/core.md)`.
- **Cross-References immer als relative Links.** Format: `[Anzeigename](../ordner/datei.md)`.
- **Spec-Template anwenden.** Jede Datei bekommt den Header mit Status, Crate, Dependencies.
- **Inhaltlich nichts ändern.** Kein Refactoring der Specs selbst, nur Aufteilung und Formatierung. Wenn etwas unklar zugeordnet ist, im Zweifel in die thematisch nächste Datei und eine Notiz in "Offene Fragen" hinterlassen.
- **Quelldateien behalten.** Die Originale nicht löschen, sondern nach `docs/specs/_archive/` verschieben, bis die Restrukturierung verifiziert ist.

### Verifikation

Nach Abschluss:

1. Alle Dateien in der Zielstruktur müssen dem Template entsprechen
2. Alle Sections aus den Quelldateien müssen zugeordnet sein (Checkliste gegen Mapping-Tabelle)
3. Alle relativen Links müssen funktionieren
4. `_index.md` Status-Tabelle muss alle Specs auflisten
5. Dependency Graph in `_index.md` muss konsistent mit den `Depends on`-Feldern der einzelnen Specs sein
