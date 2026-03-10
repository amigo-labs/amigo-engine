use amigo_core::Rect;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Sprite-sheet animation (original)
// ---------------------------------------------------------------------------

/// A single frame of animation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimFrame {
    /// UV rect within the sprite sheet/texture.
    pub uv: Rect,
    /// Duration of this frame in ticks.
    pub duration: u32,
}

/// An animation sequence (e.g., "walk_right", "idle", "attack").
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Animation {
    pub name: String,
    pub frames: Vec<AnimFrame>,
    pub looping: bool,
}

impl Animation {
    pub fn total_duration(&self) -> u32 {
        self.frames.iter().map(|f| f.duration).sum()
    }
}

/// Playback mode for an animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    Loop,
    Once,
    PingPong,
}

/// Animation player that tracks current frame and timing.
#[derive(Clone, Debug)]
pub struct AnimPlayer {
    pub current_animation: String,
    pub frame_index: usize,
    pub ticks_in_frame: u32,
    pub play_mode: PlayMode,
    pub finished: bool,
    pub speed: f32,
    /// Events collected during the last `update` call.
    pub pending_events: Vec<AnimEvent>,
}

impl AnimPlayer {
    pub fn new(animation: impl Into<String>) -> Self {
        Self {
            current_animation: animation.into(),
            frame_index: 0,
            ticks_in_frame: 0,
            play_mode: PlayMode::Loop,
            finished: false,
            speed: 1.0,
            pending_events: Vec::new(),
        }
    }

    /// Play a new animation, resetting if different from current.
    pub fn play(&mut self, name: &str, mode: PlayMode) {
        if self.current_animation != name {
            self.current_animation = name.to_string();
            self.frame_index = 0;
            self.ticks_in_frame = 0;
            self.finished = false;
        }
        self.play_mode = mode;
    }

    /// Advance the animation by one tick.
    pub fn update(&mut self, animation: &Animation) {
        self.update_with_events(animation, None);
    }

    /// Advance the animation by one tick, collecting events from the given
    /// [`EventTrack`] that fire during this tick.
    pub fn update_with_events(
        &mut self,
        animation: &Animation,
        event_track: Option<&EventTrack>,
    ) {
        self.pending_events.clear();

        if self.finished || animation.frames.is_empty() {
            return;
        }

        // Compute the *time* (in ticks) before and after advancing so we can
        // query the event track for any events that fall within this window.
        let time_before = self.current_time(animation);

        self.ticks_in_frame += 1;
        let current_frame = &animation.frames[self.frame_index];

        if self.ticks_in_frame >= current_frame.duration {
            self.ticks_in_frame = 0;
            self.frame_index += 1;

            if self.frame_index >= animation.frames.len() {
                match self.play_mode {
                    PlayMode::Loop => self.frame_index = 0,
                    PlayMode::Once => {
                        self.frame_index = animation.frames.len() - 1;
                        self.finished = true;
                    }
                    PlayMode::PingPong => {
                        // Reverse the frames
                        self.frame_index = animation.frames.len().saturating_sub(2);
                    }
                }
            }
        }

        let time_after = self.current_time(animation);

        // Collect events that occurred in the [time_before, time_after) window.
        if let Some(track) = event_track {
            let collected = track.collect_events(time_before, time_after);
            self.pending_events
                .extend(collected.into_iter().cloned());
        }
    }

    /// Get the current frame's UV rect.
    pub fn current_uv(&self, animation: &Animation) -> Rect {
        if animation.frames.is_empty() {
            return Rect::new(0.0, 0.0, 1.0, 1.0);
        }
        let idx = self.frame_index.min(animation.frames.len() - 1);
        animation.frames[idx].uv
    }

    /// Compute the elapsed time (in ticks) based on frame index and ticks
    /// within the current frame.
    fn current_time(&self, animation: &Animation) -> f32 {
        let mut t: f32 = 0.0;
        for (i, frame) in animation.frames.iter().enumerate() {
            if i >= self.frame_index {
                break;
            }
            t += frame.duration as f32;
        }
        t += self.ticks_in_frame as f32;
        t
    }
}

/// Animation library holding all named animations.
#[derive(Clone, Debug, Default)]
pub struct AnimationLibrary {
    animations: FxHashMap<String, Animation>,
}

impl AnimationLibrary {
    pub fn new() -> Self {
        Self {
            animations: FxHashMap::default(),
        }
    }

