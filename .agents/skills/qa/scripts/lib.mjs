// Gate tooling shared by the qa, spec-build and spec-check skills and by the
// agent adapters (e.g. the Claude Code hooks). Plain Node without
// dependencies, so it runs in every clone and under every agent. `.mjs`
// because a `.js` file would be ESM or CommonJS depending on the nearest
// package.json.
//
// Source of truth: https://github.com/amigo-labs/skills (skills/qa/scripts/).
// A consuming repository vendors it with `node .agents/sync.mjs`; edit it
// there, not in the copy.

import { spawn, spawnSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** Where the gate configuration lives, relative to the repository root, in lookup order. */
export const GATES_FILES = ['.agents/gates.json', '.claude/gates.json'];

/** The repository root: the scripts sit at <root>/<agent dir>/skills/qa/scripts/. */
export function repoRootFromScript(scriptUrl) {
  return resolve(dirname(fileURLToPath(scriptUrl)), '..', '..', '..', '..');
}

/** The gate configuration of a repository, or null when there is none (or it is broken). */
export function loadGates(dir) {
  for (const file of GATES_FILES) {
    let text;
    try {
      text = readFileSync(join(dir, file), 'utf8');
    } catch {
      continue;
    }
    try {
      return { ...JSON.parse(text), $file: file };
    } catch {
      return null;
    }
  }
  return null;
}

const MAX_GATE_OUTPUT = 32 * 1024 * 1024;
// "nicht gefunden": cmd.exe reports a missing command in the system language
const ERROR_LINE = /error|×|✖|failed|FAIL|panicked|not found|not recognized|nicht gefunden/i;

/** A gate that runs past its time limit is killed with everything it started. */
function killTree(child) {
  try {
    if (process.platform === 'win32') spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true });
    else process.kill(-child.pid, 'SIGKILL');
  } catch {
    // already gone
  }
}

function runGate(dir, gate, timeoutMs) {
  return new Promise(resolvePromise => {
    const started = Date.now();
    let output = '';
    let timedOut = false;
    let settled = false;
    const settle = status => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolvePromise({ status: timedOut ? 'timeout' : status, output, durationMs: Date.now() - started });
    };

    // Detached only outside Windows, for the process group the timeout kills:
    // there a detached process gets no console, and its shell would open a
    // visible window for every console program it starts
    const child = spawn(gate.command, {
      cwd: join(dir, gate.cwd ?? '.'),
      shell: true,
      windowsHide: true,
      detached: process.platform !== 'win32',
      stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, FORCE_COLOR: '0', NO_COLOR: '1', CARGO_TERM_COLOR: 'never' },
    });
    const collect = chunk => {
      if (output.length < MAX_GATE_OUTPUT) output += chunk;
    };
    child.stdout.setEncoding('utf8').on('data', collect);
    child.stderr.setEncoding('utf8').on('data', collect);
    const timer = setTimeout(() => {
      timedOut = true;
      killTree(child);
    }, timeoutMs);
    child.on('error', error => {
      output += `\n${error.message}`;
      settle('failed');
    });
    child.on('close', code => settle(code === 0 ? 'ok' : 'failed'));
  });
}

/** The gates each gate waits for, as indices. An unknown id is -1 and can never be met. */
function dependencies(gates) {
  const indexById = new Map(gates.map((gate, index) => [gate.id, index]));
  return gates.map(gate => (gate.needs ?? []).map(id => indexById.get(id) ?? -1));
}

/**
 * Runs a list from gates.json: every gate as soon as the gates it needs are
 * green, so independent gates run at the same time. A gate whose needs are
 * not green (or can never be met) is skipped. Results come back in list
 * order, each `{ gate, status, output, durationMs, blockedBy? }` with status
 * ok | failed | timeout | skipped; `onSettled` sees each one as it finishes.
 */
