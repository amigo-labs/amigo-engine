use crate::play_state::{PlayModeManager, PlayState};
use crate::{AmigoLevel, EditorCommand, EditorState};
use amigo_core::{Color, Rect};
use amigo_input::InputState;
use amigo_ui::UiContext;

// ---------------------------------------------------------------------------
// Editor events (emitted through the engine's EventHub)
// ---------------------------------------------------------------------------

/// Event emitted when the editor executes a command (paint, place, etc.).
#[derive(Clone, Debug)]
pub struct EditorCommandEvent(pub EditorCommand);

/// Event emitted when the play state changes.
#[derive(Clone, Debug)]
pub struct PlayStateChanged {
    pub old: PlayState,
    pub new: PlayState,
}

/// Event emitted when the editor loads or creates a level.
#[derive(Clone, Debug)]
pub struct LevelLoaded {
    pub name: String,
}

// ---------------------------------------------------------------------------
// EditorPlugin — bundles editor state as an engine resource
// ---------------------------------------------------------------------------

/// Bundles the editor's runtime data so it can be stored as a resource.
///
/// Games add editor support via the engine's resource system:
///
/// ```rust,ignore
/// // In Game::init:
/// ctx.resources.insert(EditorRuntime::new(my_level));
///
/// // In Game::update:
/// if let Some(editor) = ctx.resources.get_mut::<EditorRuntime>() {
///     editor.update(&ctx.input);
/// }
/// ```
pub struct EditorRuntime {
    pub state: EditorState,
    pub play_mode: PlayModeManager,
    pub level: AmigoLevel,
    /// Whether the editor overlay is visible ([`EditorRuntimeKeys::overlay`]
    /// toggles).
    pub overlay_visible: bool,
    /// The keys this runtime reacts to.
    pub keys: EditorRuntimeKeys,
}

/// Keys of [`EditorRuntime`]. The defaults are F10–F12, because the engine's
/// debug overlay owns F1–F8 and its built-in editor F9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorRuntimeKeys {
    /// Start playing from the edited level, or stop and restore it.
    pub play: winit::keyboard::KeyCode,
    /// Pause or resume while playing.
    pub pause: winit::keyboard::KeyCode,
    /// Show or hide the overlay.
    pub overlay: winit::keyboard::KeyCode,
}

impl Default for EditorRuntimeKeys {
    fn default() -> Self {
        use winit::keyboard::KeyCode;
        Self {
            play: KeyCode::F10,
            pause: KeyCode::F11,
            overlay: KeyCode::F12,
        }
    }
}

/// "F10" for `KeyCode::F10`.
fn key_label(key: winit::keyboard::KeyCode) -> String {
    format!("{key:?}")
}

impl EditorRuntime {
    pub fn new(level: AmigoLevel) -> Self {
        let mut state = EditorState::new();
        state.active = true;
        Self {
            state,
            play_mode: PlayModeManager::new(),
            level,
            overlay_visible: true,
            keys: EditorRuntimeKeys::default(),
        }
    }

    /// Create with default empty level.
    pub fn with_empty_level(width: u32, height: u32, tile_size: u32) -> Self {
        Self::new(AmigoLevel::new("Untitled", width, height, tile_size))
    }

    /// Process a frame of editor input. Call from Game::update().
    ///
    /// Returns any editor command that was produced (e.g. from undo/redo
    /// shortcuts) so the game can apply it to the tilemap.
    pub fn update(&mut self, input: &InputState) -> Option<EditorCommand> {
        let keys = self.keys;

        // Play / stop.
        if input.pressed(keys.play) {
            let old = self.play_mode.state().clone();
            match old {
                PlayState::Editing => {
                    // Serialize current level as snapshot
                    let snapshot = ron::ser::to_string(&self.level)
                        .unwrap_or_default()
                        .into_bytes();
                    self.play_mode.start_play(snapshot);
                    self.state.active = false;
                }
                PlayState::Playing | PlayState::Paused => {
                    // Restore level from snapshot
                    if let Some(data) = self.play_mode.stop_play()
                        && let Ok(s) = std::str::from_utf8(&data)
                        && let Ok(restored) = ron::from_str::<AmigoLevel>(s)
                    {
                        self.level = restored;
                    }
                    self.state.active = true;
                }
            }
        }

        // Pause / resume during play.
        if input.pressed(keys.pause) {
            if self.play_mode.is_playing() {
                self.play_mode.pause();
            } else if self.play_mode.is_paused() {
                self.play_mode.resume();
            }
        }

        // Overlay visibility.
        if input.pressed(keys.overlay) {
            self.overlay_visible = !self.overlay_visible;
        }

        // Tick play mode
        self.play_mode.tick();

        // Process editor-specific input only while editing
        if self.state.active {
            return self.handle_editor_input(input);
        }

        None
    }

    fn handle_editor_input(&mut self, input: &InputState) -> Option<EditorCommand> {
        use winit::keyboard::KeyCode;

        // Undo/Redo
        if input.held(KeyCode::ControlLeft) && input.pressed(KeyCode::KeyZ) {
            let cmd = if input.held(KeyCode::ShiftLeft) {
                self.state.redo()
            } else {
                self.state.undo()
            };
            // Keep this runtime's own level in step; the command is also
            // returned for game state that mirrors it (a tilemap, say).
            if let Some(cmd) = &cmd {
                let _ = crate::apply(&mut self.level, cmd);
            }
            return cmd;
        }

        None
    }