    pub fn add(&mut self, animation: Animation) {
        self.animations.insert(animation.name.clone(), animation);
    }

    pub fn get(&self, name: &str) -> Option<&Animation> {
        self.animations.get(name)
    }
}

// ---------------------------------------------------------------------------
// Skeletal Animation System
// ---------------------------------------------------------------------------

/// Unique identifier for a bone within a [`Skeleton`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoneId(pub u16);

/// A 2D affine transform decomposed into translation, rotation, and scale.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoneTransform {
    pub position: (f32, f32),
    pub rotation: f32,
    pub scale: (f32, f32),
}

impl Default for BoneTransform {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0),
            rotation: 0.0,
            scale: (1.0, 1.0),
        }
    }
}

impl BoneTransform {
    /// Linearly interpolate between two transforms.
    pub fn lerp(&self, other: &BoneTransform, t: f32) -> BoneTransform {
        BoneTransform {
            position: (
                self.position.0 + (other.position.0 - self.position.0) * t,
                self.position.1 + (other.position.1 - self.position.1) * t,
            ),
            rotation: self.rotation + (other.rotation - self.rotation) * t,
            scale: (
                self.scale.0 + (other.scale.0 - self.scale.0) * t,
                self.scale.1 + (other.scale.1 - self.scale.1) * t,
            ),
        }
    }

    /// Combine two transforms: apply `child` in the local space of `self`.
    ///
    /// This performs a simplified 2-D concatenation:
    ///   - Scale the child position by parent scale
    ///   - Rotate the child position by parent rotation
    ///   - Translate by parent position
    ///   - Rotations add, scales multiply
    pub fn concatenate(&self, child: &BoneTransform) -> BoneTransform {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        let sx = child.position.0 * self.scale.0;
        let sy = child.position.1 * self.scale.1;
        BoneTransform {
            position: (
                self.position.0 + cos * sx - sin * sy,
                self.position.1 + sin * sx + cos * sy,
            ),
            rotation: self.rotation + child.rotation,
            scale: (
                self.scale.0 * child.scale.0,
                self.scale.1 * child.scale.1,
            ),
        }
    }
}

/// A single bone in a [`Skeleton`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    pub parent: Option<BoneId>,
    pub local_transform: BoneTransform,
}

/// A 2-D skeleton made up of a hierarchy of [`Bone`]s.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skeleton {
    pub name: String,
    pub bones: Vec<Bone>,
}

impl Skeleton {
    /// Create a new, empty skeleton.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bones: Vec::new(),
        }
    }

    /// Add a bone and return its [`BoneId`].
    pub fn add_bone(
        &mut self,
        name: impl Into<String>,
        parent: Option<BoneId>,
        local_transform: BoneTransform,
    ) -> BoneId {
        let id = BoneId(self.bones.len() as u16);
        self.bones.push(Bone {
            name: name.into(),
            parent,
            local_transform,
        });
        id
    }

    /// Find a bone by name, returning its [`BoneId`].
    pub fn find_bone(&self, name: &str) -> Option<BoneId> {
        self.bones
            .iter()
            .position(|b| b.name == name)
            .map(|i| BoneId(i as u16))
    }

    /// Compute the world-space transform of the given bone by walking up the
    /// parent chain and concatenating transforms.
    pub fn world_transform(&self, bone_id: BoneId) -> BoneTransform {
        self.world_transform_with_overrides(bone_id, None)
    }

    /// Like [`world_transform`](Self::world_transform) but allows per-bone
    /// overrides (e.g. from a sampled animation pose).
    pub fn world_transform_with_overrides(
        &self,
        bone_id: BoneId,
        overrides: Option<&[BoneTransform]>,
    ) -> BoneTransform {
        // Collect the chain from bone_id up to the root.
        let mut chain = Vec::new();
        let mut current = Some(bone_id);
        while let Some(id) = current {
            let bone = &self.bones[id.0 as usize];
            let t = if let Some(ovr) = overrides {
                ovr.get(id.0 as usize)
                    .copied()
                    .unwrap_or(bone.local_transform)
            } else {
                bone.local_transform
            };
            chain.push(t);
            current = bone.parent;
        }

        // Walk from root to leaf, concatenating.
        chain
            .iter()
            .rev()
            .copied()
            .reduce(|parent, child| parent.concatenate(&child))
            .unwrap_or_default()
    }
}

