use serde::{Serialize, Deserialize};

/// Lerp between two RGBA colors by factor `t` (0.0 = a, 1.0 = b).
pub fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A named collection of visual atmosphere parameters.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AtmospherePreset {
    pub name: String,
    pub ambient_color: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_density: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub vignette_strength: f32,
    pub particle_tint: [f32; 4],
}

impl AtmospherePreset {
    /// Neutral daytime atmosphere.
    pub fn default() -> Self {
        Self {
            name: "default".to_string(),
            ambient_color: [1.0, 1.0, 1.0, 1.0],
            fog_color: [0.8, 0.85, 0.9, 1.0],
            fog_density: 0.0,
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            vignette_strength: 0.0,
            particle_tint: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Warm sunset pirate theme.
    pub fn pirate() -> Self {
        Self {
            name: "pirate".to_string(),
            ambient_color: [1.0, 0.75, 0.5, 1.0],
            fog_color: [0.9, 0.55, 0.3, 1.0],
            fog_density: 0.15,
            brightness: 1.05,
            contrast: 1.1,
            saturation: 1.15,
            vignette_strength: 0.25,
            particle_tint: [1.0, 0.8, 0.5, 1.0],
        }
    }

    /// Bright, sandy yellow haze.
    pub fn desert() -> Self {
        Self {
            name: "desert".to_string(),
            ambient_color: [1.0, 0.92, 0.7, 1.0],
            fog_color: [0.95, 0.88, 0.6, 1.0],
            fog_density: 0.2,
            brightness: 1.2,
            contrast: 1.05,
            saturation: 0.9,
            vignette_strength: 0.1,
            particle_tint: [1.0, 0.95, 0.7, 1.0],
        }
    }

    /// Dark, green-tinted forest.
    pub fn dark_forest() -> Self {
        Self {
            name: "dark_forest".to_string(),
            ambient_color: [0.3, 0.45, 0.25, 1.0],
            fog_color: [0.15, 0.25, 0.1, 1.0],
            fog_density: 0.35,
            brightness: 0.6,
            contrast: 1.1,
            saturation: 0.85,
            vignette_strength: 0.45,
            particle_tint: [0.4, 0.7, 0.3, 1.0],
        }
    }

    /// Green-tinted, high contrast digital rain.
    pub fn matrix() -> Self {
        Self {
            name: "matrix".to_string(),
            ambient_color: [0.0, 1.0, 0.2, 1.0],
            fog_color: [0.0, 0.15, 0.02, 1.0],
            fog_density: 0.1,
            brightness: 0.85,
            contrast: 1.5,
            saturation: 0.6,
            vignette_strength: 0.5,
            particle_tint: [0.0, 1.0, 0.3, 1.0],
        }
    }

    /// Cold blue icy tint.
    pub fn ice() -> Self {
        Self {
            name: "ice".to_string(),
            ambient_color: [0.7, 0.85, 1.0, 1.0],
            fog_color: [0.75, 0.88, 1.0, 1.0],
            fog_density: 0.18,
            brightness: 1.1,
            contrast: 1.0,
            saturation: 0.8,
            vignette_strength: 0.15,
            particle_tint: [0.8, 0.9, 1.0, 1.0],
        }
    }

    /// Dark blue nighttime with heavy vignette.
    pub fn night() -> Self {
        Self {
            name: "night".to_string(),
            ambient_color: [0.15, 0.15, 0.35, 1.0],
            fog_color: [0.05, 0.05, 0.15, 1.0],
            fog_density: 0.25,
            brightness: 0.5,
            contrast: 1.2,
            saturation: 0.7,
            vignette_strength: 0.6,
            particle_tint: [0.2, 0.2, 0.5, 1.0],
        }
    }
}

/// Controls visual atmosphere per world/level, including fog, ambient light,
/// color grading, particle tinting, and smooth transitions between presets.
pub struct AtmosphereManager {
    current: AtmospherePreset,
    target: Option<AtmospherePreset>,
    transition_progress: f32,
    transition_duration: f32,
    presets: Vec<AtmospherePreset>,
}

impl AtmosphereManager {
    /// Create a new atmosphere manager with the given default preset.
    pub fn new(default_preset: AtmospherePreset) -> Self {
        Self {
            current: default_preset,
            target: None,
            transition_progress: 0.0,
            transition_duration: 0.0,
            presets: Vec::new(),
        }
    }

    /// Add a named preset to the library.
    pub fn add_preset(&mut self, preset: AtmospherePreset) {
        self.presets.push(preset);
    }

    /// Begin a smooth transition to the named preset over `duration` seconds.
    /// Returns `true` if the preset was found, `false` otherwise.
    pub fn transition_to(&mut self, preset_name: &str, duration: f32) -> bool {
        if let Some(preset) = self.presets.iter().find(|p| p.name == preset_name) {
            self.target = Some(preset.clone());
            self.transition_progress = 0.0;
            self.transition_duration = duration;
            true
        } else {
            false
        }
    }

    /// Immediately snap to the named preset with no transition.
    /// Returns `true` if the preset was found, `false` otherwise.
    pub fn set_immediate(&mut self, preset_name: &str) -> bool {
        if let Some(preset) = self.presets.iter().find(|p| p.name == preset_name) {
            self.current = preset.clone();
            self.target = None;
            self.transition_progress = 0.0;
            self.transition_duration = 0.0;
            true
        } else {
            false
        }
    }

    /// Advance the transition by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        if let Some(ref target) = self.target {
            if self.transition_duration > 0.0 {
                self.transition_progress += dt / self.transition_duration;
            } else {
                self.transition_progress = 1.0;
            }

            if self.transition_progress >= 1.0 {
                self.transition_progress = 1.0;
                self.current = target.clone();
                self.target = None;
                self.transition_progress = 0.0;
                self.transition_duration = 0.0;
            }
        }
    }

