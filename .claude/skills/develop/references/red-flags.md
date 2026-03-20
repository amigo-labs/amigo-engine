# Red Flags — When to Stop, Change Course, or Revert

## Immediate Stop (discuss with user before continuing)

**3+ failed fixes on the same problem.**
You have tried three different approaches and none work. This is not an implementation problem — it is an architecture problem or a misunderstanding of the requirements. Stop. Explain what you tried and why each failed. Ask the user how to proceed.

**Scope is growing beyond the original request.**
The user asked for X, and you find yourself also doing Y and Z "because they're related." Stop. Complete X first. Suggest Y and Z as separate tasks.

**Tests are being commented out or deleted to make things pass.**
Tests exist for a reason. If a test fails after your change, either your change is wrong or the test needs to be updated to reflect intentionally changed behavior. Never delete a test to make the suite green.

**A single task is touching more than 5 files.**
The task is too large. Split it into smaller tasks. Each task should touch 1-3 files.

**You are guessing instead of reading.**
If you find yourself writing code based on assumptions about how a module works without having read that module's source code — stop and read it first.

## Course Correction (update the ADR/plan, inform user)

**A migration step revealed unexpected complexity.**
The step you planned as "simple" turned out to require changes across multiple modules. Update the ADR's migration path. Re-estimate remaining steps. Inform the user.

**The approach works but is slower/worse than expected.**
Check abort criteria. If a measurable threshold is not met, the ADR says to abandon. Follow the abort criteria.

**Feature-flagged code has been sitting unused for multiple tasks.**
If you added new code behind a feature flag several tasks ago and still haven't wired it up — either the migration path is wrong or the feature flag was premature. Re-evaluate.

**Existing tests fail and you don't understand why.**
Do not "fix" a test you don't understand. Read the test. Understand what behavior it verifies. Then determine if your change intentionally altered that behavior or if you introduced a bug.

## Revert Triggers (go back to last known good state)

**Compile errors that cascade across the codebase.**
If your change causes errors in 10+ files, revert the last commit. The approach is too invasive for a single step.

**Abort criteria from the ADR are met.**
The ADR explicitly says "if X, abandon." Follow it. Revert to the commit before the migration started.

**The user says stop.**
Immediately stop. Do not "just finish this one thing." Commit or stash what you have and wait for direction.

## Recovery Strategies

**After reverting:** Do not retry the same approach. Analyze why it failed. Write down the failure reason in the ADR's Updates section. Propose an alternative approach to the user.

**After scope creep:** Create a separate todo item or suggest a new `/develop` session for the out-of-scope work. Return focus to the original task.

**After 3+ failed fixes:** Step back and describe the problem to the user as precisely as possible. Include: what you expected, what actually happened, what you tried, and why each attempt failed. The user may have context you lack.
