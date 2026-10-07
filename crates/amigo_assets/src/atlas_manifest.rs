//! Sprite sheets described by a manifest: `assets/sprites/**/*.atlas.ron`.
//!
//! One sheet image holds many sprites, each with one or more frames and a
//! pivot (origin) per frame. The sheet is uploaded once; sprites point into
//! it by UV. A sprite with `fps` also becomes an [`Animation`] of the same
//! name.
//!
//! ```ron
//! (
//!     image: "hamster.png",
//!     sprites: {
//!         "hamster/fly": (
//!             frames: [(x: 0, y: 0, w: 32, h: 32), (x: 32, y: 0, w: 32, h: 32)],
//!             origin: (16.0, 16.0),
//!             fps: Some(19.0),
//!             looping: true,
//!         ),
//!     },
//! )
//! ```

use amigo_animation::{AnimFrame, Animation};
use amigo_core::{Rect, TimeInfo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// File name suffix of an atlas manifest.
pub const ATLAS_SUFFIX: &str = ".atlas.ron";

/// Whether `path` names an atlas manifest.
pub fn is_atlas_manifest(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(ATLAS_SUFFIX))
}

/// A sheet image and the sprites on it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtlasManifest {
    /// Sheet image, relative to the manifest.
    pub image: String,
    /// Sprites on the sheet, by name. Names are global like other sprite
    /// names; a clash with another sprite is an error.
    pub sprites: BTreeMap<String, AtlasSprite>,
    /// Mip levels below the full-size image. 0, the default, means none.
    /// With `k > 0` every frame must be aligned to `2^k` pixels and separated
    /// from every other frame by at least `2^k` pixels of fully transparent
    /// gutter.
    #[serde(default)]
    pub mip_levels: u32,
}

/// One sprite on a sheet.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtlasSprite {
    /// At least one frame.
    pub frames: Vec<AtlasFrame>,
    /// Pivot in pixels from each frame's top-left corner.
    #[serde(default)]
    pub origin: (f32, f32),
    /// When set, the sprite also registers an `Animation` of the same name.
    /// Must be finite and in `(0, TICKS_PER_SECOND]`.
    #[serde(default)]
    pub fps: Option<f32>,
    #[serde(default)]
    pub looping: bool,
}

/// A frame rectangle on the sheet, in pixels.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct AtlasFrame {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Overrides the sprite's `origin` for this frame.
    #[serde(default)]
    pub origin: Option<(f32, f32)>,
}

/// What is wrong with a manifest.
#[derive(Clone, Debug, thiserror::Error)]
pub enum AtlasError {
    #[error("{path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("{path}: image '{image}' not found or unreadable")]
    Image { path: PathBuf, image: String },
    #[error("{path}: sprite '{sprite}' frame {frame} lies outside the {width}x{height} sheet")]
    FrameOutOfBounds {
        path: PathBuf,
        sprite: String,
        frame: usize,
        width: u32,
        height: u32,
    },
    #[error("{path}: sprite '{sprite}' has no frames")]
    NoFrames { path: PathBuf, sprite: String },
    #[error("{path}: sprite name '{sprite}' is already used by {other}")]
    DuplicateName {
        path: PathBuf,
        sprite: String,
        other: String,
    },
    #[error("{path}: sprite '{sprite}' fps {fps} is not in (0, {max}]")]
    InvalidFps {
        path: PathBuf,
        sprite: String,
        fps: f32,
        max: u32,
    },
    #[error(
        "{path}: sprite '{sprite}' frame {frame} is not aligned to or isolated by {gutter} px for mip_levels {mip_levels}"
    )]
    MipPadding {
        path: PathBuf,
        sprite: String,
        frame: usize,
        mip_levels: u32,
        gutter: u32,
    },
}

/// One frame of a sprite, resolved against the texture it lives on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteFrame {
    /// Pixel rectangle on the texture.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// The same rectangle in UV space.
    pub uv: Rect,
    /// Pivot in pixels from the frame's top-left corner.
    pub origin: [f32; 2],
}

impl SpriteFrame {
    /// The frame covering a whole `w`×`h` texture, pivot at the top-left.
    pub fn whole(w: u32, h: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            w,
            h,
            uv: Rect::new(0.0, 0.0, 1.0, 1.0),
            origin: [0.0, 0.0],
        }
    }

    /// The rectangle `(x, y, w, h)` of a `tex_w`×`tex_h` texture.
    pub fn on_texture(x: u32, y: u32, w: u32, h: u32, tex: (u32, u32), origin: [f32; 2]) -> Self {
        let (tw, th) = (tex.0.max(1) as f32, tex.1.max(1) as f32);
        Self {
            x,
            y,
            w,
            h,
            uv: Rect::new(x as f32 / tw, y as f32 / th, w as f32 / tw, h as f32 / th),
            origin,
        }
    }
}

