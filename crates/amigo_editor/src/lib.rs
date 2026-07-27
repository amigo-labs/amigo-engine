pub mod auto_path;
pub mod collision_editor;
pub mod heatmap;
pub mod play_state;
pub mod playtest;
pub mod plugin;
pub mod tidal_playground;
pub mod ui;
pub mod visual_script;
pub mod wizard;
pub mod wizard_ui;

#[cfg(feature = "td")]
pub mod wave_editor;

#[cfg(feature = "egui")]
pub mod egui_ui;

#[cfg(feature = "editor_v2")]
pub mod editor_v2;

#[cfg(feature = "editor_v2")]
pub mod inspector;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Editor commands (undo / redo)
// ---------------------------------------------------------------------------

/// A reversible command that can be applied to a level.
#[derive(Clone, Debug, PartialEq)]
pub enum EditorCommand {
    PaintTile {
        layer: usize,
        x: i32,
        y: i32,
        old_tile: u16,
        new_tile: u16,
    },
    PlaceEntity {
        /// Index the entity occupies in `AmigoLevel::entities`.
        ///
        /// Carried so that [`EditorCommand::inverse`] can name the exact entity
        /// to remove. Without it, undoing a placement had to guess.
        index: usize,
        entity_type: String,
        x: f32,
        y: f32,
    },
    RemoveEntity {
        index: usize,
        entity_type: String,
        x: f32,
        y: f32,
    },
    MovePath {
        path_index: usize,
        point_index: usize,
        old_pos: (f32, f32),
        new_pos: (f32, f32),
    },
}

impl EditorCommand {
    /// Returns the inverse of this command (used when undoing / redoing).
    pub fn inverse(&self) -> Self {
        match self {
            EditorCommand::PaintTile {
                layer,
                x,
                y,
                old_tile,
                new_tile,
            } => EditorCommand::PaintTile {
                layer: *layer,
                x: *x,
                y: *y,
                old_tile: *new_tile,
                new_tile: *old_tile,
            },
            EditorCommand::PlaceEntity {
                index,
                entity_type,
                x,
                y,
            } => EditorCommand::RemoveEntity {
                index: *index,
                entity_type: entity_type.clone(),
                x: *x,
                y: *y,
            },
            EditorCommand::RemoveEntity {
                index,
                entity_type,
                x,
                y,
            } => EditorCommand::PlaceEntity {
                index: *index,
                entity_type: entity_type.clone(),
                x: *x,
                y: *y,
            },
            EditorCommand::MovePath {
                path_index,
                point_index,
                old_pos,
                new_pos,
            } => EditorCommand::MovePath {
                path_index: *path_index,
                point_index: *point_index,
                old_pos: *new_pos,
                new_pos: *old_pos,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Editor tool palette
// ---------------------------------------------------------------------------

/// The currently active editor tool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditorTool {
    #[default]
    Select,
    PaintTile,
    Erase,
    Fill,
    PlaceEntity,
    PathEdit,
}

// ---------------------------------------------------------------------------
// Editor state
// ---------------------------------------------------------------------------

/// Runtime state of the level editor.
pub struct EditorState {
    pub active: bool,
    pub tool: EditorTool,
    pub selected_tile: u16,
    pub selected_entity_type: String,
    pub selected_path: Option<usize>,
    pub undo_stack: Vec<EditorCommand>,
    pub redo_stack: Vec<EditorCommand>,
    pub grid_visible: bool,
    pub show_collision: bool,
    pub show_paths: bool,
    pub cursor_tile: Option<(i32, i32)>,
    /// The new-project wizard (Some while active).
    pub project_wizard: Option<wizard::NewProjectWizard>,
    /// The resulting project after the wizard completes.
    pub created_project: Option<amigo_core::game_preset::GameProject>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            active: false,
            tool: EditorTool::Select,
            selected_tile: 0,
            selected_entity_type: String::new(),
            selected_path: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            grid_visible: true,
            show_collision: false,
            show_paths: true,
            cursor_tile: None,
            project_wizard: None,
            created_project: None,
        }
    }

    /// Toggle the editor on or off.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Execute a command, pushing it onto the undo stack and clearing the redo
    /// stack (since the timeline has diverged).
    pub fn execute(&mut self, cmd: EditorCommand) {
        self.redo_stack.clear();
        self.undo_stack.push(cmd);
    }

    /// Pop the most recent command from the undo stack, push its inverse onto
    /// the redo stack, and return the inverse command so the caller can apply it.
    pub fn undo(&mut self) -> Option<EditorCommand> {
        let cmd = self.undo_stack.pop()?;
        let inverse = cmd.inverse();
        self.redo_stack.push(cmd);
        Some(inverse)
    }

    /// Pop the most recent command from the redo stack, push it onto the undo
    /// stack, and return a clone so the caller can re-apply it.
    ///
    /// Returns the command itself, not its inverse: [`undo`](Self::undo) already
    /// handed the caller the inverse to apply, so redoing means applying the
    /// original again. Returning the inverse here made redo repeat the undo —
    /// for `PaintTile` it re-applied `old_tile`, so redo did nothing at all.
    pub fn redo(&mut self) -> Option<EditorCommand> {
        let cmd = self.redo_stack.pop()?;
        self.undo_stack.push(cmd.clone());
        Some(cmd)
    }

    /// Discard all undo and redo history.
    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Open the new-project wizard.
    pub fn open_new_project_wizard(&mut self) {
        self.project_wizard = Some(wizard::NewProjectWizard::new());
    }

    /// Returns true while the wizard is open.
    pub fn is_wizard_open(&self) -> bool {
        self.project_wizard.is_some()
    }

    /// Take the created project (consumes it). Call after wizard completes.
    pub fn take_created_project(&mut self) -> Option<amigo_core::game_preset::GameProject> {
        self.created_project.take()
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Level data types (.amigo format)
// ---------------------------------------------------------------------------

/// A single tile layer in a level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerData {
    pub name: String,
    pub tiles: Vec<u16>,
    pub visible: bool,
}

/// A placed entity instance inside a level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityPlacement {
    pub entity_type: String,
    pub x: f32,
    pub y: f32,
    pub properties: HashMap<String, String>,
}

/// A named path (e.g. for AI movement or camera rails).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathData {
    pub name: String,
    pub points: Vec<(f32, f32)>,
    pub closed: bool,
}

