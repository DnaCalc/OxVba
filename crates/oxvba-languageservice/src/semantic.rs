use std::sync::Arc;

use oxvba_compiler::frontend_diagnostics::{FrontendDiagnostic, FrontendDiagnosticSeverity};
use oxvba_compiler::frontend_hir::HirDeclKind;
use oxvba_compiler::frontend_query::FrontendQueryDatabase;
use oxvba_compiler::frontend_symbols::{FrontendSourceSpan, SymbolNamespace};
use oxvba_compiler::frontend_type_hooks::TypedHirModule;
use oxvba_compiler::resolve::BoundType;
use oxvba_compiler::{OptionalDefaultValue, VbaTypeId};
use oxvba_syntax::{Parse, SyntaxKind, parse};

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
    pub optional: bool,
    pub param_array: bool,
    pub default_value: Option<OptionalDefaultValue>,
}

/// The core analysis unit: an immutable snapshot of a single module's
/// parse tree, front-end symbol table, callable signatures, and diagnostics.
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

    // Step 2: Use compiler front-end HIR/SemanticModel facts for document symbols and callable
    // signatures. Unsupported front-end syntax reports diagnostics rather than rebuilding the
    // retired legacy BoundModule semantic surface.
    let frontend_typed = frontend_queries.bind().ok();
    let (symbols, callables) = match frontend_typed.as_ref() {
        Some(typed) => (
            symbol_table_from_frontend_hir(&parse_arc, typed, &provenance),
            callables_from_frontend_hir(typed),
        ),
        None => (SymbolTable::default(), Vec::new()),
    };

    SemanticSnapshot {
        source: source_arc,
        parse: parse_arc,
        symbols,
        callables,
        diagnostics: frontend_diagnostics,
        provenance,
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
                let parameter_hook = typed.hooks.parameter(*param);
                Some(CallableParameterInfo {
                    name,
                    ty,
                    optional: parameter_hook.is_some_and(|hook| hook.optional),
                    param_array: parameter_hook.is_some_and(|hook| hook.param_array),
                    default_value: parameter_hook.and_then(|hook| hook.default_value.clone()),
                })
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
    fn snapshot_callables_preserve_optional_string_boolean_defaults() {
        let src = "Sub Use(Optional ByVal text As String = \"ready\", Optional ByVal flag As Boolean = True)\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);

        let callable = snap
            .callables
            .iter()
            .find(|callable| callable.name == "Use")
            .expect("callable from compiler HIR facts");
        assert_eq!(callable.params.len(), 2);
        assert_eq!(callable.params[0].name, "text");
        assert_eq!(callable.params[0].ty, BoundType::String);
        assert!(callable.params[0].optional);
        assert_eq!(
            callable.params[0].default_value,
            Some(OptionalDefaultValue::ExplicitString("ready".to_string()))
        );
        assert_eq!(callable.params[1].name, "flag");
        assert_eq!(callable.params[1].ty, BoundType::Boolean);
        assert!(callable.params[1].optional);
        assert_eq!(
            callable.params[1].default_value,
            Some(OptionalDefaultValue::ExplicitBool(true))
        );
    }

    #[test]
    fn snapshot_callables_preserve_param_array_flag() {
        let src = "Sub Collect(ParamArray values() As Variant)\nEnd Sub\n";
        let snap = build_semantic_snapshot(src);

        let callable = snap
            .callables
            .iter()
            .find(|callable| callable.name == "Collect")
            .expect("callable from compiler HIR facts");
        assert_eq!(callable.params.len(), 1);
        assert_eq!(callable.params[0].name, "values");
        assert!(callable.params[0].param_array);
        assert!(!callable.params[0].optional);
        assert_eq!(callable.params[0].default_value, None);
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

    #[test]
    fn snapshot_covers_matrix_route_overlay_shapes_from_frontend_hir() {
        let cases = [
            (
                "static declaration",
                "Sub Main()\nStatic counter As Long\ncounter = counter + 1\nEnd Sub\n",
                "counter",
                BoundType::Long,
            ),
            (
                "exponent expression",
                "Sub Main()\nDim x\nx = 2 ^ 3\nEnd Sub\n",
                "x",
                BoundType::Variant,
            ),
            (
                "qualified identifier",
                "Sub Main()\nDim obj\nDim x\nx = obj.Child.Value\nEnd Sub\n",
                "obj",
                BoundType::Variant,
            ),
            (
                "trivia and comments",
                "' leading comment\n\nSub Main()\nDim x\nx = 1 ' trailing comment\nEnd Sub\n",
                "x",
                BoundType::Variant,
            ),
        ];

        for (label, source, symbol_name, expected_type) in cases {
            let snap = build_semantic_snapshot(source);
            assert!(
                snap.diagnostics.is_empty(),
                "{label} should build without language-service diagnostics: {:?}",
                snap.diagnostics
            );
            let symbol = snap
                .symbols
                .symbols
                .iter()
                .find(|symbol| symbol.name == symbol_name)
                .unwrap_or_else(|| {
                    panic!(
                        "{label} should expose `{symbol_name}` from frontend HIR facts: {:?}",
                        snap.symbols.symbols
                    )
                });
            assert_eq!(symbol.kind, SymbolKind::Variable, "{label}");
            assert_eq!(symbol.bound_type, expected_type, "{label}");
        }
    }

    #[test]
    fn snapshot_seed_corpus_uses_frontend_facts_for_source_backed_rows() {
        let fixtures = oxvba_compiler::frontend_diff::frontend_rework_seed_corpus();
        let mut checked = Vec::new();
        for fixture in fixtures {
            if !matches!(
                fixture.class,
                oxvba_compiler::frontend_diff::FrontendCorpusClass::CompilerUnit
                    | oxvba_compiler::frontend_diff::FrontendCorpusClass::ConformanceCase
                    | oxvba_compiler::frontend_diff::FrontendCorpusClass::HostProject
            ) {
                continue;
            }
            let Some(source) = fixture.source.as_deref() else {
                continue;
            };
            let snap = build_semantic_snapshot(source);
            assert!(
                snap.diagnostics.is_empty(),
                "{} should build a frontend-backed semantic snapshot without diagnostics: {:?}",
                fixture.name,
                snap.diagnostics
            );
            assert!(
                !snap.symbols.symbols.is_empty() || !snap.callables.is_empty(),
                "{} should expose frontend symbols or callables",
                fixture.name
            );
            checked.push(fixture.name);
        }
        assert_eq!(
            checked,
            vec![
                "examples_basic_arithmetic".to_string(),
                "conformance_call_coercion_mixed_variant_to_long".to_string(),
                "inline_statement_separator_bridge_improvement".to_string(),
                "integration_host_project_residual".to_string(),
            ]
        );
    }
}
