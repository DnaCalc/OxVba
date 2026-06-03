use crate::document::DocumentId;
use crate::semantic::CallableSignatureInfo;
use crate::span::{
    SemanticProvenance, SpannedDiagnostic, SymbolIdentity, SymbolInfo, SymbolKind,
    SymbolProvenanceKind, TextSpan,
};
use crate::workspace::Workspace;

use oxvba_compiler::lsp_support::intrinsic_spec;
use oxvba_compiler::{OptionalDefaultValue, ProjectManifest};
use oxvba_syntax::SyntaxKind;

/// A position in a document (byte offset).
pub type Position = u32;

/// A location: document + span.
#[derive(Debug, Clone)]
pub struct Location {
    pub document: DocumentId,
    pub span: TextSpan,
    pub symbol_identity: Option<SymbolIdentity>,
    pub provenance: Option<SemanticProvenance>,
}

/// A document-level symbol entry suitable for outline/navigation views.
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: TextSpan,
    pub detail: Option<String>,
    pub container_name: Option<String>,
    pub symbol_identity: SymbolIdentity,
    pub provenance: SemanticProvenance,
}

/// A workspace-level symbol result.
#[derive(Debug, Clone)]
pub struct WorkspaceSymbol {
    pub document: DocumentId,
    pub symbol: DocumentSymbol,
}

/// Semantic token/classification kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticTokenKind {
    Keyword,
    Procedure,
    Property,
    Variable,
    Parameter,
    Constant,
    TypeDef,
    EnumDef,
    Event,
    External,
    Intrinsic,
}

/// A semantic token/classification entry over a source span.
#[derive(Debug, Clone)]
pub struct SemanticClassification {
    pub span: TextSpan,
    pub kind: SemanticTokenKind,
    pub symbol_identity: Option<SymbolIdentity>,
    pub provenance: Option<SemanticProvenance>,
}

/// A completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub source_document: Option<DocumentId>,
    pub provenance: Option<SemanticProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionKind {
    Keyword,
    Variable,
    Procedure,
    Parameter,
    Constant,
    Intrinsic,
    Property,
}

/// Signature help for a procedure call.
#[derive(Debug, Clone)]
pub struct SignatureHelp {
    pub name: String,
    pub parameters: Vec<ParameterInfo>,
    pub return_type: String,
    pub active_parameter: usize,
    pub source_document: Option<DocumentId>,
    pub provenance: Option<SemanticProvenance>,
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub type_name: String,
    pub optional: bool,
    pub param_array: bool,
    pub default_value: Option<OptionalDefaultValue>,
}

/// Hover information for a symbol.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub label: String,
    pub detail: Option<String>,
    pub symbol_identity: Option<SymbolIdentity>,
    pub provenance: Option<SemanticProvenance>,
}

/// Rename preparation at a given cursor position.
#[derive(Debug, Clone)]
pub struct RenamePreparation {
    pub current_name: String,
    pub placeholder: TextSpan,
    pub declaration: Location,
    pub symbol_identity: SymbolIdentity,
    pub provenance: SemanticProvenance,
    pub reference_analysis: ReferenceUpdateAnalysis,
}

/// Safety analysis for updating all references of a symbol.
#[derive(Debug, Clone)]
pub struct ReferenceUpdateAnalysis {
    pub declaration: Location,
    pub references: Vec<Location>,
    pub writable_documents: Vec<DocumentId>,
    pub blocked_documents: Vec<DocumentId>,
    pub issues: Vec<ReferenceUpdateIssue>,
    pub safe_to_apply: bool,
}

/// A transport-neutral planned code action over workspace text.
#[derive(Debug, Clone)]
pub struct CodeActionPlan {
    pub title: String,
    pub kind: CodeActionKind,
    pub document: DocumentId,
    pub edits: Vec<TextEdit>,
    pub diagnostic: SpannedDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    QuickFix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub span: TextSpan,
    pub new_text: String,
}

