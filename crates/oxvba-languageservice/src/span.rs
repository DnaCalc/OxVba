use oxvba_compiler::resolve::BoundType;

/// A byte-offset span in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: u32,
    pub end: u32,
}

impl TextSpan {
    pub fn new(start: u32, end: u32) -> Self {
        TextSpan { start, end }
    }

    pub fn len(&self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn contains(&self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }
}

/// What kind of symbol this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Procedure,
    Parameter,
    Constant,
    TypeDef,
    EnumDef,
    EnumMember,
    External,
    Property,
    Event,
}

/// Scope identifier: 0 = module-level, N = procedure index (1-based).
pub type ScopeId = u32;

/// Semantic information about a symbol.
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub bound_type: BoundType,
    pub definition_span: TextSpan,
    pub scope: ScopeId,
}

/// A diagnostic with source location.
#[derive(Debug, Clone)]
pub struct SpannedDiagnostic {
    pub span: TextSpan,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}
