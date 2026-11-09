// Stop: before Claude may end a turn that changed code, the fast gates from
// the gate configuration (.agents/gates.json) must be green. A red gate blocks the stop and hands
// Claude the errors -- deterministic, unlike a rule that asks for the same
// thing. Gates without `needs` between them run at the same time, and one
// block names every red gate.
//
// - Runs only when the working tree differs from the session start and from
//   the last green run: a turn without changes costs nothing.
// - Only gates whose `paths` match a path changed in this session run: a turn
//   that touched only docs/ does not wait for clippy.
// - A new turn over the tree the last red run already reported is not checked
//   again: the errors reached Claude once, and blocking every later answer
//   would only repeat them. A change to the tree checks again.
// - At most MAX_BLOCKS blocks per turn, then the turn ends with a message to
//   the user. Claude Code caps consecutive blocks itself; ending with an
//   explanation is better than hitting that cap.
// - Locked files (a running dev server, an open IDE) are not a code problem
//   and do not block.
// - STOP_GATE=off skips the gate, e.g. for a bulk migration.

import {
  changedPaths,
  errorLines,
  gateApplies,
  loadGates,
  loadState,
  print,
  projectDir,
  readStdinJson,
  runGates,
  saveState,
  treeState,
} from './hook-lib.mjs';

const MAX_BLOCKS = 3;
const MAX_ERROR_LINES = 30;
const COMMAND_TIMEOUT_MS = 240000;
const LOCK_PATTERN = /being used by another process|EBUSY|os error 32/i;

function isLocked(result) {
  return result.status === 'failed' && LOCK_PATTERN.test(result.output);
}

function redSection(result) {
  const fix = result.gate.fix ? [`Auto-fixable with: ${result.gate.fix}`] : [];
  return ['', `Quality gate "${result.gate.name}" is red (${result.gate.command}).`, ...fix, ...errorLines(result.output, MAX_ERROR_LINES)];
}

try {
  const input = await readStdinJson();
  const dir = projectDir(input);
  const gates = loadGates(dir);
  const tree = treeState(dir);

  if (process.env.STOP_GATE === 'off' || !gates?.fast?.length || !tree) {
    process.exit(0);
  }

  const state = loadState(dir, input.session_id);
  const unchanged = tree.hash === state.baseline || tree.hash === state.green || (state.baseline === undefined && !tree.dirty);
  // stop_hook_active: Claude stops again right after a block, within the same turn
  const newTurn = input.stop_hook_active !== true;
  if (unchanged || (newTurn && tree.hash === state.red)) {
    process.exit(0);
  }

  const paths = changedPaths(dir, state.baseHead);
  const selected = gates.fast.filter(gate => gateApplies(gate, paths));
  if (selected.length === 0) {
    saveState(dir, input.session_id, { ...state, green: tree.hash, blocks: 0 });
    process.exit(0);
  }
  const blocksThisTurn = newTurn ? 0 : (state.blocks ?? 0);

  const results = await runGates(dir, selected, { timeoutMs: COMMAND_TIMEOUT_MS });
  const red = results.filter(result => result.status === 'failed' && !isLocked(result));

  if (red.length === 0) {
    const unchecked = results.filter(result => result.status !== 'ok');
    if (unchecked.length > 0) {
      const reason = result => (result.status === 'timeout' ? 'time limit' : isLocked(result) ? 'file locked' : 'not started');
      print({ systemMessage: unchecked.map(result => `Quality gate "${result.gate.name}" could not run (${reason(result)}) - not blocking.`).join('\n') });
      process.exit(0);
    }
    saveState(dir, input.session_id, { ...state, green: tree.hash, blocks: 0 });
    process.exit(0);
  }

  const blocks = blocksThisTurn + 1;
  if (blocks > MAX_BLOCKS) {
    saveState(dir, input.session_id, { ...state, red: tree.hash, blocks: 0 });
    const commands = red.map(result => `"${result.gate.name}" (${result.gate.command})`).join(', ');
    print({ systemMessage: `Quality gate ${commands} is still red after ${MAX_BLOCKS} attempts.` });
    process.exit(0);
  }

  saveState(dir, input.session_id, { ...state, red: tree.hash, blocks });
  const skipped = results.filter(result => result.status === 'skipped').map(result => result.gate.name);
  print({
    decision: 'block',
    reason: [
      'Fix the cause. Do not lower a lint level and do not add a suppression.',
      ...red.flatMap(redSection),
      ...(skipped.length > 0 ? ['', `Not run because a gate before them is red: ${skipped.join(', ')}`] : []),
    ].join('\n'),
  });
} catch {
  // A broken gate must not trap the session: fail open, CI still checks
}
