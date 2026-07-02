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
    /// VBA's compile diagnostic for an undeclared variable under an explicit
    /// declaration context, and for `ReDim Preserve` names that are not already
    /// declared.
    #[error("Variable not defined")]
    VariableNotDefined { name: String },
    /// A bare name resolved to multiple equally viable project-level candidates.
    #[error("ambiguous name detected: {name}")]
    AmbiguousName { name: String },
    /// A module namespace was used where VBA requires a variable or procedure.
    #[error("expected variable or procedure, not module: {name}")]
    ExpectedVariableOrProcedureNotModule { name: String },
    /// A statement-position call target did not resolve to any Sub or Function.
    #[error("Sub or Function not defined")]
    SubOrFunctionNotDefined { name: String },
    /// An assignment target/intent is invalid (e.g. `Set` on a scalar).
    #[error("invalid assignment: {0}")]
    InvalidAssignment(String),
    /// A parameter declared as an array cannot be explicitly `ByVal` in VBA.
    #[error("Array argument must be ByRef")]
    ArrayArgumentMustBeByRef,
    /// A fixed-size array cannot be assigned as a whole-array lhs (`dst = src`).
    #[error("Can't assign to array")]
    CantAssignToArray,
    /// A ByRef argument l-value has a different declared type than its parameter.
    #[error("ByRef argument type mismatch: expected {expected}, got {actual}")]
    ByRefTypeMismatch { expected: String, actual: String },
    /// VBA's generic compile-time type mismatch diagnostic.
    #[error("Type mismatch")]
    TypeMismatch,
    /// Too many arguments were supplied for a procedure/property call.
    #[error("Wrong number of arguments or invalid property assignment")]
    WrongNumberOfArgumentsOrInvalidPropertyAssignment,
    /// A required argument was omitted.
    #[error("Argument not optional: {parameter}")]
    ArgumentNotOptional { parameter: String },
    /// A statement-only Sub was used where a value-producing expression is required.
    #[error("Expected Function or variable: {name}")]
    ExpectedFunctionOrVariable { name: String },
    /// A scalar expression or field was indexed where VBA requires an array.
    #[error("Expected array")]
    ExpectedArray { name: String },
    /// `LSet` was used on a target whose declared type is neither String nor UDT.
    #[error("LSet allowed only on strings and user-defined types")]
    LSetTargetType,
    /// `RSet` was used on a target whose declared type is not String.
    #[error("RSet allowed only on strings")]
    RSetTargetType,
    /// A line label is defined more than once within a single procedure — a VBA
    /// compile error ("Duplicate declaration in current scope"). Label scope is
    /// per-procedure, so the same name in a different procedure is fine.
    #[error("duplicate label `{name}` in current scope")]
    DuplicateLabel { name: String },
    /// A `GoTo`/`GoSub`/`On Error GoTo`/`Resume`/computed branch referenced a label
    /// that is not defined in the same procedure.
    #[error("Label not defined")]
    LabelNotDefined { name: String },
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
            BindError::VariableNotDefined { name } => Diagnostic::error(
                "BIND-E-VARIABLE-NOT-DEFINED",
                DiagnosticPhase::Bind,
                "Variable not defined",
            )
            .with_help(format!("Declare `{name}` before using it.")),
            BindError::AmbiguousName { name } => Diagnostic::error(
                "BIND-E-AMBIGUOUS-NAME",
                DiagnosticPhase::Bind,
                format!("ambiguous name detected: {name}"),
            )
            .with_help(
                "Qualify the member with its module name, or rename one of the public declarations.",
            ),
            BindError::ExpectedVariableOrProcedureNotModule { name } => Diagnostic::error(
                "BIND-E-EXPECTED-VARIABLE-OR-PROCEDURE-NOT-MODULE",
                DiagnosticPhase::Bind,
                format!("expected variable or procedure, not module: {name}"),
            )
            .with_help("Use `Module.Member` qualification, or rename the colliding public member."),
            BindError::SubOrFunctionNotDefined { name } => Diagnostic::error(
                "BIND-E-SUB-OR-FUNCTION-NOT-DEFINED",
                DiagnosticPhase::Bind,
                "Sub or Function not defined",
            )
            .with_help(format!("Define `{name}` as a Sub or Function, or correct the call spelling.")),
            BindError::InvalidAssignment(message) => Diagnostic::error(
                "BIND-E-INVALID-ASSIGNMENT",
                DiagnosticPhase::Bind,
                format!("invalid assignment: {message}"),
            )
            .with_help("Check whether the target is assignable and whether Set/Let semantics match the value."),
            BindError::ArrayArgumentMustBeByRef => Diagnostic::error(
                "BIND-E-ARRAY-ARGUMENT-MUST-BYREF",
                DiagnosticPhase::Bind,
                "Array argument must be ByRef",
            )
            .with_help("Remove ByVal from the array parameter declaration."),
            BindError::CantAssignToArray => Diagnostic::error(
                "BIND-E-CANT-ASSIGN-TO-ARRAY",
                DiagnosticPhase::Bind,
                "Can't assign to array",
            )
            .with_help("Assign into a dynamic array target or copy elements individually."),
            BindError::ByRefTypeMismatch { expected, actual } => Diagnostic::error(
                "BIND-E-BYREF-TYPE-MISMATCH",
                DiagnosticPhase::Bind,
                format!("ByRef argument type mismatch: expected {expected}, got {actual}"),
            )
            .with_help(
                "Pass a variable with the exact declared type, or parenthesize the argument to pass a coerced temporary.",
            ),
            BindError::TypeMismatch => Diagnostic::error(
                "BIND-E-TYPE-MISMATCH",
                DiagnosticPhase::Bind,
                "Type mismatch",
            )
            .with_help("Use compatible declared types for this statement."),
            BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment => Diagnostic::error(
                "BIND-E-WRONG-NUMBER-OF-ARGUMENTS",
                DiagnosticPhase::Bind,
                "Wrong number of arguments or invalid property assignment",
            )
            .with_help("Check the procedure signature and supplied argument list."),
            BindError::ArgumentNotOptional { parameter } => Diagnostic::error(
                "BIND-E-ARGUMENT-NOT-OPTIONAL",
                DiagnosticPhase::Bind,
                format!("Argument not optional: {parameter}"),
            )
            .with_help("Supply the required argument, or mark the parameter Optional."),
            BindError::ExpectedFunctionOrVariable { name } => Diagnostic::error(
                "BIND-E-EXPECTED-FUNCTION-OR-VARIABLE",
                DiagnosticPhase::Bind,
                format!("Expected Function or variable: {name}"),
            )
            .with_help("Use a Function or Property Get in value context, or call the Sub as a statement."),
            BindError::ExpectedArray { name } => Diagnostic::error(
                "BIND-E-EXPECTED-ARRAY",
                DiagnosticPhase::Bind,
                "Expected array",
            )
            .with_help(format!("`{name}` is not an array.")),
            BindError::LSetTargetType => Diagnostic::error(
                "BIND-E-LSET-TARGET-TYPE",
                DiagnosticPhase::Bind,
                "LSet allowed only on strings and user-defined types",
            )
            .with_help("Use LSet with a String target, or implement the UDT record-copy path."),
            BindError::RSetTargetType => Diagnostic::error(
                "BIND-E-RSET-TARGET-TYPE",
                DiagnosticPhase::Bind,
                "RSet allowed only on strings",
            )
            .with_help("Use RSet with a String target."),
            BindError::DuplicateLabel { name } => Diagnostic::error(
                "BIND-E-DUPLICATE-LABEL",
                DiagnosticPhase::Bind,
                format!("duplicate label `{name}` in current scope"),
            )
            .with_help("A line label must be unique within a procedure; rename or remove the duplicate."),
            BindError::LabelNotDefined { name } => Diagnostic::error(
                "BIND-E-LABEL-NOT-DEFINED",
                DiagnosticPhase::Bind,
                "Label not defined",
            )
            .with_help(format!("Define `{name}` in this procedure, or fix the branch target.")),
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

    #[test]
    fn module_as_value_has_stable_code() {
        let diagnostic = BindError::ExpectedVariableOrProcedureNotModule {
            name: "Clash".to_string(),
        }
        .to_diagnostic();
        assert_eq!(
            diagnostic.code.as_str(),
            "BIND-E-EXPECTED-VARIABLE-OR-PROCEDURE-NOT-MODULE"
        );
        assert!(
            diagnostic
                .message
                .contains("expected variable or procedure, not module: Clash")
        );
    }

    #[test]
    fn byref_type_mismatch_has_stable_code() {
        let diagnostic = BindError::ByRefTypeMismatch {
            expected: "Long".to_string(),
            actual: "Integer".to_string(),
        }
        .to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "BIND-E-BYREF-TYPE-MISMATCH");
        assert!(
            diagnostic
                .message
                .contains("ByRef argument type mismatch: expected Long, got Integer")
        );
    }

    #[test]
    fn wrong_number_of_arguments_has_stable_code() {
        let diagnostic =
            BindError::WrongNumberOfArgumentsOrInvalidPropertyAssignment.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "BIND-E-WRONG-NUMBER-OF-ARGUMENTS");
        assert!(
            diagnostic
                .message
                .contains("Wrong number of arguments or invalid property assignment")
        );
    }

    #[test]
    fn argument_not_optional_has_stable_code() {
        let diagnostic = BindError::ArgumentNotOptional {
            parameter: "b".to_string(),
        }
        .to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "BIND-E-ARGUMENT-NOT-OPTIONAL");
        assert!(diagnostic.message.contains("Argument not optional: b"));
    }

    #[test]
    fn expected_function_or_variable_has_stable_code() {
        let diagnostic = BindError::ExpectedFunctionOrVariable {
            name: "DoIt".to_string(),
        }
        .to_diagnostic();
        assert_eq!(
            diagnostic.code.as_str(),
            "BIND-E-EXPECTED-FUNCTION-OR-VARIABLE"
        );
        assert!(
            diagnostic
                .message
                .contains("Expected Function or variable: DoIt")
        );
    }

    #[test]
    fn expected_array_has_vba_message() {
        let diagnostic = BindError::ExpectedArray {
            name: "s.Value".to_string(),
        }
        .to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "BIND-E-EXPECTED-ARRAY");
        assert_eq!(diagnostic.message, "Expected array");
    }
}