/// A single keyframe for a bone animation channel.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoneKeyframe {
    pub time: f32,
    pub position: (f32, f32),
    pub rotation: f32,
    pub scale: (f32, f32),
}

/// Animation channel for a single bone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoneAnimation {
    pub bone_id: BoneId,
    pub keyframes: Vec<BoneKeyframe>,
}

impl BoneAnimation {
    /// Sample this channel at the given time, linearly interpolating between
    /// the two surrounding keyframes.
    pub fn sample(&self, time: f32) -> BoneTransform {
        if self.keyframes.is_empty() {
            return BoneTransform::default();
        }
        if self.keyframes.len() == 1 || time <= self.keyframes[0].time {
            let kf = &self.keyframes[0];
            return BoneTransform {
                position: kf.position,
                rotation: kf.rotation,
                scale: kf.scale,
            };
        }
        let last = self.keyframes.last().unwrap();
        if time >= last.time {
            return BoneTransform {
                position: last.position,
                rotation: last.rotation,
                scale: last.scale,
            };
        }

        // Find the two keyframes surrounding `time`.
        let idx = self
            .keyframes
            .iter()
            .position(|kf| kf.time > time)
            .unwrap_or(self.keyframes.len() - 1);
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];
        let span = b.time - a.time;
        let t = if span > 0.0 {
            (time - a.time) / span
        } else {
            0.0
        };

        let ta = BoneTransform {
            position: a.position,
            rotation: a.rotation,
            scale: a.scale,
        };
        let tb = BoneTransform {
            position: b.position,
            rotation: b.rotation,
            scale: b.scale,
        };
        ta.lerp(&tb, t)
    }
}

/// A full skeletal animation clip containing channels for multiple bones.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkeletalAnimation {
    pub name: String,
    pub duration: f32,
    pub bone_anims: Vec<BoneAnimation>,
    pub looping: bool,
}

impl SkeletalAnimation {
    /// Sample every bone channel at the given time, returning one
    /// [`BoneTransform`] per bone in the skeleton (identity for bones
    /// without a channel).
    pub fn sample(&self, skeleton: &Skeleton, time: f32) -> Vec<BoneTransform> {
        let mut pose = vec![BoneTransform::default(); skeleton.bones.len()];
        // Copy rest-pose from skeleton.
        for (i, bone) in skeleton.bones.iter().enumerate() {
            pose[i] = bone.local_transform;
        }
        // Overwrite with sampled values.
        for ba in &self.bone_anims {
            let idx = ba.bone_id.0 as usize;
            if idx < pose.len() {
                pose[idx] = ba.sample(time);
            }
        }
        pose
    }
}

/// Plays back a [`SkeletalAnimation`], tracking time and looping.
#[derive(Clone, Debug)]
pub struct SkeletalPlayer {
    pub current_animation: String,
    pub time: f32,
    pub speed: f32,
    pub finished: bool,
}

impl SkeletalPlayer {
    pub fn new(animation: impl Into<String>) -> Self {
        Self {
            current_animation: animation.into(),
            time: 0.0,
            speed: 1.0,
            finished: false,
        }
    }

    /// Advance the player by `dt` seconds.
    pub fn update(&mut self, dt: f32, skel_anim: &SkeletalAnimation) {
        if self.finished {
            return;
        }
        self.time += dt * self.speed;
        if self.time >= skel_anim.duration {
            if skel_anim.looping {
                self.time %= skel_anim.duration;
            } else {
                self.time = skel_anim.duration;
                self.finished = true;
            }
        }
    }

    /// Sample the pose at the current playback time.
    pub fn sample(
        &self,
        skeleton: &Skeleton,
        skel_anim: &SkeletalAnimation,
    ) -> Vec<BoneTransform> {
        skel_anim.sample(skeleton, self.time)
    }

