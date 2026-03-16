//! Wave Editor — visual editing of tower-defense wave definitions.
//!
//! Feature-gated behind `td`. Provides data structures and UI for
//! designing waves (enemy types, counts, timing, spawn points).

use amigo_core::waves::{SpawnGroup, WaveDef};
use amigo_core::{Color, Rect};
use amigo_input::InputState;
use amigo_ui::UiContext;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Wave editor commands (undo / redo)
// ---------------------------------------------------------------------------

/// Reversible commands for the wave editor.
#[derive(Clone, Debug)]
pub enum WaveCommand {
    AddWave {
        index: usize,
    },
    RemoveWave {
        index: usize,
        wave: WaveDef,
    },
    AddGroup {
        wave_index: usize,
        group: SpawnGroup,
    },
    RemoveGroup {
        wave_index: usize,
        group_index: usize,
        group: SpawnGroup,
    },
    EditGroup {
        wave_index: usize,
        group_index: usize,
        old: SpawnGroup,
        new: SpawnGroup,
    },
    EditDelay {
        wave_index: usize,
        old_delay: f32,
        new_delay: f32,
    },
    MoveWave {
        from: usize,
        to: usize,
    },
}

impl WaveCommand {
    pub fn inverse(&self) -> Self {
        match self {
            WaveCommand::AddWave { index } => WaveCommand::RemoveWave {
                index: *index,
                wave: WaveDef::new(), // placeholder, caller patches
            },
            WaveCommand::RemoveWave { index, wave: _ } => WaveCommand::AddWave { index: *index },
            WaveCommand::AddGroup { wave_index, group } => WaveCommand::RemoveGroup {
                wave_index: *wave_index,
                group_index: 0, // patched by caller
                group: group.clone(),
            },
            WaveCommand::RemoveGroup {
                wave_index,
                group_index: _,
                group,
            } => WaveCommand::AddGroup {
                wave_index: *wave_index,
                group: group.clone(),
            },
            WaveCommand::EditGroup {
                wave_index,
                group_index,
                old,
                new,
            } => WaveCommand::EditGroup {
                wave_index: *wave_index,
                group_index: *group_index,
                old: new.clone(),
                new: old.clone(),
            },
            WaveCommand::EditDelay {
                wave_index,
                old_delay,
                new_delay,
            } => WaveCommand::EditDelay {
                wave_index: *wave_index,
                old_delay: *new_delay,
                new_delay: *old_delay,
            },
            WaveCommand::MoveWave { from, to } => WaveCommand::MoveWave {
                from: *to,
                to: *from,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Wave editor state
// ---------------------------------------------------------------------------

/// Runtime state of the wave editor panel.
pub struct WaveEditorState {
    pub waves: Vec<WaveDef>,
    pub selected_wave: Option<usize>,
    pub selected_group: Option<usize>,
    pub undo_stack: Vec<WaveCommand>,
    pub redo_stack: Vec<WaveCommand>,
    pub scroll_offset: f32,
}

impl WaveEditorState {
    pub fn new() -> Self {
        Self {
            waves: Vec::new(),
            selected_wave: None,
            selected_group: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            scroll_offset: 0.0,
        }
    }

    pub fn from_waves(waves: Vec<WaveDef>) -> Self {
        Self {
            waves,
            selected_wave: None,
            selected_group: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            scroll_offset: 0.0,
        }
    }

    /// Execute a command, apply it, and push onto undo stack.
    pub fn execute(&mut self, cmd: WaveCommand) {
        self.apply(&cmd);
        self.redo_stack.clear();
        self.undo_stack.push(cmd);
    }

    /// Undo the last command.
    pub fn undo(&mut self) -> bool {
        if let Some(cmd) = self.undo_stack.pop() {
            let inv = cmd.inverse();
            self.apply(&inv);
            self.redo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    /// Redo the last undone command.
    pub fn redo(&mut self) -> bool {
        if let Some(cmd) = self.redo_stack.pop() {
            self.apply(&cmd);
            self.undo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    fn apply(&mut self, cmd: &WaveCommand) {
        match cmd {
            WaveCommand::AddWave { index } => {
                let wave = WaveDef::new();
                if *index >= self.waves.len() {
                    self.waves.push(wave);
                } else {
                    self.waves.insert(*index, wave);
                }
            }
            WaveCommand::RemoveWave { index, .. } => {
                if *index < self.waves.len() {
                    self.waves.remove(*index);
                    // Fix selection
                    if let Some(sel) = self.selected_wave {
                        if sel >= self.waves.len() {
                            self.selected_wave = self.waves.len().checked_sub(1);
                        }
                    }
                }
            }
            WaveCommand::AddGroup { wave_index, group } => {
                if let Some(wave) = self.waves.get_mut(*wave_index) {
                    wave.groups.push(group.clone());
                }
            }
            WaveCommand::RemoveGroup {
                wave_index,
                group_index,
                ..
            } => {
                if let Some(wave) = self.waves.get_mut(*wave_index) {
                    if *group_index < wave.groups.len() {
                        wave.groups.remove(*group_index);
                    }
                }
            }
            WaveCommand::EditGroup {
                wave_index,
                group_index,
                new,
                ..
            } => {
                if let Some(wave) = self.waves.get_mut(*wave_index) {
                    if let Some(group) = wave.groups.get_mut(*group_index) {
                        *group = new.clone();
                    }
                }
            }
            WaveCommand::EditDelay {
                wave_index,
                new_delay,
                ..
            } => {
                if let Some(wave) = self.waves.get_mut(*wave_index) {
                    wave.start_delay = *new_delay;
                }
            }
            WaveCommand::MoveWave { from, to } => {
                if *from < self.waves.len() && *to < self.waves.len() {
                    let wave = self.waves.remove(*from);
                    self.waves.insert(*to, wave);
                    self.selected_wave = Some(*to);
                }
            }
        }
    }

    /// Add a new empty wave at the end.
    pub fn add_wave(&mut self) {
        let index = self.waves.len();
        self.execute(WaveCommand::AddWave { index });
        self.selected_wave = Some(index);
    }

    /// Remove the selected wave.
    pub fn remove_selected_wave(&mut self) {
        if let Some(idx) = self.selected_wave {
            if idx < self.waves.len() {
                let wave = self.waves[idx].clone();
                self.execute(WaveCommand::RemoveWave { index: idx, wave });
            }
        }
    }

    /// Add a spawn group to the selected wave.
    pub fn add_group_to_selected(&mut self, enemy_type: u32) {
        if let Some(wi) = self.selected_wave {
            let group = SpawnGroup {
                enemy_type,
                count: 5,
                spawn_interval: 0.5,
                spawn_point: 0,
            };
            self.execute(WaveCommand::AddGroup {
                wave_index: wi,
                group,
            });
        }
    }

    /// Total enemies across all waves.
    pub fn total_enemies(&self) -> u32 {
        self.waves.iter().map(|w| w.total_enemies()).sum()
    }
}

impl Default for WaveEditorState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Wave editor UI
// ---------------------------------------------------------------------------

const WAVE_PANEL_BG: Color = Color {
    r: 0.12,
    g: 0.12,
    b: 0.15,
    a: 0.95,
};
const WAVE_SELECTED: Color = Color {
    r: 0.25,
    g: 0.4,
    b: 0.65,
    a: 1.0,
};
const WAVE_ITEM: Color = Color {
    r: 0.2,
    g: 0.2,
    b: 0.25,
    a: 1.0,
};
const HEADER: Color = Color {
    r: 0.8,
    g: 0.9,
    b: 1.0,
    a: 1.0,
};

/// Draw the wave editor panel. Call from the editor UI.
pub fn draw_wave_editor(
    ui: &mut UiContext,
    state: &mut WaveEditorState,
    input: &InputState,
    x: f32,
    y: f32,
    panel_w: f32,
    panel_h: f32,
) {
    // Background
    ui.panel(Rect::new(x, y, panel_w, panel_h), WAVE_PANEL_BG);

    let mut cy = y + 4.0;

    // Header
    ui.pixel_text("Wave Editor", x + 4.0, cy, HEADER);
    cy += 14.0;
    ui.separator(x + 2.0, cy, panel_w - 4.0);
    cy += 4.0;

    // Summary
    let summary = format!(
        "{} waves, {} total enemies",
        state.waves.len(),
        state.total_enemies()
    );
    ui.pixel_text(&summary, x + 4.0, cy, Color::new(0.7, 0.7, 0.7, 1.0));
    cy += 14.0;

    // Add wave button
    if ui.text_button("+ Wave", x + 4.0, cy, input) {
        state.add_wave();
    }
    cy += 20.0;

    // Wave list
    let list_h = (panel_h - (cy - y) - 4.0).max(0.0);
    let item_h = 18.0;

    for (i, wave) in state.waves.iter().enumerate() {
        let iy = cy + i as f32 * item_h - state.scroll_offset;
        if iy < cy - item_h || iy > cy + list_h {
            continue;
        }

        let is_selected = state.selected_wave == Some(i);
        let bg = if is_selected {
            WAVE_SELECTED
        } else {
            WAVE_ITEM
        };
        let item_rect = Rect::new(x + 4.0, iy, panel_w - 8.0, item_h - 2.0);
        ui.filled_rect(item_rect, bg);

        let label = format!(
            "W{}: {} grp, {} enemies, {:.1}s",
            i + 1,
            wave.groups.len(),
            wave.total_enemies(),
            wave.start_delay,
        );
        ui.pixel_text(&label, x + 8.0, iy + 3.0, Color::WHITE);

        // Click to select
        let mouse = input.mouse_pos();
        if item_rect.contains(mouse.x, mouse.y)
            && input.mouse_pressed(winit::event::MouseButton::Left)
        {
            state.selected_wave = Some(i);
            state.selected_group = None;
        }
    }

    // Handle scroll
    let scroll = input.scroll_delta();
    if scroll != 0.0 {
        let mouse = input.mouse_pos();
        let list_rect = Rect::new(x, cy, panel_w, list_h);
        if list_rect.contains(mouse.x, mouse.y) {
            state.scroll_offset = (state.scroll_offset - scroll * 18.0).max(0.0);
        }
    }

    // Keyboard shortcuts
    handle_wave_shortcuts(state, input);
}

fn handle_wave_shortcuts(state: &mut WaveEditorState, input: &InputState) {
    use winit::keyboard::KeyCode;

    // Ctrl+Z/Ctrl+Shift+Z for undo/redo
    if input.held(KeyCode::ControlLeft) && input.pressed(KeyCode::KeyZ) {
        if input.held(KeyCode::ShiftLeft) {
            state.redo();
        } else {
            state.undo();
        }
    }

    // Delete to remove selected wave
    if input.pressed(KeyCode::Delete) {
        state.remove_selected_wave();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_wave() {
        let mut state = WaveEditorState::new();
        state.add_wave();
        assert_eq!(state.waves.len(), 1);
        assert_eq!(state.selected_wave, Some(0));

        state.add_wave();
        assert_eq!(state.waves.len(), 2);

        state.selected_wave = Some(0);
        state.remove_selected_wave();
        assert_eq!(state.waves.len(), 1);
    }

    #[test]
    fn undo_redo_add_wave() {
        let mut state = WaveEditorState::new();
        state.add_wave();
        state.add_wave();
        assert_eq!(state.waves.len(), 2);

        state.undo();
        assert_eq!(state.waves.len(), 1);

        state.redo();
        assert_eq!(state.waves.len(), 2);
    }

    #[test]
    fn add_group() {
        let mut state = WaveEditorState::new();
        state.add_wave();
        state.selected_wave = Some(0);
        state.add_group_to_selected(1);

        assert_eq!(state.waves[0].groups.len(), 1);
        assert_eq!(state.waves[0].groups[0].enemy_type, 1);
        assert_eq!(state.waves[0].groups[0].count, 5);
    }

    #[test]
    fn total_enemies() {
        let mut state = WaveEditorState::new();
        state.add_wave();
        state.selected_wave = Some(0);
        state.add_group_to_selected(1); // 5 enemies
        state.add_group_to_selected(2); // 5 enemies

        assert_eq!(state.total_enemies(), 10);
    }

    #[test]
    fn from_existing_waves() {
        let waves = vec![
            WaveDef::new().with_delay(2.0).with_group(1, 10, 0.5, 0),
            WaveDef::new()
                .with_delay(5.0)
                .with_group(2, 20, 0.3, 0)
                .with_group(3, 5, 1.0, 1),
        ];
        let state = WaveEditorState::from_waves(waves);
        assert_eq!(state.waves.len(), 2);
        assert_eq!(state.total_enemies(), 35);
    }

    #[test]
    fn move_wave() {
        let mut state = WaveEditorState::new();
        state.add_wave(); // Wave 0
        state.selected_wave = Some(0);
        state.add_group_to_selected(1);
        state.add_wave(); // Wave 1
        state.selected_wave = Some(1);
        state.add_group_to_selected(2);

        assert_eq!(state.waves[0].groups[0].enemy_type, 1);
        assert_eq!(state.waves[1].groups[0].enemy_type, 2);

        state.execute(WaveCommand::MoveWave { from: 0, to: 1 });
        assert_eq!(state.waves[0].groups[0].enemy_type, 2);
        assert_eq!(state.waves[1].groups[0].enemy_type, 1);
    }
}
