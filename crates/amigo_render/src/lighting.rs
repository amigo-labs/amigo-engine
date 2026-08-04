use amigo_core::Color;
use bytemuck::{Pod, Zeroable};

/// Global ambient light applied uniformly across the scene.
pub struct AmbientLight {
    pub color: Color,
    pub intensity: f32,
}

/// A point light that illuminates a circular area with configurable falloff.
pub struct PointLight {
    pub position: (f32, f32),
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    /// Exponent for the attenuation curve. Higher values produce a sharper falloff.
    pub falloff: f32,
}

/// GPU-friendly representation of a single point light, suitable for uniform buffers.
///
/// Field order matters and is not arbitrary: WGSL aligns `vec4<f32>` to 16 bytes,
/// so a `vec2` before the colour would put the colour at offset 16 on the GPU
/// while Rust has it at offset 8 — every light would read garbage. Colour first
/// makes both layouts agree: colour 0..16, position 16..24, radius 24, intensity
/// 28, falloff 32, padding to a 48-byte stride.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct LightData {
    pub color: [f32; 4],
    /// Light position. `build_uniform_data` leaves this in whatever space the
    /// caller set; the render pass converts to view-local coordinates.
    pub position: [f32; 2],
    pub radius: f32,
    pub intensity: f32,
    pub falloff: f32,
    pub _pad: [f32; 3],
}

/// Header written at the start of the lighting uniform buffer.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct LightingHeader {
    pub ambient_color: [f32; 4],
    /// Size of the visible world rect, so the shader can map a fragment's UV to
    /// the same coordinate space the light positions are in.
    pub view_size: [f32; 2],
    pub light_count: u32,
    pub _padding: u32,
}

/// Collects ambient and point light data and produces a byte buffer for the GPU.
pub struct LightingState {
    pub ambient: AmbientLight,
    pub lights: Vec<PointLight>,
    pub max_lights: usize,
}

impl LightingState {
    /// Creates a new lighting state with white ambient light at full intensity.
    pub fn new() -> Self {
        Self {
            ambient: AmbientLight {
                color: Color::new(1.0, 1.0, 1.0, 1.0),
                intensity: 1.0,
            },
            lights: Vec::new(),
            max_lights: 64,
        }
    }

    /// Adds a point light and returns its index.
    pub fn add_light(&mut self, light: PointLight) -> usize {
        let index = self.lights.len();
        self.lights.push(light);
        index
    }

    /// Removes the point light at the given index.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn remove_light(&mut self, index: usize) {
        self.lights.remove(index);
    }

    /// Removes all point lights.
    pub fn clear_lights(&mut self) {
        self.lights.clear();
    }

    /// Sets the ambient light color and intensity.
    pub fn set_ambient(&mut self, color: Color, intensity: f32) {
        self.ambient.color = color;
        self.ambient.intensity = intensity;
    }

    /// Returns the current number of point lights.
    pub fn light_count(&self) -> usize {
        self.lights.len()
    }

    /// Whether this state would change the image.
    ///
    /// White ambient at full intensity with no point lights multiplies the scene
    /// by 1.0, so running the lighting pass for it would cost a fullscreen pass
    /// to achieve nothing.
    pub fn is_active(&self) -> bool {
        if !self.lights.is_empty() {
            return true;
        }
        let a = &self.ambient;
        let neutral = (a.color.r * a.intensity - 1.0).abs() < f32::EPSILON
            && (a.color.g * a.intensity - 1.0).abs() < f32::EPSILON
            && (a.color.b * a.intensity - 1.0).abs() < f32::EPSILON;
        !neutral
    }

    /// Serializes the ambient light and up to `max_lights` point lights into a
    /// byte buffer suitable for uploading to a GPU uniform/storage buffer.
    ///
    /// Layout:
    ///   - [`LightingHeader`] (ambient color pre-multiplied by intensity, light count, padding)
    ///   - N x [`LightData`] (one per active point light, capped at `max_lights`)
    pub fn build_uniform_data(&self) -> Vec<u8> {
        let count = self.lights.len().min(self.max_lights);

        let header = LightingHeader {
            ambient_color: [
                self.ambient.color.r * self.ambient.intensity,
                self.ambient.color.g * self.ambient.intensity,
                self.ambient.color.b * self.ambient.intensity,
                self.ambient.color.a,
            ],
            view_size: [1.0, 1.0],
            light_count: count as u32,
            _padding: 0,
        };

        let header_bytes = bytemuck::bytes_of(&header);

        let mut buf = Vec::with_capacity(
            std::mem::size_of::<LightingHeader>() + count * std::mem::size_of::<LightData>(),
        );
        buf.extend_from_slice(header_bytes);

        for light in self.lights.iter().take(count) {
            let data = LightData {
                color: [light.color.r, light.color.g, light.color.b, light.color.a],
                position: [light.position.0, light.position.1],
                radius: light.radius,
                intensity: light.intensity,
                falloff: light.falloff,
                _pad: [0.0; 3],
            };
            buf.extend_from_slice(bytemuck::bytes_of(&data));
        }

        buf
    }
}

impl Default for LightingState {
    fn default() -> Self {
        Self::new()
    }
}
