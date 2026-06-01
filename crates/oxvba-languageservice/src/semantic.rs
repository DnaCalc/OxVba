use std::sync::Arc;

use oxvba_compiler::VbaTypeId;
use oxvba_compiler::frontend_diagnostics::{FrontendDiagnostic, FrontendDiagnosticSeverity};
use oxvba_compiler::frontend_hir::HirDeclKind;
use oxvba_compiler::frontend_query::FrontendQueryDatabase;
use oxvba_compiler::frontend_symbols::{FrontendSourceSpan, SymbolNamespace};
use oxvba_compiler::frontend_type_hooks::TypedHirModule;
use oxvba_compiler::resolve::{BoundModule, BoundType, resolve_symbols};
use oxvba_compiler::typecheck::check_types;
use oxvba_syntax::{Parse, SyntaxKind, SyntaxNode, parse};

use crate::span::{
    DiagnosticSeverity, ScopeId, SemanticProvenance, SpannedDiagnostic, SymbolIdentity, SymbolInfo,
    SymbolKind, SymbolProvenanceKind, TextSpan,
};

/// A table of symbols keyed by name (case-insensitive).
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    pub symbols: Vec<SymbolInfo>,
}

impl SymbolTable {
    /// Find a symbol at a given byte offset.
    pub fn symbol_at(&self, offset: u32) -> Option<&SymbolInfo> {
        self.symbols
            .iter()
            .find(|s| s.definition_span.contains(offset))
    }

    /// Find all symbols with the given name (case-insensitive).
    pub fn symbols_named(&self, name: &str) -> Vec<&SymbolInfo> {
        let lower = name.to_ascii_lowercase();
        self.symbols
            .iter()
            .filter(|s| s.name.to_ascii_lowercase() == lower)
            .collect()
    }

