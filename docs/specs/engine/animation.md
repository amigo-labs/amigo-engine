---
status: done
crate: amigo_animation
depends_on: ["engine/core"]
last_updated: 2026-03-16
---

# Animation System

## Purpose

Sprite-based animation system for the Amigo Engine, driving frame-based animations from Aseprite data.

## Behavior

Sprite animations from Aseprite tags (see assets/pipeline). Frame-based with per-frame duration in ticks. `AnimPlayer` plays `Loop`, `Once` or `PingPong`; `play_animation` uses the animation's own mode (`looping` → Loop, else Once), `advance(&AnimationLibrary)` ticks the current animation by name, `restart` replays a finished one. `DrawContext::draw_animated`/`draw_animated_ex` draw the current frame. Event tracks fire events in each tick's window, including the last tick of a pass (a wrap in `Loop`, the finishing tick in `Once`); in `PingPong` events fire on the forward leg only. Phase 2: skeletal animation for large bosses.
