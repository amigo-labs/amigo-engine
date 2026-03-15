use serde::{Deserialize, Serialize};

/// Engine configuration loaded from amigo.toml.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineConfig {
    pub window: WindowConfig,
    pub render: RenderConfig,
    pub audio: AudioConfig,
    pub dev: DevConfig,
    #[serde(default)]
    pub splash: SplashConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenderConfig {
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub scale_mode: String,
    /// Art style: "pixel_art" (default), "raster_art", or "hybrid".
    #[serde(default = "default_art_style")]
    pub art_style: String,
}

fn default_art_style() -> String {
    "pixel_art".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioConfig {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DevConfig {
    pub hot_reload: bool,
    pub debug_overlay: bool,
    pub api_server: bool,
    pub api_port: u16,
    /// Run in headless mode (no window/renderer). Simulation only, controlled via API.
    #[serde(default)]
    pub headless: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplashConfig {
    pub enabled: bool,
}

impl Default for SplashConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig {
                title: "Amigo Game".to_string(),
                width: 1280,
                height: 720,
                fullscreen: false,
                vsync: true,
            },
            render: RenderConfig {
                virtual_width: 480,
                virtual_height: 270,
                scale_mode: "pixel_perfect".to_string(),
                art_style: "pixel_art".to_string(),
            },
            audio: AudioConfig {
                master_volume: 0.8,
                sfx_volume: 1.0,
                music_volume: 0.6,
            },
            dev: DevConfig {
                hot_reload: true,
                debug_overlay: true,
                api_server: false,
                api_port: 9999,
                headless: false,
            },
            splash: SplashConfig::default(),
        }
    }
}

impl EngineConfig {
    /// Try to load from amigo.toml, falling back to defaults.
    /// Respects `AMIGO_HEADLESS=1` and `AMIGO_API=1` environment variables.
    pub fn load() -> Self {
        let path = std::path::Path::new("amigo.toml");
        let mut config = if path.exists() {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str(&contents) {
                    config
                } else {
                    Self::default()
                }
            } else {
                Self::default()
            }
        } else {
            Self::default()
        };

        // Environment variable overrides
        if std::env::var("AMIGO_HEADLESS").as_deref() == Ok("1") {
            config.dev.headless = true;
            config.dev.api_server = true;
        }
        if std::env::var("AMIGO_API").as_deref() == Ok("1") {
            config.dev.api_server = true;
        }

        config
    }
}
