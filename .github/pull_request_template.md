## Summary

<!-- What does this change and why? Link the spec, ADR, or issue it implements. -->

## Changes

-

## Verification

<!-- How was this checked? `just ci` covers fmt, check, clippy (all targets and
     the feature matrix), tests, doc tests, and rustdoc. Note anything that could
     not be verified here, e.g. on-GPU rendering. -->

- [ ] `just ci` passes locally
- [ ] Simulation code stays on `Fix`/`SimVec2` (no `f32`, wall-clock time, or unordered iteration feeding game state)
- [ ] `CHANGELOG.md` `[Unreleased]` updated for user-visible changes
- [ ] Breaking changes to `amigo_engine::prelude` are called out
