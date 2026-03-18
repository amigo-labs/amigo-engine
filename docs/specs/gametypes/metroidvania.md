# Metroidvania

> **Status:** draft
> **Priorität:** nice-to-have

## Überblick

Metroidvania-Spiele auf Amigo Engine: Exploration-Graph, Ability-Gating
(Bereiche hinter Fähigkeits-Sperren), Entdeckungs-Minimap und non-lineares
Leveldesign.

## Scope (tbd)

- [ ] **Exploration-Graph**: Räume als Graph-Nodes, Verbindungen mit Gating-Conditions
- [ ] **Ability-Gating**: Türen/Wege nur mit bestimmten Fähigkeiten passierbar
- [ ] **Fähigkeiten-System**: Double Jump, Dash, Wall Jump, Grapple etc. (freischaltbar)
- [ ] **Entdeckungs-Minimap**: Räume auf Minimap enthüllen sich beim Betreten ([minimap](../engine/minimap.md))
- [ ] **Raum-Übergänge**: Screen-Transition beim Wechsel zwischen Räumen
- [ ] **Backtracking-Marking**: Markierung nicht zugänglicher Bereiche für späteres Rückkehren
- [ ] **Save Rooms / Checkpoints**: Spezielle Räume zum Speichern und Heilen
- [ ] **Boss-Räume**: Verschlossene Areale, die nach Boss-Kill geöffnet bleiben
- [ ] Integration mit [fog-of-war](../engine/fog-of-war.md) für unexplored Rooms
- [ ] Integration mit [minimap](../engine/minimap.md) für Karten-System
- [ ] Offene Fragen: Wie detailliert soll der Exploration-Graph im Editor editierbar sein?

## Referenzen

- Hollow Knight: Exploration-Graph, Ability-Gating, Minimap-Enthüllung
- Super Metroid: Room-Transitions, Backtracking
- [engine/minimap](../engine/minimap.md) → Entdeckungs-Karte
- [engine/fog-of-war](../engine/fog-of-war.md) → Unerkundete Bereiche
