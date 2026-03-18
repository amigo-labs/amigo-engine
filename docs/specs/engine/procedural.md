# Procedural Generation

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** sehr wichtig

## Überblick

Prozedurale Generierung ermöglicht Sandbox-Level-Erstellung, Roguelike-Dungeons
und organische Terrain-Variation. Kernkomponenten sind eine Noise-API (Simplex/Perlin)
und Wave Function Collapse (WFC) für tilemap-basierte Levelgenerierung.

## Scope (tbd)

- [ ] **Noise-API**: Simplex Noise 2D/3D, Perlin Noise, Fractal Brownian Motion (fBm)
- [ ] **Seed-System**: Reproduzierbare Generierung via `u64`-Seed
- [ ] **Wave Function Collapse (WFC)**: Constraint-basierte Tilemap-Generierung
- [ ] WFC-Constraint-Definition in RON (Tile-Nachbarschaftsregeln)
- [ ] **Room-and-Corridor**: Klassischer Dungeon-Generator als High-Level-API
- [ ] **Biome-Zuordnung**: Noise-basiertes Biome-Mapping für Overworld-Generierung
- [ ] Fixed-Point-Kompatibilität für deterministischen Noise bei gleichem Seed
- [ ] Offene Fragen: Wie tief ist WFC-Integration mit Tilemap-System?

## Referenzen

- [engine/dynamic-tilemap](dynamic-tilemap.md) → Tilemap als Generierungsziel
- [gametypes/roguelike](../gametypes/roguelike.md) → Hauptabnehmer für Dungeon-Gen
- fast_noise_lite (Rust port) als potentielle Dependency
