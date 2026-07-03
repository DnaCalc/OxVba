//! Canonical OxIR normalization passes.

use oxvba_bundle::NumericCoerceTarget;

use crate::ids::TempId;
use crate::inst::OxInst;
use crate::program::OxProgram;
use crate::ty::{ObjClass, OxTy};
use crate::value::{OxCoerceTarget, OxConst, OxOperand, OxPlace};

/// Normalize `Assign` so representation-changing/value-changing copies go through
/// explicit `Box`/`Unbox`/`Coerce` instructions before the final representation-preserving
/// assignment.
pub fn normalize_assigns(program: &mut OxProgram) {
    let global_tys = program
        .globals
        .iter()
        .map(|global| global.ty.clone())
        .collect::<Vec<_>>();
    for func in &mut program.funcs {
        let local_tys = func
            .locals
            .iter()
            .map(|local| local.ty.clone())
            .collect::<Vec<_>>();
        let mut temps = std::mem::take(&mut func.temps);
        for block in &mut func.blocks {
            let mut out = Vec::with_capacity(block.instrs.len());
            for inst in std::mem::take(&mut block.instrs) {
                match inst {
                    OxInst::Assign { dst, value } => {
                        let dst_ty = place_ty(&dst, &local_tys, &global_tys, &temps);
                        let src_ty = operand_ty(&value, &local_tys, &global_tys, &temps);
                        normalize_assign_inst(&mut out, &mut temps, dst, value, dst_ty, src_ty);
                    }
                    other => out.push(other),
                }
            }
            block.instrs = out;
        }
        func.temps = temps;
    }
}

fn normalize_assign_inst(
    out: &mut Vec<OxInst>,
    temps: &mut Vec<OxTy>,
    dst: OxPlace,
    value: OxOperand,
    dst_ty: Option<OxTy>,
    src_ty: Option<OxTy>,
) {
    // The lowerer uses raw `Empty` stores as VM-visible reset/finalization sentinels
    // (notably `For Each` exhaustion). Preserve them; normal user coercions should arrive as
    // explicit `Coerce`.
    if matches!(value, OxOperand::Const(OxConst::Empty)) {
        out.push(OxInst::Assign { dst, value });
        return;
    }
    let (Some(dst_ty), Some(src_ty)) = (dst_ty, src_ty) else {
        out.push(OxInst::Assign { dst, value });
        return;
    };
    if assign_repr_preserving(&dst_ty, &src_ty) {
        out.push(OxInst::Assign { dst, value });
        return;
    }
    let tmp = fresh_temp(temps, dst_ty.clone());
    let Some(convert) = conversion_for_target(tmp, value.clone(), &dst_ty, &src_ty) else {
        out.push(OxInst::Assign { dst, value });
        return;
    };
    out.push(convert);
    out.push(OxInst::Assign {
        dst,
        value: OxOperand::Use(tmp),
    });
}

fn fresh_temp(temps: &mut Vec<OxTy>, ty: OxTy) -> OxPlace {
    let id = TempId(temps.len());
    temps.push(ty);
    OxPlace::Temp(id)
}

fn conversion_for_target(
    dst: OxPlace,
    src: OxOperand,
    dst_ty: &OxTy,
    src_ty: &OxTy,
) -> Option<OxInst> {
    if dst_ty.is_variant() && !src_ty.is_variant() {
        return Some(OxInst::Box {
            dst,
            src,
            from: src_ty.clone(),
        });
    }
    if let Some(target) = numeric_target(dst_ty) {
        return Some(OxInst::Coerce {
            dst,
            src,
            target: OxCoerceTarget::Numeric(target),
        });
    }
    match dst_ty {
        OxTy::Str => Some(OxInst::Coerce {
            dst,
            src,
            target: OxCoerceTarget::Str,
        }),
        OxTy::FixedStr(n) => Some(OxInst::Coerce {
            dst,
            src,
            target: OxCoerceTarget::FixedStr(*n),
        }),
        _ if src_ty.is_variant() => Some(OxInst::Unbox {
            dst,
            src,
            to: dst_ty.clone(),
            checked: true,
        }),
        _ => None,
    }
}

pub(crate) fn assign_repr_preserving(dst_ty: &OxTy, src_ty: &OxTy) -> bool {
    if dst_ty == src_ty {
        return true;
    }
    // `Object(Untyped)` is currently also the fallback for unresolved UDT records
    // (ProgramResolver lacks a record-layout table). Keep this representation-neutral for
    // vm3 until record identities are fully threaded through OxIR.
    matches!(
        (dst_ty, src_ty),
        (OxTy::Object(_), OxTy::Object(_)) | (OxTy::Object(ObjClass::Untyped), OxTy::Variant)
    )
}

