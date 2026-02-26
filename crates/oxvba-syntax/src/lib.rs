//! oxvba-syntax: lexer, parser, and lossless concrete syntax tree scaffolding.

pub mod green;
pub mod lexer;
pub mod parser;
pub mod red;
pub mod syntax_kind;

pub use parser::{ParseError, SyntaxTree, parse};
pub use syntax_kind::SyntaxKind;
