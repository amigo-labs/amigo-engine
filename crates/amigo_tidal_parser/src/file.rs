use std::path::Path;

use crate::ast::*;
use crate::lexer;
use crate::parser::Parser;

/// File-level error.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error in stem '{stem}': {source}")]
    Parse {
        stem: String,
        source: crate::parser::ParseError,
    },
    #[error("lex error in stem '{stem}': {source}")]
    Lex {
        stem: String,
        source: lexer::LexError,
    },
    #[error("no stems found in file")]
    NoStems,
    #[error("missing metadata field: {0}")]
    MissingField(String),
}

/// Load a `.amigo.tidal` file into a Composition.
pub fn load(path: &Path) -> Result<Composition, FileError> {
    let content = std::fs::read_to_string(path)?;
    parse_amigo_tidal(&content)
}

/// Save a Composition to `.amigo.tidal` format.
pub fn save(composition: &Composition, path: &Path) -> Result<(), FileError> {
    let content = format_amigo_tidal(composition);
    std::fs::write(path, content)?;
    Ok(())
}

/// Parse .amigo.tidal text content.
pub fn parse_amigo_tidal(content: &str) -> Result<Composition, FileError> {
    let mut meta = CompositionMeta::default();
    let mut name = String::new();
    let mut bpm = 120.0;
    let mut stems: Vec<(String, String)> = Vec::new();
    let mut current_stem: Option<(String, String)> = None;
    let mut in_meta = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("-- amigo:meta") {
            in_meta = true;
            continue;
        }

        if trimmed.starts_with("-- amigo:stem ") {
            in_meta = false;
            // Flush previous stem.
            if let Some(stem) = current_stem.take() {
                stems.push(stem);
            }
            let stem_name = trimmed
                .strip_prefix("-- amigo:stem ")
                .unwrap_or("unknown")
                .trim()
                .to_string();
            current_stem = Some((stem_name, String::new()));
            continue;
        }

        if in_meta {
            if let Some(rest) = trimmed.strip_prefix("-- name:") {
                name = rest.trim().trim_matches('"').to_string();
            } else if let Some(rest) = trimmed.strip_prefix("-- bpm:") {
                bpm = rest.trim().parse().unwrap_or(120.0);
            } else if let Some(rest) = trimmed.strip_prefix("-- source:") {
                meta.source = Some(rest.trim().trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("-- license:") {
                meta.license = Some(rest.trim().trim_matches('"').to_string());
            } else if let Some(rest) = trimmed.strip_prefix("-- author:") {
                meta.author = Some(rest.trim().trim_matches('"').to_string());
            }
            continue;
        }

        if let Some((_, ref mut body)) = current_stem {
            body.push_str(line);
            body.push('\n');
        }
    }

    // Flush last stem.
    if let Some(stem) = current_stem.take() {
        stems.push(stem);
    }

    if stems.is_empty() {
        return Err(FileError::NoStems);
    }

    let mut parsed_stems = Vec::new();
    let mut max_cycle_length = 1.0_f64;

    for (stem_name, body) in &stems {
        let trimmed_body = body.trim();
        if trimmed_body.is_empty() {
            parsed_stems.push(Stem {
                name: stem_name.clone(),
                voices: Vec::new(),
            });
            continue;
        }

        let tokens = lexer::tokenize(trimmed_body).map_err(|e| FileError::Lex {
            stem: stem_name.clone(),
            source: e,
        })?;

        let mut parser = Parser::new(tokens);
        let (voice, _transforms, cycle_length) =
            parser
                .parse_voice_def()
                .map_err(|e| FileError::Parse {
                    stem: stem_name.clone(),
                    source: e,
                })?;

        max_cycle_length = max_cycle_length.max(cycle_length);

        parsed_stems.push(Stem {
            name: stem_name.clone(),
            voices: vec![voice],
        });
    }

    Ok(Composition {
        name,
        bpm,
        cycle_length: max_cycle_length,
        stems: parsed_stems,
        metadata: meta,
    })
}

/// Format a Composition as .amigo.tidal text.
pub fn format_amigo_tidal(comp: &Composition) -> String {
    let mut out = String::new();

    // Metadata block.
    out.push_str("-- amigo:meta\n");
    out.push_str(&format!("-- name: \"{}\"\n", comp.name));
    out.push_str(&format!("-- bpm: {}\n", comp.bpm));
    if let Some(ref source) = comp.metadata.source {
        out.push_str(&format!("-- source: \"{source}\"\n"));
    }
    if let Some(ref license) = comp.metadata.license {
        out.push_str(&format!("-- license: \"{license}\"\n"));
    }
    if let Some(ref author) = comp.metadata.author {
        out.push_str(&format!("-- author: \"{author}\"\n"));
    }
    out.push('\n');

    // Stems.
    for (i, stem) in comp.stems.iter().enumerate() {
        out.push_str(&format!("-- amigo:stem {}\n", stem.name));
        let voice_label = format!("d{}", i + 1);

        for voice in &stem.voices {
            let slow = if comp.cycle_length > 1.0 {
                format!(" $ slow {}", comp.cycle_length)
            } else {
                String::new()
            };

            out.push_str(&format!("{voice_label}{slow} $\n"));
            out.push_str(&format!("  n \"{}\"\n", format_pattern(&voice.note_pattern)));
            if let Some(ref amp) = voice.amp_pattern {
                out.push_str(&format!("  # amp \"{}\"\n", format_pattern(amp)));
            }
            if let Some(ref legato) = voice.legato_pattern {
                out.push_str(&format!("  # legato \"{}\"\n", format_pattern(legato)));
            }
        }
        out.push('\n');
    }

    out
}

fn format_pattern(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Atom(atom) => format_atom(atom),
        Pattern::Sequence(elems) => elems.iter().map(format_pattern).collect::<Vec<_>>().join(" "),
        Pattern::Group(elems) => {
            let inner = elems.iter().map(format_pattern).collect::<Vec<_>>().join(" ");
            format!("[{inner}]")
        }
        Pattern::Repeat(inner, n) => format!("{}*{n}", format_pattern(inner)),
        Pattern::Replicate(inner, n) => format!("{}!{n}", format_pattern(inner)),
        Pattern::SlowDiv(inner, n) => format!("{}/{n}", format_pattern(inner)),
        Pattern::Stack(layers) => {
            let inner = layers
                .iter()
                .map(format_pattern)
                .collect::<Vec<_>>()
                .join(", ");
            format!("stack [{inner}]")
        }
    }
}

