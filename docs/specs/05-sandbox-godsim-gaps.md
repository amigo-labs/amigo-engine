# Amigo Engine – Feature-Gap-Analyse: Sandbox & God Sim

> Zweck: Identifiziert fehlende Engine-Features für Terraria-Style (Sandbox/Survival) und WorldBox-Style (God Sim) Spiele. Keine Game-Design-Specs – nur Engine-Erweiterungen.
> Datum: 2026-03-17

---

## Übersicht: Was welches Genre braucht

| Engine-Feature | TD | Shooter | Sandbox | God Sim | Threadwalker |
|----------------|:--:|:-------:|:-------:|:-------:|:------------:|
| Static Tilemap | ✅ | ✅ | – | – | ✅ |
| **Dynamic Tilemap** | – | – | ✅ | ✅ | – |
| **Chunk Streaming** | – | – | ✅ | ✅ | ○ |
| **2D Lighting** | – | – | ✅ | ○ | ✅ |
| **Liquid Simulation** | – | – | ✅ | ✅ | – |
| **Particle System** | ○ | ✅ | ✅ | ✅ | ○ |
| **Inventory / Items** | – | ○ | ✅ | – | ✅ |
| **Crafting** | – | – | ✅ | – | ○ |
| **Agent AI (Autonomous)** | – | – | ○ | ✅ | – |
| **Simulation Tick** | – | – | ○ | ✅ | – |
| **Terrain Modification** | – | – | ✅ | ✅ | – |
| **Zoom / LOD Camera** | – | – | – | ✅ | – |
| Collision (Spatial Hash) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Procgen | – | ✅ | ✅ | ✅ | – |
| Spawner / Waves | ✅ | ✅ | ○ | ○ | – |
| Projectiles | ✅ | ✅ | ✅ | ○ | – |
| Scoring | – | ✅ | – | – | – |
| Pathfinding | ✅ | – | ✅ | ✅ | ✅ |
| Save/Load | ○ | ○ | ✅ | ✅ | ✅ |

✅ = Kernfeature, ○ = nice-to-have, – = nicht benötigt

---

## Neue Engine-Specs

### 1. `engine/dynamic-tilemap.md`

**Benötigt von:** Sandbox, God Sim
**Erweitert:** `engine/tilemap.md` (oder ersetzt Teile davon)

Das ist das größte fehlende Stück. Die aktuelle Tilemap-Spec geht von statischen, zur Buildtime definierten Maps aus. Sandbox und God Sim brauchen:

- **Block Place/Destroy zur Runtime** – Spieler oder Simulation verändert die Welt
- **Tile-Typen mit Properties:** Hardness (Abbauzeit), Drop-Table, Collision-Shape, Light-Emission, Liquid-Durchlässigkeit
- **Tile Updates:** Gras wächst, Sand fällt (Gravity-Tiles), Fackeln gehen aus – regelbasierte Updates pro Tick
- **Background/Foreground Layer:** Terraria hat zwei Ebenen – Wände (Hintergrund) und Blöcke (Vordergrund)
- **Dirty Region Tracking:** Nur geänderte Bereiche neu rendern, Collision-Grid updaten, Lighting neu berechnen
- **Tile-Events:** `on_place`, `on_destroy`, `on_neighbor_change`, `on_tick` – Hooks für spielspezifische Logik

```rust
pub trait DynamicTileMap {
    fn get_tile(&self, pos: TilePos) -> TileId;
    fn set_tile(&mut self, pos: TilePos, tile: TileId) -> TileChange;
    fn get_tile_properties(&self, id: TileId) -> &TileProperties;
    fn query_rect(&self, rect: TileRect) -> TileIterator;
    fn dirty_regions(&self) -> &[ChunkPos];  // Seit letztem Frame geändert
}
```

**Designentscheidung:** Separate Spec oder Erweiterung von `tilemap.md`? Empfehlung: Eigene Spec `dynamic-tilemap.md`, die `tilemap.md` als Basis importiert und die Mutation-API draufsetzt. Statische Tilemaps bleiben für TD und Shooter unverändert.

---

### 2. `engine/chunks.md`

**Benötigt von:** Sandbox, God Sim (Threadwalker profitiert)

Große Welten passen nicht komplett in den Speicher. Chunk-System für Streaming:

