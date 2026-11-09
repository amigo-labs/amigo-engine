// Claude Code SessionStart hook: remembers the state of the working tree the session started
// with. The Stop gate only checks once the tree differs from it -- otherwise
// a question asked in a repository that is red already would be blocked for
// changes Claude never made. The start commit lets the Stop gate tell which
// paths this session changed.
//
// It also hands the session the state of the open feature specs, so "carry
// on" or a new feature request finds its spec without searching first.

import { headCommit, loadState, openSpecs, print, projectDir, readStdinJson, saveState, treeState } from './hook-lib.mjs';

try {
  const input = await readStdinJson();
  const dir = projectDir(input);
  const tree = treeState(dir);
  if (tree) {
    const previous = loadState(dir, input.session_id);
    saveState(dir, input.session_id, { ...previous, baseline: tree.hash, baseHead: previous.baseHead ?? headCommit(dir), blocks: 0 });
  }

  const specs = openSpecs(dir);
  if (specs.length > 0) {
    const additionalContext = ['Open feature specs under docs/specs/ (state and next command):', ...specs.map(line => `- ${line}`)].join('\n');
    print({ hookSpecificOutput: { hookEventName: 'SessionStart', additionalContext } });
  }
} catch {
  // A hook must never break the session start
}
