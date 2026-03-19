/// TidalCycles mini-notation parser for the Amigo Engine.
///
/// Parses a subset of TidalCycles mini-notation into an AST, evaluates
/// patterns into time-resolved note events, and handles the `.amigo.tidal`
/// file format with metadata and stem definitions.
///
/// # Example
///
/// ```
/// use amigo_tidal_parser::{parse_mini, evaluate_pattern};
/// use amigo_tidal_parser::ast::*;
///
/// // Parse a mini-notation string.
/// let pattern = parse_mini("c4 d4 e4 ~ g4").unwrap();
///
/// // Load a full composition from a file.
/// // let comp = amigo_tidal_parser::load("assets/music/overworld.amigo.tidal").unwrap();
/// // let events = evaluate_pattern(&comp, 0);
/// ```
pub mod ast;
pub mod eval;
pub mod file;
pub mod lexer;
pub mod parser;

pub use ast::{
    Composition, CompositionMeta, Instrument, NoteValue, Pattern, PatternAtom, PitchClass, Stem,
    Transform, Voice,
};
pub use eval::{apply_transform, evaluate_pattern, NoteEvent};
pub use file::{format_amigo_tidal, load, parse_amigo_tidal, save};
pub use parser::parse_mini;
