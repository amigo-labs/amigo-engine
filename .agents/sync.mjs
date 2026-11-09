#!/usr/bin/env node
// Vendors the shared agent skills of amigo-labs/skills into a repository and
// keeps them in sync, for every coding agent at once:
//
//   .agents/skills/<name>/   the canonical copy (Codex, Gemini CLI, Copilot,
//                            Cursor, OpenCode, … read this folder)
//   .claude/skills/<name>/   a plain copy for Claude Code, which reads only its
//                            own folder (plus any other folder in `mirrors`)
//
// Copies rather than symlinks or submodules: a fresh clone, a cloud session
// and a Windows checkout without symlink support all see the same files.
//
// Usage, from the repository root (Node 18+, no dependencies):
//
//   node .agents/sync.mjs                  update from the source in .agents/skills.json
//   node .agents/sync.mjs --from ../skills  update from a local checkout of amigo-labs/skills
//   node .agents/sync.mjs --ref <ref>       update from another branch, tag or commit
//   node .agents/sync.mjs --mirror          only copy .agents/skills to the mirrors
//                                           (after editing a skill of this repository)
//   node .agents/sync.mjs --check           offline drift check for CI and the gates
//
// Configuration: .agents/skills.json
//
//   {
//     "source": "https://github.com/amigo-labs/skills.git",
//     "ref": "main",
//     "skills": ["spec", "spec-build", "qa"],     shared skills to vendor
//     "mirrors": [".claude/skills"],              copies of .agents/skills
//     "claudeHooks": true,                        vendor adapters/claude/hooks to .claude/hooks
//     "agentsMd": ["spec-workflow"]               managed blocks in AGENTS.md
//   }
//
// Skills only this repository has live in .agents/skills too; they are
// mirrored but never overwritten. The lock file .agents/skills.lock.json
// records the source commit and a hash of every vendored file, so --check can
// tell a local edit of a vendored file (change it in amigo-labs/skills
// instead) from a stale mirror (run --mirror).
//
// Source of truth: https://github.com/amigo-labs/skills (sync.mjs).

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve, sep } from 'node:path';

const CONFIG = '.agents/skills.json';
const LOCK = '.agents/skills.lock.json';
const SKILLS_DIR = '.agents/skills';
const HOOKS_DIR = '.claude/hooks';
const SELF = '.agents/sync.mjs';
const AGENTS_MD = 'AGENTS.md';
const DEFAULTS = { source: 'https://github.com/amigo-labs/skills.git', ref: 'main', skills: [], mirrors: ['.claude/skills'], claudeHooks: false, agentsMd: [] };

const root = process.cwd();

function fail(message) {
  console.error(message);
  process.exit(2);
}

function posix(path) {
  return path.split(sep).join('/');
}

/** Text with LF line endings; a file with a NUL byte stays as it is. */
function normalized(buffer) {
  if (buffer.includes(0)) return buffer;
  return Buffer.from(buffer.toString('utf8').replace(/\r\n/g, '\n'), 'utf8');
}

function hash(buffer) {
  return createHash('sha256').update(normalized(buffer)).digest('hex');
}

/** Every file below a directory, as paths relative to it (forward slashes). */
function filesBelow(dir) {
  if (!existsSync(dir)) return [];
  const out = [];
  const walk = current => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const full = join(current, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile()) out.push(posix(relative(dir, full)));
    }
  };
  walk(dir);
  return out.sort();
}

function subdirs(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter(entry => entry.isDirectory())
    .map(entry => entry.name)
    .sort();
}

function writeFile(path, buffer) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, normalized(buffer));
}

function readJson(path, fallback) {
  try {
    return JSON.parse(readFileSync(join(root, path), 'utf8'));
  } catch (error) {
    if (error.code === 'ENOENT') return fallback;
    fail(`${path} is not valid JSON: ${error.message}`);
  }
}

function git(args, cwd) {
  const result = spawnSync('git', args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) fail(`git ${args.join(' ')} failed:\n${result.stderr || result.stdout}`);
  return result.stdout.trim();
}

/** A directory holding the source at the wanted ref, plus its commit. */
function fetchSource(config, from) {
  if (from) {
    const dir = resolve(from);
    if (!existsSync(join(dir, 'skills'))) fail(`${from} is not a checkout of amigo-labs/skills (no skills/ folder).`);
    let commit = 'local';
    const head = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: dir, encoding: 'utf8' });
    if (head.status === 0) {
      commit = head.stdout.trim();
      const status = spawnSync('git', ['status', '--porcelain'], { cwd: dir, encoding: 'utf8' });
      if (status.stdout.trim()) commit += '-dirty';
    }
    return { dir, commit, cleanup: () => {} };
  }
  const dir = mkdtempSync(join(tmpdir(), 'agent-skills-'));
  git(['init', '--quiet'], dir);
  git(['fetch', '--quiet', '--depth', '1', config.source, config.ref], dir);
  git(['checkout', '--quiet', 'FETCH_HEAD'], dir);
  return { dir, commit: git(['rev-parse', 'HEAD'], dir), cleanup: () => rmSync(dir, { recursive: true, force: true }) };
}

