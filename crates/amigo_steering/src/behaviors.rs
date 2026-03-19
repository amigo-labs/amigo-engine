use amigo_core::math::{Fix, SimVec2};
use serde::{Deserialize, Serialize};

/// A steering agent with configurable behaviors and force limits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SteeringAgent {
    pub max_speed: Fix,
    pub max_force: Fix,
    /// List of (behavior, weight) pairs. Evaluated every tick.
    pub behaviors: Vec<(SteeringBehavior, Fix)>,
}

/// Individual steering behaviors. Combine via `SteeringAgent.behaviors`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SteeringBehavior {
    /// Move directly toward target at max speed.
    Seek { target: SimVec2 },
    /// Move toward target, decelerating within `decel_radius`.
    Arrive { target: SimVec2, decel_radius: Fix },
    /// Flee from threat at max speed.
    Flee { target: SimVec2 },
    /// Push away from neighbors within `radius` (anti-stacking).
    Separation { radius: Fix },
    /// Move toward the center of nearby neighbors.
    Cohesion,
    /// Match velocity of nearby neighbors.
    Alignment,
    /// Follow a sequence of waypoints. `look_ahead` skips to the next waypoint.
    PathFollow {
        waypoints: Vec<SimVec2>,
        look_ahead: Fix,
    },
}

/// Compute the combined steering force for an agent this tick.
///
/// Priority rule: if Separation force magnitude exceeds a small threshold,
/// it overrides other behaviors. Otherwise all behaviors are weighted-summed.
///
/// `neighbors` — (position, velocity) of nearby entities (pre-filtered by caller).
pub fn compute_steering(
    agent: &SteeringAgent,
    self_pos: SimVec2,
    self_vel: SimVec2,
    neighbors: &[(SimVec2, SimVec2)],
) -> SimVec2 {
    let mut separation_total = SimVec2::ZERO;
    let mut other_total = SimVec2::ZERO;

    for (behavior, weight) in &agent.behaviors {
        let force = behavior_force(behavior, self_pos, self_vel, neighbors, agent.max_speed);
        match behavior {
            SteeringBehavior::Separation { .. } => {
                separation_total = separation_total + force * *weight;
            }
            _ => {
                other_total = other_total + force * *weight;
            }
        }
    }

    // Priority: significant separation force takes priority over other behaviors.
    let sep_len = separation_total.length();
    let priority_threshold = Fix::from_num(0.1_f32);

    let result = if sep_len > priority_threshold {
        separation_total
    } else {
        other_total + separation_total
    };

    truncate(result, agent.max_force)
}

fn behavior_force(
    behavior: &SteeringBehavior,
    self_pos: SimVec2,
    self_vel: SimVec2,
    neighbors: &[(SimVec2, SimVec2)],
    max_speed: Fix,
) -> SimVec2 {
    match behavior {
        SteeringBehavior::Seek { target } => seek(*target, self_pos, self_vel, max_speed),
        SteeringBehavior::Arrive {
            target,
            decel_radius,
        } => arrive(*target, self_pos, self_vel, max_speed, *decel_radius),
        SteeringBehavior::Flee { target } => flee(*target, self_pos, self_vel, max_speed),
        SteeringBehavior::Separation { radius } => {
            separation(self_pos, neighbors, *radius, max_speed)
        }
        SteeringBehavior::Cohesion => cohesion(self_pos, self_vel, neighbors, max_speed),
        SteeringBehavior::Alignment => alignment(self_vel, neighbors, max_speed),
        SteeringBehavior::PathFollow {
            waypoints,
            look_ahead,
        } => path_follow(self_pos, self_vel, waypoints, *look_ahead, max_speed),
    }
}

fn seek(target: SimVec2, pos: SimVec2, vel: SimVec2, max_speed: Fix) -> SimVec2 {
    let desired = scale_to(target - pos, max_speed);
    desired - vel
}

fn arrive(
    target: SimVec2,
    pos: SimVec2,
    vel: SimVec2,
    max_speed: Fix,
    decel_radius: Fix,
) -> SimVec2 {
    let delta = target - pos;
    let dist = delta.length();
    if dist == Fix::ZERO {
        return -vel; // steer to stop
    }
    let speed = if decel_radius > Fix::ZERO && dist < decel_radius {
        max_speed * dist / decel_radius
    } else {
        max_speed
    };
    let desired = scale_to(delta, speed);
    desired - vel
}

fn flee(threat: SimVec2, pos: SimVec2, vel: SimVec2, max_speed: Fix) -> SimVec2 {
    let desired = scale_to(pos - threat, max_speed);
    desired - vel
}

