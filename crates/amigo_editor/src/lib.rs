pub mod auto_path;
pub mod collision_editor;
pub mod heatmap;
pub mod play_state;
pub mod playtest;
pub mod plugin;
pub mod session;
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

use std::collections::HashMap;

// The level document lives in `amigo_core::level` so games can read it without
// depending on the editor; re-exported here for existing callers.
pub use amigo_core::level::{
    AmigoLevel, EntityPlacement, LayerData, MAX_LEVEL_TILES, PathData, grid_len, load_level,
    save_level,
};
pub use session::{EditorAction, EditorSession, PointerState};

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
        /// Carried so undoing a removal brings the entity back whole.
        properties: HashMap<String, String>,
    },
    RemoveEntity {
        index: usize,
        entity_type: String,
        x: f32,
        y: f32,
        properties: HashMap<String, String>,
    },
    MovePath {
        path_index: usize,
        point_index: usize,
        old_pos: (f32, f32),
        new_pos: (f32, f32),
    },
    /// Insert `path` at `index` in `AmigoLevel::paths`.
    AddPath { index: usize, path: PathData },
    /// Remove the path at `index` (which must be `path`).
    RemovePath { index: usize, path: PathData },
    /// Several commands as one undo step: a brush stroke, a flood fill, a
    /// filled rectangle. Applied in order, undone in reverse.
    Batch(Vec<EditorCommand>),
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
                properties,
            } => EditorCommand::RemoveEntity {
                index: *index,
                entity_type: entity_type.clone(),
                x: *x,
                y: *y,
                properties: properties.clone(),
            },
            EditorCommand::RemoveEntity {
                index,
                entity_type,
                x,
                y,
                properties,
            } => EditorCommand::PlaceEntity {
                index: *index,
                entity_type: entity_type.clone(),
                x: *x,
                y: *y,
                properties: properties.clone(),
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
            EditorCommand::AddPath { index, path } => EditorCommand::RemovePath {
                index: *index,
                path: path.clone(),
            },
            EditorCommand::RemovePath { index, path } => EditorCommand::AddPath {
                index: *index,
                path: path.clone(),
            },
            EditorCommand::Batch(commands) => {
                EditorCommand::Batch(commands.iter().rev().map(Self::inverse).collect())
            }
        }
    }

    /// Paint `tile` at `(x, y)` on `layer`, remembering what was there.
    /// `None` outside the level or when the tile is already `tile`.
    pub fn paint(level: &AmigoLevel, layer: usize, x: i32, y: i32, tile: u16) -> Option<Self> {
        let old_tile = level.tile(layer, x, y)?;
        (old_tile != tile).then_some(EditorCommand::PaintTile {
            layer,
            x,
            y,
            old_tile,
            new_tile: tile,
        })
    }

    /// Remove the entity at `index`, remembering it whole for undo.
    pub fn remove_entity(level: &AmigoLevel, index: usize) -> Option<Self> {
        let e = level.entities.get(index)?;
        Some(EditorCommand::RemoveEntity {
            index,
            entity_type: e.entity_type.clone(),
            x: e.x,
            y: e.y,
            properties: e.properties.clone(),
        })
    }

    /// Paint every cell of the `w × h` rectangle at `(x, y)` as one command,
    /// clipped to the level. `None` when nothing would change.
    pub fn fill_rect(
        level: &AmigoLevel,
        layer: usize,
        (x, y): (i32, i32),
        (w, h): (i32, i32),
        tile: u16,
    ) -> Option<Self> {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = x.saturating_add(w).min(level.width as i32);
        let y1 = y.saturating_add(h).min(level.height as i32);
        let cells: Vec<_> = (y0..y1)
            .flat_map(|cy| (x0..x1).map(move |cx| (cx, cy)))
            .filter_map(|(cx, cy)| Self::paint(level, layer, cx, cy, tile))
            .collect();
        (!cells.is_empty()).then_some(EditorCommand::Batch(cells))
    }

    /// Flood-fill the 4-connected area of equal tiles around `(x, y)` with
    /// `tile`, as one command. `None` outside the level or when the area
    /// already is `tile`.
    pub fn flood_fill(level: &AmigoLevel, layer: usize, x: i32, y: i32, tile: u16) -> Option<Self> {
        let target = level.tile(layer, x, y)?;
        if target == tile {
            return None;
        }
        let mut seen = vec![false; level.layers[layer].tiles.len()];
        let mut stack = vec![(x, y)];
        let mut cells = Vec::new();
        while let Some((cx, cy)) = stack.pop() {
            let Some(index) = level.cell_index(cx, cy) else {
                continue;
            };
            if seen[index] || level.layers[layer].tiles[index] != target {
                continue;
            }
            seen[index] = true;
            cells.extend(Self::paint(level, layer, cx, cy, tile));
            stack.extend([(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)]);
        }
        Some(EditorCommand::Batch(cells))
    }
}

