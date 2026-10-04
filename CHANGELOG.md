# Changelog

Notable changes per release. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed — rendering

- **Pixel-perfect scaling exists.** `[render] scale_mode` was parsed and never
  read: the projection mapped the virtual resolution straight onto the window,
  so any window that was not an exact multiple stretched the image and gave
  virtual pixels uneven widths. Every stage now renders into an offscreen
  scene target that a final pass scales into the window. `pixel_perfect` (the
  default) uses the largest whole-number scale with letterbox bars
  (`Renderer::letterbox_color`), `fit` keeps the aspect ratio at any scale, and
  `stretch` keeps the old behaviour. An unknown value logs a warning.
- Mouse positions go through the same viewport, so `mouse_ui_pos` and
  `mouse_world_pos` stay on the right virtual pixel when the image is
  letterboxed. `mouse_world_pos` is also recomputed every frame instead of
  only when the cursor moves, so it follows a moving camera, and the camera's
  edge-pan and free-pan modes receive the cursor at all
  (`Camera::set_mouse_normalized` had no caller).
- `art_style` was never applied either. The pixel snap it controls was set on
  the renderer's own camera, which the game's camera replaces every frame; it
  is now set on the game's camera.
- API screenshots include lighting, post-processing and UI, and no longer fail
  validation on surfaces whose format is not `Rgba8UnormSrgb` (most desktop
  surfaces are BGRA).

### Changed — rendering

- With `art_style = "pixel_art"` (the default) or `"hybrid"`, the scene
  renders at the virtual resolution. Sprites at fractional positions snap to
  virtual pixels instead of moving in window-pixel steps, and bloom and the CRT
  filter work per virtual pixel. `"raster_art"` renders at the window size of
  the viewport, as before.

## [0.1.0] — 2026-10-04

The first release with attached binaries; v0.0.1–v0.0.3 were tagged but the
release workflow never published anything (see "Fixed — distribution").

### Security

- **Any web page could drive the JSON-RPC API.** The server binds 127.0.0.1,
  but a cross-origin `fetch` could smuggle a request line into an HTTP body,
  and `dev.save_snapshot` / `screenshot` wrote to any path (e.g. `~/.bashrc`).
  A non-JSON line now closes the connection, and request paths must stay in
  the project directory or the system temp dir, also through symlinks. The
  check covers every command that carries a path, including those passed on
  to the game (`save`, `load`, `replay.*`, `debug.dump_state`).
- **Code injection in the audio pipeline.** The MIDI path was spliced into
  `python -c` source; a stem file named `x'); __import__('os').system(...)`
  ran arbitrary Python. Paths now travel through `sys.argv`, stem names must
  be plain file names, and `amigo setup` passes its install dir through the
  environment rather than a shell string.
- **Path traversal in `amigo-audiogen`.** `generate_music` built output paths
  from unsanitized tool arguments, so a model's tool call could write outside
  the project.
- `cargo update` closes nine RustSec advisories in the lockfile (rustls,
  rustls-webpki, crossbeam-epoch, quick-xml, webbrowser). CI now runs
  `cargo deny check`.
- The kira 0.9 → 0.12 upgrade drops `ringbuf` 0.3 (RUSTSEC-2026-0293), the
  last open advisory; `deny.toml` no longer ignores any.

### Breaking

- `SystemGraph::run` (feature `system_graph`) executes batches sequentially;
  parallel dispatch is the new `unsafe fn run_parallel`, whose safety
  contract states what the safe API could not enforce (see ADR-0002).
- `WavePhase` gains an `Idle` variant, the spawner's initial state.
  `WaveSpawner::reset` returns to `Idle`.
- `social_deduction::cast_vote` takes `target: Option<EntityId>` (`None` =
  skip) instead of a raw `u32` choice.
- wgpu 24 → 30 and egui 0.31 → 0.36; `amigo_render` exposes their types,
  including `PostProcessPipeline` from the prelude (its methods take
  `wgpu::Device`/`TextureView`). Games that pass it objects from their own
  wgpu dependency must move that dependency to wgpu 30.
  `Renderer::render`/`begin_frame` return `amigo_render::SurfaceError`
  (wgpu dropped its own), and a `FrameInProgress` is presented with
  `renderer.queue.present(frame.output)`.
