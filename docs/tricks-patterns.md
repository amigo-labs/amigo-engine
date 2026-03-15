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

**Lösung:** Simulation nutzt `BTreeMap` (sortiert) oder `IndexMap` (Insertion-Order). Iteration ist immer in derselben Reihenfolge. `FxHashMap` (rustc-hash) wird für O(1)-Lookups genutzt wo Iteration nicht nötig ist.

```rust
// Gut (deterministisch):
let towers: BTreeMap<EntityId, TowerData> = ...;
for (id, tower) in &towers { ... }  // immer sortiert nach ID

// Schlecht (nicht-deterministisch):
let towers: HashMap<EntityId, TowerData> = ...;
for (id, tower) in &towers { ... }  // Reihenfolge variiert!
```

**Wo:** Jede Simulation-Loop die über Entities iteriert.

---

### 25. Command-Based Architecture

**Problem:** Im Multiplayer sendet Client A "Tower platzieren bei (5,3)." Client B muss exakt dasselbe tun. Direkte ECS-Manipulation ist nicht serialisierbar.

**Lösung:** Alle Spieler-Aktionen als serialisierbare `GameCommand` Enums. Commands werden über das Netzwerk gesendet. Die Simulation führt Commands aus – nicht Raw-Input.

```rust
pub enum GameCommand {
    PlaceTower { x: Fix, y: Fix, tower_type: TowerId },
    SellTower { tower_id: EntityId },
    SetTargetPriority { tower_id: EntityId, priority: TargetPriority },
    StartWave,
    ActivateAbility { ability: AbilityId },
    // ... alle Spieler-Aktionen
}

// Replay = Vec<(Tick, GameCommand)>
// Multiplayer = Commands über UDP senden
// Undo = Command rückwärts ausführen
```

**Wo:** Jede Spieler-Interaktion. Replay-System, Multiplayer, Undo/Redo im Editor.

---

### 26. Lockstep Multiplayer

**Problem:** Zwei Spieler spielen zusammen. Die Simulation muss auf beiden identisch sein.

**Lösung:** Lockstep-Protokoll. Beide Clients laufen exakt synchron:

```
Tick 100:
  Client A sendet: [PlaceTower(5,3)]     → an Client B
  Client B sendet: [StartWave]            → an Client A

  Beide warten bis sie die Commands des anderen haben.

  Dann: beide simulieren Tick 100 mit [PlaceTower(5,3), StartWave]
  → identisches Ergebnis (dank Fixed-Point + Seeded RNG + geordnete Iteration)
```

Desync-Detection via CRC: beide Clients berechnen einen Checksum über den GameState. Unterschied → Desync-Warnung.

**Wo:** Co-op Multiplayer. Funktioniert weil die gesamte Simulation deterministisch ist.

---

## SPEICHER & PERFORMANCE

---

### 27. Object Pool (Partikel)

**Problem:** Explosion spawnt 200 Partikel, nächster Frame alle weg, 150 neue. Hunderte Allokationen pro Frame → Allocator leidet, Speicher fragmentiert.

**Lösung:** Vorallozierter Pool. Slots werden aktiviert/deaktiviert, nie alloziert/freigegeben.

```rust
pub struct ParticlePool {
    particles: Vec<Particle>,      // fest alloziert beim Start (z.B. 1000)
    active: BitSet,                 // welche Slots aktiv sind
    first_free: usize,              // nächster freier Slot (Linked-List durch freie Slots)
}

fn spawn(&mut self) -> Option<&mut Particle> {
    if self.first_free < self.particles.len() {
        let idx = self.first_free;
        self.active.set(idx, true);
        self.first_free = self.particles[idx].next_free;
        Some(&mut self.particles[idx])
    } else { None }  // Pool voll
}

fn despawn(&mut self, idx: usize) {
    self.active.set(idx, false);
    self.particles[idx].next_free = self.first_free;
    self.first_free = idx;
}
```

**Wo:** Partikel, Projektile, Floating-Text (Damage Numbers), temporäre Effekte.

