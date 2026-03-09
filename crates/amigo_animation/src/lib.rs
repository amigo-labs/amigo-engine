use amigo_core::Rect;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

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
        if self.finished || animation.frames.is_empty() {
            return;
        }

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
    }

    /// Get the current frame's UV rect.
    pub fn current_uv(&self, animation: &Animation) -> Rect {
        if animation.frames.is_empty() {
            return Rect::new(0.0, 0.0, 1.0, 1.0);
        }
        let idx = self.frame_index.min(animation.frames.len() - 1);
        animation.frames[idx].uv
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
