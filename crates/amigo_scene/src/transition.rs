use amigo_core::Color;

/// Types of visual transitions between scenes.
#[derive(Clone, Debug)]
pub enum TransitionKind {
    /// Simple fade to a color and back.
    Fade { color: Color },
    /// Slide the old scene out and new scene in from a direction.
    Slide { direction: SlideDirection },
    /// Wipe across the screen.
    Wipe { direction: SlideDirection },
    /// Instant cut (no animation).
    Cut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideDirection {
    Left,
    Right,
    Up,
    Down,
}

/// A scene transition animation controller.
#[derive(Clone, Debug)]
pub struct Transition {
    pub kind: TransitionKind,
    /// Total duration in seconds.
    pub duration: f32,
    /// Current elapsed time.
    elapsed: f32,
    /// Phase: FadeOut (0..0.5) or FadeIn (0.5..1.0)
    phase: TransitionPhase,
    /// Whether the transition has completed.
    finished: bool,
    /// Whether the scene swap has happened (at the midpoint).
    swapped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionPhase {
    Out,
    In,
}

impl Transition {
    pub fn new(kind: TransitionKind, duration: f32) -> Self {
        Self {
            kind,
            duration: duration.max(0.01),
            elapsed: 0.0,
            phase: TransitionPhase::Out,
            finished: false,
            swapped: false,
        }
    }

    pub fn fade(duration: f32) -> Self {
        Self::new(TransitionKind::Fade { color: Color::BLACK }, duration)
    }

    pub fn fade_white(duration: f32) -> Self {
        Self::new(TransitionKind::Fade { color: Color::WHITE }, duration)
    }

    pub fn slide(direction: SlideDirection, duration: f32) -> Self {
        Self::new(TransitionKind::Slide { direction }, duration)
    }

    pub fn wipe(direction: SlideDirection, duration: f32) -> Self {
        Self::new(TransitionKind::Wipe { direction }, duration)
    }

    pub fn cut() -> Self {
        Self::new(TransitionKind::Cut, 0.0)
    }

    /// Advance the transition. Returns true when the scene should be swapped
    /// (exactly once, at the midpoint).
    pub fn update(&mut self, dt: f32) -> bool {
        if self.finished {
            return false;
        }

        self.elapsed += dt;
        let progress = (self.elapsed / self.duration).clamp(0.0, 1.0);

        if matches!(self.kind, TransitionKind::Cut) {
            self.finished = true;
            self.swapped = true;
            return true;
        }

        if progress >= 0.5 && !self.swapped {
            self.phase = TransitionPhase::In;
            self.swapped = true;
            return true;
        }

        if progress >= 1.0 {
            self.finished = true;
        }

        false
    }

    /// Get the current visual overlay parameters for rendering.
    pub fn render_info(&self) -> TransitionRenderInfo {
        let progress = (self.elapsed / self.duration).clamp(0.0, 1.0);

        match &self.kind {
            TransitionKind::Fade { color } => {
                // Alpha: 0→1 (first half), 1→0 (second half)
                let alpha = if progress < 0.5 {
                    progress * 2.0
                } else {
                    (1.0 - progress) * 2.0
                };
                TransitionRenderInfo {
                    overlay_color: Some(Color::new(color.r, color.g, color.b, alpha)),
                    offset_x: 0.0,
                    offset_y: 0.0,
                    clip_fraction: 1.0,
                }
            }
            TransitionKind::Slide { direction } => {
                let t = if progress < 0.5 {
                    progress * 2.0
                } else {
                    (1.0 - progress) * 2.0
                };
                let (ox, oy) = match direction {
                    SlideDirection::Left => (-t, 0.0),
                    SlideDirection::Right => (t, 0.0),
                    SlideDirection::Up => (0.0, -t),
                    SlideDirection::Down => (0.0, t),
                };
                TransitionRenderInfo {
                    overlay_color: None,
                    offset_x: ox,
                    offset_y: oy,
                    clip_fraction: 1.0,
                }
            }
            TransitionKind::Wipe { direction: _ } => {
                let t = if progress < 0.5 {
                    progress * 2.0
                } else {
                    1.0 - (progress - 0.5) * 2.0
                };
                TransitionRenderInfo {
                    overlay_color: Some(Color::new(0.0, 0.0, 0.0, 1.0)),
                    offset_x: 0.0,
                    offset_y: 0.0,
                    clip_fraction: t,
                }
            }
            TransitionKind::Cut => TransitionRenderInfo {
                overlay_color: None,
                offset_x: 0.0,
                offset_y: 0.0,
                clip_fraction: 1.0,
            },
        }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn phase(&self) -> TransitionPhase {
        self.phase
    }

    pub fn progress(&self) -> f32 {
        (self.elapsed / self.duration).clamp(0.0, 1.0)
    }
}

/// Information needed by the renderer to draw the transition overlay.
#[derive(Clone, Debug)]
pub struct TransitionRenderInfo {
    /// Full-screen color overlay (if any).
    pub overlay_color: Option<Color>,
    /// Normalized offset for slide effect (-1.0 to 1.0).
    pub offset_x: f32,
    pub offset_y: f32,
    /// For wipe: fraction of screen that should be covered (0.0 to 1.0).
    pub clip_fraction: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_lifecycle() {
        let mut t = Transition::fade(1.0);
        assert!(!t.is_finished());

        // Advance to just before midpoint
        assert!(!t.update(0.4));
        assert_eq!(t.phase(), TransitionPhase::Out);

        // Cross midpoint → should trigger swap
        assert!(t.update(0.2));
        assert_eq!(t.phase(), TransitionPhase::In);

        // Advance to end
        assert!(!t.update(0.5));
        assert!(t.is_finished());
    }

    #[test]
    fn cut_is_instant() {
        let mut t = Transition::cut();
        assert!(t.update(0.0));
        assert!(t.is_finished());
    }

    #[test]
    fn fade_alpha_peaks_at_midpoint() {
        let mut t = Transition::fade(2.0);
        t.update(1.0); // exactly midpoint
        let info = t.render_info();
        let alpha = info.overlay_color.unwrap().a;
        assert!((alpha - 1.0).abs() < 0.01);
    }
}
