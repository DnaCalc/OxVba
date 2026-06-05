//! Binder errors.

use oxvba_symbol::SymbolModelError;

/// An error raised while binding the CST + resolution into Core IR.
#[derive(Debug, thiserror::Error)]
pub enum BindError {
    /// The symbol model failed to build (parse/resolution error).
    #[error("symbol model error: {0}")]
    Symbol(#[from] SymbolModelError),
    /// A module failed to parse (should be caught by the symbol build first).
    #[error("parse error in module {module}: {message}")]
    Parse { module: String, message: String },
    /// A name could not be resolved in the given context.
    #[error("unresolved name `{name}` ({context})")]
    Unresolved { name: String, context: String },
    /// An assignment target/intent is invalid (e.g. `Set` on a scalar).
    #[error("invalid assignment: {0}")]
    InvalidAssignment(String),
    /// The CST shape was not what the construct requires.
    #[error("malformed construct: {0}")]
    Malformed(String),
    /// A construct the binder does not yet lower.
    #[error("unsupported construct: {0}")]
    Unsupported(String),
}
