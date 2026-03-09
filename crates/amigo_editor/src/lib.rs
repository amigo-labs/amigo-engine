/// Editor plugin (Phase 4, currently a stub).
/// Enabled via `--features editor`.
pub struct EditorPlugin {
    pub active: bool,
}

impl EditorPlugin {
    pub fn new() -> Self {
        Self { active: false }
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
    }
}

impl Default for EditorPlugin {
    fn default() -> Self {
        Self::new()
    }
}