#[derive(Debug, Clone)]
pub struct ReferenceUpdateIssue {
    pub kind: ReferenceUpdateIssueKind,
    pub document: Option<DocumentId>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceUpdateIssueKind {
    GeneratedDocument,
    ProjectReferenceDocument,
    ImportedTypeLibraryDocument,
    MissingDocument,
}

// ── VBA keyword list for completions ────────────────────────────────

const VBA_KEYWORDS: &[&str] = &[
    "Sub",
    "Function",
    "End",
    "If",
    "Then",
    "Else",
    "ElseIf",
    "For",
    "To",
    "Step",
    "Next",
    "Do",
    "Loop",
    "While",
    "Wend",
    "Until",
    "Select",
    "Case",
    "With",
    "Dim",
    "Const",
    "Public",
    "Private",
    "Static",
    "Set",
    "Let",
    "As",
    "New",
    "Nothing",
    "ByRef",
    "ByVal",
    "Optional",
    "ParamArray",
    "Property",
    "Get",
    "Call",
    "Exit",
    "On",
    "Error",
    "Resume",
    "Erase",
    "ReDim",
    "GoTo",
    "GoSub",
    "Return",
    "True",
    "False",
    "Not",
    "And",
    "Or",
    "Xor",
    "Mod",
    "Like",
    "Is",
    "Me",
    "Debug",
    "Stop",
];

// ── Intrinsic function names for completions ────────────────────────

const INTRINSIC_NAMES: &[&str] = &[
    "Abs",
    "Asc",
    "AscW",
    "Atn",
    "CBool",
    "CByte",
    "CCur",
    "CDate",
    "CDbl",
    "CDec",
    "Chr",
    "ChrW",
    "CInt",
    "CLng",
    "CLngLng",
    "CLngPtr",
    "Cos",
    "CSng",
    "CStr",
    "CVar",
    "CVErr",
    "Date",
    "DateAdd",
    "DateDiff",
    "DatePart",
    "DateSerial",
    "DateValue",
    "Day",
    "Dir",
    "DoEvents",
    "Environ",
    "EOF",
    "Err",
    "Exp",
    "FileDateTime",
    "FileLen",
    "Fix",
    "Format",
    "FreeFile",
    "Hex",
    "Hour",
    "IIf",
    "InStr",
    "InStrRev",
    "Int",
    "IsArray",
    "IsDate",
    "IsEmpty",
    "IsError",
    "IsMissing",
    "IsNull",
    "IsNumeric",
    "IsObject",
    "Join",
    "LBound",
    "LCase",
    "Left",
    "Len",
    "LenB",
    "Log",
    "LTrim",
    "Mid",
    "Minute",
    "Month",
    "MonthName",
    "MsgBox",
    "Now",
    "Oct",
    "Replace",
    "RGB",
    "Right",
    "Rnd",
    "Round",
    "RTrim",
    "Second",
    "Sgn",
    "Sin",
    "Space",
    "Split",
    "Sqr",
    "Str",
    "StrComp",
    "String",
    "StrReverse",
    "Switch",
    "Tab",
    "Tan",
    "Time",
    "Timer",
    "TimeSerial",
    "TimeValue",
    "Trim",
    "TypeName",
    "UBound",
    "UCase",
    "Val",
    "Weekday",
    "WeekdayName",
    "Year",
];

/// High-level project-oriented trait for language service consumers (§4.8).
///
/// Uses `ProjectManifest` and module names (strings) instead of `DocumentId`.
pub trait LanguageServiceProvider {
    fn diagnostics(&self, project: &ProjectManifest, module: &str) -> Vec<SpannedDiagnostic>;
    fn symbols(&self, project: &ProjectManifest, module: &str) -> Vec<SymbolInfo>;
    fn document_symbols(&self, project: &ProjectManifest, module: &str) -> Vec<DocumentSymbol>;
    fn workspace_symbols(&self, project: &ProjectManifest, query: &str) -> Vec<WorkspaceSymbol>;
    fn semantic_classifications(
        &self,
        project: &ProjectManifest,
        module: &str,
    ) -> Vec<SemanticClassification>;
    fn completions(
        &self,
        project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Vec<CompletionItem>;
    fn signature_help(
        &self,
        project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<SignatureHelp>;
    fn go_to_definition(
        &self,
        project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<Location>;
    fn find_references(
        &self,
        project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Vec<Location>;
    fn prepare_rename(
        &self,
        project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<RenamePreparation>;
    fn reference_update_analysis(
        &self,
        project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<ReferenceUpdateAnalysis>;
    fn code_actions(&self, project: &ProjectManifest, module: &str) -> Vec<CodeActionPlan>;
    fn hover(&self, project: &ProjectManifest, module: &str, pos: Position) -> Option<HoverInfo>;
}

/// The language service: provides IDE features over a workspace.
///
/// This implements the contract from §4.8 of HOSTING_PROJECT_TOOLING_PROPOSAL.
/// Each method operates on a single workspace and delegates to the
/// per-document `SemanticSnapshot`.
pub struct LanguageService {
    pub workspace: Workspace,
}

#[derive(Debug, Clone)]
struct ResolvedSymbol {
    document: DocumentId,
    symbol: SymbolInfo,
}

impl LanguageService {
    pub fn new(workspace: Workspace) -> Self {
        LanguageService { workspace }
    }

    pub fn from_project(project: ProjectManifest) -> Self {
        LanguageService {
            workspace: Workspace::new().with_project(project),
        }
    }

    /// Get all diagnostics for a document.
    pub fn diagnostics(&self, module: &DocumentId) -> Vec<SpannedDiagnostic> {
        self.workspace
            .snapshot(module)
            .map(|s| s.diagnostics.clone())
            .unwrap_or_default()
    }

    /// Get all symbols for a document.
    pub fn symbols(&self, module: &DocumentId) -> Vec<SymbolInfo> {
        self.workspace
            .snapshot(module)
            .map(|s| s.symbols.symbols.clone())
            .unwrap_or_default()
    }

    /// Get document symbols for a single document.
    pub fn document_symbols(&self, module: &DocumentId) -> Vec<DocumentSymbol> {
        let mut symbols = self
            .workspace
            .snapshot(module)
            .map(|s| {
                let procedure_names = procedure_names_by_scope(&s.parse.syntax());
                let mut items = s
                    .symbols
                    .symbols
                    .iter()
                    .map(|sym| DocumentSymbol {
                        name: sym.name.clone(),
                        kind: sym.kind,
                        span: sym.definition_span,
                        detail: Some(format!("{:?}", sym.bound_type)),
                        container_name: if sym.scope == 0 {
                            None
                        } else {
                            procedure_names.get(&sym.scope).cloned()
                        },
                        symbol_identity: sym.identity.clone(),
                        provenance: sym.provenance.clone(),
                    })
                    .collect::<Vec<_>>();
                items.sort_by_key(|sym| sym.span.start);
                items
            })
            .unwrap_or_default();
        symbols.sort_by_key(|sym| sym.span.start);
        symbols
    }

    /// Get workspace symbols, optionally filtered by a case-insensitive prefix.
    pub fn workspace_symbols(&self, query: &str) -> Vec<WorkspaceSymbol> {
        let query_lower = query.to_ascii_lowercase();
        let mut items = Vec::new();

        for document in self.workspace.document_ids() {
            for symbol in self.document_symbols(document) {
                if query_lower.is_empty()
                    || symbol.name.to_ascii_lowercase().starts_with(&query_lower)
                {
                    items.push(WorkspaceSymbol {
                        document: document.clone(),
                        symbol,
                    });
                }
            }
        }

        items.sort_by(|left, right| {
            left.symbol
                .name
                .cmp(&right.symbol.name)
                .then_with(|| left.document.0.cmp(&right.document.0))
        });
        items
    }

    /// Get semantic classifications for a document.
    pub fn semantic_classifications(&self, module: &DocumentId) -> Vec<SemanticClassification> {
        let snap = match self.workspace.snapshot(module) {
            Some(snapshot) => snapshot,
            None => return Vec::new(),
        };

        let mut classifications = Vec::new();
        for (kind, text, offset) in collect_all_tokens(&snap.parse.syntax()) {
            if kind.is_trivia() {
                continue;
            }

            let span = TextSpan::new(offset, offset + text.len() as u32);
            let token_lower = text.to_ascii_lowercase();

            if VBA_KEYWORDS
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case(text))
            {
                classifications.push(SemanticClassification {
                    span,
                    kind: SemanticTokenKind::Keyword,
                    symbol_identity: None,
                    provenance: None,
                });
                continue;
            }

            if kind == SyntaxKind::Ident {
                if let Some(resolved) = self.resolve_symbol_at(module, offset) {
                    classifications.push(SemanticClassification {
                        span,
                        kind: semantic_kind_for_symbol(resolved.symbol.kind),
                        symbol_identity: Some(resolved.symbol.identity.clone()),
                        provenance: Some(resolved.symbol.provenance.clone()),
                    });
                    continue;
                }

                if intrinsic_spec(&token_lower).is_some() {
                    classifications.push(SemanticClassification {
                        span,
                        kind: SemanticTokenKind::Intrinsic,
                        symbol_identity: None,
                        provenance: None,
                    });
                }
            }
        }

        classifications
    }

    /// Get all symbols across all documents.
    pub fn all_symbols(&self) -> Vec<(DocumentId, SymbolInfo)> {
        let mut result = Vec::new();
        for id in self.workspace.document_ids() {
            if let Some(snap) = self.workspace.snapshot(id) {
                for sym in &snap.symbols.symbols {
                    result.push((id.clone(), sym.clone()));
                }
            }
        }
        result
    }

    /// Completions at a given position in a document.
    pub fn completions(&self, module: &DocumentId, position: Position) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        // Extract prefix: partial identifier at cursor position
        let prefix = self.prefix_at_position(module, position);
        let prefix_lower = prefix.to_ascii_lowercase();

        // Keywords
        for kw in VBA_KEYWORDS {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: CompletionKind::Keyword,
                detail: None,
                source_document: None,
                provenance: None,
            });
        }

        // Intrinsic functions
        for name in INTRINSIC_NAMES {
            let detail = intrinsic_spec(&name.to_ascii_lowercase()).map(|spec| {
                if spec.min_arity == spec.max_arity {
                    format!("({} args)", spec.min_arity)
                } else {
                    format!("({}-{} args)", spec.min_arity, spec.max_arity)
                }
            });
            items.push(CompletionItem {
                label: name.to_string(),
                kind: CompletionKind::Intrinsic,
                detail,
                source_document: None,
                provenance: None,
            });
        }

        // Symbols from the current document
        if let Some(snap) = self.workspace.snapshot(module) {
            // Determine scope at position
            let scope = self.scope_at_position(module, position);

            for sym in &snap.symbols.symbols {
                // Module-level symbols are always visible; local symbols only
                // if we're in the same scope
                if sym.scope == 0 || sym.scope == scope {
                    let ck = match sym.kind {
                        SymbolKind::Variable => CompletionKind::Variable,
                        SymbolKind::Procedure => CompletionKind::Procedure,
                        SymbolKind::Parameter => CompletionKind::Parameter,
                        SymbolKind::Constant => CompletionKind::Constant,
                        SymbolKind::Property => CompletionKind::Property,
                        _ => CompletionKind::Variable,
                    };
                    items.push(CompletionItem {
                        label: sym.name.clone(),
                        kind: ck,
                        detail: Some(completion_detail(sym, module)),
                        source_document: Some(module.clone()),
                        provenance: Some(sym.provenance.clone()),
                    });
                }
            }
        }

        // Cross-module symbols
        for id in self.workspace.document_ids() {
            if id == module {
                continue;
            }
            if let Some(snap) = self.workspace.snapshot(id) {
                for sym in &snap.symbols.symbols {
                    if sym.scope == 0 {
                        items.push(CompletionItem {
                            label: sym.name.clone(),
                            kind: match sym.kind {
                                SymbolKind::Procedure => CompletionKind::Procedure,
                                SymbolKind::Property => CompletionKind::Property,
                                _ => CompletionKind::Variable,
                            },
                            detail: Some(completion_detail(sym, id)),
                            source_document: Some(id.clone()),
                            provenance: Some(sym.provenance.clone()),
                        });
                    }
                }
            }
        }

        dedupe_completions(&mut items, module);

        // Filter by prefix (case-insensitive). Empty prefix returns all.
        if !prefix_lower.is_empty() {
            items.retain(|item| item.label.to_ascii_lowercase().starts_with(&prefix_lower));
        }

        items
    }

    /// Signature help: parameter info for a procedure call at position.
    ///
    /// Uses paren-depth-aware backward walk to correctly identify the
    /// enclosing call even when there are nested calls like `f(g(|x))`.
    pub fn signature_help(&self, module: &DocumentId, position: Position) -> Option<SignatureHelp> {
        let snap = self.workspace.snapshot(module)?;
        let root = snap.parse.syntax();
        let all_tokens = collect_all_tokens(&root);

        // Find the index of the last token at or before `position`.
        let start_idx = {
            let mut idx = 0;
            for (i, &(_, _, offset)) in all_tokens.iter().enumerate() {
                if offset <= position {
                    idx = i;
                } else {
                    break;
                }
            }
            idx
        };

        // Walk backward, tracking paren depth and counting commas.
        let mut depth: i32 = 0;
        let mut comma_count: usize = 0;
        let mut call_name: Option<&str> = None;

        for i in (0..=start_idx).rev() {
            let (kind, _text, _) = all_tokens[i];
            match kind {
                SyntaxKind::RParen => depth += 1,
                SyntaxKind::LParen => {
                    depth -= 1;
                    if depth < 0 {
                        // Found the opening paren of our call.
                        // The call target is the preceding Ident.
                        let mut j = i;
                        loop {
                            if j == 0 {
                                break;
                            }
                            j -= 1;
                            if !all_tokens[j].0.is_trivia() {
                                if all_tokens[j].0 == SyntaxKind::Ident {
                                    call_name = Some(all_tokens[j].1);
                                }
                                break;
                            }
                        }
                        break;
                    }
                }
                SyntaxKind::Comma if depth == 0 => comma_count += 1,
                _ => {}
            }
        }

        if call_name.is_none() {
            let mut fallback_name: Option<&str> = None;
            let mut fallback_commas: usize = 0;
            let mut saw_argument_tokens = false;

            for i in (0..=start_idx).rev() {
                let (kind, text, _) = all_tokens[i];
                if text.contains('\n') || text.contains('\r') {
                    break;
                }
                if kind.is_trivia() {
                    continue;
                }
                if kind == SyntaxKind::Comma {
                    fallback_commas += 1;
                    saw_argument_tokens = true;
                    continue;
                }
                if kind == SyntaxKind::Ident {
                    fallback_name = Some(text);
                    break;
                }
                saw_argument_tokens = true;
            }

            if saw_argument_tokens {
                call_name = fallback_name;
                comma_count = fallback_commas;
            }
        }

        let name = call_name?;
        let name_lower = name.to_ascii_lowercase();
        let (document_id, callable, provenance) =
            self.resolve_callable_signature(module, &name_lower)?;

        let parameters = callable
            .params
            .iter()
            .map(|p| ParameterInfo {
                name: p.name.clone(),
                type_name: format!("{:?}", p.ty),
                optional: p.optional,
                param_array: p.param_array,
                default_value: p.default_value.clone(),
            })
            .collect();

        Some(SignatureHelp {
            name: callable.name,
            parameters,
            return_type: format!("{:?}", callable.return_type),
            active_parameter: comma_count,
            source_document: Some(document_id),
            provenance: Some(provenance),
        })
    }

    /// Go to definition: find where the symbol at position is defined.
    pub fn go_to_definition(&self, module: &DocumentId, position: Position) -> Option<Location> {
        let resolved = self.resolve_symbol_at(module, position)?;
        Some(self.location_for_symbol(&resolved.document, &resolved.symbol))
    }

    /// Find all references to the symbol at position.
    pub fn find_references(&self, module: &DocumentId, position: Position) -> Vec<Location> {
        let mut locations = Vec::new();

        let target = match self.resolve_symbol_at(module, position) {
            Some(symbol) => symbol,
            None => return locations,
        };
        let ident_lower = target.symbol.name.to_ascii_lowercase();

        // Search all documents for matching identifiers
        for doc_id in self.workspace.document_ids() {
            if let Some(snap) = self.workspace.snapshot(doc_id) {
                let root = snap.parse.syntax();
                let all_tokens = collect_all_tokens(&root);

                for (kind, text, offset) in &all_tokens {
                    if *kind == SyntaxKind::Ident && text.to_ascii_lowercase() == ident_lower {
                        let occurrence = match self.resolve_symbol_at(doc_id, *offset) {
                            Some(occurrence) => occurrence,
                            None => continue,
                        };
                        if occurrence.symbol.identity != target.symbol.identity {
                            continue;
                        }

                        locations.push(Location {
                            document: doc_id.clone(),
                            span: TextSpan::new(*offset, offset + text.len() as u32),
                            symbol_identity: Some(occurrence.symbol.identity.clone()),
                            provenance: Some(occurrence.symbol.provenance.clone()),
                        });
                    }
                }
            }
        }

        locations
    }

    /// Prepare a rename operation around the current symbol occurrence.
    pub fn prepare_rename(
        &self,
        module: &DocumentId,
        position: Position,
    ) -> Option<RenamePreparation> {
        let occurrence_span = self.identifier_span_at_position(module, position)?;
        let resolved = self.resolve_symbol_at(module, position)?;
        let declaration = self.location_for_symbol(&resolved.document, &resolved.symbol);
        let reference_analysis = self.reference_update_analysis(module, position)?;

        Some(RenamePreparation {
            current_name: resolved.symbol.name.clone(),
            placeholder: occurrence_span,
            declaration,
            symbol_identity: resolved.symbol.identity.clone(),
            provenance: resolved.symbol.provenance.clone(),
            reference_analysis,
        })
    }

    /// Analyze whether all references for the symbol at the given position can
    /// be updated safely by an editor/host.
    pub fn reference_update_analysis(
        &self,
        module: &DocumentId,
        position: Position,
    ) -> Option<ReferenceUpdateAnalysis> {
        let resolved = self.resolve_symbol_at(module, position)?;
        let declaration = self.location_for_symbol(&resolved.document, &resolved.symbol);
        let references = self.find_references(module, position);

        let mut writable_documents = std::collections::BTreeSet::new();
        let mut blocked_documents = std::collections::BTreeSet::new();
        let mut issues = Vec::new();

        for document_id in references
            .iter()
            .map(|location| location.document.clone())
            .chain(std::iter::once(declaration.document.clone()))
        {
            let Some(document) = self.workspace.document(&document_id) else {
                blocked_documents.insert(document_id.0.clone());
                issues.push(ReferenceUpdateIssue {
                    kind: ReferenceUpdateIssueKind::MissingDocument,
                    document: Some(document_id),
                    message: "workspace document is no longer available".to_string(),
                });
                continue;
            };

            match document.provenance_kind {
                SymbolProvenanceKind::SourceModule => {
                    writable_documents.insert(document_id.0.clone());
                }
                SymbolProvenanceKind::Generated => {
                    blocked_documents.insert(document_id.0.clone());
                    issues.push(ReferenceUpdateIssue {
                        kind: ReferenceUpdateIssueKind::GeneratedDocument,
                        document: Some(document_id),
                        message: "generated documents are not safe direct rename targets"
                            .to_string(),
                    });
                }
                SymbolProvenanceKind::ProjectReference => {
                    blocked_documents.insert(document_id.0.clone());
                    issues.push(ReferenceUpdateIssue {
                        kind: ReferenceUpdateIssueKind::ProjectReferenceDocument,
                        document: Some(document_id),
                        message: "referenced-project documents require explicit multi-project edit ownership"
                            .to_string(),
                    });
                }
                SymbolProvenanceKind::ImportedTypeLibraryProjection => {
                    blocked_documents.insert(document_id.0.clone());
                    issues.push(ReferenceUpdateIssue {
                        kind: ReferenceUpdateIssueKind::ImportedTypeLibraryDocument,
                        document: Some(document_id),
                        message: "imported-typelib projections are not writable rename targets"
                            .to_string(),
                    });
                }
            }
        }

        let safe_to_apply = blocked_documents.is_empty();

        Some(ReferenceUpdateAnalysis {
            declaration,
            references,
            writable_documents: writable_documents
                .into_iter()
                .map(DocumentId::new)
                .collect(),
            blocked_documents: blocked_documents.into_iter().map(DocumentId::new).collect(),
            safe_to_apply,
            issues,
        })
    }

    /// Plan bounded diagnostics-driven code actions for a document.
    pub fn code_actions(&self, module: &DocumentId) -> Vec<CodeActionPlan> {
        let mut actions = Vec::new();
        let Some(snapshot) = self.workspace.snapshot(module) else {
            return actions;
        };

        for diagnostic in &snapshot.diagnostics {
            if let Some(variable_name) = undeclared_variable_name(&diagnostic.message) {
                if let Some(reference_span) = self.undeclared_variable_anchor_span(
                    module,
                    snapshot.source.as_ref(),
                    diagnostic.span,
                    variable_name,
                ) && let Some(insert_span) =
                    self.local_declaration_insertion_span(module, reference_span.start)
                {
                    let newline = preferred_newline(snapshot.source.as_ref());
                    let inserted = format!("    Dim {variable_name} As Variant{newline}");
                    actions.push(CodeActionPlan {
                        title: format!("Declare local variable '{variable_name}'"),
                        kind: CodeActionKind::QuickFix,
                        document: module.clone(),
                        edits: vec![TextEdit {
                            span: insert_span,
                            new_text: inserted,
                        }],
                        diagnostic: diagnostic.clone(),
                    });
                }
                continue;
            }

            if ptrsafe_required_diagnostic(&diagnostic.message)
                && let Some(insert_span) =
                    declare_ptrsafe_insertion_span(snapshot.source.as_ref(), &diagnostic.message)
            {
                actions.push(CodeActionPlan {
                    title: "Add PtrSafe keyword".to_string(),
                    kind: CodeActionKind::QuickFix,
                    document: module.clone(),
                    edits: vec![TextEdit {
                        span: insert_span,
                        new_text: "PtrSafe ".to_string(),
                    }],
                    diagnostic: diagnostic.clone(),
                });
            }
        }

        actions
    }

    /// Hover info for the symbol at position.
    pub fn hover(&self, module: &DocumentId, position: Position) -> Option<HoverInfo> {
        if let Some(resolved) = self.resolve_symbol_at(module, position) {
            let sym = &resolved.symbol;
            let label = match sym.kind {
                SymbolKind::Procedure => format!("Sub/Function {}", sym.name),
                SymbolKind::Property => format!("Property {}", sym.name),
                SymbolKind::Variable => format!("Dim {} As {:?}", sym.name, sym.bound_type),
                SymbolKind::Parameter => {
                    format!("{} As {:?}", sym.name, sym.bound_type)
                }
                SymbolKind::Constant => format!("Const {}", sym.name),
                SymbolKind::External => format!("Declare {}", sym.name),
                _ => sym.name.clone(),
            };
            return Some(HoverInfo {
                label,
                detail: Some(format!(
                    "{:?} | {}",
                    sym.bound_type,
                    semantic_provenance_label(&sym.provenance, &resolved.document)
                )),
                symbol_identity: Some(sym.identity.clone()),
                provenance: Some(sym.provenance.clone()),
            });
        }

        // Check intrinsics
        let ident = self.identifier_at_position(module, position)?;
        let ident_lower = ident.to_ascii_lowercase();
        if let Some(spec) = intrinsic_spec(&ident_lower) {
            return Some(HoverInfo {
                label: format!("Intrinsic function: {ident}"),
                detail: Some(format!(
                    "Args: {}-{}, Surface: {:?}",
                    spec.min_arity, spec.max_arity, spec.surface
                )),
                symbol_identity: None,
                provenance: None,
            });
        }

        None
    }

    // ── Internal helpers ────────────────────────────────────

    /// Determine which scope (0=module, N=proc index) the position falls in.
    fn scope_at_position(&self, module: &DocumentId, position: Position) -> u32 {
        let snap = match self.workspace.snapshot(module) {
            Some(s) => s,
            None => return 0,
        };

        let root = snap.parse.syntax();
        let proc_kinds = [
            SyntaxKind::SubDecl,
            SyntaxKind::FunctionDecl,
            SyntaxKind::PropertyDecl,
        ];

        for (idx, node) in root
            .child_nodes()
            .into_iter()
            .filter(|n| proc_kinds.contains(&n.kind()))
            .enumerate()
        {
            let (start, end) = node.text_range();
            if position >= start && position < end {
                return (idx + 1) as u32;
            }
        }
        0
    }

    /// Find the identifier token at or near a byte position.
    fn identifier_at_position(&self, module: &DocumentId, position: Position) -> Option<String> {
        let snap = self.workspace.snapshot(module)?;
        let root = snap.parse.syntax();
        let all_tokens = collect_all_tokens(&root);

        for (kind, text, offset) in &all_tokens {
            if *kind == SyntaxKind::Ident {
                let end = offset + text.len() as u32;
                if position >= *offset && position < end {
                    return Some(text.to_string());
                }
            }
        }
        None
    }

    fn identifier_span_at_position(
        &self,
        module: &DocumentId,
        position: Position,
    ) -> Option<TextSpan> {
        let snap = self.workspace.snapshot(module)?;
        let root = snap.parse.syntax();
        let all_tokens = collect_all_tokens(&root);

        for (kind, text, offset) in &all_tokens {
            if *kind == SyntaxKind::Ident {
                let end = offset + text.len() as u32;
                if position >= *offset && position <= end {
                    return Some(TextSpan::new(*offset, end));
                }
            }
        }

        None
    }

    fn local_declaration_insertion_span(
        &self,
        module: &DocumentId,
        position: Position,
    ) -> Option<TextSpan> {
        let snapshot = self.workspace.snapshot(module)?;
        let scope = self.scope_at_position(module, position);
        if scope == 0 {
            return None;
        }

        let proc_kinds = [
            SyntaxKind::SubDecl,
            SyntaxKind::FunctionDecl,
            SyntaxKind::PropertyDecl,
        ];
        let node = snapshot
            .parse
            .syntax()
            .child_nodes()
            .into_iter()
            .filter(|n| proc_kinds.contains(&n.kind()))
            .nth((scope - 1) as usize)?;
        let (start, _) = node.text_range();
        let source = snapshot.source.as_ref().as_bytes();
        let mut index = start as usize;

        while index < source.len() {
            match source[index] {
                b'\n' => {
                    let insert_at = (index + 1) as u32;
                    return Some(TextSpan::new(insert_at, insert_at));
                }
                b'\r' => {
                    let mut insert_at = index + 1;
                    if insert_at < source.len() && source[insert_at] == b'\n' {
                        insert_at += 1;
                    }
                    let insert_at = insert_at as u32;
                    return Some(TextSpan::new(insert_at, insert_at));
                }
                _ => index += 1,
            }
        }

        Some(TextSpan::new(start, start))
    }

    fn undeclared_variable_anchor_span(
        &self,
        module: &DocumentId,
        source: &str,
        fallback: TextSpan,
        identifier: &str,
    ) -> Option<TextSpan> {
        if !fallback.is_empty() {
            return Some(fallback);
        }

        let snapshot = self.workspace.snapshot(module)?;
        for (kind, text, offset) in collect_all_tokens(&snapshot.parse.syntax()) {
            if kind != SyntaxKind::Ident || !text.eq_ignore_ascii_case(identifier) {
                continue;
            }

            if self.resolve_symbol_at(module, offset).is_none() {
                return Some(TextSpan::new(offset, offset + text.len() as u32));
            }
        }

        find_identifier_span_in_source(source, identifier)
    }

    /// Extract the partial identifier (prefix) at cursor position for
    /// completion filtering. Returns empty string if cursor is not
    /// inside or right after an identifier.
    fn prefix_at_position(&self, module: &DocumentId, position: Position) -> String {
        let snap = match self.workspace.snapshot(module) {
            Some(s) => s,
            None => return String::new(),
        };
        let root = snap.parse.syntax();
        let all_tokens = collect_all_tokens(&root);

        for (kind, text, offset) in &all_tokens {
            if *kind == SyntaxKind::Ident {
                let end = offset + text.len() as u32;
                // Cursor is inside or at the end of this identifier
                if position >= *offset && position <= end {
                    let len = (position - offset) as usize;
                    return text[..len.min(text.len())].to_string();
                }
            }
        }
        String::new()
    }

    fn resolve_symbol_at(&self, module: &DocumentId, position: Position) -> Option<ResolvedSymbol> {
        let snap = self.workspace.snapshot(module)?;

        if let Some(symbol) = snap.symbols.symbol_at(position) {
            return Some(ResolvedSymbol {
                document: module.clone(),
                symbol: symbol.clone(),
            });
        }

        let ident = self.identifier_at_position(module, position)?;
        let ident_lower = ident.to_ascii_lowercase();
        let scope = self.scope_at_position(module, position);

        if let Some(symbol) = snap
            .symbols
            .symbols
            .iter()
            .find(|sym| sym.scope == scope && sym.name.to_ascii_lowercase() == ident_lower)
        {
            return Some(ResolvedSymbol {
                document: module.clone(),
                symbol: symbol.clone(),
            });
        }

        if let Some(symbol) = snap
            .symbols
            .symbols
            .iter()
            .find(|sym| sym.scope == 0 && sym.name.to_ascii_lowercase() == ident_lower)
        {
            return Some(ResolvedSymbol {
                document: module.clone(),
                symbol: symbol.clone(),
            });
        }

        let mut candidates = self
            .workspace
            .cross_module_symbols(&ident_lower)
            .iter()
            .filter(|sym| sym.identity.document_id != module.0)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            symbol_provenance_priority(&left.provenance)
                .cmp(&symbol_provenance_priority(&right.provenance))
                .then_with(|| left.identity.document_id.cmp(&right.identity.document_id))
                .then_with(|| left.identity.scope.cmp(&right.identity.scope))
                .then_with(|| left.name.cmp(&right.name))
        });

        let symbol = (*candidates.first()?).clone();
        Some(ResolvedSymbol {
            document: DocumentId::new(symbol.identity.document_id.clone()),
            symbol,
        })
    }

