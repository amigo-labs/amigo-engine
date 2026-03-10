# Plan: 4 neue Engine-Systeme (nur Engine-Level)

## Prinzip: Was gehört in die Engine?
- **Engine**: Generische Mechaniken, die in vielen Games wiederverwendbar sind
- **Game**: Konkreter Content (welche Crops, welche Bullets, welche Puzzle-Steine)
- Alles hier ist **datengetrieben** — das Game definiert die Daten, die Engine führt die Logik aus

---

## 1. `farming.rs` — Growth & Calendar System

**Engine-Level Mechaniken:**

### GrowthStage System
- `GrowthStage { id, duration_ticks, next_stage }` — generische Wachstums-Stages
- `GrowthDef { stages: Vec<GrowthStage>, requires_water, requires_light }` — Definition einer wachsenden Sache
- `GrowthInstance { def_id, current_stage, ticks_in_stage, watered, lit }` — laufende Instanz
- `tick_growth()` — advanced alle Instanzen, gibt Events zurück (StageChanged, Completed, Withered)

### Calendar System
- `Calendar { day, season, year, ticks_per_day }` — Zeitrechnung
- `Season` enum (Spring, Summer, Autumn, Winter) — je 28 Tage
- `tick_calendar()` — advanced Kalender, gibt DayChanged/SeasonChanged Events
- `TimeOfDay { hour, minute }` — abgeleitet aus Tick-Position im Tag

### TileGrid (interaktives Grid)
- `FarmTile { soil_state, moisture, fertility, content }` — generischer Tile
- `SoilState` enum (Empty, Tilled, Planted, Watered)
- `FarmGrid { width, height, tiles }` — Grid mit Tile-Operationen
- `till()`, `water()`, `plant()`, `harvest()` — Operationen die Events erzeugen

**NICHT in der Engine:** Welche Pflanzen es gibt, Preise, NPC-Dialoge, Shop-System (Dialog + Inventory existieren bereits)

---

## 2. `bullet_pattern.rs` — Bullet Spawner System

**Engine-Level Mechaniken:**

### BulletPool
- `Bullet { position, velocity, lifetime, damage, radius, active }` — einzelnes Projektil
- `BulletPool { bullets: Vec<Bullet>, active_count }` — Object Pool für Performance
- `spawn()`, `despawn()`, `tick()` — Pool-Management
- Collision-Check gegen Rect/Circle Shapes (nutzt existierende collision.rs)

### Pattern System
- `PatternShape` enum:
  - `Radial { count, speed }` — gleichmäßig im Kreis
  - `Spiral { count, speed, rotation_speed }` — rotierende Spirale
  - `Aimed { count, spread_angle, speed }` — auf Ziel gerichtet mit Streuung
  - `Wave { count, amplitude, frequency, speed }` — Sinuswelle
  - `Random { count, min_speed, max_speed }` — zufällige Richtungen
- `BulletEmitter { position, pattern, fire_rate, timer, rotation }` — feuert Patterns
- `tick_emitters()` — advanced alle Emitter, spawnt Bullets in Pool

### Boss Pattern Sequencer
- `PhasePattern { emitters: Vec<BulletEmitter>, duration_ticks }` — eine Phase
- `PatternSequence { phases, current_phase, loop_mode }` — Sequenz von Phasen
- `LoopMode` enum (Once, Loop, PingPong)

**NICHT in der Engine:** Konkrete Boss-Patterns, Bullet-Sprites, Scoring, Graze-Reward-Logik

---

## 3. `puzzle.rs` — Grid Puzzle System

**Engine-Level Mechaniken:**

### PuzzleGrid
- `Cell<T: Copy + Eq>` — generische Zelle (T = game-defined enum für Farben/Typen)
- `PuzzleGrid<T> { width, height, cells }` — 2D Grid
- `get()`, `set()`, `swap()` — Basis-Operationen
- `apply_gravity(direction)` — Zellen fallen in eine Richtung (für Match-3, Tetris)
- `is_empty()`, `find_all(predicate)` — Queries

