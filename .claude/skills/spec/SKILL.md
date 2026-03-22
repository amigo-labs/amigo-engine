---
name: spec
description: >
  Create a spec or ADR for a new feature, module, or architectural change.
  Uses the project's templates and conventions. Output goes to specs/active/.
---

# /spec — Design Artifact Generator

Create a design artifact (Spec or ADR) for the given topic.

The user's request: $ARGUMENTS

---

## Step 1: Determine artifact type

```
Is this a NEW module with a clear API contract?
├── YES → SPEC (use template below)
└── NO  → Is this changing existing code or architecture?
    ├── YES → ADR (use template below)
    └── NO  → Ask the user to clarify
```

## Step 2: Explore the codebase

1. Read the relevant source files (NOT just docs/specs — read actual implementation)
2. Identify the crate this belongs to
3. Identify dependencies and dependents
4. Summarize findings to the user before writing

## Step 3: Write the artifact

**For Specs:** Use template at `.claude/skills/develop/templates/spec.md`
- Place at `specs/active/<kebab-case-name>.md`
- Include complete public API in Rust code blocks
- Define all behaviors, edge cases, error conditions
- List non-goals explicitly

**For ADRs:** Use template at `.claude/skills/develop/templates/adr.md`
- Place at `specs/active/<kebab-case-name>.md`
- Reference specific file paths and types
- Include numbered migration steps with verification
- Define abort criteria

## Step 4: Extract acceptance criteria

After writing the artifact, extract testable acceptance criteria:

- Every public API method → one AC
- Every described behavior → one AC
- Every edge case → one AC
- Every error type → one AC
- Always include quality gates:
  - `cargo check --workspace` compiles
  - `cargo test --workspace` passes
  - `cargo clippy --workspace -- -D warnings` clean
  - `cargo fmt --all --check` clean

## Step 5: Present for review

Present the artifact and acceptance criteria to the user. Wait for approval before marking as ready for implementation.

## Frontmatter format

```yaml
---
status: spec | in-progress | done
crate: amigo_<name>
depends_on: ["crate/module"]
last_updated: YYYY-MM-DD
type: spec | adr
---
```

## Rules

- Write in the same language the user uses (German or English)
- Be specific — reference file paths, types, line numbers
- Don't be vague — "improve performance" is not an acceptance criterion
- Every AC must be verifiable by running a command or reading a specific file
