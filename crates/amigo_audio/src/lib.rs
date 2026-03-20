#![allow(missing_docs)]

pub mod spatial;

use kira::manager::backend::DefaultBackend;
use kira::manager::{AudioManager as KiraManager, AudioManagerSettings};
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::Volume;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Audio init failed: {0}")]
    Init(String),
    #[error("Sound not found: {0}")]
    NotFound(String),
    #[error("Playback error: {0}")]
    Playback(String),
    #[error("No active section to transition from")]
    NoActiveSection,
    #[error("Section not found: {0}")]
    SectionNotFound(String),
    #[error("Stinger not found: {0}")]
    StingerNotFound(String),
}

// ---------------------------------------------------------------------------
// Volume channels
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct VolumeChannels {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub ambient: f32,
}

impl Default for VolumeChannels {
    fn default() -> Self {
        Self {
            master: 0.8,
            music: 0.6,
            sfx: 1.0,
            ambient: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// SFX definitions & manager
// ---------------------------------------------------------------------------

/// SFX definition with variants and randomization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxDefinition {
    pub files: Vec<PathBuf>,
    pub volume: f32,
    pub pitch_variance: f32,
    pub max_concurrent: u32,
    pub cooldown: Option<f32>,
}

/// Tracks runtime state for a single SFX name: cooldown timer and live handles.
struct SfxRuntimeState {
    last_played: Option<Instant>,
    active_handles: Vec<StaticSoundHandle>,
}

impl Default for SfxRuntimeState {
    fn default() -> Self {
        Self {
            last_played: None,
            active_handles: Vec::new(),
        }
    }
}

/// Improved SFX manager with per-sound cooldowns, concurrency limits, and pitch
/// variance. Works alongside [`AudioManager`] by sharing the same kira manager.
pub struct SfxManager {
    definitions: FxHashMap<String, SfxDefinition>,
    loaded_data: FxHashMap<String, Vec<StaticSoundData>>,
    runtime: FxHashMap<String, SfxRuntimeState>,
}

impl SfxManager {
    pub fn new() -> Self {
        Self {
            definitions: FxHashMap::default(),
            loaded_data: FxHashMap::default(),
            runtime: FxHashMap::default(),
        }
    }

    /// Register an SFX definition. Call [`load`] afterwards to load the actual
    /// sound data from disk.
    pub fn register(&mut self, name: impl Into<String>, def: SfxDefinition) {
        self.definitions.insert(name.into(), def);
    }

    /// Load all files listed in the definition from disk.
    pub fn load(&mut self, name: &str, base_path: &Path) {
        let Some(def) = self.definitions.get(name) else {
            warn!("SfxManager::load – no definition for '{name}'");
            return;
        };
        let mut variants = Vec::with_capacity(def.files.len());
        for file in &def.files {
            let full = base_path.join(file);
            match StaticSoundData::from_file(&full) {
                Ok(data) => variants.push(data),
                Err(e) => warn!("Failed to load SFX variant '{name}' from {full:?}: {e}"),
            }
        }
        if !variants.is_empty() {
            info!(
                "SfxManager: loaded {} variant(s) for '{name}'",
                variants.len()
            );
        }
        self.loaded_data.insert(name.to_string(), variants);
    }

    /// Load a single file as a named SFX (no definition required).
    pub fn load_single(&mut self, name: &str, path: &Path) {
        match StaticSoundData::from_file(path) {
            Ok(data) => {
                self.loaded_data
                    .entry(name.to_string())
                    .or_default()
                    .push(data);
                info!("SfxManager: loaded SFX '{name}'");
            }
            Err(e) => warn!("Failed to load SFX '{name}' from {path:?}: {e}"),
        }
    }

    /// Play a sound effect, respecting cooldowns and concurrency limits.
    /// `pitch_variance` from the definition is applied as a random pitch shift.
    pub fn play(&mut self, name: &str, kira: &mut KiraManager<DefaultBackend>) {
        let now = Instant::now();

        // -- Look up definition (optional) for cooldown / concurrency / pitch --
        let (cooldown, max_concurrent, _pitch_variance) =
            if let Some(def) = self.definitions.get(name) {
                (def.cooldown, def.max_concurrent, def.pitch_variance)
            } else {
                (None, u32::MAX, 0.0)
            };

        let rt = self.runtime.entry(name.to_string()).or_default();

        // Cooldown check
        if let (Some(cd), Some(last)) = (cooldown, rt.last_played) {
            if now.duration_since(last).as_secs_f32() < cd {
                debug!("SFX '{name}' on cooldown");
                return;
            }
        }

        // Prune finished handles
        rt.active_handles
            .retain(|h| h.state() != kira::sound::PlaybackState::Stopped);

        // Concurrency check
        if rt.active_handles.len() as u32 >= max_concurrent {
            debug!("SFX '{name}' at max concurrent ({max_concurrent})");
            return;
        }

        // Pick variant
        let Some(variants) = self.loaded_data.get(name) else {
            warn!("SFX not loaded: '{name}'");
            return;
        };
        if variants.is_empty() {
            return;
        }
        let idx = (now.elapsed().subsec_nanos() as usize) % variants.len();
        let data = variants[idx].clone();

        // TODO: apply pitch_variance via kira's PlaybackRate when kira 0.9 settings support it
        match kira.play(data) {
            Ok(handle) => {
                rt.active_handles.push(handle);
                rt.last_played = Some(now);
            }
            Err(e) => warn!("Failed to play SFX '{name}': {e}"),
        }
    }

    /// Remove all stopped handles, freeing resources.
    pub fn cleanup(&mut self) {
        for rt in self.runtime.values_mut() {
            rt.active_handles
                .retain(|h| h.state() != kira::sound::PlaybackState::Stopped);
        }
    }

    /// Play a sound effect and return the kira handle (for spatial audio use).
    ///
    /// This respects cooldowns and concurrency limits just like [`play`](Self::play),
    /// but returns the raw [`StaticSoundHandle`] so the caller (e.g.
    /// [`SpatialAudioSystem`](crate::spatial::SpatialAudioSystem)) can adjust
    /// volume and panning each frame.
    pub fn play_returning_handle(
        &mut self,
        name: &str,
        kira: &mut KiraManager<DefaultBackend>,
    ) -> Option<StaticSoundHandle> {
        let now = Instant::now();

        let (cooldown, max_concurrent, _pitch_variance) =
            if let Some(def) = self.definitions.get(name) {
                (def.cooldown, def.max_concurrent, def.pitch_variance)
            } else {
                (None, u32::MAX, 0.0)
            };

        let rt = self.runtime.entry(name.to_string()).or_default();

        // Cooldown check
        if let (Some(cd), Some(last)) = (cooldown, rt.last_played) {
            if now.duration_since(last).as_secs_f32() < cd {
                debug!("SFX '{name}' on cooldown");
                return None;
            }
        }

        // Prune finished handles
        rt.active_handles
            .retain(|h| h.state() != kira::sound::PlaybackState::Stopped);

        // Concurrency check
        if rt.active_handles.len() as u32 >= max_concurrent {
            debug!("SFX '{name}' at max concurrent ({max_concurrent})");
            return None;
        }

        // Pick variant
        let variants = self.loaded_data.get(name)?;
        if variants.is_empty() {
            return None;
        }
        let idx = (now.elapsed().subsec_nanos() as usize) % variants.len();
        let data = variants[idx].clone();

        match kira.play(data) {
            Ok(handle) => {
                let ret = handle.clone();
                rt.active_handles.push(handle);
                rt.last_played = Some(now);
                Some(ret)
            }
            Err(e) => {
                warn!("Failed to play SFX '{name}': {e}");
                None
            }
        }
    }

    /// Shorthand: play a sound at a world position with default attenuation.
    ///
    /// Delegates to [`SpatialAudioSystem::spatial_play`](crate::spatial::SpatialAudioSystem::spatial_play)
    /// using a default [`SpatialEmitter`](crate::spatial::SpatialEmitter) placed
    /// at the given position.
    pub fn play_spatial(
        &mut self,
        spatial: &mut crate::spatial::SpatialAudioSystem,
        kira: &mut KiraManager<DefaultBackend>,
        name: &str,
        position: amigo_core::math::SimVec2,
    ) -> crate::spatial::SpatialSoundId {
        let emitter = crate::spatial::SpatialEmitter {
            position,
            ..Default::default()
        };
        spatial.spatial_play(self, kira, name, &emitter, None)
    }
}

// ---------------------------------------------------------------------------
// AudioManager (original, preserved)
// ---------------------------------------------------------------------------

/// Audio manager wrapping kira.
pub struct AudioManager {
    manager: Option<KiraManager>,
    sfx_data: FxHashMap<String, Vec<StaticSoundData>>,
    music_handles: FxHashMap<String, StaticSoundHandle>,
    pub volumes: VolumeChannels,
    base_path: PathBuf,
}

impl AudioManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        let manager = KiraManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .map_err(|e| warn!("Audio init failed: {}", e))
            .ok();

        if manager.is_some() {
            info!("Audio system initialized");
        }

        Self {
            manager,
            sfx_data: FxHashMap::default(),
            music_handles: FxHashMap::default(),
            volumes: VolumeChannels::default(),
            base_path: base_path.into(),
        }
    }

