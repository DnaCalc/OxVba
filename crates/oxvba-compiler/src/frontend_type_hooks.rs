use std::collections::BTreeMap;

use crate::frontend_hir::{
    BoundHirModule, CstBackpointer, HirArenas, HirBuildError, HirBuiltinType, HirCallId,
    HirDeclKind, HirExprId, HirExprKind, HirLiteral, HirStmtId, HirStmtKind, HirType, HirTypeId,
    HirTypeKind, build_hir_from_source,
};
use crate::frontend_symbols::{FrontendSourceSpan, SymbolId, SymbolNamespace};
use crate::{CoercionKindDescriptor, OptionalDefaultValue, ParameterPassingMode, VbaTypeId};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirAssignmentIntent {
    Let,
    Set,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirDeclaredTypeHook {
    pub symbol: SymbolId,
    pub hir_type: HirTypeId,
    pub runtime_type: VbaTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirCoercionHook {
    pub expr: HirExprId,
    pub source_type: VbaTypeId,
    pub target_type: VbaTypeId,
    pub kind: CoercionKindDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirCallSiteHook {
    pub call: HirCallId,
    pub target: SymbolId,
    pub args: Vec<HirArgumentHook>,
    pub return_type: Option<VbaTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirArgumentHook {
    pub expr: Option<HirExprId>,
    pub parameter: HirParameterHook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirParameterHook {
    pub symbol: SymbolId,
    pub declared_type: VbaTypeId,
    pub passing_mode: ParameterPassingMode,
    pub optional: bool,
    pub param_array: bool,
    pub default_value: Option<OptionalDefaultValue>,
}

#[derive(Debug, Clone, Default)]
pub struct HirTypeHooks {
    declared_types_by_symbol: BTreeMap<SymbolId, HirDeclaredTypeHook>,
    assignment_intents: BTreeMap<HirStmtId, HirAssignmentIntent>,
    call_sites: BTreeMap<HirCallId, HirCallSiteHook>,
    coercions_by_expr: BTreeMap<HirExprId, Vec<HirCoercionHook>>,
}

#[derive(Debug, Clone)]
pub struct TypedHirModule {
    pub module: BoundHirModule,
    pub hooks: HirTypeHooks,
}

impl HirTypeHooks {
    pub fn record_declared_type(&mut self, hook: HirDeclaredTypeHook) {
        self.declared_types_by_symbol.insert(hook.symbol, hook);
    }

    pub fn declared_type(&self, symbol: SymbolId) -> Option<&HirDeclaredTypeHook> {
        self.declared_types_by_symbol.get(&symbol)
    }

    pub fn declared_types(&self) -> impl Iterator<Item = &HirDeclaredTypeHook> {
        self.declared_types_by_symbol.values()
    }

    pub fn record_assignment_intent(&mut self, stmt: HirStmtId, intent: HirAssignmentIntent) {
        self.assignment_intents.insert(stmt, intent);
    }

    pub fn assignment_intent(&self, stmt: HirStmtId) -> Option<HirAssignmentIntent> {
        self.assignment_intents.get(&stmt).copied()
    }

    pub fn assignment_intents(
        &self,
    ) -> impl Iterator<Item = (HirStmtId, HirAssignmentIntent)> + '_ {
        self.assignment_intents
            .iter()
            .map(|(stmt, intent)| (*stmt, *intent))
    }

    pub fn record_call_site(&mut self, hook: HirCallSiteHook) {
        self.call_sites.insert(hook.call, hook);
    }

    pub fn call_site(&self, call: HirCallId) -> Option<&HirCallSiteHook> {
        self.call_sites.get(&call)
    }

    pub fn call_sites(&self) -> impl Iterator<Item = &HirCallSiteHook> {
        self.call_sites.values()
    }

    pub fn record_coercion(&mut self, hook: HirCoercionHook) {
        self.coercions_by_expr
            .entry(hook.expr)
            .or_default()
            .push(hook);
    }

    pub fn coercions_for_expr(&self, expr: HirExprId) -> &[HirCoercionHook] {
        self.coercions_by_expr
            .get(&expr)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn coercions(&self) -> impl Iterator<Item = &HirCoercionHook> {
        self.coercions_by_expr.values().flatten()
    }
}

pub fn collect_type_hooks_from_source(
    module_name: &str,
    source: &str,
) -> Result<TypedHirModule, HirBuildError> {
    let mut module = build_hir_from_source(module_name, source)?;
    let mut hooks = HirTypeHooks::default();
    let mut symbol_types = BTreeMap::<SymbolId, VbaTypeId>::new();

    for symbol in module.symbols.symbols().to_vec() {
        if !matches!(
            symbol.namespace,
            SymbolNamespace::Local | SymbolNamespace::Parameter
        ) {
            continue;
        }
        let Some(span) = symbol.provenance.span else {
            continue;
        };
        let Some((runtime_type, type_span)) = declared_type_after_span(source, span) else {
            continue;
        };
        let hir_type = module.arenas.alloc_type(HirType {
            cst: CstBackpointer {
                syntax_kind: "TypeRef".to_string(),
                span: type_span,
            },
            kind: HirTypeKind::Builtin(hir_builtin_type(runtime_type)),
        });
        hooks.record_declared_type(HirDeclaredTypeHook {
            symbol: symbol.id,
            hir_type,
            runtime_type,
        });
        symbol_types.insert(symbol.id, runtime_type);
    }

    let parsed = oxvba_syntax::parse(source);
    for node in parsed.syntax().child_nodes() {
        collect_procedure_return_type_hooks(&mut module, &mut hooks, &mut symbol_types, node);
    }

    for decl in module.declarations.clone() {
        let Some(decl) = module.arenas.decl(decl).cloned() else {
            continue;
        };
        if let HirDeclKind::Procedure { body, .. } = decl.kind {
            for stmt in body {
                collect_stmt_type_hooks(&module.arenas, &symbol_types, &mut hooks, stmt);
            }
        }
    }

    Ok(TypedHirModule { module, hooks })
}

fn collect_procedure_return_type_hooks(
    module: &mut BoundHirModule,
    hooks: &mut HirTypeHooks,
    symbol_types: &mut BTreeMap<SymbolId, VbaTypeId>,
    node: SyntaxNode<'_>,
) {
    if node.kind() == SyntaxKind::FunctionDecl || node.kind() == SyntaxKind::PropertyDecl {
        record_procedure_return_type_hook(module, hooks, symbol_types, node);
    }

    for child in node.child_nodes() {
        collect_procedure_return_type_hooks(module, hooks, symbol_types, child);
    }
}

fn record_procedure_return_type_hook(
    module: &mut BoundHirModule,
    hooks: &mut HirTypeHooks,
    symbol_types: &mut BTreeMap<SymbolId, VbaTypeId>,
    node: SyntaxNode<'_>,
) {
    let (Some(name), Some(type_ref)) = (node.name_token(), node.return_type()) else {
        return;
    };
    let symbol_name = procedure_symbol_lookup_name(node, name.text);
    let Some(symbol) = module
        .symbols
        .lookup_name(&symbol_name)
        .and_then(|name_id| {
            module
                .symbols
                .symbols()
                .iter()
                .find(|symbol| {
                    symbol.name == name_id && symbol.namespace == SymbolNamespace::Procedure
                })
                .map(|symbol| symbol.id)
        })
    else {
        return;
    };
    let Some(runtime_type) = parse_runtime_type(type_ref.text().trim()) else {
        return;
    };

    let (start, end) = type_ref.text_range();
    let hir_type = module.arenas.alloc_type(HirType {
        cst: CstBackpointer {
            syntax_kind: "TypeRef".to_string(),
            span: FrontendSourceSpan {
                start: start as usize,
                end: end as usize,
            },
        },
        kind: HirTypeKind::Builtin(hir_builtin_type(runtime_type)),
    });
    hooks.record_declared_type(HirDeclaredTypeHook {
        symbol,
        hir_type,
        runtime_type,
    });
    symbol_types.insert(symbol, runtime_type);
    set_decl_return_type(module, symbol, hir_type);
}

fn set_decl_return_type(module: &mut BoundHirModule, symbol: SymbolId, hir_type: HirTypeId) {
    for decl_id in module.declarations.clone() {
        let Some(decl) = module.arenas.decl_mut(decl_id) else {
            continue;
        };
        if decl.symbol != symbol {
            continue;
        }
        if let HirDeclKind::Procedure { return_type, .. } = &mut decl.kind {
            *return_type = Some(hir_type);
        }
    }
}

fn procedure_symbol_lookup_name(node: SyntaxNode<'_>, name: &str) -> String {
    let name = normalize_identifier_token(name);
    if node.kind() != SyntaxKind::PropertyDecl {
        return name.to_string();
    }
    let lower = node.text().to_ascii_lowercase();
    let prefix = if lower.contains("property let") {
        "property_let"
    } else if lower.contains("property set") {
        "property_set"
    } else {
        "property_get"
    };
    format!("{prefix}_{name}")
}

fn collect_stmt_type_hooks(
    hir: &HirArenas,
    symbol_types: &BTreeMap<SymbolId, VbaTypeId>,
    hooks: &mut HirTypeHooks,
    stmt: HirStmtId,
) {
    let Some(stmt_data) = hir.stmt(stmt).cloned() else {
        return;
    };
    match stmt_data.kind {
        HirStmtKind::Let { target, value } => {
            hooks.record_assignment_intent(stmt, HirAssignmentIntent::Let);
            record_assignment_coercion(hir, symbol_types, hooks, value, target);
        }
        HirStmtKind::Set { .. } => hooks.record_assignment_intent(stmt, HirAssignmentIntent::Set),
        HirStmtKind::Block(stmts) => {
            for stmt in stmts {
                collect_stmt_type_hooks(hir, symbol_types, hooks, stmt);
            }
        }
        HirStmtKind::If {
            then_body,
            else_body,
            ..
        } => {
            for stmt in then_body.into_iter().chain(else_body) {
                collect_stmt_type_hooks(hir, symbol_types, hooks, stmt);
            }
        }
        HirStmtKind::DoWhile { body, .. } => {
            for stmt in body {
                collect_stmt_type_hooks(hir, symbol_types, hooks, stmt);
            }
        }
        HirStmtKind::SelectCase {
            arms, else_body, ..
        } => {
            for (_, body) in arms {
                for stmt in body {
                    collect_stmt_type_hooks(hir, symbol_types, hooks, stmt);
                }
            }
            for stmt in else_body {
                collect_stmt_type_hooks(hir, symbol_types, hooks, stmt);
            }
        }
        HirStmtKind::ForRange { body, .. } => {
            for stmt in body {
                collect_stmt_type_hooks(hir, symbol_types, hooks, stmt);
            }
        }
        HirStmtKind::ForEach { body, .. } => {
            for stmt in body {
                collect_stmt_type_hooks(hir, symbol_types, hooks, stmt);
            }
        }
        HirStmtKind::Empty
        | HirStmtKind::Expr(_)
        | HirStmtKind::ExitDo
        | HirStmtKind::ExitFor
        | HirStmtKind::ExitProcedure
        | HirStmtKind::OnErrorResumeNext
        | HirStmtKind::OnErrorGoto0
        | HirStmtKind::OnErrorGotoLabel { .. }
        | HirStmtKind::ResumeNext
        | HirStmtKind::Resume
        | HirStmtKind::ResumeLabel { .. }
        | HirStmtKind::Label { .. }
        | HirStmtKind::GoTo { .. }
        | HirStmtKind::GoSub { .. }
        | HirStmtKind::Return
        | HirStmtKind::Erase { .. } => {}
    }
}

fn record_assignment_coercion(
    hir: &HirArenas,
    symbol_types: &BTreeMap<SymbolId, VbaTypeId>,
    hooks: &mut HirTypeHooks,
    value: HirExprId,
    target: HirExprId,
) {
    let Some(target_symbol) = name_symbol(hir, target) else {
        return;
    };
    let Some(target_type) = symbol_types.get(&target_symbol).copied() else {
        return;
    };
    let Some(source_type) = infer_expr_type(hir, symbol_types, value) else {
        return;
    };
    if source_type != target_type {
        hooks.record_coercion(HirCoercionHook {
            expr: value,
            source_type,
            target_type,
            kind: CoercionKindDescriptor::Let,
        });
    }
}

fn name_symbol(hir: &HirArenas, expr: HirExprId) -> Option<SymbolId> {
    match hir.expr(expr).map(|expr| &expr.kind) {
        Some(HirExprKind::Name(symbol)) => Some(*symbol),
        _ => None,
    }
}

fn infer_expr_type(
    hir: &HirArenas,
    symbol_types: &BTreeMap<SymbolId, VbaTypeId>,
    expr: HirExprId,
) -> Option<VbaTypeId> {
    match hir.expr(expr).map(|expr| &expr.kind)? {
        HirExprKind::Name(symbol) => symbol_types.get(symbol).copied(),
        HirExprKind::Literal(HirLiteral::Bool(_)) => Some(VbaTypeId::Boolean),
        HirExprKind::Literal(HirLiteral::Int(_)) => Some(VbaTypeId::Long),
        HirExprKind::Literal(HirLiteral::String(_)) => Some(VbaTypeId::String),
        HirExprKind::Literal(HirLiteral::Empty | HirLiteral::Null) => Some(VbaTypeId::Variant),
        HirExprKind::Literal(HirLiteral::Nothing) => Some(VbaTypeId::Object),
        HirExprKind::Binary { lhs, rhs, .. } => {
            let lhs_type = infer_expr_type(hir, symbol_types, *lhs)?;
            let rhs_type = infer_expr_type(hir, symbol_types, *rhs)?;
            Some(binary_result_type(lhs_type, rhs_type))
        }
        _ => None,
    }
}

fn binary_result_type(lhs: VbaTypeId, rhs: VbaTypeId) -> VbaTypeId {
    if lhs == VbaTypeId::Variant || rhs == VbaTypeId::Variant {
        return VbaTypeId::Variant;
    }
    if lhs == VbaTypeId::String || rhs == VbaTypeId::String {
        return VbaTypeId::String;
    }
    if lhs == VbaTypeId::Double || rhs == VbaTypeId::Double {
        return VbaTypeId::Double;
    }
    if lhs == VbaTypeId::Single || rhs == VbaTypeId::Single {
        return VbaTypeId::Single;
    }
    if lhs == VbaTypeId::LongLong || rhs == VbaTypeId::LongLong {
        return VbaTypeId::LongLong;
    }
    if lhs == VbaTypeId::Long || rhs == VbaTypeId::Long {
        return VbaTypeId::Long;
    }
    lhs
}

fn declared_type_after_span(
    source: &str,
    span: FrontendSourceSpan,
) -> Option<(VbaTypeId, FrontendSourceSpan)> {
    let suffix = source.get(span.end..)?;
    let line_end = span.end + suffix.find('\n').unwrap_or(suffix.len());
    let segment = source.get(span.end..line_end)?;
    let lower = segment.to_ascii_lowercase();
    let as_pos = lower.find(" as ")?;
    let after_as = span.end + as_pos + " as ".len();
    let ty_text = source
        .get(after_as..line_end)?
        .trim_start()
        .split(|ch: char| ch == ',' || ch == ')' || ch.is_whitespace())
        .next()?;
    let ty_start = source
        .get(after_as..line_end)?
        .find(ty_text)
        .map(|offset| after_as + offset)?;
    let runtime_type = parse_runtime_type(ty_text)?;
    Some((
        runtime_type,
        FrontendSourceSpan {
            start: ty_start,
            end: ty_start + ty_text.len(),
        },
    ))
}

fn parse_runtime_type(text: &str) -> Option<VbaTypeId> {
    match text.to_ascii_lowercase().as_str() {
        "boolean" => Some(VbaTypeId::Boolean),
        "byte" => Some(VbaTypeId::Byte),
        "integer" => Some(VbaTypeId::Integer),
        "long" => Some(VbaTypeId::Long),
        "longlong" => Some(VbaTypeId::LongLong),
        "longptr" => Some(VbaTypeId::LongPtr),
        "single" => Some(VbaTypeId::Single),
        "double" => Some(VbaTypeId::Double),
        "currency" => Some(VbaTypeId::Currency),
        "date" => Some(VbaTypeId::Date),
        "string" => Some(VbaTypeId::String),
        "variant" => Some(VbaTypeId::Variant),
        "object" => Some(VbaTypeId::Object),
        _ => None,
    }
}

fn normalize_identifier_token(text: &str) -> &str {
    text.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(text)
}

fn hir_builtin_type(runtime_type: VbaTypeId) -> HirBuiltinType {
    match runtime_type {
        VbaTypeId::Boolean => HirBuiltinType::Boolean,
        VbaTypeId::Integer => HirBuiltinType::Integer,
        VbaTypeId::Long => HirBuiltinType::Long,
        VbaTypeId::Double => HirBuiltinType::Double,
        VbaTypeId::String => HirBuiltinType::String,
        VbaTypeId::Object => HirBuiltinType::Object,
        _ => HirBuiltinType::Variant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_hir::{
        CstBackpointer, HirArenas, HirCall, HirExpr, HirExprKind, HirLiteral, HirStmt, HirStmtKind,
        HirType, HirTypeKind,
    };
    use crate::frontend_symbols::{
        FrontendSourceSpan, SourceProvenance, SymbolModel, SymbolNamespace,
    };

    fn cst(kind: &str) -> CstBackpointer {
        CstBackpointer {
            syntax_kind: kind.to_string(),
            span: FrontendSourceSpan { start: 0, end: 1 },
        }
    }

    fn provenance() -> SourceProvenance {
        SourceProvenance {
            module_name: Some("Module1".to_string()),
            span: Some(FrontendSourceSpan { start: 0, end: 1 }),
        }
    }

    #[test]
    fn type_hooks_connect_declared_symbol_type_to_runtime_type() {
        let mut symbols = SymbolModel::default();
        let symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Local,
                "count",
                provenance(),
            )
            .expect("symbol");
        let mut hir = HirArenas::default();
        let ty = hir.alloc_type(HirType {
            cst: cst("TypeExpr"),
            kind: HirTypeKind::Builtin(crate::frontend_hir::HirBuiltinType::Long),
        });

        let mut hooks = HirTypeHooks::default();
        hooks.record_declared_type(HirDeclaredTypeHook {
            symbol,
            hir_type: ty,
            runtime_type: VbaTypeId::Long,
        });

        assert_eq!(
            hooks.declared_type(symbol).map(|hook| hook.runtime_type),
            Some(VbaTypeId::Long)
        );
        assert_eq!(
            hooks.declared_type(symbol).map(|hook| hook.hir_type),
            Some(ty)
        );
    }

    #[test]
    fn type_hooks_record_let_set_assignment_intent_and_coercion() {
        let mut hir = HirArenas::default();
        let target = hir.alloc_expr(HirExpr {
            cst: cst("NameExpr"),
            kind: HirExprKind::Missing,
        });
        let value = hir.alloc_expr(HirExpr {
            cst: cst("StringLiteral"),
            kind: HirExprKind::Literal(HirLiteral::String("1".to_string())),
        });
        let stmt = hir.alloc_stmt(HirStmt {
            cst: cst("AssignStmt"),
            kind: HirStmtKind::Let { target, value },
        });

        let mut hooks = HirTypeHooks::default();
        hooks.record_assignment_intent(stmt, HirAssignmentIntent::Let);
        hooks.record_coercion(HirCoercionHook {
            expr: value,
            source_type: VbaTypeId::String,
            target_type: VbaTypeId::Long,
            kind: CoercionKindDescriptor::Let,
        });

        assert_eq!(
            hooks.assignment_intent(stmt),
            Some(HirAssignmentIntent::Let)
        );
        assert_eq!(hooks.coercions_for_expr(value).len(), 1);
        assert_eq!(
            hooks.coercions_for_expr(value)[0].kind,
            CoercionKindDescriptor::Let
        );
    }

    #[test]
    fn type_hooks_record_call_site_parameter_mechanics() {
        let mut symbols = SymbolModel::default();
        let proc_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Procedure,
                "TakeValue",
                provenance(),
            )
            .expect("procedure");
        let required_param = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Parameter,
                "value",
                provenance(),
            )
            .expect("required param");
        let optional_param = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Parameter,
                "count",
                provenance(),
            )
            .expect("optional param");
        let param_array = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Parameter,
                "rest",
                provenance(),
            )
            .expect("param array");

        let mut hir = HirArenas::default();
        let target = hir.alloc_expr(HirExpr {
            cst: cst("NameExpr"),
            kind: HirExprKind::Name(proc_symbol),
        });
        let arg = hir.alloc_expr(HirExpr {
            cst: cst("IntLiteral"),
            kind: HirExprKind::Literal(HirLiteral::Int(7)),
        });
        let call = hir.alloc_call(HirCall {
            cst: cst("CallExpr"),
            target,
            args: vec![arg],
        });

        let mut hooks = HirTypeHooks::default();
        hooks.record_call_site(HirCallSiteHook {
            call,
            target: proc_symbol,
            return_type: Some(VbaTypeId::Variant),
            args: vec![
                HirArgumentHook {
                    expr: Some(arg),
                    parameter: HirParameterHook {
                        symbol: required_param,
                        declared_type: VbaTypeId::Long,
                        passing_mode: ParameterPassingMode::ByRef,
                        optional: false,
                        param_array: false,
                        default_value: None,
                    },
                },
                HirArgumentHook {
                    expr: None,
                    parameter: HirParameterHook {
                        symbol: optional_param,
                        declared_type: VbaTypeId::Long,
                        passing_mode: ParameterPassingMode::ByVal,
                        optional: true,
                        param_array: false,
                        default_value: Some(OptionalDefaultValue::ExplicitI32(3)),
                    },
                },
                HirArgumentHook {
                    expr: None,
                    parameter: HirParameterHook {
                        symbol: param_array,
                        declared_type: VbaTypeId::Variant,
                        passing_mode: ParameterPassingMode::ByRef,
                        optional: false,
                        param_array: true,
                        default_value: None,
                    },
                },
            ],
        });

        let call_site = hooks.call_site(call).expect("call site hook");
        assert_eq!(call_site.target, proc_symbol);
        assert_eq!(
            call_site.args[0].parameter.passing_mode,
            ParameterPassingMode::ByRef
        );
        assert!(call_site.args[1].parameter.optional);
        assert_eq!(
            call_site.args[1].parameter.default_value,
            Some(OptionalDefaultValue::ExplicitI32(3))
        );
        assert!(call_site.args[2].parameter.param_array);
    }

    #[test]
    fn type_hooks_collect_declared_types_from_source_backed_hir() {
        let source =
            "Sub Main(ByVal seed As Long)\n    Dim label As String\n    label = \"ok\"\nEnd Sub\n";
        let typed = collect_type_hooks_from_source("Module1", source).expect("typed HIR");
        let parameter = typed
            .module
            .symbols
            .symbols()
            .iter()
            .find(|symbol| {
                symbol.namespace == SymbolNamespace::Parameter
                    && typed
                        .module
                        .symbols
                        .name(symbol.name)
                        .is_some_and(|name| name.folded == "seed")
            })
            .expect("seed parameter");
        let local = typed
            .module
            .symbols
            .symbols()
            .iter()
            .find(|symbol| {
                symbol.namespace == SymbolNamespace::Local
                    && typed
                        .module
                        .symbols
                        .name(symbol.name)
                        .is_some_and(|name| name.folded == "label")
            })
            .expect("label local");

        assert_eq!(
            typed
                .hooks
                .declared_type(parameter.id)
                .map(|hook| hook.runtime_type),
            Some(VbaTypeId::Long)
        );
        assert_eq!(
            typed
                .hooks
                .declared_type(local.id)
                .map(|hook| hook.runtime_type),
            Some(VbaTypeId::String)
        );
    }

    #[test]
    fn type_hooks_collect_function_return_type_from_source_backed_hir() {
        let source = "Function Alpha() As Object\nAlpha = Nothing\nEnd Function\n";
        let typed = collect_type_hooks_from_source("Module1", source).expect("typed HIR");
        let function = typed
            .module
            .symbols
            .symbols()
            .iter()
            .find(|symbol| {
                symbol.namespace == SymbolNamespace::Procedure
                    && typed
                        .module
                        .symbols
                        .name(symbol.name)
                        .is_some_and(|name| name.folded == "alpha")
            })
            .expect("alpha function");

        assert_eq!(
            typed
                .hooks
                .declared_type(function.id)
                .map(|hook| hook.runtime_type),
            Some(VbaTypeId::Object)
        );
    }

    #[test]
    fn type_hooks_collect_let_intent_and_string_to_long_coercion_from_hir() {
        let source = "Sub Main()\n    Dim count As Long\n    count = \"1\"\nEnd Sub\n";
        let typed = collect_type_hooks_from_source("Module1", source).expect("typed HIR");
        let assignment = typed
            .module
            .declarations
            .iter()
            .filter_map(|decl| typed.module.arenas.decl(*decl))
            .find_map(|decl| match &decl.kind {
                crate::frontend_hir::HirDeclKind::Procedure { body, .. } => body
                    .iter()
                    .find_map(|stmt| find_let_stmt(&typed.module.arenas, *stmt)),
                _ => None,
            })
            .expect("assignment");

        assert_eq!(
            typed.hooks.assignment_intent(assignment.0),
            Some(HirAssignmentIntent::Let)
        );
        let coercions = typed.hooks.coercions_for_expr(assignment.2);
        assert_eq!(coercions.len(), 1, "{coercions:#?}");
        assert_eq!(coercions[0].source_type, VbaTypeId::String);
        assert_eq!(coercions[0].target_type, VbaTypeId::Long);
        assert_eq!(coercions[0].kind, CoercionKindDescriptor::Let);
    }

    fn find_let_stmt(
        hir: &HirArenas,
        stmt: HirStmtId,
    ) -> Option<(HirStmtId, HirExprId, HirExprId)> {
        match hir.stmt(stmt).map(|stmt| &stmt.kind) {
            Some(HirStmtKind::Let { target, value }) => Some((stmt, *target, *value)),
            Some(HirStmtKind::Block(children)) => {
                children.iter().find_map(|child| find_let_stmt(hir, *child))
            }
            _ => None,
        }
    }
}
