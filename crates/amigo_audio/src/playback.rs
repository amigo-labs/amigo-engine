//! Playback control types (docs/specs/engine/audio-playback.md) and the pure
//! functions behind them: resolving a play range against a file, and
//! rendering a finite loop into one gapless buffer.

use kira::Frame;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Refers to one started sound. Copy, never dangling: once the sound ends,
/// every operation on it is a no-op and `state` reports `Stopped`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoundHandle {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl SoundHandle {
    /// A handle that never referred to a sound.
    pub const NONE: SoundHandle = SoundHandle {
        index: u32::MAX,
        generation: 0,
    };

    pub fn is_none(self) -> bool {
        self == Self::NONE
    }
}

/// The mixer bus a sound plays on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bus {
    Music,
    #[default]
    Sfx,
    Ambient,
}

impl Bus {
    pub(crate) const ALL: [Bus; 3] = [Bus::Music, Bus::Sfx, Bus::Ambient];

    pub(crate) fn index(self) -> usize {
        match self {
            Bus::Music => 0,
            Bus::Sfx => 1,
            Bus::Ambient => 2,
        }
    }
}

/// A point in a sound file. `Samples` counts sample frames at the file's own
/// sample rate, so a value read from the file's metadata can be used as is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Position {
    Seconds(f64),
    Samples(u64),
}

impl Default for Position {
    /// `Seconds(0.0)`.
    fn default() -> Self {
        Position::Seconds(0.0)
    }
}

impl Position {
    /// Sample frames at `sample_rate`: `round(s · sample_rate)`, negative and
    /// non-finite seconds as 0.
    pub(crate) fn frames(self, sample_rate: u32) -> u64 {
        match self {
            Position::Samples(n) => n,
            Position::Seconds(s) if s.is_finite() && s > 0.0 => {
                (s * sample_rate as f64).round() as u64
            }
            Position::Seconds(_) => 0,
        }
    }
}

/// How often the played range repeats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    /// Play the range once.
    #[default]
    Once,
    /// Play `n` times in total, back to back, without a gap. `Count(0)` and
    /// `Count(1)` are the same as `Once`.
    Count(u32),
    /// Until stopped.
    Forever,
}

/// A linear volume ramp. `Fade::NONE` changes immediately.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fade {
    pub seconds: f32,
}

impl Fade {
    pub const NONE: Fade = Fade { seconds: 0.0 };

    pub fn secs(seconds: f32) -> Fade {
        Fade { seconds }
    }

    pub(crate) fn tween(self) -> kira::Tween {
        let seconds = if self.seconds.is_finite() {
            self.seconds.max(0.0)
        } else {
            0.0
        };
        kira::Tween {
            duration: std::time::Duration::from_secs_f32(seconds),
            ..Default::default()
        }
    }
}

/// Where a started sound is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundState {
    /// Started with a delay that has not elapsed yet.
    Scheduled,
    Playing,
    /// Paused by `pause_all`.
    Paused,
    /// Finished, stopped, never started, or the handle is `NONE`.
    Stopped,
}

/// How to start a sound.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaySettings {
    pub bus: Bus,
    /// Linear amplitude; 1.0 plays the file as recorded. Clamped to 0.0..=4.0.
    pub volume: f32,
    /// Seconds from the `play` call until the sound starts. `<= 0` is now.
    pub delay: f64,
    /// Where in the file playback starts.
    pub start: Position,
    /// How much to play from `start`. `None` plays to the end of the file.
    pub length: Option<Position>,
    /// The part that `loops` repeats, in file positions. `None` repeats the
    /// whole played range `start..start + length`.
    pub loop_region: Option<(Position, Position)>,
    pub loops: LoopMode,
    pub fade_in: Fade,
    /// 1.0 is normal speed and pitch. Clamped to 0.01..=8.0.
    pub playback_rate: f64,
    /// -1.0 is left, 0.0 centre, 1.0 right. Clamped.
    pub panning: f32,
}

impl Default for PlaySettings {
    /// Sfx bus, volume 1.0, no delay, from the start to the end, once, no
    /// fade, rate 1.0, centred.
    fn default() -> Self {
        Self {
            bus: Bus::Sfx,
            volume: 1.0,
            delay: 0.0,
            start: Position::default(),
            length: None,
            loop_region: None,
            loops: LoopMode::Once,
            fade_in: Fade::NONE,
            playback_rate: 1.0,
            panning: 0.0,
        }
    }
}

impl PlaySettings {
    /// `PlaySettings { bus, ..Default::default() }`.
    pub fn on(bus: Bus) -> Self {
        Self {
            bus,
            ..Default::default()
        }
    }
}

/// How the resolved range repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Repeat {
    Once,
    Count(u32),
    Forever,
}

/// A play range in sample frames of one variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ResolvedRange {
    /// Played range `start..end`, `start < end <= frames`.
    pub start: usize,
    pub end: usize,
    /// The region that repeats, within the played range; `None` plays
    /// straight through.
    pub region: Option<(usize, usize)>,
    pub repeat: Repeat,
    /// A loop was asked for but its region was empty after clamping.
    pub empty_region: bool,
}

