# Amigo Engine – Tricks, Techniken & Patterns

## Internes Referenzdokument für Entwickler

---

## Übersicht

Dieses Dokument beschreibt alle cleveren Techniken, Optimierungen und Architektur-Patterns die die Amigo Engine nutzt. Jede Technik erklärt: **Was ist das Problem?**, **Wie löst die Engine es?**, **Wo wird es benutzt?**, und **Code-Skizze**.

---

## DATENSTRUKTUREN & ECS

---

### 1. SparseSet

**Problem:** Random Access auf Entity-Komponenten (O(1)) UND Cache-freundliche Iteration über alle Komponenten gleichzeitig. Arrays können das eine, HashMaps das andere – nicht beides.

**Lösung:** Zwei Arrays: ein großes "sparse" Array (Index = Entity-ID, Wert = Position im Dense Array) und ein kleines "dense" Array (kompakt, ohne Löcher, Cache-freundlich).

```
Sparse: [_, _, 0, _, 2, _, _, 1, _, _, 3]   ← Index = Entity-ID
Dense:  [Pos_A, Pos_B, Pos_C, Pos_D]        ← kompakt, iterierbar
IDs:    [  2,     7,     4,    10  ]          ← welche Entity pro Slot
```

- **Random Access:** `sparse[entity_id]` → Dense-Index → O(1)
- **Iteration:** Linear über Dense Array → perfekte Cache-Locality
- **Insert:** Append an Dense, setze Sparse-Eintrag → O(1)
- **Remove:** Swap-Remove im Dense (letztes Element an die Lücke), update Sparse → O(1)

**Wo:** Jeder Komponenten-Typ hat sein eigenes SparseSet. Position, Velocity, Health, SpriteComp – alle in separaten Dense Arrays, alle parallel iterierbar.

**Serialisierung:** Nur Dense-Array + IDs werden gespeichert. Sparse wird beim Laden neu aufgebaut.

---

### 2. Change Tracking (BitSet)

**Problem:** 500 Entities haben eine Position. 30 ändern sich pro Frame. Das Rendering will nur die geänderten Sprites neu sortieren.

**Lösung:** Ein BitSet neben dem Dense Array. Jedes Bit = "wurde seit letztem Query geändert?" `get_mut()` setzt das Bit automatisch.

```rust
pub fn get_mut(&mut self, entity: EntityId) -> &mut T {
    let dense_idx = self.sparse[entity.id()];
    self.changed.set(dense_idx, true);  // automatisch markiert!
    &mut self.dense[dense_idx]
}

pub fn query_changed(&self) -> impl Iterator<Item = (EntityId, &T)> {
    self.changed.iter_ones().map(|i| (self.ids[i], &self.dense[i]))
}

pub fn clear_changed(&mut self) {
    self.changed.clear();
}
```

**Wo:** Rendering (nur geänderte Sprites neu sortieren), Netzwerk (nur geänderte Komponenten senden), Save (nur dirty-markierte Entities speichern).

---

### 3. State-Scoped Entity Cleanup

**Problem:** State-Wechsel (Playing → Menu). Hunderte Gameplay-Entities müssen weg. Eins vergessen → Memory Leak oder Ghost-Entities.

**Lösung:** `StateScoped`-Komponente. Beim State-Wechsel: automatisch alle Entities mit dem alten State despawnen.

```rust
// Spawn:
world.spawn()
    .with(Enemy { ... })
    .with(StateScoped(GameState::Playing));

// State-Wechsel zu Menu → Engine despawnt automatisch ALLES mit Playing
// Kein manuelles Aufräumen. Kein Vergessen möglich.
```

**Wo:** Jeder State-Übergang. Besonders wichtig für Threadwalker: Welt-Wechsel despawnt alle Entities der vorherigen Welt automatisch.

---

### 4. Hybrid Component Storage

