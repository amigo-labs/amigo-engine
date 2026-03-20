---
number: "0014"
title: Hierarchische Szenen-Komposition
status: proposed
date: 2026-03-20
---

# ADR-0014: Hierarchische Szenen-Komposition

## Status

proposed

## Context

Scene management lives in `crates/amigo_scene/src/lib.rs`. The `SceneManager` (line 56) holds a flat `stack: Vec<Box<dyn Scene>>` where only the topmost scene receives `update()` and `draw()` calls (lines 92, 116). Scenes implement the `Scene` trait (line 35) with lifecycle hooks `on_enter` / `on_pause` / `on_resume` / `on_exit` and must return a `SceneAction` from `update()` (line 49). The available actions are `Continue`, `Push`, `Pop`, `Replace`, and `Quit` (lines 7-16).

The current model has three limitations:

1. **No parallel scenes**: When a scene is pushed (e.g., a pause menu over gameplay), the underlying scene is paused (`on_pause()` at line 67) and does not receive `update()` or `draw()` calls. There is no way to render the gameplay scene dimmed behind the pause menu, or to run an ambient animation while a dialogue overlay is active. The `draw()` method (line 116) only draws `stack.last()`.

2. **Flat ECS scope**: All scenes share a single `World` instance on `GameContext` (`crates/amigo_engine/src/engine.rs`, line 375 via `game_ctx`). A pause-menu scene and the gameplay scene operate on the same entity pool. The `StateScoped(u32)` tag component (`crates/amigo_core/src/ecs/world.rs`, line 69) provides basic cleanup via `world.cleanup_state(state_id)` (line 211), but there is no isolation -- a menu scene can accidentally query gameplay entities, and spawning UI entities pollutes the gameplay world.

3. **No nested composition**: A cutscene that contains a mini-game (e.g., a fishing mini-game inside an RPG) must either be flattened into the main scene stack or use ad-hoc state machines. There is no concept of a scene owning sub-scenes with independent lifecycles.

The engine's main loop (`engine.rs`, lines 706-719) calls `self.game.update(&mut state.game_ctx)` which returns a `SceneAction`, and then flushes the world (line 731). The `World::flush()` method (line 227 in `world.rs`) processes pending despawns across all SparseSet fields (lines 230-248) and all dynamic component storages (lines 237-239). Creating a sub-world would require either a separate `World` instance or a scoping mechanism within the existing world.

Scene transitions are handled by `Transition` in `crates/amigo_scene/src/transition.rs` which provides fade, slide, wipe, and cut animations with a midpoint swap trigger. Transitions operate between two scenes on the stack and would need to work with nested scenes as well.

## Decision

Introduce **hierarchical scene composition** behind the `scene_v2` feature flag. The design adds:

1. **`SceneNode` tree replacing flat stack**: Replace `Vec<Box<dyn Scene>>` with a tree of `SceneNode`s. Each node holds a `Box<dyn Scene>`, a `Vec<SceneNode>` of children, and a `SceneConfig` controlling update/draw behavior. The root node is an implicit container. Children are rendered in order (bottom to top) so overlay scenes naturally draw on top.

2. **Scene update modes**: Each `SceneNode` has a `SceneConfig`:
   - `update_mode: UpdateMode` -- `Active` (receives update), `Paused` (skipped), `Background` (receives update with a flag indicating it is not the focus).
   - `draw_mode: DrawMode` -- `Visible`, `Hidden`, `Transparent(f32)` (drawn with alpha dimming for overlay effects).
   - `owns_world: bool` -- if true, the scene gets its own `World` sub-scope (see below).

3. **Sub-world scoping** (optional per scene): When `owns_world: true`, the scene node receives a separate `World` instance. The sub-world is created empty and the scene populates it in `on_enter()`. The parent world remains untouched. This avoids the `StateScoped` workaround for menu/overlay entities. Crucially, the sub-world does **not** duplicate the parent's data -- it is a fresh `World::new()` with its own entity arena, SparseSet fields, and dynamic component storage.

4. **Extended `SceneAction`**: Add new variants to support the tree model:
   ```rust
   pub enum SceneAction {
       Continue,
       Push(Box<dyn SceneFactory>),
       PushOverlay(Box<dyn SceneFactory>, SceneConfig),
       Pop,
       Replace(Box<dyn SceneFactory>),
       Quit,
   }
   ```
   `PushOverlay` pushes a child scene as a sibling of the current scene's children with the given config, allowing the parent to continue updating and drawing.

5. **Scene-aware update loop**: The `SceneManager::update()` method walks the tree depth-first. For each node with `UpdateMode::Active` or `UpdateMode::Background`, it calls `scene.update()`. If a node has `owns_world: true`, it temporarily swaps `game_ctx.world` with the node's sub-world before calling update, and swaps back after. This keeps the `Scene` trait API unchanged -- scenes still receive `&mut GameContext` and do not need to know about sub-worlds.

### Alternatives Considered

