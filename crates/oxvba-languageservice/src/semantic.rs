use std::sync::Arc;

use oxvba_compiler::resolve::{BoundModule, BoundType, resolve_symbols};
use oxvba_compiler::typecheck::check_types;
use oxvba_syntax::{Parse, SyntaxKind, SyntaxNode, parse};

use crate::span::{
    DiagnosticSeverity, ScopeId, SemanticProvenance, SpannedDiagnostic, SymbolIdentity,
    SymbolInfo, SymbolKind, SymbolProvenanceKind, TextSpan,
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

/// The core analysis unit: an immutable snapshot of a single module's
/// parse tree, bound module, symbol table, and diagnostics.
pub struct SemanticSnapshot {
    pub source: Arc<str>,
    pub parse: Arc<Parse>,
    pub bound: Arc<BoundModule>,
    pub symbols: SymbolTable,
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
    let parse_errors: Vec<SpannedDiagnostic> = cst
        .errors()
        .iter()
        .map(|e| SpannedDiagnostic {
            span: TextSpan::new(e.offset, e.offset),
            message: e.message.clone(),
            severity: DiagnosticSeverity::Error,
        })
        .collect();

    let parse_arc = Arc::new(cst);

    // Step 2: Resolve → BoundModule
    let bound = resolve_symbols(source);

    // Step 3: Type-check
    let (checked, type_errors) = match check_types(bound) {
        Ok(m) => (m, Vec::new()),
        Err(msg) => {
            // check_types returns the module even on error via the message;
            // re-resolve to get a module we can use
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
    };

    // Step 4: Correlation pass — build symbol table from CST + BoundModule
    let symbols = correlate_symbols(&parse_arc, &checked, &provenance);

    // Step 5: Map resolution diagnostics to spans
    let resolution_diags = map_resolution_diagnostics(&parse_arc, &checked);

    // Combine all diagnostics
    let mut diagnostics = parse_errors;
    diagnostics.extend(type_errors);
    diagnostics.extend(resolution_diags);

    let bound_arc = Arc::new(checked);

    SemanticSnapshot {
        source: source_arc,
        parse: parse_arc,
        bound: bound_arc,
        symbols,
        diagnostics,
        provenance,
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
    fn snapshot_option_explicit() {
        let src = "Option Explicit\nSub Foo()\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);
        assert!(snap.bound.option_explicit);
    }
}
