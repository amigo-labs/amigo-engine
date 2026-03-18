# Puzzle Game

> **Status:** draft
> **Priorität:** nice-to-have

## Überblick

Puzzle-Spiele auf Amigo Engine: Globaler Undo-Stack, zugbasierter Tick-Modus
und Constraint-Solving. Ermöglicht Sokoban-Style, Schachpuzzle und
physikbasierte Puzzle ohne Ad-hoc-Lösungen.

## Scope (tbd)

- [ ] **Globaler Undo-Stack**: Jeder Spielerzug wird als Command gespeichert (Command-Pattern)
- [ ] **Redo**: Vorwärts-Navigation nach Undo
- [ ] **Zugbasierter Tick-Modus**: Simulation schreitet nur bei Spieleraktion fort (vs. Fixed Timestep)
- [ ] **Constraint-System**: Zug-Validierung vor Ausführung (z.B. "Kiste kann nicht in Wand")
- [ ] **Level-Completion-Check**: Automatische Bedingungsprüfung nach jedem Zug
- [ ] **Hints-System**: Optionale Tipp-Anzeige (vorberechnete Lösungsschritte)
- [ ] **Level-Editor**: Puzzle-Level in RON definieren, Editor-Unterstützung
- [ ] Integration mit [state-rewind](../engine/state-rewind.md) für Frame-genaues Undo
- [ ] Integration mit [engine/save-load](../engine/save-load.md) für Level-Fortschritt
- [ ] Offene Fragen: Automatisches Lösen via A* für Hint-Generierung?

## Referenzen

- Baba Is You: Regel-basierte Puzzle, Undo-Stack
- Sokoban: Klassisches Zug-basiertes Puzzle
- [engine/state-rewind](../engine/state-rewind.md) → Undo-Grundlage
