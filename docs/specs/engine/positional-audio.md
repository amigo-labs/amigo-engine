---
status: spec
crate: amigo_audio
depends_on: ["engine/audio", "engine/camera"]
last_updated: 2026-03-18
---

# Positional Audio

## Purpose

Spatial audio system that calculates volume attenuation and stereo panning based
on the 2D distance between sound emitters and a listener (derived from the
active camera). The current `SfxManager` plays all sounds at uniform volume
regardless of world position. This spec adds a `SpatialListener`, per-entity
`SpatialEmitter` components, configurable distance attenuation models, and
stereo panning -- all layered on top of the existing kira-based audio
infrastructure without replacing it.

## Public API

### Attenuation Models

```rust
use amigo_core::math::{Fix, SimVec2};

/// Distance-based volume attenuation curve.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttenuationModel {
    /// Linear falloff: volume = 1.0 - (distance / max_distance).
    Linear,
    /// Inverse-square falloff: volume = ref_distance^2 / distance^2.
    InverseSquare { ref_distance: Fix },
    /// Custom curve defined as a set of (distance_fraction, volume) control
    /// points interpolated linearly. distance_fraction is distance / max_distance
    /// in [0.0, 1.0]. Points must be sorted by distance_fraction.
    Custom { points: Vec<(Fix, Fix)> },
}
```

### SpatialEmitter

```rust
/// Component attached to entities that emit spatial sound.
/// Stored in a `SparseSet<SpatialEmitter>` keyed by `EntityId`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpatialEmitter {
    /// World-space position of the sound source (simulation coordinates).
    pub position: SimVec2,
    /// Maximum distance at which the sound is still audible.
    /// Beyond this radius the sound is fully silent.
    pub max_distance: Fix,
    /// Attenuation curve applied within [0, max_distance].
    pub attenuation: AttenuationModel,
    /// Multiplier applied after attenuation (per-emitter gain).
    pub gain: f32,
    /// When true, panning is disabled (sound plays centered regardless of
    /// X-offset from listener). Useful for ambient area sounds.
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
```

### SpatialListener

```rust
/// Singleton representing the listening point in the world.
/// Typically derived from the active camera each frame.
pub struct SpatialListener {
    /// World-space position of the listener (simulation coordinates).
    pub position: SimVec2,
    /// Half-width of the viewport in world units (used for panning normalization).
    pub viewport_half_width: Fix,
}

impl SpatialListener {
    /// Create a listener from the active camera.
    pub fn from_camera(camera: &Camera) -> Self {
        let view = camera.view_rect();
        Self {
            position: SimVec2::new(
                Fix::from_num(camera.position.x),
                Fix::from_num(camera.position.y),
            ),
            viewport_half_width: Fix::from_num(view.width() * 0.5),
        }
    }
}
```

### SpatialAudioSystem

```rust
/// Orchestrates spatial audio calculations and applies results to kira handles.
pub struct SpatialAudioSystem {
    /// Active spatial sound instances being tracked.
    active: FxHashMap<SpatialSoundId, SpatialSoundInstance>,
    next_id: u32,
}

/// Opaque identifier for a playing spatial sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpatialSoundId(u32);

struct SpatialSoundInstance {
    handle: StaticSoundHandle,
    emitter_entity: Option<EntityId>,
    /// Snapshot of emitter params at play time (used if entity has no emitter component).
    fallback_emitter: SpatialEmitter,
}

impl SpatialAudioSystem {
    pub fn new() -> Self;

    /// Play a spatial sound. If `entity` is Some, the emitter position is read
    /// from the entity's `SpatialEmitter` component each frame. Otherwise the
    /// position from `emitter` is used as a static location.
    pub fn spatial_play(
        &mut self,
        sfx: &mut SfxManager,
        kira: &mut KiraManager<DefaultBackend>,
        name: &str,
        emitter: &SpatialEmitter,
        entity: Option<EntityId>,
    ) -> SpatialSoundId;

    /// Update all active spatial sounds: recalculate volume and panning
    /// based on current listener and emitter positions.
    /// Call once per frame after camera update.
    pub fn update(
        &mut self,
        listener: &SpatialListener,
        emitters: &SparseSet<SpatialEmitter>,
    );

    /// Stop a spatial sound immediately.
    pub fn stop(&mut self, id: SpatialSoundId);

    /// Stop all spatial sounds for a given entity.
    pub fn stop_entity(&mut self, entity: EntityId);

    /// Remove finished sounds from tracking.
    pub fn cleanup(&mut self);

    /// Number of currently tracked spatial sounds.
    pub fn active_count(&self) -> usize;
}
```

