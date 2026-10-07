//! One editing session: the level being edited, the file it came from, its
//! undo history, and what the viewport tools and the API do to it.
//!
//! The engine owns one of these when built with the `editor` feature. Nothing
//! here needs a window or egui, so everything a mouse, a shortcut or a JSON-RPC
//! client can do to a level is testable on the CPU.

use crate::{
    AmigoLevel, EditorCommand, EditorState, EditorTool, EntityPlacement, PathData, ZoneDef,
    load_level, save_level,
};
use amigo_core::{Color, Rect};
use serde_json::Value;
use std::path::{Path, PathBuf};
use winit::keyboard::KeyCode;

/// Something the editor UI asks the engine to do with the level file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorAction {
    /// Start a new level next to the current one.
    NewLevel,
    /// Write the level to its file.
    Save,
    /// Write the level to this file, which becomes its file.
    SaveAs(PathBuf),
    /// Edit the level in this file instead.
    Open(PathBuf),
    /// Throw away unsaved changes and read the file again.
    Reload,
    /// Write the emitter entity at this index as `assets/data/<name>.emitter.ron`
    /// (the engine does this; it knows the particle types).
    SaveEmitter(usize),
}

/// What the editor's panels keep between frames: open dialogs, text being
/// typed, the entity or zone being edited. No egui types, so it lives in the
/// session and survives the editor being closed and opened.
#[derive(Clone, Debug, Default)]
pub struct EditorUiState {
    /// The Open dialog is showing.
    pub open_dialog: bool,
    /// The Save As dialog is showing, with the file name typed so far.
    pub save_as: Option<String>,
    /// The entity being edited in the inspector: its index and a copy.
    pub entity_edit: Option<(usize, EntityPlacement)>,
    /// The zone being edited in the inspector: its index and a copy.
    pub zone_edit: Option<(usize, ZoneDef)>,
    /// The key of a property about to be added in the inspector.
    pub new_property: String,
    /// The tileset field, before it is applied.
    pub tileset: Option<String>,
    /// The auto-decorate dialog: tiles (comma separated) and density.
    pub decorate: Option<(String, f32)>,
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
    /// Shift is held: the select tool adds to the selection.
    pub shift: bool,
}

/// A drag in progress with the select, zone or path tool.
#[derive(Clone, Debug, PartialEq)]
enum Drag {
    /// Moving the selected entities; their positions when the drag began.
    Move {
        start: (f32, f32),
        originals: Vec<(usize, EntityPlacement)>,
    },
    /// A selection rectangle from `start`.
    Band { start: (f32, f32), add: bool },
    /// A new zone from tile `start`.
    Zone { start: (i32, i32) },
    /// Moving point `point` of path `path`, which was at `old`.
    Point {
        path: usize,
        point: usize,
        old: (f32, f32),
    },
}

