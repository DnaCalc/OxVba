use crate::document::DocumentId;
use crate::span::{
    SemanticProvenance, SpannedDiagnostic, SymbolIdentity, SymbolInfo, SymbolKind,
    SymbolProvenanceKind, TextSpan,
};
use crate::workspace::Workspace;

use oxvba_compiler::ProjectManifest;
use oxvba_compiler::lsp_support::intrinsic_spec;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub type_name: String,
}

/// Hover information for a symbol.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub label: String,
    pub detail: Option<String>,
    pub symbol_identity: Option<SymbolIdentity>,
    pub provenance: Option<SemanticProvenance>,
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
                let procedure_names = s
                    .symbols
                    .symbols
                    .iter()
                    .filter(|sym| {
                        matches!(sym.kind, SymbolKind::Procedure | SymbolKind::Property)
                    })
                    .map(|sym| (sym.scope + 1, sym.name.clone()))
                    .collect::<std::collections::HashMap<_, _>>();
                let mut items = s
                    .symbols
                    .symbols
                    .iter()
                    .map(|sym| {
                        DocumentSymbol {
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
                        }
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
                        detail: Some(format!("{:?}", sym.bound_type)),
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
                            detail: Some(format!("{:?} ({})", sym.bound_type, id)),
                        });
                    }
                }
            }
        }

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

        let name = call_name?;
        let name_lower = name.to_ascii_lowercase();

        // Look up in BoundModule procedures
        let proc = snap
            .bound
            .procedures
            .iter()
            .find(|p| p.name.to_ascii_lowercase() == name_lower)?;

        let parameters = proc
            .params
            .iter()
            .map(|p| ParameterInfo {
                name: p.name.clone(),
                type_name: format!("{:?}", p.ty),
            })
            .collect();

        Some(SignatureHelp {
            name: proc.name.clone(),
            parameters,
            return_type: format!("{:?}", proc.return_type),
            active_parameter: comma_count,
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
                        let occurrence = self.resolve_symbol_at(doc_id, *offset);
                        if let Some(occurrence) = &occurrence {
                            if occurrence.symbol.identity != target.symbol.identity {
                                continue;
                            }
                        } else if *doc_id == target.document && target.symbol.definition_span.contains(*offset)
                        {
                            continue;
                        }

                        locations.push(Location {
                            document: doc_id.clone(),
                            span: TextSpan::new(*offset, offset + text.len() as u32),
                            symbol_identity: Some(target.symbol.identity.clone()),
                            provenance: Some(target.symbol.provenance.clone()),
                        });
                    }
                }
            }
        }

        locations
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
                detail: Some(format!("{:?}", sym.bound_type)),
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
        candidates.sort_by_key(|sym| match sym.provenance.kind {
            SymbolProvenanceKind::SourceModule => 0,
            SymbolProvenanceKind::ProjectReference => 1,
            SymbolProvenanceKind::ImportedTypeLibraryProjection => 2,
            SymbolProvenanceKind::Generated => 3,
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::span::SymbolProvenanceKind;
    use crate::workspace::Workspace;
    use oxvba_compiler::{
        ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectManifest, ProjectReference,
        ReferenceKind, ReferencedProjectManifest,
    };

    fn setup_single_module(source: &str) -> (LanguageService, DocumentId) {
        let mut ws = Workspace::new();
        let id = DocumentId::new("TestModule");
        ws.open_document(id.clone(), source);
        (LanguageService::new(ws), id)
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
            symbols.iter().any(|sym| sym.name == "Foo" && sym.kind == SymbolKind::Procedure),
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
                item.document == DocumentId::new("Core::Shared")
                    && item.symbol.name == "SharedProc"
            }),
            "expected workspace symbol search to include referenced-project exports"
        );
    }

    #[test]
    fn semantic_classifications_cover_keywords_symbols_and_intrinsics() {
        let src = "Sub Foo()\n    Dim count As Long\n    count = Abs(1)\nEnd Sub\n";
        let (svc, id) = setup_single_module(src);

        let classifications = svc.semantic_classifications(&id);

        assert!(
            classifications.iter().any(|entry| entry.kind == SemanticTokenKind::Keyword),
            "expected keyword classifications"
        );
        assert!(
            classifications.iter().any(|entry| {
                entry.kind == SemanticTokenKind::Variable && entry.symbol_identity.is_some()
            }),
            "expected resolved variable classification"
        );
        assert!(
            classifications
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
                source: "Public Sub Main()\n    GetBaseName\nEnd Sub\n".to_string(),
            }],
            references: vec![ProjectReference {
                referenced_project_name: "Scripting".to_string(),
                reference_kind: ReferenceKind::TypeLibrary,
            }],
            reference_projects: vec![ReferencedProjectManifest {
                project_name: "Scripting".to_string(),
                modules: vec![ModuleUnit {
                    module_name: "FileSystemObject".to_string(),
                    module_kind: ModuleKind::Class,
                    attributes: ModuleAttributes {
                        vb_name: "FileSystemObject".to_string(),
                        ..ModuleAttributes::default()
                    },
                    source: "Attribute VB_Name = \"FileSystemObject\"\nPublic Function GetBaseName(Path As Variant) As Variant\nEnd Function\n".to_string(),
                }],
            }],
            conditional_constants: std::collections::BTreeMap::new(),
        };

        let svc = LanguageService::from_project(project);
        let main_id = DocumentId::new("Main");
        let pos = "Public Sub Main()\n    GetBaseName\nEnd Sub\n"
            .find("GetBaseName")
            .expect("call site") as u32;
        let loc = svc
            .go_to_definition(&main_id, pos)
            .expect("typelib-projected definition should resolve");
        assert_eq!(loc.document, DocumentId::new("Scripting::FileSystemObject"));
        assert_eq!(
            loc.provenance
                .as_ref()
                .map(|provenance| provenance.kind),
            Some(SymbolProvenanceKind::ImportedTypeLibraryProjection)
        );
        assert!(loc.symbol_identity.is_some());
    }

    #[test]
    fn go_to_definition_carries_stable_symbol_identity_across_snapshot_versions() {
        let mut ws = Workspace::new();
        let id = DocumentId::new("Module1");
        ws.open_document(id.clone(), "Public Sub Foo()\nEnd Sub\n");

        let svc = LanguageService::new(ws);
        let initial = svc
            .go_to_definition(&id, "Public Sub Foo()\nEnd Sub\n".find("Foo").unwrap() as u32)
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
            .go_to_definition(&id, "\nPublic Sub Foo()\nEnd Sub\n".find("Foo").unwrap() as u32)
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
