use crate::AssetError;
use amigo_animation::{AnimFrame, Animation};
use amigo_core::Rect;
use asefile::AsepriteFile;
use image::RgbaImage;
use std::path::Path;
use tracing::{info, warn};

/// Data extracted from an Aseprite file.
pub struct AsepriteData {
    /// Name derived from filename.
    pub name: String,
    /// Composited frames as RGBA images.
    pub frames: Vec<RgbaImage>,
    /// Frame durations in milliseconds.
    pub frame_durations_ms: Vec<u32>,
    /// Named animations from Aseprite tags.
    pub animations: Vec<Animation>,
    /// Width of each frame.
    pub width: u32,
    /// Height of each frame.
    pub height: u32,
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
    let num_frames = ase.num_frames();

    info!(
        "Loading Aseprite: {} ({}x{}, {} frames)",
        name, width, height, num_frames
    );

    // Composite each frame (all layers merged)
    let mut frames = Vec::with_capacity(num_frames as usize);
    let mut frame_durations_ms = Vec::with_capacity(num_frames as usize);

    for frame_idx in 0..num_frames {
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
        let tag_name = tag.name().to_string();
        let from = tag.from_frame() as usize;
        let to = tag.to_frame() as usize;
        let looping = true; // Default to looping; game can override

        let anim_frames: Vec<AnimFrame> = (from..=to)
            .map(|i| {
                // Each frame gets a UV rect within the sprite sheet
                // In dev mode: frame index determines which texture to use
                // For now: store frame index as uv.x, actual UV computed at texture upload
                let duration_ms = frame_durations_ms.get(i).copied().unwrap_or(100);
                // Convert ms to ticks (60 fps = ~16.67ms per tick)
                let duration_ticks = (duration_ms as f32 / 16.667).round().max(1.0) as u32;

                AnimFrame {
                    uv: Rect::new(i as f32, 0.0, 1.0, 1.0), // frame index encoded in uv.x
                    duration: duration_ticks,
                }
            })
            .collect();

        animations.push(Animation {
            name: format!("{}/{}", name, tag_name),
            frames: anim_frames,
            looping,
        });
    }

    // If no tags, create a default animation with all frames
    if animations.is_empty() && num_frames > 1 {
        let anim_frames: Vec<AnimFrame> = (0..num_frames as usize)
            .map(|i| {
                let duration_ms = frame_durations_ms.get(i).copied().unwrap_or(100);
                let duration_ticks = (duration_ms as f32 / 16.667).round().max(1.0) as u32;
                AnimFrame {
                    uv: Rect::new(i as f32, 0.0, 1.0, 1.0),
                    duration: duration_ticks,
                }
            })
            .collect();

        animations.push(Animation {
            name: format!("{}/default", name),
            frames: anim_frames,
            looping: true,
        });
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