fn separation(
    pos: SimVec2,
    neighbors: &[(SimVec2, SimVec2)],
    radius: Fix,
    max_speed: Fix,
) -> SimVec2 {
    let mut force = SimVec2::ZERO;
    for (npos, _) in neighbors {
        let delta = pos - *npos;
        let dist = delta.length();
        if dist < radius && dist > Fix::ZERO {
            // Stronger force the closer the neighbor
            let strength = max_speed * (radius - dist) / radius;
            force = force + scale_to(delta, strength);
        }
    }
    force
}

fn cohesion(
    pos: SimVec2,
    vel: SimVec2,
    neighbors: &[(SimVec2, SimVec2)],
    max_speed: Fix,
) -> SimVec2 {
    if neighbors.is_empty() {
        return SimVec2::ZERO;
    }
    let mut center = SimVec2::ZERO;
    for (npos, _) in neighbors {
        center.x += npos.x;
        center.y += npos.y;
    }
    let n = Fix::from_num(neighbors.len() as i32);
    center = SimVec2::new(center.x / n, center.y / n);
    seek(center, pos, vel, max_speed)
}

fn alignment(vel: SimVec2, neighbors: &[(SimVec2, SimVec2)], max_speed: Fix) -> SimVec2 {
    if neighbors.is_empty() {
        return SimVec2::ZERO;
    }
    let mut avg_vel = SimVec2::ZERO;
    for (_, nvel) in neighbors {
        avg_vel.x += nvel.x;
        avg_vel.y += nvel.y;
    }
    let n = Fix::from_num(neighbors.len() as i32);
    avg_vel = SimVec2::new(avg_vel.x / n, avg_vel.y / n);
    truncate(avg_vel - vel, max_speed)
}

fn path_follow(
    pos: SimVec2,
    vel: SimVec2,
    waypoints: &[SimVec2],
    _look_ahead: Fix,
    max_speed: Fix,
) -> SimVec2 {
    if waypoints.is_empty() {
        return SimVec2::ZERO;
    }
    // Find the nearest waypoint, then seek the one after it (look-ahead of 1)
    let mut nearest_idx = 0;
    let mut nearest_dist_sq = Fix::MAX;
    for (i, wp) in waypoints.iter().enumerate() {
        let d = pos.distance_squared(*wp);
        if d < nearest_dist_sq {
            nearest_dist_sq = d;
            nearest_idx = i;
        }
    }
    let target_idx = (nearest_idx + 1).min(waypoints.len() - 1);
    seek(waypoints[target_idx], pos, vel, max_speed)
}

/// Scale vector to given magnitude. Returns ZERO if vector is zero.
fn scale_to(v: SimVec2, magnitude: Fix) -> SimVec2 {
    let n = v.normalize();
    if n == SimVec2::ZERO {
        SimVec2::ZERO
    } else {
        n * magnitude
    }
}

