// Claude Code PreToolUse hook (Bash): before a `git commit` runs, the `fast`
// gates from the gate configuration (.agents/gates.json) whose `paths` match
// the files going into the commit must be green. A red gate denies the commit
// and hands Claude the errors. This is where the slower checks (type-check,
// whole-workspace clippy, UI lint) belong: once per commit instead of after
// every turn (stop-gate.mjs runs only the quick `stop` list).
//
// - Any other command passes at once.
// - Files going into the commit: the staged ones, plus every changed tracked
//   file for `git commit -a` / `--all`. When the same command stages files
//   first (`git add … && git commit …`), the hook runs before that `git add`,
//   so every uncommitted path counts.
// - A gate that runs into the time limit refuses the commit too: an unchecked
//   commit is not a green one. The message points to `gates.mjs fast`, which
//   has no such limit; a green run there lets the next commit through.
// - Skipped when `gates.mjs fast` was green on exactly this tree (spec-build
//   runs it right before committing).
// - Locked files (a running dev server, an open IDE) do not block.
// - COMMIT_GATE=off skips it, e.g. for a bulk migration.

import {
  commitPaths,
  errorLines,
  expandGates,
  gateApplies,
  lastGreen,
  loadGates,
  print,
  projectDir,
  readStdinJson,
  recordGreen,
  runGates,
  treeState,
  uncommittedPaths,
} from './hook-lib.mjs';

const MAX_ERROR_LINES = 30;
const COMMAND_TIMEOUT_MS = Number(process.env.COMMIT_GATE_TIMEOUT_MS) || 240000;
const LOCK_PATTERN = /being used by another process|EBUSY|os error 32/i;
// `git commit`, also with options before the subcommand (`git -C dir commit`)
const GIT_COMMIT = /(^|[;&|(\s])git(\s+(-C\s+\S+|-c\s+\S+|--[\w-]+(=\S+)?))*\s+commit(?![\w-])/;
// `git add` / `rm` / `mv` in the same command, before the commit
const STAGES_IN_COMMAND = /(^|[;&|(\s])git(\s+(-C\s+\S+|-c\s+\S+|--[\w-]+(=\S+)?))*\s+(add|rm|mv|stage)(?![\w-])/;
const ALL_TRACKED = /\scommit(?![\w-])[^;&|]*\s(-[a-zA-Z]*a[a-zA-Z]*|--all)(?![\w-])/;

function deny(reason) {
  print({ hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'deny', permissionDecisionReason: reason } });
}

try {
  const input = await readStdinJson();
  const command = input.tool_input?.command ?? '';
  if (process.env.COMMIT_GATE === 'off' || input.tool_name !== 'Bash' || !GIT_COMMIT.test(command)) {
    process.exit(0);
  }

  const dir = projectDir(input);
  const fast = loadGates(dir)?.fast;
  const tree = treeState(dir);
  if (!fast?.length || !tree || lastGreen(dir, 'fast') === tree.hash) process.exit(0);

  const paths = STAGES_IN_COMMAND.test(command) ? (uncommittedPaths(dir) ?? []) : commitPaths(dir, { allTracked: ALL_TRACKED.test(command) });
  const selected = expandGates(dir, fast.filter(gate => gateApplies(gate, paths)), paths);
  if (selected.length === 0) process.exit(0);

  const results = await runGates(dir, selected, { timeoutMs: COMMAND_TIMEOUT_MS });
  const red = results.filter(result => result.status === 'failed' && !LOCK_PATTERN.test(result.output));
  const timedOut = results.filter(result => result.status === 'timeout');
  if (red.length === 0 && timedOut.length === 0) {
    if (selected.length === fast.length && results.every(result => result.status === 'ok')) recordGreen(dir, 'fast', tree.hash);
    process.exit(0);
  }

  const skipped = results.filter(result => result.status === 'skipped').map(result => result.gate.name);
  deny(
    [
      red.length > 0
        ? 'Commit refused: a quality gate is red. Fix the cause, then commit again. Do not lower a lint level and do not add a suppression.'
        : 'Commit refused: a quality gate did not finish in time, so the commit is unchecked.',
      ...red.flatMap(result => [
        '',
        `Quality gate "${result.gate.name}" is red (${result.gate.command}).`,
        ...(result.gate.fix ? [`Auto-fixable with: ${result.gate.fix}`] : []),
        ...(result.gate.note ? [`Note: ${result.gate.note}`] : []),
        ...errorLines(result.output, MAX_ERROR_LINES),
      ]),
      ...timedOut.flatMap(result => [
        '',
        `Quality gate "${result.gate.name}" ran past ${Math.round(COMMAND_TIMEOUT_MS / 1000)} s (${result.gate.command}).`,
        'Run `node .agents/skills/qa/scripts/gates.mjs fast` (no time limit, e.g. in the background); when it is green on this tree, commit again and the gate lets it through.',
      ]),
      ...(skipped.length > 0 ? ['', `Not run because a gate before them is red: ${skipped.join(', ')}`] : []),
    ].join('\n'),
  );
} catch {
  // Fail open: CI still checks
}