**Problem:** Die Engine hat "heiße" Komponenten (Position, Velocity – fast jede Entity hat sie, in jedem Frame gelesen) und "kalte" Komponenten (TowerData, QuestProgress – nur bestimmte Entities, selten gelesen). Ein einheitliches System verschwendet Cache für kalte Daten oder ist langsam für heiße.

**Lösung:** Heiße Komponenten als statisch typisierte SparseSet-Felder. Kalte, game-spezifische Komponenten in einer `HashMap<TypeId, Box<dyn AnyStorage>>`.

```rust
pub struct World {
    // Hot path – statische Felder, kein HashMap-Lookup
    pub positions: SparseSet<Position>,
    pub velocities: SparseSet<Velocity>,
    pub sprites: SparseSet<SpriteComp>,
    pub healths: SparseSet<Health>,

    // Cold path – dynamisch, für game-spezifische Komponenten
    pub extensions: HashMap<TypeId, Box<dyn AnyStorage>>,
}
```

**Wo:** Engine-interne Komponenten (Position, Velocity, Collider) = statisch. Game-Komponenten (TowerData, QuestLog, ArmState) = dynamisch.

---

### 5. Tick Scheduler

**Problem:** Manche Systeme müssen nicht jeden Frame laufen. Pathfinding alle 10 Frames reicht. AI-Entscheidungen alle 30 Frames. Aber der Game Loop läuft mit 60 FPS.

**Lösung:** `scheduler.every(n, system)` – System wird nur alle N Ticks ausgeführt.

```rust
scheduler.every(1, physics_system);        // jeden Frame
scheduler.every(10, pathfinding_system);    // alle 10 Frames
scheduler.every(30, ai_decision_system);    // alle 30 Frames
scheduler.every(60, save_autosave);         // jede Sekunde
```

**Wo:** Pathfinding, AI, Autosave, Flow Field Neuberechnung, Atmospheric Transitions (langsam, braucht nicht 60 FPS).

---

## KOLLISION & SPATIAL

---

### 6. Spatial Hash

**Problem:** "Welche Entities sind in der Nähe?" Naiv: O(n²). Bei 500 Entities = 250.000 Checks pro Frame.

**Lösung:** Unsichtbares Raster über die Welt. Entities werden in Zellen einsortiert. Queries prüfen nur 9 Nachbarzellen statt alle Entities.

```rust
pub struct SpatialHash {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<EntityId>>,
}

// Query: nur 9 Zellen statt N Entities
fn query_nearby(&self, pos: Vec2, radius: f32) -> Vec<EntityId> {
    let r = (radius / self.cell_size).ceil() as i32;
    let cx = (pos.x / self.cell_size) as i32;
    let cy = (pos.y / self.cell_size) as i32;
    // iteriere (cx-r..=cx+r) × (cy-r..=cy+r)
}
```

Wird jeden Frame komplett neu aufgebaut (schneller als inkrementelle Updates bei sich bewegenden Entities).

**Wo:** Kollisionserkennung, Range-Checks (Tower-Reichweite), Proximity-Queries (NPCs in der Nähe), AoE-Schaden.

---

### 7. AABB Collision (Axis-Aligned Bounding Box)

**Problem:** Zwei Sprites überlappen sich – kollidieren sie? Pixel-perfekte Kollision ist zu teuer für 60 FPS.

**Lösung:** Jede Entity hat ein unsichtbares Rechteck (AABB). Überlappung von zwei Rechtecken = eine einzige Bedingung:

```rust
fn aabb_overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w &&
    a.x + a.w > b.x &&
    a.y < b.y + b.h &&
    a.y + a.h > b.y
}
```

Vier Vergleiche. Schneller geht's nicht. Kombiniert mit Spatial Hash: erst grobe Nachbarschafts-Suche, dann feine AABB-Prüfung. Zweistufig.

**Wo:** Jede physische Interaktion. Projektil trifft Enemy, Spieler berührt Item, Entity tritt in Trigger-Zone.

---

## PATHFINDING

---

### 8. A* auf Tile-Grid