const BLOCK_BEGIN = name => `<!-- amigo-skills:begin ${name} -->`;
const BLOCK_END = name => `<!-- amigo-skills:end ${name} -->`;
const BLOCK_NOTE = '<!-- Managed by .agents/sync.mjs from amigo-labs/skills (agents-md/). Edit it there, not here. -->';

/** The body of a managed block in a text, or null when its markers are missing. */
function blockBody(text, name) {
  const begin = text.indexOf(BLOCK_BEGIN(name));
  const end = text.indexOf(BLOCK_END(name));
  if (begin < 0 || end < begin) return null;
  return text
    .slice(begin + BLOCK_BEGIN(name).length, end)
    .replace(/\r\n/g, '\n')
    .replace(BLOCK_NOTE, '')
    .trim();
}

function replaceBlock(text, name, body) {
  const block = `${BLOCK_BEGIN(name)}\n${BLOCK_NOTE}\n\n${body.trim()}\n\n${BLOCK_END(name)}`;
  const begin = text.indexOf(BLOCK_BEGIN(name));
  const end = text.indexOf(BLOCK_END(name));
  if (begin >= 0 && end > begin) return text.slice(0, begin) + block + text.slice(end + BLOCK_END(name).length);
  return `${text.trimEnd()}\n\n${block}\n`;
}

/** The skills a SKILL.md names under `metadata: requires:` (space-separated). */
function requires(skillMd) {
  const frontmatter = skillMd.replace(/\r\n/g, '\n').match(/^---\n([\s\S]*?)\n---/);
  const line = frontmatter?.[1].match(/^\s+requires:\s*(.+)$/m);
  return line ? line[1].trim().split(/\s+/) : [];
}

/** Makes every mirror an exact copy of .agents/skills. */
function mirror(config) {
  const sourceDir = join(root, SKILLS_DIR);
  const files = filesBelow(sourceDir);
  for (const target of config.mirrors) {
    const targetDir = join(root, target);
    const wanted = new Set(files);
    for (const file of filesBelow(targetDir)) if (!wanted.has(file)) rmSync(join(targetDir, file));
    for (const name of subdirs(targetDir)) if (filesBelow(join(targetDir, name)).length === 0) rmSync(join(targetDir, name), { recursive: true });
    for (const file of files) writeFile(join(targetDir, file), readFileSync(join(sourceDir, file)));
  }
  console.log(`Mirrored ${subdirs(sourceDir).length} skills to ${config.mirrors.join(', ') || '(no mirrors)'}.`);
}

function update(config, options) {
  const previous = readJson(LOCK, { files: {}, blocks: {} });
  const source = fetchSource({ ...config, ref: options.ref ?? config.ref }, options.from);
  try {
    const files = {};
    const install = (from, to) => {
      const buffer = readFileSync(join(source.dir, from));
      writeFile(join(root, to), buffer);
      files[to] = hash(buffer);
    };

    for (const name of config.skills) {
      const from = join(source.dir, 'skills', name);
      if (!existsSync(join(from, 'SKILL.md'))) fail(`Skill "${name}" does not exist in the source (skills/${name}/SKILL.md).`);
      for (const needed of requires(readFileSync(join(from, 'SKILL.md'), 'utf8'))) {
        if (!config.skills.includes(needed)) fail(`Skill "${name}" requires "${needed}": add it to "skills" in ${CONFIG}.`);
      }
    }
    if (config.claudeHooks && !config.skills.includes('qa')) fail(`"claudeHooks" needs the "qa" skill (the hooks run its gate scripts): add it to "skills" in ${CONFIG}.`);
    for (const name of config.skills) {
      const from = join(source.dir, 'skills', name);
      rmSync(join(root, SKILLS_DIR, name), { recursive: true, force: true });
      for (const file of filesBelow(from)) install(`skills/${name}/${file}`, `${SKILLS_DIR}/${name}/${file}`);
    }
    if (config.claudeHooks) {
      for (const file of filesBelow(join(source.dir, 'adapters', 'claude', 'hooks'))) install(`adapters/claude/hooks/${file}`, `${HOOKS_DIR}/${file}`);
    }
    install('sync.mjs', SELF);

    // Vendored files of the previous sync that this one no longer brings
    for (const path of Object.keys(previous.files ?? {})) {
      if (files[path]) continue;
      rmSync(join(root, path), { force: true });
      const skill = path.startsWith(`${SKILLS_DIR}/`) ? path.split('/')[2] : null;
      if (skill && filesBelow(join(root, SKILLS_DIR, skill)).length === 0) rmSync(join(root, SKILLS_DIR, skill), { recursive: true, force: true });
    }

    const blocks = {};
    if (config.agentsMd.length > 0) {
      const path = join(root, AGENTS_MD);
      let text = existsSync(path) ? readFileSync(path, 'utf8').replace(/\r\n/g, '\n') : '# AGENTS.md\n';
      for (const name of config.agentsMd) {
        const blockFile = join(source.dir, 'agents-md', `${name}.md`);
        if (!existsSync(blockFile)) fail(`AGENTS.md block "${name}" does not exist in the source (agents-md/${name}.md).`);
        const body = readFileSync(blockFile, 'utf8').replace(/\r\n/g, '\n').trim();
        text = replaceBlock(text, name, body);
        blocks[name] = hash(Buffer.from(body));
      }
      writeFileSync(path, text);
    }

    mirror(config);
    const lock = { $comment: 'Written by .agents/sync.mjs. Do not edit.', source: config.source, ref: options.ref ?? config.ref, commit: source.commit, files, blocks };
    writeFileSync(join(root, LOCK), `${JSON.stringify(lock, null, 2)}\n`);
    console.log(`Synced ${config.skills.length} skills from ${config.source} @ ${source.commit}.`);
  } finally {
    source.cleanup();
  }
}