---

### 28. Capacity Hints (Pre-Allocation)

**Problem:** Ein SparseSet wächst dynamisch. Jedes `Vec::push()` kann eine Reallocation auslösen (teuer, kopiert alles).

**Lösung:** Bei SparseSet-Erstellung die erwartete Entity-Anzahl angeben. Pre-Allocation einmalig beim Start.

```rust
let mut positions = SparseSet::<Position>::with_capacity(1000);
let mut velocities = SparseSet::<Velocity>::with_capacity(1000);
let mut sprites = SparseSet::<SpriteComp>::with_capacity(1000);
// → 0 Reallocations während des gesamten Spiels
```

**Wo:** Alle SparseSet-Instanzen, alle Vec-basierten Systeme, Partikel-Pools.

---

### 29. Arena Allocator (Bumpalo)

**Problem:** Pro Frame werden viele kleine, temporäre Daten erzeugt (Render-Commands, Event-Listen, Debug-Strings). `malloc`/`free` für jedes einzelne → langsam.

**Lösung:** Bumpalo Arena: ein großer Speicherblock, Allokationen "bumpen" nur einen Pointer vorwärts. Am Frame-Ende: gesamte Arena wird in einem Schritt zurückgesetzt.

```rust
let arena = Bump::new();

// Frame-Start:
let render_cmds = arena.alloc_slice_fill_default::<RenderCmd>(500);
let events = bumpalo::vec![in &arena; Event::default(); 100];
// ... benutze render_cmds und events ...

// Frame-Ende:
arena.reset();  // EIN Pointer-Reset, fertig. Keine einzelnen Frees.
```

**Wo:** Temporäre Daten pro Frame: Render-Listen, Event-Queues, Debug-Ausgaben, Spatial Hash Rebuild.

---

### 30. Memory Debug Overlay

**Problem:** Wo geht der Speicher hin? Gibt es Leaks? Wächst der VRAM?

**Lösung:** Debug Overlay (F6) zeigt live: RAM-Nutzung, VRAM-Nutzung, Entity-Count pro Typ, Partikel-Pool-Auslastung, Texture Atlas Größe.

```
┌─ Memory ──────────────────┐
│ RAM:   142 MB / 512 MB    │
│ VRAM:   48 MB / 2048 MB   │
│ Entities: 347             │
│   Position: 347           │
│   Velocity: 298           │
│   Sprite:   312           │
│ Particles: 84/1000 (8%)   │
│ Atlases: 3 (12 MB)        │
│ Audio: 8 MB               │
└───────────────────────────┘
```

**Wo:** Debug-Modus. Wird in jedem Frame aktualisiert. Leak-Detection: wenn Entity-Count stetig wächst ohne State-Wechsel → Warnung in der Konsole.

---

## INPUT

---

### 31. Action-Based Input (Abstraction Layer)

**Problem:** Der Spieler drückt "W" auf der Tastatur, "Up" auf dem D-Pad, oder schiebt den linken Stick. Alles soll "nach oben bewegen" heißen. Und der Spieler will vielleicht umbelegen.

**Lösung:** Abstraktions-Layer. Das Spiel fragt nie nach konkreten Tasten, sondern nach Actions.

```rust
// Das Spiel fragt:
if engine.input().held(Action::MoveUp) { ... }
if engine.input().just_pressed(Action::Confirm) { ... }

// Die Zuordnung kommt aus input.ron:
(
    actions: {
        "move_up": [Key(W), Key(Up), GamepadAxis(LeftStickY, Negative)],
        "confirm": [Key(Space), Key(Enter), GamepadButton(South)],
    },
)
```

Hot-reloadable. Spieler kann im Settings-Menü umbelegen. Gamepad-Support gratis.

---

### 32. Gamepad Hot-Plug

**Problem:** Spieler steckt Controller ein/aus während das Spiel läuft.

**Lösung:** Engine feuert Events: `GamepadConnected(id)` / `GamepadDisconnected(id)`. Das Spiel entscheidet was passiert (Pause? Controller-Auswahl-Screen? Ignorieren?).

