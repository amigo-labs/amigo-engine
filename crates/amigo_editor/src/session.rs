//! One editing session: the level being edited, the file it came from, its
//! undo history, and what the viewport tools and the API do to it.
//!
//! The engine owns one of these when built with the `editor` feature. Nothing
//! here needs a window or egui, so everything a mouse, a shortcut or a JSON-RPC
//! client can do to a level is testable on the CPU.

use crate::{AmigoLevel, EditorCommand, EditorState, EditorTool, PathData, load_level, save_level};
use amigo_core::{Color, Rect};
use serde_json::Value;
use std::path::{Path, PathBuf};
use winit::keyboard::KeyCode;

/// Something the editor UI asks the engine to do with the level file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorAction {
    /// Start a new level next to the current one.
    NewLevel,
    /// Write the level to its file.
    Save,
    /// Throw away unsaved changes and read the file again.
    Reload,
}

/// The left mouse button over the viewport, for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerState {
    /// Cursor position in world space.
    pub world: (f32, f32),
    /// Went down this frame.
    pub pressed: bool,
    /// Is down.
    pub held: bool,
}

/// The level being edited and everything needed to change it.
pub struct EditorSession {
    pub state: EditorState,
    pub level: AmigoLevel,
    /// The `.amigo` file the level is saved to.
    pub path: PathBuf,
    /// Where new levels go.
    pub levels_dir: PathBuf,
    /// Changed since the last save or load.
    pub dirty: bool,
    /// The last thing worth telling the user ("Saved …", "Could not load …").
    pub status: Option<String>,
    /// Tiles painted by the brush stroke in progress; recorded as one undo
    /// step when the button comes up.
    stroke: Vec<EditorCommand>,
}

impl EditorSession {
    /// Edit `level`, saved to `path`.
    pub fn new(level: AmigoLevel, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let levels_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        Self {
            state: EditorState::new(),
            level,
            path,
            levels_dir,
            dirty: false,
            status: None,
            stroke: Vec::new(),
        }
    }

