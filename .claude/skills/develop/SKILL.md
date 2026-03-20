---
name: develop
description: >
  Use when implementing features, refactoring, or extending code.
  Drives verified development: designs with specs or ADRs, extracts
  testable acceptance criteria, implements iteratively, and verifies
  completeness before declaring done.
---

# /develop — Verified Iterative Development

You are entering **verified development mode**. Every implementation must be designed, broken into acceptance criteria, implemented iteratively, and verified before completion.

The user's request: $ARGUMENTS

---

## Phase 1: Explore

Before designing anything, understand the current state.

1. Read the code areas relevant to the request. Read **implementation files**, not just specs or docs.
2. Identify existing tests that cover the affected area — these are your safety net.
3. Identify dependents: what code calls into or depends on the area being changed?
4. Summarize your findings to the user. Wait for confirmation before proceeding.

Do NOT skip this phase. Do NOT start writing code or design documents until you have read the relevant source files and presented your understanding.

---

## Phase 2: Triage — Spec or ADR?

Choose the right design artifact based on the nature of the work:

```
Does the affected code already exist?
├── NO → Is the API clearly definable upfront?
│   ├── YES → SPEC MODE (new module with clear contract)
│   └── NO  → ADR MODE (exploratory new development)
└── YES → Are you changing existing behavior?
    ├── YES → ADR MODE (migration, refactoring, rearchitecture)
    └── NO  → SPEC MODE (extension with clear interface)
```

Tell the user which mode you chose and why. The user can override.

### Spec Mode

Create a spec at the appropriate location following project conventions.
For format, best practices, and common mistakes: read [`references/spec-guide.md`](references/spec-guide.md).
Use template: [`templates/spec.md`](templates/spec.md).

### ADR Mode

Create an ADR at `docs/adrs/NNNN-<slug>.md`. Auto-detect the next number.
For format, best practices, and common mistakes: read [`references/adr-guide.md`](references/adr-guide.md).
Use template: [`templates/adr.md`](templates/adr.md).

**Wait for user approval of the design artifact before proceeding.**

---

## Phase 3: Extract Acceptance Criteria

**This is the most important phase.** Before writing any implementation code, extract explicit, testable criteria from the design artifact.

Use template: [`templates/acceptance-criteria.md`](templates/acceptance-criteria.md).

For detailed extraction rules: read [`references/verification-protocol.md`](references/verification-protocol.md).

### Extraction rules (summary)

**From Specs:**
- Every public API method → one API Completeness criterion
- Every described behavior → one Behavior criterion
- Every edge case mentioned → one Behavior criterion
- Every error type → one criterion

**From ADRs:**
- Every migration step → one criterion
- "Old behavior must be preserved" → characterization test criteria
- "New code behind feature flag" → compile criteria (with and without flag)
- Every abort criterion → a red-flag monitoring rule

**Always include these Quality Gates:**
- `cargo check --workspace` compiles without errors
- `cargo test --workspace` — all tests pass
- `cargo clippy --workspace -- -D warnings` — no warnings
- `cargo fmt --all --check` — correctly formatted
- New public API has at least one test per method
- No `unwrap()` in library code

**Always include Convention Compliance criteria** based on the project's conventions (read CLAUDE.md, conventions.md, or equivalent).

Present the acceptance criteria to the user. They may add, remove, or modify criteria. **Do not proceed until the user approves the criteria.**

---

## Phase 4: Implement Iteratively

Break the work into small tasks. Use TodoWrite to track progress.

**Per task:**
1. Mark the task as `in_progress` in TodoWrite
2. If the area lacks test coverage: write characterization tests FIRST (tests that document current behavior before you change it)
3. Implement the change for this one task
4. Run quality gates:
   ```
   cargo check --workspace
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   cargo fmt --all --check
   ```
5. If any gate fails → fix it before proceeding. Do not skip.
6. Commit with a focused message describing this single step
7. Mark the task as `completed` in TodoWrite
8. Check off any acceptance criteria this task satisfies
9. Refine the next 2-3 tasks based on what you learned

**Check in with the user** every 3 completed tasks, or immediately when:
- Something unexpected happens
- You need to change the approach
- Scope is growing beyond the original request

For detailed iteration protocol, feature flags, and commit rules: read [`references/iteration-playbook.md`](references/verification-protocol.md).

---

## Phase 5: Completion Verification

**You are NOT done until this phase is complete.**

Before saying "done" or "implemented" or "finished":

1. Go through EVERY acceptance criterion one by one
2. For each criterion, verify it:
   - API criteria: use Grep to confirm the function/method exists with correct signature
   - Behavior criteria: confirm a test exists that covers this behavior, run it
   - Quality gates: run the cargo commands, show the output
   - Convention compliance: read the relevant code and confirm
3. Mark each criterion as ✅ (verified) or ❌ (not met)
4. If ANY criterion is ❌ → fix it or discuss with the user. Do NOT declare done.
5. Present the completed checklist to the user with verification evidence
6. Only THEN say the work is complete

For the full verification protocol: read [`references/verification-protocol.md`](references/verification-protocol.md).

---

## Hard Rules

These are non-negotiable. They apply to every `/develop` session:

1. **You are NOT done** while any acceptance criterion is unchecked
2. **You are NOT done** while `cargo check` or `cargo test` fails
3. **Never say** "this should work" — verify it. Run the command. Read the output.
4. **Never skip** quality gates because "it's a small change"
5. **3+ failed fixes** on the same problem → stop. This is an architecture problem, not an implementation problem. Discuss with the user.
6. **Scope creep** ("I should also fix this while I'm here") → NO. Stay on task. Suggest it as a separate `/develop` session.
7. **Commenting out tests** instead of fixing them → NEVER. Fix the test or discuss with the user why the behavior changed.

For a complete list of red flags and recovery strategies: read [`references/red-flags.md`](references/red-flags.md).

---

## Quick Reference

| Phase | Output | Gate |
|-------|--------|------|
| Explore | Summary of current state | User confirms understanding |
| Design | Spec or ADR | User approves artifact |
| Criteria | Acceptance checklist | User approves criteria |
| Implement | Working code + commits | Quality gates pass per task |
| Verify | Completed checklist | ALL criteria verified |
