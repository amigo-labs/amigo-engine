//! Timeline / Cutscene system: keyframe-based sequencing for scripted events.
//!
//! A timeline contains parallel tracks (camera paths, dialogue triggers, sound cues,
//! entity spawns, tween values) synchronized to a shared clock.

use crate::math::{RenderVec2, SimVec2};
use crate::tween::EasingFn;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Keyframe
// ---------------------------------------------------------------------------

/// A single keyframe: a value at a specific time with easing to the next.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyframe<T> {
    /// Time in seconds from timeline start.
    pub time: f32,
    /// Value at this keyframe.
    pub value: T,
    /// Easing to the next keyframe.
    pub easing: EasingFn,
}

// ---------------------------------------------------------------------------
// Track support types
// ---------------------------------------------------------------------------

/// A dialogue trigger at a specific timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueTrigger {
    pub time: f32,
    pub dialogue_key: String,
    pub blocking: bool,
}

/// Animation override value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimOverrideValue {
    pub clip: String,
    pub speed: f32,
}

/// A sound cue at a timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundCue {
    pub time: f32,
    pub sound_name: String,
    pub kind: SoundCueKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SoundCueKind {
    Sfx,
    MusicTransition,
    Stop { fade_seconds: f32 },
}

/// Entity spawn/despawn event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnEvent {
    pub time: f32,
    pub kind: SpawnEventKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SpawnEventKind {
    Spawn {
        template: String,
        position: SimVec2,
        tag: String,
    },
    Despawn {
        tag: String,
    },
}

// ---------------------------------------------------------------------------
// Track
// ---------------------------------------------------------------------------

/// A named track within a timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Track {
    CameraPath {
        name: String,
        points: Vec<Keyframe<RenderVec2>>,
        zoom: Option<Vec<Keyframe<f32>>>,
    },
    Dialogue {
        name: String,
        triggers: Vec<DialogueTrigger>,
    },
    AnimOverride {
        name: String,
        entity_tag: String,
        keyframes: Vec<Keyframe<AnimOverrideValue>>,
    },
    Sound {
        name: String,
        cues: Vec<SoundCue>,
    },
    EntitySpawn {
        name: String,
        events: Vec<SpawnEvent>,
    },
    TweenTrack {
        name: String,
        target_tag: String,
        keyframes: Vec<Keyframe<f32>>,
    },
}

// ---------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------

/// A complete timeline definition, typically loaded from RON.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Timeline {
    pub name: String,
    pub duration: f32,
    pub skippable: bool,
    pub tracks: Vec<Track>,
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

/// Playback state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Playing,
    Paused,
    WaitingForDialogue,
    Finished,
}

/// Events emitted by the timeline player during update.
/// Game code processes these to apply effects to the engine systems.
#[derive(Clone, Debug)]
pub enum TimelineEvent {
    /// Camera should move to this position.
    CameraMove {
        position: RenderVec2,
        zoom: Option<f32>,
    },
    /// Play a sound effect.
    PlaySfx { name: String },
    /// Transition music to a new section.
    MusicTransition { name: String },
    /// Stop a sound with fade.
    StopSound { fade_seconds: f32 },
    /// Start a dialogue sequence.
    StartDialogue { key: String, blocking: bool },
    /// Spawn an entity.
    SpawnEntity {
        template: String,
        position: SimVec2,
        tag: String,
    },
    /// Despawn an entity by tag.
    DespawnEntity { tag: String },
    /// Apply a tween value to a named target.
    TweenValue { target_tag: String, value: f32 },
    /// Override an entity's animation.
    AnimOverride {
        entity_tag: String,
        clip: String,
        speed: f32,
    },
}

/// Runtime timeline player.
pub struct TimelinePlayer {
    timeline: Option<Timeline>,
    current_time: f32,
    speed: f32,
    state: PlaybackState,
    fired_events: FxHashSet<(usize, usize)>,
    entity_tags: FxHashMap<String, crate::ecs::EntityId>,
}

impl TimelinePlayer {
    pub fn new() -> Self {
        Self {
            timeline: None,
            current_time: 0.0,
            speed: 1.0,
            state: PlaybackState::Idle,
            fired_events: FxHashSet::default(),
            entity_tags: FxHashMap::default(),
        }
    }