- `EguiRenderer::render`, `draw_editor_panels` and `draw_editor_v2_panels`
  take the root `&mut egui::Ui` instead of `&egui::Context`; egui 0.36
  lays panels out inside a `Ui`.
- The minimum supported Rust version is 1.95 (egui 0.36).

### Fixed — soundness and determinism

- `SystemGraph::run` let safe code race: inserting the first component of a
  new dynamic type rehashed the storage map under a sibling system.
- A component written through a stale `EntityId` blocked the reused slot for
  good; `World::add`/`insert_dynamic` now ignore dead ids, and despawn
  removals are visible through `removed()` until the next flush.
- `SimVec2::length` overflowed above ~32767 (a negative length in release)
  and returned 0 for tiny vectors; it is now exact on the raw bits.
- Rewind: recording after a rewind diffed against the discarded future, and
  deltas after an evicted keyframe could not be rebuilt.
- `TickScheduler` is `Clone` and serializable, so rollback and rewind
  snapshots can restore it.

### Fixed — gameplay

- Tower defense: the spawner panicked on the second tick when no wave had
  been started, waves auto-started during Build, and Victory was never
  reached.
- Localized strings mangled every non-ASCII character in interpolated text.
- `Inventory::add` stored stacks above `max_stack` and then underflowed.
- Dialog nodes skipped in a cycle overflowed the stack (an abort).
- Level files with short tile layers panicked in `tile_at`; dungeon configs
  with oversized or inverted room bounds divided by zero. Room placement also
  rejected rooms that fit with exactly their border and never used the last
  valid origin, so dungeon layouts for a given seed change.
- The deckbuilder lost the cards left in hand when a combat was won.
- The player at entity index 0 could never be voted out.
- Damage over time below 60 dps did nothing at 60 Hz; a player with a bomb
  never died from a missed deathbomb; ping-pong platforms teleported between
  their ends; turn combat ticked statuses for the wrong combatant.
- Circle-circle contact normals pointed the wrong way, so physics pushed
  overlapping circles and capsules into each other.

### Fixed — engine, rendering, audio

- A key press was seen by every tick of a multi-tick frame (Esc opened a menu
  that then closed itself).
- Pixel UI hit-tested clicks in window pixels while drawing in virtual
  pixels; widgets now use `InputState::mouse_ui_pos()`.
- Sprites on the same z drew in texture-load order instead of submission
  order (a fade overlay ended up under everything).
- The sprite and UI batchers were never cleared on a surface error or in the
  editor path, growing until the first visible frame panicked.
- The camera panicked when the level was smaller than the view; huge particle
  emission rates hung the game.
- A surface reporting Outdated waited for the next resize event to be
  reconfigured, and a minimized or occluded window logged a render error
  every frame. Outdated now reconfigures like Lost; skipped frames log at
  trace level.
- Stopped or replaced music kept playing (kira handles were dropped, not
  stopped). The tile-light flood fill grew exponentially near the map edge.
- `TaskPool` workers died with a panicking task; `autosave_slots = 0`
  panicked instead of disabling autosave.
- `GameContext::new` no longer opens the audio device: the engine opens it at
  startup (`AudioManager::open_device`), anything else on the first sound.
  Test binaries that built several contexts crashed on Windows with
  `STATUS_ACCESS_VIOLATION`, because cpal keeps its device enumerator in a
  static that lives in the COM apartment of the first test's thread.

### Changed — dependencies

- wgpu 30 and egui 0.36 (see Breaking). The renderer passes the window's
  display handle to the wgpu instance, which GLES needs to present on
  Wayland.
- kira 0.12 (audio; decibel volumes and sub-tracks behind the unchanged
  `amigo_audio` API, without kira's new libdbus default), ureq 3 (ComfyUI client:
  one pooled agent, downloads above 10 MB, no environment proxies for the
  local server), toml 1, ron 0.12, notify 8, pollster 1, tracing-tracy 0.12.

### Changed — tooling

- CI runs the `justfile` recipes, lints all targets, checks every feature via
  cargo-hack (the 67 `amigo_engine --features api` tests had never run in
  CI), builds docs and doc tests for every library crate, and tests on
  Windows and macOS. Dependabot, a PR template and issue forms were added.
