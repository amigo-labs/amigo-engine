# Input System

> Status: draft
> Crate: amigo_input
> Depends on: [engine/core](../engine/core.md)
> Last updated: 2026-03-16

## Zweck

Unified input abstraction supporting keyboard, mouse, and gamepad with action mapping.

## Verhalten

Abstract action mapping (RON-defined). Keyboard, mouse, gamepad (gilrs). API: `pressed()`, `released()`, `held()`, `axis()`, `mouse_pos()`, `mouse_world_pos()`.
