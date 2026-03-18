# Real-Time Strategy (RTS)

> **Status:** draft
> **Priorität:** nice-to-have

## Überblick

RTS-Spiele auf Amigo Engine: Einheitenauswahl, Befehls-Queue, Formationen,
Ressourcen-Management und Fog of War. Baut auf Flow Fields ([pathfinding](../engine/pathfinding.md))
und Fog of War ([fog-of-war](../engine/fog-of-war.md)) auf.

## Scope (tbd)

- [ ] **Box-Selektion**: Rechteck-Auswahl von Einheiten per Drag
- [ ] **Unit-Commands**: Move, Attack, Patrol, Hold, Stop — als ECS-Events
- [ ] **Befehls-Queue**: Shift+Klick reiht Befehle an (Waypoint-Chain)
- [ ] **Formationen**: Linie, Keil, Block — automatisches Aufstellen beim Move-Command
- [ ] **Unit-Gruppen**: Ctrl+1-9 für benannte Einheitengruppen
- [ ] **Fog of War**: Integriert via [fog-of-war](../engine/fog-of-war.md)
- [ ] **Ressourcen-Flows**: Holz, Gold, Nahrung — Produktion und Verbrauch
- [ ] **Gebäude-Placement**: Snap-to-Grid, Bau-Fortschritt, Platzierungs-Validierung
- [ ] Integration mit [pathfinding](../engine/pathfinding.md) Flow Fields für Massen-Movement
- [ ] Integration mit [steering](../engine/steering.md) für Formation-Halten
- [ ] Offene Fragen: Wie wird Multiplayer (Lockstep) mit großen Einheitenzahlen skaliert?

## Referenzen

- StarCraft: Box-Selektion, Unit-Groups, Micro-Management
- Age of Empires: Ressourcen-Flows, Formation
- [engine/pathfinding](../engine/pathfinding.md) → Flow Fields
- [engine/fog-of-war](../engine/fog-of-war.md) → Vision