- Every workspace member inherits version, edition, license and dependencies
  from the workspace; 33 unused dependency edges (including `glam`) were
  removed.
- CI builds a project from every `amigo new` template (`just templates`,
  warnings as errors). Generated games were only ever compiled by users, so a
  template that stopped building went unnoticed until someone scaffolded it.

### Fixed — distribution

- **Release binaries are produced at all.** `release.yml`'s publish job had
  `needs: build`, and the `aarch64-unknown-linux-gnu` matrix leg failed in `cargo
  build` — `gilrs`→`libudev-sys` and `kira`→`alsa-sys` resolve through pkg-config
  and need arm64 system libraries the job never installed. `fail-fast: false` kept
  the other legs running but did not stop `needs` from failing, so the release job
  was skipped. Every release run (v0.0.1, v0.0.2, v0.0.3) ended that way and every
  release has `"assets": []`, which is why `curl … install.sh | sh` 404s on every
  platform. The aarch64 Linux leg is dropped, and the publish step now asserts one
  archive per matrix entry so a partial release fails loudly instead of looking
  successful.
- **`install.sh` could not run the way it is documented.** The usage line, the
  README and the wiki all pipe it into `sh`, but the shebang was `bash` and line 4
  ran `set -euo pipefail` — a bashism dash rejects outright, so on Debian and Ubuntu
  (`/bin/sh` → dash) the script aborted with `Illegal option -o pipefail` before
  detecting anything. It is POSIX `sh` now; both pipelines were already guarded
  explicitly, so `pipefail` was buying nothing.
- Both installers' build-from-source fallback pointed at the default branch even
  though the script had just resolved a release tag, so `AMIGO_VERSION=v0.1.0`
  followed by a failed download suggested a command that installs `main`. The
  fallback shown after a download failure now pins `--tag`. The two earlier
  unsupported-platform exits keep the unpinned form deliberately: they run before
  the version is resolved, where `--tag ''` would be worse than no tag at all.
- `install.sh` offered `linux-aarch64` and `install.ps1` offered
  `aarch64-pc-windows-msvc`; neither is built, so both were guaranteed 404s. Both
  now explain that the platform builds from source, and their build-from-source
  hint points at the git URL rather than `--path tools/amigo_cli`, which a
  curl-installed user does not have.
- `release.yml` refuses a tag that does not match the workspace version. v0.0.3
  was tagged while `Cargo.toml` already said 0.1.0, so a successful run would
  have shipped binaries that write `engine_version = "0.1.0"` into projects
  under a 0.0.3 release.
- `amigo list-templates` printed names such as `sandbox-/-survival`. It now
  prints the slugs `amigo new --template` documents (`sandbox-survival`).

### Fixed — features that compiled and did nothing

- **`SceneAction::Push`/`Pop`/`Replace`.** Both engine loops matched only `Quit`, so
  returning a transition compiled and silently did nothing. They now drive a
  `GameStack`; `Game` gained `on_enter`/`on_pause`/`on_resume`/`on_exit`.
- **JSON-RPC commands in windowed mode.** The engine drained only screenshot
  requests, so roughly sixty queued methods — `camera.*`, `editor.*`,
  `debug.step`, `engine.set_property` — accumulated for the life of the process.
  That queue is what `amigo connect`, `amigo_mcp` and Claude Code drive. Both loops
  now drain through one path; the engine applies time and camera control and hands
  the rest to game code through an `ApiInbox` resource.
- **`engine.quit`.** `dev-workflow.md` has `amigo dev` send it; no such RPC method
  existed, so the only way to stop the engine was killing the process.
- **`AMIGO_API_PORT`.** Written by the CLI for `amigo dev --port`, never read by
  `EngineConfig`, so the engine always listened on 9999 and the dev loop's snapshot
  RPC went to a dead port with the error swallowed by `let _ =`.
- **`Game::on_dev_snapshot` / `on_dev_restore`.** No callers anywhere: the
  documented "state survives a recompile" loop was two trait methods nobody
  invoked. Now a full round trip through `.amigo_dev/snapshot.ron`, with
  `EngineBuilder::restore_snapshot` and `amigo run --restore-snapshot`.