    /// The interpolated ambient color for the current frame.
    pub fn current_ambient_color(&self) -> [f32; 4] {
        match &self.target {
            Some(target) => lerp_color(self.current.ambient_color, target.ambient_color, self.transition_progress),
            None => self.current.ambient_color,
        }
    }

    /// The interpolated fog color for the current frame.
    pub fn current_fog_color(&self) -> [f32; 4] {
        match &self.target {
            Some(target) => lerp_color(self.current.fog_color, target.fog_color, self.transition_progress),
            None => self.current.fog_color,
        }
    }

    /// The interpolated fog density for the current frame.
    pub fn current_fog_density(&self) -> f32 {
        match &self.target {
            Some(target) => lerp(self.current.fog_density, target.fog_density, self.transition_progress),
            None => self.current.fog_density,
        }
    }

    /// The interpolated brightness for the current frame.
    pub fn current_brightness(&self) -> f32 {
        match &self.target {
            Some(target) => lerp(self.current.brightness, target.brightness, self.transition_progress),
            None => self.current.brightness,
        }
    }

    /// The interpolated contrast for the current frame.
    pub fn current_contrast(&self) -> f32 {
        match &self.target {
            Some(target) => lerp(self.current.contrast, target.contrast, self.transition_progress),
            None => self.current.contrast,
        }
    }

    /// The interpolated saturation for the current frame.
    pub fn current_saturation(&self) -> f32 {
        match &self.target {
            Some(target) => lerp(self.current.saturation, target.saturation, self.transition_progress),
            None => self.current.saturation,
        }
    }

    /// The interpolated vignette strength for the current frame.
    pub fn current_vignette(&self) -> f32 {
        match &self.target {
            Some(target) => lerp(self.current.vignette_strength, target.vignette_strength, self.transition_progress),
            None => self.current.vignette_strength,
        }
    }

    /// Whether a transition is currently in progress.
    pub fn is_transitioning(&self) -> bool {
        self.target.is_some()
    }
}
