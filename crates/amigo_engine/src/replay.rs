//! Input replays: record what a session's input was, tick by tick, and feed
//! it back later to reproduce the session.
//!
//! The simulation is deterministic (ADR-0001), so the input alone is enough:
//! a replay stores the RNG state at the start, every tick's keyboard and
//! mouse state and bound actions (only when they change), and a hash of the
//! state after every tick. Playback feeds the stored input in place of the
//! live one and compares the hashes; the first tick that differs is reported
//! as a desync.
//!
//! Start one with `amigo run --record <file>` / `--replay <file>`
//! (`AMIGO_RECORD` / `AMIGO_REPLAY`), [`EngineBuilder::record_replay`] /
//! [`EngineBuilder::play_replay`], or the `replay.*` API commands.
//! `docs/specs/engine/replays.md` describes the format and its limits.
//!
//! [`EngineBuilder::record_replay`]: crate::EngineBuilder::record_replay
//! [`EngineBuilder::play_replay`]: crate::EngineBuilder::play_replay

use crate::stack::GameStack;
use crate::{Game, GameContext};
use amigo_core::SimRng;
use amigo_input::{ActionSnapshot, GamepadState, InputSnapshot, InputState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// The replay file format version this engine writes and reads.
pub const REPLAY_VERSION: u32 = 1;

/// A recorded session, as stored on disk (JSON).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Replay {
    /// [`REPLAY_VERSION`] at the time of recording.
    pub version: u32,
    /// `Game::scene_id` of the game that was active when recording started.
    pub scene: String,
    /// Whether recording started before the root game's `init`, so playback
    /// can start from a fresh game. Otherwise it started mid-session and
    /// playback restores [`start_state`](Self::start_state) first.
    pub from_start: bool,
    /// `ctx.time.tick` when recording started.
    pub start_tick: u64,
    /// Number of ticks recorded.
    pub ticks: u64,
    /// The seed the session was started with, for reference.
    pub seed: u64,
    /// `ctx.rng` when recording started; playback restores it.
    pub rng_state: u64,
    /// `Game::on_dev_snapshot` when recording started mid-session.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub start_state: serde_json::Value,
    /// The input of every tick on which it differed from the tick before,
    /// in tick order. The first entry is the first recorded tick.
    pub inputs: Vec<ReplayInput>,
    /// The state hash after each recorded tick, `ticks` entries.
    pub hashes: Vec<u64>,
    /// Free-form notes (game name, version, player).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

/// The input of one tick in a [`Replay`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplayInput {
    pub tick: u64,
    pub input: InputSnapshot,
    #[serde(default)]
    pub actions: ActionSnapshot,
}

impl Replay {
    /// Write the replay as JSON, creating the parent directory.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let json = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Read a replay written by [`save`](Self::save).
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let replay: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if replay.version != REPLAY_VERSION {
            return Err(format!(
                "{}: replay format version {} (this engine reads {REPLAY_VERSION})",
                path.display(),
                replay.version
            ));
        }
        if replay.hashes.len() as u64 != replay.ticks {
            return Err(format!(
                "{}: {} hashes for {} ticks",
                path.display(),
                replay.hashes.len(),
                replay.ticks
            ));
        }
        Ok(replay)
    }
}

/// What the replay machinery is doing, from [`GameContext::replay_status`]
/// or `engine.get_property {"key": "replay"}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ReplayStatus {
    pub mode: ReplayMode,
    /// The file being played, or the file a launch recording goes to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// First recorded tick.
    pub start_tick: u64,
    /// Ticks recorded or played so far.
    pub ticks: u64,
    /// Ticks in the replay being played.
    pub total_ticks: u64,
    /// The first tick whose state differed from the recording, if any.
    pub desync_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    /// Nothing recorded or played this session.
    #[default]
    Off,
    Recording,
    Playing,
    /// A playback ran to its end (or was stopped); see `desync_tick`.
    Finished,
}

