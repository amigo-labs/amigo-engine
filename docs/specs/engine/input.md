---
status: draft
crate: amigo_input
depends_on: ["engine/core"]
last_updated: 2026-03-16
---

# Input System

## Purpose

Unified input abstraction supporting keyboard, mouse, and gamepad with action mapping.

## Behavior

Abstract action mapping (RON-defined). Keyboard, mouse, gamepad (gilrs). API: `pressed()`, `released()`, `held()`, `axis()`, `mouse_pos()`, `mouse_world_pos()`.