**Problem:** Ein NPC soll den kürzesten Weg durch eine Tilemap finden, um Hindernisse herum.

**Lösung:** A* auf dem Tile-Grid. Jede Tile ist ein Knoten, begehbare Nachbarn sind Kanten. Heuristik: Manhattan-Distanz (oder Chebyshev für diagonale Bewegung).

```rust
fn a_star(start: TilePos, goal: TilePos, tilemap: &Tilemap) -> Option<Vec<TilePos>> {
    let mut open = BinaryHeap::new();  // Priority Queue
    let mut came_from = HashMap::new();
    let mut g_score = HashMap::new();  // Kosten vom Start

    open.push(Node { pos: start, f: heuristic(start, goal) });
    g_score.insert(start, 0);

    while let Some(current) = open.pop() {
        if current.pos == goal { return reconstruct_path(came_from, goal); }

        for neighbor in tilemap.walkable_neighbors(current.pos) {
            let tentative_g = g_score[&current.pos] + move_cost(current.pos, neighbor);
            if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                open.push(Node { pos: neighbor, f: tentative_g + heuristic(neighbor, goal) });
            }
        }
    }
    None  // kein Pfad gefunden
}
```

**Wo:** RPG-Welten (Meridian – NPCs navigieren), Dungeon Crawler (Kabinett – Feinde jagen den Spieler), Survival (Knochenhain – Feinde finden das Camp).

---

### 9. Waypoint Pathfinding

**Problem:** Tower Defense – Enemies folgen einem festen Pfad. A* wäre Overkill.

**Lösung:** Editor-definierte Punkte. Enemy folgt der Liste, interpoliert zwischen Punkten.

```rust
pub struct WaypointPath {
    pub points: Vec<SimVec2>,
}

pub struct PathFollower {
    pub path_index: usize,     // welcher Pfad
    pub segment: usize,        // zwischen Punkt N und N+1
    pub progress: Fix,         // 0.0 = bei Punkt N, 1.0 = bei Punkt N+1
}

fn follow_path(follower: &mut PathFollower, path: &WaypointPath, speed: Fix, dt: Fix) {
    follower.progress += speed * dt / segment_length(path, follower.segment);
    while follower.progress >= Fix::ONE {
        follower.progress -= Fix::ONE;
        follower.segment += 1;
        if follower.segment >= path.points.len() - 1 {
            // Ziel erreicht!
        }
    }
}
```

**Wo:** TD (Rostgarten), jede Situation mit festen Routen (Patrouillen in Stealth, NPC-Wanderrouten).

---

### 10. Flow Field

**Problem:** 200 Enemies sollen alle zum selben Ziel. 200× A* pro Frame = zu teuer.

**Lösung:** Einmal ein Richtungsfeld berechnen (BFS vom Ziel). Jede Zelle bekommt einen Pfeil "geh in diese Richtung." Jeder Enemy liest nur seine Zelle – O(1).

```
┌────┬────┬────┬────┐
│ ↘  │ →  │ →  │ ↓  │
├────┼────┼────┼────┤    Berechnung: BFS vom Ziel (★)
│ ↓  │ ██ │ →  │ ↓  │    Jede Zelle zeigt zum Nachbarn
├────┼────┼────┼────┤    mit den niedrigsten Kosten
│ →  │ →  │ →  │ ★  │
└────┴────┴────┴────┘

Enemy bei (0,0): liest ↘ → bewegt sich diagonal
```

Neuberechnung nur wenn sich die Tilemap ändert oder das Ziel wechselt. Für statische Karten: einmal berechnen, ewig nutzen.

**Wo:** Horden-Szenarien (Knochenhain – Nachts greifen Baumwurzeln das Camp an), RTS-artige Situationen, jede Szene mit vielen Entities die zum selben Ziel wollen.

---

## RENDERING

---

### 11. Sprite Batcher (Texture Atlas Grouping)

**Problem:** 500 Sprites rendern = 500 Draw Calls. GPU-Overhead pro Draw Call ist hoch. Ergebnis: Ruckeln.

