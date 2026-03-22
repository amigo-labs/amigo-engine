use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Position snapshots
// ---------------------------------------------------------------------------

/// A snapshot of one entity's position for network sync.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EntityPositionSnapshot {
    pub entity_index: u32,
    pub x: i32,
    pub y: i32,
    /// Direction the entity is facing (0-7, 8-directional).
    pub facing: u8,
    /// Whether the entity is performing an action (walking, working, etc.).
    pub action_flag: u8,
}

/// Door state update for network sync.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DoorUpdate {
    pub door_id: u32,
    /// 0 = Open, 1 = Closed, 2 = Locked.
    pub state: u8,
    pub lock_timer: f32,
}

/// Summary of task progress (no spoilers about which tasks).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TaskProgressSummary {
    pub completed: u16,
    pub total: u16,
}

// ---------------------------------------------------------------------------
// Filtered snapshot (per-player)
// ---------------------------------------------------------------------------

/// A per-player filtered state snapshot for one tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilteredSnapshot {
    pub tick: u64,
    /// Entity positions this player is allowed to see.
    pub visible_entities: Vec<EntityPositionSnapshot>,
    /// Door states that changed this tick.
    pub door_updates: Vec<DoorUpdate>,
    /// Task completion count.
    pub task_progress: TaskProgressSummary,
}

// ---------------------------------------------------------------------------
// Delta encoding
// ---------------------------------------------------------------------------

/// A position delta (or full snapshot for new entities).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum EntityPositionDelta {
    /// Full position (first time this entity appears).
    Full(EntityPositionSnapshot),
    /// Delta from previous position.
    Delta {
        entity_index: u32,
        dx: i16,
        dy: i16,
        facing: u8,
        action_flag: u8,
    },
    /// Entity is no longer visible.
    Removed { entity_index: u32 },
}

/// Delta-encodes position snapshots between ticks.
#[derive(Clone, Debug, Default)]
pub struct PositionDeltaEncoder {
    previous: Vec<EntityPositionSnapshot>,
}

impl PositionDeltaEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Encode current positions as deltas from the previous tick.
    pub fn encode(&mut self, current: &[EntityPositionSnapshot]) -> Vec<EntityPositionDelta> {
        let mut deltas = Vec::new();

        // Emit deltas or full snapshots for current entities.
        for snap in current {
            if let Some(prev) = self
                .previous
                .iter()
                .find(|p| p.entity_index == snap.entity_index)
            {
                let dx = snap.x - prev.x;
                let dy = snap.y - prev.y;
                // Only emit delta if something changed.
                if dx != 0
                    || dy != 0
                    || snap.facing != prev.facing
                    || snap.action_flag != prev.action_flag
                {
                    deltas.push(EntityPositionDelta::Delta {
                        entity_index: snap.entity_index,
                        dx: dx as i16,
                        dy: dy as i16,
                        facing: snap.facing,
                        action_flag: snap.action_flag,
                    });
                }
            } else {
                deltas.push(EntityPositionDelta::Full(*snap));
            }
        }

        // Emit Removed for entities that were in previous but not in current.
        for prev in &self.previous {
            if !current.iter().any(|c| c.entity_index == prev.entity_index) {
                deltas.push(EntityPositionDelta::Removed {
                    entity_index: prev.entity_index,
                });
            }
        }

        self.previous = current.to_vec();
        deltas
    }

    /// Decode deltas back to full positions using the previous snapshot.
    pub fn decode(&mut self, deltas: &[EntityPositionDelta]) -> Vec<EntityPositionSnapshot> {
        for delta in deltas {
            match delta {
                EntityPositionDelta::Full(snap) => {
                    if let Some(existing) = self
                        .previous
                        .iter_mut()
                        .find(|p| p.entity_index == snap.entity_index)
                    {
                        *existing = *snap;
                    } else {
                        self.previous.push(*snap);
                    }
                }
                EntityPositionDelta::Delta {
                    entity_index,
                    dx,
                    dy,
                    facing,
                    action_flag,
                } => {
                    if let Some(existing) = self
                        .previous
                        .iter_mut()
                        .find(|p| p.entity_index == *entity_index)
                    {
                        existing.x += *dx as i32;
                        existing.y += *dy as i32;
                        existing.facing = *facing;
                        existing.action_flag = *action_flag;
                    }
                }
                EntityPositionDelta::Removed { entity_index } => {
                    self.previous.retain(|p| p.entity_index != *entity_index);
                }
            }
        }

        self.previous.clone()
    }

    /// Reset encoder state (e.g., on reconnect).
    pub fn reset(&mut self) {
        self.previous.clear();
    }
}

// ---------------------------------------------------------------------------
// Snapshot builder
// ---------------------------------------------------------------------------

/// Builds per-player filtered snapshots from the full game state.
pub struct SnapshotBuilder;

