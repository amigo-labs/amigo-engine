# AGENTS.md

Guidance for every coding agent working in this repository (Claude Code reads
it through `CLAUDE.md`, which only imports this file).

## What this is

`amigo-engine` is a 2D pixel-art game engine in pure Rust: deterministic
fixed-point ECS, a wgpu renderer, an egui-based level editor, AI asset
pipelines (ComfyUI), and TidalCycles-based chiptune music. Cargo workspace,
edition 2024 (MSRV in `rust-version`), MIT OR Apache-2.0.

## Before you build: system libraries

On Linux the engine links against alsa (audio), udev (gamepads), and
wayland/X11 (windowing). Without them `cargo check` fails in `libudev-sys`'s
build script, which looks like a Rust error but is not:

```sh
./scripts/install-system-deps.sh
```

That script is also wired to a `SessionStart` hook in `.claude/settings.json`,
so a fresh session installs them automatically. Keep its package list in sync
with `.github/actions/install-system-deps/action.yml`,
`docs/wiki/Installation.md`, and `CONTRIBUTING.md`.

There is no GPU in a typical cloud session. Anything that opens a window
cannot run here — use `AMIGO_HEADLESS=1` (implies the API server) and drive the
engine over JSON-RPC, or assert on the CPU-side draw lists.

## The gate

CI runs the `justfile` recipes, not copies of them, so `just ci` reproduces
CI exactly. `just` may not be installed; the recipes are then run
individually:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings   # warnings are errors
cargo test --workspace
```

Plus, before pushing, the rest of `just ci`: `clippy-features` (every crate
once per feature via cargo-hack, then `--all-features`), `test-features`,
`test-doc` (doc tests of every library crate), and `doc` (rustdoc for every
library crate with `RUSTDOCFLAGS="-D warnings"`). Feature combinations break
in ways the default build does not. CI also runs `cargo deny check`
(advisories, licenses, sources; see `deny.toml`) and the tests on Windows and
macOS.

The full matrix fills the disk of a cloud session quickly; set
`CARGO_INCREMENTAL=0` there and delete `target/debug/incremental` if a build
dies with "No space left on device".

## Layout

```
crates/     19 library crates, all prefixed amigo_
tools/      amigo_cli (the `amigo` binary), amigo_mcp, amigo_artgen,
            amigo_audiogen, amigo_comfyui
examples/   11 runnable demos, each a workspace member
docs/specs/ per-module specifications; docs/specs/index.md is the overview
            (feature specs with acceptance criteria sit at its top level)
