use crate::frontend_hir::{
    HirCallId, HirDeclId, HirDeclKind, HirExprId, HirExprKind, HirLiteral, HirStmtId, HirStmtKind,
};
use crate::frontend_structural_intrinsics::StructuralIntrinsic;
use crate::frontend_symbols::SymbolId;
use crate::frontend_type_hooks::{HirCallSiteHook, HirCoercionHook, TypedHirModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirLoweringContract {
    pub entry_decl: HirDeclId,
    pub calls: Vec<HirCallSiteHook>,
    pub returns: Vec<HirExprId>,
    pub writebacks: Vec<HirWritebackContract>,
    pub frame_overlay: HirFrameOverlay,
    pub structural_intrinsics: Vec<StructuralIntrinsic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirWritebackContract {
    pub call: HirCallId,
    pub arg_expr: HirExprId,
    pub target_stmt: Option<HirStmtId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirFrameOverlay {
    pub locals: Vec<HirFrameSlot>,
    pub temporaries: Vec<HirFrameSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirFrameSlot {
    pub slot: usize,
    pub source: HirFrameSlotSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirFrameSlotSource {
    Symbol(crate::frontend_symbols::SymbolId),
    Temporary(HirExprId),
    Coercion(HirCoercionHook),
}

impl HirLoweringContract {
    pub fn uses_legacy_intrinsic_names(&self) -> bool {
        false
    }

    pub fn assumes_flat_slots(&self) -> bool {
        false
    }
}

pub fn collect_lowering_contracts_from_typed_hir(
    typed_hir: &TypedHirModule,
) -> Vec<HirLoweringContract> {
    let mut contracts = Vec::new();
    for decl_id in &typed_hir.module.declarations {
        let Some(decl) = typed_hir.module.arenas.decl(*decl_id) else {
            continue;
        };
        let HirDeclKind::Procedure { body, .. } = &decl.kind else {
            continue;
        };

        let mut returns = Vec::new();
        let mut structural_intrinsics = Vec::new();
        for stmt in body {
            collect_stmt_contract_facts(
                typed_hir,
                decl.symbol,
                *stmt,
                &mut returns,
                &mut structural_intrinsics,
            );
        }

        let locals = typed_hir
            .hooks
            .declared_types()
            .filter(|hook| symbol_belongs_to_decl_span(typed_hir, hook.symbol, *decl_id))
            .filter(|hook| !is_declaration_modifier_symbol(typed_hir, hook.symbol))
            .enumerate()
            .map(|(slot, hook)| HirFrameSlot {
                slot,
                source: HirFrameSlotSource::Symbol(hook.symbol),
            })
            .collect::<Vec<_>>();

        let temporaries = typed_hir
            .hooks
            .coercions()
            .filter(|hook| expr_belongs_to_decl_span(typed_hir, hook.expr, *decl_id))
            .enumerate()
            .map(|(offset, hook)| HirFrameSlot {
                slot: locals.len() + offset,
                source: HirFrameSlotSource::Coercion(hook.clone()),
            })
            .collect::<Vec<_>>();

        contracts.push(HirLoweringContract {
            entry_decl: *decl_id,
            calls: typed_hir.hooks.call_sites().cloned().collect(),
            returns,
            writebacks: Vec::new(),
            frame_overlay: HirFrameOverlay {
                locals,
                temporaries,
            },
            structural_intrinsics,
        });
    }
    contracts
}

pub fn symbol_folded_name(typed_hir: &TypedHirModule, symbol: SymbolId) -> Option<&str> {
    let symbol = typed_hir.module.symbols.symbol(symbol)?;
    Some(typed_hir.module.symbols.name(symbol.name)?.folded.as_str())
}

fn symbol_belongs_to_decl_span(
    typed_hir: &TypedHirModule,
    symbol: SymbolId,
    decl: HirDeclId,
) -> bool {
    let Some(symbol_span) = typed_hir
        .module
        .symbols
        .symbol(symbol)
        .and_then(|symbol| symbol.provenance.span)
    else {
        return false;
    };
    let Some(decl_span) = typed_hir.module.arenas.decl(decl).map(|decl| decl.cst.span) else {
        return false;
    };
    symbol_span.start >= decl_span.start && symbol_span.end <= decl_span.end
}

fn expr_belongs_to_decl_span(typed_hir: &TypedHirModule, expr: HirExprId, decl: HirDeclId) -> bool {
    let Some(expr_span) = typed_hir.module.arenas.expr(expr).map(|expr| expr.cst.span) else {
        return false;
    };
    let Some(decl_span) = typed_hir.module.arenas.decl(decl).map(|decl| decl.cst.span) else {
        return false;
    };
    expr_span.start >= decl_span.start && expr_span.end <= decl_span.end
}

fn is_declaration_modifier_symbol(typed_hir: &TypedHirModule, symbol: SymbolId) -> bool {
    matches!(
        symbol_folded_name(typed_hir, symbol),
        Some("withevents" | "optional" | "byval" | "byref" | "paramarray")
    )
}

fn collect_stmt_contract_facts(
    typed_hir: &TypedHirModule,
    proc_symbol: SymbolId,
    stmt: HirStmtId,
    returns: &mut Vec<HirExprId>,
    structural_intrinsics: &mut Vec<StructuralIntrinsic>,
) {
    let Some(stmt_data) = typed_hir.module.arenas.stmt(stmt) else {
        return;
    };
    match &stmt_data.kind {
        HirStmtKind::Let { target, value } | HirStmtKind::Set { target, value } => {
            if matches!(
                typed_hir.module.arenas.expr(*target).map(|expr| &expr.kind),
                Some(HirExprKind::Name(symbol)) if *symbol == proc_symbol
            ) {
                returns.push(*value);
            }
            collect_expr_structural_intrinsics(typed_hir, *target, structural_intrinsics);
            collect_expr_structural_intrinsics(typed_hir, *value, structural_intrinsics);
        }
        HirStmtKind::Expr(expr) => {
            collect_expr_structural_intrinsics(typed_hir, *expr, structural_intrinsics)
        }
        HirStmtKind::If {
            condition,
            then_body,
            else_body,
        } => {
            collect_expr_structural_intrinsics(typed_hir, *condition, structural_intrinsics);
            for child in then_body.iter().chain(else_body) {
                collect_stmt_contract_facts(
                    typed_hir,
                    proc_symbol,
                    *child,
                    returns,
                    structural_intrinsics,
                );
            }
        }
        HirStmtKind::DoWhile {
            condition, body, ..
        } => {
            collect_expr_structural_intrinsics(typed_hir, *condition, structural_intrinsics);
            for child in body {
                collect_stmt_contract_facts(
                    typed_hir,
                    proc_symbol,
                    *child,
                    returns,
                    structural_intrinsics,
                );
            }
        }
        HirStmtKind::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            collect_expr_structural_intrinsics(typed_hir, *expr, structural_intrinsics);
            for (clauses, body) in arms {
                for clause in clauses {
                    collect_expr_structural_intrinsics(typed_hir, *clause, structural_intrinsics);
                }
                for child in body {
                    collect_stmt_contract_facts(
                        typed_hir,
                        proc_symbol,
                        *child,
                        returns,
                        structural_intrinsics,
                    );
                }
            }
            for child in else_body {
                collect_stmt_contract_facts(
                    typed_hir,
                    proc_symbol,
                    *child,
                    returns,
                    structural_intrinsics,
                );
            }
        }
        HirStmtKind::ForRange {
            start,
            end,
            step,
            body,
            ..
        } => {
            collect_expr_structural_intrinsics(typed_hir, *start, structural_intrinsics);
            collect_expr_structural_intrinsics(typed_hir, *end, structural_intrinsics);
            if let Some(step) = step {
                collect_expr_structural_intrinsics(typed_hir, *step, structural_intrinsics);
            }
            for child in body {
                collect_stmt_contract_facts(
                    typed_hir,
                    proc_symbol,
                    *child,
                    returns,
                    structural_intrinsics,
                );
            }
        }
        HirStmtKind::Block(children) => {
            for child in children {
                collect_stmt_contract_facts(
                    typed_hir,
                    proc_symbol,
                    *child,
                    returns,
                    structural_intrinsics,
                );
            }
        }
        HirStmtKind::Empty => {}
    }
}

fn collect_expr_structural_intrinsics(
    typed_hir: &TypedHirModule,
    expr: HirExprId,
    structural_intrinsics: &mut Vec<StructuralIntrinsic>,
) {
    let Some(expr_data) = typed_hir.module.arenas.expr(expr) else {
        return;
    };
    match &expr_data.kind {
        HirExprKind::Literal(HirLiteral::Null) => {
            structural_intrinsics.push(StructuralIntrinsic::NullLiteral)
        }
        HirExprKind::Literal(HirLiteral::Nothing) => {
            structural_intrinsics.push(StructuralIntrinsic::NothingLiteral)
        }
        HirExprKind::Unary { expr, .. } => {
            collect_expr_structural_intrinsics(typed_hir, *expr, structural_intrinsics)
        }
        HirExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_structural_intrinsics(typed_hir, *lhs, structural_intrinsics);
            collect_expr_structural_intrinsics(typed_hir, *rhs, structural_intrinsics);
        }
        HirExprKind::Call(call) => {
            if let Some(call_data) = typed_hir.module.arenas.call(*call) {
                collect_expr_structural_intrinsics(
                    typed_hir,
                    call_data.target,
                    structural_intrinsics,
                );
                for arg in &call_data.args {
                    collect_expr_structural_intrinsics(typed_hir, *arg, structural_intrinsics);
                }
            }
        }
        HirExprKind::Member(member) => {
            if let Some(member_data) = typed_hir.module.arenas.member(*member)
                && let Some(receiver) = member_data.receiver
            {
                collect_expr_structural_intrinsics(typed_hir, receiver, structural_intrinsics);
            }
        }
        HirExprKind::Missing
        | HirExprKind::Literal(HirLiteral::Empty)
        | HirExprKind::Literal(HirLiteral::Bool(_))
        | HirExprKind::Literal(HirLiteral::Int(_))
        | HirExprKind::Literal(HirLiteral::String(_))
        | HirExprKind::Name(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_symbols::SymbolId;
    use crate::frontend_type_hooks::{HirArgumentHook, HirParameterHook};
    use crate::{ParameterPassingMode, VbaTypeId};

    #[test]
    fn lowering_contract_carries_descriptor_backed_calls_returns_and_writebacks() {
        let call_hook = HirCallSiteHook {
            call: HirCallId(1),
            target: SymbolId(2),
            args: vec![HirArgumentHook {
                expr: Some(HirExprId(3)),
                parameter: HirParameterHook {
                    symbol: SymbolId(4),
                    declared_type: VbaTypeId::Long,
                    passing_mode: ParameterPassingMode::ByRef,
                    optional: false,
                    param_array: false,
                    default_value: None,
                },
            }],
            return_type: Some(VbaTypeId::Long),
        };
        let contract = HirLoweringContract {
            entry_decl: HirDeclId(0),
            calls: vec![call_hook],
            returns: vec![HirExprId(5)],
            writebacks: vec![HirWritebackContract {
                call: HirCallId(1),
                arg_expr: HirExprId(3),
                target_stmt: Some(HirStmtId(6)),
            }],
            frame_overlay: HirFrameOverlay {
                locals: vec![HirFrameSlot {
                    slot: 0,
                    source: HirFrameSlotSource::Symbol(SymbolId(7)),
                }],
                temporaries: Vec::new(),
            },
            structural_intrinsics: vec![StructuralIntrinsic::OmittedArgument],
        };

        assert_eq!(contract.calls[0].target, SymbolId(2));
        assert_eq!(contract.returns, vec![HirExprId(5)]);
        assert_eq!(contract.writebacks[0].arg_expr, HirExprId(3));
        assert!(!contract.uses_legacy_intrinsic_names());
        assert!(!contract.assumes_flat_slots());
    }

    #[test]
    fn lowering_contract_is_collected_from_typed_hir_production_surface() {
        let source = "Function Main(ByVal seed As Long) As Long\nDim total As Long\ntotal = seed\nMain = total\nEnd Function\n";
        let typed_hir =
            crate::frontend_type_hooks::collect_type_hooks_from_source("Module1", source)
                .expect("typed HIR");
        let contracts = collect_lowering_contracts_from_typed_hir(&typed_hir);
        assert_eq!(contracts.len(), 1);
        let contract = &contracts[0];
        assert_eq!(contract.frame_overlay.locals.len(), 2);
        assert_eq!(contract.returns.len(), 1);
        assert!(!contract.uses_legacy_intrinsic_names());
        assert!(!contract.assumes_flat_slots());

        let local_names = contract
            .frame_overlay
            .locals
            .iter()
            .filter_map(|slot| match slot.source {
                HirFrameSlotSource::Symbol(symbol) => symbol_folded_name(&typed_hir, symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(local_names.contains(&"seed"));
        assert!(local_names.contains(&"total"));
    }
}
