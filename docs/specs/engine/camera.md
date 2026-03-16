# Camera System

> Status: draft
> Crate: amigo_camera
> Depends on: [engine/core](../engine/core.md)
> Last updated: 2026-03-16

## Zweck

Camera management with pre-built patterns and effects for 2D games.

## Verhalten

Pre-built patterns: Fixed, Follow (with deadzone + lookahead), FollowSmooth, ScreenLock (Zelda), RoomTransition (Metroidvania), BossArena, CinematicPan.

Effects: shake (configurable decay), zoom (with easing).

Parallax: each tile layer has independent scroll factor.