/// The engine's replay state, kept on the [`GameContext`].
#[derive(Default)]
pub(crate) struct ReplayDriver {
    mode: Mode,
    /// Status of the last finished playback.
    last: Option<ReplayStatus>,
}

#[derive(Default)]
enum Mode {
    #[default]
    Off,
    Recording(Box<Recording>),
    Playing(Box<Playing>),
}

struct Recording {
    replay: Replay,
    last_input: Option<(InputSnapshot, ActionSnapshot)>,
    /// Where to write on shutdown, for a recording started at launch.
    save_on_exit: Option<PathBuf>,
}

struct Playing {
    replay: Replay,
    path: String,
    /// Next entry of `replay.inputs` to apply.
    cursor: usize,
    current: (InputSnapshot, ActionSnapshot),
    played: u64,
    desync_tick: Option<u64>,
    /// The live gamepads, set aside: their raw state cannot be replayed, so
    /// the game sees none while the replay drives it.
    live_gamepad: GamepadState,
}

impl ReplayDriver {
    pub(crate) fn status(&self) -> ReplayStatus {
        match &self.mode {
            Mode::Off => self.last.clone().unwrap_or_default(),
            Mode::Recording(r) => ReplayStatus {
                mode: ReplayMode::Recording,
                path: r.save_on_exit.as_ref().map(|p| p.display().to_string()),
                start_tick: r.replay.start_tick,
                ticks: r.replay.ticks,
                total_ticks: 0,
                desync_tick: None,
            },
            Mode::Playing(p) => ReplayStatus {
                mode: ReplayMode::Playing,
                path: Some(p.path.clone()),
                start_tick: p.replay.start_tick,
                ticks: p.played,
                total_ticks: p.replay.ticks,
                desync_tick: p.desync_tick,
            },
        }
    }
}

/// Start recording. `from_start` means the root game has not been
/// initialized yet (a launch recording); otherwise the game's
/// `on_dev_snapshot` is stored so playback can restore it.
pub(crate) fn start_recording(
    ctx: &mut GameContext,
    stack: &GameStack,
    from_start: bool,
    save_on_exit: Option<PathBuf>,
) -> Result<(), String> {
    match ctx.replay.mode {
        Mode::Off => {}
        Mode::Recording(_) => return Err("already recording".into()),
        Mode::Playing(_) => return Err("a replay is playing".into()),
    }
    let (scene, start_state) = match stack.top() {
        Some(game) if !from_start => (game.scene_id().to_string(), game.on_dev_snapshot(ctx)),
        Some(game) => (game.scene_id().to_string(), serde_json::Value::Null),
        None => (String::new(), serde_json::Value::Null),
    };
    if !from_start && start_state.is_null() {
        warn!(
            "Recording mid-session, but {scene} returns nothing from on_dev_snapshot: \
             the replay can only reproduce this session if its state is rebuilt \
             exactly. Record from launch (`amigo run --record`) to be safe."
        );
    }
    ctx.replay.mode = Mode::Recording(Box::new(Recording {
        replay: Replay {
            version: REPLAY_VERSION,
            scene,
            from_start,
            start_tick: ctx.time.tick,
            ticks: 0,
            seed: ctx.seed(),
            rng_state: ctx.rng.state(),
            start_state,
            inputs: Vec::new(),
            hashes: Vec::new(),
            metadata: BTreeMap::from([(
                "engine_version".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            )]),
        },
        last_input: None,
        save_on_exit,
    }));
    ctx.replay.last = None;
    info!("Replay: recording from tick {}", ctx.time.tick);
    Ok(())
}

/// Stop recording and hand back what was recorded.
pub(crate) fn stop_recording(ctx: &mut GameContext) -> Result<Replay, String> {
    match std::mem::take(&mut ctx.replay.mode) {
        Mode::Recording(r) => {
            info!("Replay: recorded {} ticks", r.replay.ticks);
            Ok(r.replay)
        }
        other => {
            ctx.replay.mode = other;
            Err("not recording".into())
        }
    }
}