- **Chunk-Größe:** Konfigurierbar (z.B. 64x64 oder 128x128 Tiles)
- **Load/Unload-Radius:** Chunks um die Kamera herum geladen, Rest auf Disk
- **Async Loading:** Chunks im Hintergrund laden/speichern, kein Frame-Stutter
- **Chunk-States:** `Unloaded → Loading → Active → Saving → Unloaded`
- **Simulation-Radius:** Aktive Simulation nur in nahen Chunks, weiter entfernte nur bei explizitem Tick (God Sim: selektive Updates)
- **Chunk-Serialisierung:** Kompaktes Binärformat, RLE-Kompression für Tile-Daten
- **World-Koordinaten:** Globales Koordinatensystem, Chunk-Position daraus abgeleitet

```rust
pub trait ChunkManager {
    fn load_radius(&self) -> u32;
    fn active_chunks(&self) -> &[ChunkPos];
    fn request_chunk(&mut self, pos: ChunkPos);
    fn save_chunk(&mut self, pos: ChunkPos);
    fn set_center(&mut self, world_pos: Vec2Fixed);  // Kamera-Position
}
```

**Relation zu Procgen:** Chunks die zum ersten Mal geladen werden, werden generiert. `procgen.md` liefert den Generator, `chunks.md` managed den Lifecycle.

---

### 3. `engine/lighting.md`

**Benötigt von:** Sandbox (Kern), Threadwalker (Atmosphäre), God Sim (nice-to-have)

2D-Lighting für Pixel Art. Kein Raytracing – Tile-basierte Lichtausbreitung:

- **Light Sources:** Point Lights (Fackeln, Laternen), Ambient Light (Himmel, Biom), Emissive Tiles (Lava, Glowstone)
- **Light Propagation:** Flood-Fill von Lichtquellen, Abschwächung pro Tile-Distanz, Blockierung durch opake Tiles
- **Farbiges Licht:** RGB-Kanäle separat (Fackel = warm, Kristall = blau, Lava = rot)
- **Day/Night Cycle:** Ambient Light ändert sich über Zeit, Oberflächen-Tiles empfangen Himmelslicht
- **Smooth Lighting:** Interpolation zwischen Tile-Lichtwerten für weiche Übergänge (kein hartes Grid)
- **Performance:** Nur Dirty Regions neu berechnen, Light-Map als separater Buffer, GPU-seitig mit Tile-Farben multipliziert

```rust
pub struct LightMap {
    // Pro Tile: 3 Bytes (R, G, B) für Lichtlevel 0-255
}

pub trait LightingSystem {
    fn add_light(&mut self, pos: TilePos, color: LightColor, intensity: u8, radius: u8);
    fn remove_light(&mut self, pos: TilePos);
    fn recalculate_dirty(&mut self, tilemap: &DynamicTileMap);
    fn get_light_at(&self, pos: TilePos) -> LightColor;
}
```

---

### 4. `engine/liquids.md`

**Benötigt von:** Sandbox (Kern), God Sim (Regen, Flüsse, Ozeane)

Cellular-Automata-basierte Flüssigkeitssimulation:

- **Flüssigkeitstypen:** Wasser, Lava, (erweiterbar: Gift, Honig, etc.)
- **Level-System:** 8 Stufen pro Tile (0 = leer, 7 = voll) – visuell als Füllhöhe
- **Fließregeln:** Runter > Seitwärts > Verteilen. Konfigurierbar pro Flüssigkeitstyp (Viskosität)
- **Interaktionen:** Wasser + Lava = Obsidian (oder Steam-Partikel). Regelbasiert, als Daten-Asset definierbar
- **Drucksimulation:** Optional für tiefe Wasserkörper – Wasser "drückt" seitwärts unter Gewicht
- **Update-Rate:** Nicht jeden Frame – alle N Ticks, gestaffelt über Chunks (nicht alle Chunks gleichzeitig)
- **Rendering:** Animierte Oberfläche, Transparenz, Tile-Blend am Rand

```rust
pub struct LiquidCell {
    liquid_type: LiquidType,
    level: u8,        // 0-7
    settled: bool,    // Keine Änderung seit N Ticks → überspringen
}

pub trait LiquidSimulation {
    fn step(&mut self, chunk: &mut ChunkData);
    fn set_liquid(&mut self, pos: TilePos, liquid: LiquidType, level: u8);
    fn get_liquid(&self, pos: TilePos) -> LiquidCell;
}
```

---

### 5. `engine/particles.md`

**Benötigt von:** Shooter (Explosionen), Sandbox (Block-Break, Lava-Spritzer), God Sim (Wetter, Katastrophen), Threadwalker (Atmosphäre)

