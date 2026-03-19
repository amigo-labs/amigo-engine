use amigo_core::math::{Fix, SimVec2};
use serde::{Deserialize, Serialize};

use crate::behaviors::{SteeringAgent, SteeringBehavior};

/// RON-serializable steering configuration.
/// Uses f32 for human-friendly authoring; converted to Fix on load.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SteeringConfig {
    pub max_speed: f32,
    pub max_force: f32,
    pub separation_radius: f32,
    pub separation_weight: f32,
    pub seek_weight: f32,
    pub decel_radius: f32,
}

impl Default for SteeringConfig {
    fn default() -> Self {
        Self {
            max_speed: 2.0,
            max_force: 4.0,
            separation_radius: 16.0,
            separation_weight: 1.0,
            seek_weight: 0.8,
            decel_radius: 32.0,
        }
    }
}

impl SteeringConfig {
    /// Build a SteeringAgent with Separation + Arrive toward `target`.
    pub fn to_agent(&self, target: SimVec2) -> SteeringAgent {
        SteeringAgent {
            max_speed: Fix::from_num(self.max_speed),
            max_force: Fix::from_num(self.max_force),
            behaviors: vec![
                (
                    SteeringBehavior::Separation {
                        radius: Fix::from_num(self.separation_radius),
                    },
                    Fix::from_num(self.separation_weight),
                ),
                (
                    SteeringBehavior::Arrive {
                        target,
                        decel_radius: Fix::from_num(self.decel_radius),
                    },
                    Fix::from_num(self.seek_weight),
                ),
            ],
        }
    }

    /// Deserialize from a RON string.
    pub fn from_ron(src: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(src)
    }

    /// Serialize to a RON string.
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_roundtrip() {
        let cfg = SteeringConfig {
            max_speed: 3.5,
            max_force: 7.0,
            separation_radius: 12.0,
            separation_weight: 1.5,
            seek_weight: 0.6,
            decel_radius: 48.0,
        };
        let ron_str = cfg.to_ron();
        let parsed = SteeringConfig::from_ron(&ron_str).expect("RON parse failed");
        let _ = parsed.to_agent(SimVec2::ZERO); // ensure to_agent compiles
        assert!((parsed.max_speed - cfg.max_speed).abs() < f32::EPSILON);
        assert!((parsed.max_force - cfg.max_force).abs() < f32::EPSILON);
        assert!((parsed.separation_radius - cfg.separation_radius).abs() < f32::EPSILON);
    }

    #[test]
    fn to_agent_builds_correct_behavior_count() {
        let cfg = SteeringConfig::default();
        let agent = cfg.to_agent(SimVec2::from_f32(100.0, 100.0));
        assert_eq!(
            agent.behaviors.len(),
            2,
            "Default config: Separation + Arrive"
        );
        assert_eq!(agent.max_speed, Fix::from_num(cfg.max_speed));
    }
}
