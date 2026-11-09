---
name: spec-extract
description: Reverse engineering — writes feature specs from existing code, or records the current state of the code in an open spec.
argument-hint: '<area or feature-name> | --analyze [<spec-name>]'
metadata:
  requires: spec qa
---

# Spec Extract

You analyse existing code and turn it into feature specs in the format of the `spec` skill. Two modes:

| Call | Mode | Function |
| ---- | ---- | -------- |
| `spec-extract <area>` | Reverse engineering | Derive a spec from code |
| `spec-extract --analyze [<name>]` | Current state | Enrich an existing spec with the state of the code |

The template and the AC rules are those of `spec` (`.agents/skills/spec/SKILL.md`) — read them first. **Project specifics** come from the **spec workflow profile** in `AGENTS.md` (section `## Spec workflow profile`); this skill names its keys in bold.

**Agent notes:** `$ARGUMENTS` is what the user passed, `$0` its first word. The `!` command lines under Context are filled in by Claude Code; under another agent run them yourself. "Ask the user" means the agent's question tool (Claude Code: AskUserQuestion) or else a plain question that ends the turn.

---

## Mode: reverse engineering (`<area>`)

### 1. Explore the code

Find all code for `$ARGUMENTS` (subagents for broad searches where the agent has them), guided by the **Code map**:

- The logic, its data structures and commands
- File formats, codecs and their round-trip tests, if any
- Bindings, adapters and API surfaces between the layers
- UI: components, menus, keys, settings, user-visible strings
- Tests of every kind the profile's **Tests** names, and which CI job runs them
- Sections of the **Design spec** that describe the area

### 2. Reconstruct

From the code derive:

- **What it does** — summarize the feature from the implementation
- **Acceptance criteria** — testable ACs in the five forms from the actual behavior (not what it should do)
- **Technical notes** — existing data structures, file-format fields, API shapes, UI patterns
- **Edge cases** — error handling, guard clauses, validations
- **Evidence** — per AC the existing test that checks exactly this behavior (test name + file). Status `built` if such a test exists, else `open`. A test that only a CI job runs: the AC carries `CI: <job>` after its form.

### 3. Present + refine

Show the user:

- "Here is what the code does for `$ARGUMENTS`"
- "These ACs are covered by tests: …"
- "These have no tests: …"
- "These gaps / inconsistencies (also against the design spec) I found: …"

Ask: "Shall I save it like this, or do you want to add planned improvements as new ACs?"

### 4. Write the spec

Write to `docs/specs/$0.md` with the template from `spec`. If that name belongs to a design spec, pick `$0-<aspect>`.

- Checkboxes stay `[ ]`, status `todo` — even if everything seems covered. Only `spec-check` decides on `implemented`.
- Tell the user: if all ACs are covered, `spec-check $0` is enough for acceptance; if tests are missing, `spec-build $0` closes the gaps.

---

## Mode: current state (`--analyze`)

### 1. Load specs

- Scan `docs/specs/` for feature specs with `status: todo`
- With a name: only that spec
- Without a name and several: process them one after another

### 2. Analyse the code

Per spec:

1. Identify affected areas from Context and Technical notes
2. Read the code and analyse the current state
3. Match ACs against the code — are implementation approaches visible?
4. Identify relevant types, commands, API surfaces, components, tests

### 3. Write the current state

Update or add the `## Current state` section in the spec:

```markdown
## Current state

> Last updated: {YYYY-MM-DD}

### Existing implementation

- `{path}` — {what exists, e.g. "resize engine with name planning"}

### AC coverage (estimated)

| AC    | Status      | Reason                        |
| ----- | ----------- | ----------------------------- |
| AC-01 | Partial     | Engine exists, no dialog yet  |
| AC-02 | Not started | No code                       |
```

**An existing `## Current state` is replaced completely** — the code is the truth.

---

## Commit

Whether you commit the written files without asking is decided by the **commit mode** in Context (`SPEC_AUTOCOMMIT`, per developer):

- **on** (default — also when the Context line is missing or says anything other than `ask`): `spec-build` only starts on a clean working tree and reads the committed spec — so commit yourself:
  1. Add a new spec to the spec index (**Spec index**, default `docs/specs/README.md`), then stage only the affected files, by path: `git add docs/specs/<name>.md <spec index>` — never `git add -A`, no other files.
  2. Commit `docs(spec): extract <name>` or `docs(spec): analyze <name>`.
  3. On the protected branch (**Branches**, default `main`) first switch to `feat/<name>`, where `spec-build` continues.
- **ask**: ask the user before the commit — do not end the turn without this question. The question names the files and the commit message; the options are "Commit now" and "Continue without commit":
  - "Commit now": commit as with **on**.
  - "Continue without commit": you don't commit. Name the `git add` command with exactly these paths and the commit message, and point out that `spec-build` only starts once the specs are committed.

In both modes no `git push` — pushing and merging is the user's job.

## Rules

- **Document what IS, not what SHOULD be** — reverse engineering is a snapshot of the current state
- **No spec changes** except `## Current state` in analyze mode
- **Don't change the front-matter status** of an existing spec — only `spec-check` sets `implemented`
- **Take types and code verbatim** — don't paraphrase
- Specs are **English** unless the profile's **Language** says otherwise; talk to the user in their language

## Context

Existing specs:
!`grep -Li '^## Acceptance criteria' docs/specs/*.md 2>/dev/null | sed 's/$/ (design spec)/'; grep -li '^## Acceptance criteria' docs/specs/*.md 2>/dev/null || echo "No feature specs yet"`

Commit mode:
!`node .agents/skills/qa/scripts/commit-mode.mjs 2>/dev/null || echo "on"`
