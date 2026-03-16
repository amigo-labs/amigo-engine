# amigo-game – Project Setup & Implementation Plan

## Repository: amigo-labs/amigo-game

---

## 1. Repo-Struktur

```
amigo-game/
├── Cargo.toml                        # Workspace root
├── .gitignore
├── README.md
├── CLAUDE.md                          # Claude Code Kontext (Thanos-Style)
│
├── docs/
│   └── specs/
│       ├── threadwalker-spec.md       # Game Design (Story, Welten, Charaktere)
│       ├── amigo-td-spec.md           # TD-spezifisches Game Design
│       ├── amigo-td-ui-spec.md        # UI/UX Design
│       ├── amigo-artgen-spec.md       # Art Pipeline
│       └── amigo-audiogen-spec.md     # Audio Pipeline
│
├── game/                              # Haupt-Crate: das Spiel
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                    # Entry Point
│       ├── lib.rs                     # Game struct, implements engine's Game trait
│       ├── states.rs                  # GameState enum (Loading, Loom, InWorld, Pause, ...)
│       │
│       ├── loom/                      # Worldmap ("Das Loom")
│       │   ├── mod.rs
│       │   ├── loom_state.rs          # Loom navigation state
│       │   ├── loom_renderer.rs       # Fäden, Knoten, Partikel
│       │   ├── world_node.rs          # Welt-Knoten (locked/unlocked/completed)
│       │   └── transition.rs          # Loom → World Übergang
│       │
│       ├── player/                    # Amigo (persistent across worlds)
│       │   ├── mod.rs
│       │   ├── amigo.rs               # Amigo's core data (HP, inventory, arm state)
│       │   ├── arm.rs                 # Der mechanische Arm (Thread-Sense/Grip/Cut)
│       │   ├── inventory.rs           # Cross-World Items
│       │   └── journal.rs             # Orins Journal, Lore-Einträge, Sammelstücke
│       │
│       ├── bessi/                     # Companion
│       │   ├── mod.rs
│       │   ├── bessi.rs               # Bessi state, form evolution
│       │   └── bessi_behavior.rs      # Per-world Verhalten, Reaktionen
│       │
│       ├── worlds/                    # Welt-Implementierungen
│       │   ├── mod.rs                 # World trait + Registry
│       │   ├── meridian/              # Welt 1: Hub (RPG)
│       │   │   ├── mod.rs
│       │   │   └── ...
│       │   ├── kippfels/              # Welt 2: Platformer
│       │   ├── rostgarten/            # Welt 3: TD
│       │   ├── kabinett/              # Welt 4: Dungeon Crawler
│       │   ├── meer/                  # Welt 5: Stealth
│       │   ├── amboss/                # Welt 6: Crafting/Farming
│       │   ├── sporenwolke/           # Welt 7: Pokémon-style
│       │   ├── frequenz/              # Welt 8: Rhythm-Shmup
│       │   ├── knochenhain/           # Welt 9: Survival
│       │   ├── parlament/             # Welt 10: Taktik-RPG
│       │   ├── glaszeit/              # Welt 11: Puzzle-Adventure
│       │   ├── inverse_meridian/      # Welt 12: Story RPG
│       │   └── archiv/               # Welt 13: Roguelike Finale
│       │
│       ├── story/                     # Story-System
│       │   ├── mod.rs
│       │   ├── flags.rs               # Story-Flags (global state)
│       │   ├── dialogue.rs            # Dialogue-System
│       │   └── acts.rs                # Akt-Tracking (I, II, III)
│       │
│       ├── cross_world/               # Cross-World Systeme
│       │   ├── mod.rs
│       │   ├── items.rs               # Cross-World Item Definitionen
│       │   ├── dependencies.rs        # Welches Item braucht welche Welt
│       │   └── crafting.rs            # Grist's Schmiede-System
│       │
│       └── ui/                        # Game-spezifische UI
│           ├── mod.rs
│           ├── loom_ui.rs             # Worldmap UI
│           ├── hud.rs                 # In-World HUD (variiert per Genre)
│           ├── inventory_ui.rs        # Inventar-Anzeige
│           ├── journal_ui.rs          # Journal-UI
│           └── dialogue_ui.rs         # Dialogue-Box
│
├── assets/
│   ├── amigo.toml                     # Engine Config
│   ├── input.ron                      # Key Bindings
│   │
│   ├── data/
│   │   ├── worlds.ron                 # Welt-Registry (ID, Name, Genre, Dependencies)
│   │   ├── items.ron                  # Cross-World Items
│   │   ├── story_flags.ron            # Story-Flag Definitionen
│   │   └── worlds/
│   │       ├── meridian.world.ron
│   │       ├── kippfels.world.ron
│   │       └── ...
│   │
│   ├── sprites/
│   │   ├── amigo/                     # Amigo Sprites (all animations)
│   │   ├── bessi/                     # Bessi Sprites (form variants)
│   │   ├── loom/                      # Worldmap Assets
│   │   ├── ui/                        # UI Frames, Icons
│   │   └── worlds/                    # Per-World Sprites
│   │       ├── meridian/
│   │       ├── kippfels/
│   │       └── ...
│   │
│   └── audio/
│       ├── music/
│       │   ├── loom/                  # Worldmap Musik
│       │   └── worlds/                # Per-World Musik + Stems
│       ├── sfx/
│       └── ambient/
│
└── tools/                             # Optional: Game-spezifische Tools
    └── ...
```

