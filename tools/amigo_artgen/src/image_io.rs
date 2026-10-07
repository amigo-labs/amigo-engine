//! Reading and writing the images the post-processing tools work on, the
//! palettes they take, and laying frames out on a sheet.

use crate::postprocess::PixelBuffer;
use crate::style::StyleDef;
use std::path::Path;

/// Decode a PNG (or any format the `image` crate was built with) as RGBA.
pub fn load_image(path: &Path) -> Result<PixelBuffer, String> {
    let img = image::open(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?
        .to_rgba8();
    let (width, height) = img.dimensions();
    let data = img.pixels().map(|p| p.0).collect();
    Ok(PixelBuffer {
        width,
        height,
        data,
    })
}

/// Write `buf` as an RGBA PNG, creating the directory.
pub fn save_png(buf: &PixelBuffer, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    let bytes: Vec<u8> = buf.data.iter().flatten().copied().collect();
    let img = image::RgbaImage::from_raw(buf.width, buf.height, bytes)
        .ok_or_else(|| format!("{}x{} buffer has the wrong size", buf.width, buf.height))?;
    img.save_with_format(path, image::ImageFormat::Png)
        .map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// PICO-8's 16 colours.
const PICO8: [&str; 16] = [
    "#000000", "#1d2b53", "#7e2553", "#008751", "#ab5236", "#5f574f", "#c2c3c7", "#fff1e8",
    "#ff004d", "#ffa300", "#ffec27", "#00e436", "#29adff", "#83769c", "#ff77a8", "#ffccaa",
];

/// The original Game Boy's four greens.
const GAMEBOY: [&str; 4] = ["#0f380f", "#306230", "#8bac0f", "#9bbc0f"];

/// Names [`parse_palette`] knows besides the built-in styles.
pub const NAMED_PALETTES: [&str; 2] = ["pico8", "gameboy"];

/// A palette from its description, tried in this order:
///
/// - `pico8` or `gameboy`;
/// - the name of a built-in style (its palette);
/// - colours inline: `#rrggbb` separated by commas or spaces;
/// - a file relative to `project_dir`: `.hex` (one `rrggbb` per line), `.gpl`
///   (GIMP palette) or an image (its distinct opaque colours, in reading
///   order, at most 256).
pub fn parse_palette(spec: &str, project_dir: &Path) -> Result<Vec<[u8; 3]>, String> {
    let spec = spec.trim();
    let hex_list = |list: &[&str]| {
        list.iter()
            .filter_map(|h| StyleDef::parse_hex_color(h))
            .collect()
    };
    match spec.to_ascii_lowercase().as_str() {
        "pico8" | "pico-8" => return Ok(hex_list(&PICO8)),
        "gameboy" | "game-boy" | "gb" => return Ok(hex_list(&GAMEBOY)),
        _ => {}
    }
    if let Some(style) = StyleDef::find(spec) {
        return Ok(style.palette_rgb());
    }
    if spec.starts_with('#') {
        let colors: Vec<[u8; 3]> = spec
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(|h| {
                StyleDef::parse_hex_color(h).ok_or_else(|| format!("'{h}' is not a #rrggbb colour"))
            })
            .collect::<Result<_, _>>()?;
        return non_empty(colors, spec);
    }

    let path = project_dir.join(spec);
    if !path.is_file() {
        return Err(format!(
            "unknown palette '{spec}': use {}, a built-in style, #rrggbb colours, or a .hex, .gpl or image file",
            NAMED_PALETTES.join(", ")
        ));
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let colors = match ext.as_str() {
        "hex" | "txt" => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("could not read {}: {e}", path.display()))?;
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with(';'))
                .filter_map(StyleDef::parse_hex_color)
                .collect()
        }
        "gpl" => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("could not read {}: {e}", path.display()))?;
            text.lines()
                .filter_map(|line| {
                    let mut parts = line.split_whitespace().map(|p| p.parse::<u8>());
                    match (parts.next(), parts.next(), parts.next()) {
                        (Some(Ok(r)), Some(Ok(g)), Some(Ok(b))) => Some([r, g, b]),
                        _ => None,
                    }
                })
                .collect()
        }
        _ => {
            let image = load_image(&path)?;
            let mut colors: Vec<[u8; 3]> = Vec::new();
            for p in image.data.iter().filter(|p| p[3] > 0) {
                let c = [p[0], p[1], p[2]];
                if !colors.contains(&c) {
                    if colors.len() == 256 {
                        return Err(format!("{} has more than 256 colours", path.display()));
                    }
                    colors.push(c);
                }
            }
            colors
        }
    };
    non_empty(colors, spec)
}

fn non_empty(colors: Vec<[u8; 3]>, spec: &str) -> Result<Vec<[u8; 3]>, String> {
    if colors.is_empty() {
        Err(format!("palette '{spec}' has no colours"))
    } else {
        Ok(colors)
    }
}

