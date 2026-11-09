---
name: spec
description: Captures or revises a feature spec in dialogue — WHAT gets built, with testable acceptance criteria and a clarification pass. Use this skill when the user describes a new feature or changed behavior ("I want …", "it should …", "add a … tool"), even without the word spec. Not for bug fixes, refactorings, tooling or questions.
argument-hint: '<feature-name>'
metadata:
  requires: qa
---

# Spec: $ARGUMENTS

You define a feature spec together with the user. This is collaborative — research, propose, refine. The spec is the only brief for `spec-build`, which then implements it **without asking**: whatever is not settled here, the AI decides alone later.

**Project specifics** come from the **spec workflow profile** in `AGENTS.md` (section `## Spec workflow profile`). Read it first; this skill names its keys in bold (**Design spec**, **Decisions**, **Code map**, …).

**Agent notes:** `$ARGUMENTS` is what the user passed, `$0` its first word — the spec name. The `!` command lines under Context are filled in by Claude Code; under another agent run them yourself. "Ask the user" means the agent's question tool (Claude Code: AskUserQuestion) or else a plain question that ends the turn. "Subagent without chat history" means a fresh agent that sees only the brief you give it (Claude Code: the Agent tool, `general-purpose`).

## Step 1: Does the spec exist?

Check `docs/specs/$0.md` (name in ASCII kebab-case):

- **Exists with `## Acceptance criteria`** (heading in any case) → revision mode (see below)
- **Exists without acceptance criteria** → that is a design spec (**Design spec** in the profile), not a feature spec. Propose a different name, e.g. `$0-<aspect>`.
- **New** → continue with step 2

If the request touches an existing feature spec (see Context), propose its revision mode instead of creating a second one.

## Step 2: Research + questions

Both in parallel:

**Research** (subagents for broad searches where the agent has them):

- Find the sections of the **Design spec** the feature builds on, and its entry in the spec index (**Spec index**, default `docs/specs/README.md`: order, backlog) if any
- Find existing code in the feature area, guided by the **Code map**
- Identify reusable types, commands, components and test helpers
- Check for conflicts with other feature specs and with the **Decisions**

**Then present the findings and ask questions** in small batches (1–4 at a time):

- **What & why** — what should happen? What is out of scope, and why?
- The project's own questions from **Spec questions** (e.g. platform, mode, UI entry point, speed budget)
- **Edge cases** — propose concrete scenarios and let the user confirm or reject them

**Propose instead of only asking**: "I would put the logic in the core and expose it through the existing adapter — does that fit?" beats "Where should this live?"

If the user says at the first question that they want no spec ("just do it"), stop and implement directly.

**Invent nothing:** what the user doesn't answer becomes an assumption (with a reason) or an open question — never a silent guess.

**Architecture:** if the feature needs a decision the profile reserves for the user (**Stopping points**: new dependency, public API shape, cross-crate or cross-package boundary, file-format change), ask it here. The answer goes where **Decisions** says, written in step 3 alongside the spec.

## Step 3: Write

Write the spec to `docs/specs/$0.md` (template below). Size it per **Sizing**: a feature too big for a few sessions is split into several specs before writing. Add it to the spec index (order table, or remove its backlog row); create `docs/specs/README.md` if the repository has no index yet.

## Step 4: Clarification pass

Start a subagent **without chat history**. It gets only the path of the spec and this brief: read only the spec (code only where a term would otherwise stay unclear) and list findings in six categories, each with a verbatim quote. Empty categories read "none found". No proposed solutions.

1. **Contradictions** — statements that cannot all be true at once
2. **Ambiguous terms** — words with several readings or undefined domain terms ("active", "selected", "valid", "current")
3. **Untestable criteria** — ACs whose violation cannot be observed
4. **Implementation instead of intent** — requirements that prescribe a technique instead of a behavior
5. **Missing error cases** — missing, duplicate or broken input, empty or huge input, something that changes or disappears meanwhile, unreachable dependencies, concurrency
6. **Unfounded assumptions** — decisions or out-of-scope items without a "because"

The subagent is a stranger on purpose: what it can't understand without the chat, `spec-build` won't understand either. An agent without subagents does the pass itself, reading only the spec file as if for the first time.

Turn the findings into questions for the user, adjust the spec, repeat the pass until no findings remain or the user consciously accepts the rest as assumptions.

## Step 5: Commit

Whether you commit the spec without asking is decided by the **commit mode** in Context (`SPEC_AUTOCOMMIT`, per developer):