/// Apply `cmd` to `level`.
///
/// Every variant is checked first, so a command that does not fit the level
/// (a tile outside it, an index past the end, a path point that does not
/// exist) is refused and leaves `level` untouched instead of panicking. A
/// [`EditorCommand::Batch`] is all or nothing.
pub fn apply(level: &mut AmigoLevel, cmd: &EditorCommand) -> Result<(), String> {
    check(level, cmd)?;
    apply_checked(level, cmd);
    Ok(())
}

fn check(level: &AmigoLevel, cmd: &EditorCommand) -> Result<(), String> {
    match cmd {
        EditorCommand::PaintTile { layer, x, y, .. } => level
            .tile(*layer, *x, *y)
            .map(|_| ())
            .ok_or_else(|| format!("tile ({x}, {y}) on layer {layer} is outside the level")),
        EditorCommand::PlaceEntity { index, .. } if *index > level.entities.len() => Err(format!(
            "entity index {index} is past the end ({} entities)",
            level.entities.len()
        )),
        EditorCommand::PlaceEntity { .. } => Ok(()),
        EditorCommand::RemoveEntity {
            index,
            entity_type,
            x,
            y,
            properties,
        } => match level.entities.get(*index) {
            // Undo puts back exactly what the command names, so it has to be
            // what is there.
            Some(e)
                if e.entity_type == *entity_type
                    && e.x == *x
                    && e.y == *y
                    && e.properties == *properties =>
            {
                Ok(())
            }
            Some(e) => Err(format!(
                "entity {index} is a '{}', not the '{entity_type}' the command removes",
                e.entity_type
            )),
            None => Err(format!("no entity at index {index}")),
        },
        EditorCommand::MovePath {
            path_index,
            point_index,
            ..
        } => level
            .paths
            .get(*path_index)
            .and_then(|p| p.points.get(*point_index))
            .map(|_| ())
            .ok_or_else(|| format!("path {path_index} has no point {point_index}")),
        EditorCommand::AddPath { index, .. } if *index > level.paths.len() => Err(format!(
            "path index {index} is past the end ({} paths)",
            level.paths.len()
        )),
        EditorCommand::AddPath { .. } => Ok(()),
        EditorCommand::RemovePath { index, path } => match level.paths.get(*index) {
            Some(p) if p == path => Ok(()),
            Some(p) => Err(format!(
                "path {index} is '{}', not the '{}' the command removes",
                p.name, path.name
            )),
            None => Err(format!("no path at index {index}")),
        },
        EditorCommand::Batch(commands) => {
            // Entity and path indices shift as a batch runs, so check each
            // step against the level as it will be by then.
            if commands
                .iter()
                .all(|c| matches!(c, EditorCommand::PaintTile { .. }))
            {
                return commands.iter().try_for_each(|c| check(level, c));
            }
            let mut scratch = level.clone();
            for c in commands {
                check(&scratch, c)?;
                apply_checked(&mut scratch, c);
            }
            Ok(())
        }
    }
}

