# City Builder

> **Status:** draft
> **Priorität:** nice-to-have

## Überblick

City Builder / Management-Spiele auf Amigo Engine: Ressourcen-Flows als Graph,
Zoning-System, Happiness-Aggregation und infrastrukturbasiertes Wirtschaftssystem.

## Scope (tbd)

- [ ] **Ressourcen-Flows**: Gerichteter Graph — Produzenten → Lager → Konsumenten
- [ ] **Zoning**: Wohn-, Gewerbe-, Industrie-Zonen als Tilemap-Layer
- [ ] **Straßen-Netz**: Routing von Ressourcen über Straßen (Graph-Traversal)
- [ ] **Happiness-Aggregation**: Gebäude-Zufriedenheit aus mehreren Faktoren (Lärm, Services, Jobs)
- [ ] **Bevölkerungs-Simulation**: Einwohner mit Bedürfnissen (Arbeit, Wohnen, Entertainment)
- [ ] **Bau-System**: Platzieren und Entfernen von Gebäuden ([engine/tilemap](../engine/tilemap.md))
- [ ] **Statistik-Overlay**: Ressourcen-Flows, Happiness-Map als Farb-Overlay
- [ ] **Katastrophen**: Feuer, Überschwemmung als Event-Typen
- [ ] Integration mit [procedural](../engine/procedural.md) für Karten-Generierung
- [ ] Integration mit [engine/chunks](../engine/chunks.md) für große Welten
- [ ] Offene Fragen: Wie granular ist die Bevölkerungs-Simulation? Agent-basiert oder aggregiert?

## Referenzen

- SimCity 2000 / Cities: Skylines als Genre-Referenz
- [engine/chunks](../engine/chunks.md) → Große Karten
- [engine/procedural](../engine/procedural.md) → Karten-Generierung
