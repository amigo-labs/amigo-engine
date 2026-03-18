# Roguelike

> **Status:** draft
> **Priorität:** sehr wichtig

## Überblick

Roguelike-Spiele auf Amigo Engine: Permadeath, prozedurale Dungeon-Generierung,
Seed-basierte Reproduzierbarkeit und Meta-Progression zwischen Runs.

## Scope (tbd)

- [ ] **Permadeath**: Tod = Zurück zum Start (konfigurierbar: Hard / Soft Permadeath)
- [ ] **Seed-System**: Jeder Run hat einen `u64`-Seed für reproduzierbare Level
- [ ] **Dungeon-Generierung**: Room-and-Corridor via [procedural](../engine/procedural.md)
- [ ] **Meta-Progression**: Unlocks, permanente Upgrades zwischen Runs (persistiert via [save-load](../engine/save-load.md))
- [ ] **Item-/Relic-System**: Synergien zwischen Items, zufällige Drops
- [ ] **Floor-Progression**: Eskalierender Schwierigkeitsgrad, Boss-Floors
- [ ] **Cursed Items / Risk-Reward**: Items mit positiven und negativen Effekten
- [ ] **Run-Statistiken**: Tod-Grund, Erreichter Floor, Gesammelte Items (für Post-Mortem)
- [ ] Integration mit [procedural](../engine/procedural.md) für Level-Gen
- [ ] Integration mit [achievements](../engine/achievements.md) für Run-Milestones
- [ ] Offene Fragen: Turn-based vs. Echtzeit? Wie tief ist Meta-Progression?

## Referenzen

- Hades: Meta-Progression, Narrative-Integration, Echtzeit
- Dead Cells: Roguelite mit Platformer-Elementen
- [engine/procedural](../engine/procedural.md) → Dungeon-Generierung
- [engine/save-load](../engine/save-load.md) → Meta-Progression-Persistenz
