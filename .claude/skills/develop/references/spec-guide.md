# Spec Guide

## When to Use a Spec

Use a spec when:
- You are building a **new module** that does not exist yet
- The **public API can be defined upfront** before implementation
- The module has **clear boundaries** and few dependencies on existing code
- Deleting the module = complete revert (no migration needed)

Do NOT use a spec when you are changing existing code, migrating between implementations, or when the API will only become clear through iteration. Use an ADR instead.

## Spec Format

Use the template at `templates/spec.md`. The format follows the project's existing convention.

### Required Sections

**YAML Frontmatter:**
- `status`: Start with `spec`. Update to `done` after implementation is complete and verified.
- `crate`: Which crate this belongs to
- `depends_on`: List of spec dependencies (by path, e.g., `["engine/core"]`)
- `last_updated`: Date of last change

**Purpose:**
One paragraph. Why does this module exist? What problem does it solve? Who uses it?

Bad: "Provides inventory functionality."
Good: "Generic item and inventory system for RPG-style games. Provides an item registry for definitions, grid-based inventory with automatic stacking, and equipment slots with type validation."

**Public API:**
The contract. Include complete Rust code blocks with:
- All public structs, enums, traits with their fields/variants/methods
- All public functions with full signatures
- Derive macros that are part of the contract (e.g., `Serialize, Deserialize`)
- Doc comments for non-obvious behavior

This section IS the contract. Implement exactly this, nothing more, nothing less.

**Behavior:**
Describe what happens at runtime. Focus on:
- Normal flow: "When X, the system does Y"
- Edge cases: "When the inventory is full, `add()` returns the overflow"
- Error conditions: "Returns `AssetError::NotFound` if the path does not exist"
- Ordering guarantees: "Events are processed in the order they were emitted"

**Internal Design:**
Implementation suggestions. This is NOT a contract — the implementer may deviate.
- Data structures and their rationale
- Algorithms and their complexity
- Performance considerations

**Non-Goals:**
Explicitly state what this module does NOT do. This prevents scope creep during implementation.

**Open Questions:**
Unresolved decisions. These MUST be resolved before implementation begins.

## Common Mistakes

1. **Vague Public API**: "Provides methods for managing items" — instead, write the actual function signatures
2. **Missing error types**: Every function that can fail needs an error type in the API
3. **Over-specified Internal Design**: The internal design should suggest, not mandate. If you find yourself writing implementation code in the spec, you're over-specifying.
4. **No edge cases in Behavior**: If `add()` can overflow, the spec must say what happens
5. **Non-Goals that are actually goals**: If something is mentioned in Non-Goals but the module actually needs it, it should be a goal
6. **Stale depends_on**: Dependencies that no longer exist or have been renamed

## Quality Checklist for Specs

Before presenting a spec to the user, verify:
- [ ] Every public type has all fields/variants listed
- [ ] Every public method has a full signature (params + return type)
- [ ] Every error condition is documented with its error type
- [ ] Edge cases are explicitly described in Behavior
- [ ] Non-Goals are specific, not vague
- [ ] Open Questions have no impact on the Public API (or the API accounts for both answers)
- [ ] The spec can be implemented without reading any other document

See `examples/good-spec-excerpt.md` for a concrete example.