### Convenience Extension on SfxManager

```rust
impl SfxManager {
    /// Shorthand: play a sound at a world position with default attenuation.
    /// Delegates to `SpatialAudioSystem::spatial_play`.
    pub fn play_spatial(
        &mut self,
        spatial: &mut SpatialAudioSystem,
        kira: &mut KiraManager<DefaultBackend>,
        name: &str,
        position: SimVec2,
    ) -> SpatialSoundId;
}
```

## Behavior

- **Update cycle**: `SpatialAudioSystem::update()` is called once per frame
  after the camera has been updated. For each active spatial sound it:
  1. Resolves the emitter position (from the ECS `SparseSet<SpatialEmitter>` if
     an entity was provided, otherwise from the stored fallback).
  2. Computes the distance from listener to emitter.
  3. If distance > max_distance, sets volume to 0 and skips panning.
  4. Otherwise applies the `AttenuationModel` to compute a volume factor in
     [0.0, 1.0].
  5. Multiplies by the emitter's `gain` and the SFX channel volume from
     `VolumeChannels`.
  6. Computes stereo panning as `(emitter.x - listener.x) / viewport_half_width`,
     clamped to [-1.0, 1.0]. Left is -1, right is +1.
  7. Applies volume and panning to the kira `StaticSoundHandle`.

- **Attenuation math**:
  - `Linear`: `volume = clamp(1.0 - distance / max_distance, 0.0, 1.0)`
  - `InverseSquare`: `volume = clamp(ref_distance^2 / distance^2, 0.0, 1.0)`
  - `Custom`: linear interpolation between sorted control points.

- **Lifecycle**: Sounds that finish playing (kira reports `Stopped`) are
  automatically removed during `cleanup()`. Entities that are despawned should
  call `stop_entity()` to halt their sounds.

- **Thread safety**: All spatial audio state lives on the main thread alongside
  `SfxManager`. Kira handles the audio thread internally.

## Internal Design

- `FxHashMap<SpatialSoundId, SpatialSoundInstance>` for O(1) lookup by handle.
- Each frame iterates all active instances (~tens, rarely hundreds). No
  acceleration structure needed for typical 2D game sound counts.
- Panning is applied via kira's `StaticSoundHandle::set_panning()` (0.0 = left,
  0.5 = center, 1.0 = right). The computed pan value [-1, 1] is mapped to
  [0, 1] before passing to kira.
- Volume is applied via `StaticSoundHandle::set_volume()` using `kira::Volume`.
- Fixed-point distance math uses `SimVec2` and `Fix` (I16F16) to stay
  consistent with the simulation coordinate system. Final volume/pan values
  are converted to f32 for kira.

## Non-Goals

- **3D audio / HRTF.** This is a 2D engine. Spatial audio is limited to
  distance attenuation and stereo panning.
- **Reverb zones / environmental effects.** Per-zone reverb requires DSP
  infrastructure that kira does not expose simply. Deferred to a future spec.
- **Occlusion / obstruction.** Raycasting through tilemap walls to dampen
  sound is interesting but out of scope. Noted as a future extension.
- **Doppler effect.** Pitch shifting based on relative velocity adds complexity
  with minimal payoff for typical 2D games. Listed as optional future work.
- **Replacing SfxManager.** This system extends SfxManager, it does not replace
  it. Non-spatial sounds continue to use `sfx.play()` as before.

## Open Questions

- Should `SpatialEmitter` support velocity for future Doppler implementation,
  or should velocity be added later as a separate component?
- Is kira's built-in panning granular enough, or do we need a custom panning
  node for smoother stereo imaging?
- Should there be an `OcclusionZone` component that reduces volume for sounds
  behind walls? If so, how does it interact with the tilemap collision layer?
- Maximum number of simultaneous spatial sounds before performance degrades?
  Need profiling to set a sane default cap.

## Referenzen

- [engine/audio](audio.md) -- Kira wrapper, SfxManager, VolumeChannels
- [engine/camera](camera.md) -- Camera position and view_rect for listener
- [engine/core](core.md) -- ECS SparseSet, EntityId, SimVec2, Fix
- FMOD Spatial Audio documentation (feature reference)
- kira `StaticSoundHandle::set_volume()` / `set_panning()` API
