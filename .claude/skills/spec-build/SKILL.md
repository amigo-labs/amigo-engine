---
name: spec-build
description: Implements a feature spec autonomously — test first, code, one commit per AC block (optionally asking before each commit), then acceptance by an independent verifier and rework in a loop until the spec is implemented. Use this skill when the user wants to implement, build or continue an existing feature spec ("implement X", "carry on", "build the spec").
argument-hint: '<spec-name>'
metadata:
  requires: qa spec-check
---

# Spec Build: $ARGUMENTS

You implement `docs/specs/$0.md` **without asking**. The spec is the brief, the tests are the proof, and the last word belongs to a verifier that has never seen your chat. You only ask the user when a precondition is violated or a stopping point needs them — otherwise you run until `READY` or the round limit. In commit mode `ask` you additionally ask before every commit (see Commit mode).

**Project specifics** come from the **spec workflow profile** in `AGENTS.md` (section `## Spec workflow profile`); this skill names its keys in bold.

**Agent notes:** `$ARGUMENTS` is what the user passed, `$0` its first word — the spec name. The `!` command lines under Context are filled in by Claude Code; under another agent run them yourself. "Ask the user" means the agent's question tool (Claude Code: AskUserQuestion) or else a plain question that ends the turn. "Subagent without chat history" means a fresh agent that sees only the brief you give it (Claude Code: the Agent tool, `general-purpose`). Long commands (the full gates) may run for many minutes: give them a long timeout or run them in the background and wait.

Without an argument: take the feature spec with `status: todo` from Context; if there are several, let the user pick once.

## Preconditions

Check in this order, all before the first change. If one is violated, the skill aborts with the list of all violated points, without changing anything — not even creating a branch:

1. **Clean working tree** (`git status --porcelain` empty). Uncommitted changes — someone else's or those of an aborted block — would end up in the block commits and follow a branch switch; the user commits, stashes or discards them. In commit mode `ask` this is the normal case after the user picked "Continue without commit": they commit after their review and call the skill again.
2. **Which version of the spec counts:** the one being worked on. If the checkout is on the protected branch (**Branches**, default `main`) and `feat/$0` exists, it is that branch's version — read it without switching: `git show feat/$0:docs/specs/$0.md`. Otherwise the version in the checkout.
3. That version exists, has `## Acceptance criteria` and is `status: todo`.
4. Its `## Open questions` is empty (no bullets). Otherwise: point to `spec $0`.
5. Every AC carries one of the five forms and is falsifiable; manual ACs additionally carry `manual: {reason}` after the form.
6. **Build is green before you start:** the profile's **Build check** for the areas the spec touches. If it is red on the clean checkout, abort and report — never build on top of a broken state.
7. **Switch branch** — only now, when 1–6 hold: if the checkout is on the protected branch, create `feat/$0` and switch to it (if it exists: switch). On any other branch the skill stays there — the user chose it.

## Commit mode

Context names the commit mode (`SPEC_AUTOCOMMIT`, per developer). It applies to **every** commit of this skill — blocks, gate fixes, spec with open questions, completion:

- **on** (default — also when the Context line is missing or says anything other than `ask`): commit as described at each point, without asking.
- **ask**: at every commit point ask the user — never end the turn for it, the loop waits for the answer. The question names, by path, the files belonging to this run since the last commit and the commit message with the AC IDs (`spec-check` finds the commits by them later). The options are "Commit now" and "Continue without commit":
  - "Commit now": commit exactly these files by path, as described at that point, then continue.
  - "Continue without commit": don't commit and continue. The files go into the next question, their AC IDs into the next commit message. Resume only reads committed evidence; if the run aborts with uncommitted blocks, the "clean working tree" precondition stops the next call and the user decides about the changes.

## Resume

What counts is the **committed** spec (`git show HEAD:docs/specs/$0.md`), not the working tree: a block's Evidence row is written in the same commit as its code, so a committed `built` proves the commit.

- ACs with status `built` or `verified` are done — don't rebuild these blocks.
- The loop starts at the ACs with status `open`.

An abort or a new session costs nothing this way. If uncommitted changes of an aborted block are lying around, the "clean working tree" precondition applies: the user decides whether they get committed or discarded.

## Plan

