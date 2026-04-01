//! oxvba-languageservice: bounded OxVba language-service core.
//!
//! Provides a workspace model, semantic snapshots, and language service
//! features (completions, go-to-definition, find-references, hover, etc.)
//! over VBA source modules.
//!
//! The current crate is the validated bounded internal surface tracked by
//! `LSF-0001`; it is not yet the full first-class editor platform described by
//! `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md`.

pub mod document;
pub mod semantic;
pub mod service;
pub mod span;
pub mod workspace;

pub use document::{Document, DocumentId};
pub use semantic::{
    SemanticSnapshot, SymbolTable, build_semantic_snapshot, build_semantic_snapshot_with_provenance,
};
pub use service::{
    CompletionItem, CompletionKind, HoverInfo, LanguageService, LanguageServiceProvider, Location,
    ParameterInfo, SignatureHelp,
};
pub use span::{
    DiagnosticSeverity, ScopeId, SemanticProvenance, SpannedDiagnostic, SymbolIdentity,
    SymbolInfo, SymbolKind, SymbolProvenanceKind, TextSpan,
};
pub use workspace::{Workspace, WorkspaceStats};