```rust
for event in engine.events::<GamepadEvent>() {
    match event {
        GamepadConnected(id) => show_toast("Controller verbunden!"),
        GamepadDisconnected(id) => pause_game(),
    }
}
```

**Wo:** Jede Plattform. Besonders wichtig für Couch-Gaming.

---

## ASSET MANAGEMENT

---

### 33. Dual Asset Loader

**Problem:** Während der Entwicklung willst du Sprites direkt aus Aseprite-Dateien laden (Hot Reload!). Für den Release willst du alles in ein gepacktes Archiv (schneller, kleiner, tamper-proof).

**Lösung:** Zwei Loader hinter derselben API:

```
Dev-Modus:                        Release-Modus:
  assets/sprites/hero.aseprite      game.pak (Archiv)
  assets/sprites/tiles.png          ├── textures.atlas (gepackt)
  assets/data/player.ron            ├── data.bin (serialisiert)
  → Direkt von Disk laden           └── audio.bin (komprimiert)
  → Hot Reload bei Änderung         → Einmal beim Start laden
```

```rust
// Game-Code: identisch in beiden Modi
let sprite = engine.assets().load_sprite("sprites/hero");
let data: PlayerStats = engine.assets().load_ron("data/player.ron");
// → Der Loader entscheidet ob von Disk oder aus .pak
```

**Wo:** Überall. `amigo pack` CLI-Command packt alles für Release.

---

### 34. Hot Reload (File Watcher)

**Problem:** Sprite geändert → Alt-Tab → Spiel neustarten → 30 Sekunden warten → Ergebnis sehen. Kreativitätskiller.

**Lösung:** `notify` Crate beobachtet das Assets-Verzeichnis. Bei Dateiänderung: Asset neu laden, im Spiel ersetzen. Ohne Neustart.

```
1. Artist ändert hero.aseprite in Aseprite, speichert
2. notify feuert FileChanged("assets/sprites/hero.aseprite")
3. Engine parst die Datei neu
4. Sprite-Handle zeigt auf neue Daten
5. Nächster Frame: neues Sprite sichtbar

Gesamtzeit: < 100ms
```

Funktioniert für: Sprites, Tilemaps, RON-Dateien (Stats, Configs), Audio, Shader. Nicht im Release-Modus (kein File Watcher nötig wenn alles in .pak ist).

**Wo:** Entwicklung. Besonders mächtig mit Art Studio: Sprite generiert → in Assets-Ordner → sofort im Spiel sichtbar.

---

### 35. Synchronous Loading (Cartridge Style)

**Problem:** Asynchrones Asset-Loading ist komplex (Futures, Loading-States, Placeholder-Textures). Für ein Pixel Art Spiel mit kleinen Assets: Overkill.

**Lösung:** Assets werden synchron beim Start geladen. Wie eine Spielkonsolen-Cartridge: alles da, sofort verfügbar. Async nur an Level-Transitions (wo eh ein Ladescreen angezeigt wird).

```rust
// Startup: synchron, blockierend
let assets = engine.assets().load_all("assets/");  // alles laden, fertig

// Level-Transition: async mit Ladescreen
engine.load_async("levels/world_3/", |progress| {
    render_loading_screen(progress);  // 0% ... 50% ... 100%
});
```

**Wo:** Startup, Level-Transitions. Im Game Loop: keine Ladeoperationen, alles sofort da.

---

## SAVE & REPLAY

---

### 36. Save-Slot System

**Problem:** Spieler will mehrere Speicherstände. Autosave soll nicht den manuellen Save überschreiben. Corrupted Saves sollen erkannt werden.

**Lösung:** Slot-basiertes System mit Metadata, Kompression und Integritätsprüfung.

```rust
pub struct SaveSlot {
    pub slot_id: u8,
    pub metadata: SlotInfo,     // Timestamp, Spielzeit, Label – OHNE den Save zu laden
    pub data: Vec<u8>,          // LZ4-komprimierter GameState
    pub crc: u32,               // Corruption-Check
}
```

