use crate::ecs::EntityId;
use crate::math::{RenderVec2, SimVec2};

// ---------------------------------------------------------------------------
// Generic Finite State Machine
// ---------------------------------------------------------------------------

/// A state identifier. Games define their own states as u32 constants or enums.
pub type StateId = u32;

/// Transition condition evaluated each tick.
pub struct Transition {
    pub from: StateId,
    pub to: StateId,
    pub condition: Box<dyn Fn(&AiContext) -> bool + Send>,
}

/// Context passed to AI conditions and actions.
pub struct AiContext {
    pub entity: EntityId,
    pub position: SimVec2,
    pub target_pos: Option<SimVec2>,
    pub target_entity: Option<EntityId>,
    pub health_fraction: f32,
    pub distance_to_target: f32,
    pub time_in_state: f32,
    pub custom: [f32; 4],
}

impl AiContext {
    pub fn new(entity: EntityId) -> Self {
        Self {
            entity,
            position: SimVec2::ZERO,
            target_pos: None,
            target_entity: None,
            health_fraction: 1.0,
            distance_to_target: f32::MAX,
            time_in_state: 0.0,
            custom: [0.0; 4],
        }
    }
}

/// A finite state machine for entity AI.
pub struct StateMachine {
    pub current_state: StateId,
    pub previous_state: StateId,
    transitions: Vec<Transition>,
    time_in_state: f32,
    state_just_entered: bool,
}

impl StateMachine {
    pub fn new(initial_state: StateId) -> Self {
        Self {
            current_state: initial_state,
            previous_state: initial_state,
            transitions: Vec::new(),
            time_in_state: 0.0,
            state_just_entered: true,
        }
    }

    /// Add a transition rule.
    pub fn add_transition(
        &mut self,
        from: StateId,
        to: StateId,
        condition: impl Fn(&AiContext) -> bool + Send + 'static,
    ) {
        self.transitions.push(Transition {
            from,
            to,
            condition: Box::new(condition),
        });
    }

    /// Evaluate transitions and advance state. Returns the active state.
    pub fn update(&mut self, ctx: &AiContext, dt: f32) -> StateId {
        self.time_in_state += dt;
        self.state_just_entered = false;

        // Check transitions from current state
        for t in &self.transitions {
            if t.from == self.current_state && (t.condition)(ctx) {
                self.previous_state = self.current_state;
                self.current_state = t.to;
                self.time_in_state = 0.0;
                self.state_just_entered = true;
                break;
            }
        }

        self.current_state
    }

    /// Force a state change (bypasses transitions).
    pub fn force_state(&mut self, state: StateId) {
        self.previous_state = self.current_state;
        self.current_state = state;
        self.time_in_state = 0.0;
        self.state_just_entered = true;
    }

    pub fn time_in_state(&self) -> f32 {
        self.time_in_state
    }

    pub fn just_entered(&self) -> bool {
        self.state_just_entered
    }
}

// ---------------------------------------------------------------------------
// Common AI state constants
// ---------------------------------------------------------------------------

/// Standard AI states (games can use their own u32 values).
pub mod states {
    use super::StateId;
    pub const IDLE: StateId = 0;
    pub const PATROL: StateId = 1;
    pub const CHASE: StateId = 2;
    pub const ATTACK: StateId = 3;
    pub const FLEE: StateId = 4;
    pub const DEAD: StateId = 5;
    pub const STUNNED: StateId = 6;
    pub const RETURN_HOME: StateId = 7;
}

// ---------------------------------------------------------------------------
// Common AI transition helpers
// ---------------------------------------------------------------------------

/// Create a standard monster AI with common transitions.
pub fn monster_ai(
    aggro_range: f32,
    attack_range: f32,
    flee_health: f32,
    leash_range: f32,
) -> StateMachine {
    let mut sm = StateMachine::new(states::IDLE);

    // IDLE → CHASE: target in aggro range
    sm.add_transition(states::IDLE, states::CHASE, move |ctx| {
        ctx.distance_to_target < aggro_range && ctx.target_entity.is_some()
    });

    // PATROL → CHASE: target in aggro range
    sm.add_transition(states::PATROL, states::CHASE, move |ctx| {
        ctx.distance_to_target < aggro_range && ctx.target_entity.is_some()
    });

    // CHASE → ATTACK: target in attack range
    sm.add_transition(states::CHASE, states::ATTACK, move |ctx| {
        ctx.distance_to_target < attack_range
    });

    // ATTACK → CHASE: target left attack range
    sm.add_transition(states::ATTACK, states::CHASE, move |ctx| {
        ctx.distance_to_target >= attack_range
    });

    // CHASE → RETURN_HOME: target too far (leash)
    sm.add_transition(states::CHASE, states::RETURN_HOME, move |ctx| {
        ctx.distance_to_target > leash_range
    });

    // RETURN_HOME → IDLE: arrived
    sm.add_transition(states::RETURN_HOME, states::IDLE, |ctx| {
        ctx.time_in_state > 3.0
    });

    // Any combat state → FLEE: low health
    sm.add_transition(states::CHASE, states::FLEE, move |ctx| {
        ctx.health_fraction < flee_health
    });
    sm.add_transition(states::ATTACK, states::FLEE, move |ctx| {
        ctx.health_fraction < flee_health
    });

    // Any state → DEAD: health depleted
    sm.add_transition(states::IDLE, states::DEAD, |ctx| ctx.health_fraction <= 0.0);
    sm.add_transition(states::PATROL, states::DEAD, |ctx| {
        ctx.health_fraction <= 0.0
    });
    sm.add_transition(states::CHASE, states::DEAD, |ctx| {
        ctx.health_fraction <= 0.0
    });
    sm.add_transition(states::ATTACK, states::DEAD, |ctx| {
        ctx.health_fraction <= 0.0
    });
    sm.add_transition(states::FLEE, states::DEAD, |ctx| ctx.health_fraction <= 0.0);

    sm
}

