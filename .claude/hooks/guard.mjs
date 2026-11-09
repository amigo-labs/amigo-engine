// Claude Code PreToolUse hook (Edit|Write|MultiEdit): refuses edits that add a
// suppression to code (`#[allow(`, `@ts-ignore`, `#[ignore`, ... -- the list
// is `suppressions` in the gate configuration, the file types are
// `suppressionExtensions`) instead of fixing the cause. AGENTS.md forbids them
// unless justified; that justification needs a human.
//
// Counted is what the edit ADDS: existing suppressions stay editable. This is
// early feedback, not the safety net -- Bash can write files past it. The net
// is code review and CI.

import { existsSync, readFileSync } from 'node:fs';
import { extname, isAbsolute, join, relative } from 'node:path';

import { loadGates, print, projectDir, readStdinJson } from './hook-lib.mjs';

// Used when the gate configuration names no `suppressionExtensions`
const DEFAULT_EXTENSIONS = ['.rs', '.ts', '.mts', '.cts', '.js', '.mjs', '.cjs', '.svelte', '.css'];
// Agent configuration and vendored skills may quote the patterns
const EXEMPT_DIRS = new Set(['.claude', '.agents']);

function occurrences(text, pattern) {
  return text.split(pattern).length - 1;
}

/** The file content before and after the tool call. */
function contents(toolName, toolInput, filePath) {
  const before = existsSync(filePath) ? readFileSync(filePath, 'utf8') : '';
  if (toolName === 'Write') return { before, after: toolInput.content ?? '' };

  const edits = toolName === 'MultiEdit' ? (toolInput.edits ?? []) : [toolInput];
  let after = before;
  for (const edit of edits) {
    const oldString = edit.old_string ?? '';
    const newString = edit.new_string ?? '';
    after = edit.replace_all ? after.split(oldString).join(newString) : after.replace(oldString, () => newString);
  }
  return { before, after };
}

function deny(reason) {
  print({ hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'deny', permissionDecisionReason: reason } });
}

try {
  const input = await readStdinJson();
  const dir = projectDir(input);
  const toolInput = input.tool_input ?? {};
  const filePath = toolInput.file_path ? (isAbsolute(toolInput.file_path) ? toolInput.file_path : join(dir, toolInput.file_path)) : null;
  const relativePath = filePath === null ? null : relative(dir, filePath);
  const inProject = relativePath !== null && !relativePath.startsWith('..') && !isAbsolute(relativePath);
  const exempt = inProject && EXEMPT_DIRS.has(relativePath.split(/[\\/]/)[0]);
  const gates = loadGates(dir);
  const extensions = new Set((gates?.suppressionExtensions ?? DEFAULT_EXTENSIONS).map(extension => extension.toLowerCase()));

  if (inProject && !exempt && extensions.has(extname(filePath).toLowerCase())) {
    const { before, after } = contents(input.tool_name, toolInput, filePath);
    const patterns = gates?.suppressions ?? [];
    const added = patterns.filter(pattern => occurrences(after, pattern) > occurrences(before, pattern));
    if (added.length > 0) {
      deny(
        `New suppression (${added.join(', ')}) refused: fix the cause instead of silencing the warning or test. ` +
          'If it is truly justified (AGENTS.md says when, e.g. a documented reason or an issue link), stop and ask the user to add it.',
      );
    }
  }
} catch {
  // Fail open: review and CI are the safety net
}
