---
status: spec
crate: amigo_render
depends_on: ["engine/camera", "engine/fog-of-war"]
last_updated: 2026-03-18
---

# Minimap

## Purpose

Abstrahierte Karten-Ansicht der Spielwelt als HUD-Element. Zeigt Tilemap, Entity-Pins und Fog-of-War in reduzierter Auflösung. Wichtig für Tower Defense (Übersicht über Gegnerwellen), RTS (strategische Planung) und Metroidvania (Erkundungsfortschritt).

## Existierende Bausteine

- `Camera` mit Zoom + Viewport in `crates/amigo_render/src/camera.rs` — Minimap nutzt eigene Camera-Instanz mit niedrigem Zoom
- `PostProcessor` mit Offscreen Render Targets in `crates/amigo_render/src/post_process.rs` — Minimap rendert in eigene Texture
- `FogOfWarGrid` mit `TileVisibility` in `crates/amigo_core/src/fog_of_war.rs` — Sichtbarkeitsdaten für Minimap-Overlay
- `UiContext` Immediate-Mode in `crates/amigo_ui/src/lib.rs` — Minimap als UI-Widget integriert

## Public API

### MinimapConfig

```rust
/// Konfiguration für Minimap-Darstellung und -Position.
#[derive(Clone, Debug)]
pub struct MinimapConfig {
    /// Position auf dem Bildschirm (Pixel, relativ zu top-left).
    pub screen_pos: RenderVec2,
    /// Größe in Pixel auf dem Bildschirm.
    pub size: (u32, u32),
    /// Welcher Weltbereich wird dargestellt (in Tiles).
    pub world_bounds: Rect,
    /// Stil-Optionen.
    pub style: MinimapStyle,
    /// Ob Klick-auf-Minimap die Kamera bewegen soll.
    pub click_to_jump: bool,
}

#[derive(Clone, Debug)]
pub struct MinimapStyle {
    /// Hintergrundfarbe für unerkundete Bereiche.
    pub background_color: Color,
    /// Rahmenfarbe (None = kein Rahmen).
    pub border_color: Option<Color>,
    /// Rahmenbreite in Pixel.
    pub border_width: u32,
    /// Farbe des Kamera-Viewport-Indikators.
    pub viewport_indicator_color: Color,
    /// Fog-of-War Farben.
    pub fog_hidden_color: Color,      // Shroud (unexplored)
    pub fog_explored_color: Color,    // Explored but not visible
}
```

### MinimapPin

```rust
/// Ein Marker auf der Minimap für eine Entity oder einen Point of Interest.
#[derive(Clone, Debug)]
pub struct MinimapPin {
    /// Welche Entity dieser Pin trackt (None = statischer Pin).
    pub entity: Option<EntityId>,
    /// Feste Position (nur wenn entity = None).
    pub static_pos: Option<SimVec2>,
    /// Darstellung.
    pub pin_type: PinType,
    /// Sichtbar auch im Fog-of-War? (z.B. für eigene Türme).
    pub always_visible: bool,
}

#[derive(Clone, Debug)]
pub enum PinType {
    /// Farbiger Punkt (1-3 Pixel je nach Minimap-Größe).
    Dot { color: Color },
    /// Kleines Sprite (z.B. Turm-Icon, Boss-Schädel).
    Sprite { name: String },
    /// Richtungspfeil am Minimap-Rand für Off-Screen-Entities.
    Arrow { color: Color },
}
```

### Minimap

```rust
/// Hauptstruktur des Minimap-Systems.
pub struct Minimap {
    config: MinimapConfig,
    pins: Vec<MinimapPin>,
    camera: Camera,  // Eigene Kamera-Instanz für Minimap-Viewport
}

impl Minimap {
    pub fn new(config: MinimapConfig) -> Self;

    /// Pin hinzufügen.
    pub fn add_pin(&mut self, pin: MinimapPin);
    /// Alle Pins einer Entity entfernen.
    pub fn remove_pins_for(&mut self, entity: EntityId);
    /// Alle Pins entfernen.
    pub fn clear_pins(&mut self);

    /// Konvertiert Minimap-Klickposition zu Weltkoordinaten.
    /// Gibt None zurück wenn click_to_jump deaktiviert oder Klick außerhalb der Minimap.
    pub fn screen_to_world(&self, screen_pos: RenderVec2) -> Option<RenderVec2>;

    /// Aktualisiert Pin-Positionen aus ECS-Daten.
    pub fn update(&mut self, positions: &[(EntityId, SimVec2)]);

    /// Rendert die Minimap. Aufgerufen nach dem Haupt-Render-Pass.
    pub fn render(
        &self,
        tilemap: &TilemapData,
        fog: Option<&FogOfWarGrid>,
        main_camera: &Camera,
        ctx: &mut RenderContext,
    );
}
```