/// The complete level document serialized as `.amigo` (RON format).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmigoLevel {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub layers: Vec<LayerData>,
    pub entities: Vec<EntityPlacement>,
    pub paths: Vec<PathData>,
    pub metadata: HashMap<String, String>,
}

/// Upper bound on tiles per layer, so a hostile or corrupt `.amigo` file cannot
/// make the editor allocate absurd amounts up front. 4096x4096 tiles is far past
/// any hand-authored pixel-art level.
pub const MAX_LEVEL_TILES: u64 = 4096 * 4096;

/// Number of cells in a `width * height` grid.
///
/// Computed in `u64`: `width * height` in `u32` wraps for large dimensions, and
/// a wrapped length produces a buffer far too small for the `y * width + x`
/// indexing that follows.
///
/// # Panics
/// If the grid exceeds [`MAX_LEVEL_TILES`] cells. Editor grids are sized from
/// level dimensions, so anything past this is a caller bug rather than
/// something to silently clamp.
pub fn grid_len(width: u32, height: u32) -> usize {
    let cells = u64::from(width) * u64::from(height);
    assert!(
        cells <= MAX_LEVEL_TILES,
        "grid of {}x{} = {} cells exceeds the {} cell limit",
        width,
        height,
        cells,
        MAX_LEVEL_TILES
    );
    cells as usize
}