fn apply_checked(level: &mut AmigoLevel, cmd: &EditorCommand) {
    match cmd {
        EditorCommand::PaintTile {
            layer,
            x,
            y,
            new_tile,
            ..
        } => {
            if let Some(index) = level.cell_index(*x, *y) {
                level.layers[*layer].tiles[index] = *new_tile;
            }
        }
        EditorCommand::PlaceEntity {
            index,
            entity_type,
            x,
            y,
            properties,
        } => level.entities.insert(
            *index,
            EntityPlacement {
                entity_type: entity_type.clone(),
                x: *x,
                y: *y,
                properties: properties.clone(),
            },
        ),
        EditorCommand::RemoveEntity { index, .. } => {
            level.entities.remove(*index);
        }
        EditorCommand::MovePath {
            path_index,
            point_index,
            new_pos,
            ..
        } => level.paths[*path_index].points[*point_index] = *new_pos,
        EditorCommand::AddPath { index, path } => level.paths.insert(*index, path.clone()),
        EditorCommand::RemovePath { index, .. } => {
            level.paths.remove(*index);
        }
        EditorCommand::Batch(commands) => {
            for c in commands {
                apply_checked(level, c);
            }
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
    /// Index into `AmigoLevel::layers` that painting goes to.
    pub selected_layer: usize,
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
            selected_tile: 1,
            selected_layer: 0,
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

    /// Apply `cmd` to `level` and record it for undo.
    ///
    /// [`execute`](Self::execute) only records; this is what the editor UI,
    /// the viewport tools and the API use, so an undo stack entry always
    /// matches a change the level actually got.
    pub fn execute_in(&mut self, level: &mut AmigoLevel, cmd: EditorCommand) -> Result<(), String> {
        apply(level, &cmd)?;
        self.execute(cmd);
        Ok(())
    }

    /// Undo the last command on `level`. `false` when there was nothing to
    /// undo (or the level no longer fits the command, which is then dropped).
    pub fn undo_in(&mut self, level: &mut AmigoLevel) -> bool {
        let Some(inverse) = self.undo() else {
            return false;
        };
        if apply(level, &inverse).is_err() {
            self.redo_stack.pop();
            return false;
        }
        true
    }

    /// Redo the last undone command on `level`.
    pub fn redo_in(&mut self, level: &mut AmigoLevel) -> bool {
        let Some(cmd) = self.redo() else {
            return false;
        };
        if apply(level, &cmd).is_err() {
            self.undo_stack.pop();
            return false;
        }
        true
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

    fn level() -> AmigoLevel {
        AmigoLevel::new("test", 8, 6, 16)
    }

    fn place(index: usize, entity_type: &str) -> EditorCommand {
        EditorCommand::PlaceEntity {
            index,
            entity_type: entity_type.into(),
            x: 1.0,
            y: 2.0,
            properties: HashMap::from([("hp".to_string(), "3".to_string())]),
        }
    }

    fn path(name: &str) -> PathData {
        PathData {
            name: name.into(),
            points: vec![(0.0, 0.0), (16.0, 0.0)],
            closed: false,
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
            place(5, "slime"),
            place(5, "slime").inverse(),
            EditorCommand::AddPath {
                index: 0,
                path: path("patrol"),
            },
            EditorCommand::Batch(vec![paint(1, 2), place(0, "slime")]),
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
        state.execute(place(7, "chest"));

        match state.undo().expect("something to undo") {
            EditorCommand::RemoveEntity { index, .. } => assert_eq!(index, 7),
            other => panic!("expected RemoveEntity, got {:?}", other),
        }
    }

    // ── Applying commands ────────────────────────────────────────

    /// Every variant, applied then undone then redone through the state,
    /// lands back exactly where it started and where it ended.
    #[test]
    fn every_command_round_trips_through_undo_and_redo() {
        let mut base = level();
        base.entities.push(EntityPlacement {
            entity_type: "chest".into(),
            x: 0.0,
            y: 0.0,
            properties: HashMap::new(),
        });
        base.paths.push(path("existing"));

        let commands = [
            EditorCommand::paint(&base, 0, 3, 4, 9).unwrap(),
            place(1, "slime"),
            EditorCommand::remove_entity(&base, 0).unwrap(),
            EditorCommand::MovePath {
                path_index: 0,
                point_index: 1,
                old_pos: (16.0, 0.0),
                new_pos: (32.0, 8.0),
            },
            EditorCommand::AddPath {
                index: 1,
                path: path("new"),
            },
            EditorCommand::RemovePath {
                index: 0,
                path: path("existing"),
            },
            EditorCommand::fill_rect(&base, 0, (1, 1), (3, 2), 5).unwrap(),
        ];

        for cmd in commands {
            let mut level = base.clone();
            let mut state = EditorState::new();
            state
                .execute_in(&mut level, cmd.clone())
                .unwrap_or_else(|e| panic!("{cmd:?}: {e}"));
            let after = format!("{level:?}");
            assert_ne!(after, format!("{base:?}"), "{cmd:?} changed nothing");

            assert!(state.undo_in(&mut level));
            assert_eq!(format!("{level:?}"), format!("{base:?}"), "undo of {cmd:?}");
            assert!(state.redo_in(&mut level));
            assert_eq!(format!("{level:?}"), after, "redo of {cmd:?}");
        }
    }

    /// The menu's Undo used to pop the stack and drop the inverse it got back,
    /// so nothing in the level changed.
    #[test]
    fn undo_in_changes_the_level() {
        let mut level = level();
        let mut state = EditorState::new();
        let cmd = EditorCommand::paint(&level, 0, 2, 2, 7).unwrap();
        state.execute_in(&mut level, cmd).unwrap();
        assert_eq!(level.tile(0, 2, 2), Some(7));
        assert!(state.undo_in(&mut level));
        assert_eq!(level.tile(0, 2, 2), Some(0));
        assert!(!state.undo_in(&mut level), "nothing left to undo");
    }

    #[test]
    fn commands_that_do_not_fit_are_refused_without_changes() {
        let mut level = level();
        let before = format!("{level:?}");
        for bad in [
            EditorCommand::PaintTile {
                layer: 0,
                x: 8,
                y: 0,
                old_tile: 0,
                new_tile: 1,
            },
            EditorCommand::PaintTile {
                layer: 3,
                x: 0,
                y: 0,
                old_tile: 0,
                new_tile: 1,
            },
            place(4, "slime"),
            place(0, "slime").inverse(),
            EditorCommand::MovePath {
                path_index: 0,
                point_index: 0,
                old_pos: (0.0, 0.0),
                new_pos: (1.0, 1.0),
            },
            EditorCommand::RemovePath {
                index: 0,
                path: path("none"),
            },
            // All or nothing: the second step is out of bounds.
            EditorCommand::Batch(vec![
                paint(0, 1),
                EditorCommand::PaintTile {
                    layer: 0,
                    x: -1,
                    y: 0,
                    old_tile: 0,
                    new_tile: 1,
                },
            ]),
        ] {
            assert!(
                apply(&mut level, &bad).is_err(),
                "{bad:?} should be refused"
            );
        }
        assert_eq!(format!("{level:?}"), before);
    }

    #[test]
    fn a_batch_checks_indices_as_they_shift() {
        let mut level = level();
        // Two placements at the end, then removing the first: valid only
        // because the first placement made room.
        let batch =
            EditorCommand::Batch(vec![place(0, "a"), place(1, "b"), place(0, "a").inverse()]);
        apply(&mut level, &batch).expect("valid in sequence");
        assert_eq!(level.entities.len(), 1);
        assert_eq!(level.entities[0].entity_type, "b");
        apply(&mut level, &batch.inverse()).expect("and back");
        assert!(level.entities.is_empty());
    }

    #[test]
    fn paint_skips_unchanged_and_outside_cells() {
        let mut level = level();
        assert!(
            EditorCommand::paint(&level, 0, 0, 0, 0).is_none(),
            "already 0"
        );
        assert!(EditorCommand::paint(&level, 0, 99, 0, 1).is_none());
        level.layers[0].tiles[0] = 1;
        assert_eq!(
            EditorCommand::paint(&level, 0, 0, 0, 2),
            Some(EditorCommand::PaintTile {
                layer: 0,
                x: 0,
                y: 0,
                old_tile: 1,
                new_tile: 2
            })
        );
    }

    #[test]
    fn fill_rect_clips_to_the_level() {
        let level = level();
        let Some(EditorCommand::Batch(cells)) =
            EditorCommand::fill_rect(&level, 0, (6, 4), (5, 5), 3)
        else {
            panic!("expected a batch");
        };
        assert_eq!(cells.len(), 2 * 2, "8x6 level: columns 6-7, rows 4-5");
        assert!(EditorCommand::fill_rect(&level, 0, (20, 20), (2, 2), 3).is_none());
    }

    #[test]
    fn flood_fill_stays_inside_its_region() {
        let mut level = AmigoLevel::new("fill", 4, 3, 16);
        // A wall down column 1 splits the level in two.
        for y in 0..3 {
            level.layers[0].tiles[y * 4 + 1] = 9;
        }
        let cmd = EditorCommand::flood_fill(&level, 0, 3, 0, 5).unwrap();
        apply(&mut level, &cmd).unwrap();
        let row = |y: usize| level.layers[0].tiles[y * 4..y * 4 + 4].to_vec();
        assert_eq!(row(0), [0, 9, 5, 5]);
        assert_eq!(row(2), [0, 9, 5, 5]);
        assert!(EditorCommand::flood_fill(&level, 0, 3, 0, 5).is_none());
    }
}