## Behavior

- **Tile-basiertes Rendering**: Die Minimap rendert die Tilemap als farbige Pixel (1 Tile = 1 Pixel auf der Minimap). Farben werden aus dem Tile-Typ abgeleitet (Gras=grün, Wasser=blau, Wand=grau). Schneller als Sprite-Downscaling und visuell klarer.
- **Fog-of-War-Integration**: `Hidden` Tiles werden mit `fog_hidden_color` überlagert (typisch schwarz). `Explored` Tiles werden mit `fog_explored_color` halbtransparent überlagert (typisch 50% schwarz). `Visible` Tiles werden normal gerendert. Pins auf `Hidden` Tiles sind unsichtbar, es sei denn `always_visible = true`.
- **Viewport-Indikator**: Ein farbiger Rahmen auf der Minimap zeigt den sichtbaren Bereich der Hauptkamera an. Skaliert korrekt mit Kamera-Zoom.
- **Pin-Rendering**: Pins werden nach der Tilemap gerendert. `Dot`-Pins als farbige Pixel. `Sprite`-Pins als kleine Icons. `Arrow`-Pins am Rand der Minimap für Entities außerhalb des dargestellten Bereichs.
- **Click-to-Jump**: Wenn aktiviert, wird ein Klick innerhalb der Minimap-Fläche in Weltkoordinaten konvertiert. Die Hauptkamera springt zur berechneten Position. Nützlich für RTS und große TD-Maps.
- **Update-Zyklus**: `update()` liest aktuelle Entity-Positionen und aktualisiert Pin-Positionen. Wird einmal pro Frame aufgerufen. Rendering erfolgt separat in `render()`.

## Internal Design

- Eigene `Camera`-Instanz mit niedrigem Zoom (z.B. 0.05 = 20x herausgezoomt), zentriert auf `world_bounds`.
- Rendering in eigene Offscreen-Texture (via `PostProcessor::create_offscreen_target()`), dann als Sprite in die UI geblittet.
- Tilemap-Farben werden einmalig aus dem Tileset abgeleitet (Durchschnittsfarbe pro Tile-Typ) und gecacht.
- Pin-Positionen werden von SimVec2 (Simulation) nach Minimap-Pixel konvertiert via linearer Transformation: `minimap_px = (world_pos - world_bounds.origin) / world_bounds.size * minimap_size`.

## Non-Goals

- **3D-Minimap / Rotation.** Immer achsenparallele Top-Down-Ansicht.
- **Dynamische Minimap-Größe.** Config wird bei Erstellung gesetzt, nicht zur Laufzeit geändert. Neues `Minimap`-Objekt erstellen bei Resize.
- **Mehrere Minimaps gleichzeitig.** Ein Minimap-Widget pro Szene. Technisch möglich, aber kein offizieller Support.
- **Minimap-eigene Partikel/Animationen.** Pins sind statische Marker. Animierte Icons können über `Sprite`-PinType mit animierten Spritesheet-Frames realisiert werden (Spiellogik, nicht Minimap-System).

## Open Questions

- Sollen Tile-Farben aus einem RON-Config kommen statt automatisch aus dem Tileset?
- Braucht es einen "Reveal All"-Debug-Modus der Fog-of-War ignoriert?
- Soll die Minimap als eigenständiges `UiDrawCommand` in `amigo_ui` integriert werden?

## Referenzen

- [engine/camera](camera.md) → Minimap Camera Viewport
- [engine/fog-of-war](fog-of-war.md) → TileVisibility für Sichtbarkeits-Masking
- [engine/ui](ui.md) → Minimap als HUD-Element
- [engine/rendering](rendering.md) → Offscreen Render Targets
- [gametypes/rts](../gametypes/rts.md) → Strategische Minimap mit Click-to-Jump
- [gametypes/metroidvania](../gametypes/metroidvania.md) → Erkundungs-Minimap