impl AmigoLevel {
    /// Check that the level's declared dimensions match its data.
    ///
    /// Consumers index layer tiles as `y * width + x`, so a layer whose tile
    /// count disagrees with `width * height` means out-of-bounds reads or
    /// silently wrong tiles. `tile_size == 0` divides by zero when converting
    /// world positions to tile coordinates.
    pub fn validate(&self) -> Result<(), String> {
        if self.tile_size == 0 {
            return Err("tile_size must be greater than 0".to_string());
        }

        let expected = u64::from(self.width) * u64::from(self.height);
        if expected > MAX_LEVEL_TILES {
            return Err(format!(
                "level is {}x{} = {} tiles, which exceeds the {} tile limit",
                self.width, self.height, expected, MAX_LEVEL_TILES
            ));
        }

        for (i, layer) in self.layers.iter().enumerate() {
            if layer.tiles.len() as u64 != expected {
                return Err(format!(
                    "layer {} ('{}') has {} tiles but the level is {}x{} = {}",
                    i,
                    layer.name,
                    layer.tiles.len(),
                    self.width,
                    self.height,
                    expected
                ));
            }
        }

        for (i, path) in self.paths.iter().enumerate() {
            if path.closed && path.points.len() < 3 {
                return Err(format!(
                    "path {} ('{}') is closed but has only {} point(s)",
                    i,
                    path.name,
                    path.points.len()
                ));
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Save / Load
// ---------------------------------------------------------------------------

/// Serialize a level to RON and write it to the given path.
pub fn save_level(path: &std::path::Path, level: &AmigoLevel) -> Result<(), std::io::Error> {
    let ron_string = ron::ser::to_string_pretty(level, ron::ser::PrettyConfig::default())
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(path, ron_string)
}

/// Load a level from a RON file at the given path.
///
/// The file is validated before being handed back — see
/// [`AmigoLevel::validate`]. A `.amigo` file is external input like any other
/// asset, and consumers index layer tiles as `y * width + x`.
pub fn load_level(path: &std::path::Path) -> Result<AmigoLevel, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let level: AmigoLevel = ron::from_str(&contents).map_err(|e| e.to_string())?;
    level.validate()?;
    Ok(level)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn paint(old_tile: u16, new_tile: u16) -> EditorCommand {
        EditorCommand::PaintTile {
            layer: 0,
            x: 3,
            y: 4,
            old_tile,
            new_tile,
        }
    }

    fn level(width: u32, height: u32, tile_count: usize) -> AmigoLevel {
        AmigoLevel {
            name: "test".into(),
            width,
            height,
            tile_size: 16,
            layers: vec![LayerData {
                name: "ground".into(),
                tiles: vec![0; tile_count],
                visible: true,
            }],
            entities: Vec::new(),
            paths: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    // ── Undo / redo ───────────────────────────────────────────────

    /// The caller applies whatever undo/redo hands back. `redo` used to return
    /// the inverse — the same thing `undo` had already applied — so redoing a
    /// paint re-applied `old_tile` and the redo was a no-op.
    #[test]
    fn redo_returns_the_original_command_not_its_inverse() {
        let mut state = EditorState::new();
        state.execute(paint(7, 9));

        let undone = state.undo().expect("something to undo");
        assert_eq!(
            undone,
            paint(9, 7),
            "undo should hand back the inverse to apply"
        );

        let redone = state.redo().expect("something to redo");
        assert_eq!(
            redone,
            paint(7, 9),
            "redo should re-apply the original command"
        );
    }

    #[test]
    fn undo_redo_cycle_is_stable() {
        let mut state = EditorState::new();
        state.execute(paint(1, 2));

        for _ in 0..3 {
            assert_eq!(state.undo().unwrap(), paint(2, 1));
            assert!(state.can_redo());
            assert_eq!(state.redo().unwrap(), paint(1, 2));
            assert!(state.can_undo());
        }
    }

    #[test]
    fn execute_clears_the_redo_stack() {
        let mut state = EditorState::new();
        state.execute(paint(1, 2));
        state.undo();
        assert!(state.can_redo());

        state.execute(paint(3, 4));
        assert!(!state.can_redo(), "a new command diverges the timeline");
    }

    /// `inverse` must round-trip for every variant, otherwise undo/redo drifts.
    #[test]
    fn inverse_is_an_involution() {
        let commands = [
            paint(1, 2),
            EditorCommand::PlaceEntity {
                index: 5,
                entity_type: "slime".into(),
                x: 1.0,
                y: 2.0,
            },
            EditorCommand::RemoveEntity {
                index: 5,
                entity_type: "slime".into(),
                x: 1.0,
                y: 2.0,
            },
            EditorCommand::MovePath {
                path_index: 1,
                point_index: 2,
                old_pos: (0.0, 1.0),
                new_pos: (2.0, 3.0),
            },
        ];

        for cmd in commands {
            assert_eq!(
                cmd.inverse().inverse(),
                cmd,
                "inverse of inverse should be the original: {:?}",
                cmd
            );
        }
    }

    /// Undoing a placement has to name the entity that was placed. The inverse
    /// used to hardcode `index: 0` with a comment telling the caller to patch
    /// it, so an unpatched undo removed whichever entity happened to be first.
    #[test]
    fn undoing_a_placement_targets_the_placed_entity() {
        let mut state = EditorState::new();
        state.execute(EditorCommand::PlaceEntity {
            index: 7,
            entity_type: "chest".into(),
            x: 32.0,
            y: 64.0,
        });

        match state.undo().expect("something to undo") {
            EditorCommand::RemoveEntity { index, .. } => assert_eq!(index, 7),
            other => panic!("expected RemoveEntity, got {:?}", other),
        }
    }

    // ── Level validation ─────────────────────────────────────────

    #[test]
    fn validate_accepts_a_consistent_level() {
        assert!(level(8, 8, 64).validate().is_ok());
    }

    #[test]
    fn validate_rejects_tile_count_mismatch() {
        let err = level(8, 8, 63).validate().expect_err("should be rejected");
        assert!(
            err.contains("63"),
            "error should name the actual count: {err}"
        );
    }

    #[test]
    fn validate_rejects_zero_tile_size() {
        let mut l = level(8, 8, 64);
        l.tile_size = 0;
        assert!(l.validate().is_err(), "tile_size 0 divides by zero later");
    }

    #[test]
    fn validate_rejects_absurd_dimensions() {
        // Declared dimensions that would overflow a u32 multiply.
        let mut l = level(u32::MAX, u32::MAX, 0);
        l.tile_size = 16;
        assert!(l.validate().is_err());
    }

    #[test]
    fn validate_rejects_closed_path_without_enough_points() {
        let mut l = level(4, 4, 16);
        l.paths.push(PathData {
            name: "loop".into(),
            points: vec![(0.0, 0.0), (1.0, 1.0)],
            closed: true,
        });
        assert!(l.validate().is_err());
    }

    #[test]
    fn load_level_rejects_an_inconsistent_file() {
        // width*height says 64 tiles, the layer carries 4.
        let bogus = level(8, 8, 4);
        let path = std::env::temp_dir().join(format!("amigo_bogus_{}.amigo", std::process::id()));
        save_level(&path, &bogus).expect("write");

        let err = load_level(&path).expect_err("should refuse an inconsistent level");
        assert!(err.contains("tiles"), "unexpected error: {err}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_load_round_trip() {
        let original = level(6, 5, 30);
        let path = std::env::temp_dir().join(format!("amigo_ok_{}.amigo", std::process::id()));
        save_level(&path, &original).expect("write");

        let loaded = load_level(&path).expect("read back");
        assert_eq!(loaded.width, 6);
        assert_eq!(loaded.height, 5);
        assert_eq!(loaded.layers[0].tiles.len(), 30);

        let _ = std::fs::remove_file(&path);
    }

    // ── Grid sizing ──────────────────────────────────────────────

    /// `width * height` in u32 wraps: 65536*65536 is 0, which used to produce an
    /// empty buffer that later got indexed as if it were full.
    #[test]
    fn grid_len_does_not_wrap() {
        assert_eq!(grid_len(4096, 4096), 4096 * 4096);
        assert_eq!(grid_len(0, 0), 0);
    }

    #[test]
    #[should_panic(expected = "exceeds")]
    fn grid_len_rejects_wrapping_dimensions() {
        // In u32 this multiplies to 0.
        grid_len(65536, 65536);
    }
}