    /// Draw the editor overlay. Call from Game::draw().
    pub fn draw_overlay(&self, ui: &mut UiContext, screen_w: f32, screen_h: f32) {
        if !self.overlay_visible {
            return;
        }

        // Draw play-state indicator in top-center
        let indicator_w = 120.0;
        let x = (screen_w - indicator_w) * 0.5;
        let y = 2.0;

        let (label, color) = match self.play_mode.state() {
            PlayState::Editing => ("EDIT MODE", Color::new(0.3, 0.8, 0.3, 1.0)),
            PlayState::Playing => ("PLAYING", Color::new(0.8, 0.3, 0.3, 1.0)),
            PlayState::Paused => ("PAUSED", Color::new(0.8, 0.8, 0.3, 1.0)),
        };

        ui.filled_rect(
            Rect::new(x, y, indicator_w, 16.0),
            Color::new(0.0, 0.0, 0.0, 0.7),
        );
        ui.pixel_text(label, x + 4.0, y + 4.0, color);

        // Show play ticks when playing/paused
        if !self.play_mode.is_editing() {
            let ticks = self.play_mode.play_ticks();
            let secs = ticks as f32 / 60.0;
            let time_str = format!("{:.1}s", secs);
            ui.pixel_text(
                &time_str,
                x + indicator_w - 40.0,
                y + 4.0,
                Color::new(0.8, 0.8, 0.8, 1.0),
            );
        }

        // Hotkey hints at bottom
        let hint_y = screen_h - 14.0;
        let hint_color = Color::new(0.6, 0.6, 0.6, 0.8);
        let (play, pause, overlay) = (
            key_label(self.keys.play),
            key_label(self.keys.pause),
            key_label(self.keys.overlay),
        );
        let hint = if self.play_mode.is_editing() {
            format!("{play}:Play  {overlay}:Overlay")
        } else {
            format!("{play}:Stop  {pause}:Pause  {overlay}:Overlay")
        };
        ui.pixel_text(&hint, 4.0, hint_y, hint_color);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_level() -> AmigoLevel {
        AmigoLevel::new("Test", 8, 8, 16)
    }

    #[test]
    fn new_editor_runtime() {
        let rt = EditorRuntime::new(test_level());
        assert!(rt.state.active);
        assert!(rt.play_mode.is_editing());
        assert!(rt.overlay_visible);
    }

    fn press(key: winit::keyboard::KeyCode) -> InputState {
        use winit::event::ElementState;
        use winit::keyboard::PhysicalKey;
        let mut input = InputState::new();
        input.handle_key_event(PhysicalKey::Code(key), ElementState::Pressed);
        input
    }

    #[test]
    fn the_keys_stay_clear_of_the_debug_overlay() {
        use winit::keyboard::KeyCode;
        let mut rt = EditorRuntime::new(test_level());
        // F5-F7 belong to the debug overlay: nothing happens.
        for key in [KeyCode::F5, KeyCode::F6, KeyCode::F7] {
            rt.update(&press(key));
        }
        assert!(rt.play_mode.is_editing());
        assert!(rt.overlay_visible);

        rt.update(&press(KeyCode::F10));
        assert!(rt.play_mode.is_playing());
        rt.update(&press(KeyCode::F11));
        assert!(rt.play_mode.is_paused());
        rt.update(&press(KeyCode::F12));
        assert!(!rt.overlay_visible);
        rt.update(&press(KeyCode::F10));
        assert!(rt.play_mode.is_editing());
        assert!(rt.state.active);
    }

    #[test]
    fn keys_can_be_rebound() {
        use winit::keyboard::KeyCode;
        let mut rt = EditorRuntime::new(test_level());
        rt.keys.play = KeyCode::KeyP;
        rt.update(&press(KeyCode::F10));
        assert!(rt.play_mode.is_editing());
        rt.update(&press(KeyCode::KeyP));
        assert!(rt.play_mode.is_playing());
        assert_eq!(key_label(KeyCode::F10), "F10");
    }

    #[test]
    fn empty_level_constructor() {
        let rt = EditorRuntime::with_empty_level(16, 16, 8);
        assert_eq!(rt.level.width, 16);
        assert_eq!(rt.level.height, 16);
        assert_eq!(rt.level.tile_size, 8);
        assert_eq!(rt.level.layers.len(), 1);
        assert_eq!(rt.level.layers[0].tiles.len(), 256);
    }

    #[test]
    fn level_snapshot_roundtrip() {
        let mut rt = EditorRuntime::new(test_level());
        // Modify the level
        rt.level.name = "Modified".to_string();

        // Serialize and snapshot
        let snapshot = ron::ser::to_string(&rt.level).unwrap().into_bytes();
        rt.play_mode.start_play(snapshot);
        assert!(rt.play_mode.is_playing());

        // Further modify during play
        rt.level.name = "During Play".to_string();

        // Stop and restore
        let data = rt.play_mode.stop_play().unwrap();
        let s = std::str::from_utf8(&data).unwrap();
        let restored: AmigoLevel = ron::from_str(s).unwrap();
        assert_eq!(restored.name, "Modified");
    }
}
