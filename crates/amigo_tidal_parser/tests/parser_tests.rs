use amigo_tidal_parser::lexer::tokenize;
use amigo_tidal_parser::parser::parse_mini;
use amigo_tidal_parser::parser::Parser;
use amigo_tidal_parser::{NoteValue, Pattern, PatternAtom, PitchClass, Transform};

#[test]
fn simple_sequence() {
    let pat = parse_mini("c4 d4 e4").unwrap();
    match pat {
        Pattern::Sequence(elems) => {
            assert_eq!(elems.len(), 3);
            assert_eq!(
                elems[0],
                Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
            );
            assert_eq!(
                elems[1],
                Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::D, 4)))
            );
            assert_eq!(
                elems[2],
                Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::E, 4)))
            );
        }
        _ => panic!("expected Sequence"),
    }
}

#[test]
fn sequence_with_rests() {
    let pat = parse_mini("c4 ~ e4 ~").unwrap();
    match pat {
        Pattern::Sequence(elems) => {
            assert_eq!(elems.len(), 4);
            assert_eq!(elems[1], Pattern::Atom(PatternAtom::Rest));
            assert_eq!(elems[3], Pattern::Atom(PatternAtom::Rest));
        }
        _ => panic!("expected Sequence"),
    }
}

#[test]
fn nested_group() {
    let pat = parse_mini("[c4 d4] e4").unwrap();
    match pat {
        Pattern::Sequence(elems) => {
            assert_eq!(elems.len(), 2);
            match &elems[0] {
                Pattern::Group(inner) => {
                    assert_eq!(inner.len(), 2);
                    assert_eq!(
                        inner[0],
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
                    );
                    assert_eq!(
                        inner[1],
                        Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::D, 4)))
                    );
                }
                other => panic!("expected Group, got {other:?}"),
            }
        }
        _ => panic!("expected Sequence"),
    }
}

#[test]
fn repeat_operator() {
    let pat = parse_mini("c4*4").unwrap();
    match pat {
        Pattern::Repeat(inner, 4) => {
            assert_eq!(
                *inner,
                Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
            );
        }
        _ => panic!("expected Repeat(_, 4), got {pat:?}"),
    }
}

#[test]
fn replicate_operator() {
    let pat = parse_mini("c4!3").unwrap();
    match pat {
        Pattern::Replicate(inner, 3) => {
            assert_eq!(
                *inner,
                Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
            );
        }
        _ => panic!("expected Replicate(_, 3), got {pat:?}"),
    }
}

#[test]
fn repeat_and_replicate_differ() {
    let repeat = parse_mini("c4*4").unwrap();
    let replicate = parse_mini("c4!4").unwrap();
    // They should parse into different AST variants.
    assert!(matches!(repeat, Pattern::Repeat(_, 4)));
    assert!(matches!(replicate, Pattern::Replicate(_, 4)));
}

#[test]
fn slow_div_operator() {
    let pat = parse_mini("c4/2").unwrap();
    match pat {
        Pattern::SlowDiv(inner, 2) => {
            assert_eq!(
                *inner,
                Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
            );
        }
        _ => panic!("expected SlowDiv(_, 2)"),
    }
}

#[test]
fn stack_pattern() {
    let input = r#"d1 $ slow 8 $ stack [
        n "c5 d5 e5" # amp "0.8 0.7 0.9",
        n "e4 ~ g4" # amp "0.5 0 0.5"
    ]"#;
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let (voice, transforms, _) = parser.parse_voice_def().unwrap();

    assert_eq!(transforms, vec![Transform::Slow(8.0)]);
    match voice.note_pattern {
        Pattern::Stack(layers) => assert_eq!(layers.len(), 2),
        other => panic!("expected Stack with 2 layers, got {other:?}"),
    }
}

#[test]
fn voice_def_with_slow_and_fast() {
    let input = r#"d1 $ slow 4 $ fast 2 $ n "c4 d4 e4""#;
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let (_, transforms, cycle_length) = parser.parse_voice_def().unwrap();

    assert_eq!(transforms.len(), 2);
    assert_eq!(transforms[0], Transform::Slow(4.0));
    assert_eq!(transforms[1], Transform::Fast(2.0));
    // slow 4 * fast 2 = cycle_length 4 / 2 = 2
    assert!((cycle_length - 2.0).abs() < 0.001);
}

#[test]
fn voice_def_with_rev() {
    let input = r#"d1 $ rev $ n "c4 d4 e4""#;
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let (_, transforms, _) = parser.parse_voice_def().unwrap();

    assert_eq!(transforms, vec![Transform::Rev]);
}

#[test]
fn voice_def_with_amp_and_legato() {
    let input = r#"d1 $ n "c4 d4 e4" # amp "0.8 0.7 0.9" # legato "1.0 0.5 1.0""#;
    let tokens = tokenize(input).unwrap();
    let mut parser = Parser::new(tokens);
    let (voice, _, _) = parser.parse_voice_def().unwrap();

    assert!(voice.amp_pattern.is_some());
    assert!(voice.legato_pattern.is_some());
}

#[test]
fn number_pattern() {
    let pat = parse_mini("0.8 0.7 0.9 0").unwrap();
    match pat {
        Pattern::Sequence(elems) => {
            assert_eq!(elems.len(), 4);
            assert_eq!(elems[0], Pattern::Atom(PatternAtom::Number(0.8)));
            assert_eq!(elems[3], Pattern::Atom(PatternAtom::Number(0.0)));
        }
        _ => panic!("expected Sequence"),
    }
}

#[test]
fn sample_pattern() {
    let pat = parse_mini("bd ~ sd ~ bd ~ sd bd").unwrap();
    match pat {
        Pattern::Sequence(elems) => {
            assert_eq!(elems.len(), 8);
            assert!(matches!(&elems[0], Pattern::Atom(PatternAtom::Sample(s)) if s == "bd"));
            assert_eq!(elems[1], Pattern::Atom(PatternAtom::Rest));
            assert!(matches!(&elems[2], Pattern::Atom(PatternAtom::Sample(s)) if s == "sd"));
        }
        _ => panic!("expected Sequence"),
    }
}

#[test]
fn deeply_nested_groups() {
    let pat = parse_mini("[[c4 d4] [e4 f4]] g4").unwrap();
    match pat {
        Pattern::Sequence(elems) => {
            assert_eq!(elems.len(), 2);
            match &elems[0] {
                Pattern::Group(inner) => {
                    assert_eq!(inner.len(), 2);
                    assert!(matches!(&inner[0], Pattern::Group(_)));
                    assert!(matches!(&inner[1], Pattern::Group(_)));
                }
                other => panic!("expected nested Group, got {other:?}"),
            }
        }
        _ => panic!("expected Sequence"),
    }
}
