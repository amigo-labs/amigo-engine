use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers and states
// ---------------------------------------------------------------------------

/// Unique door identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DoorId(pub u32);

/// Current state of a door.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorState {
    Open,
    Closed,
    Locked,
}

/// Who can interact with a door.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorAccess {
    /// Anyone can open/close.
    Public,
    /// Only members of a specific team.
    TeamOnly(u8),
    /// Nobody can open (system only).
    SystemOnly,
}

// ---------------------------------------------------------------------------
// Definition
// ---------------------------------------------------------------------------

/// Definition for a door or vent in the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoorDef {
    pub id: DoorId,
    /// Corresponds to a TriggerZone id in the collision layer.
    pub zone_id: u32,
    /// Initial state when the round begins.
    pub initial_state: DoorState,
    /// Access control.
    pub access: DoorAccess,
    /// If true, this is a vent (affects movement speed, restricted access).
    pub is_vent: bool,
    /// If Some, the door auto-locks for this many seconds after closing.
    pub auto_lock_duration: Option<f32>,
}

// ---------------------------------------------------------------------------
// Runtime instance
// ---------------------------------------------------------------------------

/// Runtime state for a single door instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoorInstance {
    pub def_id: DoorId,
    pub zone_id: u32,
    pub state: DoorState,
    pub is_vent: bool,
    /// Remaining lock timer (only when state == Locked).
    pub lock_timer: f32,
    pub access: DoorAccess,
    pub auto_lock_duration: Option<f32>,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events produced by the door system.
#[derive(Clone, Debug)]
pub enum DoorEvent {
    Opened {
        door_id: DoorId,
        zone_id: u32,
    },
    Closed {
        door_id: DoorId,
        zone_id: u32,
    },
    Locked {
        door_id: DoorId,
        zone_id: u32,
        duration: f32,
    },
    Unlocked {
        door_id: DoorId,
        zone_id: u32,
    },
}

// ---------------------------------------------------------------------------
// Door manager
// ---------------------------------------------------------------------------

/// Manages all doors in the current map.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DoorManager {
    doors: Vec<DoorInstance>,
}

impl DoorManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize doors from definitions.
    pub fn spawn_doors(&mut self, defs: &[DoorDef]) {
        self.doors.clear();
        for def in defs {
            self.doors.push(DoorInstance {
                def_id: def.id,
                zone_id: def.zone_id,
                state: def.initial_state,
                is_vent: def.is_vent,
                lock_timer: 0.0,
                access: def.access.clone(),
                auto_lock_duration: def.auto_lock_duration,
            });
        }
    }

    /// Try to open a door. Returns event if successful.
    pub fn open(&mut self, door_id: DoorId, team: u8) -> Option<DoorEvent> {
        let door = self.doors.iter_mut().find(|d| d.def_id == door_id)?;

        if door.state == DoorState::Open || door.state == DoorState::Locked {
            return None;
        }

        if !check_access(&door.access, team) {
            return None;
        }

        door.state = DoorState::Open;
        Some(DoorEvent::Opened {
            door_id,
            zone_id: door.zone_id,
        })
    }

    /// Try to close a door. If auto_lock_duration is set, transitions to Locked.
    pub fn close(&mut self, door_id: DoorId, team: u8) -> Option<DoorEvent> {
        let door = self.doors.iter_mut().find(|d| d.def_id == door_id)?;

        if door.state != DoorState::Open {
            return None;
        }

        if !check_access(&door.access, team) {
            return None;
        }

        if let Some(lock_dur) = door.auto_lock_duration {
            door.state = DoorState::Locked;
            door.lock_timer = lock_dur;
            Some(DoorEvent::Locked {
                door_id,
                zone_id: door.zone_id,
                duration: lock_dur,
            })
        } else {
            door.state = DoorState::Closed;
            Some(DoorEvent::Closed {
                door_id,
                zone_id: door.zone_id,
            })
        }
    }

    /// System-level lock (bypasses access control).
    pub fn lock(&mut self, door_id: DoorId, duration: f32) -> Option<DoorEvent> {
        let door = self.doors.iter_mut().find(|d| d.def_id == door_id)?;
        door.state = DoorState::Locked;
        door.lock_timer = duration;
        Some(DoorEvent::Locked {
            door_id,
            zone_id: door.zone_id,
            duration,
        })
    }

    /// System-level unlock (bypasses access control).
    pub fn unlock(&mut self, door_id: DoorId) -> Option<DoorEvent> {
        let door = self.doors.iter_mut().find(|d| d.def_id == door_id)?;
        if door.state != DoorState::Locked {
            return None;
        }
        door.state = DoorState::Closed;
        door.lock_timer = 0.0;
        Some(DoorEvent::Unlocked {
            door_id,
            zone_id: door.zone_id,
        })
    }

    /// Tick lock timers. Returns unlock events for doors whose lock expired.
    pub fn update(&mut self, dt: f32) -> Vec<DoorEvent> {
        let mut events = Vec::new();

        for door in &mut self.doors {
            if door.state == DoorState::Locked && door.lock_timer > 0.0 {
                door.lock_timer -= dt;
                if door.lock_timer <= 0.0 {
                    door.lock_timer = 0.0;
                    door.state = DoorState::Closed;
                    events.push(DoorEvent::Unlocked {
                        door_id: door.def_id,
                        zone_id: door.zone_id,
                    });
                }
            }
        }

        events
    }

    /// Get the current state of a door.
    pub fn state(&self, door_id: DoorId) -> Option<DoorState> {
        self.doors
            .iter()
            .find(|d| d.def_id == door_id)
            .map(|d| d.state)
    }

    /// Get all doors as a slice (for rendering/collision sync).
    pub fn doors(&self) -> &[DoorInstance] {
        &self.doors
    }

    /// Query: is the door with this zone_id currently passable?
    pub fn is_passable(&self, zone_id: u32) -> bool {
        !self
            .doors
            .iter()
            .any(|d| d.zone_id == zone_id && d.state != DoorState::Open)
    }

    /// Reset all doors to initial state.
    pub fn reset(&mut self, defs: &[DoorDef]) {
        self.spawn_doors(defs);
    }
}