    /// Open the first `.amigo` file in `levels_dir` (by name), or start an
    /// untitled level there when it has none. A file that does not load is
    /// reported in [`status`](Self::status) and the next one is tried.
    pub fn open(levels_dir: impl Into<PathBuf>) -> Self {
        let levels_dir = levels_dir.into();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&levels_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|ext| ext == "amigo"))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();

        let mut problems = Vec::new();
        for path in files {
            match load_level(&path) {
                Ok(level) => {
                    let mut session = Self::new(level, &path);
                    session.levels_dir = levels_dir;
                    session.status = Some(if problems.is_empty() {
                        format!("Opened {}", path.display())
                    } else {
                        format!("Opened {} ({})", path.display(), problems.join("; "))
                    });
                    return session;
                }
                Err(e) => problems.push(format!("skipped {}: {e}", path.display())),
            }
        }

        let path = levels_dir.join(next_free_name(&levels_dir));
        let mut session = Self::new(AmigoLevel::new("Untitled", 30, 20, 16), path);
        session.levels_dir = levels_dir;
        session.dirty = true;
        session.status = Some(if problems.is_empty() {
            "No level found; started a new one".to_string()
        } else {
            format!("Started a new level ({})", problems.join("; "))
        });
        session
    }

    /// Apply `cmd` and record it for undo.
    pub fn execute(&mut self, cmd: EditorCommand) -> Result<(), String> {
        self.finish_stroke();
        self.state.execute_in(&mut self.level, cmd)?;
        self.dirty = true;
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        self.finish_stroke();
        let undone = self.state.undo_in(&mut self.level);
        self.dirty |= undone;
        undone
    }

    pub fn redo(&mut self) -> bool {
        self.finish_stroke();
        let redone = self.state.redo_in(&mut self.level);
        self.dirty |= redone;
        redone
    }

    /// Write the level to [`path`](Self::path). Returns the path written.
    pub fn save(&mut self) -> Result<PathBuf, String> {
        self.finish_stroke();
        self.level.validate()?;
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        save_level(&self.path, &self.level)
            .map_err(|e| format!("could not save {}: {e}", self.path.display()))?;
        self.dirty = false;
        self.status = Some(format!("Saved {}", self.path.display()));
        Ok(self.path.clone())
    }

    /// Save to `path`, which becomes the level's file from now on.
    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> Result<PathBuf, String> {
        let previous = std::mem::replace(&mut self.path, path.into());
        let saved = self.save();
        if saved.is_err() {
            self.path = previous;
        }
        saved
    }

    /// Read the level's file again, dropping unsaved changes and history.
    pub fn reload(&mut self) -> Result<(), String> {
        let path = self.path.clone();
        self.load(path)
    }

    /// Edit the level in `path` instead. On failure nothing changes.
    pub fn load(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let level =
            load_level(&path).map_err(|e| format!("could not load {}: {e}", path.display()))?;
        self.stroke.clear();
        self.level = level;
        self.path = path;
        self.state.clear_history();
        self.state.selected_layer = 0;
        self.dirty = false;
        self.status = Some(format!("Loaded {}", self.path.display()));
        Ok(())
    }

    /// Start an empty `width × height` level with the current tile size, to
    /// be saved as the next free `level_NN.amigo`.
    pub fn new_level(&mut self, width: u32, height: u32) -> Result<(), String> {
        let cells = u64::from(width) * u64::from(height);
        if width == 0 || height == 0 || cells > crate::MAX_LEVEL_TILES {
            return Err(format!("a {width}x{height} level is not allowed"));
        }
        let name = next_free_name(&self.levels_dir);
        self.stroke.clear();
        self.level = AmigoLevel::new(
            name.trim_end_matches(".amigo"),
            width,
            height,
            self.level.tile_size,
        );
        self.path = self.levels_dir.join(name);
        self.state.clear_history();
        self.state.selected_layer = 0;
        self.dirty = true;
        self.status = Some(format!("New level {}", self.path.display()));
        Ok(())
    }

    /// Run an [`EditorAction`]. Returns the file written, for a save.
    pub fn perform(&mut self, action: EditorAction) -> Result<Option<PathBuf>, String> {
        let result = match action {
            EditorAction::Save => self.save().map(Some),
            EditorAction::Reload => self.reload().map(|()| None),
            EditorAction::NewLevel => self
                .new_level(self.level.width, self.level.height)
                .map(|()| None),
        };
        if let Err(e) = &result {
            self.status = Some(e.clone());
        }
        result
    }

    /// The tile under a world position (may be outside the level).
    pub fn tile_at(&self, (x, y): (f32, f32)) -> (i32, i32) {
        let ts = self.level.tile_size.max(1) as f32;
        ((x / ts).floor() as i32, (y / ts).floor() as i32)
    }

    /// Drive the active tool with the mouse. Call once per frame while the
    /// editor is active and the pointer is over the viewport.
    ///
    /// Paint and Erase work while the button is held, and a whole stroke is
    /// one undo step; Fill and entity placement act on the press only.
    pub fn pointer(&mut self, pointer: PointerState) {
        let (tx, ty) = self.tile_at(pointer.world);
        self.state.cursor_tile = Some((tx, ty));
        let layer = self.state.selected_layer;

        match self.state.tool {
            EditorTool::PaintTile | EditorTool::Erase if pointer.held => {
                if self.state.tool == EditorTool::Erase
                    && pointer.pressed
                    && let Some(index) = self.entity_in_tile(tx, ty)
                {
                    // Erasing over an entity removes it instead of the tile.
                    if let Some(cmd) = EditorCommand::remove_entity(&self.level, index) {
                        let _ = self.execute(cmd);
                    }
                    return;
                }
                let tile = if self.state.tool == EditorTool::Erase {
                    0
                } else {
                    self.state.selected_tile
                };
                if let Some(cmd) = EditorCommand::paint(&self.level, layer, tx, ty, tile)
                    && crate::apply(&mut self.level, &cmd).is_ok()
                {
                    self.stroke.push(cmd);
                    self.dirty = true;
                }
            }
            EditorTool::Fill if pointer.pressed => {
                if let Some(cmd) =
                    EditorCommand::flood_fill(&self.level, layer, tx, ty, self.state.selected_tile)
                {
                    let _ = self.execute(cmd);
                }
            }
            EditorTool::PlaceEntity
                if pointer.pressed && self.level.cell_index(tx, ty).is_some() =>
            {
                let ts = self.level.tile_size as f32;
                let cmd = EditorCommand::PlaceEntity {
                    index: self.level.entities.len(),
                    entity_type: self.entity_type(),
                    x: tx as f32 * ts,
                    y: ty as f32 * ts,
                    properties: Default::default(),
                };
                let _ = self.execute(cmd);
            }
            _ => {}
        }

        if !pointer.held {
            self.finish_stroke();
        }
    }

    /// Record the brush stroke in progress as one undo step.
    pub fn finish_stroke(&mut self) {
        match self.stroke.len() {
            0 => {}
            1 => self.state.execute(self.stroke.remove(0)),
            _ => {
                let stroke = std::mem::take(&mut self.stroke);
                self.state.execute(EditorCommand::Batch(stroke));
            }
        }
    }

    fn entity_type(&self) -> String {
        let t = self.state.selected_entity_type.trim();
        if t.is_empty() {
            "entity".to_string()
        } else {
            t.to_string()
        }
    }

    /// The last entity whose position lies in tile `(tx, ty)`.
    fn entity_in_tile(&self, tx: i32, ty: i32) -> Option<usize> {
        self.level
            .entities
            .iter()
            .rposition(|e| self.tile_at((e.x, e.y)) == (tx, ty))
    }

    /// Editor shortcuts. Returns an action for the engine to run (Ctrl+S).
    ///
    /// Ctrl+Z undo, Ctrl+Y or Ctrl+Shift+Z redo, Ctrl+S save; S select,
    /// P paint, E erase, F fill, N place entity, G grid, `[`/`]` previous and
    /// next tile, Tab next layer.
    pub fn handle_key(&mut self, key: KeyCode, ctrl: bool, shift: bool) -> Option<EditorAction> {
        if ctrl {
            match key {
                KeyCode::KeyZ if shift => {
                    self.redo();
                }
                KeyCode::KeyZ => {
                    self.undo();
                }
                KeyCode::KeyY => {
                    self.redo();
                }
                KeyCode::KeyS => return Some(EditorAction::Save),
                _ => {}
            }
            return None;
        }
        let state = &mut self.state;
        match key {
            KeyCode::KeyS => state.tool = EditorTool::Select,
            KeyCode::KeyP => state.tool = EditorTool::PaintTile,
            KeyCode::KeyE => state.tool = EditorTool::Erase,
            KeyCode::KeyF => state.tool = EditorTool::Fill,
            KeyCode::KeyN => state.tool = EditorTool::PlaceEntity,
            KeyCode::KeyG => state.grid_visible = !state.grid_visible,
            KeyCode::BracketLeft => state.selected_tile = state.selected_tile.saturating_sub(1),
            KeyCode::BracketRight => state.selected_tile = state.selected_tile.saturating_add(1),
            KeyCode::Tab => {
                let layers = self.level.layers.len().max(1);
                state.selected_layer = (state.selected_layer + 1) % layers;
            }
            _ => {}
        }
        None
    }

    /// World-space rectangles that show the level over the game: a coloured
    /// square per non-empty tile (there is no tileset to draw them with), the
    /// grid, entity markers, and the cursor. Only what overlaps `view`.
    pub fn overlay(&self, view: Rect) -> Vec<(Rect, Color)> {
        let mut out = Vec::new();
        let level = &self.level;
        let ts = level.tile_size.max(1) as f32;
        let (lw, lh) = (level.width as f32 * ts, level.height as f32 * ts);

        // Visible tile range, clamped to the level.
        let x0 = ((view.x / ts).floor().max(0.0) as u32).min(level.width);
        let y0 = ((view.y / ts).floor().max(0.0) as u32).min(level.height);
        let x1 = (((view.x + view.w) / ts).ceil().max(0.0) as u32).min(level.width);
        let y1 = (((view.y + view.h) / ts).ceil().max(0.0) as u32).min(level.height);

        for (i, layer) in level.layers.iter().enumerate() {
            if !layer.visible {
                continue;
            }
            // Layers other than the one being painted are drawn fainter.
            let alpha = if i == self.state.selected_layer {
                0.6
            } else {
                0.25
            };
            for y in y0..y1 {
                for x in x0..x1 {
                    let index = (y as usize) * (level.width as usize) + x as usize;
                    let tile = layer.tiles.get(index).copied().unwrap_or(0);
                    if tile != 0 {
                        let [r, g, b] = tile_preview_color(tile);
                        out.push((
                            Rect::new(x as f32 * ts, y as f32 * ts, ts, ts),
                            Color::from_rgba(r, g, b, 255).with_alpha(alpha),
                        ));
                    }
                }
            }
        }

        if self.state.grid_visible {
            let line = Color::new(1.0, 1.0, 1.0, 0.12);
            let (top, bottom) = (y0 as f32 * ts, y1 as f32 * ts);
            let (left, right) = (x0 as f32 * ts, x1 as f32 * ts);
            for x in x0..=x1 {
                out.push((Rect::new(x as f32 * ts, top, 1.0, bottom - top), line));
            }
            for y in y0..=y1 {
                out.push((Rect::new(left, y as f32 * ts, right - left, 1.0), line));
            }
        }
        // The level's edge, so its extent is visible even with the grid off.
        outline(
            &mut out,
            Rect::new(0.0, 0.0, lw, lh),
            Color::new(1.0, 0.8, 0.2, 0.5),
        );

        for e in &level.entities {
            if view.contains(e.x, e.y) {
                let [r, g, b] = tile_preview_color(name_hash(&e.entity_type));
                out.push((
                    Rect::new(e.x + 2.0, e.y + 2.0, ts - 4.0, ts - 4.0),
                    Color::from_rgba(r, g, b, 200),
                ));
                outline(
                    &mut out,
                    Rect::new(e.x + 1.0, e.y + 1.0, ts - 2.0, ts - 2.0),
                    Color::WHITE,
                );
            }
        }

        if let Some((tx, ty)) = self.state.cursor_tile
            && level.cell_index(tx, ty).is_some()
        {
            outline(
                &mut out,
                Rect::new(tx as f32 * ts, ty as f32 * ts, ts, ts),
                Color::new(1.0, 1.0, 1.0, 0.9),
            );
        }
        out
    }

    /// Execute an `editor.*` command from the JSON-RPC API (the action name
    /// without the `editor.` prefix). Returns the file written, for `save`.
    ///
    /// The parameters are the ones `amigo_api` queues: `layer` is a layer
    /// name, tile and entity positions are in tiles and world units
    /// respectively.
    pub fn run_api(&mut self, action: &str, params: &Value) -> Result<Option<PathBuf>, String> {
        match action {
            "new_level" => {
                let width = u32_param(params, "width").unwrap_or(30);
                let height = u32_param(params, "height").unwrap_or(20);
                self.new_level(width, height)?;
                if let Some(world) = params.get("world").and_then(Value::as_str) {
                    self.level
                        .metadata
                        .insert("world".to_string(), world.to_string());
                }
            }
            "paint_tile" => {
                let layer = self.layer_param(params)?;
                let (x, y) = (i32_param(params, "x")?, i32_param(params, "y")?);
                let tile = tile_param(params)?;
                if self.level.cell_index(x, y).is_none() {
                    return Err(format!("tile ({x}, {y}) is outside the level"));
                }
                if let Some(cmd) = EditorCommand::paint(&self.level, layer, x, y, tile) {
                    self.execute(cmd)?;
                }
            }
            "fill_rect" => {
                let layer = self.layer_param(params)?;
                let at = (i32_param(params, "x")?, i32_param(params, "y")?);
                let size = (i32_param(params, "w")?, i32_param(params, "h")?);
                let tile = tile_param(params)?;
                if let Some(cmd) = EditorCommand::fill_rect(&self.level, layer, at, size, tile) {
                    self.execute(cmd)?;
                }
            }
            "place_entity" => {
                let entity_type = params
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or("missing 'type'")?;
                let cmd = EditorCommand::PlaceEntity {
                    index: self.level.entities.len(),
                    entity_type: entity_type.to_string(),
                    x: f32_param(params, "x")?,
                    y: f32_param(params, "y")?,
                    properties: Default::default(),
                };
                self.execute(cmd)?;
            }
            "add_path" => {
                let points = params
                    .get("points")
                    .and_then(Value::as_array)
                    .ok_or("'points' must be a list")?
                    .iter()
                    .map(point)
                    .collect::<Result<Vec<_>, _>>()?;
                if points.is_empty() {
                    return Err("a path needs at least one point".to_string());
                }
                let index = self.level.paths.len();
                let path = PathData {
                    name: format!("path_{}", index + 1),
                    points,
                    closed: params
                        .get("closed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                self.execute(EditorCommand::AddPath { index, path })?;
            }
            "move_path_point" => {
                let path_index = usize_param(params, "path")?;
                let point_index = usize_param(params, "point")?;
                let new_pos = point(params.get("new_pos").ok_or("missing 'new_pos'")?)?;
                let old_pos = self
                    .level
                    .paths
                    .get(path_index)
                    .and_then(|p| p.points.get(point_index))
                    .copied()
                    .ok_or_else(|| format!("path {path_index} has no point {point_index}"))?;
                self.execute(EditorCommand::MovePath {
                    path_index,
                    point_index,
                    old_pos,
                    new_pos,
                })?;
            }
            "undo" => {
                self.undo();
            }
            "redo" => {
                self.redo();
            }
            "save" => {
                let saved = match params.get("path").and_then(Value::as_str) {
                    Some(path) => self.save_as(path)?,
                    None => self.save()?,
                };
                return Ok(Some(saved));
            }
            "load" => {
                let path = params
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing 'path'")?;
                self.load(path)?;
            }
            "auto_decorate" => {
                return Err("editor.auto_decorate is not implemented yet".to_string());
            }
            other => return Err(format!("unknown editor command '{other}'")),
        }
        Ok(None)
    }

    /// A summary the engine publishes as the `editor` property
    /// (`engine.get_property`), so an API client can see what its
    /// commands did.
    pub fn summary(&self) -> Value {
        serde_json::json!({
            "path": self.path.display().to_string(),
            "name": self.level.name,
            "width": self.level.width,
            "height": self.level.height,
            "tile_size": self.level.tile_size,
            "layers": self.level.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
            "entities": self.level.entities.len(),
            "paths": self.level.paths.len(),
            "dirty": self.dirty,
            "undo": self.state.undo_stack.len(),
            "redo": self.state.redo_stack.len(),
            "active": self.state.active,
        })
    }

    fn layer_param(&self, params: &Value) -> Result<usize, String> {
        match params.get("layer") {
            Some(Value::String(name)) => self.level.layer_index(name).ok_or_else(|| {
                let names: Vec<_> = self.level.layers.iter().map(|l| l.name.as_str()).collect();
                format!("no layer named '{name}' (layers: {})", names.join(", "))
            }),
            Some(v) => v
                .as_u64()
                .map(|i| i as usize)
                .filter(|&i| i < self.level.layers.len())
                .ok_or_else(|| format!("no layer {v}")),
            None => Ok(self.state.selected_layer),
        }
    }
}

/// A colour per tile id for the editor's preview squares and palette, so the
/// same id looks the same in both. Id 0 is empty and never drawn.
pub fn tile_preview_color(tile: u16) -> [u8; 3] {
    // Golden-ratio hue steps keep neighbouring ids apart.
    let hue = (f32::from(tile) * 0.618_034).fract() * 6.0;
    let (s, v) = (0.55, 0.9);
    let c = v * s;
    let x = c * (1.0 - ((hue % 2.0) - 1.0).abs());
    let (r, g, b) = match hue as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to_u8 = |f: f32| ((f + m) * 255.0).round() as u8;
    [to_u8(r), to_u8(g), to_u8(b)]
}

fn name_hash(name: &str) -> u16 {
    name.bytes()
        .fold(7u16, |h, b| h.wrapping_mul(31).wrapping_add(u16::from(b)))
        .max(1)
}

fn outline(out: &mut Vec<(Rect, Color)>, r: Rect, color: Color) {
    out.push((Rect::new(r.x, r.y, r.w, 1.0), color));
    out.push((Rect::new(r.x, r.y + r.h - 1.0, r.w, 1.0), color));
    out.push((Rect::new(r.x, r.y, 1.0, r.h), color));
    out.push((Rect::new(r.x + r.w - 1.0, r.y, 1.0, r.h), color));
}

/// `level_01.amigo`, `level_02.amigo`, … — the first that does not exist.
fn next_free_name(dir: &Path) -> String {
    (1..)
        .map(|n| format!("level_{n:02}.amigo"))
        .find(|name| !dir.join(name).exists())
        .unwrap_or_else(|| "level.amigo".to_string())
}

fn i32_param(params: &Value, key: &str) -> Result<i32, String> {
    params
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|v| i32::try_from(v).ok())
        .ok_or_else(|| format!("'{key}' must be an integer"))
}

