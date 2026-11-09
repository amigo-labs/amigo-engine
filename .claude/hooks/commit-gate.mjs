// Claude Code PreToolUse hook (Bash): before a `git commit` runs, the `fast`
// gates from the gate configuration (.agents/gates.json) whose `paths` match
// the files going into the commit must be green. A red gate denies the commit
// and hands Claude the errors. This is where the slower checks (type-check,
// whole-workspace clippy, UI lint) belong: once per commit instead of after
// every turn (stop-gate.mjs runs only the quick `stop` list).
//
// - Any other command passes at once.
// - Files going into the commit: the staged ones, plus every changed tracked
//   file for `git commit -a` / `--all`.
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
} from './hook-lib.mjs';

const MAX_ERROR_LINES = 30;
const COMMAND_TIMEOUT_MS = 240000;
const LOCK_PATTERN = /being used by another process|EBUSY|os error 32/i;
// `git commit`, also with options before the subcommand (`git -C dir commit`)
const GIT_COMMIT = /(^|[;&|(\s])git(\s+(-C\s+\S+|-c\s+\S+|--[\w-]+(=\S+)?))*\s+commit(?![\w-])/;
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

  const paths = commitPaths(dir, { allTracked: ALL_TRACKED.test(command) });
  const selected = expandGates(dir, fast.filter(gate => gateApplies(gate, paths)), paths);
  if (selected.length === 0) process.exit(0);

  const results = await runGates(dir, selected, { timeoutMs: COMMAND_TIMEOUT_MS });
  const red = results.filter(result => result.status === 'failed' && !LOCK_PATTERN.test(result.output));
  if (red.length === 0) {
    if (selected.length === fast.length && results.every(result => result.status === 'ok')) recordGreen(dir, 'fast', tree.hash);
    process.exit(0);
  }

  const skipped = results.filter(result => result.status === 'skipped').map(result => result.gate.name);
  deny(
    [
      'Commit refused: a quality gate is red. Fix the cause, then commit again. Do not lower a lint level and do not add a suppression.',
      ...red.flatMap(result => [
        '',
        `Quality gate "${result.gate.name}" is red (${result.gate.command}).`,
        ...(result.gate.fix ? [`Auto-fixable with: ${result.gate.fix}`] : []),
        ...(result.gate.note ? [`Note: ${result.gate.note}`] : []),
        ...errorLines(result.output, MAX_ERROR_LINES),
      ]),
      ...(skipped.length > 0 ? ['', `Not run because a gate before them is red: ${skipped.join(', ')}`] : []),
    ].join('\n'),
  );
} catch {
  // Fail open: CI still checks
}
