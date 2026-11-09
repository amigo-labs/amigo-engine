// Shared by the Claude Code hooks. The gate configuration and the gate runner
// are agent-neutral and live with the qa skill (.agents/skills/qa/scripts/);
// this file only adds what a Claude Code hook needs: its stdin, the session
// state and the working-tree fingerprint.
//
// Source of truth: https://github.com/amigo-labs/skills (adapters/claude/hooks/).
// Vendored by `node .agents/sync.mjs`; edit it there, not in the copy.

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export { errorLines, loadGates, openSpecs, runGates } from '../../.agents/skills/qa/scripts/lib.mjs';

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

function git(dir, args) {
  const result = spawnSync('git', ['-C', dir, ...args], { encoding: 'buffer', timeout: 10000, maxBuffer: 64 * 1024 * 1024 });
  return result.status === 0 ? result.stdout : null;
}

export function headCommit(dir) {
  return git(dir, ['rev-parse', 'HEAD'])?.toString('utf8').trim() ?? null;
}

/** Repository-relative paths (forward slashes) of every uncommitted change. */
function uncommittedPaths(dir) {
  const status = git(dir, ['status', '--porcelain=v1', '-z', '--untracked-files=all']);
  if (status === null) return null;
  const paths = [];
  const entries = status.toString('utf8').split('\0');
  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index];
    if (entry.length < 4) continue;
    paths.push(entry.slice(3));
    // Renames and copies carry their original path as the next entry
    if (entry[0] === 'R' || entry[0] === 'C') paths.push(entries[++index]);
  }
  return paths.filter(Boolean);
}

/**
 * A fingerprint of the working tree: HEAD plus every uncommitted path and its
 * content. Equal fingerprints mean nothing changed since the last check.
 */
export function treeState(dir) {
  const head = git(dir, ['rev-parse', 'HEAD']) ?? Buffer.alloc(0);
  const paths = uncommittedPaths(dir);
  if (paths === null) return null;

  const hash = createHash('sha256').update(head);
  for (const filePath of paths) {
    try {
      hash.update(filePath).update(readFileSync(join(dir, filePath)));
    } catch {
      hash.update(filePath).update('<deleted>');
    }
  }
  return { hash: hash.digest('hex'), dirty: paths.length > 0 };
}

/**
 * Paths changed in this session: everything uncommitted plus what the commits
 * since `baseHead` touched. Without a base only the uncommitted paths count.
 */
export function changedPaths(dir, baseHead) {
  const paths = new Set(uncommittedPaths(dir) ?? []);
  if (baseHead) {
    const committed = git(dir, ['diff', '--name-only', '-z', baseHead, 'HEAD']);
    for (const path of committed?.toString('utf8').split('\0') ?? []) if (path) paths.add(path);
  }
  return [...paths].map(path => path.replaceAll('\\', '/'));
}

/** Whether a gate concerns the given changes: no `paths` means always. */
export function gateApplies(gate, paths) {
  if (!Array.isArray(gate.paths)) return true;
  return paths.some(path => gate.paths.some(prefix => path.startsWith(prefix)));
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