export function runGates(dir, gates, { timeoutMs, onSettled = () => {} }) {
  const needs = dependencies(gates);
  const results = gates.map(() => undefined);
  let running = 0;

  return new Promise(resolvePromise => {
    const finish = (index, result) => {
      results[index] = { gate: gates[index], ...result };
      onSettled(results[index]);
    };

    const schedule = () => {
      let changed = true;
      while (changed) {
        changed = false;
        for (const [index, gate] of gates.entries()) {
          if (results[index]) continue;
          const blocking = needs[index].find(need => need === -1 || (results[need] && !['ok', 'running'].includes(results[need].status)));
          if (blocking !== undefined) {
            finish(index, { status: 'skipped', output: '', durationMs: 0, blockedBy: blocking === -1 ? null : gates[blocking].name });
            changed = true;
          } else if (needs[index].every(need => results[need]?.status === 'ok')) {
            results[index] = { gate, status: 'running' };
            running++;
            runGate(dir, gate, timeoutMs).then(result => {
              running--;
              finish(index, result);
              schedule();
            });
          }
        }
      }
      if (running > 0) return;
      // Whatever is still waiting now waits for itself: a cycle
      for (const index of gates.keys()) {
        if (!results[index]) finish(index, { status: 'skipped', output: '', durationMs: 0, blockedBy: null });
      }
      resolvePromise(results);
    };

    schedule();
  });
}

/** The lines of a gate's output that name the errors, or its last lines if none does. */
export function errorLines(output, max) {
  const lines = output
    .split(/\r?\n/)
    .map(line => line.trimEnd())
    .filter(Boolean);
  const errors = [...new Set(lines.filter(line => ERROR_LINE.test(line)))];
  return (errors.length > 0 ? errors : lines.slice(-max)).slice(0, max);
}

/**
 * Whether the spec skills commit without asking: on unless the developer sets
 * SPEC_AUTOCOMMIT=false in their environment (for Claude Code e.g. under `env`
 * in .claude/settings.local.json). Committing on someone's behalf is up to
 * that person, so this is never read from a tracked file.
 */
export function commitMode(env = process.env) {
  const value = env.SPEC_AUTOCOMMIT;
  if (value === undefined || value === '' || value === 'true') return { autoCommit: true };
  if (value === 'false') return { autoCommit: false };
  return { autoCommit: true, problem: `invalid SPEC_AUTOCOMMIT=${JSON.stringify(value)} (allowed: true, false)` };
}

/** The lines of a level-2 section up to the next level-2 heading. */
function sectionLines(lines, heading) {
  const start = lines.findIndex(line => line.trim().toLowerCase() === heading.toLowerCase());
  if (start < 0) return [];
  const body = lines.slice(start + 1);
  const end = body.findIndex(line => line.startsWith('## '));
  return end < 0 ? body : body.slice(0, end);
}

/**
 * One line per feature spec directly under docs/specs/ that is still
 * `status: todo` and has an `## Acceptance criteria` section: how many of its
 * criteria the Evidence table shows as built, plus the open questions that
 * keep spec-build from starting. Design documents (other status, no criteria)
 * are skipped rather than guessed at.
 */
export function openSpecs(dir) {
  const specsDir = join(dir, 'docs', 'specs');
  let names;
  try {
    names = readdirSync(specsDir).filter(name => name.endsWith('.md'));
  } catch {
    return [];
  }

  const summaries = [];
  for (const name of names.sort()) {
    let content;
    try {
      content = readFileSync(join(specsDir, name), 'utf8').replace(/\r\n/g, '\n');
    } catch {
      continue;
    }
    const frontmatter = content.match(/^---\n([\s\S]*?)\n---/);
    if (!frontmatter || !/^status:\s*todo\s*$/m.test(frontmatter[1])) continue;

    const lines = content.split('\n');
    const criteria = sectionLines(lines, '## Acceptance criteria').filter(line => /^- \[[ xX]\] AC-\d+/.test(line));
    if (criteria.length === 0) continue;
    const proven = sectionLines(lines, '## Evidence').filter(line => /^\|\s*AC-\d+\s*\|.*\|\s*(built|verified)\s*\|/.test(line));
    const openQuestions = sectionLines(lines, '## Open questions').filter(line => /^\s*[-*] \S/.test(line));

    const specName = name.slice(0, -'.md'.length);
    const open = openQuestions.length > 0 ? `, ${openQuestions.length} open questions` : '';
    // spec-build refuses a spec with open questions: they are settled in /spec first
    const next = openQuestions.length > 0 ? `/spec ${specName}` : `/spec-build ${specName}`;
    summaries.push(`${specName}: ${proven.length}/${criteria.length} ACs proven${open} -> ${next}`);
  }
  return summaries;
}