    fn location_for_symbol(&self, document: &DocumentId, symbol: &SymbolInfo) -> Location {
        Location {
            document: document.clone(),
            span: symbol.definition_span,
            symbol_identity: Some(symbol.identity.clone()),
            provenance: Some(symbol.provenance.clone()),
        }
    }

    fn resolve_callable_signature(
        &self,
        module: &DocumentId,
        name_lower: &str,
    ) -> Option<(DocumentId, CallableSignatureInfo, SemanticProvenance)> {
        let current = self.workspace.snapshot(module)?;
        if let Some(callable) = current
            .callables
            .iter()
            .find(|callable| callable.name.to_ascii_lowercase() == name_lower)
        {
            let provenance = self
                .workspace
                .document(module)
                .map(|document| document.semantic_provenance())?;
            return Some((module.clone(), callable.clone(), provenance));
        }

        for document in self.workspace.document_ids() {
            if document == module {
                continue;
            }
            let snap = match self.workspace.snapshot(document) {
                Some(snapshot) => snapshot,
                None => continue,
            };
            if let Some(callable) = snap
                .callables
                .iter()
                .find(|callable| callable.name.to_ascii_lowercase() == name_lower)
            {
                let provenance = self
                    .workspace
                    .document(document)
                    .map(|doc| doc.semantic_provenance())?;
                return Some((document.clone(), callable.clone(), provenance));
            }
        }

        None
    }
}

// ── LanguageServiceProvider impl ──────────────────────────────

impl LanguageServiceProvider for LanguageService {
    fn diagnostics(&self, _project: &ProjectManifest, module: &str) -> Vec<SpannedDiagnostic> {
        let id = DocumentId::new(module);
        self.diagnostics(&id)
    }