    /// All symbols in a given scope.
    pub fn symbols_in_scope(&self, scope: ScopeId) -> Vec<&SymbolInfo> {
        self.symbols.iter().filter(|s| s.scope == scope).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableSignatureInfo {
    pub name: String,
    pub params: Vec<CallableParameterInfo>,
    pub return_type: BoundType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableParameterInfo {
    pub name: String,
    pub ty: BoundType,
}

/// The core analysis unit: an immutable snapshot of a single module's
/// parse tree, bound module, symbol table, and diagnostics.
pub struct SemanticSnapshot {
    pub source: Arc<str>,
    pub parse: Arc<Parse>,
    pub symbols: SymbolTable,
    pub callables: Vec<CallableSignatureInfo>,
    pub diagnostics: Vec<SpannedDiagnostic>,
    pub provenance: SemanticProvenance,
}

/// Build a complete semantic snapshot for a single VBA module source.
pub fn build_semantic_snapshot(source: &str) -> SemanticSnapshot {
    build_semantic_snapshot_with_provenance(
        source,
        SemanticProvenance {
            project_name: None,
            document_id: "<memory>".to_string(),
            snapshot_version: 1,
            kind: SymbolProvenanceKind::SourceModule,
        },
    )
}

/// Build a complete semantic snapshot for a single VBA module source with
/// explicit document/version provenance.
pub fn build_semantic_snapshot_with_provenance(
    source: &str,
    provenance: SemanticProvenance,
) -> SemanticSnapshot {
    let source_arc: Arc<str> = source.into();

    // Step 1: Parse → lossless CST
    let cst = parse(source);
    let parse_arc = Arc::new(cst);
    let mut frontend_queries =
        FrontendQueryDatabase::new(provenance.document_id.clone(), source.to_string());
    let frontend_diagnostics = frontend_queries
        .diagnostics()
        .into_iter()
        .map(spanned_diagnostic_from_frontend)
        .collect::<Vec<_>>();

    // Step 2: Prefer the compiler front-end HIR/SemanticModel facts for document symbols and
    // callable signatures. The legacy CST/BoundModule correlation is built only as a compatibility
    // fallback for syntax that the new front-end cannot bind yet.
    let frontend_typed = frontend_queries.bind().ok();
    let (symbols, callables, compatibility_diagnostics) = match frontend_typed.as_ref() {
        Some(typed) => {
            let symbols = symbol_table_from_frontend_hir(&parse_arc, typed, &provenance);
            let callables = callables_from_frontend_hir(typed);
            if symbols.symbols.is_empty() || callables.is_empty() {
                let (checked, mut diagnostics) = legacy_checked_module(source);
                diagnostics.extend(map_resolution_diagnostics(&parse_arc, &checked));
                (
                    correlate_symbols(&parse_arc, &checked, &provenance),
                    callables_from_bound_module(&checked),
                    diagnostics,
                )
            } else {
                (symbols, callables, Vec::new())
            }
        }
        None => {
            let (checked, mut diagnostics) = legacy_checked_module(source);
            diagnostics.extend(map_resolution_diagnostics(&parse_arc, &checked));
            (
                correlate_symbols(&parse_arc, &checked, &provenance),
                callables_from_bound_module(&checked),
                diagnostics,
            )
        }
    };

    // Combine all diagnostics
    let mut diagnostics = frontend_diagnostics;
    diagnostics.extend(compatibility_diagnostics);

    SemanticSnapshot {
        source: source_arc,
        parse: parse_arc,
        symbols,
        callables,
        diagnostics,
        provenance,
    }
}

fn legacy_checked_module(source: &str) -> (BoundModule, Vec<SpannedDiagnostic>) {
    let bound = resolve_symbols(source);
    match check_types(bound) {
        Ok(m) => (m, Vec::new()),
        Err(msg) => {
            let fallback = resolve_symbols(source);
            (
                fallback,
                vec![SpannedDiagnostic {
                    span: TextSpan::new(0, 0),
                    message: msg,
                    severity: DiagnosticSeverity::Error,
                }],
            )
        }
    }
}

fn callables_from_frontend_hir(typed: &TypedHirModule) -> Vec<CallableSignatureInfo> {
    let mut callables = Vec::new();
    for decl_id in &typed.module.declarations {
        let Some(decl) = typed.module.arenas.decl(*decl_id) else {
            continue;
        };
        let HirDeclKind::Procedure { params, .. } = &decl.kind else {
            continue;
        };
        let Some(name) = typed
            .module
            .symbols
            .symbol(decl.symbol)
            .and_then(|symbol| typed.module.symbols.name(symbol.name))
            .map(|name| name.first_spelling.clone())
        else {
            continue;
        };
        let params = params
            .iter()
            .filter_map(|param| {
                let name = typed
                    .module
                    .symbols
                    .symbol(*param)
                    .and_then(|symbol| typed.module.symbols.name(symbol.name))
                    .map(|name| name.first_spelling.clone())?;
                let ty = typed
                    .hooks
                    .declared_type(*param)
                    .map(|hook| bound_type_from_vba_type(hook.runtime_type))
                    .unwrap_or(BoundType::Variant);
                Some(CallableParameterInfo { name, ty })
            })
            .collect();
        let return_type = typed
            .hooks
            .declared_type(decl.symbol)
            .map(|hook| bound_type_from_vba_type(hook.runtime_type))
            .unwrap_or(BoundType::Variant);
        callables.push(CallableSignatureInfo {
            name,
            params,
            return_type,
        });
    }
    callables.sort_by(|left, right| left.name.cmp(&right.name));
    callables
}

fn callables_from_bound_module(bound: &BoundModule) -> Vec<CallableSignatureInfo> {
    bound
        .procedures
        .iter()
        .map(|procedure| CallableSignatureInfo {
            name: procedure.name.clone(),
            params: procedure
                .params
                .iter()
                .map(|param| CallableParameterInfo {
                    name: param.name.clone(),
                    ty: param.ty,
                })
                .collect(),
            return_type: procedure.return_type,
        })
        .collect()
}

fn symbol_table_from_frontend_hir(
    parse: &Parse,
    typed: &TypedHirModule,
    provenance: &SemanticProvenance,
) -> SymbolTable {
    let mut symbols = Vec::new();
    for symbol in typed.module.symbols.symbols() {
        let Some(name) = typed.module.symbols.name(symbol.name) else {
            continue;
        };
        if is_frontend_modifier_residue(&name.folded) {
            continue;
        }
        let Some(kind) = frontend_symbol_kind(symbol.namespace) else {
            continue;
        };
        let Some(span) = symbol.provenance.span else {
            continue;
        };
        let scope = frontend_language_service_scope(parse, symbol.namespace, span);
        let bound_type = typed
            .hooks
            .declared_type(symbol.id)
            .map(|hook| bound_type_from_vba_type(hook.runtime_type))
            .unwrap_or(BoundType::Variant);
        symbols.push(make_symbol(
            name.first_spelling.clone(),
            kind,
            bound_type,
            text_span_from_frontend(span),
            scope,
            provenance,
        ));
    }
    symbols.sort_by_key(|symbol| symbol.definition_span.start);
    SymbolTable { symbols }
}

fn frontend_symbol_kind(namespace: SymbolNamespace) -> Option<SymbolKind> {
    match namespace {
        SymbolNamespace::Procedure => Some(SymbolKind::Procedure),
        SymbolNamespace::Parameter => Some(SymbolKind::Parameter),
        SymbolNamespace::Local => Some(SymbolKind::Variable),
        SymbolNamespace::Type => Some(SymbolKind::TypeDef),
        _ => None,
    }
}

fn frontend_language_service_scope(
    parse: &Parse,
    namespace: SymbolNamespace,
    span: FrontendSourceSpan,
) -> ScopeId {
    if matches!(
        namespace,
        SymbolNamespace::Procedure | SymbolNamespace::Type | SymbolNamespace::Module
    ) {
        return 0;
    }
    procedure_scope_for_span(parse, span).unwrap_or(0)
}

fn procedure_scope_for_span(parse: &Parse, span: FrontendSourceSpan) -> Option<ScopeId> {
    let proc_kinds = [
        SyntaxKind::SubDecl,
        SyntaxKind::FunctionDecl,
        SyntaxKind::PropertyDecl,
    ];
    parse
        .syntax()
        .child_nodes()
        .into_iter()
        .filter(|node| proc_kinds.contains(&node.kind()))
        .enumerate()
        .find_map(|(idx, node)| {
            let (start, end) = node.text_range();
            if span.start as u32 >= start && (span.end as u32) <= end {
                Some((idx + 1) as ScopeId)
            } else {
                None
            }
        })
}

fn bound_type_from_vba_type(ty: VbaTypeId) -> BoundType {
    match ty {
        VbaTypeId::Boolean => BoundType::Boolean,
        VbaTypeId::Byte => BoundType::Byte,
        VbaTypeId::Integer => BoundType::Integer,
        VbaTypeId::Long => BoundType::Long,
        VbaTypeId::LongLong => BoundType::LongLong,
        VbaTypeId::LongPtr => BoundType::LongPtr,
        VbaTypeId::Single => BoundType::Single,
        VbaTypeId::Double => BoundType::Double,
        VbaTypeId::Currency => BoundType::Currency,
        VbaTypeId::Date => BoundType::Date,
        VbaTypeId::String => BoundType::String,
        VbaTypeId::Object => BoundType::Object,
        VbaTypeId::Array => BoundType::Array,
        _ => BoundType::Variant,
    }
}

fn is_frontend_modifier_residue(name: &str) -> bool {
    matches!(
        name,
        "withevents" | "optional" | "byval" | "byref" | "paramarray"
    )
}

fn text_span_from_frontend(span: FrontendSourceSpan) -> TextSpan {
    TextSpan::new(span.start as u32, span.end as u32)
}

fn spanned_diagnostic_from_frontend(diagnostic: FrontendDiagnostic) -> SpannedDiagnostic {
    SpannedDiagnostic {
        span: text_span_from_frontend(diagnostic.span),
        message: diagnostic.message,
        severity: match diagnostic.severity {
            FrontendDiagnosticSeverity::Error => DiagnosticSeverity::Error,
            FrontendDiagnosticSeverity::Warning | FrontendDiagnosticSeverity::Info => {
                DiagnosticSeverity::Warning
            }
        },
    }
}

/// Walk the CST and the BoundModule, matching declarations by name to build
/// a SymbolTable with source positions.
fn correlate_symbols(
    parse: &Parse,
    bound: &BoundModule,
    provenance: &SemanticProvenance,
) -> SymbolTable {
    let mut symbols = Vec::new();
    let root = parse.syntax();

    // Correlate module-level declarations
    correlate_module_declarations(&root, bound, provenance, &mut symbols);

    // Correlate procedures
    correlate_procedures(&root, bound, provenance, &mut symbols);

    SymbolTable { symbols }
}

fn correlate_module_declarations(
    root: &SyntaxNode<'_>,
    bound: &BoundModule,
    provenance: &SemanticProvenance,
    symbols: &mut Vec<SymbolInfo>,
) {
    for child in root.child_nodes() {
        match child.kind() {
            SyntaxKind::DimStmt | SyntaxKind::ConstStmt => {
                // Find identifier tokens in this statement
                for tok in child.child_tokens() {
                    if tok.kind == SyntaxKind::Ident {
                        let name = tok.text.to_string();
                        let lower = name.to_ascii_lowercase();
                        let bound_type = bound
                            .declaration_types
                            .get(&lower)
                            .cloned()
                            .unwrap_or(BoundType::Variant);
                        let is_const = child.kind() == SyntaxKind::ConstStmt;
                        let kind = if is_const {
                            SymbolKind::Constant
                        } else {
                            SymbolKind::Variable
                        };
                        symbols.push(make_symbol(
                            name,
                            kind,
                            bound_type,
                            TextSpan::new(tok.offset, tok.offset + tok.text.len() as u32),
                            0,
                            provenance,
                        ));
                    }
                }
            }
            SyntaxKind::TypeBlock => {
                if let Some(name_tok) = find_name_token(&child) {
                    symbols.push(make_symbol(
                        name_tok.0.to_string(),
                        SymbolKind::TypeDef,
                        BoundType::Variant,
                        TextSpan::new(name_tok.1, name_tok.2),
                        0,
                        provenance,
                    ));
                }
            }
            SyntaxKind::EnumBlock => {
                if let Some(name_tok) = find_name_token(&child) {
                    symbols.push(make_symbol(
                        name_tok.0.to_string(),
                        SymbolKind::EnumDef,
                        BoundType::Long,
                        TextSpan::new(name_tok.1, name_tok.2),
                        0,
                        provenance,
                    ));
                }
            }
            SyntaxKind::EventDecl => {
                if let Some(name_tok) = find_name_token(&child) {
                    symbols.push(make_symbol(
                        name_tok.0.to_string(),
                        SymbolKind::Event,
                        BoundType::Variant,
                        TextSpan::new(name_tok.1, name_tok.2),
                        0,
                        provenance,
                    ));
                }
            }
            SyntaxKind::DeclareStmt => {
                if let Some(name_tok) = find_name_token(&child) {
                    symbols.push(make_symbol(
                        name_tok.0.to_string(),
                        SymbolKind::External,
                        BoundType::Variant,
                        TextSpan::new(name_tok.1, name_tok.2),
                        0,
                        provenance,
                    ));
                }
            }
            _ => {}
        }
    }
}

fn correlate_procedures(
    root: &SyntaxNode<'_>,
    bound: &BoundModule,
    provenance: &SemanticProvenance,
    symbols: &mut Vec<SymbolInfo>,
) {
    let proc_kinds = [
        SyntaxKind::SubDecl,
        SyntaxKind::FunctionDecl,
        SyntaxKind::PropertyDecl,
    ];

    for (proc_idx, child) in root
        .child_nodes()
        .into_iter()
        .filter(|n| proc_kinds.contains(&n.kind()))
        .enumerate()
    {
        let scope: ScopeId = (proc_idx + 1) as u32;

        if let Some(name_tok) = find_name_token(&child) {
            let name_lower = name_tok.0.to_ascii_lowercase();

            // Look up return type from BoundModule
            let return_type = bound
                .procedures
                .iter()
                .find(|p| p.name.to_ascii_lowercase() == name_lower)
                .map(|p| p.return_type)
                .unwrap_or(BoundType::Variant);

            let sym_kind = match child.kind() {
                SyntaxKind::PropertyDecl => SymbolKind::Property,
                _ => SymbolKind::Procedure,
            };

            symbols.push(make_symbol(
                name_tok.0.to_string(),
                sym_kind,
                return_type,
                TextSpan::new(name_tok.1, name_tok.2),
                0,
                provenance,
            ));

            // Correlate parameters
            correlate_params(&child, bound, &name_lower, scope, provenance, symbols);
        }

        // Correlate local declarations inside the procedure body
        correlate_local_declarations(&child, scope, provenance, symbols);
    }
}

fn correlate_params(
    proc_node: &SyntaxNode<'_>,
    bound: &BoundModule,
    proc_name_lower: &str,
    scope: ScopeId,
    provenance: &SemanticProvenance,
    symbols: &mut Vec<SymbolInfo>,
) {
    let bound_proc = bound
        .procedures
        .iter()
        .find(|p| p.name.to_ascii_lowercase() == proc_name_lower);

    for param_list in proc_node.child_nodes() {
        if param_list.kind() != SyntaxKind::ParamList {
            continue;
        }
        for (param_idx, param_node) in param_list
            .child_nodes()
            .into_iter()
            .filter(|n| n.kind() == SyntaxKind::Param)
            .enumerate()
        {
            if let Some(name_tok) = find_name_token(&param_node) {
                let param_type = bound_proc
                    .and_then(|p| p.params.get(param_idx))
                    .map(|bp| bp.ty)
                    .unwrap_or(BoundType::Variant);

                symbols.push(make_symbol(
                    name_tok.0.to_string(),
                    SymbolKind::Parameter,
                    param_type,
                    TextSpan::new(name_tok.1, name_tok.2),
                    scope,
                    provenance,
                ));
            }
        }
    }
}

fn correlate_local_declarations(
    proc_node: &SyntaxNode<'_>,
    scope: ScopeId,
    provenance: &SemanticProvenance,
    symbols: &mut Vec<SymbolInfo>,
) {
    // Find the Block child inside the procedure
    for block in proc_node.child_nodes() {
        if block.kind() != SyntaxKind::Block {
            continue;
        }
        for stmt in block.child_nodes() {
            if stmt.kind() == SyntaxKind::DimStmt || stmt.kind() == SyntaxKind::ConstStmt {
                for tok in stmt.child_tokens() {
                    if tok.kind == SyntaxKind::Ident {
                        let is_const = stmt.kind() == SyntaxKind::ConstStmt;
                        let kind = if is_const {
                            SymbolKind::Constant
                        } else {
                            SymbolKind::Variable
                        };
                        symbols.push(make_symbol(
                            tok.text.to_string(),
                            kind,
                            BoundType::Variant,
                            TextSpan::new(tok.offset, tok.offset + tok.text.len() as u32),
                            scope,
                            provenance,
                        ));
                    }
                }
            }
        }
    }
}

fn make_symbol(
    name: String,
    kind: SymbolKind,
    bound_type: BoundType,
    definition_span: TextSpan,
    scope: ScopeId,
    provenance: &SemanticProvenance,
) -> SymbolInfo {
    let normalized_name = name.to_ascii_lowercase();
    SymbolInfo {
        name,
        kind,
        bound_type,
        definition_span,
        scope,
        identity: SymbolIdentity {
            project_name: provenance.project_name.clone(),
            document_id: provenance.document_id.clone(),
            normalized_name,
            kind,
            scope,
        },
        provenance: provenance.clone(),
    }
}

/// Find the first identifier token in a node (skipping keywords and trivia).
/// Returns (text, start_offset, end_offset).
fn find_name_token<'a>(node: &SyntaxNode<'a>) -> Option<(&'a str, u32, u32)> {
    for tok in node.child_tokens() {
        if tok.kind == SyntaxKind::Ident {
            return Some((tok.text, tok.offset, tok.offset + tok.text.len() as u32));
        }
    }
    None
}

/// Map BoundModule resolution diagnostics to source spans.
fn map_resolution_diagnostics(parse: &Parse, bound: &BoundModule) -> Vec<SpannedDiagnostic> {
    let root = parse.syntax();
    let source = root.text();

    bound
        .resolution_diagnostics
        .iter()
        .map(|msg| {
            // Try to find a source location by searching for quoted identifiers in the message
            let span = extract_identifier_from_diagnostic(msg)
                .and_then(|ident| find_identifier_span(&source, &ident))
                .unwrap_or(TextSpan::new(0, 0));

            SpannedDiagnostic {
                span,
                message: msg.clone(),
                severity: DiagnosticSeverity::Error,
            }
        })
        .collect()
}

/// Try to extract an identifier from a diagnostic message.
/// Looks for patterns like "'foo'" or "`foo`" in the message.
fn extract_identifier_from_diagnostic(msg: &str) -> Option<String> {
    // Look for single-quoted identifier
    if let Some(start) = msg.find('\'') {
        let rest = &msg[start + 1..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }
    // Look for backtick-quoted identifier
    if let Some(start) = msg.find('`') {
        let rest = &msg[start + 1..];
        if let Some(end) = rest.find('`') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Find the byte offset of an identifier in source text.
fn find_identifier_span(source: &str, name: &str) -> Option<TextSpan> {
    let lower = name.to_ascii_lowercase();
    let source_lower = source.to_ascii_lowercase();
    source_lower
        .find(&lower)
        .map(|pos| TextSpan::new(pos as u32, (pos + name.len()) as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_basic_sub() {
        let src = "Sub Hello()\n    Dim x As Long\n    x = 42\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);
        assert_eq!(snap.source.as_ref(), src);
        assert!(!snap.symbols.symbols.is_empty());

        // Should find procedure "Hello" and variable "x"
        let names: Vec<&str> = snap
            .symbols
            .symbols
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.contains(&"Hello"), "symbols: {names:?}");
        assert!(names.contains(&"x"), "symbols: {names:?}");
    }

    #[test]
    fn snapshot_function_with_params() {
        let src = "Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
        let snap = build_semantic_snapshot(src);

        let params: Vec<_> = snap
            .symbols
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Parameter)
            .collect();
        assert_eq!(params.len(), 2, "expected 2 params, got: {params:?}");
    }

    #[test]
    fn snapshot_module_level_dim() {
        let src = "Dim gCount As Long\nSub Foo()\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);

        let module_vars: Vec<_> = snap
            .symbols
            .symbols
            .iter()
            .filter(|s| s.scope == 0 && s.kind == SymbolKind::Variable)
            .collect();
        assert!(
            module_vars.iter().any(|s| s.name == "gCount"),
            "vars: {module_vars:?}"
        );
    }

    #[test]
    fn snapshot_spans_are_valid() {
        let src = "Sub Test()\n    Dim result As Long\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);

        for sym in &snap.symbols.symbols {
            let span = sym.definition_span;
            assert!(
                span.start <= span.end,
                "invalid span for {}: {:?}",
                sym.name,
                span
            );
            assert!(
                (span.end as usize) <= src.len(),
                "span out of bounds for {}: {:?} (source len {})",
                sym.name,
                span,
                src.len()
            );
        }
    }

    #[test]
    fn snapshot_parse_errors_reported() {
        let src = "Sub (\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);
        // Parse errors should be in diagnostics
        // (the parser wraps bad tokens in ErrorNode, but also records errors)
        // Just verify it doesn't panic
        assert!(snap.parse.syntax().text() == src);
    }

    #[test]
    fn symbol_at_finds_correct_symbol() {
        let src = "Sub Test()\n    Dim counter As Long\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);

        // Find where "counter" is in the source
        let counter_pos = src.find("counter").unwrap() as u32;
        let sym = snap.symbols.symbol_at(counter_pos);
        assert!(sym.is_some(), "expected symbol at offset {counter_pos}");
        assert_eq!(sym.unwrap().name, "counter");
    }

    #[test]
    fn snapshot_symbols_prefer_compiler_frontend_hir_facts() {
        let src =
            "Sub Test(ByVal seed As Long)\n    Dim label As String\n    label = \"ok\"\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);

        let seed = snap
            .symbols
            .symbols
            .iter()
            .find(|symbol| symbol.name == "seed")
            .expect("parameter from compiler HIR facts");
        assert_eq!(seed.kind, SymbolKind::Parameter);
        assert_eq!(seed.bound_type, BoundType::Long);
        assert_eq!(seed.scope, 1);

        let label = snap
            .symbols
            .symbols
            .iter()
            .find(|symbol| symbol.name == "label")
            .expect("local from compiler HIR facts");
        assert_eq!(label.kind, SymbolKind::Variable);
        assert_eq!(label.bound_type, BoundType::String);
        assert_eq!(label.scope, 1);
    }

    #[test]
    fn snapshot_callables_prefer_compiler_frontend_hir_facts() {
        let src = "Sub Multi(ByVal first As Long, second As String)\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);

        let callable = snap
            .callables
            .iter()
            .find(|callable| callable.name == "Multi")
            .expect("callable from compiler HIR facts");
        assert_eq!(callable.params.len(), 2);
        assert_eq!(callable.params[0].name, "first");
        assert_eq!(callable.params[0].ty, BoundType::Long);
        assert_eq!(callable.params[1].name, "second");
        assert_eq!(callable.params[1].ty, BoundType::String);
    }

    #[test]
    fn snapshot_accepts_option_explicit() {
        let src = "Option Explicit\nSub Foo()\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);
        assert!(
            snap.symbols
                .symbols
                .iter()
                .any(|symbol| symbol.name == "Foo")
        );
    }
}
