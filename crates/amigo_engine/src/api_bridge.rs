//! Executing JSON-RPC commands inside the engine loop.
//!
//! `amigo_api` queues every mutating request into `ApiSharedState::pending_commands`
//! and expects the engine to apply it on the main thread. The headless loop
//! handled four actions inline (`pause`, `unpause`, `tick`, `quit`) and dropped
//! the rest with a comment saying game code would deal with them — but nothing
//! ever handed them to game code. The windowed loop drained nothing at all, so
//! `camera.set`, `debug.step`, `editor.*` and everything else accumulated in the
//! queue forever. That queue is what `amigo connect`, `amigo_mcp` and Claude Code
//! talk to, so in a normal windowed session none of it did anything.
//!
//! This module is the single place both loops drain from. Commands the engine
//! genuinely owns (time control and the camera) are applied here; the rest land
//! in [`ApiInbox`] where game code can reach them via `ctx.resources`.

use crate::stack::GameStack;
use crate::GameContext;
use amigo_api::handler::{ApiCommand, DevSnapshot, SharedState};
use amigo_core::RenderVec2;
use amigo_render::CameraMode;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Where `amigo dev` expects the snapshot, per docs/specs/tooling/dev-workflow.md.
pub const DEV_SNAPSHOT_PATH: &str = ".amigo_dev/snapshot.ron";

/// Time-control state the API can change, owned by the engine loop.
#[derive(Debug)]
pub struct ApiControl {
    /// Simulation is paused; only explicitly requested ticks run.
    pub paused: bool,
    /// Ticks requested via `tick` / `debug.step`, consumed by the loop.
    pub pending_ticks: u64,
    /// Simulation speed multiplier from `set_speed`.
    pub speed: f32,
    /// A `quit` command arrived.
    pub quit: bool,
}

impl Default for ApiControl {
    fn default() -> Self {
        Self {
            paused: false,
            pending_ticks: 0,
            speed: 1.0,
            quit: false,
        }
    }
}

/// Commands the engine did not handle itself.
///
/// Inserted into `GameContext::resources`, so a game reads them with
/// `ctx.resources.get_mut::<ApiInbox>()`. Drained by the game; the engine only
/// appends and warns if nobody is consuming.
#[derive(Debug, Default)]
pub struct ApiInbox {
    pub commands: Vec<ApiCommand>,
}

impl ApiInbox {
    /// Take everything queued so far.
    pub fn drain(&mut self) -> Vec<ApiCommand> {
        std::mem::take(&mut self.commands)
    }
}

/// Cap on unconsumed inbox commands.
///
/// A game that never reads the inbox would otherwise grow it without bound for
/// as long as a client keeps sending. Dropping the oldest and saying so is
/// better than leaking.
const INBOX_LIMIT: usize = 1024;