/// Truncate vector magnitude to max_force. Preserves direction.
pub(crate) fn truncate(v: SimVec2, max_force: Fix) -> SimVec2 {
    let len = v.length();
    if len > max_force && len > Fix::ZERO {
        scale_to(v, max_force)
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_seek(target: SimVec2) -> SteeringAgent {
        SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![(SteeringBehavior::Seek { target }, Fix::ONE)],
        }
    }

    fn agent_arrive(target: SimVec2, decel_radius: Fix) -> SteeringAgent {
        SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![(
                SteeringBehavior::Arrive {
                    target,
                    decel_radius,
                },
                Fix::ONE,
            )],
        }
    }

    // --- Phase 2: Seek / Arrive / Flee ---

    #[test]
    fn seek_points_toward_target() {
        let pos = SimVec2::ZERO;
        let target = SimVec2::from_f32(10.0, 0.0);
        let agent = agent_seek(target);
        let force = compute_steering(&agent, pos, SimVec2::ZERO, &[]);
        assert!(
            force.x > Fix::ZERO,
            "seek force should point in +x, got {}",
            force.x
        );
        assert_eq!(force.y, Fix::ZERO);
    }

    #[test]
    fn seek_max_force_clamped() {
        let agent = agent_seek(SimVec2::from_f32(1000.0, 0.0));
        let force = compute_steering(&agent, SimVec2::ZERO, SimVec2::ZERO, &[]);
        assert!(
            force.length() <= agent.max_force + Fix::from_num(0.01_f32),
            "force magnitude {} exceeds max_force {}",
            force.length().to_num::<f32>(),
            agent.max_force.to_num::<f32>()
        );
    }

    #[test]
    fn arrive_zero_force_at_target() {
        let target = SimVec2::from_f32(0.0, 0.0);
        let agent = agent_arrive(target, Fix::from_num(32.0_f32));
        let force = compute_steering(&agent, target, SimVec2::ZERO, &[]);
        // At the target, desired velocity is 0, so force = -vel = 0
        assert_eq!(force.x, Fix::ZERO);
        assert_eq!(force.y, Fix::ZERO);
    }

    #[test]
    fn arrive_no_overshoot() {
        // AC2: entity with Arrive must stop within 2px of target
        let target = SimVec2::from_f32(100.0, 0.0);
        let decel_radius = Fix::from_num(40.0_f32);
        let agent = SteeringAgent {
            max_speed: Fix::from_num(3.0_f32),
            max_force: Fix::from_num(6.0_f32),
            behaviors: vec![(
                SteeringBehavior::Arrive {
                    target,
                    decel_radius,
                },
                Fix::ONE,
            )],
        };
        let mut pos = SimVec2::ZERO;
        let mut vel = SimVec2::ZERO;

        for _ in 0..200 {
            let force = compute_steering(&agent, pos, vel, &[]);
            vel = truncate(vel + force, agent.max_speed);
            pos = pos + vel;
        }

        let dist = (target - pos).length().to_num::<f32>();
        assert!(
            dist <= 2.0,
            "Arrive overshot: final dist to target = {:.2}px",
            dist
        );
    }

    #[test]
    fn flee_points_away_from_threat() {
        let pos = SimVec2::ZERO;
        let threat = SimVec2::from_f32(10.0, 0.0);
        let agent = SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![(SteeringBehavior::Flee { target: threat }, Fix::ONE)],
        };
        let force = compute_steering(&agent, pos, SimVec2::ZERO, &[]);
        assert!(
            force.x < Fix::ZERO,
            "flee force should point in -x, got {}",
            force.x
        );
    }

    #[test]
    fn seek_is_deterministic() {
        let agent = agent_seek(SimVec2::from_f32(50.0, 30.0));
        let pos = SimVec2::from_f32(10.0, 5.0);
        let vel = SimVec2::from_f32(1.0, 0.5);
        let f1 = compute_steering(&agent, pos, vel, &[]);
        let f2 = compute_steering(&agent, pos, vel, &[]);
        assert_eq!(f1, f2, "Seek must be deterministic");
    }

    // --- Phase 3: Separation / Cohesion / Alignment ---

    #[test]
    fn separation_pushes_apart() {
        let pos_a = SimVec2::from_f32(0.0, 0.0);
        let pos_b = SimVec2::from_f32(1.0, 0.0); // 1px apart, within radius
        let radius = Fix::from_num(8.0_f32);
        let agent = SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![(SteeringBehavior::Separation { radius }, Fix::ONE)],
        };
        let force = compute_steering(&agent, pos_a, SimVec2::ZERO, &[(pos_b, SimVec2::ZERO)]);
        // A is at (0,0), B is at (1,0): separation should push A in -x direction
        assert!(
            force.x < Fix::ZERO,
            "separation force should push away from neighbor in +x, got {}",
            force.x
        );
    }

    #[test]
    fn separation_zero_beyond_radius() {
        let pos_a = SimVec2::ZERO;
        let pos_b = SimVec2::from_f32(100.0, 0.0); // far away
        let radius = Fix::from_num(8.0_f32);
        let agent = SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![(SteeringBehavior::Separation { radius }, Fix::ONE)],
        };
        let force = compute_steering(&agent, pos_a, SimVec2::ZERO, &[(pos_b, SimVec2::ZERO)]);
        assert_eq!(force, SimVec2::ZERO, "No separation force beyond radius");
    }

    #[test]
    fn cohesion_toward_group_center() {
        let pos = SimVec2::ZERO;
        // All neighbors are to the right
        let neighbors = vec![
            (SimVec2::from_f32(10.0, 0.0), SimVec2::ZERO),
            (SimVec2::from_f32(20.0, 0.0), SimVec2::ZERO),
        ];
        let agent = SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![(SteeringBehavior::Cohesion, Fix::ONE)],
        };
        let force = compute_steering(&agent, pos, SimVec2::ZERO, &neighbors);
        assert!(
            force.x > Fix::ZERO,
            "cohesion should pull toward group center (+x), got {}",
            force.x
        );
    }

    #[test]
    fn alignment_matches_neighbor_velocity() {
        let vel = SimVec2::ZERO; // agent is stationary
                                 // Neighbors moving in +x
        let neighbors = vec![
            (SimVec2::from_f32(5.0, 0.0), SimVec2::from_f32(2.0, 0.0)),
            (SimVec2::from_f32(10.0, 0.0), SimVec2::from_f32(2.0, 0.0)),
        ];
        let agent = SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![(SteeringBehavior::Alignment, Fix::ONE)],
        };
        let force = compute_steering(&agent, SimVec2::ZERO, vel, &neighbors);
        assert!(
            force.x > Fix::ZERO,
            "alignment should push toward neighbor avg velocity (+x)"
        );
    }

    #[test]
    fn separation_100_agents_spread_out() {
        // AC1: 100 agents in a 10x10 grid with 1px spacing.
        // After 300 ticks of separation-only, bounding box should grow significantly.
        let n = 100usize;
        let radius = Fix::from_num(12.0_f32);
        let max_speed = Fix::from_num(1.0_f32);
        let max_force = Fix::from_num(2.0_f32);

        let mut positions: Vec<SimVec2> = (0..n)
            .map(|i| SimVec2::from_f32((i % 10) as f32, (i / 10) as f32))
            .collect();
        let mut velocities: Vec<SimVec2> = vec![SimVec2::ZERO; n];

        let agent = SteeringAgent {
            max_speed,
            max_force,
            behaviors: vec![(SteeringBehavior::Separation { radius }, Fix::ONE)],
        };

        for _ in 0..300 {
            let pos_snap = positions.clone();
            let vel_snap = velocities.clone();
            for i in 0..n {
                let neighbors: Vec<_> = (0..n)
                    .filter(|&j| j != i)
                    .map(|j| (pos_snap[j], vel_snap[j]))
                    .collect();
                let force = compute_steering(&agent, pos_snap[i], vel_snap[i], &neighbors);
                velocities[i] = truncate(vel_snap[i] + force, max_speed);
                positions[i] = pos_snap[i] + velocities[i];
            }
        }

        let min_x = positions.iter().map(|p| p.x).min().unwrap();
        let max_x = positions.iter().map(|p| p.x).max().unwrap();
        let span_x = (max_x - min_x).to_num::<f32>();
        assert!(
            span_x > 50.0,
            "Agents didn't spread out enough: x-span = {:.1}px (expected > 50px)",
            span_x
        );
    }

    // --- Phase 4: PathFollow + Priority Steering ---

    #[test]
    fn path_follow_advances_along_waypoints() {
        let waypoints = vec![
            SimVec2::from_f32(0.0, 0.0),
            SimVec2::from_f32(50.0, 0.0),
            SimVec2::from_f32(100.0, 0.0),
        ];
        let start = SimVec2::from_f32(0.0, 0.0);
        let agent = SteeringAgent {
            max_speed: Fix::from_num(3.0_f32),
            max_force: Fix::from_num(6.0_f32),
            behaviors: vec![(
                SteeringBehavior::PathFollow {
                    waypoints: waypoints.clone(),
                    look_ahead: Fix::from_num(16.0_f32),
                },
                Fix::ONE,
            )],
        };
        let mut pos = start;
        let mut vel = SimVec2::ZERO;
        for _ in 0..100 {
            let force = compute_steering(&agent, pos, vel, &[]);
            vel = truncate(vel + force, agent.max_speed);
            pos = pos + vel;
        }
        // Should have advanced well beyond x=0
        assert!(
            pos.x > Fix::from_num(20.0_f32),
            "Agent didn't advance along path: x = {}",
            pos.x.to_num::<f32>()
        );
    }

    #[test]
    fn priority_separation_beats_seek() {
        // AC5: with a neighbor extremely close, separation overrides seek
        let target = SimVec2::from_f32(50.0, 0.0); // seek target to the right
        let neighbor_pos = SimVec2::from_f32(1.0, 0.0); // neighbor very close, also to the right
        let radius = Fix::from_num(8.0_f32);
        let agent = SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![
                (SteeringBehavior::Separation { radius }, Fix::ONE),
                (SteeringBehavior::Seek { target }, Fix::ONE),
            ],
        };
        let force = compute_steering(
            &agent,
            SimVec2::ZERO,
            SimVec2::ZERO,
            &[(neighbor_pos, SimVec2::ZERO)],
        );
        // Separation should dominate: net force should push left (away from neighbor at +x)
        assert!(
            force.x < Fix::ZERO,
            "Separation should override seek: force.x = {} (expected < 0)",
            force.x.to_num::<f32>()
        );
    }

    #[test]
    fn no_neighbors_only_seek_acts() {
        let target = SimVec2::from_f32(10.0, 0.0);
        let agent = SteeringAgent {
            max_speed: Fix::from_num(2.0_f32),
            max_force: Fix::from_num(4.0_f32),
            behaviors: vec![
                (
                    SteeringBehavior::Separation {
                        radius: Fix::from_num(8.0_f32),
                    },
                    Fix::ONE,
                ),
                (SteeringBehavior::Seek { target }, Fix::ONE),
            ],
        };
        let force = compute_steering(&agent, SimVec2::ZERO, SimVec2::ZERO, &[]);
        assert!(
            force.x > Fix::ZERO,
            "With no neighbors, seek should drive +x"
        );
    }
}
