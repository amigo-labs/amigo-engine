# Fog of War

> **Status:** done
> **Crate:** `amigo_core`
> **Priorität:** unbedingt

## Überblick

Fog of War verdeckt Kartenbereiche, die der Spieler noch nicht erkundet hat, oder die
außerhalb des aktuellen Sichtfelds liegen. Für Tower-Defense ist dies ein Kern-Feature:
Gegner bleiben bis zum Betreten unsichtbar, was taktische Planung erfordert.

## Scope (tbd)

- [ ] Zwei Modi: **Shroud** (unexplored, permanent) und **Fog** (explored but out-of-sight)
- [ ] Sichtfeld-Berechnung per Entity (Radius, BFS-basiert analog zu Lighting)
- [ ] Tile-Sichtbarkeits-Grid (per-Tile Visibility-State: hidden / explored / visible)
- [ ] Render-Integration: Shroud als dunkle Overlay-Textur, Fog als halbtransparentes Layer
- [ ] Performanz: inkrementelle Neuberechnung bei Entity-Bewegung (Dirty-Region)
- [ ] Integration mit Pathfinding (Gegner ignorieren Fog, Spieler-KI respektiert ihn)
- [ ] Offene Fragen: Wie interagiert FoW mit Multiplayer (shared vs. per-player Fog)?

## Referenzen

- Warcraft III / StarCraft: Shroud + Fog als Two-Layer-System
- [engine/lighting](lighting.md) → BFS Flood-Fill Propagation als Vorlage
- [engine/chunks](chunks.md) → Chunk-basiertes Sichtbarkeits-Update
