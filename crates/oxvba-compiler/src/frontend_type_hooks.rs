use std::collections::BTreeMap;

use crate::frontend_hir::{HirCallId, HirExprId, HirStmtId, HirTypeId};
use crate::frontend_symbols::SymbolId;
use crate::{CoercionKindDescriptor, OptionalDefaultValue, ParameterPassingMode, VbaTypeId};

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

impl HirTypeHooks {
    pub fn record_declared_type(&mut self, hook: HirDeclaredTypeHook) {
        self.declared_types_by_symbol.insert(hook.symbol, hook);
    }

    pub fn declared_type(&self, symbol: SymbolId) -> Option<&HirDeclaredTypeHook> {
        self.declared_types_by_symbol.get(&symbol)
    }

    pub fn record_assignment_intent(&mut self, stmt: HirStmtId, intent: HirAssignmentIntent) {
        self.assignment_intents.insert(stmt, intent);
    }

    pub fn assignment_intent(&self, stmt: HirStmtId) -> Option<HirAssignmentIntent> {
        self.assignment_intents.get(&stmt).copied()
    }

    pub fn record_call_site(&mut self, hook: HirCallSiteHook) {
        self.call_sites.insert(hook.call, hook);
    }

    pub fn call_site(&self, call: HirCallId) -> Option<&HirCallSiteHook> {
        self.call_sites.get(&call)
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
}