- **`AssetManager`.** Private to the engine, so game code could not reach
  `load_ron` and had to fall back to `std::fs` with `CARGO_MANIFEST_DIR` paths that
  only resolve in a source checkout. Now on `GameContext`.
- **`AssetManager::load_from_pak`.** No callers: `amigo pack` wrote
  `assets/packed/game.pak` and the engine went on reading loose files, so a packed
  release still needed the whole `assets/` tree beside the binary.
- **`PostProcessPipeline`.** Complete — shader, uniforms, offscreen target,
  `apply()` — and never instantiated. Bloom, CRT, vignette and colour grading were
  unreachable. Now owned by the renderer and driven from `ctx.post_effects`.
- **`DebugOverlay::overlay_lines()`.** Called by nothing but its own unit test, so
  F1–F8 flipped flags nothing read and the overlay stayed invisible.
- **`UiContext` / `UiDrawCommand`.** The whole immediate-mode widget set produced
  draw commands that nothing consumed, so every widget drew nothing. Now bridged
  to sprites and rendered in a screen-space pass after post-processing.
- **Lighting.** `LightingState` packed ambient and point lights into bytes that
  nothing uploaded: there was no shader, no pipeline and no pass. Implemented as a
  fullscreen composite between the sprite pass and post-processing.

### Fixed — other

- `amigo new` wrote the same hardcoded `main.rs` for all 18 templates and left
  `src/scenes/` empty. Templates now scaffold a playable skeleton — title menu,
  gameplay, pause overlay — with the gameplay body chosen per preset family.
- `parse_preset` covered 14 of 21 `ScenePreset` variants and silently fell through
  to `Custom` for the rest, so `--preset deckbuilder` produced a blank scene.
- `LightData`'s field order put `position: vec2` before `color: vec4`; WGSL aligns
  `vec4` to 16 bytes, so the GPU would have read every light shifted by 8 bytes.
- The fixed-timestep accumulator carried its backlog after hitting the frame cap,
  so one stall could keep re-triggering it, and paused time was banked, so
  unpausing burned through the pause. Extracted as `timestep::tick_budget`.
- `EngineConfig::load` silently fell back to defaults on a malformed `amigo.toml`,
  making a broken config indistinguishable from no config.

### Removed

- `SaveConfig::compression`. The flag was never read, there was no `lz4`
  dependency, and saves were always written uncompressed — so anyone setting it
  believed a promise nothing kept. CRC32 corruption checks, slot management and
  autosave rotation are unaffected and real.

### Added

- `scripts/install-system-deps.sh` plus a `SessionStart` hook: without the Linux
  audio/input/windowing libraries, `cargo check` dies inside `libudev-sys`'s build
  script, which reads as a Rust error but is a missing `libudev.pc`.
- `rust-toolchain.toml`, `CLAUDE.md`, this changelog.
- `amigo version` (also `--version`, `-V`): the CLI version and the engine
  revision that `amigo new` pins projects to.
- ADRs 0009–0013, which code already referenced while the files did not exist.
- Shader validation with `naga` in tests, covering the new lighting shaders and the
  pre-existing post-process and `gpu_broad_phase` shaders, which nothing checked.

### Documentation

Corrections where docs described something the code does not do: a
`Getting-Started` snippet using a `Vec2` that does not exist and a resolution that
was never the default; a `status: done` used for "the types exist" (now `partial`
where nothing runs them); crates named in `index.md` that do not exist
(`amigo_camera`, `amigo_plugin`) and dependencies that are not used (`laminar`,
`serde_ron`); a "No egui" decision recorded while the editor is an egui
application; duplicate `fog-of-war` and `spline` specs, one of each with unparseable
or missing frontmatter; `EngineError` specified but absent; and MCP tool names in
`dev-workflow.md` that never matched the code.

The README now says what state the engine is in (pre-1.0; which subsystems run
end to end and which are libraries nothing calls yet) instead of listing a
"built-in level editor" and a "deterministic ECS" without qualification, and
`Getting-Started` no longer promises `.aseprite` loading, which only PNG has.

## [0.0.3] — 2026-03-23

Released, but with no attached binaries — see the release-pipeline entry above.

## [0.0.2] — 2026-03-19

Same.