**Lösung:** Alle Sprites sammeln, nach Texture Atlas sortieren, pro Atlas EIN Draw Call.

```
Frame-Pipeline:
1. Alle Sprites einsammeln: [(atlas_id, position, src_rect, z_index), ...]
2. Nach atlas_id sortieren (sekundär: z_index)
3. Pro Atlas-Gruppe: ein einziger Vertex Buffer Upload + Draw Call

500 Sprites, 3 Atlases → 3 Draw Calls statt 500
```

**Wo:** ALLES was gerendert wird. Entities, Tiles, Partikel, UI – alles geht durch den Sprite Batcher.

---

### 12. Per-Sprite Shaders

**Problem:** Ein getroffener Enemy soll weiß aufblitzen. Ein vergifteter soll grünlich pulsieren. Ein unsichtbarer soll transparent flimmern. Aber der Sprite Batcher rendert alles in einem Batch.

**Lösung:** Shader-ID als Attribut im Vertex Buffer. Der Batcher gruppiert Sprites auch nach Shader. Built-in Shader-Set:

| Shader | Effekt | Anwendung |
|--------|--------|-----------|
| `default` | Normales Rendering | Standard |
| `flash` | Kurz komplett weiß | Treffer-Feedback |
| `outline` | 1px farbige Outline | Selektion, Hover |
| `dissolve` | Pixel lösen sich auf | Tod-Animation |
| `palette_swap` | Farben tauschen | Team-Farben, Varianten |
| `silhouette` | Einfarbige Silhouette | Hinter Wänden sichtbar |
| `wave` | Wellenförmige Verzerrung | Unterwasser, Hitze |

```rust
world.set(entity, SpriteEffect::Flash { duration: 0.1, color: Color::WHITE });
// → Sprite Batcher erkennt den Effekt, batcht mit Flash-Shader
```

**Wo:** Kampf-Feedback, Status-Effekte, visuelle Hervorhebung, Stealth (Silhouette wenn hinter Deckung).

---

### 13. Tilemap Chunk Caching

**Problem:** Die Tilemap hat 500×500 Tiles. Jeden Frame alle rendern? Verschwendung.

**Lösung:** Die Tilemap wird in Chunks aufgeteilt (16×16 Tiles). Nur sichtbare Chunks werden gerendert. Sichtbare Chunks werden in eine Texture gecacht – solange sich der Chunk nicht ändert, wird nur die gecachte Texture geblittet (EIN Quad statt 256 Tile-Draws).

```
Chunk-Cache:
┌────────┬────────┬────────┐
│Chunk(0,0)│Chunk(1,0)│Chunk(2,0)│  Sichtbar: grün
│ CACHED │ CACHED │  DIRTY │  Dirty: rot (Tile geändert → neu rendern)
├────────┼────────┼────────┤
│Chunk(0,1)│Chunk(1,1)│Chunk(2,1)│  Nicht sichtbar: gar nicht laden
│ CACHED │ CACHED │ CACHED │
├────────┼────────┼────────┤
│  OUT   │  OUT   │  OUT   │  OUT = outside viewport
└────────┴────────┴────────┘
```

**Wo:** Jede Tilemap. Besonders wichtig für große Welten (Kabinett-Dungeon, Sporenwolke-Overworld).

---

### 14. Animated Tiles

**Problem:** Wasser soll fließen, Lava soll pulsieren, Gras soll wehen. Aber Tiles sind statische Grafiken.

**Lösung:** Bestimmte Tile-IDs haben ein `animation`-Tag. Die Engine tauscht die Tile-Grafik in regelmäßigen Abständen. Der Chunk-Cache wird dabei invalidiert.

```ron
// tileset definition
(
    tiles: {
        42: (name: "water", walkable: false, animation: (
            frames: [42, 43, 44, 45],
            frame_duration: 0.25,  // Sekunden pro Frame
        )),
    },
)
```