fn check_access(access: &DoorAccess, team: u8) -> bool {
    match access {
        DoorAccess::Public => true,
        DoorAccess::TeamOnly(required) => *required == team,
        DoorAccess::SystemOnly => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_defs() -> Vec<DoorDef> {
        vec![
            DoorDef {
                id: DoorId(1),
                zone_id: 100,
                initial_state: DoorState::Closed,
                access: DoorAccess::Public,
                is_vent: false,
                auto_lock_duration: None,
            },
            DoorDef {
                id: DoorId(2),
                zone_id: 200,
                initial_state: DoorState::Closed,
                access: DoorAccess::TeamOnly(1),
                is_vent: true,
                auto_lock_duration: None,
            },
            DoorDef {
                id: DoorId(3),
                zone_id: 300,
                initial_state: DoorState::Closed,
                access: DoorAccess::Public,
                is_vent: false,
                auto_lock_duration: Some(5.0),
            },
        ]
    }

    #[test]
    fn open_and_close() {
        let defs = test_defs();
        let mut mgr = DoorManager::new();
        mgr.spawn_doors(&defs);

        assert!(!mgr.is_passable(100));
        assert!(mgr.open(DoorId(1), 0).is_some());
        assert!(mgr.is_passable(100));

        assert!(mgr.close(DoorId(1), 0).is_some());
        assert!(!mgr.is_passable(100));
    }

    #[test]
    fn team_access_denied() {
        let defs = test_defs();
        let mut mgr = DoorManager::new();
        mgr.spawn_doors(&defs);

        // Door 2 requires team 1.
        assert!(mgr.open(DoorId(2), 0).is_none());
        assert!(mgr.open(DoorId(2), 1).is_some());
    }

    #[test]
    fn auto_lock_on_close() {
        let defs = test_defs();
        let mut mgr = DoorManager::new();
        mgr.spawn_doors(&defs);

        mgr.open(DoorId(3), 0);
        let event = mgr.close(DoorId(3), 0);
        assert!(matches!(event, Some(DoorEvent::Locked { .. })));
        assert_eq!(mgr.state(DoorId(3)), Some(DoorState::Locked));

        // Can't open while locked.
        assert!(mgr.open(DoorId(3), 0).is_none());

        // Timer expires.
        let events = mgr.update(6.0);
        assert!(events
            .iter()
            .any(|e| matches!(e, DoorEvent::Unlocked { .. })));
        assert_eq!(mgr.state(DoorId(3)), Some(DoorState::Closed));
    }

    #[test]
    fn system_lock_bypasses_access() {
        let defs = vec![DoorDef {
            id: DoorId(10),
            zone_id: 10,
            initial_state: DoorState::Open,
            access: DoorAccess::SystemOnly,
            is_vent: false,
            auto_lock_duration: None,
        }];
        let mut mgr = DoorManager::new();
        mgr.spawn_doors(&defs);

        // Player can't close a SystemOnly door.
        assert!(mgr.close(DoorId(10), 0).is_none());
        // But system lock works.
        assert!(mgr.lock(DoorId(10), 3.0).is_some());
        assert_eq!(mgr.state(DoorId(10)), Some(DoorState::Locked));

        // System unlock works.
        assert!(mgr.unlock(DoorId(10)).is_some());
        assert_eq!(mgr.state(DoorId(10)), Some(DoorState::Closed));
    }
}
