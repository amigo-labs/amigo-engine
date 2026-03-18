# Bullet Patterns

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** sehr wichtig

## Überblick

Deklarative Schussmuster-Beschreibung (Bullet Pattern DSL) für Türme und Bosse.
Ermöglicht Bullet-Hell-Muster, Spread-Shots, Spiralen und zeitgesteuerte
Salven ohne hardgecodete Spawn-Logik.

## Scope (tbd)

- [ ] **Pattern-Typen**: Single, Spread (N-Way), Ring, Spiral, Burst, Aimed, Random-Cone
- [ ] **DSL in RON**: Pattern-Dateien beschreiben Winkel, Timing, Projektil-Typen
- [ ] **Sequenzierung**: Mehrere Patterns in Phasen (Boss-Phase 1 → Phase 2)
- [ ] `BulletPatternEmitter` Component mit Pattern-Handle
- [ ] Parametrisierung: Geschwindigkeit, Projektil-Typ, Schaden aus Pattern-Datei
- [ ] Integration mit [engine/particles](particles.md) für visuelle Mündungsfeuer-Effekte
- [ ] Integration mit Boss-KI (Pattern-Wechsel bei Lebens-Schwellen)
- [ ] Offene Fragen: Wie granular soll die DSL sein? Scripted vs. pure data?

## Referenzen

- Touhou Project / Ikaruga als Bullet-Hell-Referenz
- [engine/simulation](simulation.md) → Fixed Timestep für deterministisches Spawning
- [gametypes/shmup](../gametypes/shmup.md) → Hauptabnehmer
