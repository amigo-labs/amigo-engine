# Tween System

> **Status:** draft
> **Crate:** `amigo_core` (tbd)
> **Priorität:** sehr wichtig

## Überblick

Das Tween-System interpoliert Werte über Zeit mit konfigurierbaren Easing-Funktionen.
Unverzichtbar für UI-Animationen (Menü-Einblendungen), Tower-Effekte (Bounce beim Platzieren),
Kamera-Bewegungen und jede "polierte" Spielgefühl-Verbesserung.

## Scope (tbd)

- [ ] Ziel-Typen: `Fixed`, `SimVec2`, `Color`, `f32` (für visuelle Werte)
- [ ] Easing-Bibliothek: Linear, Quad, Cubic, Quart, Quint, Sine, Expo, Circ, Elastic, Back, Bounce (In/Out/InOut)
- [ ] Sequenzierung: `then()`, `delay()`, `repeat()`, `yoyo()`
- [ ] `TweenHandle` für Abbruch / Pause / Resume
- [ ] Callback bei Abschluss (`on_complete`)
- [ ] Integration mit ECS: `TweenComponent` für Entity-Properties
- [ ] Integration mit UI: Direktes Tween von UI-Element-Properties
- [ ] Fixed-Point-Kompatibilität für simulationsrelevante Tweens
- [ ] Offene Fragen: Soll Tween-State serialisierbar sein (für Save/Load)?

## Referenzen

- DOTween (Unity) als API-Vorbild
- [engine/animation](animation.md) → State-Machine als übergeordnetes System
- [engine/camera](camera.md) → Kamera-Shake und Follow nutzen ähnliche Interpolation