- **Autosave:** Rotierende N Slots (Autosave_1, Autosave_2, ...) bei konfigurierbarem Intervall
- **Quicksave/Quickload:** F5/F9
- **Plattform-aware:** Windows `AppData`, Linux `~/.local/share`
- **SlotInfo:** Lesbar ohne den ganzen Save zu laden → schnelle Slot-Übersicht im Menü

---

### 37. Replay System

**Problem:** "Wie hat der Spieler das Level geschafft?" für Debugging, Sharing, Leaderboards.

**Lösung:** Replays = Liste von `(Tick, GameCommand)`. Abspielen = frischer GameState + Commands einspeisen.

```rust
pub struct Replay {
    pub seed: u64,                          // RNG-Seed
    pub commands: Vec<(u64, GameCommand)>,   // (Tick, Command)
}

// Aufnehmen:
replay.commands.push((current_tick, command.clone()));

// Abspielen:
let mut state = GameState::new(replay.seed);
for (tick, cmd) in &replay.commands {
    while state.tick < *tick { state.simulate_tick(); }
    state.execute_command(cmd);
}
```

Funktioniert dank Determinismus (Fixed-Point + Seeded RNG + geordnete Iteration). Replay-Dateien sind winzig: nur Commands, kein Full-State.

---

## DEBUG

---

### 38. Visual Debug Layers (F-Keys)

**Problem:** Wo sind die Kollisionsboxen? Wo laufen die Pfade? Warum schießt der Tower nicht?

**Lösung:** F-Key Toggles für visuelle Debug-Overlays:

| Key | Overlay | Zeigt |
|-----|---------|-------|
| F1 | HUD | FPS, Entity Count, Draw Calls, Memory |
| F2 | Grid | Tile-Grid Linien |
| F3 | Collision | AABB-Boxen aller Collider |
| F4 | Pathfinding | A*-Pfade, Flow Fields, Waypoints |
| F5 | Spawn/Build Zones | Wo Entities spawnen / platziert werden können |
| F6 | Memory | RAM/VRAM, Pool-Auslastung, Entity-Typen |
| F7 | Entity List | Alle Entities mit Komponenten |
| F8 | Network | Lockstep-Stats, Latenz, Desync-Warnings |

Alle hinter `#[cfg(debug_assertions)]` – existieren nicht im Release Build.

---

### 39. Tracy Integration

**Problem:** "Das Spiel ruckelt bei Wave 5 mit 200 Enemies." Wo genau ist der Bottleneck?

**Lösung:** Tracy Profiler Integration über `tracing` + `tracy-client`. Jedes System, jeder Render-Pass, jede Heavy-Operation ist instrumentiert.

```rust
#[tracing::instrument]
fn physics_system(world: &mut World) {
    // ... Tracy sieht: physics_system dauert 2.3ms
}

#[tracing::instrument]
fn render_entities(renderer: &mut Renderer) {
    // ... Tracy sieht: render_entities dauert 1.1ms
}
```

Tracy zeigt: Timeline aller Systeme, CPU-Flamegraph, Memory-Allokationen, GPU-Timings. Goldstandard für Game-Performance-Analyse.

---

### 40. State Snapshot to File

**Problem:** "Es gibt einen Bug bei Wave 7 wenn der Spieler 3 Cannon-Towers hat." Wie reproduzieren?

**Lösung:** Jederzeit den kompletten GameState als RON-Datei dumpen. Laden → exakt an diesem Punkt weiterspielen.

```bash
# In-Game: F10 drückt
# → saves debug_snapshot_2026-03-15_14-23-01.ron

# Claude Code oder Entwickler:
amigo run --load-snapshot debug_snapshot_2026-03-15_14-23-01.ron
# → Spiel startet exakt in diesem Zustand
```

**Wo:** Bug-Reports, AI-Playtesting (Claude Code macht Snapshot → analysiert → ändert Code → lädt Snapshot).

---

## SPEZIAL-FEATURES

---