### Pattern Matching
- `MatchResult { cells: Vec<(u32, u32)>, pattern_type }` — gefundenes Match
- `find_horizontal_matches(min_length)` — Reihen finden
- `find_vertical_matches(min_length)` — Spalten finden
- `find_connected(x, y, predicate)` — Flood-fill für gleiche Zellen (Puzzle Bobble)
- `clear_matches()` — gefundene Matches entfernen, gibt ClearEvent zurück

### Move System
- `PuzzleMove` enum (Swap, Insert, Rotate, Slide) — generische Spielzüge
- `MoveHistory { moves: Vec<PuzzleMove>, cursor }` — Undo/Redo Stack
- `apply_move()`, `undo()`, `redo()` — Move-Management
- `validate_move()` — prüft ob ein Zug legal ist (callback-basiert)

### Block Spawning (für Tetris-artige Games)
- `BlockShape { cells: Vec<(i32, i32)> }` — relative Zellpositionen
- `rotate_cw()`, `rotate_ccw()` — Rotation
- `can_place(grid, x, y)`, `place(grid, x, y)` — Platzierung
- `BlockBag { shapes, remaining }` — faire Randomisierung (7-bag system)

**NICHT in der Engine:** Konkrete Puzzle-Regeln (wie viele Farben, Punkte pro Match, Level-Design)

---

## 4. `platformer.rs` — Platformer Controller System

**Engine-Level Mechaniken:**

### PlatformerController
- Input-Buffering: `JumpBuffer { buffered, buffer_ticks }` — Jump-Input wird N Ticks gespeichert
- `CoyoteTime { grounded_ticks, coyote_ticks }` — nach Kante verlassen noch springen
- `VariableJump { jump_velocity, cut_multiplier, holding }` — kurz drücken = kleiner Sprung
- `WallInteraction { wall_slide_speed, wall_jump_velocity, wall_jump_direction }` — Wall Slide/Jump
- `DashState { can_dash, dash_speed, dash_duration, dash_cooldown, dashing }` — Dash/Dodge
- `PlatformerState` — kombiniert alles, `tick()` gibt gewünschte Velocity zurück

### Ground Detection
- `GroundCheck { is_grounded, was_grounded, ground_normal, on_slope }` — Bodenerkennung
- `check_ground(position, shape, tilemap_query)` — prüft Boden unter Füßen
- Slope-Handling: Geschwindigkeit auf Slopes anpassen

### Moving Platforms
- `PlatformPath { waypoints: Vec<RenderVec2>, speed, mode }` — Pfad-Definition
- `PathMode` enum (Loop, PingPong, Once)
- `MovingPlatform { path, current_segment, t, riders }` — Platform-Instanz
- `tick_platforms()` — bewegt Platforms, verschiebt Riders mit

### One-Way & Through Platforms
- `PlatformType` enum (Solid, OneWay, Through) — Already partly in tilemap
- `should_collide(player_velocity, platform_type)` — Kollisions-Logik

**NICHT in der Engine:** Character-Sprites, Level-Design, feindespezifische AI, Power-Ups

---

## Implementation Order
1. `platformer.rs` — baut auf existierender physics.rs + collision.rs auf
2. `farming.rs` — unabhängig, nutzt scheduler Konzept
3. `bullet_pattern.rs` — nutzt existierende collision.rs
4. `puzzle.rs` — komplett unabhängig

## Conventions
- Builder Pattern mit `.with_*()` für alle Config-Structs
- Serde Serialize/Deserialize wo möglich (kein RenderVec2 in serialisierten Structs)
- Interne XorShift RNG wo Randomisierung nötig
- Fix (I16F16) nur wo Determinismus nötig, sonst f32
- Events als Return-Values statt Callbacks
- Tests für jedes Modul
