# Minimap

> **Status:** draft
> **Crate:** `amigo_render` (tbd)
> **Priorität:** nice-to-have

## Überblick

Fertiges Minimap-Subsystem als abstrahierte Karten-Ansicht der Spielwelt.
Zeigt Tilemap, Entities und Points of Interest in reduzierter Auflösung.
Die [camera](camera.md)-Spec erwähnt bereits ein zweites Viewport — dieses System
baut darauf auf und fügt Minimap-spezifische Logik hinzu.

## Scope (tbd)

- [ ] Minimap-Render-Texture via zweitem Kamera-Viewport ([camera](camera.md))
- [ ] Konfigurierbarer Maßstab und Position (HUD-Element)
- [ ] **Entity-Pins**: Farbige Marker für Spieler, Feinde, Türme, Waypoints
- [ ] **Fog-of-War-Integration**: Unerkundete Bereiche ausgeblendet ([fog-of-war](fog-of-war.md))
- [ ] Klickbare Minimap für Kamera-Sprung (optional)
- [ ] Custom-Styling: Rahmen, Hintergrundfarbe, Pin-Sprites
- [ ] Integration mit [engine/ui](ui.md) als UI-Widget
- [ ] Offene Fragen: Tile-basiertes Rendering oder Sprite-Downscaling?

## Referenzen

- [engine/camera](camera.md) → Minimap Camera Viewport
- [engine/fog-of-war](fog-of-war.md) → Sichtbarkeits-Masking
- [engine/ui](ui.md) → Minimap als HUD-Element
