//! oxvba-languageservice: Roslyn-class semantic workbench for VBA.
//!
//! Provides a workspace model, semantic snapshots, and language service
//! features (completions, go-to-definition, find-references, hover, etc.)
//! over VBA source modules.

pub mod document;
pub mod semantic;
pub mod service;
pub mod span;
pub mod workspace;

pub use document::{Document, DocumentId};
pub use semantic::{SemanticSnapshot, SymbolTable, build_semantic_snapshot};
pub use service::{
    CompletionItem, CompletionKind, HoverInfo, LanguageService, LanguageServiceProvider, Location,
    ParameterInfo, SignatureHelp,
};
pub use span::{DiagnosticSeverity, ScopeId, SpannedDiagnostic, SymbolInfo, SymbolKind, TextSpan};
pub use workspace::Workspace;
