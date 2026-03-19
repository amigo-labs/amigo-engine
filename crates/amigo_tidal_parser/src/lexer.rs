use crate::ast::{NoteValue, PitchClass};

/// Token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Note literal like c4, ds5, bf3.
    Note(NoteValue),
    /// Rest (~).
    Rest,
    /// Numeric literal (0.8, 1.0, 16, etc.).
    Number(f64),
    /// Drum/sample name (bd, sd, hh, etc.) — alphanumeric not matching note pattern.
    Sample(String),
    /// Quoted string delimiter (").
    Quote,
    /// Star operator (*).
    Star,
    /// Bang operator (!).
    Bang,
    /// Forward slash (/).
    Slash,
    /// Opening bracket ([).
    BracketOpen,
    /// Closing bracket (]).
    BracketClose,
    /// Hash for parameter chaining (#).
    Hash,
    /// Dollar sign for transforms ($).
    Dollar,
    /// Comma (layer separator in stack).
    Comma,
    /// Keyword tokens.
    Keyword(Keyword),
}

/// Recognized keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Stack,
    Slow,
    Fast,
    Rev,
    /// Note pattern parameter (n).
    N,
    /// Amplitude parameter.
    Amp,
    /// Legato parameter.
    Legato,
    /// Voice label d1..d9.
    Voice(u8),
}

/// Lexer error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LexError {
    #[error("unexpected character '{ch}' at position {pos}")]
    UnexpectedChar { ch: char, pos: usize },
    #[error("unterminated string at position {pos}")]
    UnterminatedString { pos: usize },
    #[error("invalid number '{text}' at position {pos}")]
    InvalidNumber { text: String, pos: usize },
}