- Read the spec in full, the design-spec sections it references (`design:` in its front matter), `AGENTS.md` (including `## Gotchas` and the profile) and the code in the feature area (**Code map**; subagents for broad searches where the agent has them).
- Group the open ACs into **blocks**: related behavior that can be tested together and reviewed in one commit series. Rather three sensible blocks than ten micro steps.
- Track one task per block (the agent's task list where it has one). No plan file, no review stop.
- **Session budget** (**Sizing**): when one session's budget is reached and a block boundary exists, stop after the block's commit and report; the user resumes with `spec-build $0`.

## Implement (per block, without asking)

1. **Test first.** For every AC of the block at least one test of the kind the profile's **Tests** names for that behavior. Run it and record the **red run**: command and output show it fails for the right reason. A test that only a CI job runs (**CI-only checks**) cannot go red locally: write it, say so under `## Decisions`, and its first CI run is the proof. A test that is green at once proves nothing — then it is wrong or the behavior already exists (record that under `## Decisions`).
2. **Implement** until the block's tests are green. Respect the profile's **Conventions** (module rules, user-visible strings, forbidden patterns).
3. **Fast gates**: `node .agents/skills/qa/scripts/gates.mjs fast`; `fix` commands where the script names them. No suppressions, no weakened tests — fix the cause.
4. **Evidence**: in `## Evidence` per AC the test name + file, status `built`. The `Commit` column stays empty — the hash only exists after the commit; `spec-check` fills it in at acceptance via the AC IDs in the commit messages.
5. **Commit** only the files this block changed, with the spec's Evidence change in the block's last commit — add explicitly by path, never `git add -A`, `git add .` or `git commit -a`. One concept per commit: a block that touches several areas is a short series of commits, one per scope, following the profile's **Commits** (scopes, what never shares a commit). Conventional Commits with the AC IDs in the message, e.g. `feat(core): crop to an aspect ratio (AC-02, AC-03)`. In commit mode `ask`, ask first (see Commit mode).

**Unclear detail decision:** don't ask. Pick the most obvious solution that fits the codebase and record it with a one-sentence reason under `## Decisions`.

**Stopping points that need the user** (the profile's **Stopping points**) — don't decide them alone: e.g. a new dependency, new `unsafe`, a test that would have to be disabled, an architectural decision missing from the **Decisions**, or an AC that turns out wrong. Skip the block, record the reason under `## Open questions` and continue with the next block. New public API that the ACs require is fine: list it under `## Decisions` and in the report for review, with doc comments where the **Conventions** ask for them.

## Full gates

After the last block run the full gates as the `qa` skill describes (`node .agents/skills/qa/scripts/gates.mjs full`): fix every red gate and start over. Fixes as a separate commit. ACs marked `CI: <job>` need a green run of that CI job: the user pushes, the verifier reads the job's result.

## Self-verification

If `## Open questions` has entries — from a skipped block or an AC that doesn't hold — no verification round starts: against open questions every round ends `NOT READY` by definition. Instead commit the spec with the open questions, stop and report — the user settles them via `spec $0`, then `spec-build` resumes.

If a manual AC (`manual: …` after the form) has no `manual:` entry in `## Evidence` yet, no verification round starts either: only a human can set that entry, every round would end `NOT READY`. Instead stop and report, with the list of manual checks (AC, what to check, entry format `manual: {who} checked {what}, {date}`). The user then accepts with `spec-check $0`.

Your own judgment doesn't count. Start a **fresh subagent without chat history** with exactly this brief:

> Follow the `spec-check` skill (`.agents/skills/spec-check/SKILL.md`) for `$0` and return the complete verdict with all blockers.

It derives everything from the repository. An agent without subagents asks the user to run `spec-check $0` in a fresh session and stops. The verdict decides:

- **`READY`**, **`READY (manual: n)`** or **`READY (CI: n)`** → continue with completion. `spec-check` has already set the spec to `implemented`.
- **`NOT READY`** → every blocker becomes a task, back to "Implement" (test first again), then full gates and another **new** subagent. At most 3 verification rounds; then stop and report, the spec stays `todo`.
- **`NOT READY — unchecked`** → the tests didn't run. Fix the cause (environment, build), then a new round; it counts as a verification round.

## Completion

1. **Keep knowledge:** what was surprising in the loop and can't be read from the code (pitfalls, environment quirks), as a short entry under `## Gotchas` in `AGENTS.md`. No noise — nothing that code or git history shows. Never into an agent's private memory.
2. **Commit** the spec, the spec index (`spec-check` moves the spec to "Done" there) and `AGENTS.md` if changed, by path: `docs(spec): mark $0 implemented`. The working tree is clean afterwards, so the next `spec-build` can start. In commit mode `ask`, ask first.
3. **Report** to the user, in their language:

```markdown
## Spec Build: {name}

**Verdict:** READY | NOT READY (after {n} rounds) | paused (session budget)
**Branch:** feat/{name} · {k} commits

| AC  | Test | Status |
| --- | ---- | ------ |

**Decisions without asking:** {list or "none"}
**New public API:** {list or "none"}
**Open:** {blockers, stopping points or manual checks}
**Not committed:** {only after "Continue without commit": paths, `git add` command and commit message}
```

## Rules

- **Never push or merge:** no `git push`, no `git merge`, no PR. The user does that.
- Never `git add -A`, never commit unrelated files, never `--no-verify`.
- Don't lower gates, don't delete, skip, ignore or weaken tests.
- The spec is the brief, not up for negotiation: don't reword ACs to make them fit. If an AC is wrong, record it under `## Open questions`.
- Spec, commit messages and code comments are **English** unless the profile's **Language** says otherwise; the report is in the user's language.

## Context

Feature specs with status todo:
!`grep -l '^status: todo' docs/specs/*.md 2>/dev/null | xargs -r grep -li '^## Acceptance criteria' 2>/dev/null || echo "No open feature specs"`

Branch and working tree:
!`git branch --show-current && git status --porcelain`

Commit mode:
!`node .agents/skills/qa/scripts/commit-mode.mjs 2>/dev/null || echo "on"`
