---
name: qa
description: Runs the project's full QA pass — every gate of the repository's CI (format, lint, type-check, tests, builds, generated-file drift, …) as listed in .agents/gates.json, independent stages in parallel, with exactly the commands CI runs. Use this skill before finishing any coding task, before a commit or PR, and whenever the user wants to know whether "everything is green".
---

# QA

The commands are not here but in `.agents/gates.json` (list `full`; `stop` and `fast` are the quicker lists the agent hooks run after a turn and before a commit; a repository not migrated yet has it at `.claude/gates.json`). It mirrors the repository's CI, minus jobs that need another platform or hardware — the profile in `AGENTS.md` (**CI-only checks**) and the `$comment` in `gates.json` name them. Green here means green in those CI steps.

## Procedure

1. From the repo root run `node .agents/skills/qa/scripts/gates.mjs full`. Cold builds and e2e tests make it slow: give it a long timeout (10 minutes and more), or run it in the background and wait for it to finish. The script runs every entry of `full`, independent stages at the same time (`needs` in `gates.json`), and prints a table, plus for every red stage its error lines, the `fix` command, a `note` if the gate has one, and the path to the full log.
2. Exit 2 means: the gate configuration is missing or broken. Report it and stop.
3. Fix **all red stages** in the code, then start the run **from scratch** — a fix can break another stage. A skipped stage is not green: it waits for a red one and only runs in the next pass. If the error lines aren't enough, read the log.
4. Where a stage has a `fix` command, it may fix what is auto-fixable (formatting, lint autofix). Start over afterwards anyway.
5. Where a red stage has a `note`, follow it — e.g. generated files that belong in the commit rather than being reverted.
6. If the change touches code that only a CI job builds or runs (**CI-only checks**), say so in the report: the local run doesn't cover it.

If the script is missing, run the entries of `full` **in the given order** one by one, each `command` in its `cwd` (relative to the repo root), stop at the first failure, fix and start over. Don't omit, rephrase or replace a command with an "equivalent" one.

## Forbidden

- Lowering lint levels or suppressing warnings — the patterns in `suppressions` of `gates.json` (e.g. `#[allow(…)]`, `#[expect(…)]`, `@ts-ignore`, `@ts-expect-error`, `eslint-disable`).
- Changing the gate configuration: `.agents/gates.json`, `.agents/skills/qa/scripts/`, `.claude/settings.json`, `.claude/hooks/`, the CI workflows.
- Deleting, skipping (`#[ignore]`, `test.skip`, `test.fixme`, …) or weakening tests to make them green.

If a warning really is a false positive after careful review, don't resolve it yourself: name the spot and the reason to the user (the forbidden patterns in `AGENTS.md` say who may add a suppression).

## Report

Short, as a table (the last run's, without the log column):

| Gate | Command | Result |
| ---- | ------- | ------ |

Plus: what was fixed (file, cause) and what stays open. No "should be green now" — only what the last run actually showed.