/// Tokenize a TidalCycles mini-notation string.
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Skip whitespace.
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Line comments (-- ...).
        if ch == '-' && i + 1 < len && chars[i + 1] == '-' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        match ch {
            '~' => {
                tokens.push(Token::Rest);
                i += 1;
            }
            '"' => {
                tokens.push(Token::Quote);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '!' => {
                tokens.push(Token::Bang);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '[' => {
                tokens.push(Token::BracketOpen);
                i += 1;
            }
            ']' => {
                tokens.push(Token::BracketClose);
                i += 1;
            }
            '#' => {
                tokens.push(Token::Hash);
                i += 1;
            }
            '$' => {
                tokens.push(Token::Dollar);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            _ if ch.is_ascii_digit()
                || (ch == '-' && i + 1 < len && chars[i + 1].is_ascii_digit()) =>
            {
                let start = i;
                if ch == '-' {
                    i += 1;
                }
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text = &input[start..i];
                let val: f64 = text.parse().map_err(|_| LexError::InvalidNumber {
                    text: text.to_string(),
                    pos: start,
                })?;
                tokens.push(Token::Number(val));
            }
            _ if ch.is_ascii_alphabetic() => {
                let start = i;
                while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word = &input[start..i];

                // Try keyword first.
                if let Some(kw) = match_keyword(word) {
                    tokens.push(Token::Keyword(kw));
                } else if let Some(note) = try_parse_note(word) {
                    tokens.push(Token::Note(note));
                } else {
                    tokens.push(Token::Sample(word.to_string()));
                }
            }
            _ => {
                return Err(LexError::UnexpectedChar { ch, pos: i });
            }
        }
    }

    Ok(tokens)
}

fn match_keyword(word: &str) -> Option<Keyword> {
    match word {
        "stack" => Some(Keyword::Stack),
        "slow" => Some(Keyword::Slow),
        "fast" => Some(Keyword::Fast),
        "rev" => Some(Keyword::Rev),
        "n" => Some(Keyword::N),
        "amp" => Some(Keyword::Amp),
        "legato" => Some(Keyword::Legato),
        // Note: d1-d9 are NOT matched here because they conflict with notes
        // like d4 (= D octave 4). The parser handles voice labels by checking
        // if the first token is a Note(D, 1..=9).
        _ => None,
    }
}

/// Try to parse a note like c4, ds5, bf3, e2.
fn try_parse_note(word: &str) -> Option<NoteValue> {
    let lower = word.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    // First char must be a-g.
    if !matches!(bytes[0], b'a'..=b'g') {
        return None;
    }

    // Find where the pitch name ends and octave begins.
    let mut split = 1;
    if split < bytes.len() && matches!(bytes[split], b's' | b'b' | b'f') {
        split += 1;
    }

    let pitch_str = &lower[..split];
    let octave_str = &lower[split..];

    let pitch_class = PitchClass::from_str_name(pitch_str)?;

    if octave_str.is_empty() {
        return None;
    }

    // Octave can be negative.
    let octave: i8 = octave_str.parse().ok()?;

    Some(NoteValue::new(pitch_class, octave))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_sequence() {
        let tokens = tokenize("c4 d4 e4").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0], Token::Note(NoteValue::new(PitchClass::C, 4)));
        assert_eq!(tokens[1], Token::Note(NoteValue::new(PitchClass::D, 4)));
        assert_eq!(tokens[2], Token::Note(NoteValue::new(PitchClass::E, 4)));
    }

    #[test]
    fn tokenize_rest_and_numbers() {
        let tokens = tokenize("~ 0.8 1.0").unwrap();
        assert_eq!(tokens[0], Token::Rest);
        assert_eq!(tokens[1], Token::Number(0.8));
        assert_eq!(tokens[2], Token::Number(1.0));
    }

    #[test]
    fn tokenize_operators() {
        let tokens = tokenize("c4*4 [d4 e4] c4!3 g4/2").unwrap();
        assert!(tokens.contains(&Token::Star));
        assert!(tokens.contains(&Token::Bang));
        assert!(tokens.contains(&Token::Slash));
        assert!(tokens.contains(&Token::BracketOpen));
        assert!(tokens.contains(&Token::BracketClose));
    }

    #[test]
    fn tokenize_keywords() {
        let tokens = tokenize("d1 $ slow 2 $ stack").unwrap();
        // d1 is lexed as Note(D, 1) — parser distinguishes voice labels.
        assert_eq!(tokens[0], Token::Note(NoteValue::new(PitchClass::D, 1)));
        assert_eq!(tokens[1], Token::Dollar);
        assert_eq!(tokens[2], Token::Keyword(Keyword::Slow));
        assert_eq!(tokens[3], Token::Number(2.0));
        assert_eq!(tokens[4], Token::Dollar);
        assert_eq!(tokens[5], Token::Keyword(Keyword::Stack));
    }

    #[test]
    fn tokenize_sharps_and_flats() {
        let tokens = tokenize("cs4 ds5 bf3 eb2").unwrap();
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0], Token::Note(NoteValue::new(PitchClass::Cs, 4)));
        assert_eq!(tokens[1], Token::Note(NoteValue::new(PitchClass::Ds, 5)));
        assert_eq!(tokens[2], Token::Note(NoteValue::new(PitchClass::As, 3)));
        assert_eq!(tokens[3], Token::Note(NoteValue::new(PitchClass::Ds, 2)));
    }

    #[test]
    fn tokenize_quoted_pattern() {
        let tokens = tokenize(r#"n "c4 d4 e4" # amp "0.8 0.7 0.9""#).unwrap();
        assert_eq!(tokens[0], Token::Keyword(Keyword::N));
        assert_eq!(tokens[1], Token::Quote);
        // Notes inside quotes.
        assert_eq!(tokens[2], Token::Note(NoteValue::new(PitchClass::C, 4)));
    }

    #[test]
    fn tokenize_drum_samples() {
        let tokens = tokenize("bd sd hh").unwrap();
        assert_eq!(tokens[0], Token::Sample("bd".to_string()));
        assert_eq!(tokens[1], Token::Sample("sd".to_string()));
        assert_eq!(tokens[2], Token::Sample("hh".to_string()));
    }

    #[test]
    fn tokenize_comments_skipped() {
        let tokens = tokenize("c4 d4 -- this is a comment\ne4").unwrap();
        assert_eq!(tokens.len(), 3);
    }

    #[test]
    fn tokenize_hash_and_params() {
        let tokens = tokenize(r#"# amp "0.8 0.7""#).unwrap();
        assert_eq!(tokens[0], Token::Hash);
        assert_eq!(tokens[1], Token::Keyword(Keyword::Amp));
    }
}