    pub fn play(&mut self, timeline: Timeline) {
        self.timeline = Some(timeline);
        self.current_time = 0.0;
        self.speed = 1.0;
        self.state = PlaybackState::Playing;
        self.fired_events.clear();
        self.entity_tags.clear();
    }

    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == PlaybackState::Paused {
            self.state = PlaybackState::Playing;
        }
    }

    pub fn stop(&mut self) {
        self.timeline = None;
        self.current_time = 0.0;
        self.state = PlaybackState::Idle;
        self.fired_events.clear();
        self.entity_tags.clear();
    }

    pub fn seek(&mut self, time: f32) {
        let old_time = self.current_time;
        self.current_time = time.max(0.0);
        // If seeking backward, remove fired events after the new time
        if time < old_time {
            self.fired_events.retain(|&(track_idx, event_idx)| {
                if let Some(tl) = &self.timeline {
                    if let Some(track) = tl.tracks.get(track_idx) {
                        let event_time = get_event_time(track, event_idx);
                        return event_time.map_or(true, |t| t <= time);
                    }
                }
                true
            });
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.max(0.0);
    }

    pub fn skip(&mut self) -> bool {
        let tl = match &self.timeline {
            Some(tl) if tl.skippable => tl,
            _ => return false,
        };
        self.current_time = tl.duration;
        self.state = PlaybackState::Finished;
        true
    }

    /// Advance playback. Returns events that occurred this frame.
    pub fn update(&mut self, dt: f32) -> (PlaybackState, Vec<TimelineEvent>) {
        let mut events = Vec::new();

        if self.state != PlaybackState::Playing {
            return (self.state, events);
        }

        let old_time = self.current_time;
        self.current_time += dt * self.speed;

        let tl = match &self.timeline {
            Some(tl) => tl,
            None => return (self.state, events),
        };

        if self.current_time >= tl.duration {
            self.current_time = tl.duration;
            self.state = PlaybackState::Finished;
        }

        // Evaluate all tracks
        for (track_idx, track) in tl.tracks.iter().enumerate() {
            match track {
                Track::CameraPath { points, zoom, .. } => {
                    if let Some(pos) = interpolate_vec2_keyframes(points, self.current_time) {
                        let z = zoom
                            .as_ref()
                            .and_then(|zk| interpolate_f32_keyframes(zk, self.current_time));
                        events.push(TimelineEvent::CameraMove {
                            position: pos,
                            zoom: z,
                        });
                    }
                }

                Track::TweenTrack {
                    target_tag,
                    keyframes,
                    ..
                } => {
                    if let Some(val) = interpolate_f32_keyframes(keyframes, self.current_time) {
                        events.push(TimelineEvent::TweenValue {
                            target_tag: target_tag.clone(),
                            value: val,
                        });
                    }
                }

                Track::Dialogue { triggers, .. } => {
                    for (event_idx, trigger) in triggers.iter().enumerate() {
                        if trigger.time > old_time
                            && trigger.time <= self.current_time
                            && self.fired_events.insert((track_idx, event_idx))
                        {
                            events.push(TimelineEvent::StartDialogue {
                                key: trigger.dialogue_key.clone(),
                                blocking: trigger.blocking,
                            });
                            if trigger.blocking {
                                self.state = PlaybackState::WaitingForDialogue;
                            }
                        }
                    }
                }

                Track::Sound { cues, .. } => {
                    for (event_idx, cue) in cues.iter().enumerate() {
                        if cue.time > old_time
                            && cue.time <= self.current_time
                            && self.fired_events.insert((track_idx, event_idx))
                        {
                            match &cue.kind {
                                SoundCueKind::Sfx => {
                                    events.push(TimelineEvent::PlaySfx {
                                        name: cue.sound_name.clone(),
                                    });
                                }
                                SoundCueKind::MusicTransition => {
                                    events.push(TimelineEvent::MusicTransition {
                                        name: cue.sound_name.clone(),
                                    });
                                }
                                SoundCueKind::Stop { fade_seconds } => {
                                    events.push(TimelineEvent::StopSound {
                                        fade_seconds: *fade_seconds,
                                    });
                                }
                            }
                        }
                    }
                }

                Track::EntitySpawn {
                    events: spawn_events,
                    ..
                } => {
                    for (event_idx, ev) in spawn_events.iter().enumerate() {
                        if ev.time > old_time
                            && ev.time <= self.current_time
                            && self.fired_events.insert((track_idx, event_idx))
                        {
                            match &ev.kind {
                                SpawnEventKind::Spawn {
                                    template,
                                    position,
                                    tag,
                                } => {
                                    events.push(TimelineEvent::SpawnEntity {
                                        template: template.clone(),
                                        position: *position,
                                        tag: tag.clone(),
                                    });
                                }
                                SpawnEventKind::Despawn { tag } => {
                                    events.push(TimelineEvent::DespawnEntity { tag: tag.clone() });
                                }
                            }
                        }
                    }
                }

                Track::AnimOverride {
                    entity_tag,
                    keyframes,
                    ..
                } => {
                    if let Some(val) = interpolate_anim_keyframes(keyframes, self.current_time) {
                        events.push(TimelineEvent::AnimOverride {
                            entity_tag: entity_tag.clone(),
                            clip: val.clip,
                            speed: val.speed,
                        });
                    }
                }
            }
        }

        (self.state, events)
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn current_time(&self) -> f32 {
        self.current_time
    }

    pub fn progress(&self) -> f32 {
        match &self.timeline {
            Some(tl) if tl.duration > 0.0 => (self.current_time / tl.duration).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    pub fn on_dialogue_complete(&mut self) {
        if self.state == PlaybackState::WaitingForDialogue {
            self.state = PlaybackState::Playing;
        }
    }
}

impl Default for TimelinePlayer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Interpolation helpers
// ---------------------------------------------------------------------------

fn interpolate_f32_keyframes(keyframes: &[Keyframe<f32>], time: f32) -> Option<f32> {
    if keyframes.is_empty() {
        return None;
    }
    if time <= keyframes[0].time {
        return Some(keyframes[0].value);
    }
    if time >= keyframes[keyframes.len() - 1].time {
        return Some(keyframes[keyframes.len() - 1].value);
    }
    // Binary search for surrounding keyframes
    for i in 0..keyframes.len() - 1 {
        let a = &keyframes[i];
        let b = &keyframes[i + 1];
        if time >= a.time && time < b.time {
            let t = (time - a.time) / (b.time - a.time);
            let eased = a.easing.apply(t);
            return Some(a.value + (b.value - a.value) * eased);
        }
    }
    Some(keyframes[keyframes.len() - 1].value)
}

fn interpolate_vec2_keyframes(keyframes: &[Keyframe<RenderVec2>], time: f32) -> Option<RenderVec2> {
    if keyframes.is_empty() {
        return None;
    }
    if time <= keyframes[0].time {
        return Some(keyframes[0].value);
    }
    if time >= keyframes[keyframes.len() - 1].time {
        return Some(keyframes[keyframes.len() - 1].value);
    }
    for i in 0..keyframes.len() - 1 {
        let a = &keyframes[i];
        let b = &keyframes[i + 1];
        if time >= a.time && time < b.time {
            let t = (time - a.time) / (b.time - a.time);
            let eased = a.easing.apply(t);
            return Some(RenderVec2::new(
                a.value.x + (b.value.x - a.value.x) * eased,
                a.value.y + (b.value.y - a.value.y) * eased,
            ));
        }
    }
    Some(keyframes[keyframes.len() - 1].value)
}

fn interpolate_anim_keyframes(
    keyframes: &[Keyframe<AnimOverrideValue>],
    time: f32,
) -> Option<AnimOverrideValue> {
    if keyframes.is_empty() {
        return None;
    }
    // AnimOverride is discrete (no interpolation between clips), use the latest keyframe
    let mut best = &keyframes[0];
    for kf in keyframes {
        if kf.time <= time {
            best = kf;
        }
    }
    Some(best.value.clone())
}

fn get_event_time(track: &Track, event_idx: usize) -> Option<f32> {
    match track {
        Track::Dialogue { triggers, .. } => triggers.get(event_idx).map(|t| t.time),
        Track::Sound { cues, .. } => cues.get(event_idx).map(|c| c.time),
        Track::EntitySpawn { events, .. } => events.get(event_idx).map(|e| e.time),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_timeline() -> Timeline {
        Timeline {
            name: "test".into(),
            duration: 5.0,
            skippable: true,
            tracks: vec![
                Track::TweenTrack {
                    name: "fade".into(),
                    target_tag: "screen_alpha".into(),
                    keyframes: vec![
                        Keyframe {
                            time: 0.0,
                            value: 0.0,
                            easing: EasingFn::Linear,
                        },
                        Keyframe {
                            time: 2.0,
                            value: 1.0,
                            easing: EasingFn::Linear,
                        },
                        Keyframe {
                            time: 5.0,
                            value: 0.0,
                            easing: EasingFn::Linear,
                        },
                    ],
                },
                Track::Sound {
                    name: "sfx".into(),
                    cues: vec![SoundCue {
                        time: 1.0,
                        sound_name: "explosion".into(),
                        kind: SoundCueKind::Sfx,
                    }],
                },
                Track::Dialogue {
                    name: "dialog".into(),
                    triggers: vec![DialogueTrigger {
                        time: 3.0,
                        dialogue_key: "intro".into(),
                        blocking: true,
                    }],
                },
            ],
        }
    }

    #[test]
    fn basic_playback() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        assert_eq!(player.state(), PlaybackState::Playing);

        let (state, events) = player.update(1.5);
        assert_eq!(state, PlaybackState::Playing);
        // Should have tween value + sfx event (sfx at 1.0 crossed)
        assert!(events
            .iter()
            .any(|e| matches!(e, TimelineEvent::TweenValue { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, TimelineEvent::PlaySfx { name } if name == "explosion")));
    }

    #[test]
    fn events_fire_once() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        player.update(1.5); // Fires sfx
        let (_, events) = player.update(0.5); // Advances to 2.0, sfx already fired
        assert!(!events
            .iter()
            .any(|e| matches!(e, TimelineEvent::PlaySfx { .. })));
    }

    #[test]
    fn blocking_dialogue() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        player.update(3.5); // Crosses dialogue at 3.0
        assert_eq!(player.state(), PlaybackState::WaitingForDialogue);

        // Advancing does nothing while waiting
        let (state, _) = player.update(1.0);
        assert_eq!(state, PlaybackState::WaitingForDialogue);

        // Complete dialogue
        player.on_dialogue_complete();
        assert_eq!(player.state(), PlaybackState::Playing);
    }

    #[test]
    fn skip() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        assert!(player.skip());
        assert_eq!(player.state(), PlaybackState::Finished);
        assert!((player.current_time() - 5.0).abs() < 0.01);
    }

    #[test]
    fn finishes_at_duration() {
        // Use a timeline without blocking dialogue
        let tl = Timeline {
            name: "simple".into(),
            duration: 2.0,
            skippable: true,
            tracks: vec![Track::TweenTrack {
                name: "fade".into(),
                target_tag: "alpha".into(),
                keyframes: vec![
                    Keyframe {
                        time: 0.0,
                        value: 0.0,
                        easing: EasingFn::Linear,
                    },
                    Keyframe {
                        time: 2.0,
                        value: 1.0,
                        easing: EasingFn::Linear,
                    },
                ],
            }],
        };
        let mut player = TimelinePlayer::new();
        player.play(tl);
        player.update(3.0); // Exceeds duration
        assert_eq!(player.state(), PlaybackState::Finished);
        assert!((player.current_time() - 2.0).abs() < 0.01);
    }

    #[test]
    fn pause_resume() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        player.update(1.0);
        player.pause();
        let t = player.current_time();
        player.update(1.0); // Should not advance
        assert!((player.current_time() - t).abs() < 0.01);
        player.resume();
        player.update(1.0);
        assert!(player.current_time() > t);
    }

    #[test]
    fn tween_interpolation() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        let (_, events) = player.update(1.0); // At t=1.0, tween should be 0.5
        let tween_event = events.iter().find_map(|e| match e {
            TimelineEvent::TweenValue { value, .. } => Some(*value),
            _ => None,
        });
        assert!(tween_event.is_some());
        assert!((tween_event.unwrap() - 0.5).abs() < 0.01);
    }

    #[test]
    fn speed_control() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        player.set_speed(2.0);
        player.update(1.0); // Should advance 2.0 seconds
        assert!((player.current_time() - 2.0).abs() < 0.01);
    }

    #[test]
    fn seek_backward_clears_fired() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        player.update(1.5); // Fires sfx at 1.0
        player.seek(0.5); // Seek back before sfx
        let (_, events) = player.update(1.0); // Should re-fire sfx
        assert!(events
            .iter()
            .any(|e| matches!(e, TimelineEvent::PlaySfx { .. })));
    }

    #[test]
    fn progress_fraction() {
        let mut player = TimelinePlayer::new();
        player.play(simple_timeline());
        player.update(2.5);
        assert!((player.progress() - 0.5).abs() < 0.01);
    }
}
