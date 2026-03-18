# Shoot'em Up (Shmup)

> **Status:** draft
> **Priorität:** sehr wichtig

## Überblick

Shoot'em Up / Bullet Hell auf Amigo Engine. Definiert Kollisions-Präzision,
Bullet-Pattern-DSL, Hitbox-Visualisierung und das Rank-System für adaptiven
Schwierigkeitsgrad.

## Scope (tbd)

- [ ] **Bullet Hell Kollision**: Kreisförmige Hitboxen, deutlich kleiner als Sprite (Grazebox)
- [ ] **Grazing**: Kugeln nah am Spieler passieren lassen gibt Bonus (Reward-Loop)
- [ ] **Bullet-Pattern-DSL**: Deklarative Muster via [bullet-patterns](../engine/bullet-patterns.md)
- [ ] **Rank-System (Dynamische Schwierigkeit)**: Rank steigt bei Gut-Spielen, sinkt bei Treffer
- [ ] **Bomb System**: Panic-Button räumt Bildschirm (Invincibility-Frames)
- [ ] **Scroll-Modi**: Vertikal, Horizontal, Mehrdirektional, Fixed Screen
- [ ] **Deathbomb**: Kurzes Fenster nach Treffer zum Bomben und Überleben
- [ ] **Extend Lives**: Leben-Punkte-Schwellen, 1-Up-Items
- [ ] Hitbox-Visualisierung im Debug-Modus ([engine/debug](../tooling/debug.md))
- [ ] Integration mit [bullet-patterns](../engine/bullet-patterns.md)
- [ ] Offene Fragen: Wie komplex wird die Rank-Formel? Sub-Pixel-Bewegung nötig?

## Referenzen

- Touhou Project: Bullet-Hell, Grazing, Rank-System
- DoDonPachi: Dense Bullet Patterns, Rank-Mechanik
- [engine/bullet-patterns](../engine/bullet-patterns.md) → Pattern-DSL
