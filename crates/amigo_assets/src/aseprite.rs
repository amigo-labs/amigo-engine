use crate::AssetError;
use amigo_animation::{AnimFrame, Animation};
use amigo_core::Rect;
use asefile::{AnimationDirection, AsepriteFile};
use image::RgbaImage;
use std::path::Path;
use tracing::{info, warn};

/// The most times a tag's `repeat` count is unrolled into frames. Aseprite
/// allows up to 65535; a sprite does not need that many copies of a frame.
const MAX_REPEAT: usize = 32;

/// Data extracted from an Aseprite file.
pub struct AsepriteData {
    /// Name derived from filename.
    pub name: String,
    /// Composited frames as RGBA images.
    pub frames: Vec<RgbaImage>,
    /// Frame durations in milliseconds.
    pub frame_durations_ms: Vec<u32>,
    /// Named animations from Aseprite tags, `"{name}/{tag}"`.
    ///
    /// Each frame's `uv` is its normalised rect in [`AsepriteData::strip`]:
    /// frame `i` of `n` is `(i/n, 0, 1/n, 1)`. A tag's direction and repeat
    /// count are already applied to the frame order: a reverse tag lists its
    /// frames backwards, a ping-pong tag lists the way back too, and a tag
    /// with a repeat count lists its frames that many times and does not loop.
    pub animations: Vec<Animation>,
    /// Width of each frame.
    pub width: u32,
    /// Height of each frame.
    pub height: u32,
}

impl AsepriteData {
    /// All frames side by side, left to right: the image the animation UVs
    /// point into.
    pub fn strip(&self) -> RgbaImage {
        let mut strip = RgbaImage::new(self.width * self.frames.len().max(1) as u32, self.height);
        for (i, frame) in self.frames.iter().enumerate() {
            image::imageops::replace(&mut strip, frame, i as i64 * self.width as i64, 0);
        }
        strip
    }
}

/// Load an Aseprite file and extract frames + animations.
pub fn load_aseprite(path: &Path) -> Result<AsepriteData, AssetError> {
    let ase = AsepriteFile::read_file(path).map_err(|e| AssetError::LoadFailed {
        path: path.display().to_string(),
        reason: format!("Aseprite parse error: {}", e),
    })?;

    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let width = ase.width() as u32;
    let height = ase.height() as u32;
    let num_frames = ase.num_frames() as usize;

    info!(
        "Loading Aseprite: {} ({}x{}, {} frames)",
        name, width, height, num_frames
    );

    // Composite each frame (all layers merged)
    let mut frames = Vec::with_capacity(num_frames);
    let mut frame_durations_ms = Vec::with_capacity(num_frames);

    for frame_idx in 0..num_frames as u32 {
        let frame = ase.frame(frame_idx);
        let img = frame.image();
        let rgba = RgbaImage::from_raw(width, height, img.into_raw()).unwrap_or_else(|| {
            warn!("Failed to create frame image for {}:{}", name, frame_idx);
            RgbaImage::new(width, height)
        });
        frames.push(rgba);
        frame_durations_ms.push(frame.duration());
    }

    // Extract animations from tags
    let mut animations = Vec::new();

    for tag_idx in 0..ase.num_tags() {
        let tag = ase.tag(tag_idx);
        let range = tag_frames(
            tag.from_frame() as usize,
            tag.to_frame() as usize,
            num_frames,
        );
        let Some(range) = range else {
            warn!(
                "{name}: tag '{}' covers frames {}..={} but the file has {num_frames}; skipping it",
                tag.name(),
                tag.from_frame(),
                tag.to_frame()
            );
            continue;
        };
        if *range.end() != tag.to_frame() as usize {
            warn!(
                "{name}: tag '{}' ends at frame {} but the file has {num_frames}; clamping",
                tag.name(),
                tag.to_frame()
            );
        }
        let (order, looping) = frame_order(
            range,
            tag.animation_direction(),
            tag.repeat().map(|n| n.get() as usize),
        );
        animations.push(build_animation(
            format!("{}/{}", name, tag.name()),
            &order,
            &frame_durations_ms,
            num_frames,
            looping,
        ));
    }

    // If no tags, create a default animation with all frames
    if animations.is_empty() && num_frames > 1 {
        let order: Vec<usize> = (0..num_frames).collect();
        animations.push(build_animation(
            format!("{}/default", name),
            &order,
            &frame_durations_ms,
            num_frames,
            true,
        ));
    }

    Ok(AsepriteData {
        name,
        frames,
        frame_durations_ms,
        animations,
        width,
        height,
    })
}

