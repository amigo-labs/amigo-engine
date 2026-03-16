# Command System & Networking

> Status: draft
> Crate: amigo_net
> Depends on: [engine/core](../engine/core.md)
> Last updated: 2026-03-16

## Zweck

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

## Verhalten

### Multiplayer (Phase 2+)

- **Co-op (2-4 players):** Shared map, lockstep protocol
- **Competitive:** Own maps, send waves to opponent
- **Spectator:** Receive-only

### Replay System

Commands logged with tick numbers. Replay = feed commands into fresh GameState.
