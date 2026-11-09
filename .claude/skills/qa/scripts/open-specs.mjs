// Lists the open feature specs under docs/specs/ with their state and the
// next command. Agents without a session-start hook run it at the start of
// a task; the Claude Code adapter hands the same lines to every session.
//
//   node .agents/skills/qa/scripts/open-specs.mjs

import { openSpecs, repoRootFromScript } from './lib.mjs';

const specs = openSpecs(repoRootFromScript(import.meta.url));
console.log(specs.length > 0 ? ['Open feature specs (state and next command):', ...specs.map(line => `- ${line}`)].join('\n') : 'No open feature specs.');