    /// Play a new animation, resetting if different from current.
    pub fn play(&mut self, name: &str) {
        if self.current_animation != name {
            self.current_animation = name.to_string();
            self.time = 0.0;
            self.finished = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Animation Blending
// ---------------------------------------------------------------------------

/// A node in a blend tree that produces a pose.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlendNode {
    /// Play a single clip by name.
    Clip(String),
    /// Linearly interpolate between two sub-trees.
    Lerp {
        a: Box<BlendNode>,
        b: Box<BlendNode>,
        factor: f32,
    },
    /// Additive blend: layer on top of base.
    Additive {
        base: Box<BlendNode>,
        layer: Box<BlendNode>,
        weight: f32,
    },
}

/// A collection of [`SkeletalAnimation`]s keyed by name.
#[derive(Clone, Debug, Default)]
pub struct SkeletalAnimationLibrary {
    pub animations: FxHashMap<String, SkeletalAnimation>,
}

impl SkeletalAnimationLibrary {
    pub fn new() -> Self {
        Self {
            animations: FxHashMap::default(),
        }
    }

    pub fn add(&mut self, anim: SkeletalAnimation) {
        self.animations.insert(anim.name.clone(), anim);
    }

    pub fn get(&self, name: &str) -> Option<&SkeletalAnimation> {
        self.animations.get(name)
    }
}

/// A blend tree that evaluates a [`BlendNode`] hierarchy to produce a final
/// pose.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlendTree {
    pub root: BlendNode,
}

impl BlendTree {
    /// Recursively evaluate the blend tree, sampling clips from `library` at
    /// the given `time`, producing one [`BoneTransform`] per bone in
    /// `skeleton`.
    pub fn evaluate(
        &self,
        skeleton: &Skeleton,
        library: &SkeletalAnimationLibrary,
        time: f32,
    ) -> Vec<BoneTransform> {
        Self::eval_node(&self.root, skeleton, library, time)
    }

    fn eval_node(
        node: &BlendNode,
        skeleton: &Skeleton,
        library: &SkeletalAnimationLibrary,
        time: f32,
    ) -> Vec<BoneTransform> {
        match node {
            BlendNode::Clip(name) => {
                if let Some(anim) = library.get(name) {
                    anim.sample(skeleton, time)
                } else {
                    tracing::warn!("BlendTree: animation clip '{}' not found", name);
                    vec![BoneTransform::default(); skeleton.bones.len()]
                }
            }
            BlendNode::Lerp { a, b, factor } => {
                let pose_a = Self::eval_node(a, skeleton, library, time);
                let pose_b = Self::eval_node(b, skeleton, library, time);
                blend_poses(&pose_a, &pose_b, *factor)
            }
            BlendNode::Additive { base, layer, weight } => {
                let pose_base = Self::eval_node(base, skeleton, library, time);
                let pose_layer = Self::eval_node(layer, skeleton, library, time);
                additive_blend_poses(&pose_base, &pose_layer, *weight)
            }
        }
    }
}

/// Linearly blend two poses element-wise.
pub fn blend_poses(a: &[BoneTransform], b: &[BoneTransform], t: f32) -> Vec<BoneTransform> {
    let len = a.len().max(b.len());
    (0..len)
        .map(|i| {
            let ta = a.get(i).copied().unwrap_or_default();
            let tb = b.get(i).copied().unwrap_or_default();
            ta.lerp(&tb, t)
        })
        .collect()
}

/// Additive blend: for each bone, add the delta of `layer` (from identity)
/// scaled by `weight` on top of `base`.
pub fn additive_blend_poses(
    base: &[BoneTransform],
    layer: &[BoneTransform],
    weight: f32,
) -> Vec<BoneTransform> {
    let len = base.len().max(layer.len());
    (0..len)
        .map(|i| {
            let b = base.get(i).copied().unwrap_or_default();
            let l = layer.get(i).copied().unwrap_or_default();
            let identity = BoneTransform::default();
            // delta = layer - identity
            BoneTransform {
                position: (
                    b.position.0 + (l.position.0 - identity.position.0) * weight,
                    b.position.1 + (l.position.1 - identity.position.1) * weight,
                ),
                rotation: b.rotation + (l.rotation - identity.rotation) * weight,
                scale: (
                    b.scale.0 + (l.scale.0 - identity.scale.0) * weight,
                    b.scale.1 + (l.scale.1 - identity.scale.1) * weight,
                ),
            }
        })
        .collect()
}

/// Tracks a cross-fade transition between two named animations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimTransition {
    pub from: String,
    pub to: String,
    pub duration: f32,
    pub elapsed: f32,
}