fn format_atom(atom: &PatternAtom) -> String {
    match atom {
        PatternAtom::Note(note) => {
            let pc = match note.pitch_class {
                PitchClass::C => "c",
                PitchClass::Cs => "cs",
                PitchClass::D => "d",
                PitchClass::Ds => "ds",
                PitchClass::E => "e",
                PitchClass::F => "f",
                PitchClass::Fs => "fs",
                PitchClass::G => "g",
                PitchClass::Gs => "gs",
                PitchClass::A => "a",
                PitchClass::As => "as",
                PitchClass::B => "b",
            };
            format!("{pc}{}", note.octave)
        }
        PatternAtom::Rest => "~".to_string(),
        PatternAtom::Number(n) => {
            if *n == n.floor() {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        PatternAtom::Sample(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::PitchClass;

    const SAMPLE_FILE: &str = r#"-- amigo:meta
-- name: "overworld_theme"
-- bpm: 140
-- source: "ozzed_adventure.wav"
-- license: "CC-BY-4.0"
-- author: "Ozzed"

-- amigo:stem melody
d1 $ slow 8 $ n "c5 d5 e5 ~ g5 a5 g5 e5" # amp "0.8 0.7 0.9 0 0.8 0.7 0.9 0.8"

-- amigo:stem bass
d2 $ slow 8 $ n "c3 ~ c3 ~ g2 ~ g2 ~" # amp "0.9!8"

-- amigo:stem percussion
d3 $ slow 8 $ n "bd ~ sd ~ bd ~ sd bd"
"#;

    #[test]
    fn parse_sample_file_metadata() {
        let comp = parse_amigo_tidal(SAMPLE_FILE).unwrap();
        assert_eq!(comp.name, "overworld_theme");
        assert!((comp.bpm - 140.0).abs() < 0.01);
        assert_eq!(comp.metadata.source.as_deref(), Some("ozzed_adventure.wav"));
        assert_eq!(comp.metadata.license.as_deref(), Some("CC-BY-4.0"));
        assert_eq!(comp.metadata.author.as_deref(), Some("Ozzed"));
    }

    #[test]
    fn parse_sample_file_stems() {
        let comp = parse_amigo_tidal(SAMPLE_FILE).unwrap();
        assert_eq!(comp.stems.len(), 3);
        assert_eq!(comp.stems[0].name, "melody");
        assert_eq!(comp.stems[1].name, "bass");
        assert_eq!(comp.stems[2].name, "percussion");
    }

    #[test]
    fn parse_melody_stem_has_notes() {
        let comp = parse_amigo_tidal(SAMPLE_FILE).unwrap();
        let melody = &comp.stems[0];
        assert_eq!(melody.voices.len(), 1);
        let voice = &melody.voices[0];
        match &voice.note_pattern {
            Pattern::Sequence(elems) => assert_eq!(elems.len(), 8),
            other => panic!("expected Sequence with 8 elements, got {other:?}"),
        }
        assert!(voice.amp_pattern.is_some());
    }

    #[test]
    fn parse_bass_stem_has_replicate() {
        let comp = parse_amigo_tidal(SAMPLE_FILE).unwrap();
        let bass = &comp.stems[1];
        let voice = &bass.voices[0];
        assert!(voice.amp_pattern.is_some());
        // amp "0.9!8" should contain a Replicate node.
        let amp = voice.amp_pattern.as_ref().unwrap();
        match amp {
            Pattern::Replicate(_, 8) => {}
            other => panic!("expected Replicate(_, 8), got {other:?}"),
        }
    }

    #[test]
    fn parse_percussion_uses_samples() {
        let comp = parse_amigo_tidal(SAMPLE_FILE).unwrap();
        let perc = &comp.stems[2];
        let voice = &perc.voices[0];
        match &voice.note_pattern {
            Pattern::Sequence(elems) => {
                // Should contain Sample("bd"), Rest, Sample("sd"), etc.
                let has_bd = elems.iter().any(|e| matches!(e, Pattern::Atom(PatternAtom::Sample(s)) if s == "bd"));
                assert!(has_bd, "expected bd sample in percussion");
            }
            other => panic!("expected Sequence, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_format_and_reparse() {
        let comp = Composition {
            name: "test".into(),
            bpm: 120.0,
            cycle_length: 1.0,
            stems: vec![Stem {
                name: "melody".into(),
                voices: vec![Voice {
                    note_pattern: Pattern::Sequence(vec![
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4))),
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::D, 4))),
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::E, 4))),
                    ]),
                    amp_pattern: None,
                    legato_pattern: None,
                }],
            }],
            metadata: CompositionMeta::default(),
        };

        let text = format_amigo_tidal(&comp);
        let reparsed = parse_amigo_tidal(&text).unwrap();
        assert_eq!(reparsed.name, "test");
        assert_eq!(reparsed.stems.len(), 1);
        assert_eq!(reparsed.stems[0].name, "melody");
    }

    #[test]
    fn cycle_length_from_slow_factor() {
        let comp = parse_amigo_tidal(SAMPLE_FILE).unwrap();
        assert!((comp.cycle_length - 8.0).abs() < 0.01);
    }
}
