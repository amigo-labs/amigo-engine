// Shared by the Claude Code hooks. The gate configuration and the gate runner
// are agent-neutral and live with the qa skill (.agents/skills/qa/scripts/);
// this file only adds what a Claude Code hook needs: its stdin, the session
// state and the working-tree fingerprint.
//
// Source of truth: https://github.com/amigo-labs/skills (adapters/claude/hooks/).
// Vendored by `node .agents/sync.mjs`; edit it there, not in the copy.

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export {
  changedPaths,
  commitPaths,
  errorLines,
  expandGates,
  gateApplies,
  headCommit,
  lastGreen,
  loadGates,
  openSpecs,
  recordGreen,
  runGates,
  treeState,
} from '../../.agents/skills/qa/scripts/lib.mjs';

// Claude Code writes one JSON document to stdin. On Windows the stream's 'end'
// does not reliably follow, so parse on every chunk and settle as soon as the
// document is complete; the idle timeout is only the fallback.
const STDIN_IDLE_TIMEOUT_MS = 1000;

export function readStdinJson() {
  return new Promise(resolve => {
    let data = '';
    let settled = false;
    let timer = null;

    const settle = value => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      try {
        process.stdin.destroy();
      } catch {
        // ignore
      }
      resolve(value);
    };
    const tryParse = () => {
      try {
        return JSON.parse(data);
      } catch {
        return undefined;
      }
    };
    const arm = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => settle(tryParse() ?? {}), STDIN_IDLE_TIMEOUT_MS);
    };

    process.stdin.setEncoding('utf8');
    process.stdin.on('data', chunk => {
      data += chunk;
      arm();
      if (!/\}\s*$/.test(data)) return;
      const parsed = tryParse();
      if (parsed !== undefined) settle(parsed);
    });
    process.stdin.on('end', () => settle(tryParse() ?? {}));
    process.stdin.on('error', () => settle({}));
    arm();
  });
}

export function projectDir(input) {
  return process.env.CLAUDE_PROJECT_DIR || input.cwd || process.cwd();
}

function stateFile(dir, sessionId) {
  const key = createHash('sha256')
    .update(`${dir}\0${sessionId ?? 'unknown'}`)
    .digest('hex')
    .slice(0, 24);
  return join(tmpdir(), 'claude-gates', `${key}.json`);
}

export function loadState(dir, sessionId) {
  try {
    return JSON.parse(readFileSync(stateFile(dir, sessionId), 'utf8'));
  } catch {
    return {};
  }
}

export function saveState(dir, sessionId, state) {
  const folder = join(tmpdir(), 'claude-gates');
  if (!existsSync(folder)) mkdirSync(folder, { recursive: true });
  writeFileSync(stateFile(dir, sessionId), JSON.stringify(state));
}

export function print(output) {
  process.stdout.write(JSON.stringify(output));
}
