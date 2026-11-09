// The spec skills read the commit mode from the one line this prints. Any
// failure prints "on", the default; the skills fall back to it as well.
//
//   node .agents/skills/qa/scripts/commit-mode.mjs

import { commitMode } from './lib.mjs';

const ON = 'on - commit without asking, as the skill describes.';
const DISABLE_HINT =
  'To be asked before every commit, set SPEC_AUTOCOMMIT=false in your environment (Claude Code: "env": { "SPEC_AUTOCOMMIT": "false" } in .claude/settings.local.json).';

try {
  const mode = commitMode();
  if (mode.autoCommit) {
    const problem = mode.problem ? ` Ignored: ${mode.problem}.` : '';
    console.log(`${ON} ${DISABLE_HINT}${problem}`);
  } else {
    console.log('ask - before every commit ask the user: "Commit now" or continue without commit (SPEC_AUTOCOMMIT=false).');
  }
} catch {
  console.log(`${ON} ${DISABLE_HINT}`);
}
