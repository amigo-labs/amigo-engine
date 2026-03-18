# Platformer / Jump'n'Run

> **Status:** draft
> **Priorität:** sehr wichtig

## Überblick

Spezifikation für Platformer-Spiele auf Basis des Amigo Engine. Definiert die
Kern-Spielmechaniken, die gutes Spielgefühl im Platformer-Genre ausmachen:
präzise Steuerung, Coyote Time, Jump Buffer und flüssige Physik.

## Scope (tbd)

- [ ] **Coyote Time**: Sprung noch kurz nach dem Verlassen einer Plattform möglich (≈6 Frames)
- [ ] **Jump Buffer**: Sprung-Input wird gepuffert (~8 Frames) für responsives Gefühl
- [ ] **Variable Jump Height**: Kurzes Drücken = kleiner Sprung, langes Drücken = großer Sprung
- [ ] **Wall Jump**: Sprung von Wänden mit konfigurierbarem Winkel und Kraft
- [ ] **Wall Slide**: Langsames Gleiten an Wänden (Friction-Reduktion)
- [ ] **Dash**: Horizontaler/Diagonaler Dash-Move (Celeste-Style)
- [ ] **Coyote-Dash**: Dash auch kurz nach Plattformverlassen möglich
- [ ] Plattform-Typen: Solid, One-Way (von unten durchdringbar), Moving, Crumbling
- [ ] Integration mit [physics](../engine/physics.md) oder AABB-Kollision
- [ ] Integration mit [tween](../engine/tween.md) für Squash & Stretch
- [ ] Offene Fragen: Rapier2D-Physik oder custom Kinematic-Controller?

## Referenzen

- Celeste (Maddy Thorson): Coyote Time, Jump Buffer, Dash
- Super Mario Bros: Variable Jump Height
- [engine/physics](../engine/physics.md) → Kinematic Rigid Body
- [engine/steering](../engine/steering.md) → NPC-Bewegung
