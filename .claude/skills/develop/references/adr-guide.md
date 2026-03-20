# ADR Guide

## When to Use an ADR

Use an ADR (Architecture Decision Record) when:
- You are **changing existing code** (refactoring, migrating, rearchitecting)
- The **final API is not clear upfront** and will emerge through iteration
- The work requires a **migration path** from current state to target state
- Multiple modules are affected and changes must be coordinated
- There is **risk** — the approach might not work and you need abort criteria

Do NOT use an ADR for new, isolated modules with clear APIs. Use a spec instead.

## ADR Format

Use the template at `templates/adr.md`. ADRs are stored in `docs/adrs/NNNN-<slug>.md`.

### Numbering

Auto-detect the next number by listing existing files in `docs/adrs/`. If no files exist, start with `0001`.

### Required Sections

**Status:**
One of: `proposed` → `accepted` → `completed` | `abandoned` | `superseded by ADR-XXXX`

**Context:**
What exists today and why it needs to change. Be specific:
- Reference actual file paths, type names, and module names
- Describe the current architecture concisely
- State the problem or limitation that motivates the change
- Include metrics if available (e.g., "collision detection takes 8ms per frame with 5k entities")

Bad: "The ECS is slow and needs improvement."
Good: "The ECS uses SparseSet storage (amigo_core/src/ecs/sparse_set.rs). Iteration over entities with Position+Velocity+Sprite requires three separate sparse lookups per entity. With 10k entities, this dominates frame time at 4ms per tick."

**Decision:**
What you are doing and why. Include:
- The chosen approach in 2-3 paragraphs
- 1-2 alternatives that were considered and why they were rejected
- Keep it concise — this is a decision record, not a design document

**Migration Path:**
Numbered steps from current state to target state. This is the most important section.

Rules:
- First 2-3 steps must be detailed and immediately actionable
- Later steps can be rough descriptions — they will be refined during implementation
- Each step must be independently committable and revertable
- Each step must have a way to verify it worked
- Steps should be ordered so that the system works after each step (no broken intermediate states)

Example:
```
1. Add `ecs_v2` module alongside existing ECS with Archetype storage struct — verify: compiles, existing tests pass
2. Implement `Archetype::insert()` and `Archetype::get()` with tests — verify: new unit tests pass
3. Add migration function `SparseSet → Archetype` for Position component — verify: round-trip test passes
4. (rough) Wire archetype storage into World behind feature flag
5. (rough) Benchmark comparison between old and new storage
6. (rough) Switchover: make archetype storage the default
```

**Abort Criteria:**
Concrete, measurable conditions under which you stop and revert. Every ADR must have at least one.

Examples:
- "If archetype storage is not at least 2x faster than sparse sets in the benchmark, abandon"
- "If migration requires changing more than 15 files in the public API, the approach is too invasive"
- "If after 2 weeks the new system cannot pass the existing test suite, revert"

**Consequences:**
What becomes easier and what becomes harder after the change.

**Updates:**
A log of changes made during implementation. Append entries as you discover things:
```
- 2026-03-20: Step 3 revealed that Position needs Copy trait for archetype storage. Updated step 4.
- 2026-03-21: Benchmark shows 3.2x improvement. Proceeding past abort criterion.
```

## Common Mistakes

1. **Writing an ADR like a spec**: ADRs don't define public API contracts. They record decisions and migration paths.
2. **Planning all steps upfront**: Only detail the first 2-3 steps. You don't know what you'll discover.
3. **No abort criteria**: Every ADR must have conditions under which you stop. If there are none, the change is either trivial (doesn't need an ADR) or you haven't thought about risk.
4. **Vague context**: "The system is slow" tells you nothing. Name files, types, and numbers.
5. **Steps that break the system**: Each step must leave the codebase in a working state. If step 3 requires step 4 to compile, merge them.
6. **Never updating the ADR**: The ADR is a living document. If the plan changes during implementation, update it.

## Quality Checklist for ADRs

Before presenting an ADR to the user, verify:
- [ ] Context references specific files, types, and modules
- [ ] Decision explains why this approach over alternatives
- [ ] First 2-3 migration steps are detailed with verification criteria
- [ ] Each step is independently committable
- [ ] At least one abort criterion exists with a measurable condition
- [ ] Consequences section has both positives and negatives

See `examples/good-adr-excerpt.md` for a concrete example.