    fn symbols(&self, _project: &ProjectManifest, module: &str) -> Vec<SymbolInfo> {
        let id = DocumentId::new(module);
        self.symbols(&id)
    }

    fn document_symbols(&self, _project: &ProjectManifest, module: &str) -> Vec<DocumentSymbol> {
        let id = DocumentId::new(module);
        self.document_symbols(&id)
    }

    fn workspace_symbols(&self, _project: &ProjectManifest, query: &str) -> Vec<WorkspaceSymbol> {
        self.workspace_symbols(query)
    }

    fn semantic_classifications(
        &self,
        _project: &ProjectManifest,
        module: &str,
    ) -> Vec<SemanticClassification> {
        let id = DocumentId::new(module);
        self.semantic_classifications(&id)
    }

    fn completions(
        &self,
        _project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Vec<CompletionItem> {
        let id = DocumentId::new(module);
        self.completions(&id, pos)
    }

    fn signature_help(
        &self,
        _project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<SignatureHelp> {
        let id = DocumentId::new(module);
        self.signature_help(&id, pos)
    }

    fn go_to_definition(
        &self,
        _project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<Location> {
        let id = DocumentId::new(module);
        self.go_to_definition(&id, pos)
    }

    fn find_references(
        &self,
        _project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Vec<Location> {
        let id = DocumentId::new(module);
        self.find_references(&id, pos)
    }

    fn prepare_rename(
        &self,
        _project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<RenamePreparation> {
        let id = DocumentId::new(module);
        self.prepare_rename(&id, pos)
    }

    fn reference_update_analysis(
        &self,
        _project: &ProjectManifest,
        module: &str,
        pos: Position,
    ) -> Option<ReferenceUpdateAnalysis> {
        let id = DocumentId::new(module);
        self.reference_update_analysis(&id, pos)
    }

    fn code_actions(&self, _project: &ProjectManifest, module: &str) -> Vec<CodeActionPlan> {
        let id = DocumentId::new(module);
        self.code_actions(&id)
    }

    fn hover(&self, _project: &ProjectManifest, module: &str, pos: Position) -> Option<HoverInfo> {
        let id = DocumentId::new(module);
        self.hover(&id, pos)
    }
}

/// Collect all tokens from a syntax tree (flattened).
fn collect_all_tokens<'a>(node: &oxvba_syntax::SyntaxNode<'a>) -> Vec<(SyntaxKind, &'a str, u32)> {
    let mut result = Vec::new();
    collect_tokens_recursive(node, &mut result);
    result
}

fn collect_tokens_recursive<'a>(
    node: &oxvba_syntax::SyntaxNode<'a>,
    result: &mut Vec<(SyntaxKind, &'a str, u32)>,
) {
    for elem in node.children() {
        match elem {
            oxvba_syntax::SyntaxElement::Token(tok) => {
                result.push((tok.kind, tok.text, tok.offset));
            }
            oxvba_syntax::SyntaxElement::Node(child) => {
                collect_tokens_recursive(&child, result);
            }
        }
    }
}

fn semantic_kind_for_symbol(kind: SymbolKind) -> SemanticTokenKind {
    match kind {
        SymbolKind::Procedure => SemanticTokenKind::Procedure,
        SymbolKind::Property => SemanticTokenKind::Property,
        SymbolKind::Variable => SemanticTokenKind::Variable,
        SymbolKind::Parameter => SemanticTokenKind::Parameter,
        SymbolKind::Constant => SemanticTokenKind::Constant,
        SymbolKind::TypeDef => SemanticTokenKind::TypeDef,
        SymbolKind::EnumDef | SymbolKind::EnumMember => SemanticTokenKind::EnumDef,
        SymbolKind::Event => SemanticTokenKind::Event,
        SymbolKind::External => SemanticTokenKind::External,
    }
}

fn completion_detail(symbol: &SymbolInfo, document: &DocumentId) -> String {
    format!(
        "{:?} ({})",
        symbol.bound_type,
        semantic_provenance_label(&symbol.provenance, document)
    )
}

fn semantic_provenance_label(provenance: &SemanticProvenance, document: &DocumentId) -> String {
    let origin = match provenance.kind {
        SymbolProvenanceKind::SourceModule => "source",
        SymbolProvenanceKind::ProjectReference => "project-reference",
        SymbolProvenanceKind::ImportedTypeLibraryProjection => "imported-typelib",
        SymbolProvenanceKind::Generated => "generated",
    };
    format!("{} [{}]", document, origin)
}

fn symbol_provenance_priority(provenance: &SemanticProvenance) -> u8 {
    match provenance.kind {
        SymbolProvenanceKind::SourceModule => 0,
        SymbolProvenanceKind::ProjectReference => 1,
        SymbolProvenanceKind::ImportedTypeLibraryProjection => 2,
        SymbolProvenanceKind::Generated => 3,
    }
}

fn procedure_names_by_scope(
    root: &oxvba_syntax::SyntaxNode<'_>,
) -> std::collections::HashMap<u32, String> {
    let proc_kinds = [
        SyntaxKind::SubDecl,
        SyntaxKind::FunctionDecl,
        SyntaxKind::PropertyDecl,
    ];

    root.child_nodes()
        .into_iter()
        .filter(|node| proc_kinds.contains(&node.kind()))
        .enumerate()
        .filter_map(|(idx, node)| {
            first_identifier_token(&node).map(|(name, _, _)| ((idx + 1) as u32, name.to_string()))
        })
        .collect()
}

fn first_identifier_token<'a>(node: &oxvba_syntax::SyntaxNode<'a>) -> Option<(&'a str, u32, u32)> {
    for token in node.child_tokens() {
        if token.kind == SyntaxKind::Ident {
            return Some((
                token.text,
                token.offset,
                token.offset + token.text.len() as u32,
            ));
        }
    }
    None
}

fn undeclared_variable_name(message: &str) -> Option<&str> {
    message.strip_prefix("use of undeclared variable: ")
}

fn ptrsafe_required_diagnostic(message: &str) -> bool {
    message.contains("PtrSafe keyword is required")
}

fn preferred_newline(source: &str) -> &str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn find_identifier_span_in_source(source: &str, identifier: &str) -> Option<TextSpan> {
    let source_lower = source.to_ascii_lowercase();
    let ident_lower = identifier.to_ascii_lowercase();
    let source_bytes = source_lower.as_bytes();
    let ident_bytes = ident_lower.as_bytes();

    if ident_bytes.is_empty() || ident_bytes.len() > source_bytes.len() {
        return None;
    }

    for start in 0..=(source_bytes.len() - ident_bytes.len()) {
        let end = start + ident_bytes.len();
        if &source_bytes[start..end] != ident_bytes {
            continue;
        }

        if start > 0 && is_identifier_byte(source_bytes[start - 1]) {
            continue;
        }
        if end < source_bytes.len() && is_identifier_byte(source_bytes[end]) {
            continue;
        }

        return Some(TextSpan::new(start as u32, end as u32));
    }

    None
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn declare_ptrsafe_insertion_span(source: &str, message: &str) -> Option<TextSpan> {
    let declared_line = extract_backtick_payload(message);
    let mut offset = 0usize;
    for line in source.split_inclusive(['\n']) {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        let line_matches = declared_line
            .as_deref()
            .is_some_and(|declared_line| trimmed.trim_start() == declared_line)
            || declared_line.is_none() && declare_without_ptrsafe_line(trimmed.trim_start());
        if !line_matches {
            offset += line.len();
            continue;
        }

        let leading_ws = trimmed.len() - trimmed.trim_start().len();
        let declare_idx = trimmed[leading_ws..]
            .to_ascii_lowercase()
            .find("declare ")?;
        let insert_at = offset + leading_ws + declare_idx + "Declare ".len();
        return Some(TextSpan::new(insert_at as u32, insert_at as u32));
    }

    None
}

fn declare_without_ptrsafe_line(trimmed_line: &str) -> bool {
    let lower = trimmed_line.to_ascii_lowercase();
    !trimmed_line.starts_with('\'')
        && lower.starts_with("declare ")
        && !lower.starts_with("declare ptrsafe ")
}

fn extract_backtick_payload(message: &str) -> Option<String> {
    let start = message.find('`')?;
    let rest = &message[start + 1..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
}

fn dedupe_completions(items: &mut Vec<CompletionItem>, current_module: &DocumentId) {
    let mut best_by_key =
        std::collections::HashMap::<(String, CompletionKind), CompletionItem>::new();
    for item in items.drain(..) {
        let key = (item.label.to_ascii_lowercase(), item.kind);
        let replace = match best_by_key.get(&key) {
            Some(existing) => is_better_completion_candidate(&item, existing, current_module),
            None => true,
        };
        if replace {
            best_by_key.insert(key, item);
        }
    }
    *items = best_by_key.into_values().collect();
    items.sort_by(|left, right| {
        completion_priority(left, current_module)
            .cmp(&completion_priority(right, current_module))
            .then_with(|| left.label.cmp(&right.label))
    });
}

fn completion_priority(item: &CompletionItem, current_module: &DocumentId) -> u8 {
    match item.source_document.as_ref() {
        Some(document) if document == current_module => 0,
        Some(_) => 1,
        None => 2,
    }
}

fn is_better_completion_candidate(
    candidate: &CompletionItem,
    existing: &CompletionItem,
    current_module: &DocumentId,
) -> bool {
    completion_priority(candidate, current_module) < completion_priority(existing, current_module)
        || (completion_priority(candidate, current_module)
            == completion_priority(existing, current_module)
            && candidate.source_document.as_ref().map(|doc| &doc.0)
                < existing.source_document.as_ref().map(|doc| &doc.0))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::span::SymbolProvenanceKind;
    use crate::workspace::Workspace;
    use oxvba_compiler::{
        ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectManifest, ProjectReference,
        ReferenceKind, ReferencedProjectManifest, module_unit_from_source,
    };

    fn setup_single_module(source: &str) -> (LanguageService, DocumentId) {
        let mut ws = Workspace::new();
        let id = DocumentId::new("TestModule");
        ws.open_document(id.clone(), source);
        (LanguageService::new(ws), id)
    }

    #[test]
    fn v02_direct_language_service_provider_exercises_product_matrix_queries() {
        let src = "Option Explicit\nPublic Sub Foo(ByVal value As Long)\nEnd Sub\nPublic Sub Bar()\n    Dim localValue As Long\n    localValue = 1\n    Foo localValue\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);
        let project = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: Vec::new(),
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let diagnostic_src = "Option Explicit\nSub Main()\n    missingValue = 1\nEnd Sub\n";
        let (diagnostic_svc, _) = setup_single_module(diagnostic_src);
        let diagnostics =
            LanguageServiceProvider::diagnostics(&diagnostic_svc, &project, "TestModule");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("undeclared variable")),
            "expected undeclared-variable diagnostic"
        );

        let document_symbols =
            LanguageServiceProvider::document_symbols(&svc, &project, "TestModule");
        assert!(document_symbols.iter().any(|symbol| symbol.name == "Foo"));
        assert!(
            document_symbols
                .iter()
                .any(|symbol| symbol.name == "localValue"
                    && symbol.container_name.as_deref() == Some("Bar"))
        );

        let workspace_symbols = LanguageServiceProvider::workspace_symbols(&svc, &project, "Fo");
        assert!(
            workspace_symbols
                .iter()
                .any(|symbol| symbol.symbol.name == "Foo")
        );

        let classifications =
            LanguageServiceProvider::semantic_classifications(&svc, &project, "TestModule");
        assert!(
            classifications
                .iter()
                .any(|classification| classification.kind == SemanticTokenKind::Keyword)
        );

        let intrinsic_src =
            "Sub Main()\n    Dim localValue As Long\n    localValue = Abs(1)\nEnd Sub\n";
        let (intrinsic_svc, _) = setup_single_module(intrinsic_src);
        let intrinsic_classifications = LanguageServiceProvider::semantic_classifications(
            &intrinsic_svc,
            &project,
            "TestModule",
        );
        assert!(
            intrinsic_classifications
                .iter()
                .any(|classification| classification.kind == SemanticTokenKind::Intrinsic)
        );

        let completion_pos = src.find("localValue").expect("localValue") as u32;
        let completions =
            LanguageServiceProvider::completions(&svc, &project, "TestModule", completion_pos);
        assert!(completions.iter().any(|item| item.label == "Foo"));
        assert!(completions.iter().any(|item| item.label == "Abs"));

        let signature_src =
            "Sub Multi(a As Long, b As String)\nEnd Sub\nSub Test()\n    Multi 1, \"x\"\nEnd Sub\n";
        let (signature_svc, _) = setup_single_module(signature_src);
        let signature_pos =
            (signature_src.find("Multi 1").expect("Multi call") + "Multi 1, ".len()) as u32;
        let signature = LanguageServiceProvider::signature_help(
            &signature_svc,
            &project,
            "TestModule",
            signature_pos,
        )
        .expect("signature help for Multi");
        assert_eq!(signature.name, "Multi");
        assert_eq!(signature.parameters.len(), 2);
        assert_eq!(signature.active_parameter, 1);

        let local_use = src.rfind("localValue").expect("localValue use") as u32;
        let hover = LanguageServiceProvider::hover(&svc, &project, "TestModule", local_use)
            .expect("hover for localValue");
        assert!(hover.label.contains("localValue"));

        let foo_call = src.find("Foo localValue").expect("Foo call") as u32;
        let definition =
            LanguageServiceProvider::go_to_definition(&svc, &project, "TestModule", foo_call)
                .expect("definition for Foo");
        assert_eq!(definition.document, id);
        assert!(definition.symbol_identity.is_some());

        let references =
            LanguageServiceProvider::find_references(&svc, &project, "TestModule", foo_call);
        assert!(
            references.len() >= 2,
            "expected declaration and call-site references"
        );

        let rename =
            LanguageServiceProvider::prepare_rename(&svc, &project, "TestModule", foo_call)
                .expect("rename preparation for Foo");
        assert_eq!(rename.current_name, "Foo");
        assert!(rename.reference_analysis.safe_to_apply);

        let analysis = LanguageServiceProvider::reference_update_analysis(
            &svc,
            &project,
            "TestModule",
            foo_call,
        )
        .expect("reference update analysis for Foo");
        assert!(analysis.safe_to_apply);
        assert!(!analysis.references.is_empty());

        let actions =
            LanguageServiceProvider::code_actions(&diagnostic_svc, &project, "TestModule");
        assert!(
            actions
                .iter()
                .any(|action| action.title.starts_with("Declare local variable")),
            "expected an undeclared-variable quick fix, got {actions:?}"
        );
    }

    #[test]
    fn diagnostics_for_valid_code() {
        let (svc, id) = setup_single_module("Sub Foo()\nEnd Sub\n");
        let diags = svc.diagnostics(&id);
        // Valid code should have no diagnostics (or only benign ones)
        for d in &diags {
            eprintln!("diag: {}", d.message);
        }
    }

    #[test]
    fn completions_include_keywords_and_symbols() {
        let (svc, id) = setup_single_module("Sub Foo()\n    Dim x As Long\nEnd Sub\n");
        let completions = svc.completions(&id, 20);

        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"Sub"), "should include keyword Sub");
        assert!(labels.contains(&"Abs"), "should include intrinsic Abs");
        assert!(labels.contains(&"Foo"), "should include procedure Foo");
    }

    #[test]
    fn document_symbols_include_container_context() {
        let src = "Public Sub Foo()\n    Dim counter As Long\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let symbols = svc.document_symbols(&id);
        assert!(
            symbols
                .iter()
                .any(|sym| sym.name == "Foo" && sym.kind == SymbolKind::Procedure),
            "expected procedure symbol in document outline"
        );
        assert!(
            symbols.iter().any(|sym| {
                sym.name == "counter"
                    && sym.kind == SymbolKind::Variable
                    && sym.container_name.as_deref() == Some("Foo")
            }),
            "expected local variable to carry containing procedure name"
        );
    }

    #[test]
    fn document_symbols_match_local_container_in_multi_procedure_module() {
        let src = "Public Sub Foo()\n    Dim firstLocal As Long\nEnd Sub\nPublic Sub Bar()\n    Dim secondLocal As Long\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let symbols = svc.document_symbols(&id);
        assert!(
            symbols.iter().any(|sym| {
                sym.name == "firstLocal" && sym.container_name.as_deref() == Some("Foo")
            }),
            "expected first local variable to stay attached to Foo"
        );
        assert!(
            symbols.iter().any(|sym| {
                sym.name == "secondLocal" && sym.container_name.as_deref() == Some("Bar")
            }),
            "expected second local variable to stay attached to Bar"
        );
    }

    #[test]
    fn workspace_symbols_filter_across_loaded_workspace() {
        let project = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..ModuleAttributes::default()
                },
                source: "Public Sub Main()\nEnd Sub\n".to_string(),
            }],
            references: vec![ProjectReference {
                referenced_project_name: "Core".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Core".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc()\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let svc = LanguageService::from_project(project);
        let symbols = svc.workspace_symbols("Sh");
        assert!(
            symbols.iter().any(|item| {
                item.document == DocumentId::new("Core::Shared") && item.symbol.name == "SharedProc"
            }),
            "expected workspace symbol search to include referenced-project exports"
        );
    }

    #[test]
    fn workspace_symbols_cover_frontend_seed_project_routes() {
        let intp_003_main_source =
            include_str!("../../../conformance/integration/projects/INTP-003/main/Main.proc.bas");
        let main = module_unit_from_source("Main", ModuleKind::Procedural, intp_003_main_source)
            .expect("main module parses");
        let math_api = module_unit_from_source(
            "MathApi",
            ModuleKind::Procedural,
            include_str!(
                "../../../conformance/integration/projects/INTP-003/references/LibMath/MathApi.proc.bas"
            ),
        )
        .expect("reference module parses");
        let project = ProjectManifest {
            project_name: "AlphaCrossRef".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main],
            references: vec![ProjectReference {
                referenced_project_name: "LibMath".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "LibMath".to_string(),
                modules: vec![math_api],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };
        let svc = LanguageService::from_project(project);
        let add_four = svc.workspace_symbols("AddFour");
        assert!(add_four.iter().any(|item| {
            item.document == DocumentId::new("LibMath::MathApi")
                && item.symbol.name == "AddFour"
                && item.symbol.provenance.kind == SymbolProvenanceKind::ProjectReference
        }));
        let main_id = DocumentId::new("Main");
        let add_four_pos = intp_003_main_source
            .find("AddFour")
            .expect("INTP-003 AddFour call site") as u32;
        let add_four_definition = svc
            .go_to_definition(&main_id, add_four_pos)
            .expect("INTP-003 qualified reference call should navigate");
        assert_eq!(
            add_four_definition.document,
            DocumentId::new("LibMath::MathApi")
        );
        assert_eq!(
            add_four_definition
                .provenance
                .as_ref()
                .map(|provenance| provenance.kind),
            Some(SymbolProvenanceKind::ProjectReference)
        );

        let intp_016_main_source =
            include_str!("../../../conformance/integration/projects/INTP-016/main/Main.proc.bas");
        let class_main =
            module_unit_from_source("Main", ModuleKind::Procedural, intp_016_main_source)
                .expect("class project main parses");
        let counter = module_unit_from_source(
            "Counter",
            ModuleKind::Class,
            include_str!(
                "../../../conformance/integration/projects/INTP-016/main/Counter.class.bas"
            ),
        )
        .expect("counter class parses");
        let adder = module_unit_from_source(
            "Adder",
            ModuleKind::Class,
            include_str!("../../../conformance/integration/projects/INTP-016/main/Adder.class.bas"),
        )
        .expect("adder class parses");
        let class_project = ProjectManifest {
            project_name: "AlphaClasses".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![class_main, counter, adder],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: std::collections::BTreeMap::new(),
        };
        let class_svc = LanguageService::from_project(class_project);
        let multiply = class_svc.workspace_symbols("Multiply");
        assert!(multiply.iter().any(|item| {
            item.document == DocumentId::new("Adder")
                && item.symbol.name == "Multiply"
                && item.symbol.provenance.kind == SymbolProvenanceKind::SourceModule
        }));
        let class_main_id = DocumentId::new("Main");
        let multiply_pos = intp_016_main_source
            .find("Multiply")
            .expect("INTP-016 Multiply call site") as u32;
        let multiply_definition = class_svc
            .go_to_definition(&class_main_id, multiply_pos)
            .expect("INTP-016 class member call should navigate");
        assert_eq!(multiply_definition.document, DocumentId::new("Adder"));
        assert_eq!(
            multiply_definition
                .provenance
                .as_ref()
                .map(|provenance| provenance.kind),
            Some(SymbolProvenanceKind::SourceModule)
        );
        let multiply_sig = class_svc
            .signature_help(&class_main_id, multiply_pos + "Multiply(5".len() as u32)
            .expect("INTP-016 class member call should expose signature help");
        assert_eq!(multiply_sig.name, "Multiply");
        assert_eq!(multiply_sig.parameters.len(), 2);
        assert_eq!(multiply_sig.source_document, Some(DocumentId::new("Adder")));
    }

    #[test]
    fn semantic_classifications_cover_keywords_symbols_and_intrinsics() {
        let src = "Sub Foo()\n    Dim count As Long\n    count = 1\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let classifications = svc.semantic_classifications(&id);

        assert!(
            classifications
                .iter()
                .any(|entry| entry.kind == SemanticTokenKind::Keyword),
            "expected keyword classifications"
        );
        assert!(
            classifications.iter().any(|entry| {
                entry.kind == SemanticTokenKind::Variable && entry.symbol_identity.is_some()
            }),
            "expected resolved variable classification"
        );

        let intrinsic_src = "Sub Foo()\n    Dim count As Long\n    count = Abs(1)\nEnd Sub\n";
        let (intrinsic_svc, intrinsic_id) = setup_single_module(intrinsic_src);
        let intrinsic_classifications = intrinsic_svc.semantic_classifications(&intrinsic_id);
        assert!(
            intrinsic_classifications
                .iter()
                .any(|entry| entry.kind == SemanticTokenKind::Intrinsic),
            "expected intrinsic classification"
        );
    }

    #[test]
    fn go_to_definition_finds_procedure() {
        let src = "Sub Foo()\nEnd Sub\nSub Bar()\n    Foo\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        // Position at "Foo" in Bar's body
        let foo_in_bar = src.rfind("Foo").unwrap() as u32;
        let loc = svc.go_to_definition(&id, foo_in_bar);
        assert!(loc.is_some(), "should find definition of Foo");
    }

    #[test]
    fn find_references_returns_all_usages() {
        let src = "Sub Foo()\nEnd Sub\nSub Bar()\n    Foo\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let foo_pos = src.find("Foo").unwrap() as u32;
        let refs = svc.find_references(&id, foo_pos);
        assert!(
            refs.len() >= 2,
            "expected at least 2 references to Foo, got {}",
            refs.len()
        );
    }

    #[test]
    fn find_references_excludes_unresolved_same_name_tokens() {
        let src = "Sub Foo()\nEnd Sub\nSub Bar()\n    Foo\nEnd Sub\nSub Baz()\n    Dim Foo As Long\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let foo_pos = src.find("Foo").unwrap() as u32;
        let refs = svc.find_references(&id, foo_pos);
        let baz_local = src.rfind("Foo").unwrap() as u32;

        assert!(
            refs.iter().all(|location| location.span.start != baz_local),
            "expected unrelated local variable with the same name to be excluded from procedure references"
        );
        assert_eq!(
            refs.len(),
            2,
            "expected only declaration and call-site references"
        );
    }

    #[test]
    fn hover_shows_type_info() {
        let src = "Sub Test()\n    Dim count As Long\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let count_pos = src.find("count").unwrap() as u32;
        let hover = svc.hover(&id, count_pos);
        assert!(hover.is_some(), "should show hover for count");
        let h = hover.unwrap();
        assert!(h.label.contains("count"), "label: {}", h.label);
    }

    #[test]
    fn hover_intrinsic_function() {
        let src = "Sub Test()\n    Dim x As Long\n    x = Abs(-1)\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let abs_pos = src.find("Abs").unwrap() as u32;
        let hover = svc.hover(&id, abs_pos);
        assert!(hover.is_some(), "should show hover for Abs");
    }

    #[test]
    fn hover_includes_provenance_for_cross_project_symbol() {
        let project = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..ModuleAttributes::default()
                },
                source: "Public Sub Main()\n    SharedProc\nEnd Sub\n".to_string(),
            }],
            references: vec![ProjectReference {
                referenced_project_name: "Core".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Core".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc()\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let svc = LanguageService::from_project(project);
        let main_id = DocumentId::new("Main");
        let pos = "Public Sub Main()\n    SharedProc\nEnd Sub\n"
            .find("SharedProc")
            .expect("call site") as u32;
        let hover = svc.hover(&main_id, pos).expect("hover should resolve");
        assert!(
            hover
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("project-reference")),
            "hover detail should surface provenance"
        );
    }

