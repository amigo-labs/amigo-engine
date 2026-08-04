# ADR-0012: Hierarchical Scene Composition as an Alternative to the Flat Stack

- Status: accepted
- Date: 2026-08-03

## Context

The engine's scene model is a stack: pushing pauses what is below, popping
resumes it, and only the top updates and draws (see
`crates/amigo_engine/src/stack.rs`). That covers menus, pause overlays and level
transitions, which is most of what a game needs.

It does not cover cases where a scene should keep running *while* something sits
on top of it, or where two scenes should be visible at once: gameplay continuing
behind a translucent inventory, a split-screen view, a minimap rendering a second
camera onto the same world. In a flat stack the parent is simply not updated.

## Decision

Provide a tree as an alternative, behind the `hierarchical_scenes` feature flag
(`crates/amigo_scene/src/hierarchical.rs`). A `SceneNode` owns a scene, an
optional isolated sub-`World`, and children. Each node carries an `UpdateMode`
(active, paused, background) and a `DrawMode` (normal, hidden, alpha-blended), so
"keep simulating but dim it" is a node property rather than a special case in the
game's update.

`HierarchicalSceneManager` traverses depth-first and its stack operations are
deliberately compatible with the flat manager's, so the tree is a superset rather
than a parallel universe. `HierarchicalSceneAction` mirrors `SceneAction` and adds
`PushOverlay` for child scenes.

## Consequences

- The default stays a flat stack: simpler, and enough for most games.
- Two scene models exist. They share `Scene`/`SceneFactory` and the conversion
  `From<SceneAction> for HierarchicalSceneAction`, which keeps the overlap from
  becoming duplication.
- Optional sub-worlds let a child scene own entities that cannot collide with the
  parent's, at the cost of not being able to query across the boundary.
- The feature is not in CI's feature matrix. It compiles under
  `cargo check --all-features`, but it is less exercised than the flat stack, and
  the flat stack is what `amigo_engine` actually drives.
