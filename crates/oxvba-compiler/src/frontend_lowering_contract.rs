use crate::frontend_hir::{HirCallId, HirDeclId, HirExprId, HirStmtId};
use crate::frontend_structural_intrinsics::StructuralIntrinsic;
use crate::frontend_type_hooks::{HirCallSiteHook, HirCoercionHook};

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
}
