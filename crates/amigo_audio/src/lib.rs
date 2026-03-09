use kira::manager::{AudioManager as KiraManager, AudioManagerSettings};
use kira::manager::backend::DefaultBackend;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Audio init failed: {0}")]
    Init(String),
    #[error("Sound not found: {0}")]
    NotFound(String),
    #[error("Playback error: {0}")]
    Playback(String),
}

/// SFX definition with variants and randomization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfxDefinition {
    pub files: Vec<PathBuf>,
    pub volume: f32,
    pub pitch_variance: f32,
    pub max_concurrent: u32,
    pub cooldown: Option<f32>,
}

/// Volume channels.
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
        let Some(manager) = &mut self.manager else { return };

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

    /// Play music from file.
    pub fn play_music(&mut self, name: &str, path: &Path) {
        let Some(manager) = &mut self.manager else { return };

        // Stop current music with same name
        self.music_handles.remove(name);

        match StaticSoundData::from_file(path) {
            Ok(data) => {
                match manager.play(data) {
                    Ok(handle) => {
                        self.music_handles.insert(name.to_string(), handle);
                        info!("Playing music: {}", name);
                    }
                    Err(e) => warn!("Failed to play music '{}': {}", name, e),
                }
            }
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
}