function check(config) {
  const lock = readJson(LOCK, null);
  if (!lock) fail(`${LOCK} is missing: run \`node ${SELF}\` once.`);
  const problems = [];

  for (const [path, expected] of Object.entries(lock.files ?? {})) {
    const full = join(root, path);
    if (!existsSync(full)) problems.push(`${path}: vendored file is missing (run \`node ${SELF}\`).`);
    else if (hash(readFileSync(full)) !== expected) problems.push(`${path}: changed locally. Vendored files are edited in amigo-labs/skills, then synced.`);
  }
  for (const name of config.skills) {
    if (!Object.keys(lock.files ?? {}).some(path => path.startsWith(`${SKILLS_DIR}/${name}/`))) problems.push(`Skill "${name}" is configured but not synced (run \`node ${SELF}\`).`);
  }
  for (const path of Object.keys(lock.files ?? {})) {
    const skill = path.startsWith(`${SKILLS_DIR}/`) ? path.split('/')[2] : null;
    if (skill && !config.skills.includes(skill)) problems.push(`Skill "${skill}" is synced but no longer configured (run \`node ${SELF}\`).`);
  }

  if (config.agentsMd.length > 0) {
    const text = existsSync(join(root, AGENTS_MD)) ? readFileSync(join(root, AGENTS_MD), 'utf8') : '';
    for (const name of config.agentsMd) {
      const body = blockBody(text, name);
      if (body === null) problems.push(`AGENTS.md: managed block "${name}" is missing (run \`node ${SELF}\`).`);
      else if (lock.blocks?.[name] !== hash(Buffer.from(body))) problems.push(`AGENTS.md: managed block "${name}" changed locally. Edit agents-md/${name}.md in amigo-labs/skills, then sync.`);
    }
  }

  const sourceDir = join(root, SKILLS_DIR);
  const files = filesBelow(sourceDir);
  for (const target of config.mirrors) {
    const targetDir = join(root, target);
    const present = filesBelow(targetDir);
    const stale = files.filter(file => !existsSync(join(targetDir, file)) || hash(readFileSync(join(targetDir, file))) !== hash(readFileSync(join(sourceDir, file))));
    const extra = present.filter(file => !files.includes(file));
    if (stale.length > 0 || extra.length > 0) {
      const list = [...stale, ...extra].slice(0, 5).join(', ');
      problems.push(`${target} differs from ${SKILLS_DIR} (${list}${stale.length + extra.length > 5 ? ', …' : ''}). Edit skills in ${SKILLS_DIR} only, then run \`node ${SELF} --mirror\`.`);
    }
  }

  if (problems.length > 0) {
    console.error(['Agent skills are out of sync:', ...problems.map(problem => `- ${problem}`)].join('\n'));
    process.exit(1);
  }
  console.log(`Agent skills in sync (${subdirs(sourceDir).length} skills, source ${lock.commit.slice(0, 12)}).`);
}

function parseArgs(argv) {
  const options = { mode: 'update' };
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index];
    if (arg === '--check') options.mode = 'check';
    else if (arg === '--mirror') options.mode = 'mirror';
    else if (arg === '--from') options.from = argv[++index];
    else if (arg === '--ref') options.ref = argv[++index];
    else if (arg === '--help' || arg === '-h') options.mode = 'help';
    else fail(`Unknown argument "${arg}". See \`node ${SELF} --help\`.`);
  }
  if ((options.from === undefined && argv.includes('--from')) || (options.ref === undefined && argv.includes('--ref'))) fail('--from and --ref need a value.');
  return options;
}

const options = parseArgs(process.argv.slice(2));
if (options.mode === 'help') {
  console.log(readFileSync(new URL(import.meta.url), 'utf8').split('\n').filter(line => line.startsWith('//')).map(line => line.slice(3)).join('\n'));
  process.exit(0);
}
const raw = readJson(CONFIG, null);
if (!raw) fail(`${CONFIG} is missing. Run this from the repository root; see the README of amigo-labs/skills.`);
const config = { ...DEFAULTS, ...raw };
if (!statSync(root).isDirectory()) fail('No working directory.');

if (options.mode === 'check') check(config);
else if (options.mode === 'mirror') mirror(config);
else update(config, options);
