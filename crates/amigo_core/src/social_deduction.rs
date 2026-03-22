use crate::combat::Cooldown;
use crate::door::{DoorId, DoorManager};
use crate::ecs::EntityId;
use crate::math::IVec2;
use crate::raycast::TileQuery;
use crate::task_system::{TaskId, TaskState};
use crate::vision_ray;
use crate::voting::{VoteConfig, VoteOutcome, VotingSession};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Roles
// ---------------------------------------------------------------------------

/// The role assigned to a player for this round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Crewmate,
    Impostor,
}

// ---------------------------------------------------------------------------
// Player state
// ---------------------------------------------------------------------------

/// Per-player state within a social deduction game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub entity: EntityId,
    pub role: Role,
    pub alive: bool,
    pub ejected: bool,
    pub kill_cooldown: Cooldown,
    pub sabotage_cooldown: Cooldown,
    pub can_use_vents: bool,
    pub vision_radius: u32,
}

// ---------------------------------------------------------------------------
// Sabotage
// ---------------------------------------------------------------------------

/// Types of sabotage an impostor can trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SabotageKind {
    /// Locks doors in a specified room for a duration.
    DoorLock { door_id: DoorId, duration: f32 },
    /// Disables a critical system; crew must repair within time limit.
    CriticalSystem { task_id: TaskId, time_limit: f32 },
    /// Reduces all crewmate vision radii.
    Lights { reduced_radius: u32, duration: f32 },
    /// Disables communications.
    Comms { duration: f32 },
}

/// A sabotage event in progress.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveSabotage {
    pub kind: SabotageKind,
    pub timer: f32,
    pub resolved: bool,
}

// ---------------------------------------------------------------------------
// Game phases
// ---------------------------------------------------------------------------

/// Phases of a social deduction round.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Phase {
    Setup,
    Playing,
    Discussion { remaining: f32 },
    Voting,
    Ejection,
    Results,
}

/// Reason the game ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WinCondition {
    TasksComplete,
    AllImpostorsEjected,
    ImpostorParity,
    SabotageUnresolved,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a social deduction game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SdConfig {
    pub impostor_count: u8,
    pub kill_cooldown: f32,
    pub sabotage_cooldown: f32,
    pub discussion_duration: f32,
    pub voting_duration: f32,
    pub crewmate_vision_radius: u32,
    pub impostor_vision_radius: u32,
    pub emergency_meetings_per_player: u8,
    pub kill_range: f32,
    pub seed: u64,
}

