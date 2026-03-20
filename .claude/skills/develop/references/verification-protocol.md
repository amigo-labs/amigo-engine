# Verification Protocol

This document defines how to extract acceptance criteria from design artifacts and how to verify them during and after implementation.

## Extracting Criteria from Specs

A spec defines a public API contract. Every element of that contract becomes a criterion.

### API Completeness

For every `pub fn`, `pub struct`, `pub enum`, `pub trait` in the spec's Public API section:
- One criterion per public method: "method exists with correct signature"
- One criterion per public type: "type exists and is public"
- One criterion per trait method: "trait method has correct signature and default behavior (if any)"

**How to extract:** Read the Public API section. Every code block is a contract. Each function signature, each struct field, each enum variant = one criterion.

### Behavior

For every statement in the Behavior section that says "when X, then Y":
- One criterion: "When X happens, Y is the result"
- These MUST have corresponding tests

For every edge case:
- One criterion: "Edge case X is handled correctly"

**How to extract:** Read the Behavior section line by line. Every "should", "must", "returns", "panics", "ignores" = one criterion.

### Error Types

For every error variant in the spec:
- One criterion: "Error variant exists"
- One criterion: "Error is returned under the documented condition"

## Extracting Criteria from ADRs

An ADR defines a migration path. The criteria track progress along that path.

### Migration Steps

For every numbered step in the Migration Path:
- One criterion: "Step N is complete and verified"
- Sub-criteria for testable outcomes of that step

### Preservation Criteria

If the ADR says "existing behavior must be preserved":
- One criterion per preserved behavior: "Behavior X still works after migration"
- These require **characterization tests** written BEFORE the migration

### Feature Flag Criteria

If new code coexists with old code behind a feature flag:
- "Crate compiles with feature flag OFF (old behavior)"
- "Crate compiles with feature flag ON (new behavior)"
- "All existing tests pass with feature flag OFF"
- "New tests pass with feature flag ON"

### Abort Criteria Monitoring

Each abort criterion from the ADR becomes a red-flag rule:
- "If [abort condition], stop and discuss with user"

## Standard Quality Gates

These are ALWAYS included, regardless of spec or ADR:

```
- [ ] `cargo check --workspace` — compiles without errors
- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] `cargo fmt --all --check` — correctly formatted
- [ ] New public API has at least one test per method
- [ ] No `unwrap()` in library code (use `expect()` with message or proper error handling)
- [ ] No `todo!()` or `unimplemented!()` left in committed code
```

## Convention Compliance

Read the project's convention files (CLAUDE.md, conventions.md, CONTRIBUTING.md) and add criteria for relevant conventions. Common ones:

- Naming: crate prefix, case conventions
- Error handling: which error crate, which patterns
- Math: simulation types vs rendering types
- Logging: which framework, which macros
- Serialization: which formats, which derives

## Verification Methods

### For API Completeness criteria

Use Grep to confirm existence:
```
grep "pub fn method_name" crate/src/lib.rs
grep "pub struct TypeName" crate/src/types.rs
```

Read the file and confirm the signature matches the spec exactly.

### For Behavior criteria

Confirm a test exists:
```
grep "fn test_.*behavior_name" crate/src/ -r
```

Run the specific test and show output:
```
cargo test -p crate_name test_name -- --nocapture
```

### For Quality Gates

Run each command and show the full output. Do not summarize — show the actual terminal output so the user can see it.

### For Convention Compliance

Read the relevant code sections and quote the lines that demonstrate compliance. Do not just say "conventions are followed" — show evidence.

## Completion Verification Checklist Format

When presenting the final verification to the user, use this format:

```markdown
## Verification Report: [Feature Name]

### API Completeness
- ✅ `method_a()` exists — verified at src/lib.rs:42
- ✅ `method_b()` exists — verified at src/lib.rs:67
- ❌ `method_c()` missing — NEEDS FIX

### Behavior
- ✅ Empty input returns None — test `test_empty_input` passes
- ✅ Overflow returns remainder — test `test_overflow` passes

### Quality Gates
- ✅ cargo check — 0 errors
- ✅ cargo test — 47 passed, 0 failed
- ✅ cargo clippy — 0 warnings
- ✅ cargo fmt — no changes needed

### Convention Compliance
- ✅ Uses `thiserror` — see src/error.rs:1
- ✅ Simulation uses Fix — see src/sim.rs:15
```

If ANY line shows ❌, you are NOT done. Fix it or discuss with the user.
