use crate::ast::*;
use crate::lexer::{Keyword, Token};

/// Parser error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("unexpected token: {0:?}")]
    UnexpectedToken(Token),
    #[error("expected {expected}, found {found:?}")]
    Expected { expected: String, found: Token },
    #[error("expected closing bracket")]
    UnclosedBracket,
    #[error("expected closing quote")]
    UnclosedQuote,
    #[error("integer expected after operator")]
    ExpectedInteger,
}

/// Parse a complete TidalCycles voice definition.
///
/// Expected input format: `d1 $ slow 8 $ stack [ ... ] # amp "..." # legato "..."`
/// or simpler: `d1 $ n "c4 d4 e4" # amp "0.8 0.7 0.9"`
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        match self.advance() {
            Some(ref tok) if tok == expected => Ok(()),
            Some(tok) => Err(ParseError::Expected {
                expected: format!("{expected:?}"),
                found: tok,
            }),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// Parse a full voice definition, returning Voice + transforms.
    pub fn parse_voice_def(&mut self) -> Result<(Voice, Vec<Transform>, f64), ParseError> {
        let mut transforms = Vec::new();
        let mut cycle_length = 1.0;

        // Skip optional voice label (d1-d9, lexed as Note(D, 1..=9)).
        if let Some(Token::Note(nv)) = self.peek() {
            if nv.pitch_class == crate::ast::PitchClass::D && (1..=9).contains(&nv.octave) {
                // Check if next token is $ (confirming this is a voice label, not a note).
                if self.tokens.get(self.pos + 1) == Some(&Token::Dollar) {
                    self.advance(); // consume the voice label
                }
            }
        }

        // Collect transforms: $ slow 8 $ fast 2 $ rev
        // The last $ before a non-transform keyword (n, stack, etc.) ends the chain.
        while matches!(self.peek(), Some(Token::Dollar)) {
            // Peek ahead to see if next-next token is a transform keyword.
            let next = self.tokens.get(self.pos + 1);
            match next {
                Some(Token::Keyword(Keyword::Slow)) => {
                    self.advance(); // consume $
                    self.advance(); // consume slow
                    let n = self.expect_number()?;
                    transforms.push(Transform::Slow(n));
                    cycle_length *= n;
                }
                Some(Token::Keyword(Keyword::Fast)) => {
                    self.advance(); // consume $
                    self.advance(); // consume fast
                    let n = self.expect_number()?;
                    transforms.push(Transform::Fast(n));
                    cycle_length /= n;
                }
                Some(Token::Keyword(Keyword::Rev)) => {
                    self.advance(); // consume $
                    self.advance(); // consume rev
                    transforms.push(Transform::Rev);
                }
                _ => {
                    // Not a transform — this $ introduces the pattern.
                    self.advance(); // consume the final $
                    break;
                }
            }
        }

        // Parse the main pattern (could be stack or a single pattern).
        let note_pattern = self.parse_top_pattern()?;

        // Parse optional parameter chains: # amp "..." # legato "..."
        let mut amp_pattern = None;
        let mut legato_pattern = None;

        while matches!(self.peek(), Some(Token::Hash)) {
            self.advance(); // consume #
            match self.advance() {
                Some(Token::Keyword(Keyword::Amp)) => {
                    amp_pattern = Some(self.parse_quoted_pattern()?);
                }
                Some(Token::Keyword(Keyword::Legato)) => {
                    legato_pattern = Some(self.parse_quoted_pattern()?);
                }
                Some(tok) => return Err(ParseError::UnexpectedToken(tok)),
                None => return Err(ParseError::UnexpectedEof),
            }
        }

        let voice = Voice {
            note_pattern,
            amp_pattern,
            legato_pattern,
        };

        Ok((voice, transforms, cycle_length))
    }

    /// Parse top-level pattern: either `stack [...]` or `n "..."` or just a quoted pattern.
    fn parse_top_pattern(&mut self) -> Result<Pattern, ParseError> {
        match self.peek() {
            Some(Token::Keyword(Keyword::Stack)) => {
                self.advance(); // consume stack
                self.parse_stack()
            }
            Some(Token::Keyword(Keyword::N)) => {
                self.advance(); // consume n
                self.parse_quoted_pattern()
            }
            Some(Token::Quote) => self.parse_quoted_pattern(),
            _ => {
                // Try to parse inline pattern atoms until we hit # or $
                self.parse_sequence_until_delimiter()
            }
        }
    }

    /// Parse `[layer1, layer2, ...]` stack notation.
    fn parse_stack(&mut self) -> Result<Pattern, ParseError> {
        self.expect(&Token::BracketOpen)?;
        let mut layers = Vec::new();

        loop {
            let pattern = self.parse_inner_voice_or_pattern()?;
            layers.push(pattern);

            match self.peek() {
                Some(Token::Comma) => {
                    self.advance();
                }
                Some(Token::BracketClose) => {
                    self.advance();
                    break;
                }
                None => return Err(ParseError::UnclosedBracket),
                _ => {
                    // Could be next item without comma in some notations.
                    if matches!(self.peek(), Some(Token::BracketClose)) {
                        self.advance();
                        break;
                    }
                }
            }
        }

        Ok(Pattern::Stack(layers))
    }

    /// Parse a pattern inside a stack layer — can have its own `n "..." # amp "..."`.
    /// The `# param "..."` chains are consumed but discarded at this level
    /// (they are only relevant when parsing full voice definitions).
    fn parse_inner_voice_or_pattern(&mut self) -> Result<Pattern, ParseError> {
        let pattern = match self.peek() {
            Some(Token::Keyword(Keyword::N)) => {
                self.advance();
                self.parse_quoted_pattern()?
            }
            Some(Token::Quote) => self.parse_quoted_pattern()?,
            _ => self.parse_sequence_until_delimiter()?,
        };

        // Consume optional parameter chains (# amp "...", # legato "...").
        while matches!(self.peek(), Some(Token::Hash)) {
            self.advance(); // consume #
            match self.peek() {
                Some(Token::Keyword(Keyword::Amp | Keyword::Legato)) => {
                    self.advance(); // consume keyword
                    if matches!(self.peek(), Some(Token::Quote)) {
                        let _ = self.parse_quoted_pattern()?; // consume and discard
                    }
                }
                _ => break,
            }
        }

        Ok(pattern)
    }

    /// Parse a quoted pattern: "c4 d4 e4 ~ g4"
    fn parse_quoted_pattern(&mut self) -> Result<Pattern, ParseError> {
        if matches!(self.peek(), Some(Token::Quote)) {
            self.advance(); // consume opening "
        }

        let mut elements = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Quote) => {
                    self.advance(); // consume closing "
                    break;
                }
                None => return Err(ParseError::UnclosedQuote),
                _ => {
                    let atom = self.parse_pattern_element()?;
                    elements.push(atom);
                }
            }
        }

        if elements.len() == 1 {
            Ok(elements.into_iter().next().unwrap())
        } else {
            Ok(Pattern::Sequence(elements))
        }
    }

    /// Parse a sequence of pattern atoms until we hit a delimiter (# $ , ] or EOF).
    fn parse_sequence_until_delimiter(&mut self) -> Result<Pattern, ParseError> {
        let mut elements = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Hash | Token::Dollar | Token::Comma | Token::BracketClose) | None => {
                    break
                }
                _ => {
                    let elem = self.parse_pattern_element()?;
                    elements.push(elem);
                }
            }
        }

        if elements.is_empty() {
            return Err(self
                .peek()
                .map(|t| ParseError::UnexpectedToken(t.clone()))
                .unwrap_or(ParseError::UnexpectedEof));
        }

        if elements.len() == 1 {
            Ok(elements.into_iter().next().unwrap())
        } else {
            Ok(Pattern::Sequence(elements))
        }
    }

    /// Parse a single pattern element with optional postfix operators (*n, !n, /n).
    fn parse_pattern_element(&mut self) -> Result<Pattern, ParseError> {
        let base = self.parse_atom()?;
        self.parse_postfix(base)
    }

    /// Parse a base atom.
    fn parse_atom(&mut self) -> Result<Pattern, ParseError> {
        match self.peek() {
            Some(Token::Note(_)) => {
                if let Some(Token::Note(note)) = self.advance() {
                    Ok(Pattern::Atom(PatternAtom::Note(note)))
                } else {
                    unreachable!()
                }
            }
            Some(Token::Rest) => {
                self.advance();
                Ok(Pattern::Atom(PatternAtom::Rest))
            }
            Some(Token::Number(_)) => {
                if let Some(Token::Number(n)) = self.advance() {
                    Ok(Pattern::Atom(PatternAtom::Number(n)))
                } else {
                    unreachable!()
                }
            }
            Some(Token::Sample(_)) => {
                if let Some(Token::Sample(s)) = self.advance() {
                    Ok(Pattern::Atom(PatternAtom::Sample(s)))
                } else {
                    unreachable!()
                }
            }
            Some(Token::BracketOpen) => {
                self.advance();
                let mut elements = Vec::new();
                loop {
                    match self.peek() {
                        Some(Token::BracketClose) => {
                            self.advance();
                            break;
                        }
                        None => return Err(ParseError::UnclosedBracket),
                        _ => {
                            let elem = self.parse_pattern_element()?;
                            elements.push(elem);
                        }
                    }
                }
                // Apply postfix to the group as a whole.
                let group = Pattern::Group(elements);
                Ok(group)
            }
            Some(tok) => Err(ParseError::UnexpectedToken(tok.clone())),
            None => Err(ParseError::UnexpectedEof),
        }
    }

    /// Parse postfix operators: *n, !n, /n.
    fn parse_postfix(&mut self, base: Pattern) -> Result<Pattern, ParseError> {
        match self.peek() {
            Some(Token::Star) => {
                self.advance();
                let n = self.expect_positive_int()?;
                let result = Pattern::Repeat(Box::new(base), n);
                self.parse_postfix(result)
            }
            Some(Token::Bang) => {
                self.advance();
                let n = self.expect_positive_int()?;
                let result = Pattern::Replicate(Box::new(base), n);
                self.parse_postfix(result)
            }
            Some(Token::Slash) => {
                self.advance();
                let n = self.expect_positive_int()?;
                let result = Pattern::SlowDiv(Box::new(base), n);
                self.parse_postfix(result)
            }
            _ => Ok(base),
        }
    }

    fn expect_number(&mut self) -> Result<f64, ParseError> {
        match self.advance() {
            Some(Token::Number(n)) => Ok(n),
            _ => Err(ParseError::ExpectedInteger),
        }
    }

    fn expect_positive_int(&mut self) -> Result<u32, ParseError> {
        match self.advance() {
            Some(Token::Number(n)) if n > 0.0 && n == n.floor() => Ok(n as u32),
            _ => Err(ParseError::ExpectedInteger),
        }
    }
}