impl AnimTransition {
    pub fn new(from: impl Into<String>, to: impl Into<String>, duration: f32) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            duration,
            elapsed: 0.0,
        }
    }

    /// Advance the transition by `dt`. Returns `true` when the transition is
    /// complete.
    pub fn update(&mut self, dt: f32) -> bool {
        self.elapsed += dt;
        self.elapsed >= self.duration
    }

    /// Current blend factor (0.0 = fully `from`, 1.0 = fully `to`).
    pub fn factor(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        }
    }

    /// Sample the blended pose at the current transition point.
    pub fn sample(
        &self,
        skeleton: &Skeleton,
        library: &SkeletalAnimationLibrary,
        from_time: f32,
        to_time: f32,
    ) -> Vec<BoneTransform> {
        let pose_from = library
            .get(&self.from)
            .map(|a| a.sample(skeleton, from_time))
            .unwrap_or_else(|| vec![BoneTransform::default(); skeleton.bones.len()]);
        let pose_to = library
            .get(&self.to)
            .map(|a| a.sample(skeleton, to_time))
            .unwrap_or_else(|| vec![BoneTransform::default(); skeleton.bones.len()]);
        blend_poses(&pose_from, &pose_to, self.factor())
    }
}

// ---------------------------------------------------------------------------
// Animation Events
// ---------------------------------------------------------------------------

/// Data payload for an [`AnimEvent`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnimEventData {
    /// No extra data.
    None,
    /// A sound effect to play.
    Sound(String),
    /// An entity/prefab to spawn.
    Spawn(String),
    /// Arbitrary user-defined string data.
    Custom(String),
}

/// An event embedded in an animation timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimEvent {
    /// Human-readable event name.
    pub name: String,
    /// Time (in the same units as the animation) at which the event fires.
    pub time: f32,
    /// Optional payload.
    pub data: AnimEventData,
}

/// A track of [`AnimEvent`]s that can be queried by time range.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EventTrack {
    pub events: Vec<AnimEvent>,
}

impl EventTrack {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add an event to the track.
    pub fn add(&mut self, event: AnimEvent) {
        self.events.push(event);
    }