**Wo:** Wasser, Lava, Fackeln, leuchtende Kristalle, Sporenregen, alles was "lebt" auf der Tilemap.

---

### 15. Auto-Tiling (Bitmask)

**Problem:** Wasser neben Land braucht Übergangs-Tiles (Ufer, Ecken). 47 mögliche Varianten manuell platzieren?

**Lösung:** Jede Tile prüft ihre 8 Nachbarn, berechnet eine Bitmask, und schlägt das passende Sprite in einer Lookup-Tabelle nach.

```
Nachbar-Bitmask:              Beispiel:
┌───┬───┬───┐                 Land│Land│Land
│ 1 │ 2 │ 4 │                 ────┼────┼────
├───┼───┼───┤                 Wasser│ X │Land  → Bitmask = 2+4+16 = 22
│ 8 │ X │16 │                 ────┼────┼────
├───┼───┼───┤                 Wasser│Wasser│Wasser
│32 │64 │128│
└───┴───┴───┘                 Tile 22 → Ufer oben-rechts
```

**Wo:** Level Editor (auto-tiling beim Malen), Tilemap-Laden. Funktioniert für Wasser, Wege, Mauern, Klippen, Höhenstufen.

---

### 16. Post-Processing Stack

**Problem:** Verschiedene Welten brauchen verschiedene visuelle Effekte. Caribbean = Bloom + Warm Color Grade. Matrix = Chromatic Aberration + CRT Scanlines. Zur Laufzeit wechseln.

**Lösung:** Post-Processing als `Vec<PostEffect>` pro Welt/Scene. Konfigurierbar in RON. Engine rendert die Szene in eine Texture, dann jeden Effekt als Fullscreen-Pass darüber.

```ron
// data/atmospheres/my_world.ron
(
    "normal": (
        post_effects: [
            Bloom(threshold: 0.8, intensity: 0.3),
            ColorGrade(lut: "luts/warm_sunset.png"),
            Vignette(strength: 0.2),
        ],
    ),
    "boss": (
        post_effects: [
            ChromaticAberration(offset: 2.0),
            Bloom(threshold: 0.6, intensity: 0.5),
            ColorGrade(lut: "luts/red_danger.png"),
            ScreenShake(intensity: 0.5),
        ],
    ),
)
```

Atmosphere-Transitions interpolieren zwischen Post-Processing Stacks (Bloom fährt hoch, Vignette fährt runter, über 2 Sekunden).

**Wo:** Stimmungs-Wechsel (calm → battle), Welt-spezifischer Look, Boss-Encounters, Cutscenes.

---

### 17. Dual-Layer Rendering (Game + Editor)

**Problem:** Das Spiel rendert bei 640×360 (Pixel Art). Der Editor braucht scharfen Text und UI bei nativer Auflösung (z.B. 2560×1440).

**Lösung:** Zwei getrennte Render-Targets:

```
1. Off-Screen Texture (640×360) → Game-Welt + Pixel UI
   └── Integer-skaliert in einen Viewport im Editor-Fenster

2. Backbuffer (native Auflösung) → egui Editor-Panels
   └── Text, Thumbnails, Dropdowns, Properties – alles scharf
```

Im Play-Mode: nur Target 1, fullscreen. Im Editor-Mode: Target 1 als Panel, Target 2 drumherum.

**Wo:** Immer. Das ist die fundamentale Render-Architektur.

---

## AUDIO

---

### 18. Adaptive Music – Vertical Layering

**Problem:** Musik soll sich mit dem Gameplay ändern. Aber ein einfacher Track-Wechsel (Crossfade) klingt nach zwei verschiedenen Songs.

**Lösung:** Mehrere Stems (Drums, Bass, Melodie, Streicher, Brass) laufen gleichzeitig, synchron. Jeder Stem hat ein Volume das von Game-Parametern gesteuert wird.