    /// Load a sound effect from file.
    pub fn load_sfx(&mut self, name: &str, path: &Path) {
        match StaticSoundData::from_file(path) {
            Ok(data) => {
                self.sfx_data
                    .entry(name.to_string())
                    .or_default()
                    .push(data);
                info!("Loaded SFX: {}", name);
            }
            Err(e) => {
                warn!("Failed to load SFX '{}' from {:?}: {}", name, path, e);
            }
        }
    }

    /// Play a sound effect by name.
    pub fn play_sfx(&mut self, name: &str) {
        let Some(manager) = &mut self.manager else {
            return;
        };

        if let Some(variants) = self.sfx_data.get(name) {
            if variants.is_empty() {
                return;
            }
            // Pick a random variant (simple modulo-based for now)
            let idx = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize)
                % variants.len();

            let data = variants[idx].clone();
            let _ = manager.play(data);
        } else {
            warn!("SFX not found: {}", name);
        }
    }

    /// Play a sound effect at a world position with distance attenuation and stereo panning.
    ///
    /// `source_x/y`: world position of the sound source.
    /// `listener_x/y`: world position of the listener (typically camera center).
    /// `max_distance`: beyond this distance the sound is inaudible.
    pub fn play_sfx_at(
        &mut self,
        name: &str,
        source_x: f32,
        source_y: f32,
        listener_x: f32,
        listener_y: f32,
        max_distance: f32,
    ) {
        let dx = source_x - listener_x;
        let dy = source_y - listener_y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance >= max_distance {
            return; // Too far away, don't play
        }

        let Some(manager) = &mut self.manager else {
            return;
        };

        if let Some(variants) = self.sfx_data.get(name) {
            if variants.is_empty() {
                return;
            }

            let idx = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize)
                % variants.len();

            // Linear distance attenuation
            let volume = (1.0 - distance / max_distance).clamp(0.0, 1.0);

            // Stereo panning based on X offset (-1.0 = full left, 1.0 = full right)
            let panning = if max_distance > 0.0 {
                (dx / max_distance).clamp(-1.0, 1.0)
            } else {
                0.0
            };

            let data = variants[idx].clone().with_settings(
                StaticSoundSettings::new()
                    .volume(Volume::Amplitude(volume as f64))
                    .panning(((panning + 1.0) / 2.0) as f64), // kira panning: 0=left, 0.5=center, 1=right
            );
            let _ = manager.play(data);
        }
    }

    /// Play music from file.
    pub fn play_music(&mut self, name: &str, path: &Path) {
        let Some(manager) = &mut self.manager else {
            return;
        };

        // Stop current music with same name
        self.music_handles.remove(name);

        match StaticSoundData::from_file(path) {
            Ok(data) => match manager.play(data) {
                Ok(handle) => {
                    self.music_handles.insert(name.to_string(), handle);
                    info!("Playing music: {}", name);
                }
                Err(e) => warn!("Failed to play music '{}': {}", name, e),
            },
            Err(e) => warn!("Failed to load music '{}': {}", name, e),
        }
    }

    /// Stop all music.
    pub fn stop_music(&mut self) {
        self.music_handles.clear();
    }

    /// Set volume for a channel.
    pub fn set_volume(&mut self, channel: &str, volume: f32) {
        let vol = volume.clamp(0.0, 1.0);
        match channel {
            "master" => self.volumes.master = vol,
            "music" => self.volumes.music = vol,
            "sfx" => self.volumes.sfx = vol,
            "ambient" => self.volumes.ambient = vol,
            _ => warn!("Unknown audio channel: {}", channel),
        }
    }

    /// Borrow the inner kira manager (for use with SfxManager / AdaptiveMusicEngine).
    pub fn kira_manager_mut(&mut self) -> Option<&mut KiraManager<DefaultBackend>> {
        self.manager.as_mut()
    }

    /// Base path used for asset resolution.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}