    #[test]
    fn prepare_rename_returns_placeholder_and_safe_local_reference_analysis() {
        let src = "Public Sub Foo()\nEnd Sub\nPublic Sub Bar()\n    Foo\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let foo_call = src.rfind("Foo").unwrap() as u32;
        let preparation = svc
            .prepare_rename(&id, foo_call)
            .expect("rename preparation");

        assert_eq!(preparation.current_name, "Foo");
        assert_eq!(preparation.placeholder.start, foo_call);
        assert!(preparation.reference_analysis.safe_to_apply);
        assert_eq!(preparation.reference_analysis.references.len(), 2);
        assert_eq!(
            preparation.reference_analysis.writable_documents,
            vec![id.clone()]
        );
    }

    #[test]
    fn reference_update_analysis_blocks_imported_typelib_projections() {
        let project = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..ModuleAttributes::default()
                },
                source: "Public Sub Main()\n    SharedProc\nEnd Sub\n".to_string(),
            }],
            references: vec![ProjectReference {
                referenced_project_name: "Lib".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Lib".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc()\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let svc = LanguageService::from_project(project);
        let main_id = DocumentId::new("Main");
        let call_pos = "Public Sub Main()\n    SharedProc\nEnd Sub\n"
            .find("SharedProc")
            .expect("call site") as u32;

        let analysis = svc
            .reference_update_analysis(&main_id, call_pos)
            .expect("reference update analysis");
        assert!(
            !analysis.safe_to_apply,
            "imported typelib projections should not be treated as writable rename targets"
        );
        assert!(
            analysis.issues.iter().any(|issue| {
                issue.kind == ReferenceUpdateIssueKind::ImportedTypeLibraryDocument
            }),
            "expected imported-typelib blocker in analysis"
        );
    }

    #[test]
    fn code_actions_offer_local_declaration_for_undeclared_variable_diagnostic() {
        let src = "Option Explicit\nSub Main()\n    x = 1\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let actions = svc.code_actions(&id);
        assert_eq!(actions.len(), 1, "expected one quick-fix plan");
        let action = &actions[0];
        assert_eq!(action.kind, CodeActionKind::QuickFix);
        assert_eq!(action.title, "Declare local variable 'x'");
        assert_eq!(action.document, id);
        assert_eq!(action.edits.len(), 1);
        assert_eq!(
            action.edits[0].new_text, "    Dim x As Variant\n",
            "expected local declaration insertion"
        );
        assert!(
            action.diagnostic.message.contains("undeclared variable"),
            "expected diagnostic-driven quick fix"
        );
    }

    #[test]
    fn code_actions_stay_empty_without_matching_fix_family() {
        let src = "Sub Main()\n    Dim x As Long\n    x = 1\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let actions = svc.code_actions(&id);
        assert!(
            actions.is_empty(),
            "expected no quick fixes for a clean document"
        );
    }

    #[test]
    fn code_actions_offer_ptrsafe_insertion_for_declare_diagnostic() {
        let src = "Declare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let actions = svc.code_actions(&id);
        assert_eq!(actions.len(), 1, "expected one PtrSafe quick-fix plan");
        let action = &actions[0];
        assert_eq!(action.title, "Add PtrSafe keyword");
        assert_eq!(action.kind, CodeActionKind::QuickFix);
        assert_eq!(action.edits.len(), 1);
        assert_eq!(action.edits[0].new_text, "PtrSafe ");
        let insert_at = "Declare ".len() as u32;
        assert_eq!(action.edits[0].span, TextSpan::new(insert_at, insert_at));
        assert!(
            action
                .diagnostic
                .message
                .contains("PtrSafe keyword is required"),
            "expected PtrSafe diagnostic-driven quick fix"
        );
    }

    #[test]
    fn undeclared_variable_quick_fix_targets_unresolved_occurrence_not_first_name_match() {
        let src = "Sub First()\n    Dim x As Long\n    x = 1\nEnd Sub\nOption Explicit\nSub Second()\n    x = 2\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let actions = svc.code_actions(&id);
        assert_eq!(
            actions.len(),
            1,
            "expected one undeclared-variable quick fix"
        );
        let insert_at =
            "Sub First()\n    Dim x As Long\n    x = 1\nEnd Sub\nOption Explicit\nSub Second()\n"
                .len() as u32;
        assert_eq!(
            actions[0].edits[0].span,
            TextSpan::new(insert_at, insert_at),
            "expected declaration insertion in the second procedure, not at the first matching identifier"
        );
    }

    #[test]
    fn ptrsafe_quick_fix_ignores_comment_line_with_matching_text() {
        let src = "' Declare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nDeclare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\nSub Main()\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let actions = svc.code_actions(&id);
        assert_eq!(actions.len(), 1, "expected one PtrSafe quick-fix plan");
        let commented_len =
            "' Declare Function HostPing Lib \"host\" Alias \"ping\" (ByVal x As Long) As Long\n"
                .len() as u32;
        let insert_at = commented_len + "Declare ".len() as u32;
        assert_eq!(
            actions[0].edits[0].span,
            TextSpan::new(insert_at, insert_at),
            "expected PtrSafe insertion on the real declaration line, not the commented text"
        );
    }

    #[test]
    fn cross_module_go_to_definition() {
        let mut ws = Workspace::new();
        let mod1 = DocumentId::new("Module1");
        let mod2 = DocumentId::new("Module2");
        ws.open_document(mod1.clone(), "Sub SharedProc()\nEnd Sub\n");
        ws.open_document(mod2.clone(), "Sub Caller()\n    SharedProc\nEnd Sub\n");

        let svc = LanguageService::new(ws);

        let src2 = "Sub Caller()\n    SharedProc\nEnd Sub\n";
        let pos = src2.find("SharedProc").unwrap() as u32;
        let loc = svc.go_to_definition(&mod2, pos);
        assert!(loc.is_some(), "should find SharedProc in Module1");
        assert_eq!(loc.unwrap().document, mod1);
    }

    #[test]
    fn completions_prefix_filtering() {
        let (svc, id) = setup_single_module("Sub Foo()\n    Dim count As Long\nEnd Sub\n");
        // Position at "co" inside "count" (offset of 'c' + 2)
        let src = "Sub Foo()\n    Dim count As Long\nEnd Sub\n";
        let count_pos = src.find("count").unwrap() as u32 + 2; // "co"
        let completions = svc.completions(&id, count_pos);

        // All returned items should start with "co" (case-insensitive)
        for item in &completions {
            assert!(
                item.label.to_ascii_lowercase().starts_with("co"),
                "completion '{}' does not start with 'co'",
                item.label
            );
        }
        // Should include Const keyword and count variable
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"Const"), "should include Const keyword");
        assert!(labels.contains(&"count"), "should include count variable");
    }

    #[test]
    fn completions_deduplicate_cross_workspace_candidates() {
        let mut ws = Workspace::new();
        let main = DocumentId::new("Main");
        ws.open_document(main.clone(), "Public Sub Main()\n    SharedProc\nEnd Sub\n");
        ws.open_document(
            DocumentId::new("Shared"),
            "Public Sub SharedProc()\nEnd Sub\n",
        );
        ws.open_document(
            DocumentId::new("SharedCopy"),
            "Public Sub SharedProc()\nEnd Sub\n",
        );
        let svc = LanguageService::new(ws);

        let pos = "Public Sub Main()\n    SharedProc\nEnd Sub\n"
            .find("SharedProc")
            .expect("call site") as u32
            + 2;
        let completions = svc.completions(&main, pos);
        let shared_proc_count = completions
            .iter()
            .filter(|item| item.label == "SharedProc")
            .count();
        assert_eq!(
            shared_proc_count, 1,
            "completion surface should dedupe matching cross-workspace symbols"
        );
    }

    #[test]
    fn signature_help_paren_depth() {
        // f(g(x), y) — cursor at 'x' inside g() should identify 'g', not 'f'
        let src = "Sub f(a As Long, b As Long)\nEnd Sub\nSub g(x As Long)\nEnd Sub\nSub Test()\n    f g(1), 2\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        // Position inside g(1) — after the '1'
        let g_call = src.find("g(1)").unwrap() as u32;
        let inside_g = g_call + 2; // at '1' inside g()
        let help = svc.signature_help(&id, inside_g);
        if let Some(h) = &help {
            assert_eq!(
                h.name.to_ascii_lowercase(),
                "g",
                "expected sig help for 'g', got '{}'",
                h.name
            );
        }
    }

    #[test]
    fn signature_help_active_parameter() {
        let src = "Sub Multi(a As Long, b As String, c As Double)\nEnd Sub\nSub Test()\n    Multi 1, \"hello\", 3.14\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        // Position after second comma — should be parameter index 2
        let call_line = src.find("Multi 1").unwrap();
        // Find the second comma
        let rest = &src[call_line..];
        let first_comma = rest.find(',').unwrap();
        let second_comma = rest[first_comma + 1..].find(',').unwrap() + first_comma + 1;
        let pos = (call_line + second_comma + 2) as u32; // after second comma + space
        let help = svc.signature_help(&id, pos);
        if let Some(h) = &help {
            assert_eq!(
                h.active_parameter, 2,
                "expected active_parameter=2, got {}",
                h.active_parameter
            );
        }
    }

    #[test]
    fn signature_help_preserves_optional_string_boolean_defaults() {
        let src = "Const CBase = &H10 + 1\nEnum Mode\nFast = 3\nSafe\nEnd Enum\nSub Use(Optional ByVal text As String = \"ready\", Optional ByVal flag As Boolean = True, Optional ByVal value As Long = CBase + Safe)\nEnd Sub\nSub Test()\n    Call Use()\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);
        let pos = src.find("Use()").expect("call") as u32 + "Use(".len() as u32;
        let help = svc
            .signature_help(&id, pos)
            .expect("signature help for optional defaults");

        assert_eq!(help.name, "Use");
        assert_eq!(help.parameters.len(), 3);
        assert_eq!(help.parameters[0].name, "text");
        assert!(help.parameters[0].optional);
        assert_eq!(
            help.parameters[0].default_value,
            Some(OptionalDefaultValue::ExplicitString("ready".to_string()))
        );
        assert_eq!(help.parameters[1].name, "flag");
        assert!(help.parameters[1].optional);
        assert_eq!(
            help.parameters[1].default_value,
            Some(OptionalDefaultValue::ExplicitBool(true))
        );
        assert_eq!(help.parameters[2].name, "value");
        assert!(help.parameters[2].optional);
        assert_eq!(
            help.parameters[2].default_value,
            Some(OptionalDefaultValue::ExplicitI32(21))
        );
    }

    #[test]
    fn signature_help_resolves_property_get_alias_from_frontend_hir() {
        let src = "Property Get Value(ByVal index As Long) As Long\nValue = index\nEnd Property\nProperty Let Value(ByVal index As Long, ByVal newValue As Long)\nEnd Property\nSub Test()\n    Dim text\n    text = Value(1)\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);
        let pos = src.find("Value(1)").expect("property call") as u32 + "Value(".len() as u32;
        let help = svc
            .signature_help(&id, pos)
            .expect("signature help for property get alias");

        assert_eq!(help.name, "Value");
        assert_eq!(help.parameters.len(), 1);
        assert_eq!(help.parameters[0].name, "index");
        assert_eq!(help.parameters[0].type_name, "Long");
        assert_eq!(help.return_type, "Long");
    }

    #[test]
    fn property_accessor_group_uses_single_frontend_symbol_identity_for_references() {
        let src = "Property Get Value(ByVal index As Long) As Long\nValue = index\nEnd Property\nProperty Let Value(ByVal index As Long, ByVal newValue As Long)\nEnd Property\nSub Test()\n    Dim text\n    text = Value(1)\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);
        let get_decl = src.find("Value").expect("property get declaration") as u32;
        let let_decl = src
            .find("Property Let Value")
            .map(|start| start + "Property Let ".len())
            .expect("property let declaration") as u32;
        let use_pos = src.find("Value(1)").expect("property get use") as u32;

        let get_definition = svc
            .go_to_definition(&id, get_decl)
            .expect("property get definition");
        let let_definition = svc
            .go_to_definition(&id, let_decl)
            .expect("property let should resolve to property group");
        let use_definition = svc
            .go_to_definition(&id, use_pos)
            .expect("property use should resolve to property group");

        assert_eq!(
            let_definition.symbol_identity,
            get_definition.symbol_identity
        );
        assert_eq!(
            use_definition.symbol_identity,
            get_definition.symbol_identity
        );

        let refs = svc.find_references(&id, get_decl);
        assert!(
            refs.iter().any(|location| location.span.start == get_decl),
            "{refs:#?}"
        );
        assert!(
            refs.iter().any(|location| location.span.start == let_decl),
            "{refs:#?}"
        );
        assert!(
            refs.iter().any(|location| location.span.start == use_pos),
            "{refs:#?}"
        );
    }

    #[test]
    fn signature_help_preserves_param_array_flag() {
        let src = "Sub Collect(ParamArray values() As Variant)\nEnd Sub\nSub Test()\n    Call Collect()\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);
        let pos = src.find("Collect()").expect("call") as u32 + "Collect(".len() as u32;
        let help = svc
            .signature_help(&id, pos)
            .expect("signature help for ParamArray");

        assert_eq!(help.name, "Collect");
        assert_eq!(help.parameters.len(), 1);
        assert_eq!(help.parameters[0].name, "values");
        assert!(help.parameters[0].param_array);
        assert!(!help.parameters[0].optional);
        assert_eq!(help.parameters[0].default_value, None);
    }

    #[test]
    fn signature_help_resolves_cross_project_callables() {
        let project = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..ModuleAttributes::default()
                },
                source: "Public Sub Main()\n    SharedProc 1, 2\nEnd Sub\n".to_string(),
            }],
            references: vec![ProjectReference {
                referenced_project_name: "Core".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Core".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc(a As Long, b As Long)\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let svc = LanguageService::from_project(project);
        let main_id = DocumentId::new("Main");
        let pos = "Public Sub Main()\n    SharedProc 1, 2\nEnd Sub\n"
            .find("1, 2")
            .expect("call arguments") as u32
            + 1;
        let help = svc
            .signature_help(&main_id, pos)
            .expect("signature help should resolve across project boundary");
        assert_eq!(help.name, "SharedProc");
        assert_eq!(help.parameters.len(), 2);
        assert_eq!(help.source_document, Some(DocumentId::new("Core::Shared")));
    }

    #[test]
    fn language_service_provider_trait() {
        let mut ws = Workspace::new();
        let id = DocumentId::new("TestModule");
        ws.open_document(id.clone(), "Sub Foo()\nEnd Sub\n");
        let svc = LanguageService::new(ws);

        let manifest = ProjectManifest {
            project_name: "TestProject".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![],
            references: vec![],
            reference_projects: vec![],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        // Call through the trait
        let provider: &dyn LanguageServiceProvider = &svc;
        let syms = provider.symbols(&manifest, "TestModule");
        assert!(!syms.is_empty(), "expected symbols from trait method");

        let completions = provider.completions(&manifest, "TestModule", 0);
        assert!(
            !completions.is_empty(),
            "expected completions from trait method"
        );
    }

    #[test]
    fn language_service_provider_go_to_def() {
        let src = "Sub Foo()\nEnd Sub\nSub Bar()\n    Foo\nEnd Sub\n";
        let mut ws = Workspace::new();
        let id = DocumentId::new("TestModule");
        ws.open_document(id, src);
        let svc = LanguageService::new(ws);

        let manifest = ProjectManifest {
            project_name: "TestProject".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![],
            references: vec![],
            reference_projects: vec![],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let provider: &dyn LanguageServiceProvider = &svc;
        let foo_pos = src.rfind("Foo").unwrap() as u32;
        let loc = provider.go_to_definition(&manifest, "TestModule", foo_pos);
        assert!(loc.is_some(), "should find Foo via trait method");
    }

    #[test]
    fn project_aware_workspace_goes_to_definition_across_project_reference() {
        let project = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..ModuleAttributes::default()
                },
                source: "Public Sub Main()\n    SharedProc\nEnd Sub\n".to_string(),
            }],
            references: vec![ProjectReference {
                referenced_project_name: "Core".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Core".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc()\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let svc = LanguageService::from_project(project);
        let main_id = DocumentId::new("Main");
        let pos = "Public Sub Main()\n    SharedProc\nEnd Sub\n"
            .find("SharedProc")
            .expect("call site") as u32;
        let loc = svc
            .go_to_definition(&main_id, pos)
            .expect("definition should resolve into reference project");
        assert_eq!(loc.document, DocumentId::new("Core::Shared"));
    }

    #[test]
    fn project_aware_workspace_loads_projected_typelib_references() {
        let project = ProjectManifest {
            project_name: "App".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Main".to_string(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: "Main".to_string(),
                    ..ModuleAttributes::default()
                },
                source: "Public Sub Main()\n    GetBaseName\n    countValue = dictionary.Count()\nEnd Sub\n".to_string(),
            }],
            references: vec![ProjectReference {
                referenced_project_name: "Scripting".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Scripting".to_string(),
                modules: vec![
                    ModuleUnit {
                        module_name: "FileSystemObject".to_string(),
                        module_kind: ModuleKind::Class,
                        attributes: ModuleAttributes {
                            vb_name: "FileSystemObject".to_string(),
                            ..ModuleAttributes::default()
                        },
                        source: "Attribute VB_Name = \"FileSystemObject\"\nPublic Function GetBaseName(Path As Variant) As Variant\nEnd Function\n".to_string(),
                    },
                    ModuleUnit {
                        module_name: "Dictionary".to_string(),
                        module_kind: ModuleKind::Class,
                        attributes: ModuleAttributes {
                            vb_name: "Dictionary".to_string(),
                            ..ModuleAttributes::default()
                        },
                        source: "Attribute VB_Name = \"Dictionary\"\nPublic Function Count() As Variant\nEnd Function\n".to_string(),
                    },
                ],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let svc = LanguageService::from_project(project);
        let main_id = DocumentId::new("Main");
        let main_source =
            "Public Sub Main()\n    GetBaseName\n    countValue = dictionary.Count()\nEnd Sub\n";
        let pos = main_source.find("GetBaseName").expect("call site") as u32;
        let loc = svc
            .go_to_definition(&main_id, pos)
            .expect("typelib-projected definition should resolve");
        assert_eq!(loc.document, DocumentId::new("Scripting::FileSystemObject"));
        assert_eq!(
            loc.provenance.as_ref().map(|provenance| provenance.kind),
            Some(SymbolProvenanceKind::ImportedTypeLibraryProjection)
        );
        assert!(loc.symbol_identity.is_some());

        let count_workspace_symbol = svc
            .workspace_symbols("Count")
            .into_iter()
            .find(|symbol| symbol.document == DocumentId::new("Scripting::Dictionary"))
            .expect("projected Dictionary.Count should be visible to workspace search");
        assert_eq!(count_workspace_symbol.symbol.name, "Count");
        assert_eq!(
            count_workspace_symbol.symbol.provenance.kind,
            SymbolProvenanceKind::ImportedTypeLibraryProjection
        );

        let count_pos = main_source.find("Count").expect("Count call site") as u32;
        let count_loc = svc
            .go_to_definition(&main_id, count_pos)
            .expect("projected Dictionary.Count should navigate");
        assert_eq!(count_loc.document, DocumentId::new("Scripting::Dictionary"));
        assert_eq!(
            count_loc
                .provenance
                .as_ref()
                .map(|provenance| provenance.kind),
            Some(SymbolProvenanceKind::ImportedTypeLibraryProjection)
        );
        assert!(count_loc.symbol_identity.is_some());

        let count_signature = svc
            .signature_help(&main_id, count_pos + "Count(".len() as u32)
            .expect("projected Dictionary.Count should expose signature help");
        assert_eq!(count_signature.name, "Count");
        assert_eq!(
            count_signature.source_document,
            Some(DocumentId::new("Scripting::Dictionary"))
        );
        assert_eq!(
            count_signature
                .provenance
                .as_ref()
                .map(|provenance| provenance.kind),
            Some(SymbolProvenanceKind::ImportedTypeLibraryProjection)
        );
        assert!(count_signature.parameters.is_empty());
    }

    #[test]
    fn go_to_definition_carries_stable_symbol_identity_across_snapshot_versions() {
        let mut ws = Workspace::new();
        let id = DocumentId::new("Module1");
        ws.open_document(id.clone(), "Public Sub Foo()\nEnd Sub\n");

        let svc = LanguageService::new(ws);
        let initial = svc
            .go_to_definition(
                &id,
                "Public Sub Foo()\nEnd Sub\n".find("Foo").unwrap() as u32,
            )
            .expect("initial definition");

        let initial_identity = initial.symbol_identity.clone().expect("symbol identity");
        let initial_version = initial
            .provenance
            .as_ref()
            .map(|provenance| provenance.snapshot_version)
            .expect("snapshot version");

        let mut ws = svc.workspace;
        ws.change_document(&id, "\nPublic Sub Foo()\nEnd Sub\n");
        let svc = LanguageService::new(ws);

        let updated = svc
            .go_to_definition(
                &id,
                "\nPublic Sub Foo()\nEnd Sub\n".find("Foo").unwrap() as u32,
            )
            .expect("updated definition");

        assert_eq!(
            updated.symbol_identity.as_ref(),
            Some(&initial_identity),
            "definition identity should survive unrelated edits"
        );
        assert!(
            updated
                .provenance
                .as_ref()
                .map(|provenance| provenance.snapshot_version)
                .expect("updated snapshot version")
                > initial_version
        );
    }

    #[test]
    fn interactive_query_harness_stays_within_local_editor_budget() {
        let mut modules = Vec::new();
        for idx in 0..24 {
            modules.push(ModuleUnit {
                module_name: format!("Module{idx}"),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes {
                    vb_name: format!("Module{idx}"),
                    ..ModuleAttributes::default()
                },
                source: if idx == 0 {
                    "Public Sub Main()\n    SharedProc\nEnd Sub\n".to_string()
                } else {
                    format!("Public Sub Worker{idx}()\nEnd Sub\n")
                },
            });
        }

        let project = ProjectManifest {
            project_name: "PerfHarness".to_string(),
            project_kind: ProjectKind::Source,
            modules,
            references: vec![ProjectReference {
                referenced_project_name: "Core".to_string(),
                reference_kind: ReferenceKind::Project,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Core".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "Shared".to_string(),
                    module_kind: ModuleKind::Procedural,
                    attributes: ModuleAttributes {
                        vb_name: "Shared".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Public Sub SharedProc()\nEnd Sub\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let open_start = Instant::now();
        let svc = LanguageService::from_project(project);
        let open_elapsed = open_start.elapsed();

        let main_id = DocumentId::new("Module0");
        let def_pos = "Public Sub Main()\n    SharedProc\nEnd Sub\n"
            .find("SharedProc")
            .expect("call site") as u32;

        let def_start = Instant::now();
        let definition = svc.go_to_definition(&main_id, def_pos);
        let def_elapsed = def_start.elapsed();
        assert!(definition.is_some(), "definition query should succeed");

        let refs_start = Instant::now();
        let refs = svc.find_references(&main_id, def_pos);
        let refs_elapsed = refs_start.elapsed();
        assert!(
            refs.len() >= 2,
            "reference query should include declaration and call site"
        );

        let interactive_budget = Duration::from_secs(2);
        assert!(
            open_elapsed < interactive_budget,
            "workspace open exceeded local editor budget: {:?}",
            open_elapsed
        );
        assert!(
            def_elapsed < interactive_budget,
            "definition query exceeded local editor budget: {:?}",
            def_elapsed
        );
        assert!(
            refs_elapsed < interactive_budget,
            "reference query exceeded local editor budget: {:?}",
            refs_elapsed
        );
    }

    #[test]
    fn scope_detection() {
        let src = "Dim g As Long\nSub First()\n    Dim a As Long\nEnd Sub\nSub Second()\n    Dim b As Long\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        // Position at module level (in "Dim g")
        assert_eq!(svc.scope_at_position(&id, 0), 0);

        // Position inside First (after "Sub First()")
        let first_body = src.find("Dim a").unwrap() as u32;
        assert_eq!(svc.scope_at_position(&id, first_body), 1);

        // Position inside Second
        let second_body = src.find("Dim b").unwrap() as u32;
        assert_eq!(svc.scope_at_position(&id, second_body), 2);
    }
}
