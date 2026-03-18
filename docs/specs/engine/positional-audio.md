# Positional Audio

> **Status:** draft
> **Crate:** `amigo_audio`
> **Priorität:** sehr wichtig

## Überblick

Positional Audio berechnet Lautstärke und Pan basierend auf der 2D-Distanz zwischen
Sound-Quelle und Zuhörer (typisch: Kamera-Zentrum). Fehlt aktuell in `amigo_audio`,
das nur nicht-räumliche Wiedergabe unterstützt.

## Scope (tbd)

- [ ] `SpatialAudioSource` Component mit `world_pos: SimVec2`, `max_distance: Fixed`
- [ ] Distance Falloff Kurven: Linear, Inverse Square, Custom
- [ ] Stereo Panning basierend auf relativer X-Position zur Kamera
- [ ] Listener-Position aus aktiver Kamera ableiten
- [ ] Kombination mit Mixer-Gruppen aus [engine/audio](audio.md)
- [ ] Doppler-Effekt (optional, low-priority)
- [ ] Offene Fragen: Occlusion (Wände dämpfen Klang)? Reverb-Zonen?

## Referenzen

- [engine/audio](audio.md) → Kira-Wrapper als Basis
- [engine/camera](camera.md) → Listener-Position
- FMOD / Wwise Spatial Audio als Feature-Referenz
