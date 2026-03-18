use amigo_core::{EasingFn, Rect, RenderVec2};

/// Camera mode presets.
#[derive(Clone, Debug)]
pub enum CameraMode {
    /// Fixed camera position.
    Fixed,
    /// Follow a target with deadzone and lookahead.
    Follow {
        /// Deadzone rect relative to camera center. Camera only moves when target exits this zone.
        deadzone: Rect,
        /// How far ahead of the target the camera looks (based on target velocity direction).
        lookahead: f32,
        /// Smoothing speed for camera movement (0 = instant, higher = smoother).
        smoothing: f32,
    },
    /// Smooth follow with damping (simple mode without deadzone).
    FollowSmooth { speed: f32 },
    /// Screen-locked (Zelda-style). Camera snaps to room grid.
    ScreenLock { room_width: f32, room_height: f32 },
    /// Smooth room transitions (Metroidvania-style).
    RoomTransition {
        room_width: f32,
        room_height: f32,
        transition_speed: f32,
    },
    /// Boss arena - camera frames a fixed area, optionally tracking midpoint between player and boss.
    BossArena {
        center: RenderVec2,
        arena_width: f32,
        arena_height: f32,
    },
    /// Cinematic pan from current position to a target over a duration.
    CinematicPan {
        from: RenderVec2,
        to: RenderVec2,
        duration: f32,
        elapsed: f32,
        easing: Easing,
    },
    /// ARPG-style: follows target but also pans when mouse is near screen edges.
    /// Good for Diablo-like or RTS games.
    EdgePan {
        /// How fast the camera follows the target.
        follow_speed: f32,
        /// Width of the edge zone in normalized screen coords (0.0–0.5, typically 0.05–0.15).
        edge_zone: f32,
        /// Edge pan speed in pixels per second.
        edge_speed: f32,
        /// Maximum distance the camera can drift from the target.
        max_drift: f32,
    },
    /// RTS / top-down free camera controlled entirely by edge-pan and keyboard.
    FreePan {
        /// Width of the edge zone in normalized screen coords.
        edge_zone: f32,
        /// Pan speed in pixels per second.
        pan_speed: f32,
    },
}

/// Re-export of EasingFn from amigo_core::tween.
/// Legacy alias — use `EasingFn` directly for new code.
pub type Easing = EasingFn;

/// Camera system managing view position, zoom, and effects.
pub struct Camera {
    pub position: RenderVec2,
    pub target: RenderVec2,
    pub zoom: f32,
    pub target_zoom: f32,
    pub zoom_speed: f32,
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub mode: CameraMode,
    /// When true, camera position is snapped to integer pixels in the projection.
    /// Enabled by default for pixel-art games; disable for raster-art or hybrid.
    pub pixel_snap: bool,

    // Bounds clamping
    pub bounds: Option<Rect>,

    // Shake effect
    shake_intensity: f32,
    shake_decay: f32,
    shake_offset: RenderVec2,
    shake_time: f32,

    // Lookahead tracking
    prev_target: RenderVec2,
    lookahead_offset: RenderVec2,

    // Room transition state
    room_target: RenderVec2,
    transitioning: bool,

    // Edge-pan: normalized mouse position (0..1)
    mouse_norm_x: f32,
    mouse_norm_y: f32,
    edge_drift: RenderVec2,
}

impl Camera {
    pub fn new(virtual_width: f32, virtual_height: f32) -> Self {
        Self {
            position: RenderVec2::ZERO,
            target: RenderVec2::ZERO,
            zoom: 1.0,
            target_zoom: 1.0,
            zoom_speed: 5.0,
            virtual_width,
            virtual_height,
            mode: CameraMode::Fixed,
            pixel_snap: true,
            bounds: None,
            shake_intensity: 0.0,
            shake_decay: 8.0,
            shake_offset: RenderVec2::ZERO,
            shake_time: 0.0,
            prev_target: RenderVec2::ZERO,
            lookahead_offset: RenderVec2::ZERO,
            room_target: RenderVec2::ZERO,
            transitioning: false,
            mouse_norm_x: 0.5,
            mouse_norm_y: 0.5,
            edge_drift: RenderVec2::ZERO,
        }
    }

    /// Update the normalized mouse position (0..1 range). Called from the engine.
    pub fn set_mouse_normalized(&mut self, nx: f32, ny: f32) {
        self.mouse_norm_x = nx.clamp(0.0, 1.0);
        self.mouse_norm_y = ny.clamp(0.0, 1.0);
    }

    pub fn set_target(&mut self, target: RenderVec2) {
        self.prev_target = self.target;
        self.target = target;
    }

    pub fn shake(&mut self, intensity: f32) {
        self.shake_intensity = intensity.max(self.shake_intensity);
    }

