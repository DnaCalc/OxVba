//! oxvba-languageservice: bounded OxVba language-service core.
//!
//! Provides a workspace model, semantic snapshots, and language service
//! features (completions, go-to-definition, find-references, hover, etc.)
//! over VBA source modules.
//!
//! The current crate is the validated bounded internal surface tracked by
//! `LSF-0001`; it is not yet the full first-class editor platform described by
//! `docs/spec/LANGUAGE_SERVICE_PLATFORM_SPEC_V2.md`.
//!
//! Host-facing guidance:
//! - direct-embed hosts such as OxIde should treat this crate as the primary
//!   semantic/query surface,
//! - project authoring and workspace target loading live in `oxvba-project`,
//! - `oxvba-lsp` should stay a thin transport layered over the same semantics.

pub mod document;
pub mod host_session;
pub mod semantic;
pub mod service;
pub mod span;
pub mod workspace;

pub use document::{Document, DocumentId};
pub use host_session::{
    HostSessionError, HostWorkspaceDocument, HostWorkspaceModuleRosterEntry, HostWorkspaceRoster,
    HostWorkspaceSession,
};
pub use semantic::{
    SemanticSnapshot, SymbolTable, build_semantic_snapshot, build_semantic_snapshot_with_provenance,
};
pub use service::{
    CodeActionKind, CodeActionPlan, CompletionItem, CompletionKind, DocumentSymbol, HoverInfo,
    LanguageService, LanguageServiceProvider, Location, ParameterInfo, ReferenceUpdateAnalysis,
    ReferenceUpdateIssue, ReferenceUpdateIssueKind, RenamePreparation, SemanticClassification,
    SemanticTokenKind, SignatureHelp, TextEdit, WorkspaceSymbol,
};
pub use span::{
    DiagnosticSeverity, ScopeId, SemanticProvenance, SpannedDiagnostic, SymbolIdentity, SymbolInfo,
    SymbolKind, SymbolProvenanceKind, TextSpan,
};
pub use workspace::{Workspace, WorkspaceStats};