/// Create a simple patrol AI that walks waypoints.
pub fn patrol_ai(aggro_range: f32, attack_range: f32) -> StateMachine {
    let mut sm = StateMachine::new(states::PATROL);

    sm.add_transition(states::PATROL, states::CHASE, move |ctx| {
        ctx.distance_to_target < aggro_range && ctx.target_entity.is_some()
    });

    sm.add_transition(states::CHASE, states::ATTACK, move |ctx| {
        ctx.distance_to_target < attack_range
    });

    sm.add_transition(states::ATTACK, states::CHASE, move |ctx| {
        ctx.distance_to_target >= attack_range
    });

    sm.add_transition(states::CHASE, states::PATROL, move |ctx| {
        ctx.distance_to_target > aggro_range * 1.5 || ctx.target_entity.is_none()
    });

    // Dead
    sm.add_transition(states::PATROL, states::DEAD, |ctx| {
        ctx.health_fraction <= 0.0
    });
    sm.add_transition(states::CHASE, states::DEAD, |ctx| {
        ctx.health_fraction <= 0.0
    });
    sm.add_transition(states::ATTACK, states::DEAD, |ctx| {
        ctx.health_fraction <= 0.0
    });

    sm
}

// ---------------------------------------------------------------------------
// Steering behaviors (for smooth movement)
// ---------------------------------------------------------------------------

/// Simple steering behaviors for AI movement.
pub struct Steering;

impl Steering {
    /// Seek: move directly toward target.
    pub fn seek(pos: RenderVec2, target: RenderVec2, max_speed: f32) -> RenderVec2 {
        let dx = target.x - pos.x;
        let dy = target.y - pos.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 0.01 {
            return RenderVec2::ZERO;
        }
        RenderVec2::new(dx / dist * max_speed, dy / dist * max_speed)
    }

    /// Flee: move directly away from target.
    pub fn flee(pos: RenderVec2, threat: RenderVec2, max_speed: f32) -> RenderVec2 {
        let dx = pos.x - threat.x;
        let dy = pos.y - threat.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 0.01 {
            return RenderVec2::new(max_speed, 0.0);
        }
        RenderVec2::new(dx / dist * max_speed, dy / dist * max_speed)
    }

    /// Arrive: seek with deceleration near target.
    pub fn arrive(
        pos: RenderVec2,
        target: RenderVec2,
        max_speed: f32,
        slow_radius: f32,
    ) -> RenderVec2 {
        let dx = target.x - pos.x;
        let dy = target.y - pos.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 0.01 {
            return RenderVec2::ZERO;
        }
        let speed = if dist < slow_radius {
            max_speed * (dist / slow_radius)
        } else {
            max_speed
        };
        RenderVec2::new(dx / dist * speed, dy / dist * speed)
    }

    /// Separation: steer away from nearby entities.
    pub fn separation(
        pos: RenderVec2,
        neighbors: &[RenderVec2],
        desired_distance: f32,
    ) -> RenderVec2 {
        let mut steer = RenderVec2::ZERO;
        let mut count = 0;
        for n in neighbors {
            let dx = pos.x - n.x;
            let dy = pos.y - n.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > 0.01 && dist < desired_distance {
                steer.x += dx / dist;
                steer.y += dy / dist;
                count += 1;
            }
        }
        if count > 0 {
            steer.x /= count as f32;
            steer.y /= count as f32;
        }
        steer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_machine_transitions() {
        let mut sm = monster_ai(100.0, 20.0, 0.2, 300.0);
        let mut ctx = AiContext::new(EntityId::from_raw(0, 0));

        // Initially idle
        assert_eq!(sm.current_state, states::IDLE);

        // Target in range → chase
        ctx.distance_to_target = 80.0;
        ctx.target_entity = Some(EntityId::from_raw(1, 0));
        sm.update(&ctx, 0.016);
        assert_eq!(sm.current_state, states::CHASE);

        // Close enough → attack
        ctx.distance_to_target = 15.0;
        sm.update(&ctx, 0.016);
        assert_eq!(sm.current_state, states::ATTACK);

        // Low health → flee
        ctx.health_fraction = 0.1;
        sm.update(&ctx, 0.016);
        assert_eq!(sm.current_state, states::FLEE);

        // Dead
        ctx.health_fraction = 0.0;
        sm.update(&ctx, 0.016);
        assert_eq!(sm.current_state, states::DEAD);
    }

    #[test]
    fn steering_seek() {
        let vel = Steering::seek(RenderVec2::new(0.0, 0.0), RenderVec2::new(10.0, 0.0), 5.0);
        assert!((vel.x - 5.0).abs() < 0.01);
        assert!(vel.y.abs() < 0.01);
    }

    #[test]
    fn steering_arrive_slows_down() {
        let vel_far = Steering::arrive(RenderVec2::ZERO, RenderVec2::new(100.0, 0.0), 10.0, 50.0);
        let vel_near = Steering::arrive(RenderVec2::ZERO, RenderVec2::new(25.0, 0.0), 10.0, 50.0);
        assert!(vel_far.x > vel_near.x);
    }
}