/// Lay `rows` of frames out on one sheet, frame `(column, row)` at
/// `(column · w, row · h)`, where `w × h` is the first frame's size. Frames
/// of another size are cropped or padded, centred.
pub fn compose_sheet(rows: &[Vec<PixelBuffer>]) -> Option<PixelBuffer> {
    let first = rows.iter().flatten().next()?;
    let (w, h) = (first.width, first.height);
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0) as u32;
    let mut sheet = PixelBuffer::new(w * columns, h * rows.len() as u32);
    for (r, row) in rows.iter().enumerate() {
        for (c, frame) in row.iter().enumerate() {
            let (ox, oy) = (c as u32 * w, r as u32 * h);
            let (sx, dx) = centre(frame.width, w);
            let (sy, dy) = centre(frame.height, h);
            for y in 0..frame.height.min(h) {
                for x in 0..frame.width.min(w) {
                    sheet.set(ox + dx + x, oy + dy + y, frame.get(sx + x, sy + y));
                }
            }
        }
    }
    Some(sheet)
}

/// Where to start reading a `from`-wide span and writing it into a `to`-wide
/// one so it is centred: `(source offset, destination offset)`.
fn centre(from: u32, to: u32) -> (u32, u32) {
    if from > to {
        ((from - to) / 2, 0)
    } else {
        (0, (to - from) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> PixelBuffer {
        PixelBuffer {
            width: w,
            height: h,
            data: vec![c; (w * h) as usize],
        }
    }

    #[test]
    fn png_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let mut buf = solid(3, 2, [10, 20, 30, 255]);
        buf.set(2, 1, [0, 0, 0, 0]);
        let path = dir.path().join("sub").join("a.png");
        save_png(&buf, &path).unwrap();
        let back = load_image(&path).unwrap();
        assert_eq!((back.width, back.height), (3, 2));
        assert_eq!(back.data, buf.data);
        assert!(load_image(&dir.path().join("missing.png")).is_err());
    }

    #[test]
    fn palettes_by_name_style_inline_and_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(parse_palette("pico8", dir.path()).unwrap().len(), 16);
        assert_eq!(
            parse_palette("gameboy", dir.path()).unwrap()[0],
            [0x0f, 0x38, 0x0f]
        );
        assert!(!parse_palette("caribbean", dir.path()).unwrap().is_empty());
        assert_eq!(
            parse_palette("#ff0000, #00ff00", dir.path()).unwrap(),
            [[255, 0, 0], [0, 255, 0]]
        );
        assert!(parse_palette("#ff0000,#zz", dir.path()).is_err());

        std::fs::write(dir.path().join("p.hex"), "ff0000\n; comment\n0000ff\n").unwrap();
        assert_eq!(
            parse_palette("p.hex", dir.path()).unwrap(),
            [[255, 0, 0], [0, 0, 255]]
        );
        std::fs::write(
            dir.path().join("p.gpl"),
            "GIMP Palette\nName: t\nColumns: 2\n#\n255 255 255 white\n  0   0   0 black\n",
        )
        .unwrap();
        assert_eq!(
            parse_palette("p.gpl", dir.path()).unwrap(),
            [[255, 255, 255], [0, 0, 0]]
        );
        let mut img = solid(2, 1, [1, 2, 3, 255]);
        img.set(1, 0, [9, 9, 9, 0]);
        save_png(&img, &dir.path().join("p.png")).unwrap();
        assert_eq!(parse_palette("p.png", dir.path()).unwrap(), [[1, 2, 3]]);

        let err = parse_palette("nope", dir.path()).unwrap_err();
        assert!(err.contains("unknown palette 'nope'"), "{err}");
    }

    #[test]
    fn sheets_put_frames_in_rows_and_columns() {
        let a = solid(2, 2, [255, 0, 0, 255]);
        let b = solid(2, 2, [0, 255, 0, 255]);
        let small = solid(1, 1, [0, 0, 255, 255]);
        let sheet = compose_sheet(&[vec![a.clone(), b], vec![small]]).unwrap();
        assert_eq!((sheet.width, sheet.height), (4, 4));
        assert_eq!(sheet.get(1, 1), [255, 0, 0, 255]);
        assert_eq!(sheet.get(2, 0), [0, 255, 0, 255]);
        // A 1x1 frame in a 2x2 cell: centred, rounding to the top left.
        assert_eq!(sheet.get(0, 2), [0, 0, 255, 255]);
        assert_eq!(sheet.get(1, 3), [0, 0, 0, 0]);
        assert_eq!(
            sheet.get(3, 3),
            [0, 0, 0, 0],
            "a short row leaves its cells empty"
        );
        assert!(compose_sheet(&[]).is_none());
    }
}
