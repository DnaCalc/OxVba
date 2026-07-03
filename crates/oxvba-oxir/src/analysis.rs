//! Shared OxIR analyses used by both verification and backends.

use crate::inst::OxInst;
use crate::program::OxFunc;
use crate::value::{OxArg, OxCallArg, OxNativeCallee, OxOperand, OxPlace};

/// Addressability facts for locals and compiler temporaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscapeFacts {
    /// Locals whose address or alias can escape the current instruction.
    pub locals: Vec<bool>,
    /// Temporaries whose address or alias can escape the current instruction.
    pub temps: Vec<bool>,
}

/// Compute the M4-1 address-escape facts for a function.
pub fn escape_facts(func: &OxFunc) -> EscapeFacts {
    let mut facts = EscapeFacts {
        locals: vec![false; func.locals.len()],
        temps: vec![false; func.temps.len()],
    };
    for block in &func.blocks {
        for inst in &block.instrs {
            mark_inst(inst, &mut facts);
        }
    }
    facts
}

/// Refresh [`crate::program::OxLocal::escaped`] flags from the shared escape analysis.
pub fn apply_escape_analysis(func: &mut OxFunc) {
    let facts = escape_facts(func);
    for (local, escaped) in func.locals.iter_mut().zip(facts.locals) {
        local.escaped = escaped;
    }
}

fn mark_inst(inst: &OxInst, facts: &mut EscapeFacts) {
    match inst {
        OxInst::AsNew { place, .. } => mark_place(place, facts),
        OxInst::CallProc { args, .. }
        | OxInst::CallProcRef { args, .. }
        | OxInst::CallExtern { args, .. }
        | OxInst::RaiseEvent { args, .. } => {
            for arg in args {
                mark_proc_arg(arg, facts);
            }
        }
        OxInst::CallNative { callee, args, .. } => {
            mark_native_callee(callee, facts);
            for arg in args {
                mark_call_arg(arg, facts);
            }
        }
        OxInst::CallByName { args, .. }
        | OxInst::ComCallEarly { args, .. }
        | OxInst::ComCallLate { args, .. } => {
            for arg in args {
                mark_call_arg(arg, facts);
            }
        }
        OxInst::ArrayLiteral { aliases, .. } => {
            for place in aliases.iter().flatten() {
                mark_place(place, facts);
            }
        }
        OxInst::ForEachInit { iter, .. } | OxInst::ForEachNext { iter, .. } => {
            mark_place(iter, facts);
        }
        OxInst::Ptr { src, .. } => mark_operand(src, facts),
        _ => {}
    }
}

fn mark_native_callee(callee: &OxNativeCallee, facts: &mut EscapeFacts) {
    if let OxNativeCallee::Declare { ptr_writebacks, .. } = callee {
        for writeback in ptr_writebacks {
            mark_place(&writeback.target, facts);
        }
    }
}

fn mark_proc_arg(arg: &OxArg, facts: &mut EscapeFacts) {
    if let OxArg::ByRef(place) = arg {
        mark_place(place, facts);
    }
}

fn mark_call_arg(arg: &OxCallArg, facts: &mut EscapeFacts) {
    if let OxCallArg::ByRef(place) = arg {
        mark_place(place, facts);
    }
}

fn mark_operand(operand: &OxOperand, facts: &mut EscapeFacts) {
    if let OxOperand::Use(place) = operand {
        mark_place(place, facts);
    }
}

fn mark_place(place: &OxPlace, facts: &mut EscapeFacts) {
    match place {
        OxPlace::Local(local) => {
            if let Some(escaped) = facts.locals.get_mut(local.0) {
                *escaped = true;
            }
        }
        OxPlace::Temp(temp) => {
            if let Some(escaped) = facts.temps.get_mut(temp.0) {
                *escaped = true;
            }
        }
        OxPlace::Global(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_bundle::ProcedureKind;

    use crate::ids::{BlockId, FuncId, LocalId, TempId};
    use crate::inst::{OxAsNew, OxBlock, OxInst, OxTerminator};
    use crate::program::{OxFunc, OxLocal};
    use crate::ty::{ClassId, OxTy};
    use crate::value::{
        DeclarePtrWriteback, OxArg, OxCallArg, OxConst, OxNativeCallee, OxOperand, OxPlace,
        PtrKind, PtrWritebackKind,
    };

    fn local(name: &str) -> OxLocal {
        OxLocal {
            name: name.into(),
            ty: OxTy::Long,
            array_element: None,
            param: None,
            escaped: false,
        }
    }

    fn func(instrs: Vec<OxInst>) -> OxFunc {
        OxFunc {
            name: "F".into(),
            kind: ProcedureKind::Sub,
            locals: vec![local("a"), local("b")],
            temps: vec![OxTy::Long, OxTy::Long],
            param_count: 0,
            return_local: None,
            blocks: vec![OxBlock {
                id: BlockId(0),
                instrs,
                fault_target: None,
                terminator: OxTerminator::Return,
            }],
            entry: BlockId(0),
        }
    }

    #[test]
    fn local_escape_rules_are_recomputed_from_ir() {
        let facts = escape_facts(&func(vec![
            OxInst::CallProc {
                dst: None,
                proc: FuncId(0),
                args: vec![OxArg::ByRef(OxPlace::Local(LocalId(0)))],
            },
            OxInst::Ptr {
                dst: OxPlace::Temp(TempId(0)),
                kind: PtrKind::Var,
                src: OxOperand::Use(OxPlace::Local(LocalId(1))),
            },
            OxInst::ArrayLiteral {
                dst: OxPlace::Temp(TempId(0)),
                values: vec![OxOperand::Const(OxConst::I32(1))],
                aliases: vec![Some(OxPlace::Local(LocalId(1)))],
                lower_bound: 0,
            },
            OxInst::ForEachInit {
                iter: OxPlace::Local(LocalId(0)),
                source: OxOperand::Use(OxPlace::Local(LocalId(1))),
            },
            OxInst::AsNew {
                place: OxPlace::Local(LocalId(1)),
                binding: OxAsNew::ProjectClass { class: ClassId(0) },
            },
        ]));
        assert_eq!(facts.locals, vec![true, true]);
    }

    #[test]
    fn temp_escape_rules_are_analysis_only() {
        let facts = escape_facts(&func(vec![
            OxInst::CallNative {
                dst: None,
                callee: OxNativeCallee::Declare {
                    descriptor_id: 0,
                    ptr_writebacks: vec![DeclarePtrWriteback {
                        arg_index: 0,
                        target: OxPlace::Temp(TempId(1)),
                        kind: PtrWritebackKind::Long,
                    }],
                },
                args: vec![OxCallArg::ByRef(OxPlace::Temp(TempId(0)))],
            },
            OxInst::ForEachNext {
                iter: OxPlace::Temp(TempId(0)),
                item: OxPlace::Local(LocalId(0)),
                has_value: OxPlace::Local(LocalId(1)),
            },
        ]));
        assert_eq!(facts.locals, vec![false, false]);
        assert_eq!(facts.temps, vec![true, true]);
    }
}