.agents/    agent skills (mirrored to .claude/skills), gates.json, sync.mjs
docs/adrs/  architecture decisions
docs/wiki/  published to the GitHub wiki by .github/workflows/wiki-sync.yml
```

`crates/amigo_engine` is the facade: `Game` trait, `Engine`/`EngineBuilder`,
the fixed-timestep loop, `GameContext`/`DrawContext`, and the `prelude`. Most
gameplay logic lives in `crates/amigo_core` (ECS, fixed-point math, save
system, and one module per game type).

## Conventions

`docs/specs/conventions.md` is authoritative. The parts that come up most:

- **Determinism.** Simulation uses `Fix` (Q16.16) and `SimVec2`; f32 and
  `RenderVec2` are for rendering only. Do not leak f32 into simulation state —
  it breaks replays and lockstep netcode (ADR-0001).
- **ECS.** SparseSet storage. Engine-core components are typed fields on
  `World`; game components go through `insert_dynamic`/`get_dynamic` (A.2).
- **Errors.** `Result` for init and asset loading; no `Result` in the frame
  hot path — assets degrade gracefully, panics mean a real bug.
- **Logging.** `tracing`, filtered via `AMIGO_LOG` (not `RUST_LOG`), e.g.
  `AMIGO_LOG=amigo_render=trace`.
- **Render stage order** (A.6): Background → Tilemap → Entities → Particles →
  Lighting → Post-Processing → **UI, which is deliberately after
  post-processing** and must not be fed through it.
- **Data formats.** TOML for engine config (`amigo.toml`), RON for game data
  and input maps.
- **Public API.** Anything re-exported through `amigo_engine::prelude` is
  stable API; breaking it needs a major version bump.

## Specs and ADRs

A spec's own frontmatter (`status:`) is the source of truth for its state;
`docs/specs/index.md` carries a summary table that has drifted before — when
they disagree, trust the spec file and fix the table. `status: done` in this
repo has historically meant "the types exist", which is not the same as
"wired into the engine": check for actual call sites before believing a
feature works end to end.

New features follow the shared spec workflow from
[amigo-labs/skills](https://github.com/amigo-labs/skills): a feature spec
`docs/specs/<name>.md` with acceptance criteria (`spec`), built test-first
(`spec-build`), accepted against a full gate run (`spec-check`); bugs with
`spec-fix`, the gates with `qa`. The rules are the managed block
"Spec-driven workflow" at the end of this file, the project facts the skills
rely on the "Spec workflow profile". Skills live in `.agents/skills/` (every
agent) and are mirrored to `.claude/skills/` for Claude Code; `node
.agents/sync.mjs` updates them. The module specs in the subfolders of
`docs/specs/` stay design specs; an architectural change gets an ADR.

## Adding a crate

Add it to `[workspace.members]` and `[workspace.dependencies]` in the root
`Cargo.toml` (path dependency with the workspace `version`, which cargo-deny's
wildcard ban needs), inherit `version`, `edition`, `rust-version`, `license`
and `repository` with `.workspace = true`, and put shared external deps in
`[workspace.dependencies]` rather than the crate's own manifest.

## Spec workflow profile

The project facts the shared spec skills (`spec`, `spec-build`, `spec-check`,
`spec-extract`, `spec-fix`, `qa`) look up by these keys.

- **Design spec** — the per-module specs under `docs/specs/<area>/`
  (`engine/`, `gametypes/`, `assets/`, `config/`, `tooling/`,
  `ai-pipelines/`; `status: done` / `draft`, see "Specs and ADRs" for what
  `done` means here) and `docs/specs/conventions.md`, which is authoritative.
- **Spec index** — `docs/specs/index.md`; add a feature spec to its table.
- **Decisions** — an ADR `docs/adrs/NNNN-<slug>.md` with the next free number,
  listed in `docs/adrs/README.md`.
- **Code map** — `crates/amigo_engine` (facade: `Game`, `Engine` /
  `EngineBuilder`, loop, contexts, `prelude`), `crates/amigo_core` (ECS,
  fixed-point math, save system, one module per game type), the other
  `crates/amigo_*` (render, audio, input, net, scene, tilemap, ui, editor,
  assets, animation, reflect, api, debug, …), `tools/` (`amigo` CLI, MCP,
  artgen, audiogen, ComfyUI), `examples/`. Layout above.
- **Spec questions** — Which crates? Simulation or render side (simulation
  state stays `Fix` / `SimVec2`, ADR-0001)? Does it change anything
  re-exported through `amigo_engine::prelude` (stable API, semver)? Behind a
  feature flag? Data in TOML (engine config) or RON (game data)? Which render
  stage (A.6)? Front matter: `crates: [amigo_core, …]`.
- **AC subject** — "The engine …".
- **Performance AC example** — "One simulation tick with 10 000 entities takes
  under 2 ms in a release build (headless test, `AMIGO_HEADLESS=1`)".
- **Tests** — unit tests inline, integration tests in each crate's `tests/`,
  doc tests for public API, feature-gated tests (`--all-features`, single
  features as in `test-features`), replay / hash tests for determinism. There
  is no GPU in a cloud session: assert on CPU-side draw lists or drive the
  engine headless over JSON-RPC (`AMIGO_HEADLESS=1`). Red run: `cargo test -p
  <crate> <test_name>`. Manual only for what needs a window, a GPU or ears.
- **CI-only checks** — `cargo deny check`, the Windows and macOS test jobs,
  the release builds and the nightly workflow.
- **Build check** — `cargo check --workspace` (system libraries first:
  `./scripts/install-system-deps.sh`).
- **Conventions** — `docs/specs/conventions.md` and "Conventions" above
  (determinism, ECS storage, errors, `tracing` with `AMIGO_LOG`, render stage
  order, data formats, stable prelude), "Adding a crate"; lints are silenced
  only with `#[expect(.., reason = ..)]`; doc comments on public items
  (`doc` gate with `-D warnings`).
- **Stopping points** — a new dependency, a new crate, a breaking change to
  anything in `amigo_engine::prelude`, f32 in simulation state, new `unsafe`,
  a test that would have to be disabled, a decision that needs an ADR, an AC
  that turns out wrong.
- **Sizing** — one session builds about one M task (3–6 files, ~400 changed
  lines); a bigger feature is split into several specs.
- **Commits** — Conventional Commits, one concept per commit, scopes by area:
  `core`, `engine`, `render`, `audio`, `input`, `net`, `assets`, `ui`,
  `editor`, `cli`, `mcp`, `specs`, `deps`, …. Example: `feat(render): add a
  bloom pass after lighting (AC-02, AC-03)`.
- **Branches** — changes reach `main` through PRs; feature branch
  `feat/<spec-name>`.

<!-- amigo-skills:begin spec-workflow -->
<!-- Managed by .agents/sync.mjs from amigo-labs/skills (agents-md/). Edit it there, not here. -->

## Spec-driven workflow

Features are built spec-driven: first a feature spec `docs/specs/<name>.md` with testable acceptance criteria, then the implementation with tests, then an independent acceptance check. The project specifics (design spec, where decisions go, code map, tests, commits) are in the **spec workflow profile** of this file; the skills read it.

