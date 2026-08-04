# Changelog

Notable changes per release. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- `install.sh` offered `linux-aarch64` and `install.ps1` offered
  `aarch64-pc-windows-msvc`; neither is built, so both were guaranteed 404s. Both
  now explain that the platform builds from source, and their build-from-source
  hint points at the git URL rather than `--path tools/amigo_cli`, which a
  curl-installed user does not have.

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

## [0.0.3] — 2026-03-23

Released, but with no attached binaries — see the release-pipeline entry above.

## [0.0.2] — 2026-03-19

Same.
