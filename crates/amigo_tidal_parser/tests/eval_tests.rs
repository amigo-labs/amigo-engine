use amigo_tidal_parser::ast::*;
use amigo_tidal_parser::{evaluate_pattern, apply_transform, NoteEvent, NoteValue, PitchClass, Transform};

fn note(pc: PitchClass, oct: i8) -> Pattern {
    Pattern::Atom(PatternAtom::Note(NoteValue::new(pc, oct)))
}

fn seq(patterns: Vec<Pattern>) -> Pattern {
    Pattern::Sequence(patterns)
}

fn make_comp(note_pattern: Pattern) -> Composition {
    Composition {
        name: "test".into(),
        bpm: 120.0,
        cycle_length: 1.0,
        stems: vec![Stem {
            name: "melody".into(),
            voices: vec![Voice {
                note_pattern,
                amp_pattern: None,
                legato_pattern: None,
            }],
        }],
        metadata: CompositionMeta::default(),
    }
}

#[test]
fn timing_even_distribution() {
    let comp = make_comp(seq(vec![
        note(PitchClass::C, 4),
        note(PitchClass::D, 4),
        note(PitchClass::E, 4),
    ]));
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 3);

    let dur = 1.0 / 3.0;
    assert!((events[0].time - 0.0).abs() < 1e-9);
    assert!((events[0].duration - dur).abs() < 1e-9);
    assert!((events[1].time - dur).abs() < 1e-9);
    assert!((events[2].time - 2.0 * dur).abs() < 1e-9);
}

#[test]
fn nested_group_timing() {
    // "[c4 d4] e4" -> c4@0.0(dur=0.25), d4@0.25(dur=0.25), e4@0.5(dur=0.5)
    let comp = make_comp(seq(vec![
        Pattern::Group(vec![note(PitchClass::C, 4), note(PitchClass::D, 4)]),
        note(PitchClass::E, 4),
    ]));
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 3);
    assert!((events[0].time - 0.0).abs() < 1e-9);
    assert!((events[0].duration - 0.25).abs() < 1e-9);
    assert!((events[1].time - 0.25).abs() < 1e-9);
    assert!((events[1].duration - 0.25).abs() < 1e-9);
    assert!((events[2].time - 0.5).abs() < 1e-9);
    assert!((events[2].duration - 0.5).abs() < 1e-9);
}

#[test]
fn rest_produces_no_event() {
    let comp = make_comp(seq(vec![
        note(PitchClass::C, 4),
        Pattern::Atom(PatternAtom::Rest),
        note(PitchClass::E, 4),
    ]));
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].note.pitch_class, PitchClass::C);
    assert_eq!(events[1].note.pitch_class, PitchClass::E);
}

#[test]
fn repeat_subdivides_slot() {
    let comp = make_comp(Pattern::Repeat(Box::new(note(PitchClass::C, 4)), 4));
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 4);
    for (i, ev) in events.iter().enumerate() {
        assert!((ev.time - i as f64 * 0.25).abs() < 1e-9);
        assert!((ev.duration - 0.25).abs() < 1e-9);
    }
}

#[test]
fn stack_layers_overlap() {
    let comp = make_comp(Pattern::Stack(vec![
        note(PitchClass::C, 4),
        note(PitchClass::E, 4),
        note(PitchClass::G, 4),
    ]));
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 3);
    // All at time 0.0 with duration 1.0.
    for ev in &events {
        assert!((ev.time - 0.0).abs() < 1e-9);
        assert!((ev.duration - 1.0).abs() < 1e-9);
    }
}

#[test]
fn slow_div_cycle_dependent() {
    let comp = make_comp(Pattern::SlowDiv(Box::new(note(PitchClass::C, 4)), 2));

    // Plays on even cycles.
    let events_0 = evaluate_pattern(&comp, 0);
    assert_eq!(events_0.len(), 1);

    // Silent on odd cycles.
    let events_1 = evaluate_pattern(&comp, 1);
    assert_eq!(events_1.len(), 0);

    // Plays again on cycle 2.
    let events_2 = evaluate_pattern(&comp, 2);
    assert_eq!(events_2.len(), 1);
}

#[test]
fn transform_slow() {
    let comp = make_comp(seq(vec![
        note(PitchClass::C, 4),
        note(PitchClass::D, 4),
        note(PitchClass::E, 4),
        note(PitchClass::F, 4),
    ]));
    let mut events = evaluate_pattern(&comp, 0);
    apply_transform(&mut events, Transform::Slow(2.0));
    // With slow 2, events at 0.0, 0.25, 0.5, 0.75 become 0.0, 0.125, 0.25, 0.375
    // and only those within 0.0..1.0 are kept (all should survive).
    assert_eq!(events.len(), 4);
    assert!((events[0].time - 0.0).abs() < 1e-9);
    assert!((events[1].time - 0.125).abs() < 1e-9);
}