    /// Collect all events whose time falls in the half-open interval
    /// `[from_time, to_time)`.
    ///
    /// When `from_time > to_time` (e.g. animation looped), this collects
    /// events in `[from_time, +inf) ∪ [0, to_time)` which is not handled
    /// here for simplicity — callers should split the range in that case.
    pub fn collect_events(&self, from_time: f32, to_time: f32) -> Vec<&AnimEvent> {
        self.events
            .iter()
            .filter(|e| e.time >= from_time && e.time < to_time)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Sprite sheet animation tests --

    fn make_sprite_animation(frame_count: usize, duration: u32, looping: bool) -> Animation {
        let frames: Vec<AnimFrame> = (0..frame_count)
            .map(|i| AnimFrame {
                uv: Rect::new(i as f32 * 0.25, 0.0, 0.25, 1.0),
                duration,
            })
            .collect();
        Animation {
            name: "test".into(),
            frames,
            looping,
        }
    }

    #[test]
    fn sprite_anim_loop() {
        let anim = make_sprite_animation(3, 2, true);
        let mut player = AnimPlayer::new("test");
        player.play_mode = PlayMode::Loop;

        // Each frame lasts 2 ticks. After 6 ticks we should wrap.
        for _ in 0..6 {
            player.update(&anim);
        }
        // After 6 ticks: frame0(2) + frame1(2) + frame2(2) -> wraps to frame 0
        assert_eq!(player.frame_index, 0);
        assert!(!player.finished);
    }

    #[test]
    fn sprite_anim_once() {
        let anim = make_sprite_animation(2, 1, false);
        let mut player = AnimPlayer::new("test");
        player.play_mode = PlayMode::Once;

        // tick 1: advances past frame0 to frame1
        player.update(&anim);
        assert_eq!(player.frame_index, 1);
        assert!(!player.finished);

        // tick 2: advances past frame1, PlayMode::Once -> finished
        player.update(&anim);
        assert!(player.finished);
        assert_eq!(player.frame_index, 1); // clamped to last frame
    }

    #[test]
    fn sprite_anim_ping_pong() {
        let anim = make_sprite_animation(3, 1, true);
        let mut player = AnimPlayer::new("test");
        player.play_mode = PlayMode::PingPong;

        // tick 1 -> frame 1
        player.update(&anim);
        assert_eq!(player.frame_index, 1);
        // tick 2 -> frame 2
        player.update(&anim);
        assert_eq!(player.frame_index, 2);
        // tick 3 -> past end, ping-pong reverses to frame 1
        player.update(&anim);
        assert_eq!(player.frame_index, 1);
        assert!(!player.finished);
    }

    // -- Skeleton & world transform --

    fn make_test_skeleton() -> (Skeleton, BoneId, BoneId, BoneId) {
        let mut skel = Skeleton::new("test_skel");
        let root = skel.add_bone(
            "root",
            None,
            BoneTransform {
                position: (10.0, 0.0),
                rotation: 0.0,
                scale: (1.0, 1.0),
            },
        );
        let child = skel.add_bone(
            "child",
            Some(root),
            BoneTransform {
                position: (5.0, 0.0),
                rotation: 0.0,
                scale: (1.0, 1.0),
            },
        );
        let grandchild = skel.add_bone(
            "grandchild",
            Some(child),
            BoneTransform {
                position: (3.0, 0.0),
                rotation: 0.0,
                scale: (1.0, 1.0),
            },
        );
        (skel, root, child, grandchild)
    }

    #[test]
    fn skeleton_world_transform_chain() {
        let (skel, root, child, grandchild) = make_test_skeleton();

        let root_world = skel.world_transform(root);
        assert!((root_world.position.0 - 10.0).abs() < 1e-5);
        assert!((root_world.position.1 - 0.0).abs() < 1e-5);

        let child_world = skel.world_transform(child);
        // root(10,0) + child(5,0) = (15, 0)
        assert!((child_world.position.0 - 15.0).abs() < 1e-5);

        let gc_world = skel.world_transform(grandchild);
        // root(10,0) + child(5,0) + gc(3,0) = (18, 0)
        assert!((gc_world.position.0 - 18.0).abs() < 1e-5);
    }

    #[test]
    fn skeleton_world_transform_with_rotation() {
        let mut skel = Skeleton::new("rot_skel");
        let root = skel.add_bone(
            "root",
            None,
            BoneTransform {
                position: (0.0, 0.0),
                rotation: std::f32::consts::FRAC_PI_2, // 90 degrees
                scale: (1.0, 1.0),
            },
        );
        let child = skel.add_bone(
            "child",
            Some(root),
            BoneTransform {
                position: (10.0, 0.0),
                rotation: 0.0,
                scale: (1.0, 1.0),
            },
        );

        let child_world = skel.world_transform(child);
        // Parent rotated 90deg, child at (10,0) local -> (0, 10) world
        assert!((child_world.position.0).abs() < 1e-4);
        assert!((child_world.position.1 - 10.0).abs() < 1e-4);
    }

    #[test]
    fn skeleton_find_bone() {
        let (skel, _root, child, _gc) = make_test_skeleton();
        assert_eq!(skel.find_bone("child"), Some(child));
        assert_eq!(skel.find_bone("nonexistent"), None);
    }

    // -- BoneAnimation keyframe sampling --

    fn make_bone_animation() -> BoneAnimation {
        BoneAnimation {
            bone_id: BoneId(0),
            keyframes: vec![
                BoneKeyframe {
                    time: 0.0,
                    position: (0.0, 0.0),
                    rotation: 0.0,
                    scale: (1.0, 1.0),
                },
                BoneKeyframe {
                    time: 1.0,
                    position: (10.0, 20.0),
                    rotation: 1.0,
                    scale: (2.0, 2.0),
                },
                BoneKeyframe {
                    time: 2.0,
                    position: (20.0, 0.0),
                    rotation: 0.0,
                    scale: (1.0, 1.0),
                },
            ],
        }
    }

    #[test]
    fn bone_animation_sample_at_keyframe() {
        let ba = make_bone_animation();
        let t = ba.sample(0.0);
        assert!((t.position.0).abs() < 1e-5);

        let t = ba.sample(1.0);
        assert!((t.position.0 - 10.0).abs() < 1e-5);
        assert!((t.position.1 - 20.0).abs() < 1e-5);
    }

    #[test]
    fn bone_animation_sample_interpolation() {
        let ba = make_bone_animation();
        let t = ba.sample(0.5);
        // Midpoint between kf0 and kf1
        assert!((t.position.0 - 5.0).abs() < 1e-5);
        assert!((t.position.1 - 10.0).abs() < 1e-5);
        assert!((t.rotation - 0.5).abs() < 1e-5);
        assert!((t.scale.0 - 1.5).abs() < 1e-5);
    }

    #[test]
    fn bone_animation_sample_clamp_before() {
        let ba = make_bone_animation();
        let t = ba.sample(-1.0);
        assert!((t.position.0).abs() < 1e-5);
    }

    #[test]
    fn bone_animation_sample_clamp_after() {
        let ba = make_bone_animation();
        let t = ba.sample(999.0);
        assert!((t.position.0 - 20.0).abs() < 1e-5);
    }

    // -- SkeletalPlayer --

    #[test]
    fn skeletal_player_looping() {
        let skel_anim = SkeletalAnimation {
            name: "walk".into(),
            duration: 1.0,
            bone_anims: vec![],
            looping: true,
        };
        let mut player = SkeletalPlayer::new("walk");
        player.update(0.7, &skel_anim);
        assert!(!player.finished);
        assert!((player.time - 0.7).abs() < 1e-5);

        player.update(0.5, &skel_anim);
        // 1.2 % 1.0 = 0.2
        assert!(!player.finished);
        assert!((player.time - 0.2).abs() < 1e-4);
    }

    #[test]
    fn skeletal_player_once() {
        let skel_anim = SkeletalAnimation {
            name: "attack".into(),
            duration: 0.5,
            bone_anims: vec![],
            looping: false,
        };
        let mut player = SkeletalPlayer::new("attack");
        player.update(0.6, &skel_anim);
        assert!(player.finished);
        assert!((player.time - 0.5).abs() < 1e-5);
    }

    #[test]
    fn skeletal_player_sample() {
        let mut skel = Skeleton::new("s");
        skel.add_bone(
            "b0",
            None,
            BoneTransform {
                position: (0.0, 0.0),
                rotation: 0.0,
                scale: (1.0, 1.0),
            },
        );
        let skel_anim = SkeletalAnimation {
            name: "a".into(),
            duration: 1.0,
            bone_anims: vec![BoneAnimation {
                bone_id: BoneId(0),
                keyframes: vec![
                    BoneKeyframe {
                        time: 0.0,
                        position: (0.0, 0.0),
                        rotation: 0.0,
                        scale: (1.0, 1.0),
                    },
                    BoneKeyframe {
                        time: 1.0,
                        position: (100.0, 0.0),
                        rotation: 0.0,
                        scale: (1.0, 1.0),
                    },
                ],
            }],
            looping: false,
        };

        let mut player = SkeletalPlayer::new("a");
        player.update(0.5, &skel_anim);
        let pose = player.sample(&skel, &skel_anim);
        assert!((pose[0].position.0 - 50.0).abs() < 1e-4);
    }

    // -- AnimTransition blending --

    #[test]
    fn anim_transition_blending() {
        let mut skel = Skeleton::new("s");
        skel.add_bone(
            "b0",
            None,
            BoneTransform::default(),
        );

        let mut lib = SkeletalAnimationLibrary::new();
        lib.add(SkeletalAnimation {
            name: "idle".into(),
            duration: 1.0,
            bone_anims: vec![BoneAnimation {
                bone_id: BoneId(0),
                keyframes: vec![BoneKeyframe {
                    time: 0.0,
                    position: (0.0, 0.0),
                    rotation: 0.0,
                    scale: (1.0, 1.0),
                }],
            }],
            looping: true,
        });
        lib.add(SkeletalAnimation {
            name: "walk".into(),
            duration: 1.0,
            bone_anims: vec![BoneAnimation {
                bone_id: BoneId(0),
                keyframes: vec![BoneKeyframe {
                    time: 0.0,
                    position: (10.0, 0.0),
                    rotation: 0.0,
                    scale: (1.0, 1.0),
                }],
            }],
            looping: true,
        });

        let mut transition = AnimTransition::new("idle", "walk", 1.0);
        // At elapsed=0, factor=0 -> fully idle (0,0)
        let pose = transition.sample(&skel, &lib, 0.0, 0.0);
        assert!((pose[0].position.0).abs() < 1e-5);

        // Advance halfway
        transition.update(0.5);
        assert!((transition.factor() - 0.5).abs() < 1e-5);
        let pose = transition.sample(&skel, &lib, 0.0, 0.0);
        // blend(0, 10, 0.5) = 5
        assert!((pose[0].position.0 - 5.0).abs() < 1e-5);

        // Advance to completion
        let done = transition.update(0.5);
        assert!(done);
        assert!((transition.factor() - 1.0).abs() < 1e-5);
        let pose = transition.sample(&skel, &lib, 0.0, 0.0);
        assert!((pose[0].position.0 - 10.0).abs() < 1e-5);
    }

    // -- EventTrack --

    #[test]
    fn event_track_collect_events() {
        let mut track = EventTrack::new();
        track.add(AnimEvent {
            name: "footstep".into(),
            time: 0.5,
            data: AnimEventData::Sound("step.wav".into()),
        });
        track.add(AnimEvent {
            name: "spawn_vfx".into(),
            time: 1.5,
            data: AnimEventData::Spawn("dust".into()),
        });
        track.add(AnimEvent {
            name: "marker".into(),
            time: 2.0,
            data: AnimEventData::None,
        });

        // Collect in [0.0, 1.0) -> only footstep at 0.5
        let events = track.collect_events(0.0, 1.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "footstep");

        // Collect in [1.0, 2.0) -> only spawn_vfx at 1.5
        let events = track.collect_events(1.0, 2.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "spawn_vfx");

        // Collect in [0.0, 2.5) -> all three (2.0 is included since < 2.5)
        let events = track.collect_events(0.0, 2.5);
        assert_eq!(events.len(), 3);

        // Exact boundary: [2.0, 3.0) includes the event at exactly 2.0
        let events = track.collect_events(2.0, 3.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "marker");

        // Empty range
        let events = track.collect_events(3.0, 4.0);
        assert!(events.is_empty());
    }

    #[test]
    fn event_track_custom_data() {
        let mut track = EventTrack::new();
        track.add(AnimEvent {
            name: "trigger".into(),
            time: 0.0,
            data: AnimEventData::Custom("damage=50".into()),
        });
        let events = track.collect_events(0.0, 1.0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, AnimEventData::Custom("damage=50".into()));
    }

    // -- AnimPlayer with events --

    #[test]
    fn anim_player_collects_events() {
        let anim = Animation {
            name: "attack".into(),
            frames: vec![
                AnimFrame {
                    uv: Rect::new(0.0, 0.0, 1.0, 1.0),
                    duration: 2,
                },
                AnimFrame {
                    uv: Rect::new(0.0, 0.0, 1.0, 1.0),
                    duration: 2,
                },
            ],
            looping: false,
        };

        let mut track = EventTrack::new();
        track.add(AnimEvent {
            name: "hit".into(),
            time: 1.0,
            data: AnimEventData::None,
        });

        let mut player = AnimPlayer::new("attack");
        player.play_mode = PlayMode::Once;

        // tick 0->1: time goes from 0 to 1, event at 1.0 is NOT in [0, 1)
        player.update_with_events(&anim, Some(&track));
        assert!(player.pending_events.is_empty());

        // tick 1->2: time goes from 1 to 2, event at 1.0 IS in [1, 2)
        player.update_with_events(&anim, Some(&track));
        assert_eq!(player.pending_events.len(), 1);
        assert_eq!(player.pending_events[0].name, "hit");
    }

    // -- BlendTree evaluation --

    #[test]
    fn blend_tree_lerp() {
        let mut skel = Skeleton::new("s");
        skel.add_bone("b0", None, BoneTransform::default());

        let mut lib = SkeletalAnimationLibrary::new();
        lib.add(SkeletalAnimation {
            name: "a".into(),
            duration: 1.0,
            bone_anims: vec![BoneAnimation {
                bone_id: BoneId(0),
                keyframes: vec![BoneKeyframe {
                    time: 0.0,
                    position: (0.0, 0.0),
                    rotation: 0.0,
                    scale: (1.0, 1.0),
                }],
            }],
            looping: true,
        });
        lib.add(SkeletalAnimation {
            name: "b".into(),
            duration: 1.0,
            bone_anims: vec![BoneAnimation {
                bone_id: BoneId(0),
                keyframes: vec![BoneKeyframe {
                    time: 0.0,
                    position: (20.0, 0.0),
                    rotation: 2.0,
                    scale: (3.0, 3.0),
                }],
            }],
            looping: true,
        });

        let tree = BlendTree {
            root: BlendNode::Lerp {
                a: Box::new(BlendNode::Clip("a".into())),
                b: Box::new(BlendNode::Clip("b".into())),
                factor: 0.25,
            },
        };
        let pose = tree.evaluate(&skel, &lib, 0.0);
        // lerp(0, 20, 0.25) = 5
        assert!((pose[0].position.0 - 5.0).abs() < 1e-5);
        assert!((pose[0].rotation - 0.5).abs() < 1e-5);
    }
}
