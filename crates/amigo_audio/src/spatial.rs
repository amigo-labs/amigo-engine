//! Pure-math spatial audio: distance attenuation, stereo panning, and data
//! structures for positional sound. No kira dependency — the integration layer
//! is left for game code.

use amigo_core::math::{Fix, SimVec2};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Attenuation models
// ---------------------------------------------------------------------------

/// Distance-based volume attenuation curve.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttenuationModel {
    /// Linear falloff: `volume = 1.0 - (distance / max_distance)`.
    Linear,
    /// Inverse-square falloff: `volume = ref_distance² / distance²`.
    InverseSquare {
        /// Reference distance at which volume is 1.0.
        ref_distance: Fix,
    },
    /// Custom curve defined as `(distance_fraction, volume)` control points
    /// interpolated linearly. `distance_fraction` is `distance / max_distance`
    /// in \[0.0, 1.0\]. Points **must** be sorted by `distance_fraction`.
    Custom {
        /// Sorted control points.
        points: Vec<(Fix, Fix)>,
    },
}

impl AttenuationModel {
    /// Evaluate the attenuation curve.
    ///
    /// * `distance` — world-space distance from listener to emitter.
    /// * `max_distance` — maximum audible distance of the emitter.
    ///
    /// Returns a volume factor in \[0.0, 1.0\].
    pub fn apply(&self, distance: Fix, max_distance: Fix) -> f32 {
        // Beyond max_distance ⇒ silent.
        if distance >= max_distance {
            return 0.0;
        }
        // At distance zero (or negative) ⇒ full volume.
        if distance <= Fix::ZERO {
            return 1.0;
        }

        match self {
            AttenuationModel::Linear => {
                // volume = 1.0 - distance / max_distance
                let ratio: f32 = (distance / max_distance).to_num();
                (1.0 - ratio).clamp(0.0, 1.0)
            }
            AttenuationModel::InverseSquare { ref_distance } => {
                // volume = ref_distance² / distance²
                let rd: f32 = ref_distance.to_num();
                let d: f32 = distance.to_num();
                if d <= 0.0 {
                    return 1.0;
                }
                ((rd * rd) / (d * d)).clamp(0.0, 1.0)
            }
            AttenuationModel::Custom { points } => {
                if points.is_empty() {
                    return 1.0;
                }
                let frac: f32 = (distance / max_distance).to_num();
                lerp_custom_curve(points, frac)
            }
        }
    }
}

/// Linearly interpolate a custom attenuation curve.
fn lerp_custom_curve(points: &[(Fix, Fix)], frac: f32) -> f32 {
    // Before first point — clamp to first volume.
    let first_frac: f32 = points[0].0.to_num();
    let first_vol: f32 = points[0].1.to_num();
    if frac <= first_frac {
        return first_vol.clamp(0.0, 1.0);
    }

    // Walk the curve.
    for window in points.windows(2) {
        let (f0, v0) = (window[0].0.to_num::<f32>(), window[0].1.to_num::<f32>());
        let (f1, v1) = (window[1].0.to_num::<f32>(), window[1].1.to_num::<f32>());
        if frac <= f1 {
            let span = f1 - f0;
            if span <= f32::EPSILON {
                return v1.clamp(0.0, 1.0);
            }
            let t = (frac - f0) / span;
            return (v0 + (v1 - v0) * t).clamp(0.0, 1.0);
        }
    }

    // Past last point — clamp to last volume.
    let last_vol: f32 = points.last().unwrap().1.to_num();
    last_vol.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// SpatialEmitter
// ---------------------------------------------------------------------------

/// Component attached to entities that emit spatial sound.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpatialEmitter {
    /// World-space position of the sound source (simulation coordinates).
    pub position: SimVec2,
    /// Maximum distance at which the sound is still audible.
    pub max_distance: Fix,
    /// Attenuation curve applied within \[0, max_distance\].
    pub attenuation: AttenuationModel,
    /// Multiplier applied after attenuation (per-emitter gain).
    pub gain: f32,
    /// When `true`, panning is disabled and the sound plays centred.
    pub mono_center: bool,
}

