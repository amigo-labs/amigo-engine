use amigo_tidal_parser::lexer::{tokenize, Keyword, Token};
use amigo_tidal_parser::{NoteValue, PitchClass};

#[test]
fn note_parsing_basic() {
    let tokens = tokenize("c4 d4 e4 f4 g4 a4 b4").unwrap();
    assert_eq!(tokens.len(), 7);
    assert_eq!(tokens[0], Token::Note(NoteValue::new(PitchClass::C, 4)));
    assert_eq!(tokens[6], Token::Note(NoteValue::new(PitchClass::B, 4)));
}

#[test]
fn note_parsing_sharps() {
    let tokens = tokenize("cs4 ds5 fs3 gs2 as1").unwrap();
    assert_eq!(tokens.len(), 5);
    assert_eq!(tokens[0], Token::Note(NoteValue::new(PitchClass::Cs, 4)));
    assert_eq!(tokens[1], Token::Note(NoteValue::new(PitchClass::Ds, 5)));
    assert_eq!(tokens[2], Token::Note(NoteValue::new(PitchClass::Fs, 3)));
}

#[test]
fn note_parsing_flats() {
    let tokens = tokenize("eb4 bb3 db5").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0], Token::Note(NoteValue::new(PitchClass::Ds, 4)));
    assert_eq!(tokens[1], Token::Note(NoteValue::new(PitchClass::As, 3)));
    assert_eq!(tokens[2], Token::Note(NoteValue::new(PitchClass::Cs, 5)));
}

#[test]
fn rest_token() {
    let tokens = tokenize("~").unwrap();
    assert_eq!(tokens, vec![Token::Rest]);
}

#[test]
fn number_tokens() {
    let tokens = tokenize("0.8 1.0 0 42").unwrap();
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], Token::Number(0.8));
    assert_eq!(tokens[1], Token::Number(1.0));
    assert_eq!(tokens[2], Token::Number(0.0));
    assert_eq!(tokens[3], Token::Number(42.0));
}

#[test]
fn operator_tokens() {
    let tokens = tokenize("* ! / [ ] # $ ,").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Star,
            Token::Bang,
            Token::Slash,
            Token::BracketOpen,
            Token::BracketClose,
            Token::Hash,
            Token::Dollar,
            Token::Comma,
        ]
    );
}

#[test]
fn keyword_stack() {
    let tokens = tokenize("stack").unwrap();
    assert_eq!(tokens, vec![Token::Keyword(Keyword::Stack)]);
}

#[test]
fn keyword_transforms() {
    let tokens = tokenize("slow fast rev").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Keyword(Keyword::Slow),
            Token::Keyword(Keyword::Fast),
            Token::Keyword(Keyword::Rev),
        ]
    );
}

#[test]
fn keyword_params() {
    let tokens = tokenize("n amp legato").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Keyword(Keyword::N),
            Token::Keyword(Keyword::Amp),
            Token::Keyword(Keyword::Legato),
        ]
    );
}

#[test]
fn sample_names() {
    let tokens = tokenize("bd sd hh cp").unwrap();
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0], Token::Sample("bd".into()));
    assert_eq!(tokens[1], Token::Sample("sd".into()));
    assert_eq!(tokens[2], Token::Sample("hh".into()));
    assert_eq!(tokens[3], Token::Sample("cp".into()));
}

#[test]
fn comments_are_stripped() {
    let tokens = tokenize("c4 -- this is a comment\nd4").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], Token::Note(NoteValue::new(PitchClass::C, 4)));
    assert_eq!(tokens[1], Token::Note(NoteValue::new(PitchClass::D, 4)));
}

#[test]
fn full_voice_definition_tokenizes() {
    let input = r#"d1 $ slow 8 $ n "c5 d5 e5 ~ g5" # amp "0.8 0.7 0.9 0 0.8""#;
    let tokens = tokenize(input).unwrap();
    assert!(tokens.len() > 10);
    // Verify key structural tokens are present.
    assert!(tokens.contains(&Token::Dollar));
    assert!(tokens.contains(&Token::Keyword(Keyword::Slow)));
    assert!(tokens.contains(&Token::Keyword(Keyword::N)));
    assert!(tokens.contains(&Token::Hash));
    assert!(tokens.contains(&Token::Keyword(Keyword::Amp)));
}