    /// Set zoom with smooth transition.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.target_zoom = zoom.max(0.1);
    }

    /// Set zoom immediately without transition.
    pub fn set_zoom_immediate(&mut self, zoom: f32) {
        self.zoom = zoom.max(0.1);
        self.target_zoom = self.zoom;
    }

    /// Set world bounds the camera cannot exceed.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = Some(bounds);
    }

    pub fn clear_bounds(&mut self) {
        self.bounds = None;
    }

    /// Start a cinematic pan from current position to a target.
    pub fn start_cinematic_pan(&mut self, to: RenderVec2, duration: f32, easing: Easing) {
        self.mode = CameraMode::CinematicPan {
            from: self.position,
            to,
            duration,
            elapsed: 0.0,
            easing,
        };
    }

    /// Returns true if a cinematic pan is currently playing.
    pub fn is_panning(&self) -> bool {
        matches!(self.mode, CameraMode::CinematicPan { .. })
    }

    /// Returns true if a room transition is in progress.
    pub fn is_transitioning(&self) -> bool {
        self.transitioning
    }

    pub fn update(&mut self, dt: f32) {
        match &self.mode {
            CameraMode::Fixed => {}

            CameraMode::Follow {
                deadzone,
                lookahead,
                smoothing,
            } => {
                let deadzone = deadzone.clone();
                let lookahead = *lookahead;
                let smoothing = *smoothing;

                // Calculate lookahead based on target velocity
                let vel = RenderVec2::new(
                    self.target.x - self.prev_target.x,
                    self.target.y - self.prev_target.y,
                );
                let target_lookahead = RenderVec2::new(vel.x * lookahead, vel.y * lookahead);
                self.lookahead_offset = self.lookahead_offset.lerp(target_lookahead, 3.0 * dt);

                // Deadzone: only move camera when target exits the zone
                let cam_relative_x = self.target.x - self.position.x;
                let cam_relative_y = self.target.y - self.position.y;

                let dz_left = deadzone.x;
                let dz_right = deadzone.x + deadzone.w;
                let dz_top = deadzone.y;
                let dz_bottom = deadzone.y + deadzone.h;

                let mut desired = self.position;

                if cam_relative_x < dz_left {
                    desired.x = self.target.x - dz_left;
                } else if cam_relative_x > dz_right {
                    desired.x = self.target.x - dz_right;
                }

                if cam_relative_y < dz_top {
                    desired.y = self.target.y - dz_top;
                } else if cam_relative_y > dz_bottom {
                    desired.y = self.target.y - dz_bottom;
                }

                desired.x += self.lookahead_offset.x;
                desired.y += self.lookahead_offset.y;

                if smoothing > 0.0 {
                    self.position = self.position.lerp(desired, smoothing * dt);
                } else {
                    self.position = desired;
                }
            }

            CameraMode::FollowSmooth { speed } => {
                let speed = *speed;
                self.position = self.position.lerp(self.target, speed * dt);
            }

            CameraMode::ScreenLock {
                room_width,
                room_height,
            } => {
                let rw = *room_width;
                let rh = *room_height;
                self.position.x = (self.target.x / rw).floor() * rw + rw * 0.5;
                self.position.y = (self.target.y / rh).floor() * rh + rh * 0.5;
                self.transitioning = false;
            }

            CameraMode::RoomTransition {
                room_width,
                room_height,
                transition_speed,
            } => {
                let rw = *room_width;
                let rh = *room_height;
                let speed = *transition_speed;

                let target_room_x = (self.target.x / rw).floor() * rw + rw * 0.5;
                let target_room_y = (self.target.y / rh).floor() * rh + rh * 0.5;
                let new_room_target = RenderVec2::new(target_room_x, target_room_y);

                if (new_room_target.x - self.room_target.x).abs() > 0.01
                    || (new_room_target.y - self.room_target.y).abs() > 0.01
                {
                    self.room_target = new_room_target;
                    self.transitioning = true;
                }

                self.position = self.position.lerp(self.room_target, speed * dt);

                if (self.position.x - self.room_target.x).abs() < 0.5
                    && (self.position.y - self.room_target.y).abs() < 0.5
                {
                    self.position = self.room_target;
                    self.transitioning = false;
                }
            }

            CameraMode::BossArena {
                center,
                arena_width,
                arena_height,
            } => {
                let center = *center;
                let aw = *arena_width;
                let ah = *arena_height;

                // Frame the arena: zoom to fit, center on arena
                let zoom_x = self.virtual_width / aw;
                let zoom_y = self.virtual_height / ah;
                self.target_zoom = zoom_x.min(zoom_y);
                self.position = self.position.lerp(center, 3.0 * dt);
            }

            CameraMode::EdgePan {
                follow_speed,
                edge_zone,
                edge_speed,
                max_drift,
            } => {
                let follow_speed = *follow_speed;
                let edge_zone = *edge_zone;
                let edge_speed = *edge_speed;
                let max_drift = *max_drift;

                // Follow target smoothly
                self.position = self.position.lerp(self.target, follow_speed * dt);

                // Edge panning
                let mut pan = RenderVec2::ZERO;
                if self.mouse_norm_x < edge_zone {
                    pan.x = -edge_speed * (1.0 - self.mouse_norm_x / edge_zone);
                } else if self.mouse_norm_x > 1.0 - edge_zone {
                    pan.x = edge_speed * (self.mouse_norm_x - (1.0 - edge_zone)) / edge_zone;
                }
                if self.mouse_norm_y < edge_zone {
                    pan.y = -edge_speed * (1.0 - self.mouse_norm_y / edge_zone);
                } else if self.mouse_norm_y > 1.0 - edge_zone {
                    pan.y = edge_speed * (self.mouse_norm_y - (1.0 - edge_zone)) / edge_zone;
                }

                self.edge_drift.x += pan.x * dt;
                self.edge_drift.y += pan.y * dt;

                // Clamp drift distance
                let drift_dist = (self.edge_drift.x * self.edge_drift.x
                    + self.edge_drift.y * self.edge_drift.y)
                    .sqrt();
                if drift_dist > max_drift {
                    let scale = max_drift / drift_dist;
                    self.edge_drift.x *= scale;
                    self.edge_drift.y *= scale;
                }

                // Decay drift back toward center when mouse is centered
                if pan.x.abs() < 0.01 {
                    self.edge_drift.x *= 1.0 - 2.0 * dt;
                }
                if pan.y.abs() < 0.01 {
                    self.edge_drift.y *= 1.0 - 2.0 * dt;
                }

                self.position.x += self.edge_drift.x;
                self.position.y += self.edge_drift.y;
            }

            CameraMode::FreePan {
                edge_zone,
                pan_speed,
            } => {
                let edge_zone = *edge_zone;
                let pan_speed = *pan_speed;

                let mut pan = RenderVec2::ZERO;
                if self.mouse_norm_x < edge_zone {
                    pan.x = -pan_speed * (1.0 - self.mouse_norm_x / edge_zone);
                } else if self.mouse_norm_x > 1.0 - edge_zone {
                    pan.x = pan_speed * (self.mouse_norm_x - (1.0 - edge_zone)) / edge_zone;
                }
                if self.mouse_norm_y < edge_zone {
                    pan.y = -pan_speed * (1.0 - self.mouse_norm_y / edge_zone);
                } else if self.mouse_norm_y > 1.0 - edge_zone {
                    pan.y = pan_speed * (self.mouse_norm_y - (1.0 - edge_zone)) / edge_zone;
                }

                self.position.x += pan.x * dt;
                self.position.y += pan.y * dt;
            }

            CameraMode::CinematicPan {
                from,
                to,
                duration,
                elapsed,
                easing,
            } => {
                let from = *from;
                let to = *to;
                let duration = *duration;
                let mut elapsed = *elapsed + dt;
                let easing = *easing;

                let t = if duration > 0.0 {
                    (elapsed / duration).min(1.0)
                } else {
                    1.0
                };
                let eased = easing.apply(t);

                self.position = RenderVec2::new(
                    from.x + (to.x - from.x) * eased,
                    from.y + (to.y - from.y) * eased,
                );

                if t >= 1.0 {
                    elapsed = duration;
                }

                // Update elapsed in mode
                self.mode = CameraMode::CinematicPan {
                    from,
                    to,
                    duration,
                    elapsed,
                    easing,
                };
            }
        }

        // Smooth zoom
        if (self.zoom - self.target_zoom).abs() > 0.001 {
            self.zoom += (self.target_zoom - self.zoom) * self.zoom_speed * dt;
        } else {
            self.zoom = self.target_zoom;
        }

        // Clamp to bounds
        if let Some(bounds) = &self.bounds {
            let half_w = (self.virtual_width / self.zoom) * 0.5;
            let half_h = (self.virtual_height / self.zoom) * 0.5;

            self.position.x = self
                .position
                .x
                .clamp(bounds.x + half_w, bounds.x + bounds.w - half_w);
            self.position.y = self
                .position
                .y
                .clamp(bounds.y + half_h, bounds.y + bounds.h - half_h);
        }

        // Update shake
        if self.shake_intensity > 0.1 {
            self.shake_time += dt * 60.0;
            self.shake_offset = RenderVec2::new(
                self.shake_intensity * pseudo_random_f32(self.shake_time),
                self.shake_intensity * pseudo_random_f32(self.shake_time + 100.0),
            );
            self.shake_intensity -= self.shake_decay * dt;
            if self.shake_intensity < 0.0 {
                self.shake_intensity = 0.0;
            }
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
    pub fn screen_to_world(
        &self,
        screen_x: f32,
        screen_y: f32,
        window_width: f32,
        window_height: f32,
    ) -> RenderVec2 {
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
        let mut eff = self.effective_position();
        let half_w = (self.virtual_width / self.zoom) * 0.5;
        let half_h = (self.virtual_height / self.zoom) * 0.5;

        // Snap camera to integer pixels to avoid sub-pixel jitter in pixel-art mode.
        if self.pixel_snap {
            eff.x = eff.x.round();
            eff.y = eff.y.round();
        }

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
        [
            -(right + left) / rml,
            -(top + bottom) / tmb,
            -(far + near) / fmn,
            1.0,
        ],
    ]
}

/// Simple pseudo-random for shake (no determinism requirement for visual effects).
fn pseudo_random_f32(seed: f32) -> f32 {
    let s = (seed * 12.9898).sin() * 43758.5453;
    (s - s.floor()) * 2.0 - 1.0
}