/// One thing the engine draws over the game while the editor is open.
#[derive(Clone, Debug, PartialEq)]
pub enum OverlayItem {
    Rect(Rect, Color),
    /// A segment from `a` to `b`, `thickness` pixels wide.
    Line {
        a: (f32, f32),
        b: (f32, f32),
        thickness: f32,
        color: Color,
    },
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
    /// The select, zone or path drag in progress.
    drag: Option<Drag>,
    /// Playtest runs reported for this level (`editor.playtest_report`).
    pub playtest: crate::playtest::PlaytestResults,
    /// Heatmaps recorded for this level (`editor.heat`), by name.
    pub heatmaps: crate::heatmap::HeatmapCollection,
    /// The heatmap drawn over the level, by name.
    pub shown_heatmap: Option<String>,
    /// State of the editor's dialogs and inspectors between frames.
    pub ui: EditorUiState,
    /// Where the pointer is now, for drawing a drag in progress.
    pointer_world: (f32, f32),
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
            drag: None,
            playtest: crate::playtest::PlaytestResults::new(Default::default()),
            heatmaps: crate::heatmap::HeatmapCollection::new(),
            shown_heatmap: None,
            ui: EditorUiState::default(),
            pointer_world: (0.0, 0.0),
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
            EditorAction::SaveAs(path) => {
                let path = self.resolve_level_path(&path);
                self.save_as(path).map(Some)
            }
            EditorAction::Open(path) => {
                let path = self.resolve_level_path(&path);
                self.load(path).map(|()| None)
            }
            EditorAction::Reload => self.reload().map(|()| None),
            EditorAction::SaveEmitter(_) => Err("saving emitters is the engine's job".to_string()),
            EditorAction::NewLevel => self
                .new_level(self.level.width, self.level.height)
                .map(|()| None),
        };
        if let Err(e) = &result {
            self.status = Some(e.clone());
        }
        result
    }

    /// A file name typed in a dialog, as a path: relative names go into the
    /// levels directory, and `.amigo` is added when missing.
    pub fn resolve_level_path(&self, path: &Path) -> PathBuf {
        let mut path =
            if path.is_absolute() || path.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
                path.to_path_buf()
            } else {
                self.levels_dir.join(path)
            };
        if path.extension().is_none_or(|e| e != "amigo") {
            path.set_extension("amigo");
        }
        path
    }

    /// The game's `assets/` directory: the one the levels directory is in.
    pub fn assets_dir(&self) -> PathBuf {
        self.levels_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("assets"))
    }

    /// Copy an image, Aseprite file or atlas manifest into
    /// `assets/sprites/`, where hot reload registers it. Returns where it
    /// went.
    pub fn import_sprite(&self, from: &Path) -> Result<PathBuf, String> {
        let name = from
            .file_name()
            .ok_or_else(|| format!("{} is not a file", from.display()))?;
        let lower = name.to_string_lossy().to_lowercase();
        let importable = [".png", ".aseprite", ".ase", ".atlas.ron"]
            .iter()
            .any(|ext| lower.ends_with(ext));
        if !importable {
            return Err(format!(
                "{} is not a sprite (PNG, Aseprite or .atlas.ron)",
                from.display()
            ));
        }
        let dir = self.assets_dir().join("sprites");
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
        let to = dir.join(name);
        std::fs::copy(from, &to)
            .map_err(|e| format!("could not copy {} to {}: {e}", from.display(), to.display()))?;
        Ok(to)
    }

    /// The `.amigo` files in the levels directory, sorted.
    pub fn level_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.levels_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|ext| ext == "amigo"))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    /// Fill empty cells of the decoration layer (named `layer`, created when
    /// missing) with `tiles`, at `density` (0..1), where the level's first
    /// layer has ground and no entity, path point or zone is in the way.
    /// The pick is seeded from `seed`, so the same call decorates the same
    /// way. One undo step. Returns how many tiles were placed.
    pub fn auto_decorate(
        &mut self,
        layer: &str,
        tiles: &[u16],
        density: f32,
        seed: u64,
    ) -> Result<usize, String> {
        if tiles.is_empty() {
            return Err("no decoration tiles: pass 'tiles' or set metadata 'decor_tiles'".into());
        }
        let density = if density.is_finite() {
            density.clamp(0.0, 1.0)
        } else {
            0.1
        };
        self.finish_stroke();
        let mut level = self.level.clone();
        let mut commands = Vec::new();
        let decor = match level.layer_index(layer) {
            Some(i) => i,
            None => {
                let tiles = vec![0; crate::grid_len(level.width, level.height)];
                level.layers.push(crate::LayerData {
                    name: layer.to_string(),
                    tiles,
                    visible: true,
                });
                level.layers.len() - 1
            }
        };
        let ts = level.tile_size.max(1) as f32;
        let blocked = |x: i32, y: i32| {
            let cell = Rect::new(x as f32 * ts, y as f32 * ts, ts, ts);
            level.entities.iter().any(|e| cell.contains(e.x, e.y))
                || level
                    .paths
                    .iter()
                    .flat_map(|p| p.points.iter())
                    .any(|&(px, py)| cell.contains(px, py))
                || level.zones.iter().any(|z| {
                    Rect::new(z.x, z.y, z.w, z.h).contains(cell.x + ts / 2.0, cell.y + ts / 2.0)
                })
        };
        let mut rng = seed ^ 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for y in 0..level.height as i32 {
            for x in 0..level.width as i32 {
                let roll = next();
                let ground = level.tile(0, x, y).unwrap_or(0) != 0 || decor == 0;
                if !ground || level.tile(decor, x, y) != Some(0) || blocked(x, y) {
                    continue;
                }
                if (roll % 10_000) as f32 >= density * 10_000.0 {
                    continue;
                }
                let tile = tiles[(next() % tiles.len() as u64) as usize];
                if let Some(cmd) = EditorCommand::paint(&level, decor, x, y, tile) {
                    commands.push(cmd);
                }
            }
        }
        if decor >= self.level.layers.len() {
            // The new layer goes in with the level; undo keeps it, empty.
            self.level.layers.push(level.layers[decor].clone());
        }
        let placed = commands.len();
        if placed > 0 {
            self.execute(EditorCommand::Batch(commands))?;
        }
        self.status = Some(format!("Decorated {placed} tile(s) on '{layer}'"));
        Ok(placed)
    }

    /// Add a path from `from` to `to` (world units) around the solid tiles
    /// of layer `layer` (non-zero tiles are solid), found with A*. One undo
    /// step. Returns the new path's index.
    pub fn auto_path(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        layer: usize,
    ) -> Result<usize, String> {
        let level = &self.level;
        let tiles = &level
            .layers
            .get(layer)
            .ok_or_else(|| format!("no layer {layer}"))?
            .tiles;
        let grid = crate::auto_path::TileGrid::new(tiles, level.width, level.height);
        let generated = crate::auto_path::generate_path(
            from,
            to,
            &grid,
            level.tile_size.max(1) as f32,
            &Default::default(),
        )
        .ok_or("no way through: the start or the goal is blocked, or they are not connected")?;
        let index = level.paths.len();
        let path = PathData {
            name: format!("path_{}", index + 1),
            points: generated.points,
            closed: false,
        };
        self.execute(EditorCommand::AddPath { index, path })?;
        self.state.selected_path = Some(index);
        Ok(index)
    }

    /// Balance suggestions from the reported playtests, including the wave
    /// curve.
    pub fn balance_suggestions(&self) -> Vec<crate::playtest::BalanceSuggestion> {
        let mut out = crate::playtest::analyze_balance(&self.playtest);
        out.extend(crate::playtest::analyze_waves(&self.playtest));
        out
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
        self.pointer_world = pointer.world;
        let layer = self.state.selected_layer;

        match self.state.tool {
            EditorTool::Select => self.select_tool(pointer),
            EditorTool::Zone => self.zone_tool(pointer),
            EditorTool::PathEdit => self.path_tool(pointer),
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

    /// Pick, rubber-band select and drag entities; pick zones.
    fn select_tool(&mut self, pointer: PointerState) {
        if pointer.pressed {
            match self.entity_at(pointer.world) {
                Some(index) => {
                    let selected = &mut self.state.selected_entities;
                    if pointer.shift {
                        if let Some(pos) = selected.iter().position(|&i| i == index) {
                            selected.remove(pos);
                            return;
                        }
                        selected.push(index);
                    } else if !selected.contains(&index) {
                        *selected = vec![index];
                    }
                    self.state.selected_zone = None;
                    let originals = self
                        .state
                        .selected_entities
                        .iter()
                        .filter_map(|&i| self.level.entities.get(i).map(|e| (i, e.clone())))
                        .collect();
                    self.drag = Some(Drag::Move {
                        start: pointer.world,
                        originals,
                    });
                }
                None => {
                    if !pointer.shift {
                        self.state.selected_entities.clear();
                    }
                    self.state.selected_zone = self.zone_at(pointer.world);
                    if self.state.selected_zone.is_none() {
                        self.drag = Some(Drag::Band {
                            start: pointer.world,
                            add: pointer.shift,
                        });
                    }
                }
            }
            return;
        }
        match self.drag.take() {
            Some(Drag::Move { start, originals }) => {
                // Whole tiles, so placed entities stay on the grid.
                let ts = self.level.tile_size.max(1) as f32;
                let dx = ((pointer.world.0 - start.0) / ts).round() * ts;
                let dy = ((pointer.world.1 - start.1) / ts).round() * ts;
                for (index, original) in &originals {
                    if let Some(e) = self.level.entities.get_mut(*index) {
                        e.x = original.x + dx;
                        e.y = original.y + dy;
                    }
                }
                if pointer.held {
                    self.drag = Some(Drag::Move { start, originals });
                    return;
                }
                // Put the originals back and record the move as one step.
                let mut commands = Vec::new();
                for (index, original) in originals {
                    if let Some(e) = self.level.entities.get_mut(index) {
                        let moved = std::mem::replace(e, original.clone());
                        if moved != original {
                            commands.push(EditorCommand::SetEntity {
                                index,
                                old: original,
                                new: moved,
                            });
                        }
                    }
                }
                if !commands.is_empty() {
                    let _ = self.execute(EditorCommand::Batch(commands));
                }
            }
            Some(Drag::Band { start, add }) => {
                if pointer.held {
                    self.drag = Some(Drag::Band { start, add });
                    return;
                }
                let band = rect_between(start, pointer.world);
                let ts = self.level.tile_size as f32;
                let inside: Vec<usize> = self
                    .level
                    .entities
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| band.contains(e.x + ts / 2.0, e.y + ts / 2.0))
                    .map(|(i, _)| i)
                    .collect();
                let selected = &mut self.state.selected_entities;
                if !add {
                    selected.clear();
                }
                for i in inside {
                    if !selected.contains(&i) {
                        selected.push(i);
                    }
                }
            }
            other => self.drag = other,
        }
    }

    /// Drag out a zone, a whole number of tiles.
    fn zone_tool(&mut self, pointer: PointerState) {
        let tile = self.tile_at(pointer.world);
        if pointer.pressed {
            if let Some(index) = self.zone_at(pointer.world) {
                self.state.selected_zone = Some(index);
                return;
            }
            self.drag = Some(Drag::Zone { start: tile });
            return;
        }
        if pointer.held {
            return;
        }
        if let Some(Drag::Zone { start }) = self.drag.take() {
            let ts = self.level.tile_size as f32;
            let (x0, x1) = (
                start.0.min(tile.0).max(0),
                start.0.max(tile.0).min(self.level.width as i32 - 1),
            );
            let (y0, y1) = (
                start.1.min(tile.1).max(0),
                start.1.max(tile.1).min(self.level.height as i32 - 1),
            );
            if x0 > x1 || y0 > y1 {
                return;
            }
            let index = self.level.zones.len();
            let zone = ZoneDef {
                name: format!("zone_{}", index + 1),
                x: x0 as f32 * ts,
                y: y0 as f32 * ts,
                w: (x1 - x0 + 1) as f32 * ts,
                h: (y1 - y0 + 1) as f32 * ts,
                properties: Default::default(),
            };
            if self.execute(EditorCommand::AddZone { index, zone }).is_ok() {
                self.state.selected_zone = Some(index);
            }
        }
    }

    /// Click a point to pick and drag it; click elsewhere to add a point to
    /// the selected path, or to start a new path. Points snap to tile
    /// centres and are in world units, like entities.
    fn path_tool(&mut self, pointer: PointerState) {
        if pointer.pressed {
            if let Some((path, point)) = self.path_point_at(pointer.world) {
                self.state.selected_path = Some(path);
                self.state.selected_point = Some((path, point));
                let old = self.level.paths[path].points[point];
                self.drag = Some(Drag::Point { path, point, old });
                return;
            }
            let pos = self.tile_centre(pointer.world);
            match self
                .state
                .selected_path
                .filter(|&p| p < self.level.paths.len())
            {
                Some(path) => {
                    let point = self.level.paths[path].points.len();
                    let cmd = EditorCommand::AddPathPoint {
                        path_index: path,
                        point_index: point,
                        pos,
                    };
                    if self.execute(cmd).is_ok() {
                        self.state.selected_point = Some((path, point));
                    }
                }
                None => {
                    let index = self.level.paths.len();
                    let path = PathData {
                        name: format!("path_{}", index + 1),
                        points: vec![pos],
                        closed: false,
                    };
                    if self.execute(EditorCommand::AddPath { index, path }).is_ok() {
                        self.state.selected_path = Some(index);
                        self.state.selected_point = Some((index, 0));
                    }
                }
            }
            return;
        }
        if let Some(Drag::Point { path, point, old }) = self.drag.take() {
            let new_pos = self.tile_centre(pointer.world);
            if let Some(p) = self
                .level
                .paths
                .get_mut(path)
                .and_then(|p| p.points.get_mut(point))
            {
                *p = new_pos;
            }
            if pointer.held {
                self.drag = Some(Drag::Point { path, point, old });
                return;
            }
            if let Some(p) = self
                .level
                .paths
                .get_mut(path)
                .and_then(|p| p.points.get_mut(point))
            {
                *p = old;
            }
            if new_pos != old {
                let _ = self.execute(EditorCommand::MovePath {
                    path_index: path,
                    point_index: point,
                    old_pos: old,
                    new_pos,
                });
            }
        }
    }

    /// Delete what is selected: the selected entities, else the selected
    /// zone, else (with the path tool) the selected path point. One undo step.
    pub fn delete_selection(&mut self) -> bool {
        if let Some(cmd) =
            EditorCommand::remove_entities(&self.level, &self.state.selected_entities)
        {
            self.state.selected_entities.clear();
            return self.execute(cmd).is_ok();
        }
        if let Some(index) = self.state.selected_zone.take()
            && let Some(cmd) = EditorCommand::remove_zone(&self.level, index)
        {
            return self.execute(cmd).is_ok();
        }
        if let Some((path, point)) = self.state.selected_point.take()
            && let Some(&pos) = self.level.paths.get(path).and_then(|p| p.points.get(point))
        {
            let cmd = if self.level.paths[path].points.len() == 1 {
                self.state.selected_path = None;
                EditorCommand::RemovePath {
                    index: path,
                    path: self.level.paths[path].clone(),
                }
            } else {
                EditorCommand::RemovePathPoint {
                    path_index: path,
                    point_index: point,
                    pos,
                }
            };
            return self.execute(cmd).is_ok();
        }
        false
    }

    /// The topmost entity under a world position.
    fn entity_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let ts = self.level.tile_size as f32;
        self.level
            .entities
            .iter()
            .rposition(|e| x >= e.x && y >= e.y && x < e.x + ts && y < e.y + ts)
    }

    /// The topmost zone under a world position.
    fn zone_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        self.level
            .zones
            .iter()
            .rposition(|z| x >= z.x && y >= z.y && x < z.x + z.w && y < z.y + z.h)
    }

    /// The path point within half a tile of a world position.
    fn path_point_at(&self, (x, y): (f32, f32)) -> Option<(usize, usize)> {
        let reach = self.level.tile_size as f32 / 2.0;
        self.level.paths.iter().enumerate().find_map(|(pi, p)| {
            p.points
                .iter()
                .position(|&(px, py)| (px - x).abs() <= reach && (py - y).abs() <= reach)
                .map(|point| (pi, point))
        })
    }

    fn tile_centre(&self, world: (f32, f32)) -> (f32, f32) {
        let ts = self.level.tile_size.max(1) as f32;
        let (tx, ty) = self.tile_at(world);
        ((tx as f32 + 0.5) * ts, (ty as f32 + 0.5) * ts)
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
    /// P paint, E erase, F fill, N place entity, R path, Z zone, G grid,
    /// `[`/`]` previous and next tile, Tab next layer, Delete/Backspace
    /// delete the selection, Escape clear it.
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
            KeyCode::KeyR => state.tool = EditorTool::PathEdit,
            KeyCode::KeyZ => state.tool = EditorTool::Zone,
            KeyCode::Escape => {
                state.selected_entities.clear();
                state.selected_zone = None;
                state.selected_point = None;
            }
            KeyCode::Delete | KeyCode::Backspace => {
                self.delete_selection();
            }
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

    /// The sprite the level's tiles are drawn with: `metadata["tileset"]`,
    /// tile `n` being cell `n - 1` of the sheet, row by row. `None` draws
    /// coloured squares instead.
    pub fn tileset(&self) -> Option<&str> {
        self.level
            .metadata
            .get("tileset")
            .map(String::as_str)
            .filter(|s| !s.trim().is_empty())
    }

    /// World-space rectangles that show the level over the game: a coloured
    /// square per non-empty tile, the grid, zones, path points, entity
    /// markers, the selection and the cursor. Only what overlaps `view`.
    /// Path segments are left out; [`overlay_items`](Self::overlay_items)
    /// has them as lines.
    pub fn overlay(&self, view: Rect) -> Vec<(Rect, Color)> {
        self.overlay_items(view, true)
            .into_iter()
            .filter_map(|item| match item {
                OverlayItem::Rect(r, c) => Some((r, c)),
                OverlayItem::Line { .. } => None,
            })
            .collect()
    }

    /// Everything the engine draws over the game while the editor is open.
    /// With `tile_preview` off (the engine draws the tiles with the level's
    /// tileset itself), tiles are left out.
    pub fn overlay_items(&self, view: Rect, tile_preview: bool) -> Vec<OverlayItem> {
        let mut rects = Vec::new();
        let mut lines = Vec::new();
        let level = &self.level;
        let ts = level.tile_size.max(1) as f32;
        let (lw, lh) = (level.width as f32 * ts, level.height as f32 * ts);

        // Visible tile range, clamped to the level.
        let x0 = ((view.x / ts).floor().max(0.0) as u32).min(level.width);
        let y0 = ((view.y / ts).floor().max(0.0) as u32).min(level.height);
        let x1 = (((view.x + view.w) / ts).ceil().max(0.0) as u32).min(level.width);
        let y1 = (((view.y + view.h) / ts).ceil().max(0.0) as u32).min(level.height);

        for (i, layer) in level.layers.iter().enumerate() {
            if !tile_preview || !layer.visible {
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
                        rects.push((
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
                rects.push((Rect::new(x as f32 * ts, top, 1.0, bottom - top), line));
            }
            for y in y0..=y1 {
                rects.push((Rect::new(left, y as f32 * ts, right - left, 1.0), line));
            }
        }
        // The level's edge, so its extent is visible even with the grid off.
        outline(
            &mut rects,
            Rect::new(0.0, 0.0, lw, lh),
            Color::new(1.0, 0.8, 0.2, 0.5),
        );

        // Zones: a translucent fill and an outline, brighter when selected.
        for (i, z) in level.zones.iter().enumerate() {
            let selected = self.state.selected_zone == Some(i);
            let r = Rect::new(z.x, z.y, z.w, z.h);
            rects.push((
                r,
                Color::new(0.3, 0.6, 1.0, if selected { 0.25 } else { 0.12 }),
            ));
            outline(
                &mut rects,
                r,
                Color::new(0.4, 0.7, 1.0, if selected { 1.0 } else { 0.6 }),
            );
        }

        if let Some(map) = self
            .shown_heatmap
            .as_deref()
            .and_then(|n| self.heatmaps.get(n))
        {
            for (tx, ty, color) in map.overlay_tiles() {
                rects.push((Rect::new(tx as f32 * ts, ty as f32 * ts, ts, ts), color));
            }
        }

        if self.state.show_paths {
            for (pi, p) in level.paths.iter().enumerate() {
                let selected = self.state.selected_path == Some(pi);
                let color = if selected {
                    Color::new(1.0, 0.9, 0.2, 0.9)
                } else {
                    Color::new(1.0, 0.6, 0.1, 0.6)
                };
                let segments = p.points.windows(2).map(|w| (w[0], w[1]));
                let closing = (p.closed && p.points.len() > 2)
                    .then(|| (p.points[p.points.len() - 1], p.points[0]));
                for (a, b) in segments.chain(closing) {
                    lines.push(OverlayItem::Line {
                        a,
                        b,
                        thickness: 1.5,
                        color,
                    });
                }
                for (i, &(x, y)) in p.points.iter().enumerate() {
                    let picked = self.state.selected_point == Some((pi, i));
                    let size = if picked { 6.0 } else { 4.0 };
                    rects.push((
                        Rect::new(x - size / 2.0, y - size / 2.0, size, size),
                        if picked { Color::WHITE } else { color },
                    ));
                }
            }
        }

        for (i, e) in level.entities.iter().enumerate() {
            if view.contains(e.x, e.y) {
                let [r, g, b] = tile_preview_color(name_hash(&e.entity_type));
                rects.push((
                    Rect::new(e.x + 2.0, e.y + 2.0, ts - 4.0, ts - 4.0),
                    Color::from_rgba(r, g, b, 200),
                ));
                let selected = self.state.selected_entities.contains(&i);
                outline(
                    &mut rects,
                    Rect::new(e.x + 1.0, e.y + 1.0, ts - 2.0, ts - 2.0),
                    if selected {
                        Color::new(0.2, 1.0, 0.4, 1.0)
                    } else {
                        Color::WHITE
                    },
                );
                // A light's reach.
                if e.entity_type == "light"
                    && let Some(radius) = e
                        .properties
                        .get("radius")
                        .and_then(|r| r.parse::<f32>().ok())
                {
                    let (cx, cy) = (e.x + ts / 2.0, e.y + ts / 2.0);
                    outline(
                        &mut rects,
                        Rect::new(cx - radius, cy - radius, radius * 2.0, radius * 2.0),
                        Color::new(1.0, 0.95, 0.5, 0.4),
                    );
                }
            }
        }

        // A drag in progress.
        match &self.drag {
            Some(Drag::Band { start, .. }) => {
                let band = rect_between(*start, self.pointer_world);
                rects.push((band, Color::new(0.2, 1.0, 0.4, 0.1)));
                outline(&mut rects, band, Color::new(0.2, 1.0, 0.4, 0.8));
            }
            Some(Drag::Zone { start }) => {
                let (tx, ty) = self.tile_at(self.pointer_world);
                let (ax, ay) = (start.0.min(tx), start.1.min(ty));
                let (bx, by) = (start.0.max(tx), start.1.max(ty));
                let r = Rect::new(
                    ax as f32 * ts,
                    ay as f32 * ts,
                    (bx - ax + 1) as f32 * ts,
                    (by - ay + 1) as f32 * ts,
                );
                outline(&mut rects, r, Color::new(0.4, 0.7, 1.0, 1.0));
            }
            _ => {}
        }

        if let Some((tx, ty)) = self.state.cursor_tile
            && level.cell_index(tx, ty).is_some()
        {
            outline(
                &mut rects,
                Rect::new(tx as f32 * ts, ty as f32 * ts, ts, ts),
                Color::new(1.0, 1.0, 1.0, 0.9),
            );
        }
        let mut items: Vec<OverlayItem> = rects
            .into_iter()
            .map(|(r, c)| OverlayItem::Rect(r, c))
            .collect();
        items.extend(lines);
        items
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
                let world = params
                    .get("world")
                    .and_then(Value::as_str)
                    .unwrap_or("default");
                let tiles: Vec<u16> = match params.get("tiles").and_then(Value::as_array) {
                    Some(list) => list
                        .iter()
                        .filter_map(|v| v.as_u64().and_then(|t| u16::try_from(t).ok()))
                        .collect(),
                    None => self.decor_tiles(world),
                };
                let layer = params
                    .get("layer")
                    .and_then(Value::as_str)
                    .unwrap_or("decoration")
                    .to_string();
                let density = params
                    .get("density")
                    .and_then(Value::as_f64)
                    .map_or(0.1, |d| d as f32);
                let seed = params
                    .get("seed")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| {
                        u64::from(name_hash(&format!("{world}/{}", self.level.name)))
                    });
                self.level
                    .metadata
                    .entry("world".to_string())
                    .or_insert_with(|| world.to_string());
                self.auto_decorate(&layer, &tiles, density, seed)?;
            }
            "auto_path" => {
                let from = point(params.get("from").ok_or("missing 'from'")?)?;
                let to = point(params.get("to").ok_or("missing 'to'")?)?;
                let layer = match params.get("layer") {
                    Some(_) => self.layer_param(params)?,
                    None => self.level.layer_index("collision").unwrap_or(0),
                };
                self.auto_path(from, to, layer)?;
            }
            "add_zone" => {
                let index = self.level.zones.len();
                let zone = ZoneDef {
                    name: params
                        .get("name")
                        .and_then(Value::as_str)
                        .map_or_else(|| format!("zone_{}", index + 1), str::to_string),
                    x: f32_param(params, "x")?,
                    y: f32_param(params, "y")?,
                    w: f32_param(params, "w")?,
                    h: f32_param(params, "h")?,
                    properties: string_map(params.get("properties")),
                };
                if zone.w <= 0.0 || zone.h <= 0.0 {
                    return Err("a zone needs a positive size".to_string());
                }
                self.execute(EditorCommand::AddZone { index, zone })?;
            }
            "remove_zone" => {
                let index = usize_param(params, "index")?;
                let cmd = EditorCommand::remove_zone(&self.level, index)
                    .ok_or_else(|| format!("no zone {index}"))?;
                self.execute(cmd)?;
            }
            "set_entity" => {
                let index = usize_param(params, "index")?;
                let mut e = self
                    .level
                    .entities
                    .get(index)
                    .cloned()
                    .ok_or_else(|| format!("no entity {index}"))?;
                if let Some(t) = params.get("type").and_then(Value::as_str) {
                    e.entity_type = t.to_string();
                }
                if params.get("x").is_some() {
                    e.x = f32_param(params, "x")?;
                }
                if params.get("y").is_some() {
                    e.y = f32_param(params, "y")?;
                }
                if let Some(props) = params.get("properties") {
                    e.properties = string_map(Some(props));
                }
                if let Some(cmd) = EditorCommand::set_entity(&self.level, index, e) {
                    self.execute(cmd)?;
                }
            }
            "remove_entity" => {
                let index = usize_param(params, "index")?;
                let cmd = EditorCommand::remove_entity(&self.level, index)
                    .ok_or_else(|| format!("no entity {index}"))?;
                self.execute(cmd)?;
            }
            "set_metadata" => {
                let key = params
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or("missing 'key'")?;
                let value = params
                    .get("value")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(cmd) = EditorCommand::set_metadata(&self.level, key, value) {
                    self.execute(cmd)?;
                }
            }
            "playtest_report" => {
                let metrics: crate::playtest::PlaytestMetrics = serde_json::from_value(
                    params
                        .get("metrics")
                        .cloned()
                        .unwrap_or_else(|| params.clone()),
                )
                .map_err(|e| format!("bad playtest metrics: {e}"))?;
                self.playtest.add_run(metrics);
                self.status = Some(format!(
                    "{} playtest run(s) reported",
                    self.playtest.runs.len()
                ));
            }
            "heat" => {
                let name = params
                    .get("map")
                    .and_then(Value::as_str)
                    .unwrap_or("deaths")
                    .to_string();
                let (x, y) = (f32_param(params, "x")?, f32_param(params, "y")?);
                let value = params.get("value").and_then(Value::as_f64).unwrap_or(1.0) as f32;
                if self.heatmaps.get(&name).is_none() {
                    self.heatmaps.add(
                        &name,
                        crate::heatmap::Heatmap::new(
                            crate::heatmap::HeatmapType::Custom,
                            self.level.width,
                            self.level.height,
                            self.level.tile_size as f32,
                        ),
                    );
                }
                if let Some(map) = self.heatmaps.get_mut(&name) {
                    map.record(x, y, value);
                }
                self.shown_heatmap.get_or_insert(name);
            }
            other => return Err(format!("unknown editor command '{other}'")),
        }
        Ok(None)
    }

    /// Decoration tiles for `world`: `metadata["decor_tiles.<world>"]`, else
    /// `metadata["decor_tiles"]`, as comma-separated tile ids.
    pub fn decor_tiles(&self, world: &str) -> Vec<u16> {
        let meta = &self.level.metadata;
        meta.get(&format!("decor_tiles.{world}"))
            .or_else(|| meta.get("decor_tiles"))
            .map(|list| {
                list.split(',')
                    .filter_map(|t| t.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_default()
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
            "zones": self.level.zones.len(),
            "selected_entities": self.state.selected_entities,
            "playtest_runs": self.playtest.runs.len(),
            "suggestions": self
                .balance_suggestions()
                .iter()
                .map(|s| s.description.clone())
                .collect::<Vec<_>>(),
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

/// The rectangle with corners `a` and `b`, in any order.
fn rect_between(a: (f32, f32), b: (f32, f32)) -> Rect {
    Rect::new(
        a.0.min(b.0),
        a.1.min(b.1),
        (a.0 - b.0).abs(),
        (a.1 - b.1).abs(),
    )
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

/// A JSON object of strings (other values as their JSON text), as entity or
/// zone properties.
fn string_map(v: Option<&Value>) -> std::collections::HashMap<String, String> {
    v.and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .map(|(k, v)| {
                    let v = v.as_str().map_or_else(|| v.to_string(), str::to_string);
                    (k.clone(), v)
                })
                .collect()
        })
        .unwrap_or_default()
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
                shift: false,
            });
        }
        let &(x, y) = tiles.last().unwrap();
        s.pointer(PointerState {
            world: at_tile(x, y),
            pressed: false,
            held: false,
            shift: false,
        });
    }

    /// A click (press and release) at a world position.
    fn click(s: &mut EditorSession, world: (f32, f32), shift: bool) {
        for (pressed, held) in [(true, true), (false, false)] {
            s.pointer(PointerState {
                world,
                pressed,
                held,
                shift,
            });
        }
    }

    /// Press at `from`, move to `to` while held, release there.
    fn drag_world(s: &mut EditorSession, from: (f32, f32), to: (f32, f32)) {
        for (world, pressed, held) in [(from, true, true), (to, false, true), (to, false, false)] {
            s.pointer(PointerState {
                world,
                pressed,
                held,
                shift: false,
            });
        }
    }

    fn with_entities(s: &mut EditorSession, tiles: &[(i32, i32)]) {
        s.state.tool = EditorTool::PlaceEntity;
        for &t in tiles {
            drag(s, &[t]);
        }
        s.state.tool = EditorTool::Select;
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
                .contains("decor_tiles")
        );
        assert!(
            s.run_api("remove_zone", &json!({"index": 0}))
                .unwrap_err()
                .contains("no zone 0")
        );
        assert!(
            s.run_api("frobnicate", &Value::Null)
                .unwrap_err()
                .contains("frobnicate")
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

    fn centre(x: i32, y: i32) -> (f32, f32) {
        (x as f32 * 16.0 + 8.0, y as f32 * 16.0 + 8.0)
    }

    #[test]
    fn select_picks_shift_toggles_and_a_band_selects_inside() {
        let mut s = session();
        with_entities(&mut s, &[(1, 1), (3, 1), (5, 5)]);
        click(&mut s, centre(1, 1), false);
        assert_eq!(s.state.selected_entities, [0]);
        click(&mut s, centre(3, 1), true);
        assert_eq!(s.state.selected_entities, [0, 1]);
        click(&mut s, centre(1, 1), true);
        assert_eq!(s.state.selected_entities, [1], "shift-click toggles off");
        click(&mut s, centre(8, 7), false);
        assert!(s.state.selected_entities.is_empty(), "empty space clears");

        drag_world(&mut s, (0.0, 0.0), (72.0, 40.0));
        assert_eq!(s.state.selected_entities, [0, 1]);
        assert!(s.state.undo_stack.len() == 3, "selecting changes nothing");
    }

    #[test]
    fn dragging_the_selection_moves_it_by_whole_tiles_in_one_step() {
        let mut s = session();
        with_entities(&mut s, &[(1, 1), (3, 1)]);
        drag_world(&mut s, (0.0, 0.0), (72.0, 40.0));
        let steps = s.state.undo_stack.len();
        // 33 px right, 15 px down: two tiles and one tile.
        drag_world(&mut s, centre(1, 1), (24.0 + 33.0, 24.0 + 15.0));
        let at = |s: &EditorSession, i: usize| (s.level.entities[i].x, s.level.entities[i].y);
        assert_eq!(at(&s, 0), (48.0, 32.0));
        assert_eq!(at(&s, 1), (80.0, 32.0));
        assert_eq!(s.state.undo_stack.len(), steps + 1);
        assert!(s.undo());
        assert_eq!((at(&s, 0), at(&s, 1)), ((16.0, 16.0), (48.0, 16.0)));
    }

    #[test]
    fn delete_removes_the_selected_entities_then_the_zone() {
        let mut s = session();
        with_entities(&mut s, &[(1, 1), (3, 1), (5, 5)]);
        s.level.zones.push(ZoneDef {
            name: "spawn".into(),
            x: 0.0,
            y: 96.0,
            w: 32.0,
            h: 32.0,
            properties: Default::default(),
        });
        click(&mut s, centre(1, 1), false);
        click(&mut s, centre(5, 5), true);
        s.handle_key(KeyCode::Delete, false, false);
        assert_eq!(s.level.entities.len(), 1);
        assert_eq!(s.level.entities[0].x, 48.0);
        assert!(s.state.selected_entities.is_empty());
        assert!(s.undo(), "both in one step");
        assert_eq!(s.level.entities.len(), 3);

        click(&mut s, (8.0, 100.0), false);
        assert_eq!(s.state.selected_zone, Some(0));
        assert!(s.delete_selection());
        assert!(s.level.zones.is_empty());
        assert!(!s.delete_selection(), "nothing left selected");
    }

    #[test]
    fn the_zone_tool_drags_out_whole_tiles() {
        let mut s = session();
        s.handle_key(KeyCode::KeyZ, false, false);
        assert_eq!(s.state.tool, EditorTool::Zone);
        drag_world(&mut s, centre(3, 2), centre(1, 1));
        assert_eq!(s.level.zones.len(), 1);
        let z = &s.level.zones[0];
        assert_eq!((z.x, z.y, z.w, z.h), (16.0, 16.0, 48.0, 32.0));
        assert_eq!(z.name, "zone_1");
        assert_eq!(s.state.selected_zone, Some(0));

        // Clicking a zone picks it instead of starting another.
        s.state.selected_zone = None;
        click(&mut s, centre(2, 2), false);
        assert_eq!(s.state.selected_zone, Some(0));
        assert_eq!(s.level.zones.len(), 1);

        // Clamped to the level.
        drag_world(&mut s, centre(8, 6), (500.0, 500.0));
        let z = &s.level.zones[1];
        assert_eq!((z.x, z.y, z.w, z.h), (128.0, 96.0, 32.0, 32.0));
        assert!(s.undo());
        assert_eq!(s.level.zones.len(), 1);
    }

    #[test]
    fn the_path_tool_adds_and_drags_points_at_tile_centres() {
        let mut s = session();
        s.handle_key(KeyCode::KeyR, false, false);
        click(&mut s, (34.0, 33.0), false);
        assert_eq!(s.level.paths.len(), 1);
        assert_eq!(s.level.paths[0].points, [(40.0, 40.0)]);
        click(&mut s, centre(5, 2), false);
        assert_eq!(s.level.paths[0].points, [(40.0, 40.0), (88.0, 40.0)]);
        assert_eq!(s.state.selected_point, Some((0, 1)));

        let steps = s.state.undo_stack.len();
        drag_world(&mut s, (86.0, 42.0), centre(5, 4));
        assert_eq!(s.level.paths[0].points[1], (88.0, 72.0));
        assert_eq!(s.state.undo_stack.len(), steps + 1, "a drag is one step");

        assert!(s.delete_selection());
        assert_eq!(s.level.paths[0].points, [(40.0, 40.0)]);
        assert!(s.undo());
        assert_eq!(s.level.paths[0].points.len(), 2);

        // Deleting a path's last point removes the path.
        s.state.selected_point = Some((0, 0));
        s.delete_selection();
        s.state.selected_point = Some((0, 0));
        assert!(s.delete_selection());
        assert!(s.level.paths.is_empty());
        assert_eq!(s.state.selected_path, None);
    }

    #[test]
    fn auto_decorate_fills_free_ground_cells_reproducibly_in_one_step() {
        let mut s = session();
        s.run_api(
            "fill_rect",
            &json!({"layer": "ground", "x": 0, "y": 0, "w": 10, "h": 4, "tile": 1}),
        )
        .unwrap();
        with_entities(&mut s, &[(0, 0)]);
        let mut twin = EditorSession::new(s.level.clone(), temp_dir().join("twin.amigo"));
        let steps = s.state.undo_stack.len();

        let placed = s.auto_decorate("decoration", &[7, 8], 1.0, 42).unwrap();
        assert_eq!(placed, 10 * 4 - 1, "every ground cell but the entity's");
        let decor = s.level.layer_index("decoration").unwrap();
        assert_eq!(s.level.tile(decor, 0, 0), Some(0));
        assert!(matches!(s.level.tile(decor, 1, 0), Some(7 | 8)));
        assert_eq!(s.level.tile(decor, 1, 4), Some(0), "no ground there");
        assert_eq!(s.state.undo_stack.len(), steps + 1);

        twin.auto_decorate("decoration", &[7, 8], 1.0, 42).unwrap();
        assert_eq!(twin.level.layers[decor].tiles, s.level.layers[decor].tiles);

        // Sparser with a lower density; nothing at all at 0.
        let mut sparse = EditorSession::new(twin.level.clone(), temp_dir().join("s.amigo"));
        sparse.level.layers[decor].tiles.fill(0);
        let some = sparse.auto_decorate("decoration", &[7], 0.3, 1).unwrap();
        assert!(some > 0 && some < placed, "{some}");
        sparse.level.layers[decor].tiles.fill(0);
        assert_eq!(sparse.auto_decorate("decoration", &[7], 0.0, 1).unwrap(), 0);

        assert!(s.undo());
        assert!(s.level.layers[decor].tiles.iter().all(|&t| t == 0));
        assert!(s.auto_decorate("decoration", &[], 1.0, 0).is_err());
    }

    #[test]
    fn auto_decorate_over_the_api_reads_tiles_from_metadata() {
        let mut s = session();
        s.run_api(
            "fill_rect",
            &json!({"layer": "ground", "x": 0, "y": 0, "w": 2, "h": 1, "tile": 1}),
        )
        .unwrap();
        s.run_api(
            "set_metadata",
            &json!({"key": "decor_tiles.caves", "value": "5, 6"}),
        )
        .unwrap();
        s.run_api(
            "auto_decorate",
            &json!({"world": "caves", "density": 1.0, "layer": "props"}),
        )
        .unwrap();
        let props = s.level.layer_index("props").unwrap();
        assert!(matches!(s.level.tile(props, 0, 0), Some(5 | 6)));
        assert!(matches!(s.level.tile(props, 1, 0), Some(5 | 6)));
        assert_eq!(s.level.metadata["world"], "caves");
    }

    #[test]
    fn auto_path_goes_around_solid_tiles() {
        let mut s = session();
        s.level.layers.push(crate::LayerData {
            name: "collision".into(),
            tiles: vec![0; 80],
            visible: true,
        });
        // A wall down column 5 with a gap in the bottom row.
        for y in 0..7 {
            s.level.layers[1].tiles[y * 10 + 5] = 1;
        }
        s.run_api("auto_path", &json!({"from": [8, 8], "to": [152, 8]}))
            .unwrap();
        let points = &s.level.paths[0].points;
        assert_eq!(points.first(), Some(&(8.0, 8.0)));
        assert_eq!(points.last(), Some(&(152.0, 8.0)));
        assert!(points.iter().any(|&(_, y)| y >= 112.0), "{points:?}");
        for &(x, y) in points {
            let (tx, ty) = s.tile_at((x, y));
            assert_eq!(
                s.level.tile(1, tx, ty),
                Some(0),
                "({x}, {y}) is in the wall"
            );
        }
        assert_eq!(s.state.selected_path, Some(0));

        s.level.layers[1].tiles[75] = 1;
        let err = s
            .run_api("auto_path", &json!({"from": [8, 8], "to": [152, 8]}))
            .unwrap_err();
        assert!(err.contains("no way through"), "{err}");
    }

    #[test]
    fn api_edits_zones_entities_and_metadata_with_undo() {
        let mut s = session();
        s.run_api(
            "add_zone",
            &json!({"name": "boss", "x": 16, "y": 16, "w": 32, "h": 16, "properties": {"wave": 3}}),
        )
        .unwrap();
        assert_eq!(s.level.zones[0].name, "boss");
        assert_eq!(s.level.zones[0].properties["wave"], "3");
        assert!(
            s.run_api("add_zone", &json!({"x": 0, "y": 0, "w": 0, "h": 4}))
                .is_err()
        );

        s.run_api("place_entity", &json!({"type": "slime", "x": 0, "y": 0}))
            .unwrap();
        s.run_api(
            "set_entity",
            &json!({"index": 0, "type": "bat", "y": 32, "properties": {"hp": "4"}}),
        )
        .unwrap();
        let e = &s.level.entities[0];
        assert_eq!((e.entity_type.as_str(), e.x, e.y), ("bat", 0.0, 32.0));
        assert_eq!(e.properties["hp"], "4");
        s.run_api("undo", &Value::Null).unwrap();
        assert_eq!(s.level.entities[0].entity_type, "slime");

        s.run_api("set_metadata", &json!({"key": "tileset", "value": "tiles"}))
            .unwrap();
        assert_eq!(s.tileset(), Some("tiles"));
        s.run_api("set_metadata", &json!({"key": "tileset"}))
            .unwrap();
        assert_eq!(s.tileset(), None, "no value removes the key");
        s.run_api("undo", &Value::Null).unwrap();
        assert_eq!(s.tileset(), Some("tiles"));

        s.run_api("remove_entity", &json!({"index": 0})).unwrap();
        s.run_api("remove_zone", &json!({"index": 0})).unwrap();
        assert!(s.level.entities.is_empty() && s.level.zones.is_empty());
        let summary = s.summary();
        assert_eq!(summary["zones"], 0);
    }

    #[test]
    fn playtest_reports_and_heat_feed_the_summary_and_overlay() {
        let mut s = session();
        for times in [[100, 120, 400], [100, 140, 420]] {
            s.run_api(
                "playtest_report",
                &json!({"metrics": {"victory": false, "wave_times": times}}),
            )
            .unwrap();
        }
        let summary = s.summary();
        assert_eq!(summary["playtest_runs"], 2);
        let suggestions = summary["suggestions"].as_array().unwrap();
        assert!(
            suggestions
                .iter()
                .any(|d| d.as_str().unwrap().contains("Wave 3")),
            "{suggestions:?}"
        );

        s.run_api("heat", &json!({"x": 20, "y": 20, "value": 3}))
            .unwrap();
        assert_eq!(s.shown_heatmap.as_deref(), Some("deaths"));
        let view = Rect::new(0.0, 0.0, 160.0, 128.0);
        let without = {
            let mut quiet = session();
            quiet.state.grid_visible = s.state.grid_visible;
            quiet.overlay(view).len()
        };
        assert!(s.overlay(view).len() > without, "the heat tile is drawn");
    }

    #[test]
    fn overlay_items_draw_path_segments_as_lines() {
        let mut s = session();
        s.run_api("add_path", &json!({"points": [[8, 8], [40, 8], [40, 40]]}))
            .unwrap();
        let items = s.overlay_items(Rect::new(0.0, 0.0, 160.0, 128.0), false);
        let lines: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                OverlayItem::Line { a, b, .. } => Some((*a, *b)),
                OverlayItem::Rect(..) => None,
            })
            .collect();
        assert_eq!(
            lines,
            [((8.0, 8.0), (40.0, 8.0)), ((40.0, 8.0), (40.0, 40.0))]
        );
    }

    #[test]
    fn open_and_save_as_take_names_relative_to_the_levels_directory() {
        let dir = temp_dir();
        let mut s = EditorSession::new(AmigoLevel::new("t", 4, 4, 16), dir.join("t.amigo"));
        assert_eq!(
            s.resolve_level_path(Path::new("boss")),
            dir.join("boss.amigo")
        );
        assert_eq!(
            s.resolve_level_path(Path::new("/x/y.amigo")),
            PathBuf::from("/x/y.amigo")
        );
        s.level.layers[0].tiles[1] = 3;
        let saved = s.perform(EditorAction::SaveAs("boss".into())).unwrap();
        assert_eq!(saved, Some(dir.join("boss.amigo")));
        assert_eq!(s.path, dir.join("boss.amigo"));
        assert_eq!(s.level_files(), [dir.join("boss.amigo")]);

        let mut other = EditorSession::new(AmigoLevel::new("o", 2, 2, 16), dir.join("o.amigo"));
        other
            .perform(EditorAction::Open("boss.amigo".into()))
            .unwrap();
        assert_eq!(other.level.layers[0].tiles[1], 3);
        assert!(other.perform(EditorAction::Open("missing".into())).is_err());
        assert!(other.status.as_deref().unwrap().contains("missing"));
    }

    #[test]
    fn import_sprite_copies_images_into_assets_sprites() {
        let root = temp_dir();
        let levels = root.join("assets").join("levels");
        let s = EditorSession::new(AmigoLevel::new("t", 2, 2, 16), levels.join("t.amigo"));
        let png = root.join("hero.png");
        std::fs::write(&png, b"not really a png").unwrap();
        let to = s.import_sprite(&png).unwrap();
        assert_eq!(to, root.join("assets").join("sprites").join("hero.png"));
        assert_eq!(std::fs::read(&to).unwrap(), b"not really a png");

        let txt = root.join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();
        assert!(s.import_sprite(&txt).unwrap_err().contains("not a sprite"));
    }

    #[test]
    fn preview_colours_are_stable_and_distinct() {
        assert_eq!(tile_preview_color(3), tile_preview_color(3));
        assert_ne!(tile_preview_color(1), tile_preview_color(2));
    }
}