---

## 2. Cargo.toml

### Workspace Root

```toml
# amigo-game/Cargo.toml
[workspace]
members = ["game"]
resolver = "2"
```

### Game Crate

```toml
# amigo-game/game/Cargo.toml
[package]
name = "threadwalker"
version = "0.1.0"
edition = "2024"

[dependencies]
amigo-engine = { git = "https://github.com/amigo-labs/amigo-engine.git", branch = "main" }

# Game-spezifische Dependencies (falls nötig)
# rand = "0.8"          # falls Engine's RNG nicht reicht
# serde_json = "1.0"    # falls für Dialogue-System benötigt
```

Wenn du lokal entwickelst und die Engine parallel ändern willst, temporär umschalten:

```toml
# Lokale Entwicklung (nicht committen):
# amigo-engine = { path = "../../amigo-engine" }
```

Oder via Cargo's `[patch]` Section in der Workspace-Root:

```toml
# amigo-game/Cargo.toml
[workspace]
members = ["game"]
resolver = "2"

# Nur für lokale Entwicklung, entfernen vor Push:
# [patch."https://github.com/amigo-labs/amigo-engine.git"]
# amigo-engine = { path = "../amigo-engine" }
```

---

## 3. CLAUDE.md

```markdown
# THREADWALKER – Claude Code Context

## Project
Multi-genre pixel art adventure game built on Amigo Engine.
13 worlds, each a different genre (TD, Platformer, RPG, Shmup, etc.).

## Architecture
- Engine: amigo-engine (git dependency)
- Game: `game/` crate, implements engine's Game trait
- Each world is a module under `game/src/worlds/`
- Player (Amigo), companion (Bessi), inventory persist across worlds
- Cross-world items create dependencies between worlds

## Specs (read these first!)
- `docs/specs/threadwalker-spec.md` – Story, worlds, characters
- `docs/specs/amigo-td-spec.md` – TD gameplay (for Rostgarten world)
- `docs/specs/amigo-td-ui-spec.md` – UI/UX patterns
- `docs/specs/amigo-artgen-spec.md` – Art generation pipeline
- `docs/specs/amigo-audiogen-spec.md` – Audio generation pipeline

## Current Phase
Phase 0: Worldmap (Loom) + Navigation + Amigo core + World loading

## Conventions
- All game data in RON (assets/data/)
- Hot-reloadable assets in dev mode
- Pixel art at 640x360 virtual resolution
- All game state serializable (save/load)
```

---

## 4. Kern-Architekturen

### Game State Machine

```rust
pub enum GameState {
    Loading,                          // Startup, asset loading
    TitleScreen,                      // Main menu
    Loom,                             // Worldmap navigation
    WorldTransition(WorldId),         // Fade/animation between Loom and World
    InWorld(WorldId),                 // Active gameplay in a specific world
    Pause,                            // Pause overlay
    Cutscene(CutsceneId),            // Story cutscenes
    Dialogue(DialogueId),             // Dialogue sequences
}
```

### World Trait

Jede Welt implementiert dieses Trait. Die Engine kennt nur das Trait, nicht die konkreten Welten.

```rust
pub trait World {
    /// Einmalige Initialisierung beim ersten Betreten
    fn init(&mut self, engine: &mut Engine, player: &PlayerData);

    /// Jeden Frame aufgerufen wenn die Welt aktiv ist
    fn update(&mut self, engine: &mut Engine, player: &mut PlayerData, dt: f32);

    /// Rendering
    fn render(&self, engine: &Engine, renderer: &mut Renderer);

    /// Aufgerufen wenn Spieler die Welt verlässt
    fn on_exit(&mut self, engine: &mut Engine, player: &mut PlayerData);

    /// Aufgerufen wenn Spieler die Welt erneut betritt
    fn on_reenter(&mut self, engine: &mut Engine, player: &mut PlayerData);

    /// Welt-spezifische UI
    fn render_ui(&self, engine: &Engine, ui: &mut UiContext, player: &PlayerData);

    /// Metadata
    fn id(&self) -> WorldId;
    fn genre(&self) -> Genre;
    fn is_complete(&self) -> bool;
    fn star_rating(&self) -> Option<u8>;
}
```