1. **Entity namespacing within a single World**: Instead of sub-worlds, tag all entities with a `SceneId` component and filter queries by scene scope. Rejected because: (a) every query in every scene would need a `.with(&scene_scope)` filter, which is error-prone and verbose, (b) it does not prevent accidental cross-scene access, and (c) it pollutes the entity ID space, making `entity_count()` misleading.

2. **Full ECS duplication (fork the World)**: Clone the entire `World` when entering a sub-scene, allowing the sub-scene to modify entities freely and discarding the clone on exit. Rejected because cloning all SparseSet data for a world with thousands of entities is expensive (violates the abort criterion of "not duplicating the entire ECS"). The sub-world approach creates a fresh, empty world, which is O(1).

## Migration Path

1. **Introduce `SceneNode` and `SceneConfig`** -- Add `pub struct SceneNode { scene: Box<dyn Scene>, children: Vec<SceneNode>, config: SceneConfig, sub_world: Option<World> }` and `pub struct SceneConfig { pub update_mode: UpdateMode, pub draw_mode: DrawMode, pub owns_world: bool }` in `crates/amigo_scene/src/lib.rs`. Refactor `SceneManager` to hold a `Vec<SceneNode>` (top-level nodes, functionally identical to the current stack for now). Keep the existing `push()` / `pop()` / `replace()` methods working by treating them as operations on the top-level list. Verify: `cargo test -p amigo_scene` passes unchanged; the existing `Scene` trait and `SceneAction` enum are unmodified; `SceneManager::depth()` returns the same value.

2. **Implement tree-based update and draw traversal** -- Change `SceneManager::update()` to walk the node tree. For each node, check `config.update_mode`. If `Active` or `Background`, call `scene.update()`. If a node has `sub_world: Some(w)`, swap `game_ctx.world` with `w` before the call and swap back after. Change `SceneManager::draw()` to walk all nodes with `DrawMode::Visible` or `Transparent(alpha)` in depth-first order. Verify: push a gameplay scene, then push an overlay scene with `UpdateMode::Active` and `DrawMode::Transparent(0.5)`. Both scenes' `update()` and `draw()` are called each frame. The gameplay scene renders at 50% opacity behind the overlay.

3. (rough) Add `PushOverlay` variant to `SceneAction`. When a scene returns `PushOverlay(factory, config)`, the `SceneManager` adds the new scene as a child of the current node rather than pushing to the top-level stack. The parent scene's `config.update_mode` is set to `Background` (not `Paused` as in the current `push()`).

4. (rough) Implement sub-world creation. When a `SceneNode` is created with `owns_world: true`, allocate a `World::new()` and store it in `sub_world`. The scene's `on_enter()` populates it. On `on_exit()`, the sub-world is dropped, freeing all entities.

5. (rough) Update `Transition` to work with the tree model. A transition between two sibling nodes would temporarily render both with the transition's `render_info()` controlling alpha/offset.

6. (rough) Migrate the existing `SceneAction::Push` to implicitly use `PushOverlay` with `UpdateMode::Paused, DrawMode::Hidden` for backward compatibility, matching the current behavior where pushing pauses the underlying scene.

## Abort Criteria

- If sub-world isolation requires duplicating the entire ECS (copying all component storage from the parent world) rather than creating a fresh empty world, abandon sub-worlds and fall back to `StateScoped`-based entity tagging with query filters.
- If the tree traversal overhead (walking nodes, swapping worlds) adds more than 0.1ms per frame for a tree depth of 4 with 8 total nodes, simplify to a maximum of 2 levels (main + overlay).
- If the `World` swap mechanism (`std::mem::swap` of `game_ctx.world`) introduces unsoundness with outstanding borrows, redesign to pass the sub-world as a separate parameter to scene update/draw methods (breaking the `Scene` trait API).

## Consequences

### Positive
- Overlay scenes (pause menus, dialogue boxes, HUDs) can render on top of a still-visible gameplay scene without custom rendering hacks.
- Sub-worlds provide clean entity isolation -- menu entities never pollute the gameplay world, eliminating the need for `StateScoped` cleanup on scene transitions.
- The `Scene` trait is unchanged. Existing scenes work without modification on the flat top-level stack. Hierarchical features are opt-in via `PushOverlay` and `SceneConfig`.
- Nested composition enables patterns like a mini-game within a cutscene within a gameplay scene, each with independent entity pools.

### Negative / Trade-offs
- `std::mem::swap` of `World` on `GameContext` is a subtle pattern. If game code caches a reference or pointer to `game_ctx.world` across a scene boundary, it could observe the wrong world. Mitigation: document that `World` references must not be held across `update()` calls.
- Two update/draw traversal modes (flat stack for backward compat, tree for new code) increase `SceneManager` complexity. The flat stack should be deprecated and eventually removed.
- Sub-worlds do not share resources (e.g., asset handles, global singletons). Scenes that need shared state must use `GameContext.resources` (which is not swapped) or communicate via events.
- The `SceneConfig` adds a small amount of per-node memory and branch overhead in the traversal loop, but this is negligible for typical scene tree sizes (< 10 nodes).

## Updates

<!-- Append entries during implementation:
- YYYY-MM-DD: Discovered X, updated step N to account for Y.
-->
