use serde::{Deserialize, Serialize};

use crate::ecs::EntityId;

/// Simple integer vector for tile positions on the game grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IVec2 {
    pub x: i32,
    pub y: i32,
}

impl IVec2 {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub const ZERO: Self = Self { x: 0, y: 0 };
}

/// Identifier for a tower type in the game's tower registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TowerTypeId(pub u32);

/// Which upgrade path to follow when upgrading a tower.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UpgradePath {
    PathA,
    PathB,
}

/// A serializable command representing a player action.
///
/// Commands are the sole interface through which gameplay state is mutated,
/// making the simulation deterministic and replay-friendly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameCommand {
    PlaceTower {
        pos: IVec2,
        tower_type: TowerTypeId,
    },
    SellTower {
        tower_id: EntityId,
    },
    UpgradeTower {
        tower_id: EntityId,
        path: UpgradePath,
    },
    StartWave,
    Pause,
    Unpause,
    SetSpeed {
        multiplier: u32,
    },
    SelectTower {
        tower_id: Option<EntityId>,
    },
}

/// A per-tick queue that collects commands and drains them for processing.
#[derive(Debug, Default)]
pub struct CommandQueue {
    commands: Vec<GameCommand>,
}

impl CommandQueue {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Enqueue a command to be processed on the next drain.
    pub fn push(&mut self, cmd: GameCommand) {
        self.commands.push(cmd);
    }

    /// Drain all queued commands, returning them in insertion order.
    pub fn drain(&mut self) -> Vec<GameCommand> {
        std::mem::take(&mut self.commands)
    }

    /// Returns `true` if no commands are queued.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns the number of queued commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }
}

/// A log of all commands paired with the tick they were issued on,
/// enabling deterministic replay of a game session.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommandLog {
    entries: Vec<(u64, GameCommand)>,
}

impl CommandLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record a command that was executed at the given tick.
    pub fn record(&mut self, tick: u64, cmd: GameCommand) {
        self.entries.push((tick, cmd));
    }

    /// Iterate over all recorded (tick, command) pairs in order.
    pub fn iter(&self) -> impl Iterator<Item = &(u64, GameCommand)> {
        self.entries.iter()
    }

    /// Returns the total number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_queue_push_and_drain() {
        let mut queue = CommandQueue::new();
        assert!(queue.is_empty());

        queue.push(GameCommand::StartWave);
        queue.push(GameCommand::Pause);
        assert_eq!(queue.len(), 2);

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn command_log_record_and_iter() {
        let mut log = CommandLog::new();
        log.record(0, GameCommand::StartWave);
        log.record(5, GameCommand::SetSpeed { multiplier: 2 });

        let entries: Vec<_> = log.iter().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, 0);
        assert_eq!(entries[1].0, 5);
    }

    #[test]
    fn game_command_serde_roundtrip() {
        let cmd = GameCommand::PlaceTower {
            pos: IVec2::new(3, 7),
            tower_type: TowerTypeId(1),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: GameCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }
}
