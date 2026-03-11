# Amigo Engine — Spezifikationen

Planungsdokumente für die Amigo Engine, eine Modern Pixel Art Game Engine in Pure Rust, und ihr erstes Spiel (Tower Defense mit 6 thematischen Welten).

Diese Dokumente sind die Single Source of Truth für Architektur, Gameplay, UI und Asset-Pipelines.

---

## Dokumente

| # | Datei | Beschreibung | Status |
|---|-------|-------------|--------|
| 1 | [01-engine-spec.md](01-engine-spec.md) | Engine-Architektur: Core, ECS, Renderer, Tilemap, Audio Runtime, Networking, AI API, Editor, CLI, zukünftige Systeme | Aktiv |
| 2 | [02-td-spec.md](02-td-spec.md) | Tower Defense Game Design + UI/UX: Welten, Economy, Upgrades, Waves, HUD, Controls, Accessibility | Aktiv |
| 3 | [03-asset-pipeline-spec.md](03-asset-pipeline-spec.md) | AI Asset-Generierung: Art Pipeline (ComfyUI), Audio Pipeline (ACE-Step), Adaptive Music, MCP Tools | Aktiv |

## Lesereihenfolge

1. **01-engine-spec** — Grundverständnis der Engine-Architektur
2. **02-td-spec** — Erstes Spiel, nutzt die Engine
3. **03-asset-pipeline-spec** — Asset-Generierung (Art + Audio)

## Abhängigkeiten

```
01-engine-spec
├── 02-td-spec (nutzt Engine-Features: ECS, Tilemap, Audio, Input, UI)
└── 03-asset-pipeline-spec (MCP Tools, Post-Processing, Adaptive Music Runtime)
```

## Konventionen

- Alle Specs verwenden dieselbe Nummerierung: `§N Titel`
- Code-Beispiele in Rust, Datenformate in RON/TOML
- Querverweise am Ende jedes Dokuments
- Englische Fachbegriffe bleiben im Original