The skills live in `.agents/skills/` (read by Codex, Gemini CLI, Copilot, Cursor, OpenCode and others) and are mirrored to `.claude/skills/` for Claude Code. They are vendored from [amigo-labs/skills](https://github.com/amigo-labs/skills): change them there and run `node .agents/sync.mjs`, never edit the copies here. Skills only this repository has live in `.agents/skills/` too and are mirrored the same way.

| Skill | Purpose |
| ----- | ------- |
| `spec <name>` | Write or revise a feature spec with acceptance criteria and a clarification pass |
| `spec-build <name>` | Implement a feature spec test-first, one commit series per AC block, independent verifier loop |
| `spec-check <name>` | Independent acceptance against a full gate run; only it sets `status: implemented` |
| `spec-extract` | Spec from existing code, or current-state analysis of an open spec |
| `spec-fix <bug>` | Bug fix: root cause, regression test that is red first, minimal fix |
| `qa` | Full gate run (`.agents/gates.json`, mirrors CI) |

An agent that cannot invoke skills by name reads `.agents/skills/<name>/SKILL.md` and follows it. At the start of a task, an agent without a session-start hook runs `node .agents/skills/qa/scripts/open-specs.mjs` to see the open feature specs.

Two kinds of spec live in `docs/specs/`:

| Kind | Status | Owns |
| ---- | ------ | ---- |
| Design spec (named in the profile) | `status: draft`, no acceptance criteria | Architecture, data model, the decisions |
| Feature spec `docs/specs/<name>.md` | `status: todo` / `implemented`, `## Acceptance criteria` | One feature: what it does, how we prove it |

A feature spec references the design-spec sections it builds on and never contradicts them. A feature that needs an architectural decision (cross-crate or cross-package boundary, file format, new dependency, public API shape) gets a decision where the profile says, in the same commit as the spec. The spec index (profile **Spec index**, default `docs/specs/README.md`) lists the feature specs: order, dependencies, done, and the backlog of gaps not yet written as specs.

### Triage

Classify every new input before touching code:

| Input | Approach |
| ----- | -------- |
| New feature or changed behavior ("I want …", "it should …") | Start `spec`, no code before the spec. If the user says "just do it", implement directly. |
| Request touches an existing feature spec | Revise that spec in `spec`'s revision mode, never create a second one |
| Unclear whether small change or feature | Exactly one question: "Capture as spec" / "Implement directly" |
| Bug fix | `spec-fix`: directly, always with a regression test that is red before the fix. If the fix changes specified behavior, revise the spec too. |
| Refactoring, tooling, docs, questions | Directly, no spec |
| "Carry on", "build X", "implement the spec" | `spec-build <name>` |
| "Is X done?", acceptance after manual implementation | `spec-check <name>` |
| Document existing behavior | `spec-extract <area>` |

Only `spec-check` sets a spec to `status: implemented` — never by hand.

### Keeping knowledge

Project knowledge belongs in the repository, not on one machine:

- **Pitfalls and non-obvious knowledge** → `## Gotchas` in this file. Only what cannot be read from code or git history; no noise.
- **Architecture decisions** → where the profile's **Decisions** entry says.
- **Feature-local decisions** → `## Decisions` of the feature spec, with the reason.
- **Progress of a feature** → `## Evidence` of the feature spec (there is no separate status file).
- **Never** into an agent's private memory (e.g. Claude Code auto memory): it stays on one machine.

### Quality gates

- **Run the `qa` skill before finishing any coding task** and report the result. The gate commands are in `.agents/gates.json`: `stop` (seconds: formatting, clippy on the changed crates — the Claude Code Stop hook runs it after every turn that changed code), `fast` (before every commit — the Claude Code commit gate refuses a commit while one is red; other agents run `node .agents/skills/qa/scripts/gates.mjs fast` before committing) and `full` (mirrors CI, `node .agents/skills/qa/scripts/gates.mjs full`).
- **Fix the cause, never lower the gates.** No suppressions (the list is `suppressions` in `.agents/gates.json`), no weakened, skipped or ignored tests. The Claude Code guard hook refuses new suppressions; a justified one is added by the user.
- **Don't loosen the gate configuration**: `.agents/gates.json`, `.agents/skills/qa/scripts/`, `.claude/settings.json` and `.claude/hooks/` change only on the user's explicit request.
- Where a gate names a `fix` command (formatting, lint autofix), that fix is allowed; everything else is fixed in the code. Where a gate has a `note`, follow it.
- `STOP_GATE=off` / `COMMIT_GATE=off` in the environment switch the Claude Code Stop hook / commit gate off, e.g. for a bulk migration — only on the user's request.

<!-- amigo-skills:end spec-workflow -->