/// Convenience: parse a quoted mini-notation string directly into a Pattern.
pub fn parse_mini(input: &str) -> Result<Pattern, ParseError> {
    let tokens = crate::lexer::tokenize(input).map_err(|e| ParseError::Expected {
        expected: "valid token".into(),
        found: Token::Sample(e.to_string()),
    })?;
    let mut parser = Parser::new(tokens);
    parser.parse_sequence_until_delimiter()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{NoteValue, PitchClass};
    use crate::lexer::tokenize;

    fn parse_quoted(input: &str) -> Pattern {
        let tokens = tokenize(input).unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_quoted_pattern().unwrap()
    }

    #[test]
    fn parse_simple_sequence() {
        let pat = parse_quoted(r#""c4 d4 e4""#);
        match pat {
            Pattern::Sequence(elems) => {
                assert_eq!(elems.len(), 3);
                assert_eq!(
                    elems[0],
                    Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
                );
            }
            _ => panic!("expected Sequence, got {pat:?}"),
        }
    }

    #[test]
    fn parse_with_rest() {
        let pat = parse_quoted(r#""c4 ~ e4""#);
        match pat {
            Pattern::Sequence(elems) => {
                assert_eq!(elems.len(), 3);
                assert_eq!(elems[1], Pattern::Atom(PatternAtom::Rest));
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parse_group() {
        let pat = parse_quoted(r#""[c4 d4] e4""#);
        match pat {
            Pattern::Sequence(elems) => {
                assert_eq!(elems.len(), 2);
                match &elems[0] {
                    Pattern::Group(inner) => assert_eq!(inner.len(), 2),
                    other => panic!("expected Group, got {other:?}"),
                }
            }
            _ => panic!("expected Sequence"),
        }
    }

    #[test]
    fn parse_repeat() {
        let pat = parse_quoted(r#""c4*4""#);
        match pat {
            Pattern::Repeat(inner, 4) => {
                assert_eq!(
                    *inner,
                    Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
                );
            }
            _ => panic!("expected Repeat, got {pat:?}"),
        }
    }

    #[test]
    fn parse_replicate() {
        let pat = parse_quoted(r#""c4!3""#);
        match pat {
            Pattern::Replicate(inner, 3) => {
                assert_eq!(
                    *inner,
                    Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
                );
            }
            _ => panic!("expected Replicate, got {pat:?}"),
        }
    }

    #[test]
    fn parse_slow_div() {
        let pat = parse_quoted(r#""c4/2""#);
        match pat {
            Pattern::SlowDiv(inner, 2) => {
                assert_eq!(
                    *inner,
                    Pattern::Atom(PatternAtom::Note(NoteValue::new(PitchClass::C, 4)))
                );
            }
            _ => panic!("expected SlowDiv, got {pat:?}"),
        }
    }

    #[test]
    fn parse_number_pattern() {
        let pat = parse_quoted(r#""0.8 0.7 0.9 0""#);
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
    fn parse_voice_def_with_transforms() {
        let input = r#"d1 $ slow 8 $ n "c4 d4 e4" # amp "0.8 0.7 0.9""#;
        let tokens = tokenize(input).unwrap();
        let mut parser = Parser::new(tokens);
        let (voice, transforms, cycle_length) = parser.parse_voice_def().unwrap();

        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0], Transform::Slow(8.0));
        assert!((cycle_length - 8.0).abs() < 0.001);
        assert!(voice.amp_pattern.is_some());
    }

    #[test]
    fn parse_stack_pattern() {
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
            other => panic!("expected Stack, got {other:?}"),
        }
    }

    #[test]
    fn parse_mini_convenience() {
        let pat = parse_mini("c4 d4 e4").unwrap();
        match pat {
            Pattern::Sequence(elems) => assert_eq!(elems.len(), 3),
            _ => panic!("expected Sequence"),
        }
    }
}