/// Write a launch recording to its file, if one is running.
pub(crate) fn save_on_exit(ctx: &mut GameContext) {
    let Mode::Recording(r) = &ctx.replay.mode else {
        return;
    };
    let Some(path) = r.save_on_exit.clone() else {
        return;
    };
    match stop_recording(ctx).and_then(|replay| replay.save(&path)) {
        Ok(()) => info!("Replay written to {}", path.display()),
        Err(e) => warn!("Could not write the replay to {}: {e}", path.display()),
    }
}

/// Start playing `replay`. `before_init` means the root game has not been
/// initialized yet (a launch replay).
pub(crate) fn play(
    ctx: &mut GameContext,
    stack: &mut GameStack,
    replay: Replay,
    path: String,
    before_init: bool,
) -> Result<(), String> {
    match ctx.replay.mode {
        Mode::Off => {}
        Mode::Recording(_) => return Err("recording; stop it first".into()),
        Mode::Playing(_) => return Err("a replay is already playing".into()),
    }
    if replay.ticks == 0 {
        return Err(format!("{path} has no ticks"));
    }
    if replay.from_start && !before_init {
        warn!(
            "{path} was recorded from launch; playing it into a running game only \
             matches if the game is back in its starting state. Use \
             `amigo run --replay {path}` for an exact run."
        );
    }
    ctx.time.tick = replay.start_tick;
    ctx.rng = SimRng::new(replay.rng_state);
    if !replay.from_start {
        match stack.top_mut() {
            Some(game) if !replay.start_state.is_null() => {
                game.on_dev_restore(ctx, &replay.start_state)
            }
            _ => warn!(
                "{path} starts mid-session without a game snapshot; playing it from \
                 the current state"
            ),
        }
    }
    let live_gamepad = std::mem::replace(&mut ctx.gamepad, GamepadState::disabled());
    info!(
        "Replay: playing {path}, {} ticks from tick {}",
        replay.ticks, replay.start_tick
    );
    ctx.replay.last = None;
    ctx.replay.mode = Mode::Playing(Box::new(Playing {
        replay,
        path,
        cursor: 0,
        current: Default::default(),
        played: 0,
        desync_tick: None,
        live_gamepad,
    }));
    Ok(())
}

/// End a playback early (or at its end), handing input back to the player.
pub(crate) fn stop_playback(ctx: &mut GameContext) -> Result<(), String> {
    let p = match std::mem::take(&mut ctx.replay.mode) {
        Mode::Playing(p) => p,
        other => {
            ctx.replay.mode = other;
            return Err("no replay is playing".into());
        }
    };
    let status = ReplayStatus {
        mode: ReplayMode::Finished,
        path: Some(p.path.clone()),
        start_tick: p.replay.start_tick,
        ticks: p.played,
        total_ticks: p.replay.ticks,
        desync_tick: p.desync_tick,
    };
    match status.desync_tick {
        Some(tick) => warn!(
            "Replay {}: desync at tick {tick}, {} of {} ticks played",
            p.path, p.played, p.replay.ticks
        ),
        None => info!(
            "Replay {}: {} of {} ticks played, no desync",
            p.path, p.played, p.replay.ticks
        ),
    }
    ctx.gamepad = p.live_gamepad;
    // Whatever the replay held is not held by the player.
    ctx.input = InputState::new();
    ctx.actions.restore(&ActionSnapshot::default());
    ctx.replay.last = Some(status);
    Ok(())
}

/// Before `Game::update`: refresh the actions from live input, or feed the
/// replay's input in its place; record it when recording.
pub(crate) fn before_update(ctx: &mut GameContext) {
    let tick = ctx.time.tick;
    if let Mode::Playing(p) = &mut ctx.replay.mode {
        let inputs = &p.replay.inputs;
        while let Some(entry) = inputs.get(p.cursor).filter(|e| e.tick <= tick) {
            p.current = (entry.input.clone(), entry.actions.clone());
            p.cursor += 1;
        }
        ctx.input.restore(&p.current.0);
        ctx.actions.restore(&p.current.1);
        return;
    }
    ctx.update_actions();
    if let Mode::Recording(r) = &mut ctx.replay.mode {
        let now = (ctx.input.snapshot(), ctx.actions.snapshot());
        if r.last_input.as_ref() != Some(&now) {
            r.replay.inputs.push(ReplayInput {
                tick,
                input: now.0.clone(),
                actions: now.1.clone(),
            });
            r.last_input = Some(now);
        }
    }
}