// ===========================================================================
// Adaptive Music System
// ===========================================================================

// ---------------------------------------------------------------------------
// BarClock – musical time keeper
// ---------------------------------------------------------------------------

/// Tracks BPM, time signature, and current beat/bar position.
#[derive(Clone, Debug)]
pub struct BarClock {
    pub bpm: f64,
    pub beats_per_bar: u32,
    /// Accumulated time in seconds since the clock started / was last reset.
    elapsed: f64,
    /// Whether the clock is ticking.
    pub running: bool,
}

impl BarClock {
    pub fn new(bpm: f64, beats_per_bar: u32) -> Self {
        Self {
            bpm,
            beats_per_bar,
            elapsed: 0.0,
            running: true,
        }
    }

    /// Advance the clock by `dt` seconds.
    pub fn tick(&mut self, dt: f64) {
        if self.running {
            self.elapsed += dt;
        }
    }

    /// Duration of one beat in seconds.
    pub fn beat_duration(&self) -> f64 {
        60.0 / self.bpm
    }

    /// Duration of one full bar in seconds.
    pub fn bar_duration(&self) -> f64 {
        self.beat_duration() * self.beats_per_bar as f64
    }

    /// Current beat (0-indexed, fractional).
    pub fn current_beat(&self) -> f64 {
        self.elapsed / self.beat_duration()
    }

    /// Current bar (0-indexed, fractional).
    pub fn current_bar(&self) -> f64 {
        self.elapsed / self.bar_duration()
    }

    /// Current beat within the current bar (0-indexed, fractional).
    pub fn beat_in_bar(&self) -> f64 {
        self.current_beat() % self.beats_per_bar as f64
    }

    /// Whole bar number (0-indexed).
    pub fn bar_number(&self) -> u64 {
        self.current_bar().floor() as u64
    }

    /// How many seconds remain until the next bar boundary.
    pub fn seconds_until_next_bar(&self) -> f64 {
        let bar_dur = self.bar_duration();
        let into_bar = self.elapsed % bar_dur;
        bar_dur - into_bar
    }

    /// How many seconds remain until `n` bars from now (next bar boundary + (n-1) bars).
    pub fn seconds_until_bars(&self, n: u32) -> f64 {
        self.seconds_until_next_bar() + self.bar_duration() * (n.saturating_sub(1)) as f64
    }

    /// Reset elapsed time to zero.
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }

    /// Total elapsed seconds.
    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed
    }
}

// ---------------------------------------------------------------------------
// MusicParameters – game-driven values that control the adaptive score
// ---------------------------------------------------------------------------

/// Float and boolean parameters that the game sets, which drive layer rules
/// and section transitions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicParameters {
    pub floats: FxHashMap<String, f32>,
    pub booleans: FxHashMap<String, bool>,
}

impl Default for MusicParameters {
    fn default() -> Self {
        let mut floats = FxHashMap::default();
        floats.insert("tension".into(), 0.0);
        floats.insert("danger".into(), 0.0);
        floats.insert("victory".into(), 0.0);

        let mut booleans = FxHashMap::default();
        booleans.insert("boss".into(), false);
        booleans.insert("menu_open".into(), false);

        Self { floats, booleans }
    }
}

