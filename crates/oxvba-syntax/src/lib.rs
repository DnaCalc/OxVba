//! oxvba-syntax: lossless lexer, parser, and concrete syntax tree for VBA.
//!
//! Every byte of VBA source is preserved in the syntax tree — whitespace,
//! comments, and line continuations are all represented as trivia tokens.
//! This makes the tree suitable for IDE tooling where source fidelity matters.

pub mod green;
pub mod lexer;
pub mod parser;
pub mod red;
pub mod syntax_kind;

pub use green::{Checkpoint, GreenChild, GreenNode, GreenNodeBuilder};
pub use parser::{Parse, ParseError, parse, parse_expression};
pub use red::{SyntaxElement, SyntaxNode, SyntaxToken};
pub use syntax_kind::{SyntaxKind, keyword_kind};