/// After a whole tick: record the state hash, or compare it with the
/// recording and end the playback after its last tick.
pub(crate) fn after_tick(ctx: &mut GameContext, stack: &GameStack) {
    if matches!(ctx.replay.mode, Mode::Off) {
        return;
    }
    let hash = state_hash(ctx, stack.top());
    let finished = match &mut ctx.replay.mode {
        Mode::Off => false,
        Mode::Recording(r) => {
            r.replay.hashes.push(hash);
            r.replay.ticks += 1;
            false
        }
        Mode::Playing(p) => {
            let index = p.played as usize;
            if p.desync_tick.is_none() && p.replay.hashes.get(index) != Some(&hash) {
                let tick = p.replay.start_tick + p.played;
                warn!(
                    "Replay {}: state differs from the recording after tick {tick}",
                    p.path
                );
                p.desync_tick = Some(tick);
            }
            p.played += 1;
            p.played >= p.replay.ticks
        }
    };
    if finished {
        let _ = stop_playback(ctx);
    }
}

/// The hash recorded after each tick: the tick number, the RNG state, the
/// entity count, and the game's own [`Game::state_hash`] when it has one.
fn state_hash(ctx: &GameContext, game: Option<&dyn Game>) -> u64 {
    let game_hash = game.and_then(|g| g.state_hash(ctx));
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let words = [
        ctx.time.tick,
        ctx.rng.state(),
        ctx.world.entity_count() as u64,
        u64::from(game_hash.is_some()),
        game_hash.unwrap_or(0),
    ];
    for byte in words.iter().flat_map(|w| w.to_le_bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Replays requested at launch, by `AMIGO_RECORD` / `AMIGO_REPLAY` or the
/// builder.
#[derive(Clone, Debug, Default)]
pub(crate) struct LaunchReplay {
    pub(crate) record: Option<PathBuf>,
    pub(crate) play: Option<PathBuf>,
}

impl LaunchReplay {
    pub(crate) fn from_env() -> Self {
        let path = |key| {
            std::env::var_os(key)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            record: path("AMIGO_RECORD"),
            play: path("AMIGO_REPLAY"),
        }
    }
}

/// Run the root game's `init`, wrapped in whatever replay was requested at
/// launch: a launch recording or a from-start replay begins before `init`
/// (which may draw random numbers), a mid-session replay right after it.
///
/// `snapshot_follows` says a dev snapshot will be restored right after this;
/// a launch recording then starts once that is done, through
/// [`after_restore`], since a recording from launch would not include it.
pub(crate) fn enter_root(
    ctx: &mut GameContext,
    stack: &mut GameStack,
    launch: &LaunchReplay,
    snapshot_follows: bool,
) {
    let mut after_init = None;
    if let Some(path) = &launch.play {
        if launch.record.is_some() {
            warn!("Both a replay to play and one to record were given; only playing");
        }
        let label = path.display().to_string();
        match Replay::load(path) {
            Ok(replay) if replay.from_start => {
                if let Err(e) = play(ctx, stack, replay, label, true) {
                    warn!("Replay {}: {e}", path.display());
                }
            }
            Ok(replay) => after_init = Some((replay, label)),
            Err(e) => warn!("Could not load the replay: {e}"),
        }
    } else if let Some(path) = &launch.record
        && !snapshot_follows
        && let Err(e) = start_recording(ctx, stack, true, Some(path.clone()))
    {
        warn!("Could not start recording: {e}");
    }

    stack.enter_root(ctx);

    if let Some((replay, label)) = after_init
        && let Err(e) = play(ctx, stack, replay, label, false)
    {
        warn!("Replay: {e}");
    }
}

/// After a launch dev snapshot was restored: start the launch recording
/// [`enter_root`] held back.
#[cfg(feature = "api")]
pub(crate) fn after_restore(ctx: &mut GameContext, stack: &GameStack, launch: &LaunchReplay) {
    if launch.play.is_none()
        && let Some(path) = &launch.record
        && let Err(e) = start_recording(ctx, stack, false, Some(path.clone()))
    {
        warn!("Could not start recording: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SceneAction;
    use crate::context::DrawContext;
    use winit::event::{ElementState, MouseButton};
    use winit::keyboard::KeyCode;
    use winit::keyboard::PhysicalKey;

    /// Moves on held D, rolls the RNG on the "jump" action and adds the
    /// cursor's x on a click, so keys, actions, the mouse and `ctx.rng` all
    /// steer it.
    #[derive(Default)]
    struct Walker {
        x: i64,
        jumps: u32,
        init_roll: u64,
    }

    impl Game for Walker {
        fn init(&mut self, ctx: &mut GameContext) {
            self.init_roll = ctx.rng.next_u64();
        }

        fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
            if ctx.input.held(KeyCode::KeyD) {
                self.x += 1;
            }
            if ctx.actions.pressed("jump") {
                self.jumps += 1;
                self.x += i64::from(ctx.rng.range(0, 10));
            }
            if ctx.input.mouse_pressed(MouseButton::Left) {
                self.x += ctx.input.mouse_world_pos().x as i64;
            }
            SceneAction::Continue
        }

        fn draw(&self, _ctx: &mut DrawContext) {}

        fn state_hash(&self, _ctx: &GameContext) -> Option<u64> {
            Some((self.x as u64) ^ (u64::from(self.jumps) << 40) ^ self.init_roll.rotate_left(17))
        }

        fn on_dev_snapshot(&self, _ctx: &GameContext) -> serde_json::Value {
            serde_json::json!([self.x, self.jumps, self.init_roll])
        }

        fn on_dev_restore(&mut self, _ctx: &mut GameContext, state: &serde_json::Value) {
            let field = |i: usize| state[i].as_u64().unwrap();
            self.x = field(0) as i64;
            self.jumps = field(1) as u32;
            self.init_roll = field(2);
        }
    }

    fn context(seed: u64) -> GameContext {
        let mut ctx = GameContext::new(320.0, 180.0, "assets");
        ctx.bindings.bind_key("jump", "Space");
        ctx.reseed(seed);
        ctx
    }

    fn key(ctx: &mut GameContext, code: KeyCode, down: bool) {
        let state = if down {
            ElementState::Pressed
        } else {
            ElementState::Released
        };
        ctx.input.handle_key_event(PhysicalKey::Code(code), state);
    }

    /// The player for the recordings: walks, jumps every 37 ticks, clicks
    /// every 50.
    fn script(ctx: &mut GameContext, step: u64) {
        match step % 120 {
            0 => key(ctx, KeyCode::KeyD, true),
            80 => key(ctx, KeyCode::KeyD, false),
            _ => {}
        }
        if step % 37 == 5 {
            key(ctx, KeyCode::Space, true);
        }
        if step % 37 == 7 {
            key(ctx, KeyCode::Space, false);
        }
        if step % 50 == 20 {
            ctx.input
                .set_mouse_world_pos(amigo_core::RenderVec2::new(step as f32 * 0.5, 3.0));
            ctx.input
                .handle_mouse_button(MouseButton::Left, ElementState::Pressed);
        }
        if step % 50 == 21 {
            ctx.input
                .handle_mouse_button(MouseButton::Left, ElementState::Released);
        }
    }

    /// A different player, whose input a playback must ignore.
    fn noise(ctx: &mut GameContext, step: u64) {
        key(ctx, KeyCode::Space, step.is_multiple_of(3));
        key(ctx, KeyCode::KeyD, step.is_multiple_of(2));
    }

    fn run(
        ctx: &mut GameContext,
        stack: &mut GameStack,
        ticks: u64,
        input: fn(&mut GameContext, u64),
    ) {
        for step in 0..ticks {
            input(ctx, step);
            assert!(crate::tick::run_tick(ctx, stack, &mut []));
        }
    }

    fn hash(ctx: &GameContext, stack: &GameStack) -> Option<u64> {
        stack.top().and_then(|g| g.state_hash(ctx))
    }

    /// Record 600 ticks from launch with seed 42.
    fn record_from_launch() -> (Replay, Option<u64>) {
        let mut ctx = context(42);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        start_recording(&mut ctx, &stack, true, None).unwrap();
        stack.enter_root(&mut ctx);
        run(&mut ctx, &mut stack, 600, script);
        let end = hash(&ctx, &stack);
        (stop_recording(&mut ctx).unwrap(), end)
    }

    /// Play `replay` from launch in a fresh engine with another seed and
    /// another player at the keyboard.
    fn play_from_launch(replay: Replay) -> (GameContext, GameStack) {
        let mut ctx = context(7);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        play(&mut ctx, &mut stack, replay, "test".into(), true).unwrap();
        stack.enter_root(&mut ctx);
        run(&mut ctx, &mut stack, 600, noise);
        (ctx, stack)
    }

    #[test]
    fn a_launch_recording_replays_exactly() {
        let (replay, end) = record_from_launch();
        assert_eq!(replay.ticks, 600);
        assert_eq!(replay.hashes.len(), 600);
        assert!(replay.from_start);
        assert_eq!(replay.seed, 42);

        // Through a file, like a real one.
        let path = std::env::temp_dir().join(format!("amigo_replay_{}.json", std::process::id()));
        replay.save(&path).unwrap();
        let loaded = Replay::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded, replay);

        let (ctx, stack) = play_from_launch(loaded);
        assert_eq!(hash(&ctx, &stack), end);
        let status = ctx.replay_status();
        assert_eq!(status.mode, ReplayMode::Finished);
        assert_eq!((status.ticks, status.total_ticks), (600, 600));
        assert_eq!(status.desync_tick, None);
    }

    #[test]
    fn input_is_stored_only_when_it_changes() {
        let (replay, _) = record_from_launch();
        // 600 ticks, but only the ticks around presses, releases and clicks.
        assert!(replay.inputs.len() < 120, "{} entries", replay.inputs.len());
        assert_eq!(replay.inputs[0].tick, 0);
        assert!(replay.inputs.windows(2).all(|w| w[0].tick < w[1].tick));
    }

    #[test]
    fn a_changed_input_is_reported_as_a_desync() {
        let (mut replay, _) = record_from_launch();
        // Drop the second jump press (tick 42): the walker lands elsewhere.
        let entry = replay
            .inputs
            .iter_mut()
            .find(|e| e.tick == 42)
            .expect("the jump press at tick 42 was recorded");
        entry.input.pressed.clear();
        entry.actions.pressed.clear();

        let (ctx, _) = play_from_launch(replay);
        assert_eq!(ctx.replay_status().desync_tick, Some(42));
    }

    #[test]
    fn a_wrong_seed_is_a_desync_from_the_first_tick() {
        let (mut replay, _) = record_from_launch();
        replay.rng_state ^= 1;
        let (ctx, _) = play_from_launch(replay);
        assert_eq!(ctx.replay_status().desync_tick, Some(0));
    }

    #[test]
    fn a_mid_session_recording_restores_its_starting_state() {
        let mut ctx = context(1);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        stack.enter_root(&mut ctx);
        run(&mut ctx, &mut stack, 100, script);
        start_recording(&mut ctx, &stack, false, None).unwrap();
        run(&mut ctx, &mut stack, 300, script);
        let end = hash(&ctx, &stack);
        let replay = stop_recording(&mut ctx).unwrap();
        assert!(!replay.from_start);
        assert_eq!(replay.start_tick, 100);
        assert!(!replay.start_state.is_null());

        // Another session, somewhere else entirely.
        let mut ctx = context(2);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        stack.enter_root(&mut ctx);
        run(&mut ctx, &mut stack, 33, noise);
        play(&mut ctx, &mut stack, replay, "mid".into(), false).unwrap();
        assert_eq!(ctx.time.tick, 100);
        run(&mut ctx, &mut stack, 300, noise);
        assert_eq!(hash(&ctx, &stack), end);
        assert_eq!(ctx.replay_status().desync_tick, None);
        assert_eq!(ctx.replay_status().mode, ReplayMode::Finished);
    }

    #[test]
    fn playback_ends_by_handing_input_back() {
        let (replay, _) = record_from_launch();
        let (mut ctx, mut stack) = play_from_launch(replay);
        // The replay ended holding nothing the player holds.
        assert!(!ctx.input.held(KeyCode::KeyD));
        // Live input drives the game again.
        let before = hash(&ctx, &stack);
        run(&mut ctx, &mut stack, 1, |ctx, _| {
            key(ctx, KeyCode::KeyD, true)
        });
        assert_ne!(hash(&ctx, &stack), before);
    }

    #[test]
    fn stopping_early_keeps_the_result() {
        let (replay, _) = record_from_launch();
        let mut ctx = context(7);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        play(&mut ctx, &mut stack, replay, "early".into(), true).unwrap();
        stack.enter_root(&mut ctx);
        run(&mut ctx, &mut stack, 10, noise);
        stop_playback(&mut ctx).unwrap();
        let status = ctx.replay_status();
        assert_eq!(status.mode, ReplayMode::Finished);
        assert_eq!((status.ticks, status.total_ticks), (10, 600));
        assert!(stop_playback(&mut ctx).is_err());
    }

    #[test]
    fn one_thing_at_a_time() {
        let mut ctx = context(0);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        stack.enter_root(&mut ctx);
        assert!(stop_recording(&mut ctx).is_err());
        start_recording(&mut ctx, &stack, false, None).unwrap();
        assert!(start_recording(&mut ctx, &stack, false, None).is_err());
        run(&mut ctx, &mut stack, 3, script);
        let replay = stop_recording(&mut ctx).unwrap();
        play(&mut ctx, &mut stack, replay.clone(), "a".into(), false).unwrap();
        assert!(start_recording(&mut ctx, &stack, false, None).is_err());
        assert!(play(&mut ctx, &mut stack, replay, "b".into(), false).is_err());
    }

    #[test]
    fn empty_and_foreign_replays_are_refused() {
        let mut ctx = context(0);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        start_recording(&mut ctx, &stack, true, None).unwrap();
        let empty = stop_recording(&mut ctx).unwrap();
        assert!(play(&mut ctx, &mut stack, empty.clone(), "empty".into(), true).is_err());

        let path =
            std::env::temp_dir().join(format!("amigo_replay_v9_{}.json", std::process::id()));
        Replay {
            version: 9,
            ..empty
        }
        .save(&path)
        .unwrap();
        let err = Replay::load(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.contains("version 9"), "{err}");
    }

    #[test]
    fn every_tick_sees_the_fixed_timestep() {
        struct DtCheck(u32);
        impl Game for DtCheck {
            fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
                assert_eq!(ctx.time.dt, amigo_core::TimeInfo::TICK_DURATION as f32);
                self.0 += 1;
                SceneAction::Continue
            }
            fn draw(&self, _ctx: &mut DrawContext) {}
            fn on_dev_snapshot(&self, _ctx: &GameContext) -> serde_json::Value {
                self.0.into()
            }
        }
        let mut ctx = context(0);
        let mut stack = GameStack::new(Box::new(DtCheck(0)));
        stack.enter_root(&mut ctx);
        // A slow frame running three ticks.
        ctx.time.frame_dt = 0.05;
        run(&mut ctx, &mut stack, 3, |_, _| {});
        assert_eq!(stack.top().unwrap().on_dev_snapshot(&ctx), 3);
        assert_eq!(ctx.time.frame_dt, 0.05);
        assert_eq!(ctx.replay_status().mode, ReplayMode::Off);
    }

    #[test]
    fn a_launch_recording_is_written_on_exit() {
        let path =
            std::env::temp_dir().join(format!("amigo_replay_exit_{}/run.json", std::process::id()));
        let launch = LaunchReplay {
            record: Some(path.clone()),
            play: None,
        };
        let mut ctx = context(3);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        enter_root(&mut ctx, &mut stack, &launch, false);
        assert_eq!(ctx.replay_status().mode, ReplayMode::Recording);
        run(&mut ctx, &mut stack, 30, script);
        save_on_exit(&mut ctx);
        let replay = Replay::load(&path).unwrap();
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert_eq!(replay.ticks, 30);
        assert!(replay.from_start);
        assert_eq!(ctx.replay_status().mode, ReplayMode::Off);

        // And `enter_root` plays it back from launch.
        let replay_path =
            std::env::temp_dir().join(format!("amigo_replay_launch_{}.json", std::process::id()));
        replay.save(&replay_path).unwrap();
        let launch = LaunchReplay {
            record: None,
            play: Some(replay_path.clone()),
        };
        let mut ctx = context(99);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        enter_root(&mut ctx, &mut stack, &launch, false);
        let _ = std::fs::remove_file(&replay_path);
        run(&mut ctx, &mut stack, 30, noise);
        let status = ctx.replay_status();
        assert_eq!(
            (status.mode, status.desync_tick),
            (ReplayMode::Finished, None)
        );
    }

    #[cfg(feature = "api")]
    #[test]
    fn the_api_records_and_plays_replays() {
        use crate::api_bridge::{ApiControl, drain_api_commands, publish_snapshot};
        use amigo_api::handler::{handle_request, new_shared_state};

        let shared = new_shared_state();
        let mut control = ApiControl::default();
        let send = |method: &str, params: serde_json::Value| {
            let response = handle_request(
                &amigo_api::RpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(1),
                    method: method.to_string(),
                    params,
                },
                &shared,
            );
            assert!(response.error.is_none(), "{method}: {:?}", response.error);
        };
        let property = |ctx: &GameContext, control: &ApiControl| {
            publish_snapshot(&shared, ctx, control);
            amigo_api::lock_or_recover(&shared).snapshot.custom["replay"].clone()
        };
        // Relative paths only, like every path the API accepts; removed
        // even when an assertion fails.
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = Cleanup(format!("amigo_replay_api_test_{}", std::process::id()));
        let path = format!("{}/run.json", dir.0);

        let mut ctx = context(5);
        let mut stack = GameStack::new(Box::new(Walker::default()));
        stack.enter_root(&mut ctx);
        run(&mut ctx, &mut stack, 20, script);

        send("replay.record_start", serde_json::Value::Null);
        drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
        run(&mut ctx, &mut stack, 120, script);
        let recording = property(&ctx, &control);
        assert_eq!(recording["mode"], "recording");
        assert_eq!(recording["start_tick"], 20);
        assert_eq!(recording["ticks"], 120);
        let end = hash(&ctx, &stack);

        send("replay.record_stop", serde_json::json!({ "path": path }));
        drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
        assert_eq!(Replay::load(Path::new(&path)).unwrap().ticks, 120);

        send("replay.play", serde_json::json!({ "path": path }));
        drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
        assert_eq!(property(&ctx, &control)["mode"], "playing");
        run(&mut ctx, &mut stack, 120, noise);
        let finished = property(&ctx, &control);
        assert_eq!(finished["mode"], "finished");
        assert_eq!(finished["desync_tick"], serde_json::Value::Null);
        assert_eq!(hash(&ctx, &stack), end);

        // `replay.stop` with nothing playing changes nothing.
        send("replay.stop", serde_json::Value::Null);
        drain_api_commands(&shared, &mut ctx, &mut stack, &mut control);
        assert_eq!(property(&ctx, &control)["mode"], "finished");
    }
}
