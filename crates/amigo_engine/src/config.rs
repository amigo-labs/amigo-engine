use serde::{Deserialize, Serialize};
use tracing::warn;

/// Engine configuration loaded from amigo.toml.
///
/// The `[art]` section (art generation defaults) is parsed by `amigo_artgen`.
/// Audio generation defaults live alongside the engine audio settings in the
/// `[audio]` section and are parsed by `amigo_audiogen`. Both sets of keys
/// coexist because serde ignores unknown fields by default.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineConfig {
    /// The game's name (`name = "..."` at the top of `amigo.toml`, which
    /// `amigo new` writes). Save files go into a directory named after it;
    /// without it, the window title is used.
    #[serde(default)]
    pub name: Option<String>,
    pub window: WindowConfig,
    pub render: RenderConfig,
    pub audio: AudioConfig,
    pub dev: DevConfig,
    #[serde(default)]
    pub splash: SplashConfig,
    #[serde(default)]
    pub input: InputConfig,
    #[serde(default)]
    pub net: NetConfig,
    /// Art generation defaults (parsed by amigo_artgen, ignored by engine).
    #[serde(default, skip_serializing)]
    pub art: Option<toml::Value>,
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
    /// Seed for `GameContext::rng`. Without one, each run takes a seed from
    /// the clock and logs it.
    #[serde(default)]
    pub seed: Option<u64>,
}

impl DevConfig {
    /// The seed to start this run with: [`seed`](Self::seed), or one taken
    /// from the clock.
    pub fn resolve_seed(&self) -> u64 {
        self.seed.unwrap_or_else(|| {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            // Spread consecutive clock values over the whole range.
            amigo_core::SimRng::new(nanos).next_u64()
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SplashConfig {
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputConfig {
    /// Action bindings file (RON), relative to the working directory like
    /// `amigo.toml`. When it exists it replaces the defaults passed to
    /// `EngineBuilder::input_bindings`, so it is where a player's rebinds
    /// belong. A missing file is not an error.
    #[serde(default = "default_bindings_path")]
    pub bindings: String,
}

/// `[net]`: two-player network games (`amigo run --host` / `--join`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetConfig {
    /// Ticks between pressing a key and its effect, on both machines
    /// alike. It hides the network's latency: 3 ticks (50 ms) suits a LAN,
    /// 6-8 an internet connection. The host's value is used.
    #[serde(default = "default_input_delay")]
    pub input_delay: u32,
}

fn default_input_delay() -> u32 {
    3
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            input_delay: default_input_delay(),
        }
    }
}

fn default_bindings_path() -> String {
    "input.ron".to_string()
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            bindings: default_bindings_path(),
        }
    }
}

impl Default for SplashConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            name: None,
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
                seed: None,
            },
            splash: SplashConfig::default(),
            input: InputConfig::default(),
            net: NetConfig::default(),
            art: None,
        }
    }
}

