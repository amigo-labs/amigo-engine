pub mod ui;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Editor commands (undo / redo)
// ---------------------------------------------------------------------------

/// A reversible command that can be applied to a level.
#[derive(Clone, Debug)]
pub enum EditorCommand {
    PaintTile {
        layer: usize,
        x: i32,
        y: i32,
        old_tile: u16,
        new_tile: u16,
    },
    PlaceEntity {
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
            EditorCommand::PlaceEntity { entity_type, x, y } => EditorCommand::RemoveEntity {
                index: 0, // caller must patch the real index
                entity_type: entity_type.clone(),
                x: *x,
                y: *y,
            },
            EditorCommand::RemoveEntity {
                entity_type, x, y, ..
            } => EditorCommand::PlaceEntity {
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
    pub fn redo(&mut self) -> Option<EditorCommand> {
        let cmd = self.redo_stack.pop()?;
        let inverse = cmd.inverse();
        self.undo_stack.push(cmd);
        Some(inverse)
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

// ---------------------------------------------------------------------------
// Save / Load
// ---------------------------------------------------------------------------

/// Serialize a level to RON and write it to the given path.
pub fn save_level(path: &std::path::Path, level: &AmigoLevel) -> Result<(), std::io::Error> {
    let ron_string = ron::ser::to_string_pretty(level, ron::ser::PrettyConfig::default())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    std::fs::write(path, ron_string)
}

/// Load a level from a RON file at the given path.
pub fn load_level(path: &std::path::Path) -> Result<AmigoLevel, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    ron::from_str(&contents).map_err(|e| e.to_string())
}
