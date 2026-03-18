---
status: done
crate: amigo_camera
depends_on: ["engine/core"]
last_updated: 2026-03-18
---

# Camera System

## Purpose

Camera management with pre-built patterns and effects for 2D games.

## Behavior

### Camera Modes

```rust
pub enum CameraMode {
    /// Camera is at a fixed position, does not move.
    Fixed { position: RenderVec2 },

    /// Camera follows a target entity with deadzone and look-ahead.
    /// Used by: platformer, roguelike, metroidvania (default in-room).
    Follow {
        target: EntityId,
        deadzone: Rect,          // target can move this much before camera follows
        look_ahead: f32,         // pixels to shift in facing direction
        vertical_snap: bool,     // false = smooth vertical catch-up (platformer)
    },

    /// Camera follows target with configurable smoothing (lerp factor).
    FollowSmooth {
        target: EntityId,
        smoothing: f32,          // 0.0 = no follow, 1.0 = instant snap
    },

    /// Camera locked to a fixed rectangle (one screen = one room).
    /// Used by: puzzle, shmup (fixed arena), Zelda-style rooms.
    ScreenLock { bounds: Rect },

    /// Camera slides from one room to another over a duration.
    /// Used by: metroidvania room transitions.
    /// During transition, player input is suppressed.
    RoomTransition {
        from_bounds: Rect,
        to_bounds: Rect,
        progress: f32,           // 0.0 = at from, 1.0 = at to
        duration_secs: f32,      // default: 0.5
        easing: EasingType,      // default: EaseInOut
    },

    /// Camera locked to a boss arena rectangle.
    /// Used by: metroidvania boss fights, shmup boss phases.
    /// Similar to ScreenLock but with entrance/exit transitions.
    BossArena {
        arena_bounds: Rect,
        /// If true, smoothly transition from current pos to arena center on enter.
        enter_transition: bool,
    },

    /// Free pan via input (WASD/middle-mouse drag). Used by: RTS, city builder.
    FreePan {
        speed: f32,              // pixels per second
        bounds: Option<Rect>,    // clamp to world bounds
    },

    /// Camera pans when cursor is near screen edges. Used by: RTS.
    /// Typically combined with FreePan.
    EdgePan {
        edge_width: f32,         // pixels from screen edge to trigger
        speed: f32,              // pan speed
    },

    /// Cinematic camera pan to a target position over a duration.
    /// Used by: visual novel dramatic scenes, ability unlock showcases.
    CinematicPan {
        target: RenderVec2,
        duration_secs: f32,
        easing: EasingType,
        hold_secs: f32,          // hold at target before returning
    },
}
```

### Camera Effects

Effects: shake (configurable decay + intensity), zoom (with easing).

Parallax: each tile layer has independent scroll factor.

---

## Extensions (Sandbox/God Sim)

> Added per gap analysis (`05-sandbox-godsim-gaps.md`). Camera implementation is in `crates/amigo_render/src/camera.rs`.

### ZoomCamera: Continuous Zoom

The existing `Camera` struct already supports continuous zoom via `zoom`/`target_zoom` fields with smooth interpolation. For God Sim scenarios (zooming from a single person to the entire world map), use `set_zoom()` for smooth transitions or `set_zoom_immediate()` for instant changes.

```rust
// crates/amigo_render/src/camera.rs

pub struct Camera {
    pub zoom: f32,
    pub target_zoom: f32,
    pub zoom_speed: f32,       // Interpolation speed (default: 5.0)
    // ...
}

impl Camera {
    /// Set zoom with smooth transition.
    pub fn set_zoom(&mut self, zoom: f32);

    /// Set zoom immediately without transition.
    pub fn set_zoom_immediate(&mut self, zoom: f32);

    /// Get the visible area in world coordinates (accounts for zoom).
    pub fn view_rect(&self) -> Rect;
}
```

The zoom is applied in `projection_matrix()` -- the visible half-extents are divided by `self.zoom`, so `zoom = 0.1` shows 10x the area (world map), `zoom = 4.0` shows 1/4 the area (close-up). The `bounds` field can clamp the camera so it never scrolls outside the world.

**God Sim usage pattern:**
- Scroll wheel adjusts `set_zoom()` between e.g. `0.1` (world overview) and `4.0` (street level).
- Combine with `CameraMode::FreePan` or `CameraMode::EdgePan` for RTS-style navigation.

### MinimapCamera: Second Viewport

For minimap rendering, create a second `Camera` instance with `zoom` set to show the full world. Render the scene twice per frame:

1. Main camera: normal view with full detail.
2. Minimap camera: zoomed-out view rendered to a small offscreen texture.

The existing `begin_frame()` / `end_frame()` pattern supports multiple render passes. The `projection_matrix()` method produces the correct orthographic projection for any zoom level.

```rust
// Minimap pattern (game code, not engine struct):
let mut minimap_cam = Camera::new(minimap_width, minimap_height);
minimap_cam.set_zoom_immediate(0.05); // Show entire world
minimap_cam.position = world_center;
// Render to offscreen texture using minimap_cam.projection_matrix()
```

### LOD Hint

When the camera is zoomed far out (`zoom < 0.5`), the renderer should reduce detail:

- Skip particle effects and small decorations.
- Use simplified sprite variants (fewer animation frames).
- Reduce tilemap detail (skip decoration layers).

The `Camera::view_rect()` method returns the visible world area. Game code can use `camera.zoom` to decide LOD level:

```
zoom >= 1.0  -> Full detail
zoom >= 0.5  -> Skip particles, reduce animation frames
zoom >= 0.2  -> Simplified sprites, skip decoration layers
zoom <  0.2  -> Icon-only mode (God Sim overview)
```

This is a game-side policy decision, not enforced by the engine. The camera provides the zoom value; the game decides what to render.