/// Resolve `settings`' positions against a variant of `frames` sample frames
/// at `sample_rate`, clamped to the file. `None` when the played range is
/// empty.
pub(crate) fn resolve_range(
    settings: &PlaySettings,
    sample_rate: u32,
    frames: usize,
) -> Option<ResolvedRange> {
    let len = frames as u64;
    let start = settings.start.frames(sample_rate).min(len);
    let end = match settings.length {
        Some(length) => start.saturating_add(length.frames(sample_rate)).min(len),
        None => len,
    };
    if start >= end {
        return None;
    }
    let repeat = match settings.loops {
        LoopMode::Once | LoopMode::Count(0) | LoopMode::Count(1) => Repeat::Once,
        LoopMode::Count(n) => Repeat::Count(n),
        LoopMode::Forever => Repeat::Forever,
    };
    let (region, repeat, empty_region) = match repeat {
        Repeat::Once => (None, Repeat::Once, false),
        _ => {
            let (rs, re) = match settings.loop_region {
                Some((a, b)) => (
                    a.frames(sample_rate).clamp(start, end),
                    b.frames(sample_rate).clamp(start, end),
                ),
                None => (start, end),
            };
            if rs < re {
                (Some((rs as usize, re as usize)), repeat, false)
            } else {
                (None, Repeat::Once, true)
            }
        }
    };
    Some(ResolvedRange {
        start: start as usize,
        end: end as usize,
        region,
        repeat,
        empty_region,
    })
}

/// The lead-in `start..region_start`, `n` copies of the region, then the
/// tail `region_end..end`, as one buffer: a finite loop that is gapless,
/// sample-exact, and always plays its tail.
pub(crate) fn render_finite_loop(frames: &[Frame], range: ResolvedRange, n: u32) -> Arc<[Frame]> {
    let (rs, re) = range.region.unwrap_or((range.start, range.end));
    let region = &frames[rs..re];
    let mut out =
        Vec::with_capacity((rs - range.start) + region.len() * n as usize + (range.end - re));
    out.extend_from_slice(&frames[range.start..rs]);
    for _ in 0..n {
        out.extend_from_slice(region);
    }
    out.extend_from_slice(&frames[re..range.end]);
    out.into()
}

/// Length in frames [`render_finite_loop`] would produce.
pub(crate) fn finite_loop_len(range: ResolvedRange, n: u32) -> u64 {
    let (rs, re) = range.region.unwrap_or((range.start, range.end));
    (rs - range.start) as u64 + (re - rs) as u64 * n as u64 + (range.end - re) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(start: Position, length: Option<Position>) -> PlaySettings {
        PlaySettings {
            start,
            length,
            ..Default::default()
        }
    }

    #[test]
    fn positions_resolve_with_the_sample_rate() {
        let r = resolve_range(
            &settings(Position::Seconds(0.5), Some(Position::Samples(10))),
            100,
            1000,
        )
        .expect("non-empty");
        assert_eq!((r.start, r.end), (50, 60));
        let r = resolve_range(&settings(Position::Samples(3), None), 100, 10).expect("non-empty");
        assert_eq!((r.start, r.end), (3, 10));
    }

    #[test]
    fn a_start_past_the_end_is_empty_and_a_long_length_is_clamped() {
        assert!(resolve_range(&settings(Position::Seconds(20.0), None), 100, 1000).is_none());
        assert!(resolve_range(&settings(Position::Samples(1000), None), 100, 1000).is_none());
        let r = resolve_range(
            &settings(Position::Samples(900), Some(Position::Seconds(9.0))),
            100,
            1000,
        )
        .expect("non-empty");
        assert_eq!((r.start, r.end), (900, 1000));
    }

    #[test]
    fn an_empty_loop_region_resolves_to_no_loop() {
        let s = PlaySettings {
            loops: LoopMode::Forever,
            loop_region: Some((Position::Samples(5), Position::Samples(5))),
            ..Default::default()
        };
        let r = resolve_range(&s, 100, 10).expect("non-empty");
        assert_eq!(r.region, None);
        assert_eq!(r.repeat, Repeat::Once);
        assert!(r.empty_region);
    }

    #[test]
    fn count_zero_and_one_are_once() {
        for loops in [LoopMode::Count(0), LoopMode::Count(1), LoopMode::Once] {
            let s = PlaySettings {
                loops,
                ..Default::default()
            };
            let r = resolve_range(&s, 100, 10).expect("non-empty");
            assert_eq!((r.repeat, r.region), (Repeat::Once, None));
        }
    }

    fn ramp(n: usize) -> Vec<Frame> {
        (0..n)
            .map(|i| Frame {
                left: i as f32,
                right: -(i as f32),
            })
            .collect()
    }

    #[test]
    fn a_region_count_plays_lead_in_repeats_and_tail() {
        // 1 frame per second: played 0-6 s, region 2-4 s, Count(2).
        let frames = ramp(6);
        let s = PlaySettings {
            loops: LoopMode::Count(2),
            loop_region: Some((Position::Seconds(2.0), Position::Seconds(4.0))),
            ..Default::default()
        };
        let range = resolve_range(&s, 1, frames.len()).expect("non-empty");
        let out = render_finite_loop(&frames, range, 2);
        let lefts: Vec<f32> = out.iter().map(|f| f.left).collect();
        assert_eq!(lefts, vec![0.0, 1.0, 2.0, 3.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(finite_loop_len(range, 2), 8);
    }

    #[test]
    fn a_count_without_region_repeats_the_whole_range() {
        let frames = ramp(4);
        let s = PlaySettings {
            loops: LoopMode::Count(3),
            start: Position::Samples(1),
            ..Default::default()
        };
        let range = resolve_range(&s, 1, frames.len()).expect("non-empty");
        let out = render_finite_loop(&frames, range, 3);
        let lefts: Vec<f32> = out.iter().map(|f| f.left).collect();
        assert_eq!(lefts, vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]);
    }
}
