# ADR-0010: Reflection-Based Editor Inspector

- Status: accepted
- Date: 2026-08-03

## Context

An entity inspector has to show and edit the components on an entity. Without
runtime type information, the only options are a hand-written panel per component
type — which drifts the moment someone adds a field — or a macro that generates
one, which is the same drift with more machinery.

The engine already has `amigo_reflect` (`Reflect`, `TypeRegistry`, a derive
macro), added for exactly this kind of problem.

## Decision

The editor's inspector reads and writes components through reflection, behind the
`editor_v2` feature flag (`crates/amigo_editor/src/editor_v2.rs`). A component
that derives `Reflect` and is registered in the `TypeRegistry` becomes editable
with no editor-side code.

Undo/redo records type-erased field changes (`FieldChange`: label, previous
value, new value) in a plain `Vec` history rather than diffing whole component
snapshots. A field edit is the unit a user thinks in, so it is the unit undone,
and the history stays proportional to edits rather than to world size.

## Consequences

- Adding a field to a component adds it to the inspector for free.
- Components that do not derive `Reflect` are invisible to the inspector. That is
  the intended pressure: registration is how a type opts into tooling.
- Type-erased undo entries mean the history cannot be inspected without
  downcasting, so history entries carry a human-readable label alongside the
  value.
- Feature-gated: a shipped game links neither the inspector nor the registry.