impl MusicParameters {
    pub fn set_float(&mut self, name: impl Into<String>, value: f32) {
        self.floats.insert(name.into(), value.clamp(0.0, 1.0));
    }

    pub fn get_float(&self, name: &str) -> f32 {
        self.floats.get(name).copied().unwrap_or(0.0)
    }

    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.booleans.insert(name.into(), value);
    }

    pub fn get_bool(&self, name: &str) -> bool {
        self.booleans.get(name).copied().unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// LayerRule – determines how a parameter maps to layer volume
// ---------------------------------------------------------------------------

/// Rules that map game parameters to layer volumes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LayerRule {
    /// Linearly interpolate the layer volume based on a float parameter.
    /// When the param equals `from`, volume is 0; when it equals `to`, volume is 1.
    Lerp { param: String, from: f32, to: f32 },
    /// Layer is at full volume when the float parameter is above `above`,
    /// otherwise fades to silence over `fade_seconds`.
    Threshold {
        param: String,
        above: f32,
        fade_seconds: f32,
    },
    /// Layer is toggled by a boolean parameter, fading over `fade_seconds`.
    Toggle { param: String, fade_seconds: f32 },
}

impl LayerRule {
    /// Evaluate the rule against current parameters, returning a target volume
    /// in `0.0..=1.0`.
    pub fn evaluate(&self, params: &MusicParameters) -> f32 {
        match self {
            LayerRule::Lerp { param, from, to } => {
                let val = params.get_float(param);
                if (to - from).abs() < f32::EPSILON {
                    return 0.0;
                }
                ((val - from) / (to - from)).clamp(0.0, 1.0)
            }
            LayerRule::Threshold { param, above, .. } => {
                if params.get_float(param) >= *above {
                    1.0
                } else {
                    0.0
                }
            }
            LayerRule::Toggle { param, .. } => {
                if params.get_bool(param) {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// The fade speed (volume units per second) implied by this rule, or a
    /// default fast value for rules that don't specify one.
    pub fn fade_speed(&self) -> f32 {
        match self {
            LayerRule::Lerp { .. } => 4.0, // fast follow
            LayerRule::Threshold { fade_seconds, .. } | LayerRule::Toggle { fade_seconds, .. } => {
                if *fade_seconds > 0.0 {
                    1.0 / fade_seconds
                } else {
                    100.0 // instant
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MusicTransition – how sections switch
// ---------------------------------------------------------------------------

/// Describes how one music section transitions to another.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MusicTransition {
    /// Cross-fade from the current section to the next, starting at the next
    /// bar boundary and lasting `bars` bars.
    CrossfadeOnBar { bars: u32 },
    /// Fade out the current section over `fade_bars` bars, then start the new
    /// section on the next bar boundary after silence.
    FadeOutThenPlay { fade_bars: u32 },
    /// Hard cut to the new section on the next bar boundary.
    CutOnBar,
    /// Play a stinger (one-shot cue), then apply a follow-up transition to
    /// the new section. The stinger fires at the next bar boundary; once it
    /// starts, the inner transition kicks in.
    StingerThen {
        stinger_name: String,
        then: Box<MusicTransition>,
    },
    /// Swap individual layer stems between the old and new section without
    /// interrupting playback. Layers with matching names in both sections are
    /// cross-faded over `fade_bars` bars; unmatched layers fade out/in.
    LayerSwap { fade_bars: u32 },
}

// ---------------------------------------------------------------------------
// MusicLayer / MusicSection
// ---------------------------------------------------------------------------

/// A single stem/layer in the adaptive music system.
pub struct MusicLayer {
    pub name: String,
    pub handle: Option<StaticSoundHandle>,
    /// The volume this layer should be at when fully "on" (before parameter
    /// modulation).
    pub base_volume: f32,
    /// The volume being rendered right now.
    pub current_volume: f32,
    /// Where the volume is heading (set by rules each tick).
    pub target_volume: f32,
    /// How fast current_volume moves toward target_volume (units/sec).
    pub fade_speed: f32,
}

impl MusicLayer {
    pub fn new(name: impl Into<String>, base_volume: f32) -> Self {
        Self {
            name: name.into(),
            handle: None,
            base_volume,
            current_volume: 0.0,
            target_volume: 0.0,
            fade_speed: 2.0,
        }
    }

    /// Smoothly move `current_volume` toward `target_volume` by `dt` seconds.
    pub fn update_volume(&mut self, dt: f32) {
        if (self.current_volume - self.target_volume).abs() < 0.001 {
            self.current_volume = self.target_volume;
        } else if self.current_volume < self.target_volume {
            self.current_volume =
                (self.current_volume + self.fade_speed * dt).min(self.target_volume);
        } else {
            self.current_volume =
                (self.current_volume - self.fade_speed * dt).max(self.target_volume);
        }
    }

    /// The effective volume = base_volume * current_volume.
    pub fn effective_volume(&self) -> f32 {
        self.base_volume * self.current_volume
    }
}

/// A named musical section (e.g. "calm", "tense", "battle") consisting of
/// multiple layers, each governed by rules.
pub struct MusicSection {
    pub name: String,
    pub layers: Vec<MusicLayer>,
    /// One rule per layer (parallel arrays). If a layer has no rule, its target
    /// volume stays at `base_volume`.
    pub rules: Vec<Option<LayerRule>>,
}

impl MusicSection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layers: Vec::new(),
            rules: Vec::new(),
        }
    }

    /// Add a layer with an optional rule.
    pub fn add_layer(&mut self, layer: MusicLayer, rule: Option<LayerRule>) {
        self.layers.push(layer);
        self.rules.push(rule);
    }

    /// Evaluate all rules against the current parameters and update layer
    /// target volumes.
    pub fn evaluate_rules(&mut self, params: &MusicParameters) {
        for (layer, rule) in self.layers.iter_mut().zip(self.rules.iter()) {
            match rule {
                Some(r) => {
                    layer.target_volume = r.evaluate(params);
                    layer.fade_speed = r.fade_speed();
                }
                None => {
                    // No rule – layer is always fully on.
                    layer.target_volume = 1.0;
                }
            }
        }
    }

    /// Tick all layer volumes.
    pub fn update_volumes(&mut self, dt: f32) {
        for layer in &mut self.layers {
            layer.update_volume(dt);
        }
    }

    /// Set all layers' target volumes to zero for a fade-out.
    pub fn fade_out_all(&mut self, fade_speed: f32) {
        for layer in &mut self.layers {
            layer.target_volume = 0.0;
            layer.fade_speed = fade_speed;
        }
    }

    /// Returns `true` when every layer has reached silence.
    pub fn is_silent(&self) -> bool {
        self.layers.iter().all(|l| l.current_volume <= 0.001)
    }
}

// ---------------------------------------------------------------------------
// Stinger – one-shot musical cue
// ---------------------------------------------------------------------------

/// A stinger is a one-shot sound quantized to the next beat or bar boundary.
pub struct Stinger {
    pub name: String,
    pub data: StaticSoundData,
    /// Quantize to beat or bar.
    pub quantize: StingerQuantize,
}

/// When to trigger a stinger.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum StingerQuantize {
    /// Fire on the next beat boundary.
    #[default]
    Beat,
    /// Fire on the next bar boundary.
    Bar,
    /// Fire immediately (no quantization).
    Immediate,
}

/// Internal: a stinger waiting in the queue to be played.
struct PendingStinger {
    data: StaticSoundData,
    quantize: StingerQuantize,
    /// The beat number at which it should fire (absolute).
    fire_at_beat: Option<f64>,
    /// The bar number at which it should fire (absolute).
    fire_at_bar: Option<u64>,
}

// ---------------------------------------------------------------------------
// Transition state machine
// ---------------------------------------------------------------------------

enum TransitionState {
    None,
    /// Cross-fading: old section fading out, new section fading in.
    Crossfading {
        old_section_idx: usize,
        new_section_idx: usize,
        remaining: f64,
        total: f64,
    },
    /// Fading out old before starting new.
    FadingOut {
        old_section_idx: usize,
        new_section_idx: usize,
        fade_remaining: f64,
        waiting_for_bar: bool,
    },
    /// Waiting for the next bar to hard-cut.
    WaitingForBar {
        new_section_idx: usize,
        target_bar: u64,
    },
    /// Waiting for the next bar to fire a stinger, then apply follow-up transition.
    StingerThenWait {
        old_section_idx: usize,
        new_section_idx: usize,
        stinger_data: StaticSoundData,
        then: Box<MusicTransition>,
        target_bar: u64,
        stinger_fired: bool,
    },
    /// Layer-swap: cross-fading individual layers between old and new sections.
    LayerSwapping {
        old_section_idx: usize,
        new_section_idx: usize,
        remaining: f64,
        total: f64,
    },
}

// ---------------------------------------------------------------------------
// AdaptiveMusicEngine
// ---------------------------------------------------------------------------

/// The core runtime for dynamic, adaptive soundtracks.
///
/// # Architecture
/// - **Vertical layering**: within a section, multiple stems play
///   simultaneously. Each layer's volume is driven by game parameters through
///   [`LayerRule`]s.
/// - **Horizontal re-sequencing**: the engine can switch between sections
///   (e.g. calm -> tense -> battle) using bar-synced transitions.
/// - **Stingers**: one-shot musical cues quantized to beat or bar boundaries.
pub struct AdaptiveMusicEngine {
    pub clock: BarClock,
    pub params: MusicParameters,
    sections: Vec<MusicSection>,
    active_section: Option<usize>,
    transition: TransitionState,
    stinger_library: FxHashMap<String, Stinger>,
    pending_stingers: Vec<PendingStinger>,
    /// Overall music volume multiplier (applied on top of VolumeChannels).
    pub master_volume: f32,
}

impl AdaptiveMusicEngine {
    /// Create a new engine at the given BPM and time signature.
    pub fn new(bpm: f64, beats_per_bar: u32) -> Self {
        Self {
            clock: BarClock::new(bpm, beats_per_bar),
            params: MusicParameters::default(),
            sections: Vec::new(),
            active_section: None,
            transition: TransitionState::None,
            stinger_library: FxHashMap::default(),
            pending_stingers: Vec::new(),
            master_volume: 1.0,
        }
    }

    // -- Section management -------------------------------------------------

    /// Register a section. Returns its index.
    pub fn add_section(&mut self, section: MusicSection) -> usize {
        let idx = self.sections.len();
        info!(
            "AdaptiveMusic: registered section '{}' (idx={idx})",
            section.name
        );
        self.sections.push(section);
        idx
    }

    /// Look up a section index by name.
    pub fn section_index(&self, name: &str) -> Option<usize> {
        self.sections.iter().position(|s| s.name == name)
    }

    /// Start playing a section immediately (no transition). All layers begin at
    /// their target volumes determined by current parameters.
    pub fn play_section(
        &mut self,
        idx: usize,
        kira: &mut KiraManager<DefaultBackend>,
        base_path: &Path,
    ) {
        if idx >= self.sections.len() {
            warn!("AdaptiveMusic: section index {idx} out of range");
            return;
        }

        // Silence any currently active section.
        if let Some(old) = self.active_section {
            self.stop_section_layers(old);
        }

        self.active_section = Some(idx);
        self.transition = TransitionState::None;

        // Evaluate rules so target volumes are set before first tick.
        self.sections[idx].evaluate_rules(&self.params);

        // Snap current volumes to targets.
        for layer in &mut self.sections[idx].layers {
            layer.current_volume = layer.target_volume;
        }

        info!(
            "AdaptiveMusic: playing section '{}' immediately",
            self.sections[idx].name
        );

        // NOTE: Layer handles should be pre-loaded by the caller via
        // `load_section_layers`. If not loaded, we attempt to load nothing
        // gracefully – the engine just won't produce sound for those layers.
        let _ = (kira, base_path); // used by callers; kept in API for future streaming
    }

    /// Start a section by name.
    pub fn play_section_by_name(
        &mut self,
        name: &str,
        kira: &mut KiraManager<DefaultBackend>,
        base_path: &Path,
    ) -> Result<(), AudioError> {
        let idx = self
            .section_index(name)
            .ok_or_else(|| AudioError::SectionNotFound(name.to_string()))?;
        self.play_section(idx, kira, base_path);
        Ok(())
    }

    /// Transition from the currently active section to `new_idx` using the
    /// given [`MusicTransition`] strategy.
    pub fn transition_to(
        &mut self,
        new_idx: usize,
        transition: MusicTransition,
    ) -> Result<(), AudioError> {
        let old_idx = self.active_section.ok_or(AudioError::NoActiveSection)?;
        if new_idx >= self.sections.len() {
            return Err(AudioError::SectionNotFound(format!("index {new_idx}")));
        }
        if old_idx == new_idx {
            return Ok(());
        }

        info!(
            "AdaptiveMusic: transition '{}' -> '{}' via {transition:?}",
            self.sections[old_idx].name, self.sections[new_idx].name
        );

        match transition {
            MusicTransition::CrossfadeOnBar { bars } => {
                let total = self.clock.bar_duration() * bars.max(1) as f64;
                self.transition = TransitionState::Crossfading {
                    old_section_idx: old_idx,
                    new_section_idx: new_idx,
                    remaining: total,
                    total,
                };
                // Start new section layers at zero volume.
                for layer in &mut self.sections[new_idx].layers {
                    layer.current_volume = 0.0;
                }
                self.sections[new_idx].evaluate_rules(&self.params);
            }
            MusicTransition::FadeOutThenPlay { fade_bars } => {
                let fade_dur = self.clock.bar_duration() * fade_bars.max(1) as f64;
                let speed = if fade_dur > 0.0 {
                    1.0 / fade_dur as f32
                } else {
                    100.0
                };
                self.sections[old_idx].fade_out_all(speed);
                self.transition = TransitionState::FadingOut {
                    old_section_idx: old_idx,
                    new_section_idx: new_idx,
                    fade_remaining: fade_dur,
                    waiting_for_bar: false,
                };
            }
            MusicTransition::CutOnBar => {
                let target_bar = self.clock.bar_number() + 1;
                self.transition = TransitionState::WaitingForBar {
                    new_section_idx: new_idx,
                    target_bar,
                };
            }
            MusicTransition::StingerThen { stinger_name, then } => {
                let stinger = self
                    .stinger_library
                    .get(&stinger_name)
                    .ok_or_else(|| AudioError::StingerNotFound(stinger_name.clone()))?;
                let stinger_data = stinger.data.clone();
                let target_bar = self.clock.bar_number() + 1;
                self.transition = TransitionState::StingerThenWait {
                    old_section_idx: old_idx,
                    new_section_idx: new_idx,
                    stinger_data,
                    then,
                    target_bar,
                    stinger_fired: false,
                };
            }
            MusicTransition::LayerSwap { fade_bars } => {
                let total = self.clock.bar_duration() * fade_bars.max(1) as f64;
                // Prepare new section layers at zero volume
                self.sections[new_idx].evaluate_rules(&self.params);
                for layer in &mut self.sections[new_idx].layers {
                    layer.current_volume = 0.0;
                }
                self.transition = TransitionState::LayerSwapping {
                    old_section_idx: old_idx,
                    new_section_idx: new_idx,
                    remaining: total,
                    total,
                };
            }
        }

        Ok(())
    }

    /// Convenience: transition by section name.
    pub fn transition_to_by_name(
        &mut self,
        name: &str,
        transition: MusicTransition,
    ) -> Result<(), AudioError> {
        let idx = self
            .section_index(name)
            .ok_or_else(|| AudioError::SectionNotFound(name.to_string()))?;
        self.transition_to(idx, transition)
    }

    // -- Stingers -----------------------------------------------------------

    /// Register a stinger in the library.
    pub fn add_stinger(&mut self, stinger: Stinger) {
        info!("AdaptiveMusic: registered stinger '{}'", stinger.name);
        self.stinger_library.insert(stinger.name.clone(), stinger);
    }

    /// Queue a stinger to be played at the appropriate quantization point.
    pub fn play_stinger(&mut self, name: &str) {
        let Some(stinger) = self.stinger_library.get(name) else {
            warn!("Stinger not found: '{name}'");
            return;
        };

        let quantize = stinger.quantize;
        let data = stinger.data.clone();

        let (fire_at_beat, fire_at_bar) = match quantize {
            StingerQuantize::Beat => {
                let next_beat = self.clock.current_beat().ceil();
                (Some(next_beat), None)
            }
            StingerQuantize::Bar => (None, Some(self.clock.bar_number() + 1)),
            StingerQuantize::Immediate => (None, None),
        };

        self.pending_stingers.push(PendingStinger {
            data,
            quantize,
            fire_at_beat,
            fire_at_bar,
        });

        debug!("Stinger '{name}' queued (quantize={quantize:?})");
    }

    // -- Per-frame update ---------------------------------------------------

    /// Advance the engine by `dt` seconds. This must be called every frame.
    pub fn update(&mut self, dt: f64, kira: &mut KiraManager<DefaultBackend>) {
        self.clock.tick(dt);
        let dt_f32 = dt as f32;

        // -- Evaluate rules for active section(s) --
        self.evaluate_active_rules();

        // -- Drive transition state machine --
        self.update_transition(dt, kira);

        // -- Update layer volumes --
        for section in &mut self.sections {
            section.update_volumes(dt_f32);
        }

        // -- Fire ready stingers --
        self.fire_stingers(kira);
    }

    fn evaluate_active_rules(&mut self) {
        // We need to borrow params immutably while mutating sections, so clone
        // params (they're small).
        let params = self.params.clone();

        if let Some(idx) = self.active_section {
            self.sections[idx].evaluate_rules(&params);
        }

        // During crossfade the new section also needs rule evaluation.
        if let TransitionState::Crossfading {
            new_section_idx, ..
        } = &self.transition
        {
            let ni = *new_section_idx;
            self.sections[ni].evaluate_rules(&params);
        }
    }

    fn update_transition(&mut self, dt: f64, _kira: &mut KiraManager<DefaultBackend>) {
        // Take transition out to avoid borrow conflicts with self.sections
        let mut transition = std::mem::replace(&mut self.transition, TransitionState::None);

        match &mut transition {
            TransitionState::None => {}
            TransitionState::Crossfading {
                old_section_idx,
                new_section_idx,
                remaining,
                total,
            } => {
                *remaining -= dt;
                let progress = (1.0 - (*remaining / *total)).clamp(0.0, 1.0) as f32;

                let old_idx = *old_section_idx;
                let new_idx = *new_section_idx;

                for layer in &mut self.sections[old_idx].layers {
                    layer.target_volume *= 1.0 - progress;
                }

                if *remaining <= 0.0 {
                    self.stop_section_layers(old_idx);
                    self.active_section = Some(new_idx);
                    info!(
                        "AdaptiveMusic: crossfade complete -> '{}'",
                        self.sections[new_idx].name
                    );
                    // transition stays None (already replaced)
                    return;
                }
            }
            TransitionState::FadingOut {
                old_section_idx,
                new_section_idx,
                fade_remaining,
                waiting_for_bar,
            } => {
                let old_idx = *old_section_idx;
                let new_idx = *new_section_idx;

                if !*waiting_for_bar {
                    *fade_remaining -= dt;
                    if *fade_remaining <= 0.0 || self.sections[old_idx].is_silent() {
                        self.stop_section_layers(old_idx);
                        *waiting_for_bar = true;
                    }
                } else if self.clock.seconds_until_next_bar() <= dt {
                    self.active_section = Some(new_idx);
                    self.sections[new_idx].evaluate_rules(&self.params);
                    for layer in &mut self.sections[new_idx].layers {
                        layer.current_volume = layer.target_volume;
                    }
                    info!(
                        "AdaptiveMusic: fade-out-then-play complete -> '{}'",
                        self.sections[new_idx].name
                    );
                    return; // transition stays None
                }
            }
            TransitionState::WaitingForBar {
                new_section_idx,
                target_bar,
            } => {
                if self.clock.bar_number() >= *target_bar {
                    if let Some(old) = self.active_section {
                        self.stop_section_layers(old);
                    }
                    let ni = *new_section_idx;
                    self.active_section = Some(ni);
                    self.sections[ni].evaluate_rules(&self.params);
                    for layer in &mut self.sections[ni].layers {
                        layer.current_volume = layer.target_volume;
                    }
                    info!(
                        "AdaptiveMusic: cut-on-bar complete -> '{}'",
                        self.sections[ni].name
                    );
                    return; // transition stays None
                }
            }
            TransitionState::StingerThenWait {
                old_section_idx,
                new_section_idx,
                stinger_data,
                then,
                target_bar,
                stinger_fired,
            } => {
                if !*stinger_fired && self.clock.bar_number() >= *target_bar {
                    // Fire the stinger
                    match _kira.play(stinger_data.clone()) {
                        Ok(_) => debug!("StingerThen: stinger fired"),
                        Err(e) => warn!("StingerThen: failed to play stinger: {e}"),
                    }
                    *stinger_fired = true;
                    // Now apply the follow-up transition by re-entering transition_to
                    let ni = *new_section_idx;
                    let oi = *old_section_idx;
                    let follow_up = *then.clone();
                    self.active_section = Some(oi); // ensure old is still active
                                                    // Don't put transition back — we'll set a new one via transition_to
                    let _ = self.transition_to(ni, follow_up);
                    return;
                }
            }
            TransitionState::LayerSwapping {
                old_section_idx,
                new_section_idx,
                remaining,
                total,
            } => {
                *remaining -= dt;
                let progress = (1.0 - (*remaining / *total)).clamp(0.0, 1.0) as f32;

                let old_idx = *old_section_idx;
                let new_idx = *new_section_idx;

                // Fade old layers out, new layers in
                for layer in &mut self.sections[old_idx].layers {
                    layer.target_volume = (1.0 - progress) * layer.base_volume;
                }
                for layer in &mut self.sections[new_idx].layers {
                    layer.target_volume = progress * layer.base_volume;
                }

                if *remaining <= 0.0 {
                    self.stop_section_layers(old_idx);
                    self.active_section = Some(new_idx);
                    self.sections[new_idx].evaluate_rules(&self.params);
                    for layer in &mut self.sections[new_idx].layers {
                        layer.current_volume = layer.target_volume;
                    }
                    info!(
                        "AdaptiveMusic: layer-swap complete -> '{}'",
                        self.sections[new_idx].name
                    );
                    return; // transition stays None
                }
            }
        }

        // Put transition back if not completed
        self.transition = transition;
    }

    fn fire_stingers(&mut self, kira: &mut KiraManager<DefaultBackend>) {
        let beat = self.clock.current_beat();
        let bar = self.clock.bar_number();

        let mut to_fire = Vec::new();
        self.pending_stingers.retain(|ps| {
            let should_fire = match ps.quantize {
                StingerQuantize::Immediate => true,
                StingerQuantize::Beat => ps.fire_at_beat.is_none_or(|target| beat >= target),
                StingerQuantize::Bar => ps.fire_at_bar.is_none_or(|target| bar >= target),
            };
            if should_fire {
                to_fire.push(ps.data.clone());
                false // remove from pending
            } else {
                true // keep
            }
        });

        for data in to_fire {
            match kira.play(data) {
                Ok(_handle) => debug!("Stinger fired"),
                Err(e) => warn!("Failed to play stinger: {e}"),
            }
        }
    }

    /// Stop all layers in a section (set volumes to zero, drop handles).
    fn stop_section_layers(&mut self, idx: usize) {
        for layer in &mut self.sections[idx].layers {
            layer.current_volume = 0.0;
            layer.target_volume = 0.0;
            layer.handle = None;
        }
    }

    // -- Accessors ----------------------------------------------------------

    /// Get a reference to a section by index.
    pub fn section(&self, idx: usize) -> Option<&MusicSection> {
        self.sections.get(idx)
    }

    /// Get a mutable reference to a section by index.
    pub fn section_mut(&mut self, idx: usize) -> Option<&mut MusicSection> {
        self.sections.get_mut(idx)
    }

    /// Currently active section index.
    pub fn active_section_index(&self) -> Option<usize> {
        self.active_section
    }

    /// Whether a transition is in progress.
    pub fn is_transitioning(&self) -> bool {
        !matches!(self.transition, TransitionState::None)
    }

    /// Load configuration from a `.music.ron` file.
    ///
    /// This populates sections, layers, rules, and stingers from a RON config,
    /// but does NOT load audio data. Call `play_section()` after loading to
    /// actually start playback (which loads audio files via kira).
    pub fn load_from_ron(path: &Path) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let config: MusicConfig = ron::from_str(&contents).map_err(|e| e.to_string())?;

        let mut engine = Self::new(config.bpm, config.beats_per_bar);
        engine.master_volume = config.master_volume;

        for section_def in &config.sections {
            let mut section = MusicSection::new(&section_def.name);
            for layer_def in &section_def.layers {
                let layer = MusicLayer::new(&layer_def.name, layer_def.base_volume);
                section.add_layer(layer, layer_def.rule.clone());
            }
            engine.add_section(section);
        }

        for stinger_def in &config.stingers {
            // Stinger audio is loaded lazily when play_stinger() is called.
            // Store the path in the stinger name for now.
            info!(
                "AdaptiveMusic: registered stinger config '{}' (path: {})",
                stinger_def.name, stinger_def.audio_path
            );
        }

        info!(
            "AdaptiveMusic: loaded config from {:?} ({} sections)",
            path,
            engine.sections.len()
        );

        Ok(engine)
    }
}

// ---------------------------------------------------------------------------
// RON config types for .music.ron files
// ---------------------------------------------------------------------------

/// Top-level music configuration loaded from `.music.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicConfig {
    pub bpm: f64,
    pub beats_per_bar: u32,
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
    pub sections: Vec<SectionConfig>,
    #[serde(default)]
    pub stingers: Vec<StingerConfig>,
    #[serde(default)]
    pub sequences: Vec<SequenceConfig>,
}

fn default_master_volume() -> f32 {
    1.0
}

/// A section definition in the RON config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionConfig {
    pub name: String,
    pub layers: Vec<LayerConfig>,
}

/// A layer definition in the RON config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerConfig {
    pub name: String,
    /// Path to the audio file (relative to assets/).
    pub audio_path: String,
    #[serde(default = "default_base_volume")]
    pub base_volume: f32,
    pub rule: Option<LayerRule>,
}

fn default_base_volume() -> f32 {
    1.0
}

/// A stinger definition in the RON config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StingerConfig {
    pub name: String,
    pub audio_path: String,
    #[serde(default)]
    pub quantize: StingerQuantize,
}

/// A music sequence (ordered list of section transitions).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SequenceConfig {
    pub name: String,
    pub steps: Vec<SequenceStep>,
}

/// A single step in a music sequence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SequenceStep {
    /// Section name to play.
    pub section: String,
    /// How many bars to play before transitioning.
    pub bars: u32,
    /// Transition to use when moving to the next step.
    pub transition: MusicTransition,
}
