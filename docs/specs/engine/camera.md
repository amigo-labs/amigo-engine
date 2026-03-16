---
status: draft
crate: amigo_camera
depends_on: ["engine/core"]
last_updated: 2026-03-16
---

# Camera System

## Purpose

Camera management with pre-built patterns and effects for 2D games.

## Behavior

Pre-built patterns: Fixed, Follow (with deadzone + lookahead), FollowSmooth, ScreenLock (Zelda), RoomTransition (Metroidvania), BossArena, CinematicPan.

Effects: shake (configurable decay), zoom (with easing).

Parallax: each tile layer has independent scroll factor.
