use amigo_core::{RenderVec2, Rect};

/// Camera mode presets.
#[derive(Clone, Debug)]
pub enum CameraMode {
    /// Fixed camera position.
    Fixed,
    /// Follow a target position with optional deadzone and lookahead.
    Follow {
        deadzone: Rect,
        lookahead: f32,
    },
    /// Smooth follow with damping.
    FollowSmooth {
        speed: f32,
    },
    /// Screen-locked (Zelda-style room transitions).
    ScreenLock {
        room_width: f32,
        room_height: f32,
    },
}

/// Camera system managing view position, zoom, and effects.
pub struct Camera {
    pub position: RenderVec2,
    pub target: RenderVec2,
    pub zoom: f32,
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub mode: CameraMode,

    // Shake effect
    shake_intensity: f32,
    shake_decay: f32,
    shake_offset: RenderVec2,
}

impl Camera {
    pub fn new(virtual_width: f32, virtual_height: f32) -> Self {
        Self {
            position: RenderVec2::ZERO,
            target: RenderVec2::ZERO,
            zoom: 1.0,
            virtual_width,
            virtual_height,
            mode: CameraMode::Fixed,
            shake_intensity: 0.0,
            shake_decay: 0.95,
            shake_offset: RenderVec2::ZERO,
        }
    }

    pub fn set_target(&mut self, target: RenderVec2) {
        self.target = target;
    }

    pub fn shake(&mut self, intensity: f32) {
        self.shake_intensity = intensity.max(self.shake_intensity);
    }

    pub fn update(&mut self, dt: f32) {
        match &self.mode {
            CameraMode::Fixed => {}
            CameraMode::Follow { .. } => {
                self.position = self.target;
            }
            CameraMode::FollowSmooth { speed } => {
                let speed = *speed;
                self.position = self.position.lerp(self.target, speed * dt);
            }
            CameraMode::ScreenLock { room_width, room_height } => {
                let rw = *room_width;
                let rh = *room_height;
                self.position.x = (self.target.x / rw).floor() * rw + rw * 0.5;
                self.position.y = (self.target.y / rh).floor() * rh + rh * 0.5;
            }
        }

        // Update shake
        if self.shake_intensity > 0.1 {
            self.shake_offset = RenderVec2::new(
                (self.shake_intensity * pseudo_random_f32(self.position.x)),
                (self.shake_intensity * pseudo_random_f32(self.position.y + 100.0)),
            );
            self.shake_intensity *= self.shake_decay;
        } else {
            self.shake_intensity = 0.0;
            self.shake_offset = RenderVec2::ZERO;
        }
    }

    /// Get the effective camera position including shake offset.
    pub fn effective_position(&self) -> RenderVec2 {
        RenderVec2::new(
            self.position.x + self.shake_offset.x,
            self.position.y + self.shake_offset.y,
        )
    }

    /// Get the visible area in world coordinates.
    pub fn view_rect(&self) -> Rect {
        let eff = self.effective_position();
        let half_w = (self.virtual_width / self.zoom) * 0.5;
        let half_h = (self.virtual_height / self.zoom) * 0.5;
        Rect::new(eff.x - half_w, eff.y - half_h, half_w * 2.0, half_h * 2.0)
    }

    /// Convert screen coordinates to world coordinates.
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32, window_width: f32, window_height: f32) -> RenderVec2 {
        let eff = self.effective_position();
        let norm_x = screen_x / window_width;
        let norm_y = screen_y / window_height;
        RenderVec2::new(
            eff.x + (norm_x - 0.5) * self.virtual_width / self.zoom,
            eff.y + (norm_y - 0.5) * self.virtual_height / self.zoom,
        )
    }

    /// Build the orthographic projection matrix.
    pub fn projection_matrix(&self) -> [[f32; 4]; 4] {
        let eff = self.effective_position();
        let half_w = (self.virtual_width / self.zoom) * 0.5;
        let half_h = (self.virtual_height / self.zoom) * 0.5;

        let left = eff.x - half_w;
        let right = eff.x + half_w;
        let top = eff.y - half_h;
        let bottom = eff.y + half_h;

        ortho(left, right, bottom, top, -1.0, 1.0)
    }
}

fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let rml = right - left;
    let tmb = top - bottom;
    let fmn = far - near;
    [
        [2.0 / rml, 0.0, 0.0, 0.0],
        [0.0, 2.0 / tmb, 0.0, 0.0],
        [0.0, 0.0, -2.0 / fmn, 0.0],
        [-(right + left) / rml, -(top + bottom) / tmb, -(far + near) / fmn, 1.0],
    ]
}

/// Simple pseudo-random for shake (no determinism requirement for visual effects).
fn pseudo_random_f32(seed: f32) -> f32 {
    let s = (seed * 12.9898).sin() * 43758.5453;
    (s - s.floor()) * 2.0 - 1.0
}
