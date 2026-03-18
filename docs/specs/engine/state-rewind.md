# State Rewind

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** nice-to-have

## Überblick

Frame-by-Frame-Rückspulen des Spielzustands für Puzzle-Mechaniken (Braid-Style),
Level-Design-Tests und Debugging. Baut auf dem serialisierbaren GameState aus
[networking](networking.md) und [save-load](save-load.md) auf.

## Scope (tbd)

- [ ] **Ring-Buffer** für GameState-Snapshots (N Frames, konfigurierbare Tiefe)
- [ ] Komprimierte State-Snapshots (Delta-Encoding zwischen Frames)
- [ ] Rewind-API: `rewind_to(frame: u64)`, `step_back()`, `step_forward()`
- [ ] Zeitstempel-Tracking für visuelles Feedback (Rewind-Fortschrittsanzeige)
- [ ] Integration mit Fixed-Timestep ([simulation](simulation.md))
- [ ] Performance-Budget: Snapshot-Overhead pro Frame < 0.5ms
- [ ] Offene Fragen: Vollständige State-Kopie oder Diff-System? Max. Rewind-Tiefe?

## Referenzen

- Braid als Referenz-Spiel für Zeit-Mechaniken
- [engine/save-load](save-load.md) → Serialisierbarer GameState als Basis
- [engine/networking](networking.md) → GameState-Serialisierung