Leichtgewichtiges Partikelsystem für visuelle Effekte:

- **Emitter-basiert:** Position, Rate, Lifetime, Velocity-Range, Gravity, Farbe, Sprite
- **Emitter-Typen:** Point, Line, Circle, Rect, Burst (einmalig)
- **Partikel-Properties:** Position, Velocity, Acceleration, Color (mit Fade), Scale, Rotation, Lifetime
- **Object Pool:** Feste Kapazität (z.B. 2048), kein Alloc zur Runtime
- **Kein Gameplay-Einfluss:** Rein visuell, keine Kollision (außer optional Bounce an Tiles)
- **Presets als Asset:** TOML/RON-Definitionen für häufige Effekte (Explosion, Funken, Rauch, Regen, Schnee)

```rust
pub struct ParticleEmitter {
    shape: EmitterShape,
    rate: f32,                   // Partikel pro Sekunde (0 = Burst)
    burst_count: u16,            // Nur bei rate == 0
    particle_lifetime: RangeTicks,
    velocity: RangeVec2,
    gravity: Vec2Fixed,
    start_color: ColorRange,
    end_color: ColorRange,
    sprite: SpriteRef,
}
```

---

### 6. `engine/inventory.md`

**Benötigt von:** Sandbox (Kern), Threadwalker (Items, Quest-Gegenstände)

Generisches Item- und Inventar-System:

- **Item-Definition:** ID, Name, Stack-Size, Category, Rarity, Icon, Custom Properties (Key-Value)
- **Inventar:** Grid-basiert (Terraria-Style: Hotbar + Rucksack) oder List-basiert (simpler)
- **Stacking:** Items gleichen Typs stacken bis Max, Split/Merge
- **Item-Aktionen:** Use, Equip, Drop, Consume – als Trait, spielspezifisch implementierbar
- **Container:** Kisten, Shops, Crafting-Output – gleiche Inventar-Logik, andere UI
- **Drag & Drop:** Engine liefert die Datenlogik, UI-Layer rendert

```rust
pub trait Inventory {
    fn add_item(&mut self, item: ItemStack) -> Result<(), ItemStack>;  // Rest zurück wenn voll
    fn remove_item(&mut self, slot: SlotId, count: u16) -> Option<ItemStack>;
    fn swap_slots(&mut self, a: SlotId, b: SlotId);
    fn find_item(&self, item_type: ItemTypeId) -> Option<SlotId>;
    fn slots(&self) -> &[Option<ItemStack>];
}
```

---

### 7. `engine/crafting.md`

**Benötigt von:** Sandbox (Kern), Threadwalker (optional)

Rezeptbasiertes Crafting:

- **Rezept-Definition:** Inputs (ItemType + Count), Output (ItemType + Count), Station (optional: Werkbank, Ofen, Amboss)
- **Rezept-Lookup:** Gegeben ein Inventar und eine Station, welche Rezepte sind verfügbar?
- **Kategorien:** Für UI-Filterung (Werkzeuge, Waffen, Blöcke, Möbel, Tränke)
- **Datengetrieben:** Rezepte als TOML/RON-Assets, kein Hardcoding
- **Erweiterbar:** Modding-freundlich, neue Rezepte per Asset hinzufügbar

```rust
pub struct Recipe {
    id: RecipeId,
    inputs: Vec<(ItemTypeId, u16)>,
    output: (ItemTypeId, u16),
    station: Option<StationTypeId>,
    category: CraftingCategory,
}

pub trait CraftingSystem {
    fn available_recipes(&self, inventory: &dyn Inventory, station: Option<StationTypeId>) -> Vec<RecipeId>;
    fn craft(&self, recipe: RecipeId, inventory: &mut dyn Inventory) -> Result<ItemStack, CraftError>;
}
```

---

### 8. `engine/agents.md`

**Benötigt von:** God Sim (Kern), Sandbox (NPCs, optional)

Autonome Agenten mit Bedürfnissen und Verhalten – das Herz der God Sim:

- **Agent-Archetype:** Typ (Mensch, Tier, Monster), Stats, Bedürfnisse, Fähigkeiten
- **Bedürfnis-System:** Hunger, Schlaf, Sicherheit, Soziales – jedes Bedürfnis hat einen Wert 0-100, sinkt über Zeit, Agent priorisiert das dringendste
- **Behavior Tree / Utility AI:** Agent wählt Aktion basierend auf Bedürfnissen + Umwelt. Utility-basiert: jede mögliche Aktion hat einen Score, höchster gewinnt
- **Aktionen:** Eat, Sleep, Build, Harvest, Fight, Flee, Trade, Socialize, Explore, Worship
- **Memory:** Agent merkt sich Ressourcen-Positionen, Beziehungen zu anderen Agenten, Gefahren
- **Gruppen / Settlements:** Agenten bilden Siedlungen, teilen Ressourcen, bauen Strukturen
- **Skalierung:** God Sim braucht hunderte bis tausende gleichzeitige Agenten → LOD: nahe Agenten voll simuliert, ferne nur statistisch

```rust
pub trait Agent {
    fn needs(&self) -> &Needs;
    fn evaluate_actions(&self, ctx: &WorldContext) -> Vec<(Action, f32)>;  // Action + Score
    fn execute(&mut self, action: Action, world: &mut WorldAccess);
    fn update_needs(&mut self, dt: Ticks);
}
```

**Relation zu Pathfinding:** Agenten nutzen `pathfinding.md` für Navigation. Bei God Sim mit vielen Agenten: Flow Fields statt individuelles A* (bereits in Pathfinding-Spec vorgesehen).

---

### 9. `engine/simulation.md`

**Benötigt von:** God Sim (Kern), Sandbox (Welt-Updates)

Übergeordnetes Simulations-Framework für Welt-Updates die unabhängig vom Render-Tick laufen:

- **Sim-Tick-Rate:** Konfigurierbar, unabhängig von FPS (z.B. 10 Sim-Ticks/Sekunde)
- **Sim-Systeme:** Liquid, Tile-Updates, Agent-AI, Wetter, Tag/Nacht – jedes registriert sich als SimSystem
- **Prioritäten & Budgets:** Nicht alle Systeme müssen jeden Tick laufen. Priority-Queue, Zeitbudget pro Frame
- **Speed Control:** God Sim typisch: Pause, 1x, 2x, 5x Geschwindigkeit
- **Determinismus:** Gleicher Seed + gleiche Inputs = gleiches Ergebnis (wichtig für Replays, Multiplayer)

```rust
pub trait SimSystem {
    fn priority(&self) -> u8;
    fn tick_interval(&self) -> u32;       // Alle N Sim-Ticks
    fn update(&mut self, world: &mut World, tick: u64);
}

pub struct SimulationRunner {
    systems: Vec<Box<dyn SimSystem>>,
    tick_rate: u32,
    speed_multiplier: f32,
    paused: bool,
}
```

---

### 10. `engine/save-load.md`

**Benötigt von:** Sandbox (Kern), God Sim (Kern), Threadwalker (Kern), Shooter (nice-to-have)

Persistierung von Spielständen:

- **World-Serialisierung:** Chunks, Tile-Daten, Entities, Inventare, Agent-States
- **Incremental Save:** Nur geänderte Chunks schreiben (Dirty-Flag aus Chunk-System)
- **Save-Format:** Eigenes Binärformat mit Versionsheader, oder MessagePack/Bincode
- **Auto-Save:** Konfigurierbar (alle N Minuten), im Hintergrund
- **Save Slots:** Mehrere Spielstände, Metadata (Name, Playtime, Screenshot-Thumbnail)
- **Migration:** Versionierte Schemas, ältere Saves laden und migrieren

```rust
pub trait Saveable {
    fn serialize(&self, writer: &mut SaveWriter);
    fn deserialize(reader: &mut SaveReader, version: u32) -> Self;
}

pub trait SaveManager {
    fn save(&self, slot: SaveSlot, world: &World) -> Result<(), SaveError>;
    fn load(&self, slot: SaveSlot) -> Result<World, LoadError>;
    fn list_slots(&self) -> Vec<SaveSlotInfo>;
    fn autosave_tick(&mut self, world: &World);
}
```

---

## Erweiterungen bestehender Specs

### `engine/tilemap.md` erweitern

- Tile-Properties-System (Hardness, Emission, Solidity als Daten, nicht Hardcode)
- Background/Foreground Layer Konzept
- Hook-basierte Tile-Events (`on_neighbor_change` für Redstone-artige Logik)
- Verweis auf `dynamic-tilemap.md` für Mutation-API

### `engine/camera.md` erweitern

- **ZoomCamera:** Stufenloser Zoom (God Sim: von Einzelperson bis Weltkarte)
- **MinimapCamera:** Zweiter Viewport für Minimap-Rendering
- LOD-Hinweis: Bei starkem Rauszoomen weniger Detail rendern

