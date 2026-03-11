use crate::ecs::EntityId;
use crate::collision::ContactInfo;
use rustc_hash::FxHashSet;

/// The phase of a collision between two entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionPhase {
    /// First frame the two entities overlap.
    Enter,
    /// Entities are still overlapping (after the first frame).
    Stay,
    /// First frame the two entities stopped overlapping.
    Exit,
}

/// A collision event produced by the contact tracker.
#[derive(Clone, Debug)]
pub struct CollisionEvent {
    pub entity_a: EntityId,
    pub entity_b: EntityId,
    pub phase: CollisionPhase,
    pub contact: Option<ContactInfo>,
}

/// Canonical pair key (always ordered smaller-first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PairKey(EntityId, EntityId);

impl PairKey {
    fn new(a: EntityId, b: EntityId) -> Self {
        if a < b { Self(a, b) } else { Self(b, a) }
    }
}

/// Tracks entity-entity collisions across frames and produces
/// Enter / Stay / Exit events.
pub struct ContactTracker {
    /// Pairs that were colliding last frame.
    previous: FxHashSet<PairKey>,
    /// Pairs that are colliding this frame (built during `begin_frame` / `report`).
    current: FxHashSet<PairKey>,
    /// Events produced this frame.
    events: Vec<CollisionEvent>,
}

impl ContactTracker {
    pub fn new() -> Self {
        Self {
            previous: FxHashSet::default(),
            current: FxHashSet::default(),
            events: Vec::new(),
        }
    }

    /// Call at the start of each physics tick, before reporting collisions.
    pub fn begin_frame(&mut self) {
        self.current.clear();
        self.events.clear();
    }

    /// Report a collision between two entities this frame.
    pub fn report(&mut self, a: EntityId, b: EntityId, contact: ContactInfo) {
        let key = PairKey::new(a, b);
        self.current.insert(key);

        let phase = if self.previous.contains(&key) {
            CollisionPhase::Stay
        } else {
            CollisionPhase::Enter
        };

        self.events.push(CollisionEvent {
            entity_a: key.0,
            entity_b: key.1,
            phase,
            contact: Some(contact),
        });
    }

    /// Call at the end of each physics tick. Generates Exit events for pairs
    /// that were colliding last frame but not this frame.
    pub fn end_frame(&mut self) {
        for &key in &self.previous {
            if !self.current.contains(&key) {
                self.events.push(CollisionEvent {
                    entity_a: key.0,
                    entity_b: key.1,
                    phase: CollisionPhase::Exit,
                    contact: None,
                });
            }
        }
        std::mem::swap(&mut self.previous, &mut self.current);
    }

    /// Get all collision events for this frame (available after `end_frame`).
    pub fn events(&self) -> &[CollisionEvent] {
        &self.events
    }

    /// Remove an entity from tracking (e.g. when despawned).
    pub fn remove_entity(&mut self, entity: EntityId) {
        self.previous.retain(|pair| pair.0 != entity && pair.1 != entity);
        self.current.retain(|pair| pair.0 != entity && pair.1 != entity);
    }

    pub fn clear(&mut self) {
        self.previous.clear();
        self.current.clear();
        self.events.clear();
    }
}

impl Default for ContactTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::RenderVec2;

    fn dummy_contact() -> ContactInfo {
        ContactInfo {
            penetration: 1.0,
            normal: RenderVec2::new(1.0, 0.0),
        }
    }

    fn eid(index: u32) -> EntityId {
        EntityId::from_raw(index, 0)
    }

    #[test]
    fn enter_stay_exit_lifecycle() {
        let mut tracker = ContactTracker::new();
        let a = eid(1);
        let b = eid(2);

        // Frame 1: collision starts
        tracker.begin_frame();
        tracker.report(a, b, dummy_contact());
        tracker.end_frame();
        assert_eq!(tracker.events().len(), 1);
        assert_eq!(tracker.events()[0].phase, CollisionPhase::Enter);

        // Frame 2: collision continues
        tracker.begin_frame();
        tracker.report(a, b, dummy_contact());
        tracker.end_frame();
        assert_eq!(tracker.events().len(), 1);
        assert_eq!(tracker.events()[0].phase, CollisionPhase::Stay);

        // Frame 3: collision ends
        tracker.begin_frame();
        // don't report
        tracker.end_frame();
        assert_eq!(tracker.events().len(), 1);
        assert_eq!(tracker.events()[0].phase, CollisionPhase::Exit);

        // Frame 4: nothing
        tracker.begin_frame();
        tracker.end_frame();
        assert!(tracker.events().is_empty());
    }

    #[test]
    fn remove_entity_clears_pairs() {
        let mut tracker = ContactTracker::new();
        let a = eid(1);
        let b = eid(2);

        tracker.begin_frame();
        tracker.report(a, b, dummy_contact());
        tracker.end_frame();

        tracker.remove_entity(a);

        // No exit event since we explicitly removed
        tracker.begin_frame();
        tracker.end_frame();
        assert!(tracker.events().is_empty());
    }
}