/// The frames a tag covers, clamped to the file. `None` when the tag starts
/// past the last frame or ends before it starts.
fn tag_frames(
    from: usize,
    to: usize,
    num_frames: usize,
) -> Option<std::ops::RangeInclusive<usize>> {
    let last = num_frames.checked_sub(1)?;
    if from > last || from > to {
        return None;
    }
    Some(from..=to.min(last))
}

/// The order frames play in, and whether the result loops.
///
/// Aseprite plays a reverse tag from its last frame down, a ping-pong tag up
/// and back down (without repeating either end), and a tag with a repeat
/// count that many times before stopping.
fn frame_order(
    range: std::ops::RangeInclusive<usize>,
    direction: AnimationDirection,
    repeat: Option<usize>,
) -> (Vec<usize>, bool) {
    let forward: Vec<usize> = range.collect();
    let cycle: Vec<usize> = match direction {
        AnimationDirection::Forward => forward,
        AnimationDirection::Reverse => forward.into_iter().rev().collect(),
        AnimationDirection::PingPong => {
            let back = forward
                .iter()
                .rev()
                .skip(1)
                .take(forward.len().saturating_sub(2))
                .copied()
                .collect::<Vec<_>>();
            forward.into_iter().chain(back).collect()
        }
    };
    match repeat {
        None => (cycle, true),
        Some(times) => {
            let times = times.clamp(1, MAX_REPEAT);
            let len = cycle.len();
            (cycle.into_iter().cycle().take(len * times).collect(), false)
        }
    }
}

fn build_animation(
    name: String,
    order: &[usize],
    durations_ms: &[u32],
    num_frames: usize,
    looping: bool,
) -> Animation {
    let frame_w = 1.0 / num_frames.max(1) as f32;
    let frames = order
        .iter()
        .map(|&i| {
            let duration_ms = durations_ms.get(i).copied().unwrap_or(100);
            // Ticks at the fixed 60 Hz step, at least one.
            let duration = (duration_ms as f32 / 16.667).round().max(1.0) as u32;
            AnimFrame {
                uv: Rect::new(i as f32 * frame_w, 0.0, frame_w, 1.0),
                duration,
            }
        })
        .collect();
    Animation {
        name,
        frames,
        looping,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_ranges_are_clamped_or_rejected() {
        assert_eq!(tag_frames(0, 2, 4), Some(0..=2));
        assert_eq!(tag_frames(2, 9, 4), Some(2..=3));
        assert_eq!(tag_frames(7, 9, 4), None);
        assert_eq!(tag_frames(3, 1, 4), None);
        assert_eq!(tag_frames(0, 0, 0), None);
    }

    #[test]
    fn directions_and_repeats_unroll_into_frame_order() {
        use AnimationDirection::*;
        assert_eq!(frame_order(0..=2, Forward, None), (vec![0, 1, 2], true));
        assert_eq!(frame_order(1..=3, Reverse, None), (vec![3, 2, 1], true));
        assert_eq!(
            frame_order(0..=3, PingPong, None),
            (vec![0, 1, 2, 3, 2, 1], true)
        );
        assert_eq!(frame_order(0..=1, PingPong, None), (vec![0, 1], true));
        assert_eq!(frame_order(5..=5, PingPong, None), (vec![5], true));
        assert_eq!(
            frame_order(0..=1, Forward, Some(2)),
            (vec![0, 1, 0, 1], false)
        );
        assert_eq!(frame_order(0..=1, Forward, Some(1)), (vec![0, 1], false));
    }
}