### `engine/rendering.md` erweitern

- **Layer-Blending:** Light-Map als Multiply-Layer über Tile-Layer
- **Parallax-Hintergründe:** Mehrere Scroll-Speeds (Sandbox: Himmel, Wolken, ferne Berge)
- **LOD für Zoom:** Sprite-Vereinfachung bei weitem Zoom (God Sim)

### `engine/procgen.md` erweitern

- **Biom-System:** Noise-basierte Biom-Verteilung (Temperatur + Feuchtigkeit → Biom)
- **Terrain-Höhenprofil:** Perlin/Simplex Noise für Oberflächen-Kontur (Sandbox)
- **Ore/Resource Distribution:** Tiefenbasierte Erz-Verteilung (Sandbox)
- **Siedlungs-Platzierung:** Regelbasiert auf Terrain (God Sim)
- **Cave Generation:** Zellulärer Automat oder Wurm-Algorithmus (Sandbox)

### `engine/pathfinding.md` erweitern

- **Dynamic Navmesh Updates:** Tilemap ändert sich → Pfade invalidieren, neu berechnen
- **Flow Fields für Massen-Navigation:** Hunderte Agenten zum selben Ziel (God Sim)

### `assets/format.md` erweitern

- **Tile-Property-Definitionen als TOML:** Hardness, Drops, Light-Emission, Liquid-Behavior
- **Recipe-Format:** Crafting-Rezepte als TOML
- **Agent-Archetypes als TOML/RON:** Bedürfnis-Parameter, Fähigkeiten, Stats
- **Biom-Definitionen:** Tile-Verteilung, Vegetation, Mob-Spawns pro Biom

---

## Zusammenfassung: Neue Spec-Dateien

| Neue Spec | Primär für | Sekundär für |
|-----------|-----------|--------------|
| `engine/dynamic-tilemap.md` | Sandbox | God Sim |
| `engine/chunks.md` | Sandbox, God Sim | Threadwalker |
| `engine/lighting.md` | Sandbox | Threadwalker, God Sim |
| `engine/liquids.md` | Sandbox | God Sim |
| `engine/particles.md` | Shooter, Sandbox | God Sim, Threadwalker, TD |
| `engine/inventory.md` | Sandbox | Threadwalker |
| `engine/crafting.md` | Sandbox | Threadwalker |
| `engine/agents.md` | God Sim | Sandbox (NPCs) |
| `engine/simulation.md` | God Sim | Sandbox |
| `engine/save-load.md` | Sandbox, God Sim, Threadwalker | Shooter |

---

## Engine-Feature-Gesamtbild nach Erweiterung

```
engine/
├── core.md                  # ECS, Math, Core Types
├── rendering.md             # Render Pipeline + Lighting-Integration + Parallax + LOD
├── audio.md                 # Audio Engine
├── input.md                 # Input + Multiplayer Slots
├── tilemap.md               # Statische Tilemaps (TD, Shooter, Threadwalker)
├── dynamic-tilemap.md       # Mutation-API (Sandbox, God Sim)          ← NEU
├── chunks.md                # Chunk Streaming, Async Load              ← NEU
├── pathfinding.md           # A*, Flow Fields, Dynamic Navmesh
├── animation.md             # Sprite Animation
├── camera.md                # Follow, AutoScroll, Split, Zoom
├── ui.md                    # Pixel UI + egui
├── networking.md            # Commands, Lockstep
├── memory-performance.md    # Budgets, Pooling
├── plugin-system.md         # Feature Flags
├── projectiles.md           # Object Pool, Patterns               (Shooter)
├── collision.md             # Layers, Spatial Hash                 (Shooter)
├── spawner.md               # Formations, Waves                    (Shooter)
├── procgen.md               # BSP, Segments, Biome, Caves          (Shooter + Sandbox + God Sim)
├── scoring.md               # Score, Combo, Leaderboards           (Shooter)
├── lighting.md              # 2D Tile Lighting, Day/Night          ← NEU
├── liquids.md               # Cellular Automata Fluids             ← NEU
├── particles.md             # Emitter-basierte Effekte             ← NEU
├── inventory.md             # Items, Stacking, Containers          ← NEU
├── crafting.md              # Rezepte, Stationen                   ← NEU
├── agents.md                # Utility AI, Bedürfnisse, Settlements ← NEU
├── simulation.md            # Sim-Tick, Speed Control, Determinism ← NEU
└── save-load.md             # Persistierung, Auto-Save, Migration  ← NEU
```