fn u32_param(params: &Value, key: &str) -> Option<u32> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
}

fn usize_param(params: &Value, key: &str) -> Result<usize, String> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .ok_or_else(|| format!("'{key}' must be a non-negative integer"))
}

fn f32_param(params: &Value, key: &str) -> Result<f32, String> {
    params
        .get(key)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("'{key}' must be a number"))
}

fn tile_param(params: &Value) -> Result<u16, String> {
    params
        .get("tile")
        .and_then(Value::as_i64)
        .and_then(|v| u16::try_from(v).ok())
        .ok_or_else(|| "'tile' must be a tile id from 0 to 65535".to_string())
}

/// `[x, y]` or `{"x": x, "y": y}`.
fn point(v: &Value) -> Result<(f32, f32), String> {
    let (x, y) = match v {
        Value::Array(a) if a.len() == 2 => (a[0].as_f64(), a[1].as_f64()),
        Value::Object(o) => (
            o.get("x").and_then(Value::as_f64),
            o.get("y").and_then(Value::as_f64),
        ),
        _ => (None, None),
    };
    match (x, y) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok((x as f32, y as f32)),
        _ => Err(format!(
            "{v} is not a point: use [x, y] or {{\"x\": x, \"y\": y}}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("amigo_session_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn session() -> EditorSession {
        EditorSession::new(AmigoLevel::new("t", 10, 8, 16), temp_dir().join("t.amigo"))
    }

    fn at_tile(x: i32, y: i32) -> (f32, f32) {
        (x as f32 * 16.0 + 4.0, y as f32 * 16.0 + 4.0)
    }

    fn drag(s: &mut EditorSession, tiles: &[(i32, i32)]) {
        for (i, &(x, y)) in tiles.iter().enumerate() {
            s.pointer(PointerState {
                world: at_tile(x, y),
                pressed: i == 0,
                held: true,
            });
        }
        let &(x, y) = tiles.last().unwrap();
        s.pointer(PointerState {
            world: at_tile(x, y),
            pressed: false,
            held: false,
        });
    }

    #[test]
    fn a_brush_stroke_paints_every_tile_and_undoes_in_one_step() {
        let mut s = session();
        s.state.tool = EditorTool::PaintTile;
        s.state.selected_tile = 4;
        drag(&mut s, &[(1, 1), (2, 1), (2, 1), (3, 1)]);
        for x in 1..=3 {
            assert_eq!(s.level.tile(0, x, 1), Some(4));
        }
        assert_eq!(s.state.undo_stack.len(), 1, "one stroke, one undo step");
        assert!(s.dirty);

        assert!(s.undo());
        assert!((1..=3).all(|x| s.level.tile(0, x, 1) == Some(0)));
        assert!(s.redo());
        assert_eq!(s.level.tile(0, 2, 1), Some(4));
    }

    #[test]
    fn painting_outside_the_level_does_nothing() {
        let mut s = session();
        s.state.tool = EditorTool::PaintTile;
        s.state.selected_tile = 4;
        drag(&mut s, &[(-1, 0), (10, 0), (0, 8)]);
        assert!(s.state.undo_stack.is_empty());
        assert!(!s.dirty);
        assert_eq!(s.state.cursor_tile, Some((0, 8)));
    }

    #[test]
    fn a_click_places_exactly_one_entity_on_the_tile() {
        let mut s = session();
        s.state.tool = EditorTool::PlaceEntity;
        s.state.selected_entity_type = "slime".into();
        // Held for several frames: still one entity.
        drag(&mut s, &[(2, 3), (2, 3), (4, 3)]);
        assert_eq!(s.level.entities.len(), 1);
        let e = &s.level.entities[0];
        assert_eq!((e.entity_type.as_str(), e.x, e.y), ("slime", 32.0, 48.0));

        // Erase over it removes the entity, not the tile.
        s.state.tool = EditorTool::Erase;
        drag(&mut s, &[(2, 3)]);
        assert!(s.level.entities.is_empty());
        assert!(s.undo());
        assert_eq!(s.level.entities.len(), 1);
    }

    #[test]
    fn fill_floods_the_clicked_region() {
        let mut s = session();
        s.state.tool = EditorTool::Fill;
        s.state.selected_tile = 2;
        drag(&mut s, &[(0, 0)]);
        assert!(s.level.layers[0].tiles.iter().all(|&t| t == 2));
        assert_eq!(s.state.undo_stack.len(), 1);
    }

    #[test]
    fn shortcuts_switch_tools_and_drive_history() {
        let mut s = session();
        s.handle_key(KeyCode::KeyP, false, false);
        assert_eq!(s.state.tool, EditorTool::PaintTile);
        s.handle_key(KeyCode::BracketRight, false, false);
        assert_eq!(s.state.selected_tile, 2);
        drag(&mut s, &[(0, 0)]);
        s.handle_key(KeyCode::KeyZ, true, false);
        assert_eq!(s.level.tile(0, 0, 0), Some(0));
        s.handle_key(KeyCode::KeyY, true, false);
        assert_eq!(s.level.tile(0, 0, 0), Some(2));
        assert_eq!(
            s.handle_key(KeyCode::KeyS, true, false),
            Some(EditorAction::Save)
        );
        assert_eq!(s.handle_key(KeyCode::KeyS, false, false), None);
        assert_eq!(s.state.tool, EditorTool::Select);
    }

    #[test]
    fn open_save_reload_and_new_level() {
        let dir = temp_dir();
        // Nothing there: an untitled level that saves as level_01.
        let mut s = EditorSession::open(&dir);
        assert_eq!(s.path, dir.join("level_01.amigo"));
        s.level.layers[0].tiles[3] = 7;
        let saved = s.perform(EditorAction::Save).unwrap();
        assert_eq!(saved.as_deref(), Some(dir.join("level_01.amigo").as_path()));
        assert!(!s.dirty);

        // A broken file sorts first and is skipped, with a note.
        std::fs::write(dir.join("a_broken.amigo"), "not ron").unwrap();
        let mut reopened = EditorSession::open(&dir);
        assert_eq!(reopened.path, dir.join("level_01.amigo"));
        assert_eq!(reopened.level.layers[0].tiles[3], 7);
        assert!(reopened.status.as_deref().unwrap().contains("a_broken"));

        // Reload drops unsaved edits and history.
        reopened.state.tool = EditorTool::Erase;
        drag(&mut reopened, &[(3, 0)]);
        assert_eq!(reopened.level.layers[0].tiles[3], 0);
        reopened.perform(EditorAction::Reload).unwrap();
        assert_eq!(reopened.level.layers[0].tiles[3], 7);
        assert!(!reopened.state.can_undo());

        // A new level gets the next free name and the same size.
        reopened.perform(EditorAction::NewLevel).unwrap();
        assert_eq!(reopened.path, dir.join("level_02.amigo"));
        assert_eq!((reopened.level.width, reopened.level.height), (30, 20));
        assert!(reopened.level.layers[0].tiles.iter().all(|&t| t == 0));
    }

    #[test]
    fn a_failed_load_changes_nothing() {
        let mut s = session();
        s.level.layers[0].tiles[0] = 5;
        assert!(s.load(temp_dir().join("missing.amigo")).is_err());
        assert_eq!(s.level.layers[0].tiles[0], 5);
        assert!(s.path.ends_with("t.amigo"));
    }

    #[test]
    fn api_commands_change_the_level() {
        let mut s = session();
        s.run_api(
            "paint_tile",
            &json!({"layer": "ground", "x": 2, "y": 3, "tile": 9}),
        )
        .unwrap();
        assert_eq!(s.level.tile(0, 2, 3), Some(9));

        s.run_api(
            "fill_rect",
            &json!({"layer": "ground", "x": 0, "y": 0, "w": 3, "h": 2, "tile": 1}),
        )
        .unwrap();
        assert_eq!(s.level.tile(0, 2, 1), Some(1));
        assert_eq!(s.state.undo_stack.len(), 2, "a rectangle is one undo step");

        s.run_api(
            "place_entity",
            &json!({"type": "chest", "x": 40.0, "y": 8.0}),
        )
        .unwrap();
        assert_eq!(s.level.entities[0].entity_type, "chest");

        s.run_api("add_path", &json!({"points": [[0, 0], {"x": 16, "y": 0}]}))
            .unwrap();
        s.run_api(
            "move_path_point",
            &json!({"path": 0, "point": 1, "new_pos": [32, 8]}),
        )
        .unwrap();
        assert_eq!(s.level.paths[0].points, [(0.0, 0.0), (32.0, 8.0)]);

        s.run_api("undo", &Value::Null).unwrap();
        assert_eq!(s.level.paths[0].points[1], (16.0, 0.0));
        s.run_api("redo", &Value::Null).unwrap();
        assert_eq!(s.level.paths[0].points[1], (32.0, 8.0));

        let summary = s.summary();
        assert_eq!(summary["entities"], 1);
        assert_eq!(summary["paths"], 1);
        assert_eq!(summary["dirty"], true);
    }

    #[test]
    fn api_errors_name_the_problem() {
        let mut s = session();
        let err = s
            .run_api(
                "paint_tile",
                &json!({"layer": "sky", "x": 0, "y": 0, "tile": 1}),
            )
            .unwrap_err();
        assert!(err.contains("sky") && err.contains("ground"), "{err}");
        let err = s
            .run_api(
                "paint_tile",
                &json!({"layer": "ground", "x": 99, "y": 0, "tile": 1}),
            )
            .unwrap_err();
        assert!(err.contains("outside"), "{err}");
        assert!(
            s.run_api(
                "paint_tile",
                &json!({"layer": "ground", "x": 0, "y": 0, "tile": 70000})
            )
            .is_err()
        );
        assert!(
            s.run_api("auto_decorate", &json!({}))
                .unwrap_err()
                .contains("not implemented")
        );
        assert!(!s.dirty);
    }

    #[test]
    fn api_save_and_load() {
        let dir = temp_dir();
        let mut s = session();
        s.run_api(
            "new_level",
            &json!({"world": "caves", "width": 4, "height": 3}),
        )
        .unwrap();
        assert_eq!(
            s.level.metadata.get("world").map(String::as_str),
            Some("caves")
        );
        let target = dir.join("caves.amigo");
        let saved = s
            .run_api("save", &json!({"path": target.to_str().unwrap()}))
            .unwrap();
        assert_eq!(saved.as_deref(), Some(target.as_path()));

        let mut other = session();
        other
            .run_api("load", &json!({"path": target.to_str().unwrap()}))
            .unwrap();
        assert_eq!((other.level.width, other.level.height), (4, 3));
        assert_eq!(other.path, target);
    }

    #[test]
    fn overlay_shows_tiles_grid_entities_and_cursor_in_view_only() {
        let mut s = session();
        s.level.layers[0].tiles[0] = 3; // (0, 0)
        s.level.layers[0].tiles[9] = 3; // (9, 0), off screen below
        s.state.cursor_tile = Some((1, 1));
        s.state.grid_visible = false;
        let view = Rect::new(0.0, 0.0, 64.0, 64.0);
        let rects = s.overlay(view);
        let tiles: Vec<_> = rects
            .iter()
            .filter(|(r, _)| r.w == 16.0 && r.h == 16.0)
            .collect();
        assert_eq!(tiles.len(), 1, "only the tile in view");
        assert_eq!((tiles[0].0.x, tiles[0].0.y), (0.0, 0.0));
        assert!(
            rects
                .iter()
                .any(|(r, _)| r.x == 16.0 && r.y == 16.0 && r.h == 1.0),
            "cursor"
        );

        s.state.grid_visible = true;
        let with_grid = s.overlay(view);
        // 4 visible columns and rows: 5 lines each way.
        assert_eq!(with_grid.len(), rects.len() + 10);
    }

    #[test]
    fn preview_colours_are_stable_and_distinct() {
        assert_eq!(tile_preview_color(3), tile_preview_color(3));
        assert_ne!(tile_preview_color(1), tile_preview_color(2));
    }
}
