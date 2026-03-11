# Amigo Engine — Dokumentation

Modern Pixel Art Game Engine in Pure Rust.

---

## Spezifikationen

| # | Datei | Beschreibung | Status |
|---|-------|-------------|--------|
| 1 | [plans/01-engine-spec.md](plans/01-engine-spec.md) | Engine-Architektur: Core, ECS, Renderer, Tilemap, Audio Runtime, Networking, AI API, Editor, CLI, zukünftige Systeme | Aktiv |
| 2 | [plans/02-asset-pipeline-spec.md](plans/02-asset-pipeline-spec.md) | AI Asset-Generierung: Art Pipeline (ComfyUI), Audio Pipeline (ACE-Step), Adaptive Music, MCP Tools | Aktiv |

## Lesereihenfolge

1. **01-engine-spec** — Grundverständnis der Engine-Architektur
2. **02-asset-pipeline-spec** — Asset-Generierung (Art + Audio)

## Abhängigkeiten

```
01-engine-spec
└── 02-asset-pipeline-spec (MCP Tools, Post-Processing, Adaptive Music Runtime)
```

## Konventionen

- Alle Specs verwenden dieselbe Nummerierung: `§N Titel`
- Code-Beispiele in Rust, Datenformate in RON/TOML
- Querverweise am Ende jedes Dokuments
- Englische Fachbegriffe bleiben im Original
