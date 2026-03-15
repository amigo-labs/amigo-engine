// ---------------------------------------------------------------------------
// Edit-While-Playing state machine
// ---------------------------------------------------------------------------

/// The editor/play mode state machine.
///
/// Allows toggling between editing a level and play-testing it in real time.
/// When entering play mode, the editor state is snapshot so it can be
/// restored when returning to edit mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayState {
    /// The editor is active, game simulation paused.
    Editing,
    /// Game is running as a play-test.
    Playing,
    /// Game is paused during a play-test (can inspect state).
    Paused,
}

impl Default for PlayState {
    fn default() -> Self {
        Self::Editing
    }
}

/// Manages the edit/play lifecycle including state snapshots.
pub struct PlayModeManager {
    state: PlayState,
    /// Serialized snapshot of the level when play was started.
    /// Used to restore the level when returning to edit mode.
    level_snapshot: Option<Vec<u8>>,
    /// Number of ticks elapsed during the current play session.
    play_ticks: u64,
}

impl PlayModeManager {
    pub fn new() -> Self {
        Self {
            state: PlayState::Editing,
            level_snapshot: None,
            play_ticks: 0,
        }
    }

    /// Current state.
    pub fn state(&self) -> &PlayState {
        &self.state
    }

    pub fn is_editing(&self) -> bool {
        self.state == PlayState::Editing
    }

    pub fn is_playing(&self) -> bool {
        self.state == PlayState::Playing
    }

    pub fn is_paused(&self) -> bool {
        self.state == PlayState::Paused
    }

    /// Ticks elapsed since play was started.
    pub fn play_ticks(&self) -> u64 {
        self.play_ticks
    }

    /// Transition: Editing -> Playing.
    ///
    /// Stores a RON snapshot of the level data so it can be restored later.
    /// Returns `true` if the transition succeeded.
    pub fn start_play(&mut self, level_ron: Vec<u8>) -> bool {
        if self.state != PlayState::Editing {
            return false;
        }
        self.level_snapshot = Some(level_ron);
        self.play_ticks = 0;
        self.state = PlayState::Playing;
        true
    }

    /// Transition: Playing -> Paused.
    pub fn pause(&mut self) -> bool {
        if self.state != PlayState::Playing {
            return false;
        }
        self.state = PlayState::Paused;
        true
    }

    /// Transition: Paused -> Playing.
    pub fn resume(&mut self) -> bool {
        if self.state != PlayState::Paused {
            return false;
        }
        self.state = PlayState::Playing;
        true
    }

    /// Transition: Playing/Paused -> Editing.
    ///
    /// Returns the stored level snapshot so the caller can restore the level
    /// to its pre-play state. Returns `None` if already editing.
    pub fn stop_play(&mut self) -> Option<Vec<u8>> {
        match self.state {
            PlayState::Playing | PlayState::Paused => {
                self.state = PlayState::Editing;
                self.play_ticks = 0;
                self.level_snapshot.take()
            }
            PlayState::Editing => None,
        }
    }

    /// Toggle between the states:
    /// - Editing -> Playing (needs level data)
    /// - Playing -> Editing (restores snapshot)
    /// - Paused  -> Editing (restores snapshot)
    ///
    /// For Editing -> Playing, pass `Some(level_ron)`.
    /// Returns any restored level snapshot.
    pub fn toggle(&mut self, level_ron: Option<Vec<u8>>) -> Option<Vec<u8>> {
        match self.state {
            PlayState::Editing => {
                if let Some(data) = level_ron {
                    self.start_play(data);
                }
                None
            }
            PlayState::Playing | PlayState::Paused => self.stop_play(),
        }
    }

    /// Call once per simulation tick while playing.
    pub fn tick(&mut self) {
        if self.state == PlayState::Playing {
            self.play_ticks += 1;
        }
    }
}

impl Default for PlayModeManager {
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

    #[test]
    fn initial_state_is_editing() {
        let mgr = PlayModeManager::new();
        assert!(mgr.is_editing());
        assert!(!mgr.is_playing());
        assert!(!mgr.is_paused());
    }

    #[test]
    fn editing_to_playing() {
        let mut mgr = PlayModeManager::new();
        let snapshot = b"level data".to_vec();
        assert!(mgr.start_play(snapshot));
        assert!(mgr.is_playing());
        assert_eq!(mgr.play_ticks(), 0);
    }

    #[test]
    fn playing_to_paused_and_back() {
        let mut mgr = PlayModeManager::new();
        mgr.start_play(b"data".to_vec());

        assert!(mgr.pause());
        assert!(mgr.is_paused());

        assert!(mgr.resume());
        assert!(mgr.is_playing());
    }

    #[test]
    fn stop_play_restores_snapshot() {
        let mut mgr = PlayModeManager::new();
        let snapshot = b"original level".to_vec();
        mgr.start_play(snapshot.clone());

        mgr.tick();
        mgr.tick();
        assert_eq!(mgr.play_ticks(), 2);

        let restored = mgr.stop_play().unwrap();
        assert_eq!(restored, snapshot);
        assert!(mgr.is_editing());
        assert_eq!(mgr.play_ticks(), 0);
    }

    #[test]
    fn stop_from_paused() {
        let mut mgr = PlayModeManager::new();
        mgr.start_play(b"data".to_vec());
        mgr.pause();

        let restored = mgr.stop_play();
        assert!(restored.is_some());
        assert!(mgr.is_editing());
    }

    #[test]
    fn invalid_transitions() {
        let mut mgr = PlayModeManager::new();
        // Can't pause while editing
        assert!(!mgr.pause());
        // Can't resume while editing
        assert!(!mgr.resume());
        // Can't stop while editing
        assert!(mgr.stop_play().is_none());

        mgr.start_play(b"data".to_vec());
        // Can't start play while already playing
        assert!(!mgr.start_play(b"other".to_vec()));
    }

    #[test]
    fn toggle_roundtrip() {
        let mut mgr = PlayModeManager::new();
        let data = b"level".to_vec();

        // Edit -> Play
        let result = mgr.toggle(Some(data.clone()));
        assert!(result.is_none());
        assert!(mgr.is_playing());

        // Play -> Edit (restores)
        let result = mgr.toggle(None);
        assert_eq!(result.unwrap(), data);
        assert!(mgr.is_editing());
    }

    #[test]
    fn tick_only_counts_while_playing() {
        let mut mgr = PlayModeManager::new();
        mgr.tick(); // editing — should not count
        assert_eq!(mgr.play_ticks(), 0);

        mgr.start_play(b"data".to_vec());
        mgr.tick();
        mgr.tick();
        assert_eq!(mgr.play_ticks(), 2);

        mgr.pause();
        mgr.tick(); // paused — should not count
        assert_eq!(mgr.play_ticks(), 2);

        mgr.resume();
        mgr.tick();
        assert_eq!(mgr.play_ticks(), 3);
    }
}
