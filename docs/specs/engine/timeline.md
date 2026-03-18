# Timeline / Cutscene System

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** nice-to-have

## Überblick

Keyframe-basiertes Timeline-System für Cutscenes und geskriptete Sequenzen.
Ermöglicht zeitgesteuerte Kamera-Bewegungen, Dialogue-Trigger, Animations-Wechsel
und Sound-Events ohne Code in einer deklarativen Datei.

## Scope (tbd)

- [ ] Timeline-Dateiformat in RON: Tracks mit Keyframes und Timestamps
- [ ] **Track-Typen**: Camera-Path, Dialogue, Animation-Override, Sound, Entity-Spawn
- [ ] Keyframe-Interpolation zwischen Werten (via [tween](tween.md))
- [ ] Timeline-Player mit Play/Pause/Stop/Seek
- [ ] Skipbare Cutscenes (Input-Interrupt)
- [ ] Integration mit Scene-Stack: Timeline als eigene Scene oder Overlay
- [ ] Editor-Visualisierung der Timeline (tbd)
- [ ] Offene Fragen: Wie komplex sollen Bedingungen innerhalb von Timelines sein?

## Referenzen

- Unity Timeline / Godot AnimationPlayer als API-Referenz
- [engine/tween](tween.md) → Interpolation
- [engine/dialogue](dialogue.md) → Dialogue-Tracks
- [engine/camera](camera.md) → Kamera-Bewegungen