/// A sprite of a loaded sheet.
#[derive(Clone, Debug)]
pub struct AtlasSpriteData {
    pub name: String,
    pub frames: Vec<SpriteFrame>,
    /// Registered when the sprite has `fps`.
    pub animation: Option<Animation>,
}

/// A validated manifest with its sheet image.
#[derive(Clone, Debug)]
pub struct LoadedAtlas {
    pub manifest_path: PathBuf,
    pub image_path: PathBuf,
    pub image: image::RgbaImage,
    /// Mip levels the sheet may use: the manifest's, or 0 when it failed the
    /// padding check (see `mip_error`).
    pub mip_levels: u32,
    /// Why the requested mip levels were refused; the sheet still loads.
    pub mip_error: Option<AtlasError>,
    pub sprites: Vec<AtlasSpriteData>,
}

/// Parse a manifest's text.
pub fn parse_manifest(path: &Path, text: &str) -> Result<AtlasManifest, AtlasError> {
    ron::from_str(text).map_err(|e| AtlasError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// The sheet image path a manifest at `manifest_path` names.
pub fn image_path(manifest_path: &Path, manifest: &AtlasManifest) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or(Path::new(""))
        .join(&manifest.image)
}

/// Read and validate the manifest at `path` and its sheet image.
pub fn load_atlas(path: &Path) -> Result<LoadedAtlas, AtlasError> {
    let text = std::fs::read_to_string(path).map_err(|e| AtlasError::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let manifest = parse_manifest(path, &text)?;
    let image_path = image_path(path, &manifest);
    let image = image::open(&image_path)
        .map_err(|_| AtlasError::Image {
            path: path.to_path_buf(),
            image: manifest.image.clone(),
        })?
        .to_rgba8();
    build_atlas(path, &manifest, image_path, image)
}

/// Validate `manifest` against its sheet `image` and resolve its sprites.
pub fn build_atlas(
    manifest_path: &Path,
    manifest: &AtlasManifest,
    image_path: PathBuf,
    image: image::RgbaImage,
) -> Result<LoadedAtlas, AtlasError> {
    let path = manifest_path.to_path_buf();
    let (width, height) = image.dimensions();
    let mut sprites = Vec::with_capacity(manifest.sprites.len());
    for (name, sprite) in &manifest.sprites {
        if sprite.frames.is_empty() {
            return Err(AtlasError::NoFrames {
                path,
                sprite: name.clone(),
            });
        }
        let mut frames = Vec::with_capacity(sprite.frames.len());
        for (i, f) in sprite.frames.iter().enumerate() {
            let inside = f.w > 0
                && f.h > 0
                && f.x.checked_add(f.w).is_some_and(|r| r <= width)
                && f.y.checked_add(f.h).is_some_and(|b| b <= height);
            if !inside {
                return Err(AtlasError::FrameOutOfBounds {
                    path,
                    sprite: name.clone(),
                    frame: i,
                    width,
                    height,
                });
            }
            let (ox, oy) = f.origin.unwrap_or(sprite.origin);
            frames.push(SpriteFrame::on_texture(
                f.x,
                f.y,
                f.w,
                f.h,
                (width, height),
                [ox, oy],
            ));
        }
        let animation = match sprite.fps {
            None => None,
            Some(fps) => {
                let max = TimeInfo::TICKS_PER_SECOND;
                if !(fps.is_finite() && fps > 0.0 && fps <= max as f32) {
                    return Err(AtlasError::InvalidFps {
                        path,
                        sprite: name.clone(),
                        fps,
                        max,
                    });
                }
                let durations = frame_durations(fps, frames.len());
                Some(Animation {
                    name: name.clone(),
                    frames: frames
                        .iter()
                        .zip(durations)
                        .map(|(f, duration)| AnimFrame { uv: f.uv, duration })
                        .collect(),
                    looping: sprite.looping,
                })
            }
        };
        sprites.push(AtlasSpriteData {
            name: name.clone(),
            frames,
            animation,
        });
    }

    let (mip_levels, mip_error) = match check_mip_padding(manifest_path, manifest, &image) {
        Ok(()) => (manifest.mip_levels, None),
        Err(e) => (0, Some(e)),
    };
    Ok(LoadedAtlas {
        manifest_path: path,
        image_path,
        image,
        mip_levels,
        mip_error,
        sprites,
    })
}

/// Frame durations in ticks such that frame `k` starts at tick
/// `round(k · TICKS_PER_SECOND / fps)`: the clip's length stays within one
/// tick of exact even when `fps` does not divide the tick rate. `fps` must be
/// in `(0, TICKS_PER_SECOND]`, which keeps every frame at least one tick long.
pub fn frame_durations(fps: f32, frames: usize) -> Vec<u32> {
    let tps = TimeInfo::TICKS_PER_SECOND as f64;
    let start = |k: usize| (k as f64 * tps / fps as f64).round() as u32;
    (0..frames)
        .map(|k| (start(k + 1) - start(k)).max(1))
        .collect()
}

/// Check the manifest's `mip_levels` against the sheet: every frame aligned to
/// `2^k` px, at least `2^k` px from every other frame, and fully transparent
/// within `2^k` px around it.
pub fn check_mip_padding(
    manifest_path: &Path,
    manifest: &AtlasManifest,
    image: &image::RgbaImage,
) -> Result<(), AtlasError> {
    let k = manifest.mip_levels;
    if k == 0 {
        return Ok(());
    }
    let gutter = 1u32.checked_shl(k).unwrap_or(u32::MAX);
    let fail = |sprite: &str, frame: usize| AtlasError::MipPadding {
        path: manifest_path.to_path_buf(),
        sprite: sprite.to_string(),
        frame,
        mip_levels: k,
        gutter,
    };
    // Distinct rectangles; a frame shared by two sprites is checked once.
    let mut rects: Vec<(&str, usize, AtlasFrame)> = Vec::new();
    for (name, sprite) in &manifest.sprites {
        for (i, f) in sprite.frames.iter().enumerate() {
            if !rects
                .iter()
                .any(|(_, _, r)| (r.x, r.y, r.w, r.h) == (f.x, f.y, f.w, f.h))
            {
                rects.push((name, i, *f));
            }
        }
    }
    for &(name, i, f) in &rects {
        if [f.x, f.y, f.w, f.h].iter().any(|v| v % gutter != 0) {
            return Err(fail(name, i));
        }
    }
    let gap = |a: &AtlasFrame, b: &AtlasFrame| -> i64 {
        let (ax, ay, aw, ah) = (a.x as i64, a.y as i64, a.w as i64, a.h as i64);
        let (bx, by, bw, bh) = (b.x as i64, b.y as i64, b.w as i64, b.h as i64);
        (bx - (ax + aw))
            .max(ax - (bx + bw))
            .max(by - (ay + ah))
            .max(ay - (by + bh))
    };
    for (n, (name, i, a)) in rects.iter().enumerate() {
        if rects[n + 1..]
            .iter()
            .any(|(_, _, b)| gap(a, b) < gutter as i64)
        {
            return Err(fail(name, *i));
        }
    }
    // The ring of `gutter` px around each frame must be fully transparent.
    let (w, h) = image.dimensions();
    for &(name, i, f) in &rects {
        let x0 = f.x.saturating_sub(gutter);
        let y0 = f.y.saturating_sub(gutter);
        let x1 = (f.x + f.w).saturating_add(gutter).min(w);
        let y1 = (f.y + f.h).saturating_add(gutter).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                let inside = x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h;
                if !inside && image.get_pixel(x, y).0[3] != 0 {
                    return Err(fail(name, i));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: u32, y: u32, w: u32, h: u32) -> AtlasFrame {
        AtlasFrame {
            x,
            y,
            w,
            h,
            origin: None,
        }
    }

    fn manifest(
        sprites: &[(&str, Vec<AtlasFrame>, Option<f32>)],
        mip_levels: u32,
    ) -> AtlasManifest {
        AtlasManifest {
            image: "sheet.png".into(),
            sprites: sprites
                .iter()
                .map(|(name, frames, fps)| {
                    (
                        name.to_string(),
                        AtlasSprite {
                            frames: frames.clone(),
                            origin: (1.0, 2.0),
                            fps: *fps,
                            looping: true,
                        },
                    )
                })
                .collect(),
            mip_levels,
        }
    }

    fn build(m: &AtlasManifest, image: image::RgbaImage) -> Result<LoadedAtlas, AtlasError> {
        build_atlas(
            Path::new("a.atlas.ron"),
            m,
            PathBuf::from("sheet.png"),
            image,
        )
    }

    #[test]
    fn nineteen_fps_stays_within_a_tick_of_exact() {
        let durations = frame_durations(19.0, 40);
        assert_eq!(&durations[..4], &[3, 3, 3, 4]);
        let mut sum = 0u32;
        for (k, d) in durations.iter().enumerate() {
            sum += d;
            let exact = (k + 1) as f64 * 60.0 / 19.0;
            assert!(
                (sum as f64 - exact).abs() <= 1.0,
                "frame {k}: {sum} vs {exact}"
            );
        }
    }

    #[test]
    fn sixty_fps_is_one_tick_per_frame() {
        assert_eq!(frame_durations(60.0, 5), vec![1; 5]);
    }

    #[test]
    fn out_of_range_fps_is_refused() {
        let image = image::RgbaImage::new(8, 8);
        for fps in [0.0, -1.0, f32::NAN, f32::INFINITY, 120.0] {
            let m = manifest(&[("s", vec![frame(0, 0, 4, 4)], Some(fps))], 0);
            let err = build(&m, image.clone()).unwrap_err();
            assert!(matches!(err, AtlasError::InvalidFps { .. }), "{fps}: {err}");
        }
    }

    #[test]
    fn frames_resolve_to_uvs_and_origins() {
        let mut m = manifest(
            &[("s", vec![frame(4, 0, 4, 8), frame(0, 0, 4, 8)], Some(30.0))],
            0,
        );
        if let Some(sprite) = m.sprites.get_mut("s") {
            sprite.frames[1].origin = Some((3.0, 3.0));
        }
        let atlas = build(&m, image::RgbaImage::new(8, 8)).expect("valid");
        let s = &atlas.sprites[0];
        assert_eq!(s.frames[0].uv, Rect::new(0.5, 0.0, 0.5, 1.0));
        assert_eq!(s.frames[0].origin, [1.0, 2.0]);
        assert_eq!(s.frames[1].origin, [3.0, 3.0]);
        let anim = s.animation.as_ref().expect("fps makes an animation");
        assert_eq!(anim.name, "s");
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(anim.frames[0].duration, 2);
    }

    #[test]
    fn each_malformed_manifest_has_its_error() {
        let image = image::RgbaImage::new(8, 8);
        let none = manifest(&[("s", vec![], None)], 0);
        assert!(matches!(
            build(&none, image.clone()),
            Err(AtlasError::NoFrames { .. })
        ));
        let outside = manifest(&[("s", vec![frame(6, 6, 4, 4)], None)], 0);
        assert!(matches!(
            build(&outside, image.clone()),
            Err(AtlasError::FrameOutOfBounds { frame: 0, .. })
        ));
        let empty = manifest(&[("s", vec![frame(0, 0, 0, 4)], None)], 0);
        assert!(matches!(
            build(&empty, image),
            Err(AtlasError::FrameOutOfBounds { .. })
        ));
        assert!(matches!(
            parse_manifest(Path::new("x"), "(image: 3)"),
            Err(AtlasError::Parse { .. })
        ));
    }

    #[test]
    fn a_thin_gutter_refuses_mip_levels_but_still_loads() {
        // Two 4x4 frames 3 px apart, mip_levels 2 needs 4.
        let image = image::RgbaImage::new(16, 4);
        let m = manifest(
            &[
                ("a", vec![frame(0, 0, 4, 4)], None),
                ("b", vec![frame(7, 0, 4, 4)], None),
            ],
            2,
        );
        let atlas = build(&m, image).expect("sheet still loads");
        assert_eq!(atlas.mip_levels, 0);
        assert!(matches!(
            atlas.mip_error,
            Some(AtlasError::MipPadding { .. })
        ));
    }

    #[test]
    fn an_opaque_gutter_refuses_mip_levels() {
        let mut image = image::RgbaImage::new(16, 4);
        image.put_pixel(5, 1, image::Rgba([0, 0, 0, 1]));
        let m = manifest(
            &[
                ("a", vec![frame(0, 0, 4, 4)], None),
                ("b", vec![frame(8, 0, 4, 4)], None),
            ],
            2,
        );
        let atlas = build(&m, image).expect("loads");
        assert!(atlas.mip_error.is_some());
    }

    #[test]
    fn a_padded_sheet_keeps_its_mip_levels() {
        let image = image::RgbaImage::new(16, 4);
        let m = manifest(
            &[
                ("a", vec![frame(0, 0, 4, 4)], None),
                ("b", vec![frame(8, 0, 4, 4)], None),
            ],
            2,
        );
        let atlas = build(&m, image).expect("loads");
        assert_eq!(atlas.mip_levels, 2);
        assert!(atlas.mip_error.is_none());
    }
}
