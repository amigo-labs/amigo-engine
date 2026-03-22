---
status: done
crate: amigo_core
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Fog of War

> Erstellt via `/thanos:spec` — 2026-03-18

## Kontext

Tower-Defense und Strategie-Spiele benötigen ein Sichtbarkeits-System, das
Kartenbereiche verbirgt, bis sie vom Spieler erkundet werden. Gegner bleiben
unsichtbar, bis sie in den Sichtradius einer eigenen Einheit geraten —
das erzwingt taktische Planung und schafft Spannung.

Das System besteht aus zwei dauerhaften Zuständen:
- **Hidden**: Tile wurde noch nie gesehen (Shroud — dunkle Abdeckung)
- **Explored**: Tile war einmal sichtbar, liegt aber außerhalb des aktuellen
  Sichtfelds (Fog — halbtransparente Abdeckung)
- **Visible**: Tile liegt innerhalb des aktiven Sichtradius einer Einheit

## Scope

### In Scope

- `TileVisibility`-Enum mit drei Zuständen: `Hidden`, `Explored`, `Visible`
- `FogOfWarGrid`: Flaches 2D-Grid über Tile-Sichtbarkeits-Werte
- `update_visibility(observer_pos, radius, grid)`: BFS-basierte Sichtberechnung
- Serde-Support für Save/Load-Integration
- Vollständige Unit-Tests (5 Szenarien)

### Out of Scope

- Rendering (Overlay-Texturen, Shader) — gehört in `amigo_render`
- ECS-Integration — ist Spielcode-Aufgabe
- Line-of-Sight / Raycast-Blockierung durch undurchsichtige Tiles
- Multiplayer-Synchronisation (shared vs. per-player Fog)
- Inkrementelle Dirty-Region-Updates (Optimierung für spätere Phase)
- Integration mit Pathfinding-Systemen

## Akzeptanzkriterien

- [ ] AC1: `FogOfWarGrid::new(w, h)` — alle Tiles starten als `Hidden`
- [ ] AC2: `update_visibility` setzt alle Tiles im BFS-Radius auf `Visible`
- [ ] AC3: Tiles die vorher `Visible` waren, werden nach einem Update außerhalb
       des neuen Radius zu `Explored` — nicht zurück zu `Hidden`
- [ ] AC4: Tiles außerhalb des Radius bleiben `Hidden` (nie `Explored` durch
       ein einziges Update ohne vorherigen `Visible`-Zustand)
- [ ] AC5: `visibility_at(-1, -1)` und andere out-of-bounds-Koordinaten
       paniken nicht, sondern geben `Hidden` zurück
- [ ] AC6: Kein `unsafe`-Code, kein `f32` in der Logik
- [ ] AC7: Alle 5 Tests grün unter `cargo test -p amigo_core -- fog`

## Technische Notizen

### Algorithmus

BFS vom Observer-Tile aus über Manhattan-/Chebyshev-Nachbarn. Chebyshev-Distanz
(Schachbrett-Abstand) wird als Radius-Maßstab verwendet: ein Tile (dx, dy) ist
sichtbar wenn `max(|dx|, |dy|) <= radius`. Das ist deterministisch, integer-
basiert und vermeidet jegliche Floating-Point-Arithmetik.

Ablauf von `update_visibility`:
1. Alle aktuell `Visible`-Tiles auf `Explored` downgraden
2. BFS vom Observer, alle Tiles innerhalb Chebyshev-Radius auf `Visible` setzen

### Datenstruktur

```rust
// Flaches Vec<TileVisibility>, row-major: index = y * width + x
pub struct FogOfWarGrid {
    data: Vec<TileVisibility>,
    width: u32,
    height: u32,
}
```

Kein HashMap, kein Sparse-Speicher — das Grid deckt eine feste Kartengröße ab.
Typische Kartengröße: 64×64 bis 256×256 Tiles = 4 KB bis 64 KB.

### Betroffene Dateien

| Datei | Änderung |
| ----- | -------- |
| `crates/amigo_core/src/fog_of_war.rs` | Neues Modul (vollständig neu) |
| `crates/amigo_core/src/lib.rs` | `pub mod fog_of_war;` + Re-Exports hinzufügen |
| `docs/specs/engine/fog-of-war.md` | Status: draft → done |
| `docs/specs/index.md` | fog-of-war: draft → done |
| `specs/active/fog-of-war.md` | Diese Datei (neu) |