```
Tension = 0.2 (ruhig):     Tension = 0.8 (Kampf):
  Drums:    ░░░░░░ (leise)    Drums:    ██████ (laut)
  Bass:     ░░░░░░ (leise)    Bass:     █████░ (laut)
  Melodie:  ░░░░░░ (aus)      Melodie:  ████░░ (mittel)
  Streicher:████░░ (mittel)   Streicher:██████ (laut)
  Brass:    ░░░░░░ (aus)      Brass:    ████░░ (mittel)
```

Fades sind smooth (Lerp pro Frame). Das Stück klingt immer wie ein Song – nur die Dichte ändert sich.

**Wo:** Jede Welt mit adaptiver Musik. Parameter: Tension, Danger, Boss, Victory.

---

### 19. Bar-Synced Music Transitions

**Problem:** Musik soll von "calm" zu "battle" wechseln. Aber ein Crossfade mitten im Takt klingt furchtbar.

**Lösung:** BarClock trackt die aktuelle Position im Takt. Transitions werden als "Pending" markiert und erst am nächsten Bar-Boundary ausgeführt.

```
Beat: 1 . . . 2 . . . 3 . . . 4 . . . | 1 . . . 2 . . .
                            ↑                ↑
                     Boss spawnt!        Hier passiert der
                     → Pending            Wechsel (auf der 1)
```

**Transition-Typen:**
- `CrossfadeOnBar(2)` – 2 Takte lang überblenden
- `CutOnBar` – harter Schnitt auf der 1
- `StingerThen(sound, next)` – kurzer Akzent, dann Transition
- `FadeOutThenPlay(1)` – 1 Takt ausfaden, Stille, neues Stück
- `LayerSwap(2)` – ein Layer alle 2 Takte tauschen

---

### 20. Stinger Quantization

**Problem:** Ein Tower wird gebaut → kurzer musikalischer Akzent (Stinger). Aber der Stinger soll musikalisch zum laufenden Beat passen, nicht beliebig drübergeklatscht werden.

**Lösung:** Stingers haben ein Quantisierungs-Level:

```rust
pub enum StingerQuantize {
    Immediate,    // sofort abspielen (für dringende Events wie Life-Lost)
    NextBeat,     // auf dem nächsten Beat (für kleine Events: Tower gebaut)
    NextBar,      // auf der nächsten 1 (für große Events: Boss spawnt)
}
```

**Wo:** Alle Gameplay-Events die einen Audio-Akzent haben. Tower bauen = NextBeat. Wave Start = NextBar. Leben verloren = Immediate.

---

### 21. SFX Variant System

**Problem:** Derselbe Kanonenschuss 50× hintereinander klingt wie ein kaputter Plattenspieler.

**Lösung:** Mehrere Varianten pro Sound. Engine wählt zufällig + leichte Pitch-Variation.

```ron
(
    "impact_01": (
        files: ["sfx/impact_01a.ogg", "sfx/impact_01b.ogg", "sfx/impact_01c.ogg"],
        volume: 0.8,
        pitch_variance: 0.05,   // ±5% zufällige Tonhöhenverschiebung
        max_concurrent: 3,       // maximal 3 gleichzeitig
        cooldown: 0.05,          // mindestens 50ms zwischen Abspielen
    ),
)
```

**Wo:** Jeder wiederholende SFX. Schüsse, Schritte, Treffer, UI-Klicks.

---

## DETERMINISMUS & NETZWERK

---

### 22. Fixed-Point Arithmetik

**Problem:** `f32` ist nicht deterministisch über CPUs. Multiplayer und Replays desyncen nach 1000+ Frames.

**Lösung:** `I16F16` (16 Bit Integer + 16 Bit Fraction) für alle Simulation. Integer-Operationen sind auf jeder CPU identisch.

```
I16F16: Wert 3.75 = 0000000000000011.1100000000000000
                     ← 16 Bit Integer → ← 16 Bit Frac →

Addition = Integer-Addition → deterministisch
Multiplikation = Integer-Mult + Shift → deterministisch
```

