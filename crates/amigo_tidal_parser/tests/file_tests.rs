use amigo_tidal_parser::{
    evaluate_pattern, load, parse_amigo_tidal, save, Composition, CompositionMeta, NoteValue,
    Pattern, PatternAtom, PitchClass, Stem, Voice,
};
use std::path::Path;

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

#[test]
fn load_overworld_fixture() {
    let path = Path::new(FIXTURE_DIR).join("overworld.amigo.tidal");
    let comp = load(&path).unwrap();
    assert_eq!(comp.name, "overworld_theme");
    assert!((comp.bpm - 140.0).abs() < 0.01);
    assert_eq!(comp.stems.len(), 3);
    assert_eq!(comp.stems[0].name, "melody");
    assert_eq!(comp.stems[1].name, "bass");
    assert_eq!(comp.stems[2].name, "percussion");
}

#[test]
fn load_overworld_metadata() {
    let path = Path::new(FIXTURE_DIR).join("overworld.amigo.tidal");
    let comp = load(&path).unwrap();
    assert_eq!(comp.metadata.source.as_deref(), Some("ozzed_adventure.wav"));
    assert_eq!(comp.metadata.license.as_deref(), Some("CC-BY-4.0"));
    assert_eq!(comp.metadata.author.as_deref(), Some("Ozzed"));
}

#[test]
fn load_simple_fixture() {
    let path = Path::new(FIXTURE_DIR).join("simple.amigo.tidal");
    let comp = load(&path).unwrap();
    assert_eq!(comp.name, "simple_test");
    assert!((comp.bpm - 120.0).abs() < 0.01);
    assert_eq!(comp.stems.len(), 1);
    assert_eq!(comp.stems[0].name, "melody");
}

#[test]
fn load_str_path() {
    // Verify that load accepts &str as well as &Path.
    let path = format!("{FIXTURE_DIR}/simple.amigo.tidal");
    let comp = load(&path).unwrap();
    assert_eq!(comp.name, "simple_test");
}

#[test]
fn metadata_extraction() {
    let content = r#"-- amigo:meta
-- name: "test_song"
-- bpm: 180
-- source: "source.wav"
-- license: "MIT"
-- author: "TestAuthor"

-- amigo:stem lead
d1 $ n "c4 d4 e4"
"#;
    let comp = parse_amigo_tidal(content).unwrap();
    assert_eq!(comp.name, "test_song");
    assert!((comp.bpm - 180.0).abs() < 0.01);
    assert_eq!(comp.metadata.source.as_deref(), Some("source.wav"));
    assert_eq!(comp.metadata.license.as_deref(), Some("MIT"));
    assert_eq!(comp.metadata.author.as_deref(), Some("TestAuthor"));
}

#[test]
fn multiple_stems_separated() {
    let content = r#"-- amigo:meta
-- name: "multi"
-- bpm: 120

-- amigo:stem melody
d1 $ n "c4 d4 e4"

-- amigo:stem bass
d2 $ n "c2 ~ c2 ~"

-- amigo:stem drums
d3 $ n "bd sd bd sd"
"#;
    let comp = parse_amigo_tidal(content).unwrap();
    assert_eq!(comp.stems.len(), 3);
    assert_eq!(comp.stems[0].name, "melody");
    assert_eq!(comp.stems[1].name, "bass");
    assert_eq!(comp.stems[2].name, "drums");
}

#[test]
fn roundtrip_save_and_load() {
    let comp = Composition {
        name: "roundtrip_test".into(),
        bpm: 150.0,
        cycle_length: 4.0,
        stems: vec![
            Stem {
                name: "melody".into(),
                voices: vec![Voice {
                    note_pattern: Pattern::Sequence(vec![
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 5))),
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::E, 5))),
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::G, 5))),
                        Pattern::Atom(PatternAtom::Rest),
                    ]),
                    amp_pattern: None,
                    legato_pattern: None,
                }],
            },
            Stem {
                name: "bass".into(),
                voices: vec![Voice {
                    note_pattern: Pattern::Sequence(vec![
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 2))),
                        Pattern::Atom(PatternAtom::Rest),
                    ]),
                    amp_pattern: None,
                    legato_pattern: None,
                }],
            },
        ],
        metadata: CompositionMeta {
            source: Some("test.wav".into()),
            license: Some("CC0".into()),
            author: Some("Test".into()),
        },
    };

    let tmp = std::env::temp_dir().join("roundtrip_test.amigo.tidal");
    save(&comp, &tmp).unwrap();

    let reloaded = load(&tmp).unwrap();
    assert_eq!(reloaded.name, "roundtrip_test");
    assert!((reloaded.bpm - 150.0).abs() < 0.01);
    assert_eq!(reloaded.stems.len(), 2);
    assert_eq!(reloaded.stems[0].name, "melody");
    assert_eq!(reloaded.stems[1].name, "bass");
    assert_eq!(reloaded.metadata.license.as_deref(), Some("CC0"));

    // Clean up.
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn overworld_fixture_evaluates_to_events() {
    let path = Path::new(FIXTURE_DIR).join("overworld.amigo.tidal");
    let comp = load(&path).unwrap();
    let events = evaluate_pattern(&comp, 0);
    // Should produce events from all stems.
    assert!(!events.is_empty());
    // Melody stem has a stack with 2 layers, should produce events.
    let melody_events: Vec<_> = events.iter().filter(|e| e.stem_index == 0).collect();
    assert!(!melody_events.is_empty());
}

#[test]
fn no_stems_returns_error() {
    let content = r#"-- amigo:meta
-- name: "empty"
-- bpm: 120
"#;
    let result = parse_amigo_tidal(content);
    assert!(result.is_err());
}
