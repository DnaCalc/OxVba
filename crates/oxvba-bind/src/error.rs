//! Binder errors.

use oxvba_diagnostics::{Diagnostic, DiagnosticCause, DiagnosticPhase};
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
    /// A bare name resolved to multiple equally viable project-level candidates.
    #[error("ambiguous name detected: {name}")]
    AmbiguousName { name: String },
    /// An assignment target/intent is invalid (e.g. `Set` on a scalar).
    #[error("invalid assignment: {0}")]
    InvalidAssignment(String),
    /// A line label is defined more than once within a single procedure — a VBA
    /// compile error ("Duplicate declaration in current scope"). Label scope is
    /// per-procedure, so the same name in a different procedure is fine.
    #[error("duplicate label `{name}` in current scope")]
    DuplicateLabel { name: String },
    /// The CST shape was not what the construct requires.
    #[error("malformed construct: {0}")]
    Malformed(String),
    /// A construct the binder does not yet lower.
    #[error("unsupported construct: {0}")]
    Unsupported(String),
}

impl BindError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            BindError::Symbol(err) => err.to_diagnostic().with_cause(DiagnosticCause {
                code: Some("BIND-E-SYMBOL-MODEL".into()),
                message: "binder could not build the resolution environment".to_string(),
            }),
            BindError::Parse { module, message } => Diagnostic::error(
                "BIND-E-PARSE",
                DiagnosticPhase::Bind,
                format!("parse error in module {module}: {message}"),
            ),
            BindError::Unresolved { name, context } => Diagnostic::error(
                "BIND-E-UNRESOLVED-NAME",
                DiagnosticPhase::Bind,
                format!("unresolved name `{name}` ({context})"),
            )
            .with_help("Check the declaration spelling, module visibility, and project references."),
            BindError::AmbiguousName { name } => Diagnostic::error(
                "BIND-E-AMBIGUOUS-NAME",
                DiagnosticPhase::Bind,
                format!("ambiguous name detected: {name}"),
            )
            .with_help(
                "Qualify the member with its module name, or rename one of the public declarations.",
            ),
            BindError::InvalidAssignment(message) => Diagnostic::error(
                "BIND-E-INVALID-ASSIGNMENT",
                DiagnosticPhase::Bind,
                format!("invalid assignment: {message}"),
            )
            .with_help("Check whether the target is assignable and whether Set/Let semantics match the value."),
            BindError::DuplicateLabel { name } => Diagnostic::error(
                "BIND-E-DUPLICATE-LABEL",
                DiagnosticPhase::Bind,
                format!("duplicate label `{name}` in current scope"),
            )
            .with_help("A line label must be unique within a procedure; rename or remove the duplicate."),
            BindError::Malformed(message) => Diagnostic::error(
                "BIND-E-MALFORMED-CONSTRUCT",
                DiagnosticPhase::Bind,
                format!("malformed construct: {message}"),
            ),
            BindError::Unsupported(message) => Diagnostic::error(
                "BIND-E-UNSUPPORTED-CONSTRUCT",
                DiagnosticPhase::Bind,
                format!("unsupported construct: {message}"),
            )
            .with_help("This construct is parsed but not yet lowered by the clean binder."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BindError;

    #[test]
    fn unresolved_name_has_stable_code() {
        let diagnostic = BindError::Unresolved {
            name: "Missing".to_string(),
            context: "expression".to_string(),
        }
        .to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "BIND-E-UNRESOLVED-NAME");
    }

    #[test]
    fn ambiguous_name_has_stable_code() {
        let diagnostic = BindError::AmbiguousName {
            name: "Dup".to_string(),
        }
        .to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "BIND-E-AMBIGUOUS-NAME");
        assert!(diagnostic.message.contains("ambiguous name detected: Dup"));
    }
}
