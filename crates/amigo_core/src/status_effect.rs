use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Status effect types (real-time, not turn-based)
// ---------------------------------------------------------------------------

/// Type of status effect applied to an enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectType {
    /// Reduces movement speed by `magnitude`% (e.g. 50 = half speed).
    Slow,
    /// Completely stops movement.
    Stun,
    /// Deals `magnitude` damage per second (fire).
    Burn,
    /// Deals `magnitude` damage per second (stacking, nature).
    Poison,
    /// Reduces armor by `magnitude` points.
    ArmorBreak,
    /// Increases damage taken by `magnitude`%.
    Vulnerable,
}

/// A single active status effect instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusEffect {
    pub effect_type: EffectType,
    /// Strength of the effect (interpretation depends on type).
    pub magnitude: f32,
    /// Remaining duration in seconds.
    pub remaining: f32,
    /// Total duration for UI display.
    pub duration: f32,
    /// Source entity (for kill attribution on DoT kills).
    pub source_id: Option<u32>,
}

impl StatusEffect {
    pub fn new(effect_type: EffectType, magnitude: f32, duration: f32) -> Self {
        Self {
            effect_type,
            magnitude,
            remaining: duration,
            duration,
            source_id: None,
        }
    }

    pub fn with_source(mut self, source: u32) -> Self {
        self.source_id = Some(source);
        self
    }

    /// Fraction of duration elapsed (0.0 = just applied, 1.0 = expired).
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            1.0 - (self.remaining / self.duration)
        }
    }

    pub fn is_expired(&self) -> bool {
        self.remaining <= 0.0
    }
}

// ---------------------------------------------------------------------------
// Status effect container
// ---------------------------------------------------------------------------

/// Container for all active status effects on an entity.
#[derive(Clone, Debug, Default)]
pub struct StatusEffects {
    effects: Vec<StatusEffect>,
}

impl StatusEffects {
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Apply a new status effect. If an effect of the same type already exists,
    /// refresh duration if new one is longer, and use the stronger magnitude.
    pub fn apply(&mut self, effect: StatusEffect) {
        if let Some(existing) = self
            .effects
            .iter_mut()
            .find(|e| e.effect_type == effect.effect_type)
        {
            // Refresh: keep the stronger magnitude and longer remaining duration
            if effect.magnitude > existing.magnitude {
                existing.magnitude = effect.magnitude;
            }
            if effect.remaining > existing.remaining {
                existing.remaining = effect.remaining;
                existing.duration = effect.duration;
            }
        } else {
            self.effects.push(effect);
        }
    }

    /// Apply a stacking effect (e.g. Poison stacks). Always adds a new instance.
    pub fn apply_stacking(&mut self, effect: StatusEffect) {
        self.effects.push(effect);
    }

    /// Tick all effects by `dt` seconds and remove expired ones.
    pub fn update(&mut self, dt: f32) {
        for effect in self.effects.iter_mut() {
            effect.remaining -= dt;
        }
        self.effects.retain(|e| e.remaining > 0.0);
    }

    /// Get the speed multiplier from slow/stun effects.
    /// Returns a value between 0.0 (stunned) and 1.0 (no slow).
    pub fn speed_multiplier(&self) -> f32 {
        let mut mult = 1.0f32;
        for effect in &self.effects {
            match effect.effect_type {
                EffectType::Stun => return 0.0,
                EffectType::Slow => {
                    // Apply strongest slow (don't stack slows multiplicatively)
                    let slow_factor = 1.0 - (effect.magnitude / 100.0).clamp(0.0, 0.9);
                    mult = mult.min(slow_factor);
                }
                _ => {}
            }
        }
        mult
    }

    /// Total damage per second from all DoT effects.
    pub fn damage_per_second(&self) -> f32 {
        self.effects
            .iter()
            .filter(|e| matches!(e.effect_type, EffectType::Burn | EffectType::Poison))
            .map(|e| e.magnitude)
            .sum()
    }

    /// Total armor reduction from ArmorBreak effects.
    pub fn armor_reduction(&self) -> f32 {
        self.effects
            .iter()
            .filter(|e| e.effect_type == EffectType::ArmorBreak)
            .map(|e| e.magnitude)
            .sum()
    }

    /// Total bonus damage taken multiplier from Vulnerable effects.
    /// Returns 1.0 + total vulnerability (e.g. 1.25 for 25% more damage taken).
    pub fn damage_taken_multiplier(&self) -> f32 {
        let vuln: f32 = self
            .effects
            .iter()
            .filter(|e| e.effect_type == EffectType::Vulnerable)
            .map(|e| e.magnitude / 100.0)
            .sum();
        1.0 + vuln
    }

    /// Whether the entity has any effect of the given type.
    pub fn has(&self, effect_type: EffectType) -> bool {
        self.effects.iter().any(|e| e.effect_type == effect_type)
    }

    /// Number of active effects.
    pub fn count(&self) -> usize {
        self.effects.len()
    }

    /// Is the entity stunned?
    pub fn is_stunned(&self) -> bool {
        self.has(EffectType::Stun)
    }

    /// Clear all effects.
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Iterate active effects.
    pub fn iter(&self) -> impl Iterator<Item = &StatusEffect> {
        self.effects.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_reduces_speed() {
        let mut effects = StatusEffects::new();
        assert!((effects.speed_multiplier() - 1.0).abs() < 0.001);

        effects.apply(StatusEffect::new(EffectType::Slow, 50.0, 3.0));
        assert!((effects.speed_multiplier() - 0.5).abs() < 0.001);
    }

    #[test]
    fn stun_stops_movement() {
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffect::new(EffectType::Stun, 0.0, 2.0));
        assert_eq!(effects.speed_multiplier(), 0.0);
        assert!(effects.is_stunned());
    }

    #[test]
    fn burn_does_dot() {
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffect::new(EffectType::Burn, 10.0, 3.0));
        assert!((effects.damage_per_second() - 10.0).abs() < 0.001);
    }

    #[test]
    fn poison_stacks() {
        let mut effects = StatusEffects::new();
        effects.apply_stacking(StatusEffect::new(EffectType::Poison, 5.0, 3.0));
        effects.apply_stacking(StatusEffect::new(EffectType::Poison, 5.0, 3.0));
        assert!((effects.damage_per_second() - 10.0).abs() < 0.001);
        assert_eq!(effects.count(), 2);
    }

    #[test]
    fn effects_expire() {
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffect::new(EffectType::Slow, 50.0, 1.0));
        assert_eq!(effects.count(), 1);

        effects.update(0.5);
        assert_eq!(effects.count(), 1);

        effects.update(0.6); // past 1.0s total
        assert_eq!(effects.count(), 0);
        assert!((effects.speed_multiplier() - 1.0).abs() < 0.001);
    }

    #[test]
    fn slow_refreshes_not_stacks() {
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffect::new(EffectType::Slow, 30.0, 2.0));
        effects.apply(StatusEffect::new(EffectType::Slow, 50.0, 3.0));

        // Should have one slow with the stronger magnitude
        assert_eq!(effects.count(), 1);
        assert!((effects.speed_multiplier() - 0.5).abs() < 0.001);
    }

    #[test]
    fn vulnerable_increases_damage_taken() {
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffect::new(EffectType::Vulnerable, 25.0, 5.0));
        assert!((effects.damage_taken_multiplier() - 1.25).abs() < 0.001);
    }

    #[test]
    fn armor_break() {
        let mut effects = StatusEffects::new();
        effects.apply(StatusEffect::new(EffectType::ArmorBreak, 10.0, 5.0));
        assert!((effects.armor_reduction() - 10.0).abs() < 0.001);
    }
}