### World Registry

```rust
pub struct WorldRegistry {
    worlds: HashMap<WorldId, Box<dyn World>>,
    definitions: HashMap<WorldId, WorldDefinition>,
}

pub struct WorldDefinition {
    pub id: WorldId,
    pub name: String,
    pub genre: Genre,
    pub locked: bool,
    pub required_items: Vec<ItemId>,    // Cross-world dependencies
    pub position_on_loom: (f32, f32),   // Position auf der Worldmap
    pub connections: Vec<WorldId>,       // Verbundene Welten (Fäden)
}

pub enum Genre {
    Hub,
    Platformer,
    TowerDefense,
    DungeonCrawler,
    Stealth,
    FarmingCrafting,
    Collection,
    RhythmShmup,
    Survival,
    TacticsRPG,
    PuzzleAdventure,
    StoryRPG,
    Roguelike,
}
```

### Player Data (persistent)

```rust
pub struct PlayerData {
    // Amigo
    pub name: String,                   // "Amigo"
    pub hp: u32,
    pub max_hp: u32,
    pub arm_state: ArmState,            // Wächst über die Story

    // Bessi
    pub bessi: BessiState,

    // Inventar
    pub inventory: Inventory,           // Cross-World Items
    pub journal: Journal,               // Lore, Notizen, Sammelstücke
    pub gold: u32,                      // Universelle Währung? Oder per-World?

    // Story
    pub story_flags: HashSet<StoryFlag>,
    pub current_act: Act,
    pub worlds_visited: HashSet<WorldId>,
    pub worlds_completed: HashSet<WorldId>,
    pub total_stars: u32,

    // Save metadata
    pub playtime: f64,
    pub save_slot: u8,
}

pub enum ArmState {
    Elbow,          // Welt 1-3: bis zum Ellenbogen
    Shoulder,       // Welt 4-6: bis zur Schulter
    Chest,          // Welt 7-9: über die Brust
    Half,           // Welt 10-12: halber Körper
    Full,           // Welt 13: fast vollständig
    Resolved,       // Geheimes Ende: der Arm wird still
}
```

---

## 5. Phase 0: Loom + Navigation

### Was gebaut werden muss

1. **Game Skeleton** – main.rs, Game trait implementieren, GameState machine
2. **Loom Renderer** – Worldmap als kosmisches Web, Knoten + Fäden + Partikel
3. **Loom Navigation** – Cursor bewegt sich entlang Fäden zwischen Knoten, Gamepad + Mouse
4. **World Node UI** – Knoten anwählen → Info-Panel (Name, Genre, Status, Dependencies)
5. **Lock/Unlock System** – Items prüfen, gesperrte Welten anzeigen
6. **World Transition** – Knoten bestätigen → Fade → World laden
7. **Amigo Core** – PlayerData struct, Serialization, Save/Load
8. **Stub World** – Eine leere "Test-Welt" die nur einen Raum zeigt und zurück zum Loom lässt
9. **Bessi Idle** – Bessi schwebt neben dem Cursor auf dem Loom, pulsiert, reagiert auf Hover

### Loom Visuell

```
    Hintergrund: Tiefes Dunkelblau/Schwarz, langsame Nebel-Partikel

    Knoten: Leuchtende Kreise, 16x16 Sprites
      - Unlocked + Unvisited: weißes Pulsieren
      - Unlocked + Visited: Weltfarbe (Kupfer für Rostgarten, Grün für Sporenwolke...)
      - Completed: Volle Farbe + Sterne darunter
      - Locked: Grau, leichtes Flackern, Schloss-Icon

    Fäden: Leuchtende Linien zwischen verbundenen Knoten
      - Aktiv (freigeschaltet): sanftes Leuchten, Partikel fließen entlang
      - Inaktiv (gesperrt): dünn, grau, kaum sichtbar
      - Amigos Position: heller Punkt auf dem aktuellen Knoten

    Bessi: Schwebt neben Amigos Position, ihre Form flackert sanft

    UI: Unten: ausgewählte Welt Info
      ┌────────────────────────────────────────────────┐
      │  ROSTGARTEN                    ★★☆  Genre: TD  │
      │  "Der letzte lebende Garten"                    │
      │  Benötigt: Gravitit-Kristalle [✓]              │
      │                                                  │
      │  [A] Betreten    [B] Zurück                     │
      └────────────────────────────────────────────────┘
```