### 41. Event System (Double Buffer)

**Problem:** System A feuert Event → System B soll reagieren. Aber System B lief schon VOR System A → sieht das Event nicht.

**Lösung:** Zwei Vektoren pro Event-Typ. Write-Buffer (aktueller Tick) und Read-Buffer (vorheriger Tick). Am Tick-Ende: swap.

```
Tick 5:
  Write: [EnemyDied(42)]       ← Systeme schreiben hier
  Read:  [WaveStarted]          ← Systeme lesen hier (Events von Tick 4)

Ende Tick 5:  Swap!

Tick 6:
  Write: (leer)
  Read:  [EnemyDied(42)]        ← jetzt sichtbar für alle
```

Events leben genau 1 Tick zum Lesen. 1 Tick Verzögerung (~16ms) – nicht wahrnehmbar. Keine Race Conditions.

---

### 42. Atmosphere System (Smooth Interpolation)

**Problem:** Die Lichtstimmung soll sich ändern wenn ein Boss spawnt. Abrupter Wechsel fällt auf.

**Lösung:** `atmosphere.transition_to("boss", duration)` startet eine Interpolation. Alle Atmosphären-Parameter (Licht, Farbe, Wetter-Intensität, Post-Effects, Music) werden über `duration` Sekunden smooth übergeblendet.

```rust
pub struct Atmosphere {
    current: AtmosphereState,
    target: Option<(AtmosphereState, f32, f32)>,  // (target, duration, progress)
}

fn update_atmosphere(atm: &mut Atmosphere, dt: f32) {
    if let Some((target, duration, progress)) = &mut atm.target {
        *progress += dt / *duration;
        atm.current = AtmosphereState::lerp(&atm.current, target, *progress);
        if *progress >= 1.0 { atm.target = None; }
    }
}
```

**Wo:** Welt-Stimmung (calm → storm), Boss-Encounters, Tag/Nacht-Zyklen, Dimension-Wechsel.

---

### 43. Scene Stack

**Problem:** Gameplay läuft → Pause-Menü öffnet → das Gameplay soll "eingefroren" im Hintergrund bleiben, nicht despawnt werden.

**Lösung:** Scenes als Stack. Die oberste Scene ist aktiv. Darunter liegende Scenes sind pausiert aber noch da.

```
Stack:
  ┌────────────┐
  │ Pause Menu │  ← aktiv, rendert über dem Gameplay
  ├────────────┤
  │ Gameplay   │  ← pausiert, nicht despawnt, wird noch gerendert (gedimmt)
  ├────────────┤
  │ (base)     │
  └────────────┘

// Push → Pause öffnet sich über dem Gameplay
// Pop → zurück zum Gameplay, genau wo es war
```

**Wo:** Pause, Inventar-Overlay, Dialogue über Gameplay, Cutscene-Overlay.

---

### 44. Screen Transitions (Shader-based)

**Problem:** Welt-Wechsel: der Bildschirm soll nicht einfach hart schneiden.

**Lösung:** Transition-Effekte als Post-Processing Shader. Die aktuelle Szene rendert in Texture A, die neue in Texture B, der Transition-Shader mischt.

```rust
pub enum Transition {
    Fade { duration: f32, color: Color },           // Fade to black
    Dissolve { duration: f32, noise: TextureId },    // Pixel lösen sich auf
    Wipe { duration: f32, direction: Direction },    // Schieben
    Circle { duration: f32, center: Vec2 },          // Kreis öffnet/schließt
    VHSStatic { duration: f32 },                     // Für Threadwalker Dimension-Flip
    Custom { shader: ShaderId, duration: f32 },      // Custom WGSL
}
```

**Wo:** Welt-Wechsel (Loom → World), Cutscene-Übergänge, Tod/Respawn, Dimension-Flip (Stranger Things → VHS Static).

---

*Dieses Dokument ist ein Nachschlagewerk. Alle Techniken sind in der Engine-Spec (amigo-engine-complete.md) verankert, hier werden sie erklärt und illustriert.*
