use crate::ast::*;

/// A time-resolved note event within a single cycle.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NoteEvent {
    /// Position within the cycle (0.0 .. 1.0).
    pub time: f64,
    /// Duration relative to the cycle (0.0 .. 1.0).
    pub duration: f64,
    /// The note to play.
    pub note: NoteValue,
    /// Amplitude (0.0 .. 1.0).
    pub amplitude: f64,
    /// Legato multiplier for duration.
    pub legato: f64,
    /// Which stem this event belongs to.
    pub stem_index: usize,
}

/// Evaluate a composition for a given cycle, producing time-resolved events.
pub fn evaluate_pattern(composition: &Composition, cycle: u64) -> Vec<NoteEvent> {
    let mut events = Vec::new();

    for (stem_idx, stem) in composition.stems.iter().enumerate() {
        for voice in &stem.voices {
            let note_events = eval_pattern(&voice.note_pattern, 0.0, 1.0, cycle);
            let amp_values = voice
                .amp_pattern
                .as_ref()
                .map(|p| eval_values(p, 0.0, 1.0, cycle));
            let legato_values = voice
                .legato_pattern
                .as_ref()
                .map(|p| eval_values(p, 0.0, 1.0, cycle));

            for (i, (time, duration, atom)) in note_events.iter().enumerate() {
                if let PatternAtom::Note(note) = atom {
                    let amplitude = amp_values
                        .as_ref()
                        .and_then(|vals| vals.get(i))
                        .copied()
                        .unwrap_or(1.0);
                    let legato = legato_values
                        .as_ref()
                        .and_then(|vals| vals.get(i))
                        .copied()
                        .unwrap_or(1.0);

                    events.push(NoteEvent {
                        time: *time,
                        duration: *duration,
                        note: *note,
                        amplitude,
                        legato,
                        stem_index: stem_idx,
                    });
                }
                // Rests produce no event.
            }
        }
    }

    events.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    events
}

