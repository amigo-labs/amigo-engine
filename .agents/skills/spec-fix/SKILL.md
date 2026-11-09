---
name: spec-fix
description: Lightweight bug-fix workflow — find the root cause, write a regression test that is red first, make the minimal fix, keep affected feature specs in sync, commit. No new spec. Use for bugs and regressions ("X is broken", "fix issue #12"), not for new features or changed behavior.
argument-hint: '<bug description or issue number>'
metadata:
  requires: qa
---

# Bug fix: $ARGUMENTS

A bug fix needs no feature spec, but it always needs a regression test that is red before the fix. **Project specifics** (tests, commits, conventions) come from the **spec workflow profile** in `AGENTS.md` (section `## Spec workflow profile`).

**Agent notes:** `$ARGUMENTS` is what the user passed. The `!` command lines under Context are filled in by Claude Code; under another agent run them yourself. "Ask the user" means the agent's question tool (Claude Code: AskUserQuestion) or else a plain question that ends the turn.

## Step 1: Find the root cause

1. **Reproduce:** understand the bug from the description (and the issue, if a number is given). Search the code for the path involved (**Code map**).
2. **Trace** the code path to the root cause — not just the place where the symptom shows.
3. **Check the specs:** search `docs/specs/` for feature specs covering the affected behavior. If the fix changes specified behavior, name the spec and the AC.
4. **Present** in a few lines: where the bug is (`file:line`), what happens, the root cause, the fix approach, and affected specs.

Ask the user for confirmation before fixing, unless they already said to go ahead.

If the fix is bigger than about 50 lines or touches 5 or more files, or the "bug" is really missing behavior, it is not a bug fix: propose `spec` instead.

## Step 2: Fix

1. **Regression test first** — of the kind the profile's **Tests** names for this behavior. Run it and record the **red run**: it fails for the reason found in step 1. A test that is green before the fix proves nothing.
2. **Minimal fix** — the root cause, nothing else: no refactoring, no clean-up around it. Respect the profile's **Conventions**.
3. **Fast gates:** `node .agents/skills/qa/scripts/gates.mjs fast` until green — fix causes, no suppressions.
4. **Specs:** if the fix changes behavior a feature spec describes, revise it the way `spec`'s revision mode does (changed AC back to `[ ]` and `open`, entry under `## Decisions`). If the bug reveals an untested edge case of a spec, add it as a new AC with the next free number.

Show the user: a short diff summary, the red and the green run, spec changes if any.

## Step 3: Commit

- Only the files of the fix, by path — never `git add -A`.
- Conventional Commit `fix(<scope>): <what was broken>` (scopes from **Commits**); with an issue number add `Fixes #<n>` to the body.
- Ask before committing unless the commit mode in Context is `on` and the user already confirmed the fix. Never push.

## Rules

- **Always a regression test**, red before the fix — a fix without one is incomplete.
- **Minimal** — don't refactor, don't improve, don't clean up.
- No new feature spec; keep existing ones true.
- Commit messages and code comments in English unless the profile's **Language** says otherwise; talk to the user in their language.

## Context

Branch and recent commits:
!`git branch --show-current; git log --oneline -5 2>/dev/null`

Commit mode:
!`node .agents/skills/qa/scripts/commit-mode.mjs 2>/dev/null || echo "on"`
