//! The editor draws no interface colour of its own (amigo-theme AC-11): its
//! egui chrome takes every colour from the context's visuals, which the
//! engine fills from the shared amigo theme.

use std::fs;
use std::path::Path;

/// The raw colours that are content, not interface: `(file, line)`.
const CONTENT: [(&str, &str); 1] = [
    // A tile swatch shows the colour the viewport preview uses for that tile.
    (
        "egui_ui.rs",
        "(egui::Color32::from_rgb(r, g, b), egui::Color32::WHITE)",
    ),
];

/// Whether `line` holds a hex colour literal: `#rrggbb` or `0xrrggbb`
/// (optionally with alpha).
fn has_hex_colour(line: &str) -> bool {
    let bytes = line.as_bytes();
    (0..bytes.len()).any(|at| {
        let digits_from = match bytes[at] {
            b'#' => at + 1,
            b'0' if bytes.get(at + 1) == Some(&b'x') => at + 2,
            _ => return false,
        };
        let digits = bytes[digits_from.min(bytes.len())..]
            .iter()
            .take_while(|b| b.is_ascii_hexdigit() || **b == b'_')
            .filter(|b| b.is_ascii_hexdigit())
            .count();
        digits == 6 || digits == 8
    })
}

#[test]
fn hex_colours_are_recognised() {
    assert!(has_hex_colour("let c = 0x1d1e23;"));
    assert!(has_hex_colour("fill: \"#F5E554\""));
    assert!(has_hex_colour("0xff_00_00_ff"));
    assert!(!has_hex_colour("#[derive(Debug)]"));
    assert!(!has_hex_colour("let mask = 0xff;"));
    assert!(!has_hex_colour("h.wrapping_mul(31)"));
}

#[test]
fn the_editor_sources_hold_no_interface_colour() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    let mut files = 0;
    for entry in fs::read_dir(&src).expect("src is readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        files += 1;
        let name = path
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        let text = fs::read_to_string(&path).expect("source is readable");
        for (number, line) in text.lines().enumerate() {
            let code = line.trim();
            if code.starts_with("//") {
                continue;
            }
            let raw = code.contains("Color32::from_rgb")
                || code.contains("from_gray")
                || has_hex_colour(code);
            if raw && !CONTENT.contains(&(name.as_str(), code)) {
                found.push(format!("{name}:{}: {code}", number + 1));
            }
        }
    }
    assert!(files > 10, "the scan read the editor's sources");
    assert!(
        found.is_empty(),
        "interface colours outside the theme:\n{}",
        found.join("\n")
    );
}
