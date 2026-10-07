//! [`AudioManager`]: what a game reaches as `ctx.audio`. Handles to playing
//! sounds, start control, buses under a master, pause, loading from bytes and
//! looping music, on top of kira (docs/specs/engine/audio-playback.md).

use crate::playback::{
    Bus, Fade, LoopMode, PlaySettings, Repeat, ResolvedRange, SoundHandle, SoundState,
    finite_loop_len, render_finite_loop, resolve_range,
};
use crate::{AudioError, VolumeChannels, amplitude_to_decibels};
use kira::backend::mock::{MockBackend, MockBackendSettings};
use kira::sound::PlaybackState;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{
    AudioManager as KiraManager, AudioManagerSettings, Decibels, DefaultBackend, Panning,
    PlaybackRate, StartTime, Tween,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// The longest finite loop rendered into one buffer, in seconds of audio.
const MAX_RENDERED_SECONDS: f64 = 120.0;

/// The kira manager on a real output device, or on the mock backend.
enum Kira {
    Real(Box<KiraManager<DefaultBackend>>),
    Mock(Box<KiraManager<MockBackend>>),
}

impl Kira {
    fn add_bus(&mut self, volume: Decibels) -> Option<TrackHandle> {
        let builder = TrackBuilder::new().volume(volume);
        let result = match self {
            Kira::Real(m) => m.add_sub_track(builder).map_err(|e| e.to_string()),
            Kira::Mock(m) => m.add_sub_track(builder).map_err(|e| e.to_string()),
        };
        result
            .map_err(|e| warn!("Could not create an audio bus: {e}"))
            .ok()
    }

    fn set_master(&mut self, volume: Decibels, tween: Tween) {
        match self {
            Kira::Real(m) => m.main_track().set_volume(volume, tween),
            Kira::Mock(m) => m.main_track().set_volume(volume, tween),
        }
    }

    /// Let the mock renderer process its queued commands. A no-op on a real
    /// device.
    #[cfg(test)]
    fn process(&mut self) {
        if let Kira::Mock(m) = self {
            m.backend_mut().on_start_processing();
            m.backend_mut().process();
        }
    }
}

struct Device {
    kira: Kira,
    /// Music, sfx and ambient, by `Bus::index`.
    buses: Vec<TrackHandle>,
}

/// A sound started with [`AudioManager::play`].
struct LiveSound {
    handle: StaticSoundHandle,
    name: String,
    bus: Bus,
    volume: f32,
    /// When `play` was called.
    played_at: Instant,
    /// When the delay ends.
    starts_at: Instant,
    /// Stopped before it started: reported `Stopped` at once.
    cancelled: bool,
}

struct Slot {
    generation: u32,
    sound: Option<LiveSound>,
}

/// What a rendered buffer was built from.
#[derive(Clone, PartialEq, Eq, Hash)]
struct RenderKey {
    name: String,
    variant: usize,
    range: ResolvedRange,
}

/// Audio manager wrapping kira.
///
/// The output device is not opened by [`Self::new`] but by
/// [`Self::open_device`], which the engine calls at startup, or else by the
/// first call that needs it. Code that only builds a [`AudioManager`], such as
/// a test constructing a game context, never touches the audio backend.
/// [`Self::new_silent`] runs on kira's mock backend instead: every call works
/// and nothing is heard.
pub struct AudioManager {
    device: Option<Device>,
    /// Whether [`Self::open_device`] has run. A missing device is not retried
    /// on every sound, which would put a failing device probe in the frame.
    device_probed: bool,
    sounds: FxHashMap<String, Vec<StaticSoundData>>,
    name_volumes: FxHashMap<String, f32>,
    slots: Vec<Slot>,
    free_slots: Vec<u32>,
    rendered: FxHashMap<RenderKey, StaticSoundData>,
    warned: FxHashSet<(&'static str, String)>,
    /// Master and bus volumes. A game may write them directly; the next
    /// [`maintain`](Self::maintain) applies the change.
    pub volumes: VolumeChannels,
    applied: VolumeChannels,
    muted: [bool; 3],
    paused_at: Option<Instant>,
    music: Option<(String, SoundHandle)>,
    rng: u64,
    base_path: PathBuf,
}

impl AudioManager {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self {
            device: None,
            device_probed: false,
            sounds: FxHashMap::default(),
            name_volumes: FxHashMap::default(),
            slots: Vec::new(),
            free_slots: Vec::new(),
            rendered: FxHashMap::default(),
            warned: FxHashSet::default(),
            volumes: VolumeChannels::default(),
            applied: VolumeChannels::default(),
            muted: [false; 3],
            paused_at: None,
            music: None,
            rng: seed | 1,
            base_path: base_path.into(),
        }
    }

    /// A manager on kira's mock backend: every call works, nothing is heard.
    /// For tests and for headless mode.
    pub fn new_silent(base_path: impl Into<PathBuf>) -> Self {
        let mut audio = Self::new(base_path);
        audio.device_probed = true;
        let settings = AudioManagerSettings {
            backend_settings: MockBackendSettings {
                sample_rate: 48_000,
            },
            ..Default::default()
        };
        match KiraManager::<MockBackend>::new(settings) {
            Ok(manager) => audio.attach(Kira::Mock(Box::new(manager))),
            Err(()) => warn!("The silent audio backend failed to start"),
        }
        audio
    }

    /// Open the audio output device, if that has not been tried yet, and
    /// report whether one is open.
    ///
    /// Only the first call probes the backend; when it fails, sounds are
    /// silently skipped from then on. Call this at startup so the first sound
    /// does not pay for opening the device mid-frame.
    pub fn open_device(&mut self) -> bool {
        if !self.device_probed {
            self.device_probed = true;
            match KiraManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
                Ok(manager) => {
                    self.attach(Kira::Real(Box::new(manager)));
                    info!("Audio system initialized");
                }
                Err(e) => warn!("Audio init failed: {e}"),
            }
        }
        self.device.is_some()
    }

    /// Set up the buses on a freshly opened manager.
    fn attach(&mut self, mut kira: Kira) {
        kira.set_master(amplitude_to_decibels(self.volumes.master), Tween::default());
        let mut buses = Vec::with_capacity(3);
        for bus in Bus::ALL {
            match kira.add_bus(self.bus_decibels(bus)) {
                Some(track) => buses.push(track),
                None => return,
            }
        }
        self.device = Some(Device { kira, buses });
        self.applied = self.volumes.clone();
    }

    /// Borrow the inner kira manager (for use with SfxManager /
    /// AdaptiveMusicEngine). `None` without a real device, including on the
    /// silent backend.
    pub fn kira_manager_mut(&mut self) -> Option<&mut KiraManager<DefaultBackend>> {
        self.open_device();
        match self.device.as_mut().map(|d| &mut d.kira) {
            Some(Kira::Real(manager)) => Some(manager),
            _ => None,
        }
    }

    /// Base path used for asset resolution.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    fn warn_once(&mut self, what: &'static str, name: &str, message: impl FnOnce() -> String) {
        if self.warned.insert((what, name.to_string())) {
            warn!("{}", message());
        }
    }

    // -----------------------------------------------------------------------
    // A5: Loading
    // -----------------------------------------------------------------------

    /// Decode a file (wav, ogg, mp3, flac) and register it under `name`.
    /// Loading a second file under the same name adds a variant; `play` picks
    /// one variant at random.
    pub fn load_sound(&mut self, name: &str, path: &Path) -> Result<(), AudioError> {
        let data = StaticSoundData::from_file(path).map_err(|e| AudioError::Load {
            name: name.to_string(),
            message: format!("{}: {e}", path.display()),
        })?;
        self.add_variant(name, data);
        Ok(())
    }

    /// Same, from encoded bytes already in memory (a pak entry, a download).
    pub fn load_sound_from_bytes(
        &mut self,
        name: &str,
        bytes: Arc<[u8]>,
    ) -> Result<(), AudioError> {
        let data = StaticSoundData::from_cursor(std::io::Cursor::new(bytes)).map_err(|e| {
            AudioError::Load {
                name: name.to_string(),
                message: e.to_string(),
            }
        })?;
        self.add_variant(name, data);
        Ok(())
    }

    fn add_variant(&mut self, name: &str, data: StaticSoundData) {
        self.sounds.entry(name.to_string()).or_default().push(data);
        self.warned.retain(|(_, n)| n != name);
    }

    /// Load a sound effect from file, logging a failure instead of returning
    /// it.
    pub fn load_sfx(&mut self, name: &str, path: &Path) {
        match self.load_sound(name, path) {
            Ok(()) => info!("Loaded SFX: {}", name),
            Err(e) => warn!("{e}"),
        }
    }

    /// Forget `name` and all its variants. Playing instances stop.
    pub fn unload(&mut self, name: &str) {
        self.stop_all(name, Fade::NONE);
        self.sounds.remove(name);
        self.rendered.retain(|key, _| key.name != name);
    }

    pub fn is_loaded(&self, name: &str) -> bool {
        self.sounds.get(name).is_some_and(|v| !v.is_empty())
    }

    /// Length of the first variant in seconds.
    pub fn sound_duration(&self, name: &str) -> Option<f64> {
        let data = self.sounds.get(name)?.first()?;
        Some(data.frames.len() as f64 / data.sample_rate.max(1) as f64)
    }

    /// Sample rate of the first variant, the unit of `Position::Samples`.
    pub fn sound_sample_rate(&self, name: &str) -> Option<u32> {
        Some(self.sounds.get(name)?.first()?.sample_rate)
    }

    // -----------------------------------------------------------------------
    // A1/A2: Playing
    // -----------------------------------------------------------------------

    /// Start a loaded sound. Always returns a handle. When the sound cannot
    /// start (no audio device, unknown name, empty range, kira's sound limit),
    /// the handle is already `Stopped`.
    pub fn play(&mut self, name: &str, settings: &PlaySettings) -> SoundHandle {
        self.open_device();
        if self.device.is_none() {
            return SoundHandle::NONE;
        }
        let count = self.sounds.get(name).map_or(0, Vec::len);
        if count == 0 {
            self.warn_once("unknown", name, || format!("Sound '{name}' is not loaded"));
            return SoundHandle::NONE;
        }
        let variant = (self.next_random() % count as u64) as usize;
        let source = self.sounds[name][variant].clone();

        let Some(range) = resolve_range(settings, source.sample_rate, source.frames.len()) else {
            self.warn_once("range", name, || {
                format!("Sound '{name}': the requested range is empty, nothing plays")
            });
            return SoundHandle::NONE;
        };
        if range.empty_region {
            self.warn_once("region", name, || {
                format!("Sound '{name}': the loop region is empty, the sound plays once")
            });
        }
        let Some(mut data) = self.buffer_for(name, variant, &source, range) else {
            return SoundHandle::NONE;
        };

        let name_volume = self.name_volume(name);
        let volume = clamp_volume(settings.volume).unwrap_or(1.0);
        let delay = if settings.delay.is_finite() && settings.delay > 0.0 {
            settings.delay
        } else {
            0.0
        };
        let rate = if settings.playback_rate.is_finite() {
            settings.playback_rate.clamp(0.01, 8.0)
        } else {
            1.0
        };
        let panning = if settings.panning.is_finite() {
            settings.panning.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let mut sound_settings = StaticSoundSettings::new()
            .volume(amplitude_to_decibels(volume * name_volume))
            .playback_rate(PlaybackRate(rate))
            .panning(Panning(panning));
        if delay > 0.0 {
            sound_settings =
                sound_settings.start_time(StartTime::Delayed(Duration::from_secs_f64(delay)));
        }
        if settings.fade_in.seconds > 0.0 {
            sound_settings = sound_settings.fade_in_tween(settings.fade_in.tween());
        }
        if range.repeat == Repeat::Forever
            && let Some((rs, re)) = range.region
        {
            let base = range.start;
            sound_settings = sound_settings.loop_region(
                kira::sound::PlaybackPosition::Samples(rs - base)
                    ..kira::sound::PlaybackPosition::Samples(re - base),
            );
        }
        data.settings = sound_settings;

        let Some(device) = self.device.as_mut() else {
            return SoundHandle::NONE;
        };
        let handle = match device.buses[settings.bus.index()].play(data) {
            Ok(handle) => handle,
            Err(e) => {
                self.warn_once("limit", name, || {
                    format!("Sound '{name}' could not start: {e}")
                });
                return SoundHandle::NONE;
            }
        };
        self.insert(LiveSound {
            handle,
            name: name.to_string(),
            bus: settings.bus,
            volume,
            played_at: Instant::now(),
            starts_at: Instant::now() + Duration::from_secs_f64(delay),
            cancelled: false,
        })
    }

    /// The buffer to play for `range` of `source`: the file itself, the
    /// played sub-range, or a rendered finite loop. Cached.
    fn buffer_for(
        &mut self,
        name: &str,
        variant: usize,
        source: &StaticSoundData,
        range: ResolvedRange,
    ) -> Option<StaticSoundData> {
        let whole = range.start == 0 && range.end == source.frames.len();
        if !matches!(range.repeat, Repeat::Count(_)) && whole {
            return Some(StaticSoundData {
                sample_rate: source.sample_rate,
                frames: source.frames.clone(),
                settings: StaticSoundSettings::new(),
                slice: None,
            });
        }
        let key = RenderKey {
            name: name.to_string(),
            variant,
            range,
        };
        if let Some(data) = self.rendered.get(&key) {
            return Some(data.clone());
        }
        let frames: Arc<[kira::Frame]> = match range.repeat {
            Repeat::Count(n) => {
                let seconds = finite_loop_len(range, n) as f64 / source.sample_rate.max(1) as f64;
                if seconds > MAX_RENDERED_SECONDS {
                    self.warn_once("long", name, || {
                        format!(
                            "Sound '{name}': {n} repeats make {seconds:.0} s of audio, more than \
                             {MAX_RENDERED_SECONDS:.0} s; use LoopMode::Forever and stop it"
                        )
                    });
                    return None;
                }
                render_finite_loop(&source.frames, range, n)
            }
            Repeat::Once | Repeat::Forever => source.frames[range.start..range.end].into(),
        };
        let data = StaticSoundData {
            sample_rate: source.sample_rate,
            frames,
            settings: StaticSoundSettings::new(),
            slice: None,
        };
        self.rendered.insert(key, data.clone());
        Some(data)
    }

    fn insert(&mut self, sound: LiveSound) -> SoundHandle {
        let index = match self.free_slots.pop() {
            Some(index) => index,
            None => {
                self.slots.push(Slot {
                    generation: 1,
                    sound: None,
                });
                self.slots.len() as u32 - 1
            }
        };
        let slot = &mut self.slots[index as usize];
        slot.sound = Some(sound);
        SoundHandle {
            index,
            generation: slot.generation,
        }
    }

    fn live(&self, handle: SoundHandle) -> Option<&LiveSound> {
        let slot = self.slots.get(handle.index as usize)?;
        (slot.generation == handle.generation)
            .then_some(slot.sound.as_ref())
            .flatten()
    }

    fn live_mut(&mut self, handle: SoundHandle) -> Option<&mut LiveSound> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.sound.as_mut()
    }

    /// Stop one sound, fading it out over `fade`. A sound whose delay has not
    /// elapsed is cancelled at once.
    pub fn stop(&mut self, handle: SoundHandle, fade: Fade) {
        if let Some(sound) = self.live_mut(handle) {
            stop_sound(sound, fade);
        }
    }

    /// Stop every playing or scheduled instance of `name`.
    pub fn stop_all(&mut self, name: &str, fade: Fade) {
        for slot in &mut self.slots {
            if let Some(sound) = slot.sound.as_mut().filter(|s| s.name == name) {
                stop_sound(sound, fade);
            }
        }
    }

    /// Change one sound's volume (the `PlaySettings::volume` it started with).
    pub fn set_sound_volume(&mut self, handle: SoundHandle, volume: f32, fade: Fade) {
        let Some(volume) = clamp_volume(volume) else {
            return;
        };
        let name_volume = self.live(handle).map(|s| self.name_volume(&s.name));
        if let (Some(sound), Some(name_volume)) = (self.live_mut(handle), name_volume) {
            sound.volume = volume;
            sound
                .handle
                .set_volume(amplitude_to_decibels(volume * name_volume), fade.tween());
        }
    }

    /// Per-name volume, multiplied into every current and every future
    /// instance of `name`. Default 1.0. This is Flash's `Sound.setVolume`.
    /// It survives `unload` and a reload of the same name.
    pub fn set_name_volume(&mut self, name: &str, volume: f32, fade: Fade) {
        let Some(volume) = clamp_volume(volume) else {
            return;
        };
        self.name_volumes.insert(name.to_string(), volume);
        for slot in &mut self.slots {
            if let Some(sound) = slot.sound.as_mut().filter(|s| s.name == name) {
                sound
                    .handle
                    .set_volume(amplitude_to_decibels(sound.volume * volume), fade.tween());
            }
        }
    }

    pub fn name_volume(&self, name: &str) -> f32 {
        self.name_volumes.get(name).copied().unwrap_or(1.0)
    }

    pub fn set_playback_rate(&mut self, handle: SoundHandle, rate: f64, fade: Fade) {
        if !rate.is_finite() {
            return;
        }
        if let Some(sound) = self.live_mut(handle) {
            sound
                .handle
                .set_playback_rate(PlaybackRate(rate.clamp(0.01, 8.0)), fade.tween());
        }
    }

    pub fn state(&self, handle: SoundHandle) -> SoundState {
        let Some(sound) = self.live(handle) else {
            return SoundState::Stopped;
        };
        if sound.cancelled || sound.handle.state() == PlaybackState::Stopped {
            SoundState::Stopped
        } else if self.paused_at.is_some() {
            SoundState::Paused
        } else if Instant::now() < sound.starts_at {
            SoundState::Scheduled
        } else {
            SoundState::Playing
        }
    }

    /// `state` is `Scheduled`, `Playing` or `Paused`.
    pub fn is_playing(&self, handle: SoundHandle) -> bool {
        self.state(handle) != SoundState::Stopped
    }

    /// Live instances of `name` (not `Stopped`).
    pub fn playing_count(&self, name: &str) -> usize {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.sound.as_ref().is_some_and(|s| s.name == name))
            .filter(|(i, slot)| {
                self.is_playing(SoundHandle {
                    index: *i as u32,
                    generation: slot.generation,
                })
            })
            .count()
    }

    /// Free the slots of sounds that have ended, and apply volumes a game
    /// wrote to [`volumes`](Self::volumes) directly. The engine calls this
    /// once per frame; a game never has to.
    pub fn maintain(&mut self) {
        if self.volumes != self.applied {
            let wanted = self.volumes.clone();
            self.set_master_volume(wanted.master, Fade::NONE);
            self.set_bus_volume(Bus::Music, wanted.music, Fade::NONE);
            self.set_bus_volume(Bus::Sfx, wanted.sfx, Fade::NONE);
            self.set_bus_volume(Bus::Ambient, wanted.ambient, Fade::NONE);
            self.applied = self.volumes.clone();
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            let ended = slot
                .sound
                .as_ref()
                .is_some_and(|s| s.cancelled || s.handle.state() == PlaybackState::Stopped);
            if ended {
                slot.sound = None;
                slot.generation = slot.generation.wrapping_add(1).max(1);
                self.free_slots.push(index as u32);
            }
        }
        if let Some((_, handle)) = &self.music
            && self.live(*handle).is_none()
        {
            self.music = None;
        }
    }

    /// Play a sound effect by name, on the sfx bus.
    pub fn play_sfx(&mut self, name: &str) {
        self.play(name, &PlaySettings::default());
    }

    /// Play a sound effect at a world position with distance attenuation and
    /// stereo panning, on the sfx bus.
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
        // Linear distance attenuation; stereo panning from the x offset.
        let volume = (1.0 - distance / max_distance).clamp(0.0, 1.0);
        let panning = if max_distance > 0.0 {
            (dx / max_distance).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.play(
            name,
            &PlaySettings {
                volume,
                panning,
                ..Default::default()
            },
        );
    }

    // -----------------------------------------------------------------------
    // A3: Buses and master
    // -----------------------------------------------------------------------

    fn bus_decibels(&self, bus: Bus) -> Decibels {
        if self.muted[bus.index()] {
            Decibels::SILENCE
        } else {
            amplitude_to_decibels(self.bus_volume(bus))
        }
    }

    pub fn set_master_volume(&mut self, volume: f32, fade: Fade) {
        let Some(volume) = clamp_volume(volume) else {
            return;
        };
        self.volumes.master = volume;
        self.applied.master = volume;
        if let Some(device) = &mut self.device {
            device
                .kira
                .set_master(amplitude_to_decibels(volume), fade.tween());
        }
    }

    pub fn master_volume(&self) -> f32 {
        self.volumes.master
    }

    pub fn set_bus_volume(&mut self, bus: Bus, volume: f32, fade: Fade) {
        let Some(volume) = clamp_volume(volume) else {
            return;
        };
        *bus_field(&mut self.volumes, bus) = volume;
        *bus_field(&mut self.applied, bus) = volume;
        let db = self.bus_decibels(bus);
        if let Some(track) = self
            .device
            .as_mut()
            .and_then(|d| d.buses.get_mut(bus.index()))
        {
            track.set_volume(db, fade.tween());
        }
    }

    pub fn bus_volume(&self, bus: Bus) -> f32 {
        match bus {
            Bus::Music => self.volumes.music,
            Bus::Sfx => self.volumes.sfx,
            Bus::Ambient => self.volumes.ambient,
        }
    }

    /// Mute without losing the volume setting. A sound started on a muted bus
    /// plays silently and is heard when the bus is unmuted.
    pub fn set_bus_muted(&mut self, bus: Bus, muted: bool) {
        self.muted[bus.index()] = muted;
        let db = self.bus_decibels(bus);
        if let Some(track) = self
            .device
            .as_mut()
            .and_then(|d| d.buses.get_mut(bus.index()))
        {
            track.set_volume(db, Tween::default());
        }
    }

    pub fn is_bus_muted(&self, bus: Bus) -> bool {
        self.muted[bus.index()]
    }

    /// Set a volume by channel name: `"master"`, `"music"`, `"sfx"` or
    /// `"ambient"`, immediately.
    pub fn set_volume(&mut self, channel: &str, volume: f32) {
        match channel {
            "master" => self.set_master_volume(volume, Fade::NONE),
            "music" => self.set_bus_volume(Bus::Music, volume, Fade::NONE),
            "sfx" => self.set_bus_volume(Bus::Sfx, volume, Fade::NONE),
            "ambient" => self.set_bus_volume(Bus::Ambient, volume, Fade::NONE),
            _ => warn!("Unknown audio channel: {}", channel),
        }
    }

    // -----------------------------------------------------------------------
    // A4: Pause
    // -----------------------------------------------------------------------

    /// Pause every sound on every bus, fading out over `fade`. Paused sounds
    /// keep their position, scheduled ones their remaining delay; sounds
    /// started while paused start paused.
    pub fn pause_all(&mut self, fade: Fade) {
        if self.paused_at.is_some() {
            return;
        }
        self.paused_at = Some(Instant::now());
        if let Some(device) = &mut self.device {
            for track in &mut device.buses {
                track.pause(fade.tween());
            }
        }
    }

    /// Resume everything `pause_all` paused, fading in over `fade`.
    pub fn resume_all(&mut self, fade: Fade) {
        let Some(paused_at) = self.paused_at.take() else {
            return;
        };
        // Delays did not run down while paused.
        let now = Instant::now();
        for sound in self.slots.iter_mut().filter_map(|s| s.sound.as_mut()) {
            sound.starts_at += now.saturating_duration_since(sound.played_at.max(paused_at));
        }
        if let Some(device) = &mut self.device {
            for track in &mut device.buses {
                track.resume(fade.tween());
            }
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    // -----------------------------------------------------------------------
    // A6: Music
    // -----------------------------------------------------------------------

    /// Play the loaded sound `name` on the music bus, looping forever, and
    /// crossfade from whatever music is playing over `fade`. Returns the
    /// existing handle when `name` is already the current music, so a game
    /// can call it every frame from its state.
    pub fn start_music(&mut self, name: &str, fade: Fade) -> SoundHandle {
        if let Some((current, handle)) = &self.music
            && current == name
            && self.is_playing(*handle)
        {
            return *handle;
        }
        self.stop_bus(Bus::Music, fade);
        let handle = self.play(
            name,
            &PlaySettings {
                bus: Bus::Music,
                loops: LoopMode::Forever,
                fade_in: fade,
                ..Default::default()
            },
        );
        self.music = (!handle.is_none()).then(|| (name.to_string(), handle));
        handle
    }

    /// Fade out and stop all music.
    pub fn stop_music_with(&mut self, fade: Fade) {
        self.stop_bus(Bus::Music, fade);
        self.music = None;
    }

    /// The handle `start_music` last returned, while it is still live.
    pub fn current_music(&self) -> Option<SoundHandle> {
        self.music
            .as_ref()
            .map(|(_, handle)| *handle)
            .filter(|handle| self.is_playing(*handle))
    }

    /// Load `path` under `name` if it is not loaded yet, then
    /// `start_music(name, Fade::NONE)`: the music loops.
    pub fn play_music(&mut self, name: &str, path: &Path) {
        if !self.is_loaded(name)
            && let Err(e) = self.load_sound(name, path)
        {
            warn!("{e}");
            return;
        }
        self.start_music(name, Fade::NONE);
    }

    /// Stop all music at once.
    pub fn stop_music(&mut self) {
        self.stop_music_with(Fade::NONE);
    }

    fn stop_bus(&mut self, bus: Bus, fade: Fade) {
        for slot in &mut self.slots {
            if let Some(sound) = slot.sound.as_mut().filter(|s| s.bus == bus) {
                stop_sound(sound, fade);
            }
        }
    }

    /// Presentation-only randomness for picking variants (xorshift); never
    /// simulation state.
    fn next_random(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    #[cfg(test)]
    pub(crate) fn process_backend(&mut self) {
        if let Some(device) = &mut self.device {
            device.kira.process();
        }
    }

    #[cfg(test)]
    pub(crate) fn is_device_probed(&self) -> bool {
        self.device_probed
    }

    #[cfg(test)]
    pub(crate) fn has_device(&self) -> bool {
        self.device.is_some()
    }
}

fn stop_sound(sound: &mut LiveSound, fade: Fade) {
    if Instant::now() < sound.starts_at {
        sound.handle.stop(Tween::default());
        sound.cancelled = true;
    } else {
        sound.handle.stop(fade.tween());
    }
}

/// A volume clamped to `0.0..=4.0`; `None` (with a warning) when not finite.
fn clamp_volume(volume: f32) -> Option<f32> {
    if volume.is_finite() {
        Some(volume.clamp(0.0, 4.0))
    } else {
        warn!("Ignoring a non-finite volume");
        None
    }
}

fn bus_field(volumes: &mut VolumeChannels, bus: Bus) -> &mut f32 {
    match bus {
        Bus::Music => &mut volumes.music,
        Bus::Sfx => &mut volumes.sfx,
        Bus::Ambient => &mut volumes.ambient,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mono 16-bit WAV of `frames` samples at `rate` Hz.
    pub(crate) fn wav(frames: u32, rate: u32) -> Arc<[u8]> {
        let data_len = frames * 2;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            let s = ((i as f32 * 0.05).sin() * 8000.0) as i16;
            out.extend_from_slice(&s.to_le_bytes());
        }
        out.into()
    }

    fn silent_with(name: &str, seconds: u32) -> AudioManager {
        let mut audio = AudioManager::new_silent("assets");
        audio
            .load_sound_from_bytes(name, wav(seconds * 8000, 8000))
            .expect("test wav decodes");
        audio
    }

    #[test]
    fn building_and_configuring_leaves_the_device_closed() {
        let mut audio = AudioManager::new("assets");
        audio.load_sfx("missing", Path::new("does/not/exist.ogg"));
        audio.set_volume("music", 0.5);
        audio.stop_music();
        assert!(
            !audio.is_device_probed(),
            "only playback may open the device"
        );
        assert!(!audio.has_device());
    }

    #[test]
    fn without_a_device_play_is_silently_stopped() {
        let mut audio = AudioManager::new("assets");
        audio.device_probed = true; // as after a probe that found no device
        audio
            .load_sound_from_bytes("hit", wav(800, 8000))
            .expect("decodes without a device");
        let handle = audio.play("hit", &PlaySettings::default());
        assert_eq!(audio.state(handle), SoundState::Stopped);
        audio.play_music("theme", Path::new("does/not/exist.ogg"));
        assert!(!audio.open_device());
        assert!(audio.kira_manager_mut().is_none());
    }

    #[test]
    fn an_unknown_name_is_stopped_and_warned_about_once() {
        let mut audio = AudioManager::new_silent("assets");
        let a = audio.play("nope", &PlaySettings::default());
        let b = audio.play("nope", &PlaySettings::default());
        assert_eq!(audio.state(a), SoundState::Stopped);
        assert_eq!(audio.state(b), SoundState::Stopped);
        assert_eq!(audio.warned.len(), 1);
    }

    #[test]
    fn a_playing_sound_has_a_live_handle() {
        let mut audio = silent_with("hit", 1);
        let handle = audio.play("hit", &PlaySettings::default());
        assert!(!handle.is_none());
        assert_eq!(audio.state(handle), SoundState::Playing);
        assert_eq!(audio.playing_count("hit"), 1);
    }

    #[test]
    fn dead_and_reused_handles_do_nothing() {
        let mut audio = silent_with("hit", 1);
        let first = audio.play("hit", &PlaySettings::default());
        audio.stop(first, Fade::NONE);
        audio.process_backend();
        audio.maintain();
        assert_eq!(audio.state(first), SoundState::Stopped);

        let second = audio.play("hit", &PlaySettings::default());
        assert_eq!(second.index, first.index, "the slot is reused");
        assert_ne!(second.generation, first.generation, "with a new generation");
        for handle in [first, SoundHandle::NONE] {
            audio.stop(handle, Fade::NONE);
            audio.set_sound_volume(handle, 0.1, Fade::NONE);
            audio.set_playback_rate(handle, 2.0, Fade::NONE);
            assert_eq!(audio.state(handle), SoundState::Stopped);
            assert!(!audio.is_playing(handle));
        }
        audio.process_backend();
        assert_eq!(audio.state(second), SoundState::Playing);
    }

    #[test]
    fn name_volume_scales_new_instances_and_survives_unload() {
        let mut audio = silent_with("x", 1);
        audio.set_name_volume("x", 0.5, Fade::NONE);
        let handle = audio.play(
            "x",
            &PlaySettings {
                volume: 0.8,
                ..Default::default()
            },
        );
        let sound = audio.live(handle).expect("live");
        assert_eq!(sound.volume, 0.8);
        assert_eq!(audio.name_volume("x"), 0.5);
        audio.unload("x");
        assert!(!audio.is_loaded("x"));
        audio
            .load_sound_from_bytes("x", wav(800, 8000))
            .expect("reloads");
        assert_eq!(audio.name_volume("x"), 0.5);
    }

    #[test]
    fn a_count_too_long_to_render_is_refused_once() {
        let mut audio = silent_with("loop", 2);
        let settings = PlaySettings {
            loops: LoopMode::Count(61),
            ..Default::default()
        };
        let h = audio.play("loop", &settings);
        assert_eq!(audio.state(h), SoundState::Stopped);
        let h = audio.play("loop", &settings);
        assert_eq!(audio.state(h), SoundState::Stopped);
        assert_eq!(audio.warned.len(), 1);
        let ok = PlaySettings {
            loops: LoopMode::Count(60),
            ..Default::default()
        };
        let h = audio.play("loop", &ok);
        assert_eq!(audio.state(h), SoundState::Playing);
    }

    #[test]
    fn volumes_work_without_a_device_and_are_clamped() {
        let mut audio = AudioManager::new("assets");
        audio.set_master_volume(0.3, Fade::NONE);
        audio.set_bus_volume(Bus::Ambient, 9.0, Fade::NONE);
        audio.set_bus_volume(Bus::Sfx, f32::NAN, Fade::NONE);
        audio.set_bus_volume(Bus::Music, -1.0, Fade::NONE);
        assert_eq!(audio.master_volume(), 0.3);
        assert_eq!(audio.bus_volume(Bus::Ambient), 4.0);
        assert_eq!(audio.bus_volume(Bus::Sfx), 1.0, "NaN is ignored");
        assert_eq!(audio.bus_volume(Bus::Music), 0.0);
        audio.set_volume("sfx", 0.25);
        assert_eq!(audio.bus_volume(Bus::Sfx), 0.25);
    }

    #[test]
    fn muting_keeps_the_volume() {
        let mut audio = AudioManager::new_silent("assets");
        audio.set_bus_volume(Bus::Sfx, 0.7, Fade::NONE);
        audio.set_bus_muted(Bus::Sfx, true);
        assert!(audio.is_bus_muted(Bus::Sfx));
        assert_eq!(audio.bus_volume(Bus::Sfx), 0.7);
        audio.set_bus_muted(Bus::Sfx, false);
        assert_eq!(audio.bus_volume(Bus::Sfx), 0.7);
    }

    #[test]
    fn writing_the_volume_fields_is_applied_by_maintain() {
        let mut audio = AudioManager::new_silent("assets");
        audio.volumes.sfx = 0.2;
        assert_ne!(audio.applied.sfx, 0.2);
        audio.maintain();
        assert_eq!(audio.applied.sfx, 0.2);
        assert_eq!(audio.bus_volume(Bus::Sfx), 0.2);
    }

    #[test]
    fn pausing_holds_new_sounds_and_is_idempotent() {
        let mut audio = silent_with("hit", 1);
        audio.resume_all(Fade::NONE); // not paused: no-op
        assert!(!audio.is_paused());
        audio.pause_all(Fade::NONE);
        audio.pause_all(Fade::NONE);
        let handle = audio.play("hit", &PlaySettings::default());
        assert_eq!(audio.state(handle), SoundState::Paused);
        audio.resume_all(Fade::NONE);
        assert_eq!(audio.state(handle), SoundState::Playing);
    }

    #[test]
    fn a_delayed_sound_is_scheduled_and_a_stop_cancels_it() {
        let mut audio = silent_with("hit", 1);
        let handle = audio.play(
            "hit",
            &PlaySettings {
                delay: 30.0,
                ..Default::default()
            },
        );
        assert_eq!(audio.state(handle), SoundState::Scheduled);
        audio.stop_all("hit", Fade::secs(5.0));
        assert_eq!(audio.state(handle), SoundState::Stopped);
    }

    #[test]
    fn loading_from_bytes_reports_duration_and_keeps_variants_on_failure() {
        let mut audio = AudioManager::new("assets");
        audio
            .load_sound_from_bytes("blip", wav(4000, 8000))
            .expect("decodes");
        assert!(audio.is_loaded("blip"));
        assert_eq!(audio.sound_sample_rate("blip"), Some(8000));
        assert_eq!(audio.sound_duration("blip"), Some(0.5));
        let err = audio
            .load_sound_from_bytes("blip", Arc::from(&b"not audio at all"[..]))
            .unwrap_err();
        assert!(matches!(err, AudioError::Load { ref name, .. } if name == "blip"));
        assert_eq!(audio.sounds["blip"].len(), 1, "the earlier variant stays");
    }

    #[test]
    fn music_is_started_once_and_replaced_by_name() {
        let mut audio = silent_with("a", 1);
        audio
            .load_sound_from_bytes("b", wav(8000, 8000))
            .expect("decodes");
        let a = audio.start_music("a", Fade::secs(1.0));
        assert_eq!(audio.start_music("a", Fade::secs(1.0)), a);
        assert_eq!(audio.current_music(), Some(a));
        let b = audio.start_music("b", Fade::secs(1.0));
        assert_ne!(a, b);
        assert_eq!(audio.current_music(), Some(b));
        audio.stop_music_with(Fade::NONE);
        assert_eq!(audio.current_music(), None);
    }
}