**Wo:** JEDE Gameplay-Berechnung: Position, Velocity, Damage, Timer, Cooldowns. Rendering darf weiterhin f32 nutzen (nur visuell, nicht simulation-relevant).

---

### 23. Seeded RNG

**Problem:** Zufallswerte im Gameplay (Damage-Spread, Spawn-Varianz, Partikel) müssen reproduzierbar sein.

**Lösung:** Der RNG ist Teil des GameState, initialisiert mit einem Seed. Jeder Zufallswert kommt aus demselben deterministischen Generator.

```rust
pub struct GameState {
    pub rng: StdRng,          // Seeded, deterministisch
    // ... alles andere
}

// IMMER: state.rng.gen_range(0..100)
// NIE:   rand::thread_rng()  ← nicht reproduzierbar!
```

**Replay:** Gleicher Seed + gleiche Commands → exakt gleiche Simulation. Auch 10 Jahre später, auf anderer Hardware.

---

### 24. No HashMap Iteration in Simulation

**Problem:** `HashMap::iter()` gibt Elemente in undefinierter Reihenfolge zurück. Auf verschiedenen Maschinen kann die Reihenfolge anders sein → Determinismus gebrochen.

**Lösung:** Simulation nutzt `BTreeMap` (sortiert) oder `IndexMap` (Insertion-Order). Iteration ist deterministisch, unabhängig von Plattform oder Laufzeit.

```rust
// VERBOTEN in Simulation:
let map: HashMap<EntityId, Damage> = ...;
for (id, dmg) in map.iter() { ... }  // Reihenfolge undefiniert!

// KORREKT:
let map: BTreeMap<EntityId, Damage> = ...;
for (id, dmg) in map.iter() { ... }  // immer sortiert nach EntityId

// ODER: Iteration über SparseSet (Dense Array = feste Reihenfolge)
for (id, pos) in world.positions.iter() { ... }  // Insertion-Order
```

**Regel:** `HashMap` ist OK für Lookups (read-only), aber NIE für Iteration die Simulation-Ergebnisse beeinflusst. `FxHashMap` (rustc_hash) wird für Performance-kritische Read-Only Maps verwendet.

**Wo:** Überall in der Simulation. Damage-Berechnung, Spawn-Reihenfolge, Turn-Order, Loot-Drops.

---

## Zusammenfassung

| # | Technik | Kategorie | Komplexität |
|---|---------|-----------|-------------|
| 1 | SparseSet | ECS | Mittel |
| 2 | Change Tracking | ECS | Niedrig |
| 3 | State-Scoped Cleanup | ECS | Niedrig |
| 4 | Hybrid Component Storage | ECS | Mittel |
| 5 | Tick Scheduler | ECS | Niedrig |
| 6 | Spatial Hash | Kollision | Mittel |
| 7 | AABB Collision | Kollision | Niedrig |
| 8 | A* Pathfinding | Pathfinding | Mittel |
| 9 | Waypoint Pathfinding | Pathfinding | Niedrig |
| 10 | Flow Field | Pathfinding | Mittel |
| 11 | Sprite Batcher | Rendering | Mittel |
| 12 | Per-Sprite Shaders | Rendering | Hoch |
| 13 | Tilemap Chunk Caching | Rendering | Mittel |
| 14 | Animated Tiles | Rendering | Niedrig |
| 15 | Auto-Tiling (Bitmask) | Rendering | Mittel |
| 16 | Post-Processing Stack | Rendering | Hoch |
| 17 | Dual-Layer Rendering | Rendering | Hoch |
| 18 | Adaptive Music | Audio | Hoch |
| 19 | Bar-Synced Transitions | Audio | Hoch |
| 20 | Stinger Quantization | Audio | Mittel |
| 21 | SFX Variant System | Audio | Niedrig |
| 22 | Fixed-Point Arithmetik | Determinismus | Mittel |
| 23 | Seeded RNG | Determinismus | Niedrig |
| 24 | No HashMap Iteration | Determinismus | Niedrig |
