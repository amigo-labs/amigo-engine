//! Minimap system: abstracted map view as HUD element.
//!
//! Renders tilemap as colored pixels, entity pins, fog-of-war overlay,
//! and camera viewport indicator. Supports click-to-jump, team-colored
//! pins, and a ping system for RTS/strategy games.

use amigo_core::color::Color;
use amigo_core::ecs::EntityId;
use amigo_core::fog_of_war::{FogOfWarGrid, TileVisibility};
use amigo_core::math::{RenderVec2, SimVec2};
use amigo_core::rect::Rect;
use rustc_hash::FxHashMap;

use crate::camera::Camera;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for minimap appearance and position.
#[derive(Clone, Debug)]
pub struct MinimapConfig {
    /// Screen position in pixels (top-left corner).
    pub screen_pos: RenderVec2,
    /// Size in pixels on screen.
    pub size: (u32, u32),
    /// World region displayed (in tiles).
    pub world_bounds: Rect,
    /// Style options.
    pub style: MinimapStyle,
    /// Whether clicking the minimap moves the camera.
    pub click_to_jump: bool,
}

/// Visual style for the minimap.
#[derive(Clone, Debug)]
pub struct MinimapStyle {
    /// Background color for unexplored areas.
    pub background_color: Color,
    /// Border color (None = no border).
    pub border_color: Option<Color>,
    /// Border width in pixels.
    pub border_width: u32,
    /// Color of the camera viewport indicator.
    pub viewport_indicator_color: Color,
    /// Fog-of-war colors.
    pub fog_hidden_color: Color,
    pub fog_explored_color: Color,
}

impl Default for MinimapStyle {
    fn default() -> Self {
        Self {
            background_color: Color::BLACK,
            border_color: Some(Color::WHITE),
            border_width: 1,
            viewport_indicator_color: Color::WHITE,
            fog_hidden_color: Color::BLACK,
            fog_explored_color: Color::new(0.0, 0.0, 0.0, 0.5),
        }
    }
}

// ---------------------------------------------------------------------------
// Pin types
// ---------------------------------------------------------------------------

/// A marker on the minimap for an entity or point of interest.
#[derive(Clone, Debug)]
pub struct MinimapPin {
    /// Entity this pin tracks (None = static pin).
    pub entity: Option<EntityId>,
    /// Fixed position (only when entity = None).
    pub static_pos: Option<SimVec2>,
    /// Resolved world position (updated by Minimap::update).
    pub world_pos: RenderVec2,
    /// Display type.
    pub pin_type: PinType,
    /// Visible even in fog-of-war.
    pub always_visible: bool,
}

/// How a pin is rendered.
#[derive(Clone, Debug)]
pub enum PinType {
    /// Colored dot (1-3 pixels).
    Dot { color: Color },
    /// Small sprite icon.
    Sprite { name: String },
    /// Directional arrow at minimap edge for off-screen entities.
    Arrow { color: Color },
}

/// Standard team colors for RTS unit pins.
pub const TEAM_COLORS: [Color; 4] = [Color::GREEN, Color::RED, Color::BLUE, Color::YELLOW];

impl MinimapPin {
    /// Create a unit pin with team color.
    pub fn unit(entity: EntityId, team: u8) -> Self {
        let color = TEAM_COLORS
            .get(team as usize)
            .copied()
            .unwrap_or(Color::WHITE);
        Self {
            entity: Some(entity),
            static_pos: None,
            world_pos: RenderVec2::ZERO,
            pin_type: PinType::Dot { color },
            always_visible: false,
        }
    }

