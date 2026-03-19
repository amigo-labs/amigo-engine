use crate::config::PostprocessConfig;

/// Post-process note events after conversion.
///
/// Operations (all pure Rust, no external tools):
/// - Remove ghost notes (amplitude below threshold)
/// - Normalize velocity
/// - Merge short rests between identical notes
pub struct PostProcessor {
    config: PostprocessConfig,
}

/// Simple note representation for post-processing.
#[derive(Debug, Clone)]
pub struct RawNote {
    pub pitch: u8,
    pub time_ms: u32,
    pub duration_ms: u32,
    pub velocity: f64,
}

impl PostProcessor {
    pub fn new(config: PostprocessConfig) -> Self {
        Self { config }
    }

    /// Apply all post-processing steps to a list of notes.
    pub fn process(&self, notes: &mut Vec<RawNote>) {
        if self.config.remove_ghost_notes {
            self.remove_ghost_notes(notes);
        }
        if self.config.normalize_velocity {
            self.normalize_velocity(notes);
        }
        if self.config.merge_short_rests {
            self.merge_short_rests(notes);
        }
    }

    /// Remove notes with velocity below the ghost note threshold.
    fn remove_ghost_notes(&self, notes: &mut Vec<RawNote>) {
        notes.retain(|n| n.velocity >= self.config.ghost_note_threshold);
    }

    /// Normalize all velocities to 0.0-1.0 range.
    fn normalize_velocity(&self, notes: &mut Vec<RawNote>) {
        let max_vel = notes
            .iter()
            .map(|n| n.velocity)
            .fold(0.0_f64, f64::max);
        if max_vel > 0.0 {
            for note in notes.iter_mut() {
                note.velocity /= max_vel;
            }
        }
    }

    /// Merge notes that are separated by rests shorter than the threshold.
    fn merge_short_rests(&self, notes: &mut Vec<RawNote>) {
        if notes.len() < 2 {
            return;
        }

        let threshold = self.config.min_rest_length_ms;
        let mut i = 0;
        while i + 1 < notes.len() {
            let end_of_current = notes[i].time_ms + notes[i].duration_ms;
            let start_of_next = notes[i + 1].time_ms;
            let gap = start_of_next.saturating_sub(end_of_current);

            if gap < threshold && notes[i].pitch == notes[i + 1].pitch {
                // Extend current note to cover the gap and next note.
                let new_end = notes[i + 1].time_ms + notes[i + 1].duration_ms;
                notes[i].duration_ms = new_end - notes[i].time_ms;
                notes.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PostprocessConfig;

    fn default_processor() -> PostProcessor {
        PostProcessor::new(PostprocessConfig::default())
    }

    #[test]
    fn removes_ghost_notes() {
        let mut notes = vec![
            RawNote { pitch: 60, time_ms: 0, duration_ms: 100, velocity: 0.8 },
            RawNote { pitch: 62, time_ms: 100, duration_ms: 100, velocity: 0.05 },
            RawNote { pitch: 64, time_ms: 200, duration_ms: 100, velocity: 0.9 },
        ];
        let proc = default_processor();
        proc.remove_ghost_notes(&mut notes);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].pitch, 60);
        assert_eq!(notes[1].pitch, 64);
    }

    #[test]
    fn normalizes_velocity() {
        let mut notes = vec![
            RawNote { pitch: 60, time_ms: 0, duration_ms: 100, velocity: 50.0 },
            RawNote { pitch: 62, time_ms: 100, duration_ms: 100, velocity: 100.0 },
        ];
        let proc = default_processor();
        proc.normalize_velocity(&mut notes);
        assert!((notes[0].velocity - 0.5).abs() < 1e-9);
        assert!((notes[1].velocity - 1.0).abs() < 1e-9);
    }

    #[test]
    fn merges_short_rests() {
        let mut notes = vec![
            RawNote { pitch: 60, time_ms: 0, duration_ms: 100, velocity: 0.8 },
            // 20ms gap (< 30ms threshold), same pitch -> merge.
            RawNote { pitch: 60, time_ms: 120, duration_ms: 100, velocity: 0.8 },
        ];
        let proc = default_processor();
        proc.merge_short_rests(&mut notes);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].duration_ms, 220);
    }

    #[test]
    fn does_not_merge_different_pitches() {
        let mut notes = vec![
            RawNote { pitch: 60, time_ms: 0, duration_ms: 100, velocity: 0.8 },
            RawNote { pitch: 62, time_ms: 110, duration_ms: 100, velocity: 0.8 },
        ];
        let proc = default_processor();
        proc.merge_short_rests(&mut notes);
        assert_eq!(notes.len(), 2);
    }
}
