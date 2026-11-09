// The qa, spec-build and spec-check skills run the gates through this script
// instead of command by command, under every agent:
//
//   node .agents/skills/qa/scripts/gates.mjs full|fast
//
// Runs every gate of the list regardless of `paths` (that filter is only for
// the agent hooks). A green `fast` run is remembered for the tree it ran on, so
// a commit gate right after it does not run the same gates again. Gates
// without `needs` between them run at the same time.
// The report on stdout is a table with the path of each gate's complete
// output plus, for every red gate, its error lines and its fix command.
// Exit 0 when everything is green, 1 otherwise, 2 without a usable gates.json
// (.agents/gates.json, or .claude/gates.json in a repository not migrated yet).

import { createHash } from 'node:crypto';
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { errorLines, GATES_FILES, loadGates, recordGreen, repoRootFromScript, runGates, treeState } from './lib.mjs';

const LISTS = ['fast', 'full'];
const GATE_TIMEOUT_MS = 30 * 60 * 1000;
const MAX_ERROR_LINES = 60;

// <root>/.agents/skills/qa/scripts/ -> repository root, wherever the script is called from
const dir = repoRootFromScript(import.meta.url);

function seconds(durationMs) {
  return `${(durationMs / 1000).toFixed(1)} s`;
}

function verdict(result) {
  switch (result.status) {
    case 'ok':
      return 'green';
    case 'failed':
      return 'red';
    case 'timeout':
      return `time limit (${GATE_TIMEOUT_MS / 60000} min)`;
    default:
      return result.blockedBy ? `skipped (${result.blockedBy} not green)` : 'skipped (unknown dependency)';
  }
}

function cell(text) {
  return text.replaceAll('|', '\\|');
}

const list = process.argv[2] ?? 'full';
const logFolder = join(tmpdir(), 'agent-gates', 'logs');
const logPrefix = `${createHash('sha256').update(dir).digest('hex').slice(0, 12)}-${list}-`;

/** Logs of the previous run: a gate skipped now must not point to its old output. */
function removeOldLogs() {
  try {
    for (const name of readdirSync(logFolder).filter(candidate => candidate.startsWith(logPrefix))) rmSync(join(logFolder, name), { force: true });
  } catch {
    // no logs yet
  }
}

/** The complete output of a gate; spec-check looks up single tests there, also in a green run. */
function writeLog(result, index) {
  mkdirSync(logFolder, { recursive: true });
  const file = join(logFolder, `${logPrefix}${index}-${result.gate.id}.log`.replace(/[^\w.-]/g, '_'));
  writeFileSync(file, result.output);
  return file;
}

if (!LISTS.includes(list)) {
  console.error(`Unknown list "${list}" (allowed: ${LISTS.join(', ')}).`);
  process.exit(2);
}

const gates = loadGates(dir)?.[list];
if (!Array.isArray(gates)) {
  console.error(`No usable gate configuration (${GATES_FILES.join(' or ')} is missing or invalid).`);
  process.exit(2);
}

removeOldLogs();
const treeBefore = treeState(dir);
const results = await runGates(dir, gates, {
  timeoutMs: GATE_TIMEOUT_MS,
  onSettled: result => console.error(`${verdict(result)}: ${result.gate.name} (${seconds(result.durationMs)})`),
});
const logs = results.map((result, index) => (result.status === 'skipped' ? null : writeLog(result, index)));

const report = ['| Gate | Command | Result | Duration | Log |', '| ---- | ------- | ------ | -------- | --- |'];
for (const [index, result] of results.entries()) {
  const duration = result.status === 'skipped' ? '-' : seconds(result.durationMs);
  report.push(`| ${cell(result.gate.name)} | \`${cell(result.gate.command)}\` | ${verdict(result)} | ${duration} | ${logs[index] ?? '-'} |`);
}

for (const [index, result] of results.entries()) {
  if (result.status !== 'failed' && result.status !== 'timeout') continue;
  report.push('', `### ${result.gate.name}: ${verdict(result)}`, '', `Command: \`${result.gate.command}\` in \`${result.gate.cwd ?? '.'}\``);
  if (result.gate.fix) report.push(`Auto-fixable with: \`${result.gate.fix}\``);
  if (result.gate.note) report.push(`Note: ${result.gate.note}`);
  report.push(`Full log: ${logs[index]}`, '', '```text', ...errorLines(result.output, MAX_ERROR_LINES), '```');
}

console.log(report.join('\n'));
const green = results.every(result => result.status === 'ok');
// The commit gate skips the fast gates on a tree they were just green on
if (green && treeBefore && treeState(dir)?.hash === treeBefore.hash) recordGreen(dir, list, treeBefore.hash);
process.exit(green ? 0 : 1);