- **on** (default — also when the Context line is missing or says anything other than `ask`): `spec-build` only starts on a clean working tree and reads the committed spec — an unsaved spec would make it abort at once. So commit the spec yourself, and only it:
  1. `git add docs/specs/$0.md <spec index>` (plus the file of a decision from step 2) — never `git add -A`, no other files.
  2. Commit `docs(spec): add $0` (new) or `docs(spec): revise $0` (revision mode).
- **ask**: ask the user before the commit — do not end the turn without this question. The question names the files and the commit message; the options are "Commit now" and "Continue without commit":
  - "Commit now": commit as with **on**.
  - "Continue without commit": you don't commit. Name the `git add` command and the commit message and point out that `spec-build` only starts once the spec is committed.

Branches as the profile's **Branches** says (default: `main` is protected): if the checkout is on the protected branch, first create and switch to `feat/$0` (`git switch -c feat/$0`, or `git switch feat/$0` if it exists — uncommitted changes come along) and commit there; `spec-build` continues on that branch. In both modes no `git push` — pushing and merging is the user's job.

If uncommitted changes other than the spec are in the working tree, leave them untouched; mention in the report that `spec-build` only starts once they are dealt with.

Then show a compact summary (not the full Markdown) and point to `spec-build $0`.

## Revision mode

- Read the spec, ask what should change, re-ask only the affected sections.
- **AC IDs stay stable** — never renumber. New ACs get the next free number and an Evidence row with status `open`.
- Changed ACs always go back: checkbox `[ ]`, Evidence row to `open` with an empty `Commit` column — also mid-implementation, otherwise `spec-build`'s resume skips an AC that doesn't exist yet in its new wording.
- Dropped ACs stay as `- ~~AC-04 …~~ dropped: {reason}`; their Evidence row gets the status `dropped`.
- If the spec was `status: implemented`: back to `status: todo`.
- Record the change with its date under `## Decisions`.
- Then step 4 (clarification pass) and step 5 (commit) as usual.

## Spec template

```markdown
---
status: todo
created: {YYYY-MM-DD}
size: {S | M | L}
design: [{design spec §x.y}, …]
{one line per extra field from the profile's Spec questions}
---

# {Feature name}

## Context

{What is the problem? Why is this feature needed? What is already decided (Decisions)?}

## Scope

### In scope

- {What belongs to it}

### Out of scope

- {What doesn't} — because {reason}

## Acceptance criteria

- [ ] AC-01 (always): {AC subject} {behavior}.
- [ ] AC-02 (event): When {trigger}, then {behavior}.
- [ ] AC-03 (state): While {state}, {behavior}.
- [ ] AC-04 (error): If {error case}, then {behavior}.
- [ ] AC-05 (option): Where {option} is set, {behavior}.

## Technical notes

{Optional — data structures, file-format fields, API shape, UI sketch. Omit if not needed.}

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| {case}   | {behavior}        |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |

## Decisions

{Filled during revision and implementation — decision plus reason.}
```

**Omit empty optional sections** (Technical notes, Assumptions). `## Open questions` and `## Evidence` always stay: `spec-build` only starts when Open questions is empty, and the Evidence table gets one row per AC with status `open`.

## Rules for acceptance criteria

- **Five forms**, every AC carries its form in parentheses: `(always)` for invariants, `(event)` When … then, `(state)` While …, `(error)` If … then, `(option)` Where … is set.
- **Falsifiable**: not "should be fast" but a number with the command that measures it (the profile's **Performance AC example**). What cannot be observed goes under Open questions.
- **Behavior, not technique**: an AC survives a rewrite of the implementation.
- **Testable by automation** (the profile's **Tests**). Only what truly cannot be automated (e.g. purely visual design, a system dialog, the feel of an input device) gets `manual: {reason}` after the form, e.g. `- [ ] AC-06 (state, manual: visual design): While …`. The form stays; the suffix only says a human provides the evidence — `spec-build` stops before acceptance and lists these checks.
- **CI-only behavior** (the profile's **CI-only checks**, if any): an AC whose test runs only in one CI job says so after the form, e.g. `(event, CI: windows)`. Its evidence is the test name plus a green run of that job.
- Respect the profile's **Conventions** in what an AC may demand (e.g. a forbidden pattern can never be required).
- At least **3 edge cases**, every out-of-scope item with "— because".
- **Spec = WHAT**, not HOW — no file names or task lists in the ACs.
- Spec language is **English** unless the profile's **Language** says otherwise; talk to the user in their language.

## Context

Existing specs:
!`grep -Li '^## Acceptance criteria' docs/specs/*.md 2>/dev/null | sed 's/$/ (design spec)/'; grep -li '^## Acceptance criteria' docs/specs/*.md 2>/dev/null || echo "No feature specs yet"`

Commit mode:
!`node .agents/skills/qa/scripts/commit-mode.mjs 2>/dev/null || echo "on"`