### Worldmap-Daten

```ron
// assets/data/worlds.ron
(
    worlds: [
        (
            id: "meridian",
            name: "Meridian",
            subtitle: "Die Stadt zwischen den Fäden",
            genre: Hub,
            position: (240.0, 135.0),    // Mitte
            color: (0.9, 0.85, 0.7),
            locked: false,                // Immer offen
            required_items: [],
            connections: ["kippfels", "rostgarten", "kabinett"],
        ),
        (
            id: "kippfels",
            name: "Kippfels",
            subtitle: "Wo die Schwerkraft dem Blick folgt",
            genre: Platformer,
            position: (120.0, 60.0),
            color: (0.6, 0.7, 0.9),
            locked: false,                // Einstiegswelt
            required_items: [],
            connections: ["meridian", "frequenz"],
        ),
        (
            id: "rostgarten",
            name: "Rostgarten",
            subtitle: "Der letzte lebende Garten",
            genre: TowerDefense,
            position: (360.0, 80.0),
            color: (0.8, 0.5, 0.3),
            locked: true,
            required_items: ["gravitit_crystal"],
            connections: ["meridian", "kabinett", "sporenwolke"],
        ),
        // ... weitere Welten
    ],

    // Fäden die keine direkten Welt-Connections sind
    // (visuelle Verbindungen für Story-Beats)
    story_threads: [
        (from: "knochenhain", to: "archiv", visible_after: "act3_start"),
    ],
)
```

---

## 6. Implementierungsreihenfolge

```
Phase 0: Loom + Skeleton (2-3 Wochen)
├── Game Skeleton (main.rs, states, Game trait)
├── PlayerData + Save/Load
├── Loom Renderer (Knoten, Fäden, Partikel)
├── Loom Navigation (Cursor, Gamepad, World Select)
├── Lock/Unlock System
├── World Transition (Fade, Load)
├── Stub World (leerer Raum, zurück zum Loom)
└── Bessi Idle auf Loom

Phase 1: Erste spielbare Welt (3-4 Wochen)
├── Welt 2: Kippfels (Platformer)
│   → Einfachster Genre-Einstieg
│   → Gravity-Mechanic als Proof of Concept
│   → Erster Cross-World Item Drop (Gravitit-Kristalle)
└── Amigo Sprite + Basis-Animationen

Phase 2: Zweite Welt + Cross-World (3-4 Wochen)
├── Welt 3: Rostgarten (TD)
│   → Nutzt das komplette amigo-td-spec Design
│   → Braucht Gravitit → testet Dependencies
│   → Gibt Lebend-Samen → testet Item-Flow
├── Cross-World Item System
└── Grist's Schmiede (Basis-Crafting)

Phase 3: Hub + Story (3-4 Wochen)
├── Welt 1: Meridian (Hub)
│   → NPC-System, Dialogue, Shops
│   → Orins Journal UI
│   → Amigos Wohnung (Save Point)
├── Story-Flag System
└── Akt I Cutscenes

Phase 4+: Weitere Welten
├── Pro Welt: 2-4 Wochen je nach Genre-Komplexität
├── Reihenfolge nach Dependency-Chain:
│   Kabinett(4) → Meer(5) → Amboss(6) → Sporenwolke(7) →
│   Frequenz(8) → Knochenhain(9) → Parlament(10) →
│   Glaszeit(11) → Inverser Meridian(12) → Archiv(13)
└── Story-Beats zwischen den Welten
```

---

## 7. Quick Start: Repo anlegen

```bash
# Repo erstellen
mkdir amigo-game && cd amigo-game
git init

# Workspace + Game Crate
cargo init --name threadwalker game

# Docs
mkdir -p docs/specs
# → Specs hierhin kopieren

# Assets
mkdir -p assets/{data,sprites/{amigo,bessi,loom,ui},audio/{music,sfx,ambient}}

# CLAUDE.md
# → CLAUDE.md erstellen (siehe oben)

# Ersten Commit
git add .
git commit -m "Initial project setup: Threadwalker"

# Remote
git remote add origin git@github.com:amigo-labs/amigo-game.git
git push -u origin main
```

---

*Dieses Dokument beschreibt das Setup für amigo-labs/amigo-game. Für die Engine, siehe amigo-labs/amigo-engine. Für Game Design, siehe docs/specs/threadwalker-spec.md.*