pub(crate) fn place_ty(
    place: &OxPlace,
    locals: &[OxTy],
    globals: &[OxTy],
    temps: &[OxTy],
) -> Option<OxTy> {
    match place {
        OxPlace::Local(id) => locals.get(id.0).cloned(),
        OxPlace::Global(id) => globals.get(id.0).cloned(),
        OxPlace::Temp(id) => temps.get(id.0).cloned(),
    }
}

pub(crate) fn operand_ty(
    operand: &OxOperand,
    locals: &[OxTy],
    globals: &[OxTy],
    temps: &[OxTy],
) -> Option<OxTy> {
    match operand {
        OxOperand::Const(c) => Some(const_ty(c)),
        OxOperand::Use(place) => place_ty(place, locals, globals, temps),
    }
}

fn const_ty(c: &OxConst) -> OxTy {
    match c {
        OxConst::Empty | OxConst::Null => OxTy::Variant,
        OxConst::Nothing => OxTy::Object(ObjClass::Untyped),
        OxConst::Bool(_) => OxTy::Bool,
        OxConst::I16(_) => OxTy::Integer,
        OxConst::I32(_) => OxTy::Long,
        OxConst::I64(_) => OxTy::LongLong,
        OxConst::F32(_) => OxTy::Single,
        OxConst::F64(_) => OxTy::Double,
        OxConst::Currency(_) => OxTy::Currency,
        OxConst::Date(_) => OxTy::Date,
        OxConst::Str(_) => OxTy::Str,
    }
}

fn numeric_target(ty: &OxTy) -> Option<NumericCoerceTarget> {
    match ty {
        OxTy::Bool => Some(NumericCoerceTarget::Boolean),
        OxTy::Byte => Some(NumericCoerceTarget::Byte),
        OxTy::Integer => Some(NumericCoerceTarget::Integer),
        OxTy::Long => Some(NumericCoerceTarget::Long),
        OxTy::LongLong => Some(NumericCoerceTarget::LongLong),
        OxTy::Single => Some(NumericCoerceTarget::Single),
        OxTy::Double => Some(NumericCoerceTarget::Double),
        OxTy::Currency => Some(NumericCoerceTarget::Currency),
        OxTy::Date => Some(NumericCoerceTarget::Date),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_bundle::ProcedureKind;

    use crate::ids::{BlockId, LocalId};
    use crate::inst::{OxBlock, OxTerminator};
    use crate::program::{OxFunc, OxLocal};

    fn local(name: &str, ty: OxTy) -> OxLocal {
        OxLocal {
            name: name.into(),
            ty,
            array_element: None,
            param: None,
            escaped: false,
        }
    }

    #[test]
    fn normalize_assigns_boxes_typed_to_variant() {
        let mut program = OxProgram {
            funcs: vec![OxFunc {
                name: "Main".into(),
                kind: ProcedureKind::Sub,
                locals: vec![local("x", OxTy::Variant), local("y", OxTy::Long)],
                temps: Vec::new(),
                param_count: 0,
                return_local: None,
                blocks: vec![OxBlock {
                    id: BlockId(0),
                    instrs: vec![OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Use(OxPlace::Local(LocalId(1))),
                    }],
                    fault_target: None,
                    terminator: OxTerminator::Return,
                }],
                entry: BlockId(0),
            }],
            ..OxProgram::default()
        };
        normalize_assigns(&mut program);
        let instrs = &program.funcs[0].blocks[0].instrs;
        assert!(matches!(instrs[0], OxInst::Box { .. }));
        assert!(matches!(instrs[1], OxInst::Assign { .. }));
        assert_eq!(program.funcs[0].temps, vec![OxTy::Variant]);
    }

    #[test]
    fn normalize_assigns_coerces_numeric_to_destination_type() {
        let mut program = OxProgram {
            funcs: vec![OxFunc {
                name: "Main".into(),
                kind: ProcedureKind::Sub,
                locals: vec![local("x", OxTy::Integer)],
                temps: Vec::new(),
                param_count: 0,
                return_local: None,
                blocks: vec![OxBlock {
                    id: BlockId(0),
                    instrs: vec![OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::I32(7)),
                    }],
                    fault_target: None,
                    terminator: OxTerminator::Return,
                }],
                entry: BlockId(0),
            }],
            ..OxProgram::default()
        };
        normalize_assigns(&mut program);
        let instrs = &program.funcs[0].blocks[0].instrs;
        assert!(matches!(
            instrs[0],
            OxInst::Coerce {
                target: OxCoerceTarget::Numeric(NumericCoerceTarget::Integer),
                ..
            }
        ));
        assert!(matches!(instrs[1], OxInst::Assign { .. }));
        assert_eq!(program.funcs[0].temps, vec![OxTy::Integer]);
    }
}