impl Default for SpatialEmitter {
    fn default() -> Self {
        Self {
            position: SimVec2::ZERO,
            max_distance: Fix::from_num(400),
            attenuation: AttenuationModel::Linear,
            gain: 1.0,
            mono_center: false,
        }
    }
}

// ---------------------------------------------------------------------------
// SpatialListener
// ---------------------------------------------------------------------------

/// Singleton representing the listening point in the world.
/// Typically derived from the active camera each frame.
#[derive(Clone, Debug)]
pub struct SpatialListener {
    /// World-space position of the listener (simulation coordinates).
    pub position: SimVec2,
    /// Half-width of the viewport in world units (used for panning normalisation).
    pub viewport_half_width: Fix,
}

impl Default for SpatialListener {
    fn default() -> Self {
        Self {
            position: SimVec2::ZERO,
            viewport_half_width: Fix::from_num(200),
        }
    }
}

// ---------------------------------------------------------------------------
// SpatialSoundId — opaque handle
// ---------------------------------------------------------------------------

/// Opaque identifier for a playing spatial sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpatialSoundId(u32);

impl SpatialSoundId {
    /// Create a new id (intended for use by the audio system, not user code).
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Access the inner numeric value.
    pub fn raw(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// Pure functions: volume & panning
// ---------------------------------------------------------------------------

/// Compute the final volume for an emitter/listener pair.
///
/// Returns a value in \[0.0, emitter.gain\] (clamped to \[0.0, 1.0\] after gain).
pub fn compute_volume(emitter: &SpatialEmitter, listener: &SpatialListener) -> f32 {
    let diff = emitter.position - listener.position;
    let distance = diff.length();
    let atten = emitter.attenuation.apply(distance, emitter.max_distance);
    (atten * emitter.gain).clamp(0.0, 1.0)
}

/// Compute the stereo pan for an emitter/listener pair.
///
/// Returns a value in \[-1.0, 1.0\] where -1 is hard left and +1 is hard right.
/// If the emitter has `mono_center` set, always returns 0.0.
pub fn compute_pan(emitter: &SpatialEmitter, listener: &SpatialListener) -> f32 {
    if emitter.mono_center {
        return 0.0;
    }
    let half_w: f32 = listener.viewport_half_width.to_num();
    if half_w <= f32::EPSILON {
        return 0.0;
    }
    let dx: f32 = (emitter.position.x - listener.position.x).to_num();
    (dx / half_w).clamp(-1.0, 1.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(v: f32) -> Fix {
        Fix::from_num(v)
    }

    fn emitter_at(x: f32, y: f32) -> SpatialEmitter {
        SpatialEmitter {
            position: SimVec2::from_f32(x, y),
            ..Default::default()
        }
    }

    fn listener_at(x: f32, y: f32) -> SpatialListener {
        SpatialListener {
            position: SimVec2::from_f32(x, y),
            viewport_half_width: fix(200.0),
        }
    }

    // -- Attenuation tests --------------------------------------------------

    #[test]
    fn linear_attenuation_at_zero_distance() {
        let model = AttenuationModel::Linear;
        let vol = model.apply(fix(0.0), fix(400.0));
        assert!((vol - 1.0).abs() < 1e-4, "Expected 1.0, got {vol}");
    }

    #[test]
    fn linear_attenuation_at_half_distance() {
        let model = AttenuationModel::Linear;
        let vol = model.apply(fix(200.0), fix(400.0));
        assert!((vol - 0.5).abs() < 1e-4, "Expected 0.5, got {vol}");
    }

    #[test]
    fn linear_attenuation_at_max_distance() {
        let model = AttenuationModel::Linear;
        let vol = model.apply(fix(400.0), fix(400.0));
        assert!((vol - 0.0).abs() < 1e-4, "Expected 0.0, got {vol}");
    }

    #[test]
    fn linear_attenuation_beyond_max_distance() {
        let model = AttenuationModel::Linear;
        let vol = model.apply(fix(500.0), fix(400.0));
        assert_eq!(vol, 0.0);
    }

    #[test]
    fn inverse_square_attenuation() {
        // ref_distance=10, distance=20 ⇒ volume = 100/400 = 0.25
        let model = AttenuationModel::InverseSquare {
            ref_distance: fix(10.0),
        };
        let vol = model.apply(fix(20.0), fix(1000.0));
        assert!(
            (vol - 0.25).abs() < 1e-3,
            "Expected ~0.25, got {vol}"
        );
    }

    #[test]
    fn inverse_square_at_ref_distance() {
        let model = AttenuationModel::InverseSquare {
            ref_distance: fix(10.0),
        };
        let vol = model.apply(fix(10.0), fix(1000.0));
        assert!(
            (vol - 1.0).abs() < 1e-3,
            "Expected ~1.0, got {vol}"
        );
    }

    #[test]
    fn custom_curve_interpolation() {
        // 0.0 → 1.0,  0.5 → 0.8,  1.0 → 0.0
        let model = AttenuationModel::Custom {
            points: vec![(fix(0.0), fix(1.0)), (fix(0.5), fix(0.8)), (fix(1.0), fix(0.0))],
        };
        // At fraction 0.25 (between first two points): lerp(1.0, 0.8, 0.5) = 0.9
        let vol = model.apply(fix(100.0), fix(400.0)); // frac = 0.25
        assert!(
            (vol - 0.9).abs() < 1e-2,
            "Expected ~0.9, got {vol}"
        );
    }

    // -- Panning tests ------------------------------------------------------

    #[test]
    fn pan_centered_when_same_position() {
        let emitter = emitter_at(100.0, 50.0);
        let listener = listener_at(100.0, 50.0);
        let pan = compute_pan(&emitter, &listener);
        assert!((pan - 0.0).abs() < 1e-4, "Expected 0.0, got {pan}");
    }

    #[test]
    fn pan_right_when_emitter_is_right() {
        let emitter = emitter_at(300.0, 0.0);
        let listener = listener_at(100.0, 0.0);
        // dx = 200, half_w = 200 → pan = 1.0
        let pan = compute_pan(&emitter, &listener);
        assert!((pan - 1.0).abs() < 1e-3, "Expected 1.0, got {pan}");
    }

    #[test]
    fn pan_left_when_emitter_is_left() {
        let emitter = emitter_at(0.0, 0.0);
        let listener = listener_at(100.0, 0.0);
        // dx = -100, half_w = 200 → pan = -0.5
        let pan = compute_pan(&emitter, &listener);
        assert!((pan - (-0.5)).abs() < 1e-3, "Expected -0.5, got {pan}");
    }

    #[test]
    fn pan_clamped_to_extremes() {
        let emitter = emitter_at(1000.0, 0.0);
        let listener = listener_at(0.0, 0.0);
        let pan = compute_pan(&emitter, &listener);
        assert!((pan - 1.0).abs() < 1e-4, "Pan should clamp to 1.0, got {pan}");
    }

    #[test]
    fn mono_center_disables_panning() {
        let mut emitter = emitter_at(300.0, 0.0);
        emitter.mono_center = true;
        let listener = listener_at(0.0, 0.0);
        let pan = compute_pan(&emitter, &listener);
        assert_eq!(pan, 0.0);
    }

    // -- compute_volume integration test ------------------------------------

    #[test]
    fn volume_combines_attenuation_and_gain() {
        let mut emitter = emitter_at(200.0, 0.0);
        emitter.gain = 0.5;
        let listener = listener_at(0.0, 0.0);
        // distance = 200, max = 400, linear atten = 0.5, gain = 0.5 → 0.25
        let vol = compute_volume(&emitter, &listener);
        assert!(
            (vol - 0.25).abs() < 1e-2,
            "Expected ~0.25, got {vol}"
        );
    }

    #[test]
    fn volume_zero_beyond_max_distance() {
        let emitter = emitter_at(500.0, 0.0);
        let listener = listener_at(0.0, 0.0);
        // distance = 500 > max 400
        let vol = compute_volume(&emitter, &listener);
        assert_eq!(vol, 0.0);
    }
}
