# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

`amigo-engine` is a 2D pixel-art game engine in pure Rust: deterministic
fixed-point ECS, a wgpu renderer, an egui-based level editor, AI asset
pipelines (ComfyUI), and TidalCycles-based chiptune music. Cargo workspace,
edition 2021, MIT OR Apache-2.0.

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

CI and `just ci` run the same thing. `just` may not be installed; the
recipes in `justfile` are then run individually:

```sh
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace -- -D warnings     # warnings are errors
cargo test --workspace
```

Plus, before pushing, the feature-flag matrix and docs from `justfile`:
`clippy-features`, `test-features`, `test-doc`, and `doc` with
`RUSTDOCFLAGS="-D warnings"`. Feature combinations break in ways the default
build does not — `amigo_core --all-features` in particular.

## Layout

```
crates/     19 library crates, all prefixed amigo_
tools/      amigo_cli (the `amigo` binary), amigo_mcp, amigo_artgen,
            amigo_audiogen, amigo_comfyui
examples/   9 runnable demos, each a workspace member
docs/specs/ per-module specifications; docs/specs/index.md is the overview
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

Project skills for this workflow live in `.claude/skills/`: `spec` (write a
spec or ADR from the templates) and `develop` (spec → acceptance criteria →
implement → verify).

## Adding a crate

Add it to `[workspace.members]` and `[workspace.dependencies]` in the root
`Cargo.toml` (path dependency), use `version.workspace = true` and
`edition.workspace = true`, and put shared external deps in
`[workspace.dependencies]` rather than the crate's own manifest.