impl EngineConfig {
    /// The save configuration for this game: slots in a directory named after
    /// [`name`](Self::name), or the window title when there is none. Every
    /// game used to share one hard-coded `amigo_game` directory, so two
    /// games overwrote each other's saves.
    pub fn save_config(&self) -> amigo_core::save::SaveConfig {
        let app_name = self
            .name
            .clone()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| self.window.title.clone());
        amigo_core::save::SaveConfig {
            max_slots: 10,
            autosave_slots: 3,
            autosave_interval_secs: 300.0,
            app_name,
        }
    }

    /// Try to load from `amigo.toml` in the current directory, falling back to
    /// defaults.
    ///
    /// Environment overrides, applied after the file: `AMIGO_HEADLESS=1`,
    /// `AMIGO_API=1`, `AMIGO_API_PORT=<port>`, and `AMIGO_SEED=<seed>`.
    pub fn load() -> Self {
        Self::load_from(std::path::Path::new("amigo.toml"), |key| {
            std::env::var(key).ok()
        })
    }

    /// [`EngineConfig::load`] with the config path and environment injected, so
    /// the override logic is testable without touching process globals.
    pub(crate) fn load_from<F>(path: &std::path::Path, env: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut config = match std::fs::read_to_string(path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    // Falling back silently here made a malformed config
                    // indistinguishable from no config at all: the game would
                    // start at the wrong resolution with no hint why.
                    warn!(
                        "Ignoring '{}': {e}. Falling back to default engine config.",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                warn!("Could not read '{}': {e}. Using defaults.", path.display());
                Self::default()
            }
        };

        // Environment variable overrides
        if env("AMIGO_HEADLESS").as_deref() == Some("1") {
            config.dev.headless = true;
            config.dev.api_server = true;
        }
        if env("AMIGO_API").as_deref() == Some("1") {
            config.dev.api_server = true;
        }
        // `amigo dev --port` and `amigo run --api --port` pass the port this way.
        // Until this was read, the CLI's --port silently had no effect and the
        // dev loop's snapshot RPC went to a port nothing was listening on.
        if let Some(raw) = env("AMIGO_API_PORT") {
            match raw.parse::<u16>() {
                Ok(0) => warn!("AMIGO_API_PORT=0 is not a usable port, ignoring"),
                Ok(port) => config.dev.api_port = port,
                Err(e) => warn!("Ignoring AMIGO_API_PORT='{raw}': {e}"),
            }
        }
        if let Some(raw) = env("AMIGO_SEED") {
            match raw.parse::<u64>() {
                Ok(seed) => config.dev.seed = Some(seed),
                Err(e) => warn!("Ignoring AMIGO_SEED='{raw}': {e}"),
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;

    /// Build an `env` closure over a fixed map, so tests never touch the real
    /// process environment (which is shared across parallel test threads).
    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("amigo_cfg_{name}.toml"));
        let mut f = std::fs::File::create(&path).expect("create temp config");
        f.write_all(contents.as_bytes()).expect("write temp config");
        path
    }

    #[test]
    fn saves_are_named_after_the_game() {
        let path = write_temp(
            "named",
            r#"
            name = "space_race"

            [window]
            title = "Space Race!"
            width = 800
            height = 600
            fullscreen = false
            vsync = true

            [render]
            virtual_width = 320
            virtual_height = 180
            scale_mode = "pixel_perfect"

            [audio]
            master_volume = 1.0
            sfx_volume = 1.0
            music_volume = 1.0

            [dev]
            hot_reload = false
            debug_overlay = false
            api_server = false
            api_port = 9999
            "#,
        );
        let mut config = EngineConfig::load_from(&path, env_of(&[]));
        let _ = std::fs::remove_file(&path);
        assert_eq!(config.save_config().app_name, "space_race");

        // Without a name, the title; never the shared "amigo_game".
        config.name = None;
        assert_eq!(config.save_config().app_name, "Space Race!");
    }

    #[test]
    fn seed_comes_from_the_file_or_the_environment() {
        let path = write_temp(
            "seed",
            r#"
            [window]
            title = "Seeded"
            width = 800
            height = 600
            fullscreen = false
            vsync = true

            [render]
            virtual_width = 320
            virtual_height = 180
            scale_mode = "pixel_perfect"

            [audio]
            master_volume = 1.0
            sfx_volume = 1.0
            music_volume = 1.0

            [dev]
            hot_reload = false
            debug_overlay = false
            api_server = false
            api_port = 9999
            seed = 7
            "#,
        );
        let config = EngineConfig::load_from(&path, env_of(&[]));
        assert_eq!(config.dev.seed, Some(7));
        assert_eq!(config.dev.resolve_seed(), 7);
        let config = EngineConfig::load_from(&path, env_of(&[("AMIGO_SEED", "99")]));
        assert_eq!(config.dev.seed, Some(99));
        let config = EngineConfig::load_from(&path, env_of(&[("AMIGO_SEED", "x")]));
        assert_eq!(config.dev.seed, Some(7));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let path = std::env::temp_dir().join("amigo_cfg_definitely_absent.toml");
        let _ = std::fs::remove_file(&path);

        let config = EngineConfig::load_from(&path, env_of(&[]));

        assert_eq!(config.render.virtual_width, 480);
        assert_eq!(config.render.virtual_height, 270);
        assert_eq!(config.dev.api_port, 9999);
        assert!(!config.dev.headless);
    }

    #[test]
    fn file_values_win_over_defaults() {
        let path = write_temp(
            "valid",
            r#"
            [window]
            title = "Test"
            width = 800
            height = 600
            fullscreen = false
            vsync = false

            [render]
            virtual_width = 320
            virtual_height = 180
            scale_mode = "stretch"

            [audio]
            master_volume = 0.5
            sfx_volume = 0.5
            music_volume = 0.5

            [dev]
            hot_reload = false
            debug_overlay = false
            api_server = true
            api_port = 1234
            "#,
        );

        let config = EngineConfig::load_from(&path, env_of(&[]));

        assert_eq!(config.render.virtual_width, 320);
        assert_eq!(config.dev.api_port, 1234);
        assert!(config.dev.api_server);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn malformed_file_falls_back_to_defaults() {
        // Not silently: `load_from` warns. What matters here is that a broken
        // file cannot leave the engine in a half-populated config.
        let path = write_temp("malformed", "this is not = valid = toml [[[");

        let config = EngineConfig::load_from(&path, env_of(&[]));

        assert_eq!(config.render.virtual_width, 480);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn headless_env_implies_api_server() {
        let path = std::env::temp_dir().join("amigo_cfg_absent2.toml");
        let _ = std::fs::remove_file(&path);

        let config = EngineConfig::load_from(&path, env_of(&[("AMIGO_HEADLESS", "1")]));

        assert!(config.dev.headless);
        assert!(
            config.dev.api_server,
            "headless is only controllable over the API, so it must enable it"
        );
    }

    #[test]
    fn api_env_enables_server_without_headless() {
        let path = std::env::temp_dir().join("amigo_cfg_absent3.toml");
        let _ = std::fs::remove_file(&path);

        let config = EngineConfig::load_from(&path, env_of(&[("AMIGO_API", "1")]));

        assert!(config.dev.api_server);
        assert!(!config.dev.headless);
    }

    #[test]
    fn api_port_env_overrides_file() {
        let path = write_temp(
            "port",
            r#"
            [window]
            title = "T"
            width = 1
            height = 1
            fullscreen = false
            vsync = false
            [render]
            virtual_width = 1
            virtual_height = 1
            scale_mode = "stretch"
            [audio]
            master_volume = 1.0
            sfx_volume = 1.0
            music_volume = 1.0
            [dev]
            hot_reload = false
            debug_overlay = false
            api_server = true
            api_port = 9999
            "#,
        );

        let config = EngineConfig::load_from(&path, env_of(&[("AMIGO_API_PORT", "7777")]));

        assert_eq!(config.dev.api_port, 7777);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unusable_api_port_env_is_ignored() {
        let path = std::env::temp_dir().join("amigo_cfg_absent4.toml");
        let _ = std::fs::remove_file(&path);

        for raw in ["0", "not-a-port", "70000", ""] {
            let config = EngineConfig::load_from(&path, env_of(&[("AMIGO_API_PORT", raw)]));
            assert_eq!(
                config.dev.api_port, 9999,
                "AMIGO_API_PORT='{raw}' should be rejected, not silently applied"
            );
        }
    }
}