impl Default for SdConfig {
    fn default() -> Self {
        Self {
            impostor_count: 1,
            kill_cooldown: 25.0,
            sabotage_cooldown: 30.0,
            discussion_duration: 15.0,
            voting_duration: 30.0,
            crewmate_vision_radius: 5,
            impostor_vision_radius: 7,
            emergency_meetings_per_player: 1,
            kill_range: 2.0,
            seed: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events specific to the social deduction genre.
#[derive(Clone, Debug)]
pub enum SdEvent {
    RolesAssigned {
        impostor_count: u8,
    },
    PhaseChanged {
        from: Phase,
        to: Phase,
    },
    BodyReported {
        reporter: EntityId,
        body: EntityId,
    },
    EmergencyMeeting {
        caller: EntityId,
    },
    PlayerEjected {
        entity: EntityId,
        was_impostor: bool,
    },
    SabotageStarted {
        kind: SabotageKind,
    },
    SabotageResolved {
        kind: SabotageKind,
    },
    PlayerKilled {
        killer: EntityId,
        victim: EntityId,
    },
    GameOver {
        winner: Role,
        condition: WinCondition,
    },
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Top-level state for a social deduction game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SdState {
    pub config: SdConfig,
    pub phase: Phase,
    pub players: Vec<PlayerState>,
    pub tasks: TaskState,
    pub doors: DoorManager,
    pub vote_session: VotingSession,
    pub active_sabotage: Option<ActiveSabotage>,
    pub emergency_meetings_remaining: Vec<(EntityId, u8)>,
    pub round_number: u32,
    pub winner: Option<(Role, WinCondition)>,
}

impl SdState {
    pub fn new(config: SdConfig) -> Self {
        Self {
            phase: Phase::Setup,
            players: Vec::new(),
            tasks: TaskState::new(),
            doors: DoorManager::new(),
            vote_session: VotingSession::new(),
            active_sabotage: None,
            emergency_meetings_remaining: Vec::new(),
            round_number: 0,
            winner: None,
            config,
        }
    }
}

// ---------------------------------------------------------------------------
// System functions
// ---------------------------------------------------------------------------

/// Assign roles to players using the config seed. Transitions to Playing.
pub fn assign_roles(state: &mut SdState, entities: &[EntityId]) -> Vec<SdEvent> {
    let mut events = Vec::new();
    let count = entities.len();
    let impostor_count = (state.config.impostor_count as usize).min(count.saturating_sub(1));

    // Deterministic Fisher-Yates shuffle to pick impostors.
    let mut indices: Vec<usize> = (0..count).collect();
    let mut rng = state.config.seed;
    for i in (1..count).rev() {
        rng = xorshift64(rng);
        let j = (rng as usize) % (i + 1);
        indices.swap(i, j);
    }

    state.players.clear();
    for (idx, &entity) in entities.iter().enumerate() {
        let is_impostor = indices.iter().take(impostor_count).any(|&i| i == idx);
        let role = if is_impostor {
            Role::Impostor
        } else {
            Role::Crewmate
        };
        let vision_radius = if is_impostor {
            state.config.impostor_vision_radius
        } else {
            state.config.crewmate_vision_radius
        };

        state.players.push(PlayerState {
            entity,
            role,
            alive: true,
            ejected: false,
            kill_cooldown: Cooldown::new(state.config.kill_cooldown),
            sabotage_cooldown: Cooldown::new(state.config.sabotage_cooldown),
            can_use_vents: is_impostor,
            vision_radius,
        });

        state
            .emergency_meetings_remaining
            .push((entity, state.config.emergency_meetings_per_player));
    }

    let old_phase = state.phase;
    state.phase = Phase::Playing;
    events.push(SdEvent::RolesAssigned {
        impostor_count: impostor_count as u8,
    });
    events.push(SdEvent::PhaseChanged {
        from: old_phase,
        to: Phase::Playing,
    });
    events
}

/// Main tick function. Call every frame.
pub fn sd_tick(state: &mut SdState, dt: f32) -> Vec<SdEvent> {
    let mut events = Vec::new();

    match state.phase {
        Phase::Playing => {
            // Update cooldowns.
            for player in &mut state.players {
                player.kill_cooldown.update(dt);
                player.sabotage_cooldown.update(dt);
            }

            // Update doors.
            let _door_events = state.doors.update(dt);

            // Update sabotage.
            if let Some(ref mut sab) = state.active_sabotage {
                if !sab.resolved {
                    sab.timer -= dt;
                    if sab.timer <= 0.0 {
                        // Critical sabotage failed — impostors win.
                        if matches!(sab.kind, SabotageKind::CriticalSystem { .. }) {
                            state.winner = Some((Role::Impostor, WinCondition::SabotageUnresolved));
                            let old = state.phase;
                            state.phase = Phase::Results;
                            events.push(SdEvent::GameOver {
                                winner: Role::Impostor,
                                condition: WinCondition::SabotageUnresolved,
                            });
                            events.push(SdEvent::PhaseChanged {
                                from: old,
                                to: Phase::Results,
                            });
                            return events;
                        }
                        sab.resolved = true;
                        events.push(SdEvent::SabotageResolved {
                            kind: sab.kind.clone(),
                        });
                    }
                }
            }

            // Check win conditions.
            if let Some((winner, condition)) = check_win_conditions(state) {
                state.winner = Some((winner, condition));
                let old = state.phase;
                state.phase = Phase::Results;
                events.push(SdEvent::GameOver { winner, condition });
                events.push(SdEvent::PhaseChanged {
                    from: old,
                    to: Phase::Results,
                });
            }
        }
        Phase::Discussion { ref mut remaining } => {
            *remaining -= dt;
            if *remaining <= 0.0 {
                let old = state.phase;
                state.phase = Phase::Voting;

                let eligible: Vec<EntityId> = state
                    .players
                    .iter()
                    .filter(|p| p.alive && !p.ejected)
                    .map(|p| p.entity)
                    .collect();

                state.vote_session.start(
                    &eligible,
                    VoteConfig {
                        duration: state.config.voting_duration,
                        skip_choice: Some(0),
                        plurality_wins: true,
                        warning_threshold: 10.0,
                    },
                );

                events.push(SdEvent::PhaseChanged {
                    from: old,
                    to: Phase::Voting,
                });
            }
        }
        Phase::Voting => {
            let vote_events = state.vote_session.update(dt);
            if state.vote_session.phase() == crate::voting::VotePhase::Resolved {
                if let Some(outcome) = state.vote_session.outcome().cloned() {
                    let old = state.phase;
                    state.phase = Phase::Ejection;
                    events.push(SdEvent::PhaseChanged {
                        from: old,
                        to: Phase::Ejection,
                    });

                    // Process ejection.
                    if let VoteOutcome::Decided { winner, .. } = outcome {
                        // winner is the entity index of the player to eject (choice ID).
                        // Choice 0 = skip.
                        if winner != 0 {
                            if let Some(player) = state
                                .players
                                .iter_mut()
                                .find(|p| p.entity.index() == winner)
                            {
                                player.alive = false;
                                player.ejected = true;
                                let was_impostor = player.role == Role::Impostor;
                                events.push(SdEvent::PlayerEjected {
                                    entity: player.entity,
                                    was_impostor,
                                });
                            }
                        }
                    }

                    // Check win after ejection.
                    if let Some((winner, condition)) = check_win_conditions(state) {
                        state.winner = Some((winner, condition));
                        state.phase = Phase::Results;
                        events.push(SdEvent::GameOver { winner, condition });
                    } else {
                        // Back to playing.
                        state.phase = Phase::Playing;
                        events.push(SdEvent::PhaseChanged {
                            from: Phase::Ejection,
                            to: Phase::Playing,
                        });
                    }
                }
            }
            let _ = vote_events; // Consumed internally.
        }
        _ => {}
    }

    events
}

/// Report a dead body. Transitions from Playing to Discussion.
pub fn report_body(state: &mut SdState, reporter: EntityId, body: EntityId) -> Vec<SdEvent> {
    let mut events = Vec::new();
    if state.phase != Phase::Playing {
        return events;
    }

    // Cancel active sabotage.
    if let Some(ref mut sab) = state.active_sabotage {
        sab.resolved = true;
        events.push(SdEvent::SabotageResolved {
            kind: sab.kind.clone(),
        });
    }
    state.active_sabotage = None;

    let old = state.phase;
    state.phase = Phase::Discussion {
        remaining: state.config.discussion_duration,
    };
    events.push(SdEvent::BodyReported { reporter, body });
    events.push(SdEvent::PhaseChanged {
        from: old,
        to: state.phase,
    });
    events
}

/// Call an emergency meeting.
pub fn call_emergency_meeting(state: &mut SdState, caller: EntityId) -> Vec<SdEvent> {
    let mut events = Vec::new();
    if state.phase != Phase::Playing {
        return events;
    }

    // Check meeting allowance.
    if let Some((_, remaining)) = state
        .emergency_meetings_remaining
        .iter_mut()
        .find(|(e, _)| *e == caller)
    {
        if *remaining == 0 {
            return events;
        }
        *remaining -= 1;
    } else {
        return events;
    }

    // Cancel active sabotage.
    if let Some(ref mut sab) = state.active_sabotage {
        sab.resolved = true;
        events.push(SdEvent::SabotageResolved {
            kind: sab.kind.clone(),
        });
    }
    state.active_sabotage = None;

    let old = state.phase;
    state.phase = Phase::Discussion {
        remaining: state.config.discussion_duration,
    };
    events.push(SdEvent::EmergencyMeeting { caller });
    events.push(SdEvent::PhaseChanged {
        from: old,
        to: state.phase,
    });
    events
}

/// Impostor attempts to kill a target.
pub fn attempt_kill(
    state: &mut SdState,
    killer: EntityId,
    victim: EntityId,
    distance: f32,
) -> Vec<SdEvent> {
    let mut events = Vec::new();
    if state.phase != Phase::Playing {
        return events;
    }

    let killer_state = match state.players.iter_mut().find(|p| p.entity == killer) {
        Some(p) if p.alive && p.role == Role::Impostor && p.kill_cooldown.is_ready() => p,
        _ => return events,
    };

    if distance > state.config.kill_range {
        return events;
    }

    killer_state.kill_cooldown.trigger();

    let victim_alive = state.players.iter().any(|p| p.entity == victim && p.alive);
    if !victim_alive {
        return events;
    }

    if let Some(v) = state.players.iter_mut().find(|p| p.entity == victim) {
        v.alive = false;
    }

    events.push(SdEvent::PlayerKilled { killer, victim });
    events
}

/// Impostor triggers a sabotage.
pub fn trigger_sabotage(
    state: &mut SdState,
    impostor: EntityId,
    kind: SabotageKind,
) -> Vec<SdEvent> {
    let mut events = Vec::new();
    if state.phase != Phase::Playing {
        return events;
    }
    if state.active_sabotage.is_some() {
        return events;
    }

    let player = match state.players.iter_mut().find(|p| p.entity == impostor) {
        Some(p) if p.alive && p.role == Role::Impostor && p.sabotage_cooldown.is_ready() => p,
        _ => return events,
    };

    player.sabotage_cooldown.trigger();

    let timer = match &kind {
        SabotageKind::DoorLock { duration, .. } => *duration,
        SabotageKind::CriticalSystem { time_limit, .. } => *time_limit,
        SabotageKind::Lights { duration, .. } => *duration,
        SabotageKind::Comms { duration } => *duration,
    };

    // Apply door lock if applicable.
    if let SabotageKind::DoorLock { door_id, duration } = &kind {
        state.doors.lock(*door_id, *duration);
    }

    state.active_sabotage = Some(ActiveSabotage {
        kind: kind.clone(),
        timer,
        resolved: false,
    });

    events.push(SdEvent::SabotageStarted { kind });
    events
}

/// Cast a vote during the Voting phase.
pub fn cast_vote(state: &mut SdState, voter: EntityId, choice: u32) -> Vec<SdEvent> {
    if state.phase != Phase::Voting {
        return Vec::new();
    }
    state.vote_session.cast(voter, choice);
    Vec::new()
}

/// Check win conditions.
pub fn check_win_conditions(state: &SdState) -> Option<(Role, WinCondition)> {
    // All tasks complete.
    if state.tasks.all_complete() {
        return Some((Role::Crewmate, WinCondition::TasksComplete));
    }

    let alive_impostors = state
        .players
        .iter()
        .filter(|p| p.alive && p.role == Role::Impostor)
        .count();
    let alive_crewmates = state
        .players
        .iter()
        .filter(|p| p.alive && p.role == Role::Crewmate)
        .count();

    // All impostors ejected/killed.
    if alive_impostors == 0 {
        return Some((Role::Crewmate, WinCondition::AllImpostorsEjected));
    }

    // Impostor parity.
    if alive_impostors >= alive_crewmates {
        return Some((Role::Impostor, WinCondition::ImpostorParity));
    }

    None
}

/// Get the effective vision radius for a player (accounts for sabotage).
pub fn effective_vision_radius(state: &SdState, entity: EntityId) -> u32 {
    let player = match state.players.iter().find(|p| p.entity == entity) {
        Some(p) => p,
        None => return 0,
    };

    if player.role == Role::Impostor {
        return player.vision_radius;
    }

    // Check lights sabotage.
    if let Some(ref sab) = state.active_sabotage {
        if let SabotageKind::Lights { reduced_radius, .. } = sab.kind {
            if !sab.resolved {
                return reduced_radius;
            }
        }
    }

    player.vision_radius
}

/// Get which entities a given entity can currently see.
pub fn visible_entities(
    state: &SdState,
    observer: EntityId,
    all_positions: &[(EntityId, IVec2)],
    tiles: &dyn TileQuery,
) -> Vec<EntityId> {
    let radius = effective_vision_radius(state, observer);
    let observer_pos = match all_positions.iter().find(|(e, _)| *e == observer) {
        Some((_, pos)) => *pos,
        None => return Vec::new(),
    };

    all_positions
        .iter()
        .filter(|(entity, pos)| {
            if *entity == observer {
                return true;
            }
            vision_ray::can_see(observer_pos, *pos, radius, tiles)
        })
        .map(|(entity, _)| *entity)
        .collect()
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

fn xorshift64(mut state: u64) -> u64 {
    if state == 0 {
        state = 1;
    }
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    fn eid(n: u32) -> EntityId {
        EntityId::from_raw(n, 0)
    }

    #[test]
    fn assign_roles_deterministic() {
        let entities: Vec<EntityId> = (1..=6).map(eid).collect();
        let mut state_a = SdState::new(SdConfig {
            impostor_count: 2,
            seed: 42,
            ..Default::default()
        });
        let mut state_b = SdState::new(SdConfig {
            impostor_count: 2,
            seed: 42,
            ..Default::default()
        });

        assign_roles(&mut state_a, &entities);
        assign_roles(&mut state_b, &entities);

        let roles_a: Vec<Role> = state_a.players.iter().map(|p| p.role).collect();
        let roles_b: Vec<Role> = state_b.players.iter().map(|p| p.role).collect();
        assert_eq!(roles_a, roles_b);

        let impostor_count = roles_a.iter().filter(|r| **r == Role::Impostor).count();
        assert_eq!(impostor_count, 2);
    }

    #[test]
    fn kill_and_win_condition() {
        let entities = vec![eid(1), eid(2), eid(3)];
        let mut state = SdState::new(SdConfig {
            impostor_count: 1,
            kill_cooldown: 0.0,
            seed: 42,
            ..Default::default()
        });
        assign_roles(&mut state, &entities);

        // Find the impostor and a crewmate.
        let impostor = state
            .players
            .iter()
            .find(|p| p.role == Role::Impostor)
            .unwrap()
            .entity;
        let crewmates: Vec<EntityId> = state
            .players
            .iter()
            .filter(|p| p.role == Role::Crewmate)
            .map(|p| p.entity)
            .collect();

        // Kill one crewmate.
        let events = attempt_kill(&mut state, impostor, crewmates[0], 1.0);
        assert!(events
            .iter()
            .any(|e| matches!(e, SdEvent::PlayerKilled { .. })));

        // Now impostor count (1) == crewmate count (1) → ImpostorParity.
        assert_eq!(
            check_win_conditions(&state),
            Some((Role::Impostor, WinCondition::ImpostorParity))
        );
    }

    #[test]
    fn report_body_triggers_discussion() {
        let entities = vec![eid(1), eid(2), eid(3)];
        let mut state = SdState::new(SdConfig {
            impostor_count: 1,
            seed: 42,
            ..Default::default()
        });
        assign_roles(&mut state, &entities);

        let events = report_body(&mut state, eid(1), eid(2));
        assert!(events
            .iter()
            .any(|e| matches!(e, SdEvent::BodyReported { .. })));
        assert!(matches!(state.phase, Phase::Discussion { .. }));
    }

    #[test]
    fn emergency_meeting_limited() {
        let entities = vec![eid(1), eid(2)];
        let mut state = SdState::new(SdConfig {
            impostor_count: 1,
            emergency_meetings_per_player: 1,
            seed: 42,
            ..Default::default()
        });
        assign_roles(&mut state, &entities);

        let events = call_emergency_meeting(&mut state, eid(1));
        assert!(!events.is_empty());

        // Transition back to playing for second attempt.
        state.phase = Phase::Playing;
        let events2 = call_emergency_meeting(&mut state, eid(1));
        assert!(events2.is_empty()); // No meetings left.
    }

    #[test]
    fn lights_sabotage_reduces_vision() {
        let entities = vec![eid(1), eid(2)];
        let mut state = SdState::new(SdConfig {
            impostor_count: 1,
            sabotage_cooldown: 0.0,
            crewmate_vision_radius: 5,
            impostor_vision_radius: 7,
            seed: 42,
            ..Default::default()
        });
        assign_roles(&mut state, &entities);

        let impostor = state
            .players
            .iter()
            .find(|p| p.role == Role::Impostor)
            .unwrap()
            .entity;
        let crewmate = state
            .players
            .iter()
            .find(|p| p.role == Role::Crewmate)
            .unwrap()
            .entity;

        trigger_sabotage(
            &mut state,
            impostor,
            SabotageKind::Lights {
                reduced_radius: 2,
                duration: 30.0,
            },
        );

        assert_eq!(effective_vision_radius(&state, crewmate), 2);
        assert_eq!(effective_vision_radius(&state, impostor), 7);
    }
}