/// Apply a transform to a list of events.
pub fn apply_transform(events: &mut Vec<NoteEvent>, transform: Transform) {
    match transform {
        Transform::Slow(factor) => {
            for ev in events.iter_mut() {
                ev.time /= factor;
                ev.duration /= factor;
            }
            // Only keep events that fall within cycle 0.0..1.0
            events.retain(|ev| ev.time < 1.0);
        }
        Transform::Fast(factor) => {
            for ev in events.iter_mut() {
                ev.time *= factor;
                ev.duration *= factor;
            }
            // Keep within bounds and wrap.
            for ev in events.iter_mut() {
                ev.time %= 1.0;
            }
        }
        Transform::Rev => {
            for ev in events.iter_mut() {
                ev.time = 1.0 - ev.time - ev.duration;
                if ev.time < 0.0 {
                    ev.time = 0.0;
                }
            }
            events.sort_by(|a, b| {
                a.time
                    .partial_cmp(&b.time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
}

/// Internal: evaluate a pattern tree into (time, duration, atom) triples.
fn eval_pattern(
    pattern: &Pattern,
    start: f64,
    span: f64,
    _cycle: u64,
) -> Vec<(f64, f64, PatternAtom)> {
    match pattern {
        Pattern::Atom(atom) => {
            vec![(start, span, atom.clone())]
        }
        Pattern::Sequence(elements) => {
            let n = elements.len() as f64;
            let slot_size = span / n;
            let mut results = Vec::new();
            for (i, elem) in elements.iter().enumerate() {
                let slot_start = start + i as f64 * slot_size;
                results.extend(eval_pattern(elem, slot_start, slot_size, _cycle));
            }
            results
        }
        Pattern::Group(elements) => {
            // Same as Sequence but within the parent's single slot.
            let n = elements.len() as f64;
            let slot_size = span / n;
            let mut results = Vec::new();
            for (i, elem) in elements.iter().enumerate() {
                let slot_start = start + i as f64 * slot_size;
                results.extend(eval_pattern(elem, slot_start, slot_size, _cycle));
            }
            results
        }
        Pattern::Repeat(inner, count) => {
            // Same event repeated n times in the same time slot.
            let n = *count as f64;
            let slot_size = span / n;
            let mut results = Vec::new();
            for i in 0..*count {
                let slot_start = start + i as f64 * slot_size;
                results.extend(eval_pattern(inner, slot_start, slot_size, _cycle));
            }
            results
        }
        Pattern::Replicate(inner, count) => {
            // Same as Repeat for evaluation purposes (in TidalCycles, replicate
            // differs from repeat only in pattern structure, not timing).
            let n = *count as f64;
            let slot_size = span / n;
            let mut results = Vec::new();
            for i in 0..*count {
                let slot_start = start + i as f64 * slot_size;
                results.extend(eval_pattern(inner, slot_start, slot_size, _cycle));
            }
            results
        }
        Pattern::SlowDiv(inner, divisor) => {
            // Only produce events on every nth cycle.
            let d = *divisor as u64;
            if _cycle.is_multiple_of(d) {
                eval_pattern(inner, start, span, _cycle / d)
            } else {
                Vec::new()
            }
        }
        Pattern::Stack(layers) => {
            // All layers play simultaneously in the same time span.
            let mut results = Vec::new();
            for layer in layers {
                results.extend(eval_pattern(layer, start, span, _cycle));
            }
            results
        }
    }
}

/// Internal: evaluate a numeric value pattern into a list of values.
fn eval_values(pattern: &Pattern, start: f64, span: f64, cycle: u64) -> Vec<f64> {
    let events = eval_pattern(pattern, start, span, cycle);
    events
        .into_iter()
        .map(|(_, _, atom)| match atom {
            PatternAtom::Number(n) => n,
            PatternAtom::Rest => 0.0,
            _ => 1.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{NoteValue, PitchClass};

    fn note(pc: PitchClass, oct: i8) -> Pattern {
        Pattern::Atom(PatternAtom::Note(NoteValue::new(pc, oct)))
    }

    fn seq(patterns: Vec<Pattern>) -> Pattern {
        Pattern::Sequence(patterns)
    }

    #[test]
    fn eval_single_note() {
        let pat = note(PitchClass::C, 4);
        let events = eval_pattern(&pat, 0.0, 1.0, 0);
        assert_eq!(events.len(), 1);
        assert!((events[0].0 - 0.0).abs() < 1e-9);
        assert!((events[0].1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn eval_sequence_even_timing() {
        let pat = seq(vec![
            note(PitchClass::C, 4),
            note(PitchClass::D, 4),
            note(PitchClass::E, 4),
        ]);
        let events = eval_pattern(&pat, 0.0, 1.0, 0);
        assert_eq!(events.len(), 3);
        assert!((events[0].0 - 0.0).abs() < 1e-9);
        assert!((events[1].0 - 1.0 / 3.0).abs() < 1e-9);
        assert!((events[2].0 - 2.0 / 3.0).abs() < 1e-9);

        let expected_dur = 1.0 / 3.0;
        for ev in &events {
            assert!((ev.1 - expected_dur).abs() < 1e-9);
        }
    }

    #[test]
    fn eval_group_subdivides_slot() {
        // "[c4 d4] e4" -> c4 at 0.0 (dur 0.25), d4 at 0.25 (dur 0.25), e4 at 0.5 (dur 0.5)
        let pat = seq(vec![
            Pattern::Group(vec![note(PitchClass::C, 4), note(PitchClass::D, 4)]),
            note(PitchClass::E, 4),
        ]);
        let events = eval_pattern(&pat, 0.0, 1.0, 0);
        assert_eq!(events.len(), 3);
        assert!((events[0].0 - 0.0).abs() < 1e-9);
        assert!((events[0].1 - 0.25).abs() < 1e-9);
        assert!((events[1].0 - 0.25).abs() < 1e-9);
        assert!((events[2].0 - 0.5).abs() < 1e-9);
        assert!((events[2].1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn eval_repeat() {
        let pat = Pattern::Repeat(Box::new(note(PitchClass::C, 4)), 4);
        let events = eval_pattern(&pat, 0.0, 1.0, 0);
        assert_eq!(events.len(), 4);
        for (i, ev) in events.iter().enumerate() {
            assert!((ev.0 - i as f64 * 0.25).abs() < 1e-9);
            assert!((ev.1 - 0.25).abs() < 1e-9);
        }
    }

    #[test]
    fn eval_rest_produces_no_note_event() {
        let comp = Composition {
            name: "test".into(),
            bpm: 120.0,
            cycle_length: 1.0,
            stems: vec![Stem {
                name: "melody".into(),
                voices: vec![Voice {
                    note_pattern: seq(vec![
                        note(PitchClass::C, 4),
                        Pattern::Atom(PatternAtom::Rest),
                        note(PitchClass::E, 4),
                    ]),
                    amp_pattern: None,
                    legato_pattern: None,
                }],
            }],
            metadata: CompositionMeta::default(),
        };
        let events = evaluate_pattern(&comp, 0);
        // Only 2 note events (rest is skipped).
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn eval_stack_layers_overlap() {
        let pat = Pattern::Stack(vec![
            note(PitchClass::C, 4),
            note(PitchClass::E, 4),
            note(PitchClass::G, 4),
        ]);
        let events = eval_pattern(&pat, 0.0, 1.0, 0);
        assert_eq!(events.len(), 3);
        // All at same time.
        for ev in &events {
            assert!((ev.0 - 0.0).abs() < 1e-9);
        }
    }

    #[test]
    fn eval_slow_div_cycle_dependent() {
        let pat = Pattern::SlowDiv(Box::new(note(PitchClass::C, 4)), 2);
        // Plays on even cycles.
        let events_0 = eval_pattern(&pat, 0.0, 1.0, 0);
        assert_eq!(events_0.len(), 1);
        // Silent on odd cycles.
        let events_1 = eval_pattern(&pat, 0.0, 1.0, 1);
        assert_eq!(events_1.len(), 0);
    }

    #[test]
    fn apply_transform_rev() {
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
                    amp_pattern: None,
                    legato_pattern: None,
                }],
            }],
            metadata: CompositionMeta::default(),
        };
        let mut events = evaluate_pattern(&comp, 0);
        apply_transform(&mut events, Transform::Rev);

        // Reversed: E should come first, then D, then C.
        assert_eq!(events[0].note.pitch_class, PitchClass::E);
        assert_eq!(events[1].note.pitch_class, PitchClass::D);
        assert_eq!(events[2].note.pitch_class, PitchClass::C);
    }

    #[test]
    fn eval_with_amp_pattern() {
        let comp = Composition {
            name: "test".into(),
            bpm: 120.0,
            cycle_length: 1.0,
            stems: vec![Stem {
                name: "melody".into(),
                voices: vec![Voice {
                    note_pattern: seq(vec![note(PitchClass::C, 4), note(PitchClass::D, 4)]),
                    amp_pattern: Some(seq(vec![
                        Pattern::Atom(PatternAtom::Number(0.8)),
                        Pattern::Atom(PatternAtom::Number(0.5)),
                    ])),
                    legato_pattern: None,
                }],
            }],
            metadata: CompositionMeta::default(),
        };
        let events = evaluate_pattern(&comp, 0);
        assert_eq!(events.len(), 2);
        assert!((events[0].amplitude - 0.8).abs() < 1e-9);
        assert!((events[1].amplitude - 0.5).abs() < 1e-9);
    }
}
