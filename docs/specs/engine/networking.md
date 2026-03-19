---
status: done
crate: amigo_net
depends_on: ["engine/core"]
last_updated: 2026-03-16
---

# Command System & Networking

## Purpose

Provides the command-based input system, transport abstraction (local and networked), fully serializable game state, multiplayer protocol, and replay system. All player input becomes serializable commands -- no direct state mutation. This separation enables multiplayer, replays, save/load, and AI control through the same interface.

## Public API

### Commands

All player input becomes serializable commands. No direct state mutation.

```rust
#[derive(Clone, Serialize, Deserialize)]
pub enum GameCommand {
    PlaceTower { pos: IVec2, tower_type: TowerTypeId },
    SellTower { tower_id: EntityId },
    UpgradeTower { tower_id: EntityId, path: UpgradePath },
    StartWave,
    Pause,
    Unpause,
    // ...
}
```

### Transport Trait

```rust
pub trait Transport {
    fn send(&mut self, commands: &[GameCommand]);
    fn receive(&mut self) -> Vec<(PlayerId, Vec<GameCommand>)>;
}

// LocalTransport: singleplayer (zero overhead)
// NetworkTransport: multiplayer (UDP via laminar)
```

### GameState (Fully Serializable)

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct GameState {
    pub tick: u64,
    pub rng: SerializableRng,
    pub gold: i32,
    pub lives: i32,
    pub wave: WaveState,
    pub towers: EntityPool<Tower>,
    pub enemies: EntityPool<Enemy>,
    pub projectiles: EntityPool<Projectile>,
    pub tilemap: TileMap,
}
```

## Behavior

### Multiplayer (Phase 2+)

- **Co-op (2-4 players):** Shared map, lockstep protocol
- **Competitive:** Own maps, send waves to opponent
- **Spectator:** Receive-only

### Replay System

Commands logged with tick numbers. Replay = feed commands into fresh GameState.

## Cross-System Multiplayer Boundaries

Definiert welche Engine-Systeme im Multiplayer synchronisiert werden und welche lokal bleiben.

### Synchronisiert (über GameCommand-Protokoll)

| System | Sync-Methode | Details |
|--------|-------------|---------|
| **Simulation** | Lockstep | Alle Spieler laufen den gleichen Tick mit gleichen Commands |
| **Tilemap** | Deterministisch | Gleicher Seed → gleiche Map. Tile-Mutations via GameCommand |
| **Pathfinding** | Deterministisch | Gleicher SimVec2-Input → gleiche Pfade (Fixed-Point) |
| **Inventory/Crafting** | GameCommand | `CraftItem`, `MoveItem` als Commands. Shared Inventory im Co-op |
| **Waves/Spawning** | Deterministisch | SerializableRng garantiert gleiche Spawn-Reihenfolge |
| **Save/Load** | Host-Only | Nur Host speichert/lädt. Clients synchronisieren via State-Snapshot |
| **SimSpeed** | Host-Authoritative | Nur Host darf `Pause`/`SetSpeed`. Broadcast an alle Clients |

### Nicht synchronisiert (lokal pro Client)

| System | Grund |
|--------|-------|
| **Physics (RigidBody)** | Visuell, f32, nicht deterministisch. Ragdolls/Partikel dürfen sich unterscheiden |
| **Rendering/Camera** | Jeder Client hat eigenen Viewport, Zoom, Shake |
| **Audio** | Lokale Wiedergabe, kein Sync nötig |
| **Particles** | Rein visuell |
| **UI/Editor** | Lokal |
| **Debug Overlay** | Lokal |

### Dialogue im Co-op

Dialogue-Interaktionen im Multiplayer folgen dem **Host-Decides** Prinzip:
- Nur der Host sieht Choice-Menüs und trifft Entscheidungen
- DialogEffect-Commands werden als GameCommand an alle Clients gebroadcastet
- Clients sehen den Dialog-Text, aber nicht die Choice-UI
- Alternative (konfigurierbar): **Vote-Mode** — alle Spieler stimmen ab, Mehrheit gewinnt

### Host Migration

Host Migration ist **nicht unterstützt** in der ersten Version. Bei Host-Disconnect endet die Session. Geplant für spätere Iteration:
1. Alle Clients speichern GameState-Snapshots alle N Ticks
2. Bei Host-Disconnect wählt der Client mit niedrigster Latenz als neuer Host
3. Neuer Host sendet seinen Snapshot, alle resynchronisieren