#[test]
fn transform_fast() {
    let comp = make_comp(seq(vec![
        note(PitchClass::C, 4),
        note(PitchClass::D, 4),
    ]));
    let mut events = evaluate_pattern(&comp, 0);
    apply_transform(&mut events, Transform::Fast(2.0));
    // With fast 2, events at 0.0, 0.5 become 0.0, 0.0 (wrapped).
    assert_eq!(events.len(), 2);
}

#[test]
fn transform_rev() {
    let comp = make_comp(seq(vec![
        note(PitchClass::C, 4),
        note(PitchClass::D, 4),
        note(PitchClass::E, 4),
    ]));
    let mut events = evaluate_pattern(&comp, 0);
    apply_transform(&mut events, Transform::Rev);

    // Reversed order: E, D, C.
    assert_eq!(events[0].note.pitch_class, PitchClass::E);
    assert_eq!(events[1].note.pitch_class, PitchClass::D);
    assert_eq!(events[2].note.pitch_class, PitchClass::C);
}

#[test]
fn amp_pattern_applied() {
    let comp = Composition {
        name: "test".into(),
        bpm: 120.0,
        cycle_length: 1.0,
        stems: vec![Stem {
            name: "melody".into(),
            voices: vec![Voice {
                note_pattern: seq(vec![
                    note(PitchClass::C, 4),
                    note(PitchClass::D, 4),
                    note(PitchClass::E, 4),
                ]),
                amp_pattern: Some(seq(vec![
                    Pattern::Atom(PatternAtom::Number(0.8)),
                    Pattern::Atom(PatternAtom::Number(0.5)),
                    Pattern::Atom(PatternAtom::Number(0.3)),
                ])),
                legato_pattern: None,
            }],
        }],
        metadata: CompositionMeta::default(),
    };
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 3);
    assert!((events[0].amplitude - 0.8).abs() < 1e-9);
    assert!((events[1].amplitude - 0.5).abs() < 1e-9);
    assert!((events[2].amplitude - 0.3).abs() < 1e-9);
}

#[test]
fn legato_pattern_applied() {
    let comp = Composition {
        name: "test".into(),
        bpm: 120.0,
        cycle_length: 1.0,
        stems: vec![Stem {
            name: "melody".into(),
            voices: vec![Voice {
                note_pattern: seq(vec![
                    note(PitchClass::C, 4),
                    note(PitchClass::D, 4),
                ]),
                amp_pattern: None,
                legato_pattern: Some(seq(vec![
                    Pattern::Atom(PatternAtom::Number(1.0)),
                    Pattern::Atom(PatternAtom::Number(0.5)),
                ])),
            }],
        }],
        metadata: CompositionMeta::default(),
    };
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 2);
    assert!((events[0].legato - 1.0).abs() < 1e-9);
    assert!((events[1].legato - 0.5).abs() < 1e-9);
}

#[test]
fn multi_stem_events_have_correct_stem_index() {
    let comp = Composition {
        name: "test".into(),
        bpm: 120.0,
        cycle_length: 1.0,
        stems: vec![
            Stem {
                name: "melody".into(),
                voices: vec![Voice {
                    note_pattern: note(PitchClass::C, 5),
                    amp_pattern: None,
                    legato_pattern: None,
                }],
            },
            Stem {
                name: "bass".into(),
                voices: vec![Voice {
                    note_pattern: note(PitchClass::C, 2),
                    amp_pattern: None,
                    legato_pattern: None,
                }],
            },
        ],
        metadata: CompositionMeta::default(),
    };
    let events = evaluate_pattern(&comp, 0);
    assert_eq!(events.len(), 2);
    let melody_events: Vec<_> = events.iter().filter(|e| e.stem_index == 0).collect();
    let bass_events: Vec<_> = events.iter().filter(|e| e.stem_index == 1).collect();
    assert_eq!(melody_events.len(), 1);
    assert_eq!(bass_events.len(), 1);
    assert_eq!(melody_events[0].note.pitch_class, PitchClass::C);
    assert_eq!(melody_events[0].note.octave, 5);
    assert_eq!(bass_events[0].note.pitch_class, PitchClass::C);
    assert_eq!(bass_events[0].note.octave, 2);
}