impl SnapshotBuilder {
    /// Build a filtered snapshot for one player.
    pub fn build(
        tick: u64,
        visible_entity_indices: &[u32],
        all_positions: &[EntityPositionSnapshot],
        door_updates: &[DoorUpdate],
        task_summary: TaskProgressSummary,
    ) -> FilteredSnapshot {
        let visible_entities: Vec<EntityPositionSnapshot> = all_positions
            .iter()
            .filter(|p| visible_entity_indices.contains(&p.entity_index))
            .copied()
            .collect();

        FilteredSnapshot {
            tick,
            visible_entities,
            door_updates: door_updates.to_vec(),
            task_progress: task_summary,
        }
    }

    /// Serialize a FilteredSnapshot to bytes for transmission.
    pub fn encode_snapshot(snapshot: &FilteredSnapshot) -> Option<Vec<u8>> {
        serde_json::to_vec(snapshot).ok()
    }

    /// Deserialize a FilteredSnapshot from bytes.
    pub fn decode_snapshot(data: &[u8]) -> Option<FilteredSnapshot> {
        serde_json::from_slice(data).ok()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(idx: u32, x: i32, y: i32) -> EntityPositionSnapshot {
        EntityPositionSnapshot {
            entity_index: idx,
            x,
            y,
            facing: 0,
            action_flag: 0,
        }
    }

    #[test]
    fn delta_encode_full_then_delta() {
        let mut encoder = PositionDeltaEncoder::new();

        // First tick: all full snapshots.
        let tick1 = vec![snap(0, 100, 200), snap(1, 300, 400)];
        let deltas1 = encoder.encode(&tick1);
        assert_eq!(deltas1.len(), 2);
        assert!(matches!(deltas1[0], EntityPositionDelta::Full(_)));

        // Second tick: entity 0 moved.
        let tick2 = vec![snap(0, 105, 200), snap(1, 300, 400)];
        let deltas2 = encoder.encode(&tick2);
        // Only entity 0 changed.
        assert_eq!(deltas2.len(), 1);
        assert!(matches!(
            deltas2[0],
            EntityPositionDelta::Delta {
                entity_index: 0,
                dx: 5,
                dy: 0,
                ..
            }
        ));
    }

    #[test]
    fn delta_encode_entity_removed() {
        let mut encoder = PositionDeltaEncoder::new();

        let tick1 = vec![snap(0, 100, 200), snap(1, 300, 400)];
        encoder.encode(&tick1);

        // Entity 1 disappears.
        let tick2 = vec![snap(0, 100, 200)];
        let deltas = encoder.encode(&tick2);
        assert!(deltas
            .iter()
            .any(|d| matches!(d, EntityPositionDelta::Removed { entity_index: 1 })));
    }

    #[test]
    fn delta_decode_roundtrip() {
        let mut encoder = PositionDeltaEncoder::new();
        let mut decoder = PositionDeltaEncoder::new();

        let tick1 = vec![snap(0, 100, 200), snap(1, 300, 400)];
        let deltas1 = encoder.encode(&tick1);
        let decoded1 = decoder.decode(&deltas1);
        assert_eq!(decoded1.len(), 2);
        assert_eq!(decoded1[0].x, 100);

        let tick2 = vec![snap(0, 110, 210), snap(1, 300, 400)];
        let deltas2 = encoder.encode(&tick2);
        let decoded2 = decoder.decode(&deltas2);
        assert_eq!(decoded2[0].x, 110);
        assert_eq!(decoded2[0].y, 210);
    }

    #[test]
    fn filtered_snapshot_build() {
        let all = vec![snap(0, 10, 20), snap(1, 30, 40), snap(2, 50, 60)];
        let visible = vec![0, 2];

        let snapshot =
            SnapshotBuilder::build(42, &visible, &all, &[], TaskProgressSummary::default());
        assert_eq!(snapshot.tick, 42);
        assert_eq!(snapshot.visible_entities.len(), 2);
        assert_eq!(snapshot.visible_entities[0].entity_index, 0);
        assert_eq!(snapshot.visible_entities[1].entity_index, 2);
    }

    #[test]
    fn snapshot_serialize_roundtrip() {
        let snapshot = FilteredSnapshot {
            tick: 100,
            visible_entities: vec![snap(0, 10, 20)],
            door_updates: vec![DoorUpdate {
                door_id: 1,
                state: 2,
                lock_timer: 3.5,
            }],
            task_progress: TaskProgressSummary {
                completed: 5,
                total: 10,
            },
        };

        let bytes = SnapshotBuilder::encode_snapshot(&snapshot).unwrap();
        let decoded = SnapshotBuilder::decode_snapshot(&bytes).unwrap();
        assert_eq!(decoded.tick, 100);
        assert_eq!(decoded.visible_entities.len(), 1);
        assert_eq!(decoded.task_progress.completed, 5);
    }
}