/// Drain queued API commands, applying the ones the engine owns.
///
/// Camera commands need `ctx.camera`, so in windowed mode this must run while
/// the camera is on the `GameContext` (before it is swapped into the renderer).
pub fn drain_api_commands(
    shared: &SharedState,
    ctx: &mut GameContext,
    stack: &mut GameStack,
    control: &mut ApiControl,
) {
    let commands = {
        let mut state = amigo_api::lock_or_recover(shared);
        state.drain_commands()
    };
    if commands.is_empty() {
        return;
    }

    let mut unhandled = Vec::new();
    for cmd in commands {
        match cmd.action.as_str() {
            "pause" => control.paused = true,
            "unpause" => control.paused = false,
            "tick" => {
                control.pending_ticks += cmd
                    .params
                    .get("count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
            }
            "step" => {
                control.pending_ticks += cmd
                    .params
                    .get("ticks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
            }
            "set_speed" => match cmd.params.get("multiplier").and_then(|v| v.as_f64()) {
                Some(m) if m > 0.0 && m.is_finite() => control.speed = m as f32,
                Some(m) => warn!("Ignoring set_speed multiplier {m}: must be finite and positive"),
                None => warn!("set_speed without a multiplier"),
            },
            "quit" => control.quit = true,
            "camera.set" => {
                let x = cmd.params.get("x").and_then(|v| v.as_f64());
                let y = cmd.params.get("y").and_then(|v| v.as_f64());
                if let (Some(x), Some(y)) = (x, y) {
                    let to = RenderVec2 {
                        x: x as f32,
                        y: y as f32,
                    };
                    // Fixed mode, otherwise a follow/pan mode would drag the
                    // camera off the requested position on the next update.
                    ctx.camera.mode = CameraMode::Fixed;
                    ctx.camera.set_target(to);
                    ctx.camera.position = to;
                }
                if let Some(zoom) = cmd.params.get("zoom").and_then(|v| v.as_f64()) {
                    ctx.camera.set_zoom(zoom as f32);
                }
            }
            "camera.shake" => {
                // The camera decays shake itself (shake_decay), so only the
                // intensity carries over; `duration` has no separate knob.
                if let Some(i) = cmd.params.get("intensity").and_then(|v| v.as_f64()) {
                    ctx.camera.shake(i as f32);
                }
            }
            // `camera.get` needs no action: the loop publishes camera state into
            // the snapshot every frame, which `engine.status` returns.
            "camera.get" => {}
            "dev.save_snapshot" => {
                let path = cmd
                    .params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(DEV_SNAPSHOT_PATH));
                match save_dev_snapshot(shared, ctx, stack, control, &path) {
                    Ok(()) => info!("Dev snapshot written to {}", path.display()),
                    Err(e) => warn!("Could not write dev snapshot to {}: {e}", path.display()),
                }
            }
            "dev.restore_snapshot" => {
                match serde_json::from_value::<DevSnapshot>(cmd.params.clone()) {
                    Ok(snapshot) => apply_dev_snapshot(&snapshot, ctx, stack, control),
                    Err(e) => warn!("dev.restore_snapshot with an unreadable payload: {e}"),
                }
            }
            // `camera.follow` deliberately falls through to the inbox. Its
            // `entity_id` is a bare u64 and nothing in the repo defines how that
            // maps onto a generational `EntityId`, so the engine cannot resolve
            // it without inventing a convention. Game code knows its own
            // entities and can honour the command.
            _ => unhandled.push(cmd),
        }
    }

    if unhandled.is_empty() {
        return;
    }

    let inbox = ctx.resources.get_or_insert_with(ApiInbox::default);
    inbox.commands.extend(unhandled);
    if inbox.commands.len() > INBOX_LIMIT {
        let dropped = inbox.commands.len() - INBOX_LIMIT;
        inbox.commands.drain(..dropped);
        warn!(
            "ApiInbox over {INBOX_LIMIT} entries: dropped {dropped} oldest command(s). \
             Game code should drain ctx.resources.get_mut::<ApiInbox>() each tick."
        );
    } else {
        debug!(
            "{} API command(s) queued for game code",
            inbox.commands.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Dev snapshots
// ---------------------------------------------------------------------------

/// Seconds since the Unix epoch, as a string.
///
/// The snapshot's `timestamp` is informational (shown by `dev.snapshot_status`),
/// and there is no date/time crate in the workspace, so a raw epoch second is
/// what we can honestly produce.
fn timestamp_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

/// Capture engine + game state into a [`DevSnapshot`].
///
/// `Game::on_dev_snapshot` had no caller before this: the whole documented
/// "state survives a recompile" feature was a trait method nobody invoked.
pub fn build_dev_snapshot(
    ctx: &GameContext,
    stack: &GameStack,
    control: &ApiControl,
) -> DevSnapshot {
    let camera_pos = ctx.camera.effective_position();
    let (scene_id, game_state) = match stack.top() {
        Some(game) => (game.scene_id().to_string(), game.on_dev_snapshot(ctx)),
        None => (String::new(), serde_json::Value::Null),
    };
    DevSnapshot {
        scene_id,
        camera_pos: [camera_pos.x, camera_pos.y],
        camera_zoom: ctx.camera.zoom,
        tick: ctx.time.tick,
        paused: control.paused,
        speed_multiplier: control.speed,
        game_state,
        timestamp: timestamp_now(),
    }
}

/// Build a snapshot, publish it for `dev.snapshot_status`, and write it to disk.
fn save_dev_snapshot(
    shared: &SharedState,
    ctx: &GameContext,
    stack: &GameStack,
    control: &ApiControl,
    path: &Path,
) -> Result<(), String> {
    let snapshot = build_dev_snapshot(ctx, stack, control);

    {
        let mut s = amigo_api::lock_or_recover(shared);
        s.dev_snapshot = Some(snapshot.clone());
    }

    let serialized = ron::ser::to_string_pretty(&snapshot, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("serialize failed: {e}"))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {e}"))?;
        }
    }
    std::fs::write(path, serialized).map_err(|e| format!("write failed: {e}"))
}

/// Read a snapshot written by [`save_dev_snapshot`].
pub fn load_dev_snapshot(path: &Path) -> Result<DevSnapshot, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("read failed: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("parse failed: {e}"))
}

/// Apply a snapshot to the live engine and hand the game blob back to the game.
pub fn apply_dev_snapshot(
    snapshot: &DevSnapshot,
    ctx: &mut GameContext,
    stack: &mut GameStack,
    control: &mut ApiControl,
) {
    ctx.time.tick = snapshot.tick;
    let pos = RenderVec2 {
        x: snapshot.camera_pos[0],
        y: snapshot.camera_pos[1],
    };
    ctx.camera.position = pos;
    ctx.camera.set_target(pos);
    ctx.camera.set_zoom_immediate(snapshot.camera_zoom);
    control.paused = snapshot.paused;
    if snapshot.speed_multiplier > 0.0 && snapshot.speed_multiplier.is_finite() {
        control.speed = snapshot.speed_multiplier;
    }

    // The scene the snapshot came from cannot be reconstructed here: games are
    // pushed by opaque factory closures, so there is no registry to look a
    // scene_id up in. Restoring into a different scene is therefore possible,
    // and worth saying out loud rather than silently handing over state that
    // belongs to another scene.
    if let Some(game) = stack.top_mut() {
        let current = game.scene_id();
        if !snapshot.scene_id.is_empty() && snapshot.scene_id != current {
            warn!(
                "Restoring a snapshot taken in '{}' into '{current}'; \
                 the game decides what to do with the state blob",
                snapshot.scene_id
            );
        }
        game.on_dev_restore(ctx, &snapshot.game_state);
    }
    info!("Dev snapshot restored at tick {}", snapshot.tick);
}

/// Publish engine-owned state the API exposes read-only.
pub fn publish_snapshot(shared: &SharedState, ctx: &GameContext, control: &ApiControl) {
    let mut s = amigo_api::lock_or_recover(shared);
    s.snapshot.paused = control.paused;
    s.snapshot.speed_multiplier = control.speed;
    let pos = ctx.camera.effective_position();
    s.snapshot.custom.insert(
        "camera".to_string(),
        serde_json::json!({ "x": pos.x, "y": pos.y, "zoom": ctx.camera.zoom }),
    );
}