    /// Create a static pin at a fixed position.
    pub fn static_pin(pos: SimVec2, pin_type: PinType) -> Self {
        Self {
            entity: None,
            static_pos: Some(pos),
            world_pos: RenderVec2::new(pos.x.to_num(), pos.y.to_num()),
            pin_type,
            always_visible: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Ping system
// ---------------------------------------------------------------------------

/// Temporary marker on the minimap (alert, help request, target).
#[derive(Clone, Debug)]
pub struct MinimapPing {
    /// World position.
    pub position: SimVec2,
    /// Ping color.
    pub color: Color,
    /// Remaining ticks until the ping disappears.
    pub remaining_ticks: u16,
    /// Pulsating animation.
    pub pulse: bool,
}

// ---------------------------------------------------------------------------
// Minimap pixel output (for rendering)
// ---------------------------------------------------------------------------

/// A colored pixel in the minimap output buffer.
#[derive(Clone, Copy, Debug)]
pub struct MinimapPixel {
    pub x: u32,
    pub y: u32,
    pub color: Color,
}

// ---------------------------------------------------------------------------
// Dynamic icon system for Sprite pins
// ---------------------------------------------------------------------------

/// A small icon definition for sprite-type pins.
/// Icons are defined as a grid of colored pixels relative to the pin center.
#[derive(Clone, Debug)]
pub struct SpriteIcon {
    /// Width of the icon in pixels.
    pub width: u32,
    /// Height of the icon in pixels.
    pub height: u32,
    /// Row-major RGBA pixel data. Length must equal `width * height`.
    /// Use `Color::TRANSPARENT` for empty pixels.
    pub pixels: Vec<Color>,
}

impl SpriteIcon {
    /// Create a new sprite icon from pixel data.
    pub fn new(width: u32, height: u32, pixels: Vec<Color>) -> Self {
        debug_assert_eq!(
            pixels.len(),
            (width * height) as usize,
            "SpriteIcon pixel data must match width*height"
        );
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Create a simple single-color diamond icon (good for generic markers).
    pub fn diamond(color: Color) -> Self {
        // 3x3 diamond:  .#.
        //               ###
        //               .#.
        let t = Color::TRANSPARENT;
        Self::new(
            3,
            3,
            vec![t, color, t, color, color, color, t, color, t],
        )
    }

    /// Create a single-color square icon.
    pub fn square(size: u32, color: Color) -> Self {
        Self::new(size, size, vec![color; (size * size) as usize])
    }
}

/// Registry mapping sprite names to icon definitions.
#[derive(Clone, Debug, Default)]
pub struct IconRegistry {
    icons: FxHashMap<String, SpriteIcon>,
}

impl IconRegistry {
    /// Create an empty icon registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a named icon.
    pub fn register(&mut self, name: impl Into<String>, icon: SpriteIcon) {
        self.icons.insert(name.into(), icon);
    }

    /// Look up an icon by name.
    pub fn get(&self, name: &str) -> Option<&SpriteIcon> {
        self.icons.get(name)
    }
}

// ---------------------------------------------------------------------------
// Minimap
// ---------------------------------------------------------------------------

/// Main minimap system.
pub struct Minimap {
    /// Configuration.
    pub config: MinimapConfig,
    /// Entity pins.
    pins: Vec<MinimapPin>,
    /// Active pings.
    pings: Vec<MinimapPing>,
    /// Icon registry for sprite-type pins.
    icon_registry: IconRegistry,
}

impl Minimap {
    /// Create a new minimap with the given configuration.
    pub fn new(config: MinimapConfig) -> Self {
        Self {
            config,
            pins: Vec::new(),
            pings: Vec::new(),
            icon_registry: IconRegistry::new(),
        }
    }

    /// Returns a mutable reference to the icon registry for registering sprite icons.
    pub fn icon_registry_mut(&mut self) -> &mut IconRegistry {
        &mut self.icon_registry
    }

    /// Returns a reference to the icon registry.
    pub fn icon_registry(&self) -> &IconRegistry {
        &self.icon_registry
    }

    /// Add a pin.
    pub fn add_pin(&mut self, pin: MinimapPin) {
        self.pins.push(pin);
    }

    /// Remove all pins for an entity.
    pub fn remove_pins_for(&mut self, entity: EntityId) {
        self.pins.retain(|p| p.entity != Some(entity));
    }

    /// Remove all pins.
    pub fn clear_pins(&mut self) {
        self.pins.clear();
    }

    /// Add a ping.
    pub fn add_ping(&mut self, ping: MinimapPing) {
        self.pings.push(ping);
    }

    /// Tick pings: decrement timers, remove expired.
    pub fn tick_pings(&mut self) {
        for ping in &mut self.pings {
            ping.remaining_ticks = ping.remaining_ticks.saturating_sub(1);
        }
        self.pings.retain(|p| p.remaining_ticks > 0);
    }

    /// Convert a screen click position to world coordinates.
    /// Returns None if click_to_jump is disabled or click is outside minimap.
    pub fn screen_to_world(&self, screen_pos: RenderVec2) -> Option<RenderVec2> {
        if !self.config.click_to_jump {
            return None;
        }
        let sx = screen_pos.x - self.config.screen_pos.x;
        let sy = screen_pos.y - self.config.screen_pos.y;
        let (mw, mh) = self.config.size;
        if sx < 0.0 || sy < 0.0 || sx >= mw as f32 || sy >= mh as f32 {
            return None;
        }
        let frac_x = sx / mw as f32;
        let frac_y = sy / mh as f32;
        let wb = &self.config.world_bounds;
        Some(RenderVec2::new(wb.x + frac_x * wb.w, wb.y + frac_y * wb.h))
    }

    /// Update pin positions from ECS data.
    pub fn update(&mut self, positions: &[(EntityId, SimVec2)]) {
        for pin in &mut self.pins {
            if let Some(entity) = pin.entity {
                if let Some((_, pos)) = positions.iter().find(|(id, _)| *id == entity) {
                    pin.world_pos = RenderVec2::new(pos.x.to_num(), pos.y.to_num());
                }
            } else if let Some(pos) = pin.static_pos {
                pin.world_pos = RenderVec2::new(pos.x.to_num(), pos.y.to_num());
            }
        }
    }

    /// Convert a world position to minimap pixel coordinates.
    pub fn world_to_minimap(&self, world_pos: RenderVec2) -> (f32, f32) {
        let wb = &self.config.world_bounds;
        let frac_x = (world_pos.x - wb.x) / wb.w;
        let frac_y = (world_pos.y - wb.y) / wb.h;
        let (mw, mh) = self.config.size;
        (frac_x * mw as f32, frac_y * mh as f32)
    }

    /// Generate the minimap pixel buffer for rendering.
    /// `tile_colors` maps tile index to color (game provides this mapping).
    /// `tiles` is the flat tile array (row-major).
    /// `tile_width`/`tile_height` is the tilemap dimensions in tiles.
    pub fn generate_pixels(
        &self,
        tiles: &[u32],
        tile_width: u32,
        tile_height: u32,
        tile_colors: &dyn Fn(u32) -> Color,
        fog: Option<&FogOfWarGrid>,
    ) -> Vec<MinimapPixel> {
        let mut pixels = Vec::new();
        let (mw, mh) = self.config.size;
        let wb = &self.config.world_bounds;

        for my in 0..mh {
            for mx in 0..mw {
                // Map minimap pixel to tile coordinates
                let frac_x = mx as f32 / mw as f32;
                let frac_y = my as f32 / mh as f32;
                let tile_x = (wb.x + frac_x * wb.w) as i32;
                let tile_y = (wb.y + frac_y * wb.h) as i32;

                if tile_x < 0
                    || tile_y < 0
                    || tile_x >= tile_width as i32
                    || tile_y >= tile_height as i32
                {
                    pixels.push(MinimapPixel {
                        x: mx,
                        y: my,
                        color: self.config.style.background_color,
                    });
                    continue;
                }

                let tile_idx = (tile_y as u32 * tile_width + tile_x as u32) as usize;
                let tile_id = tiles.get(tile_idx).copied().unwrap_or(0);
                let mut color = tile_colors(tile_id);

                // Apply fog-of-war overlay
                if let Some(fog) = fog {
                    let vis = fog.visibility_at(tile_x, tile_y);
                    match vis {
                        TileVisibility::Hidden => {
                            color = self.config.style.fog_hidden_color;
                        }
                        TileVisibility::Explored => {
                            // Blend with fog_explored_color
                            let fc = self.config.style.fog_explored_color;
                            color = Color::new(
                                color.r * (1.0 - fc.a) + fc.r * fc.a,
                                color.g * (1.0 - fc.a) + fc.g * fc.a,
                                color.b * (1.0 - fc.a) + fc.b * fc.a,
                                1.0,
                            );
                        }
                        TileVisibility::Visible => {}
                    }
                }

                pixels.push(MinimapPixel {
                    x: mx,
                    y: my,
                    color,
                });
            }
        }

        // Render pins
        self.render_pins_to_pixels(&mut pixels, fog);

        // Render pings
        self.render_pings_to_pixels(&mut pixels);

        pixels
    }

    /// Determine fog-based opacity for a pin at its current world position.
    /// Returns `None` if the pin should not be rendered at all (hidden and not always_visible).
    /// Returns `Some(alpha)` where alpha is 1.0 for visible, 0.5 for explored.
    fn pin_fog_opacity(&self, pin: &MinimapPin, fog: Option<&FogOfWarGrid>) -> Option<f32> {
        if pin.always_visible {
            return Some(1.0);
        }
        if let Some(fog) = fog {
            let tx = pin.world_pos.x as i32;
            let ty = pin.world_pos.y as i32;
            let vis = fog.visibility_at(tx, ty);
            match vis {
                TileVisibility::Hidden => None,
                TileVisibility::Explored => Some(0.5),
                TileVisibility::Visible => Some(1.0),
            }
        } else {
            Some(1.0)
        }
    }

    /// Render pins into the pixel buffer, handling Dot, Sprite, and Arrow types.
    fn render_pins_to_pixels(&self, pixels: &mut Vec<MinimapPixel>, fog: Option<&FogOfWarGrid>) {
        let (mw, mh) = self.config.size;

        for pin in &self.pins {
            let opacity = match self.pin_fog_opacity(pin, fog) {
                Some(o) => o,
                None => continue, // Hidden — skip
            };

            let (px, py) = self.world_to_minimap(pin.world_pos);

            match &pin.pin_type {
                PinType::Dot { color } => {
                    let px = px as u32;
                    let py = py as u32;
                    if px < mw && py < mh {
                        let c = Color::new(color.r, color.g, color.b, color.a * opacity);
                        pixels.push(MinimapPixel { x: px, y: py, color: c });
                    }
                }

                PinType::Sprite { name } => {
                    if let Some(icon) = self.icon_registry.get(name) {
                        // Center the icon on the pin position
                        let ox = px as i32 - (icon.width as i32 / 2);
                        let oy = py as i32 - (icon.height as i32 / 2);
                        for iy in 0..icon.height {
                            for ix in 0..icon.width {
                                let sx = ox + ix as i32;
                                let sy = oy + iy as i32;
                                if sx >= 0 && sy >= 0 && (sx as u32) < mw && (sy as u32) < mh {
                                    let idx = (iy * icon.width + ix) as usize;
                                    let ic = icon.pixels[idx];
                                    // Skip transparent icon pixels
                                    if ic.a > 0.0 {
                                        let c =
                                            Color::new(ic.r, ic.g, ic.b, ic.a * opacity);
                                        pixels.push(MinimapPixel {
                                            x: sx as u32,
                                            y: sy as u32,
                                            color: c,
                                        });
                                    }
                                }
                            }
                        }
                    } else {
                        // Fallback: render as white dot if icon not found
                        let px = px as u32;
                        let py = py as u32;
                        if px < mw && py < mh {
                            let c = Color::new(1.0, 1.0, 1.0, opacity);
                            pixels.push(MinimapPixel { x: px, y: py, color: c });
                        }
                    }
                }

                PinType::Arrow { color } => {
                    // Directional arrow at minimap edge for off-screen entities.
                    // Clamp pin position to minimap edge and draw a small arrow.
                    let in_bounds = px >= 0.0
                        && py >= 0.0
                        && (px as u32) < mw
                        && (py as u32) < mh;

                    if in_bounds {
                        // Entity is on-screen within minimap — render as a dot
                        let c = Color::new(color.r, color.g, color.b, color.a * opacity);
                        pixels.push(MinimapPixel {
                            x: px as u32,
                            y: py as u32,
                            color: c,
                        });
                    } else {
                        // Clamp to minimap edge
                        let cx = px.clamp(0.0, (mw - 1) as f32) as u32;
                        let cy = py.clamp(0.0, (mh - 1) as f32) as u32;
                        let c = Color::new(color.r, color.g, color.b, color.a * opacity);
                        // Draw a 3-pixel arrow indicator at the edge
                        pixels.push(MinimapPixel { x: cx, y: cy, color: c });
                        // Add perpendicular pixels for visibility
                        let on_left = cx == 0;
                        let on_right = cx == mw - 1;
                        let on_top = cy == 0;
                        let on_bottom = cy == mh - 1;
                        if on_left || on_right {
                            // Vertical arrow tail
                            if cy > 0 {
                                pixels.push(MinimapPixel { x: cx, y: cy - 1, color: c });
                            }
                            if cy + 1 < mh {
                                pixels.push(MinimapPixel { x: cx, y: cy + 1, color: c });
                            }
                        }
                        if on_top || on_bottom {
                            // Horizontal arrow tail
                            if cx > 0 {
                                pixels.push(MinimapPixel { x: cx - 1, y: cy, color: c });
                            }
                            if cx + 1 < mw {
                                pixels.push(MinimapPixel { x: cx + 1, y: cy, color: c });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Render pings into the pixel buffer.
    fn render_pings_to_pixels(&self, pixels: &mut Vec<MinimapPixel>) {
        let (mw, mh) = self.config.size;
        for ping in &self.pings {
            let pos = RenderVec2::new(ping.position.x.to_num(), ping.position.y.to_num());
            let (px, py) = self.world_to_minimap(pos);
            let px = px as u32;
            let py = py as u32;
            if px < mw && py < mh {
                pixels.push(MinimapPixel {
                    x: px,
                    y: py,
                    color: ping.color,
                });
                // Pulse: render a 3x3 cross
                if ping.pulse {
                    for &(dx, dy) in &[(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                        let nx = px as i32 + dx;
                        let ny = py as i32 + dy;
                        if nx >= 0 && ny >= 0 && (nx as u32) < mw && (ny as u32) < mh {
                            pixels.push(MinimapPixel {
                                x: nx as u32,
                                y: ny as u32,
                                color: ping.color,
                            });
                        }
                    }
                }
            }
        }
    }

    /// Render a border around the minimap into the pixel buffer.
    /// Called by [`render`] after generating tile/pin/ping pixels.
    fn render_border_to_pixels(&self, pixels: &mut Vec<MinimapPixel>) {
        let border_color = match self.config.style.border_color {
            Some(c) => c,
            None => return,
        };
        let bw = self.config.style.border_width;
        if bw == 0 {
            return;
        }
        let (mw, mh) = self.config.size;

        for b in 0..bw {
            // Top and bottom rows
            for mx in 0..mw {
                pixels.push(MinimapPixel { x: mx, y: b, color: border_color });
                if mh > b {
                    pixels.push(MinimapPixel { x: mx, y: mh - 1 - b, color: border_color });
                }
            }
            // Left and right columns (excluding corners already drawn)
            for my in bw..(mh.saturating_sub(bw)) {
                pixels.push(MinimapPixel { x: b, y: my, color: border_color });
                if mw > b {
                    pixels.push(MinimapPixel { x: mw - 1 - b, y: my, color: border_color });
                }
            }
        }
    }

    /// Render the camera viewport indicator as a colored rectangle outline on the minimap.
    fn render_viewport_to_pixels(
        &self,
        pixels: &mut Vec<MinimapPixel>,
        camera_pos: RenderVec2,
        camera_view_size: RenderVec2,
    ) {
        let color = self.config.style.viewport_indicator_color;
        let (mw, mh) = self.config.size;

        let top_left = RenderVec2::new(
            camera_pos.x - camera_view_size.x * 0.5,
            camera_pos.y - camera_view_size.y * 0.5,
        );
        let bottom_right = RenderVec2::new(
            camera_pos.x + camera_view_size.x * 0.5,
            camera_pos.y + camera_view_size.y * 0.5,
        );

        let (x0f, y0f) = self.world_to_minimap(top_left);
        let (x1f, y1f) = self.world_to_minimap(bottom_right);

        // Clamp to minimap bounds
        let x0 = (x0f as i32).max(0) as u32;
        let y0 = (y0f as i32).max(0) as u32;
        let x1 = ((x1f as i32).max(0) as u32).min(mw.saturating_sub(1));
        let y1 = ((y1f as i32).max(0) as u32).min(mh.saturating_sub(1));

        // Top edge
        for mx in x0..=x1 {
            pixels.push(MinimapPixel { x: mx, y: y0, color });
        }
        // Bottom edge
        if y1 != y0 {
            for mx in x0..=x1 {
                pixels.push(MinimapPixel { x: mx, y: y1, color });
            }
        }
        // Left edge
        for my in (y0 + 1)..y1 {
            pixels.push(MinimapPixel { x: x0, y: my, color });
        }
        // Right edge
        if x1 != x0 {
            for my in (y0 + 1)..y1 {
                pixels.push(MinimapPixel { x: x1, y: my, color });
            }
        }
    }

    /// Generate viewport indicator rectangle (camera position on minimap).
    /// Returns `(x, y, width, height)` in screen-space pixels.
    pub fn viewport_rect(
        &self,
        camera_pos: RenderVec2,
        camera_view_size: RenderVec2,
    ) -> (f32, f32, f32, f32) {
        let (cx, cy) = self.world_to_minimap(RenderVec2::new(
            camera_pos.x - camera_view_size.x * 0.5,
            camera_pos.y - camera_view_size.y * 0.5,
        ));
        let (cw, ch) = self.world_to_minimap(camera_view_size);
        (
            self.config.screen_pos.x + cx,
            self.config.screen_pos.y + cy,
            cw,
            ch,
        )
    }

    /// Full render pass: generates tile pixels, pins, pings, viewport indicator,
    /// and border. This is the primary entry point for minimap rendering.
    ///
    /// `tile_colors` maps a tile ID to its minimap color.
    /// `tiles` is the flat row-major tile array.
    /// `tile_width`/`tile_height` is the tilemap size in tiles.
    /// `fog` is the optional fog-of-war grid.
    /// `main_camera` is the main game camera (used for viewport indicator).
    pub fn render(
        &self,
        tiles: &[u32],
        tile_width: u32,
        tile_height: u32,
        tile_colors: &dyn Fn(u32) -> Color,
        fog: Option<&FogOfWarGrid>,
        main_camera: &Camera,
    ) -> Vec<MinimapPixel> {
        let mut pixels = self.generate_pixels(tiles, tile_width, tile_height, tile_colors, fog);

        // Viewport indicator from main camera
        let view = main_camera.view_rect();
        self.render_viewport_to_pixels(
            &mut pixels,
            main_camera.effective_position(),
            RenderVec2::new(view.w, view.h),
        );

        // Border on top of everything
        self.render_border_to_pixels(&mut pixels);

        pixels
    }

    /// Convert the minimap pixel buffer into a flat RGBA byte array suitable for
    /// uploading to a GPU texture. Pixels are composited in order (later pixels
    /// overwrite earlier ones at the same coordinate).
    pub fn pixels_to_rgba(&self, pixels: &[MinimapPixel]) -> Vec<u8> {
        let (mw, mh) = self.config.size;
        let size = (mw as usize) * (mh as usize);
        let mut buf = vec![0u8; size * 4];

        for px in pixels {
            let idx = ((px.y as usize) * (mw as usize) + (px.x as usize)) * 4;
            if idx + 3 < buf.len() {
                let c = &px.color;
                if c.a >= 1.0 {
                    // Opaque: overwrite
                    buf[idx] = (c.r * 255.0) as u8;
                    buf[idx + 1] = (c.g * 255.0) as u8;
                    buf[idx + 2] = (c.b * 255.0) as u8;
                    buf[idx + 3] = 255;
                } else if c.a > 0.0 {
                    // Alpha blend over existing
                    let dst_r = buf[idx] as f32 / 255.0;
                    let dst_g = buf[idx + 1] as f32 / 255.0;
                    let dst_b = buf[idx + 2] as f32 / 255.0;
                    let a = c.a;
                    buf[idx] = ((c.r * a + dst_r * (1.0 - a)) * 255.0) as u8;
                    buf[idx + 1] = ((c.g * a + dst_g * (1.0 - a)) * 255.0) as u8;
                    buf[idx + 2] = ((c.b * a + dst_b * (1.0 - a)) * 255.0) as u8;
                    buf[idx + 3] = 255;
                }
            }
        }

        buf
    }

    /// Read-only access to the current pins.
    pub fn pins(&self) -> &[MinimapPin] {
        &self.pins
    }

    /// Read-only access to the current pings.
    pub fn pings(&self) -> &[MinimapPing] {
        &self.pings
    }

    /// Number of active pins.
    pub fn pin_count(&self) -> usize {
        self.pins.len()
    }

    /// Number of active pings.
    pub fn ping_count(&self) -> usize {
        self.pings.len()
    }

    /// Check if a screen position falls within the minimap area.
    pub fn contains_screen_pos(&self, screen_pos: RenderVec2) -> bool {
        let sx = screen_pos.x - self.config.screen_pos.x;
        let sy = screen_pos.y - self.config.screen_pos.y;
        let (mw, mh) = self.config.size;
        sx >= 0.0 && sy >= 0.0 && sx < mw as f32 && sy < mh as f32
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MinimapConfig {
        MinimapConfig {
            screen_pos: RenderVec2::new(0.0, 0.0),
            size: (100, 100),
            world_bounds: Rect::new(0.0, 0.0, 50.0, 50.0),
            style: MinimapStyle::default(),
            click_to_jump: true,
        }
    }

    #[test]
    fn screen_to_world_center() {
        let mm = Minimap::new(test_config());
        let world = mm.screen_to_world(RenderVec2::new(50.0, 50.0)).unwrap();
        assert!((world.x - 25.0).abs() < 0.5);
        assert!((world.y - 25.0).abs() < 0.5);
    }

    #[test]
    fn screen_to_world_outside() {
        let mm = Minimap::new(test_config());
        assert!(mm.screen_to_world(RenderVec2::new(200.0, 200.0)).is_none());
    }

    #[test]
    fn screen_to_world_disabled() {
        let mut config = test_config();
        config.click_to_jump = false;
        let mm = Minimap::new(config);
        assert!(mm.screen_to_world(RenderVec2::new(50.0, 50.0)).is_none());
    }

    #[test]
    fn pin_management() {
        let mut mm = Minimap::new(test_config());
        let e1 = EntityId::from_raw(1, 0);
        let e2 = EntityId::from_raw(2, 0);
        mm.add_pin(MinimapPin::unit(e1, 0));
        mm.add_pin(MinimapPin::unit(e2, 1));
        assert_eq!(mm.pin_count(), 2);
        mm.remove_pins_for(e1);
        assert_eq!(mm.pin_count(), 1);
        mm.clear_pins();
        assert_eq!(mm.pin_count(), 0);
    }

    #[test]
    fn ping_lifecycle() {
        let mut mm = Minimap::new(test_config());
        mm.add_ping(MinimapPing {
            position: SimVec2::ZERO,
            color: Color::RED,
            remaining_ticks: 3,
            pulse: false,
        });
        assert_eq!(mm.ping_count(), 1);
        mm.tick_pings();
        mm.tick_pings();
        assert_eq!(mm.ping_count(), 1);
        mm.tick_pings();
        assert_eq!(mm.ping_count(), 0); // Expired
    }

    #[test]
    fn world_to_minimap_conversion() {
        let mm = Minimap::new(test_config());
        let (mx, my) = mm.world_to_minimap(RenderVec2::new(25.0, 25.0));
        assert!((mx - 50.0).abs() < 0.5);
        assert!((my - 50.0).abs() < 0.5);
    }

    #[test]
    fn generate_pixels_basic() {
        let mm = Minimap::new(MinimapConfig {
            screen_pos: RenderVec2::ZERO,
            size: (4, 4),
            world_bounds: Rect::new(0.0, 0.0, 4.0, 4.0),
            style: MinimapStyle::default(),
            click_to_jump: false,
        });
        let tiles = vec![0u32; 16]; // 4x4 all tile 0
        let pixels = mm.generate_pixels(&tiles, 4, 4, &|_| Color::GREEN, None);
        assert_eq!(pixels.len(), 16);
        assert!((pixels[0].color.g - 1.0).abs() < 0.01); // Green
    }

    #[test]
    fn viewport_rect_center() {
        let mm = Minimap::new(test_config());
        let (x, y, w, h) =
            mm.viewport_rect(RenderVec2::new(25.0, 25.0), RenderVec2::new(10.0, 10.0));
        assert!((x - 40.0).abs() < 1.0); // (25-5)/50 * 100 = 40
        assert!((w - 20.0).abs() < 1.0); // 10/50 * 100 = 20
        let _ = (y, h);
    }
}
