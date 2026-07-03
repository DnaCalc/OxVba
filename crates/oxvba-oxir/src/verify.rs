//! A structural verifier for OxIR.
//!
//! It checks CFG well-formedness — block-id consistency, in-range terminator /
//! fault-target / label targets, a fault landing pad for every block that contains a
//! fallible instruction, valid entry/return/param shape, in-range func/import/class
//! references, in-range local/global/temp references, and that every `ComCallEarly`'s
//! [`crate::com::ComMethodRef`] resolves (interface in range, a COM — not project —
//! interface, member in range).

use crate::analysis::escape_facts;
use crate::com::{ComInterface, ComMethodRef};
use crate::inst::{ErrorHandler, OxAsNew, OxInst, terminator_operands, terminator_successors};
use crate::passes::{assign_repr_preserving, operand_ty, place_ty};
use crate::program::{OxFunc, OxProgram};
use crate::ty::OxTy;
use crate::value::{OxArg, OxCallArg, OxNativeCallee, OxOperand, OxPlace};

/// A single structural defect found by [`verify_program`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// `block.id` does not match its position in the block list.
    BlockIdMismatch {
        func: usize,
        position: usize,
        id: usize,
    },
    /// The function's entry block index is out of range.
    BadEntry {
        func: usize,
        entry: usize,
        blocks: usize,
    },
    /// A terminator names a successor block that does not exist.
    BadSuccessor {
        func: usize,
        block: usize,
        target: usize,
        blocks: usize,
    },
    /// A block's `fault_target` names a block that does not exist.
    BadFaultTarget {
        func: usize,
        block: usize,
        target: usize,
        blocks: usize,
    },
    /// A block contains a fallible instruction but has no `fault_target`.
    MissingFaultTarget {
        func: usize,
        block: usize,
        inst: usize,
    },
    /// A block ends in a fallible terminator (`Raise`/`Resume*`/`GoSubReturn`) but has
    /// no `fault_target`, so a raised error could not reach the statement's handler.
    MissingFaultTargetForTerminator { func: usize, block: usize },
    /// `param_count` exceeds the number of locals.
    BadParamCount {
        func: usize,
        param_count: usize,
        locals: usize,
    },
    /// The return local index is out of range.
    BadReturnLocal {
        func: usize,
        local: usize,
        locals: usize,
    },
    /// A place or operand names a local outside the function frame.
    BadLocalRef {
        func: usize,
        block: usize,
        inst: Option<usize>,
        local: usize,
        locals: usize,
    },
    /// A place or operand names a global outside the program global table.
    BadGlobalRef {
        func: usize,
        block: usize,
        inst: Option<usize>,
        global: usize,
        globals: usize,
    },
    /// A place or operand names a temp outside the function temp table.
    BadTempRef {
        func: usize,
        block: usize,
        inst: Option<usize>,
        temp: usize,
        temps: usize,
    },
    /// A statement-boundary temp clear floor is beyond the function temp table.
    BadStmtBoundaryTemp {
        func: usize,
        block: usize,
        inst: usize,
        clear_temps_from: usize,
        temps: usize,
    },
    /// A local's stored escape flag disagrees with the shared escape analysis.
    BadEscapedLocal {
        func: usize,
        local: usize,
        expected: bool,
        actual: bool,
    },
    /// An `Assign` copies between different machine representations without an explicit
    /// `Box`/`Unbox`/`Coerce`.
    BadAssignRepresentation {
        func: usize,
        block: usize,
        inst: usize,
        dst: OxTy,
        src: OxTy,
    },
    /// A `CallProc` names a function that does not exist.
    BadProcRef {
        func: usize,
        block: usize,
        inst: usize,
        proc: usize,
        funcs: usize,
    },
    /// A cross-bundle reference names an import that does not exist.
    BadImportRef {
        func: usize,
        block: usize,
        inst: usize,
        import: usize,
        imports: usize,
    },
    /// An object instruction names a class that does not exist.
    BadClassRef {
        func: usize,
        block: usize,
        inst: usize,
        class: usize,
        classes: usize,
    },
    /// A class field `As New` binding names a referenced project import that does not exist.
    BadClassFieldAsNewImportRef {
        class_index: usize,
        field: i32,
        import: usize,
        imports: usize,
    },
    /// A class field `As New` binding names a project class that does not exist.
    BadClassFieldAsNewClassRef {
        class_index: usize,
        field: i32,
        class: usize,
        classes: usize,
    },
    /// An `On Error GoTo <label>` names a block that does not exist.
    BadLabelTarget {
        func: usize,
        block: usize,
        inst: usize,
        target: usize,
        blocks: usize,
    },
    /// A `ComCallEarly` names an interface-table entry that does not exist.
    BadComIfaceRef {
        func: usize,
        block: usize,
        inst: usize,
        iface: usize,
        interfaces: usize,
    },
    /// A `ComCallEarly` names a *project* `Implements` interface — an early-bound COM
    /// call requires a COM interface (project interface members lower to typed proc
    /// dispatch, not `ComCallEarly`).
    ComCallEarlyOnProjectIface {
        func: usize,
        block: usize,
        inst: usize,
        iface: usize,
    },
    /// A `ComCallEarly` names a member index past the end of its COM interface's
    /// member list.
    BadComMemberRef {
        func: usize,
        block: usize,
        inst: usize,
        iface: usize,
        member: usize,
        members: usize,
    },
}

fn site(block: usize, inst: Option<usize>) -> String {
    match inst {
        Some(inst) => format!("block {block} inst {inst}"),
        None => format!("block {block} terminator"),
    }
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::BlockIdMismatch { func, position, id } => write!(
                f,
                "func {func}: block at position {position} has id {id} (must equal its position)"
            ),
            VerifyError::BadEntry {
                func,
                entry,
                blocks,
            } => {
                write!(
                    f,
                    "func {func}: entry block {entry} out of range ({blocks} blocks)"
                )
            }
            VerifyError::BadSuccessor {
                func,
                block,
                target,
                blocks,
            } => write!(
                f,
                "func {func} block {block}: successor {target} out of range ({blocks} blocks)"
            ),
            VerifyError::BadFaultTarget {
                func,
                block,
                target,
                blocks,
            } => write!(
                f,
                "func {func} block {block}: fault_target {target} out of range ({blocks} blocks)"
            ),
            VerifyError::MissingFaultTarget { func, block, inst } => write!(
                f,
                "func {func} block {block}: instruction {inst} is fallible but the block has no fault_target"
            ),
            VerifyError::MissingFaultTargetForTerminator { func, block } => write!(
                f,
                "func {func} block {block}: terminator is fallible (Raise) but the block has no fault_target"
            ),
            VerifyError::BadParamCount {
                func,
                param_count,
                locals,
            } => write!(
                f,
                "func {func}: param_count {param_count} exceeds local count {locals}"
            ),
            VerifyError::BadReturnLocal {
                func,
                local,
                locals,
            } => write!(
                f,
                "func {func}: return local {local} out of range ({locals} locals)"
            ),
            VerifyError::BadLocalRef {
                func,
                block,
                inst,
                local,
                locals,
            } => write!(
                f,
                "func {func} {}: local {local} out of range ({locals} locals)",
                site(*block, *inst)
            ),
            VerifyError::BadGlobalRef {
                func,
                block,
                inst,
                global,
                globals,
            } => write!(
                f,
                "func {func} {}: global {global} out of range ({globals} globals)",
                site(*block, *inst)
            ),
            VerifyError::BadTempRef {
                func,
                block,
                inst,
                temp,
                temps,
            } => write!(
                f,
                "func {func} {}: temp {temp} out of range ({temps} temps)",
                site(*block, *inst)
            ),
            VerifyError::BadStmtBoundaryTemp {
                func,
                block,
                inst,
                clear_temps_from,
                temps,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: StmtBoundary clear_temps_from {clear_temps_from} out of range ({temps} temps)"
            ),
            VerifyError::BadEscapedLocal {
                func,
                local,
                expected,
                actual,
            } => write!(
                f,
                "func {func}: local {local} escaped={actual} but escape analysis expects {expected}"
            ),
            VerifyError::BadAssignRepresentation {
                func,
                block,
                inst,
                dst,
                src,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: Assign copies {src:?} into {dst:?} without an explicit representation conversion"
            ),
            VerifyError::BadProcRef {
                func,
                block,
                inst,
                proc,
                funcs,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: CallProc proc {proc} out of range ({funcs} funcs)"
            ),
            VerifyError::BadImportRef {
                func,
                block,
                inst,
                import,
                imports,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: import {import} out of range ({imports} imports)"
            ),
            VerifyError::BadClassRef {
                func,
                block,
                inst,
                class,
                classes,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: class {class} out of range ({classes} classes)"
            ),
            VerifyError::BadClassFieldAsNewImportRef {
                class_index,
                field,
                import,
                imports,
            } => write!(
                f,
                "class {class_index} field {field}: As New import {import} out of range ({imports} imports)"
            ),
            VerifyError::BadClassFieldAsNewClassRef {
                class_index,
                field,
                class,
                classes,
            } => write!(
                f,
                "class {class_index} field {field}: As New class {class} out of range ({classes} classes)"
            ),
            VerifyError::BadLabelTarget {
                func,
                block,
                inst,
                target,
                blocks,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: On Error label {target} out of range ({blocks} blocks)"
            ),
            VerifyError::BadComIfaceRef {
                func,
                block,
                inst,
                iface,
                interfaces,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: ComCallEarly interface {iface} out of range ({interfaces} com_interfaces)"
            ),
            VerifyError::ComCallEarlyOnProjectIface {
                func,
                block,
                inst,
                iface,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: ComCallEarly targets project interface {iface} (needs a COM interface)"
            ),
            VerifyError::BadComMemberRef {
                func,
                block,
                inst,
                iface,
                member,
                members,
            } => write!(
                f,
                "func {func} block {block} inst {inst}: ComCallEarly member {member} out of range ({members} members of com_interface {iface})"
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Verify an entire program. Returns every structural defect found (empty ⇒ valid).
pub fn verify_program(program: &OxProgram) -> Result<(), Vec<VerifyError>> {
    let mut errors = Vec::new();
    verify_classes(program, &mut errors);
    for (fi, func) in program.funcs.iter().enumerate() {
        verify_func(program, fi, func, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn verify_classes(program: &OxProgram, errors: &mut Vec<VerifyError>) {
    let imports = program.imports.len();
    let classes = program.classes.len();
    for (class_index, class) in program.classes.iter().enumerate() {
        for field in &class.as_new_fields {
            match &field.binding {
                OxAsNew::ExternClass { import } if import.0 >= imports => {
                    errors.push(VerifyError::BadClassFieldAsNewImportRef {
                        class_index,
                        field: field.field,
                        import: import.0,
                        imports,
                    });
                }
                OxAsNew::ProjectClass { class } if class.0 >= classes => {
                    errors.push(VerifyError::BadClassFieldAsNewClassRef {
                        class_index,
                        field: field.field,
                        class: class.0,
                        classes,
                    });
                }
                _ => {}
            }
        }
    }
}

/// Check a `ComCallEarly`'s [`ComMethodRef`] resolves: the interface index is in
/// range, the entry is a COM (not project) interface, and the member index is in range.
fn check_com_call_early(
    fi: usize,
    bi: usize,
    ii: usize,
    method: ComMethodRef,
    com_interfaces: &[ComInterface],
    errors: &mut Vec<VerifyError>,
) {
    let Some(iface) = com_interfaces.get(method.iface.0) else {
        errors.push(VerifyError::BadComIfaceRef {
            func: fi,
            block: bi,
            inst: ii,
            iface: method.iface.0,
            interfaces: com_interfaces.len(),
        });
        return;
    };
    let Some(members) = iface.com_members() else {
        errors.push(VerifyError::ComCallEarlyOnProjectIface {
            func: fi,
            block: bi,
            inst: ii,
            iface: method.iface.0,
        });
        return;
    };
    if method.member >= members.len() {
        errors.push(VerifyError::BadComMemberRef {
            func: fi,
            block: bi,
            inst: ii,
            iface: method.iface.0,
            member: method.member,
            members: members.len(),
        });
    }
}

fn verify_func(program: &OxProgram, fi: usize, func: &OxFunc, errors: &mut Vec<VerifyError>) {
    let blocks = func.blocks.len();
    let locals = func.locals.len();
    let temps = func.temps.len();
    let funcs = program.funcs.len();
    let globals = program.globals.len();
    let imports = program.imports.len();
    let classes = program.classes.len();
    let com_interfaces = program.com_interfaces.as_slice();
    let local_tys = func
        .locals
        .iter()
        .map(|local| local.ty.clone())
        .collect::<Vec<_>>();
    let global_tys = program
        .globals
        .iter()
        .map(|global| global.ty.clone())
        .collect::<Vec<_>>();

    if func.entry.0 >= blocks {
        errors.push(VerifyError::BadEntry {
            func: fi,
            entry: func.entry.0,
            blocks,
        });
    }
    if func.param_count > locals {
        errors.push(VerifyError::BadParamCount {
            func: fi,
            param_count: func.param_count,
            locals,
        });
    }
    if let Some(ret) = func.return_local
        && ret.0 >= locals
    {
        errors.push(VerifyError::BadReturnLocal {
            func: fi,
            local: ret.0,
            locals,
        });
    }
    let escape_facts = escape_facts(func);
    for (local_idx, (local, expected)) in func.locals.iter().zip(escape_facts.locals).enumerate() {
        if local.escaped != expected {
            errors.push(VerifyError::BadEscapedLocal {
                func: fi,
                local: local_idx,
                expected,
                actual: local.escaped,
            });
        }
    }

    for (bi, block) in func.blocks.iter().enumerate() {
        if block.id.0 != bi {
            errors.push(VerifyError::BlockIdMismatch {
                func: fi,
                position: bi,
                id: block.id.0,
            });
        }
        for succ in terminator_successors(&block.terminator) {
            if succ.0 >= blocks {
                errors.push(VerifyError::BadSuccessor {
                    func: fi,
                    block: bi,
                    target: succ.0,
                    blocks,
                });
            }
        }
        if let Some(ft) = block.fault_target
            && ft.0 >= blocks
        {
            errors.push(VerifyError::BadFaultTarget {
                func: fi,
                block: bi,
                target: ft.0,
                blocks,
            });
        }
        for (ii, inst) in block.instrs.iter().enumerate() {
            if inst.is_fallible() && block.fault_target.is_none() {
                errors.push(VerifyError::MissingFaultTarget {
                    func: fi,
                    block: bi,
                    inst: ii,
                });
            }
            verify_inst_refs(
                fi,
                bi,
                ii,
                inst,
                funcs,
                imports,
                classes,
                com_interfaces,
                blocks,
                errors,
            );
            verify_inst_places(fi, bi, ii, inst, locals, globals, temps, errors);
            verify_assign_representation(fi, bi, ii, inst, &local_tys, &global_tys, func, errors);
        }
        for op in terminator_operands(&block.terminator) {
            check_operand(fi, bi, None, op, locals, globals, temps, errors);
        }
        // A fallible terminator (`Raise`/`Resume*`/`GoSubReturn`) likewise needs a fault
        // pad so the raised error can reach the enclosing statement's `On Error` handler.
        if block.terminator.is_fallible() && block.fault_target.is_none() {
            errors.push(VerifyError::MissingFaultTargetForTerminator {
                func: fi,
                block: bi,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_inst_refs(
    fi: usize,
    bi: usize,
    ii: usize,
    inst: &OxInst,
    funcs: usize,
    imports: usize,
    classes: usize,
    com_interfaces: &[ComInterface],
    blocks: usize,
    errors: &mut Vec<VerifyError>,
) {
    match inst {
        OxInst::CallProc { proc, .. } | OxInst::LoadProcRef { proc, .. } if proc.0 >= funcs => {
            errors.push(VerifyError::BadProcRef {
                func: fi,
                block: bi,
                inst: ii,
                proc: proc.0,
                funcs,
            })
        }
        OxInst::CallExtern { import, .. }
        | OxInst::NewExtern { import, .. }
        | OxInst::PredeclaredExtern { import, .. }
        | OxInst::PredeclaredExternSet { import, .. }
            if import.0 >= imports =>
        {
            errors.push(VerifyError::BadImportRef {
                func: fi,
                block: bi,
                inst: ii,
                import: import.0,
                imports,
            })
        }
        OxInst::NewObject { class, .. }
        | OxInst::Predeclared { class, .. }
        | OxInst::PredeclaredSet { class, .. }
            if class.0 >= classes =>
        {
            errors.push(VerifyError::BadClassRef {
                func: fi,
                block: bi,
                inst: ii,
                class: class.0,
                classes,
            })
        }
        OxInst::SetErrorHandler(ErrorHandler::GotoLabel(target)) if target.0 >= blocks => errors
            .push(VerifyError::BadLabelTarget {
                func: fi,
                block: bi,
                inst: ii,
                target: target.0,
                blocks,
            }),
        OxInst::AsNew {
            binding: OxAsNew::ExternClass { import },
            ..
        } if import.0 >= imports => errors.push(VerifyError::BadImportRef {
            func: fi,
            block: bi,
            inst: ii,
            import: import.0,
            imports,
        }),
        OxInst::AsNew {
            binding: OxAsNew::ProjectClass { class },
            ..
        } if class.0 >= classes => errors.push(VerifyError::BadClassRef {
            func: fi,
            block: bi,
            inst: ii,
            class: class.0,
            classes,
        }),
        OxInst::ComCallEarly { method, .. } => {
            check_com_call_early(fi, bi, ii, *method, com_interfaces, errors);
        }
        _ => {}
    }
}

fn verify_assign_representation(
    fi: usize,
    bi: usize,
    ii: usize,
    inst: &OxInst,
    locals: &[OxTy],
    globals: &[OxTy],
    func: &OxFunc,
    errors: &mut Vec<VerifyError>,
) {
    let OxInst::Assign { dst, value } = inst else {
        return;
    };
    if matches!(value, OxOperand::Const(crate::value::OxConst::Empty)) {
        return;
    }
    let Some(dst_ty) = place_ty(dst, locals, globals, &func.temps) else {
        return;
    };
    let Some(src_ty) = operand_ty(value, locals, globals, &func.temps) else {
        return;
    };
    if !assign_repr_preserving(&dst_ty, &src_ty) {
        errors.push(VerifyError::BadAssignRepresentation {
            func: fi,
            block: bi,
            inst: ii,
            dst: dst_ty,
            src: src_ty,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_inst_places(
    fi: usize,
    bi: usize,
    ii: usize,
    inst: &OxInst,
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    let site = Some(ii);
    match inst {
        OxInst::AsNew { place, .. } => {
            check_place(fi, bi, site, place, locals, globals, temps, errors);
        }
        OxInst::Assign { dst, value } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, value, locals, globals, temps, errors);
        }
        OxInst::Box { dst, src, .. }
        | OxInst::Unbox { dst, src, .. }
        | OxInst::Coerce { dst, src, .. }
        | OxInst::Truthy { dst, src }
        | OxInst::Neg { dst, src, .. }
        | OxInst::Not { dst, src }
        | OxInst::TypeOfIs {
            dst, object: src, ..
        }
        | OxInst::Ptr { dst, src, .. } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, src, locals, globals, temps, errors);
        }
        OxInst::ValidateAssignment { src, .. }
        | OxInst::PredeclaredSet { value: src, .. }
        | OxInst::PredeclaredExternSet { value: src, .. }
        | OxInst::AddRef { object: src }
        | OxInst::Release { object: src, .. }
        | OxInst::ErrFieldSet { src, .. } => {
            check_operand(fi, bi, site, src, locals, globals, temps, errors);
        }
        OxInst::LoadProcRef { dst, .. }
        | OxInst::NewObject { dst, .. }
        | OxInst::NewExtern { dst, .. }
        | OxInst::Predeclared { dst, .. }
        | OxInst::PredeclaredExtern { dst, .. }
        | OxInst::NewRecord { dst, .. }
        | OxInst::ErrFieldGet { dst, .. }
        | OxInst::ErlGet { dst }
        | OxInst::WithEventsNextOwner { dst } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
        }
        OxInst::VariantChanged {
            dst,
            current,
            original,
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, current, locals, globals, temps, errors);
            check_operand(fi, bi, site, original, locals, globals, temps, errors);
        }
        OxInst::Arith { dst, lhs, rhs, .. }
        | OxInst::Div { dst, lhs, rhs }
        | OxInst::Pow { dst, lhs, rhs }
        | OxInst::Concat { dst, lhs, rhs }
        | OxInst::Compare { dst, lhs, rhs, .. }
        | OxInst::CompareObjectIs { dst, lhs, rhs }
        | OxInst::Logical { dst, lhs, rhs, .. } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, lhs, locals, globals, temps, errors);
            check_operand(fi, bi, site, rhs, locals, globals, temps, errors);
        }
        OxInst::CallProc { dst, args, .. }
        | OxInst::CallProcRef { dst, args, .. }
        | OxInst::CallExtern { dst, args, .. } => {
            check_opt_place(fi, bi, site, dst, locals, globals, temps, errors);
            for arg in args {
                check_arg(fi, bi, site, arg, locals, globals, temps, errors);
            }
            if let OxInst::CallProcRef { target, .. } = inst {
                check_operand(fi, bi, site, target, locals, globals, temps, errors);
            }
        }
        OxInst::CallNative {
            dst, callee, args, ..
        } => {
            check_opt_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_native_callee(fi, bi, site, callee, locals, globals, temps, errors);
            for arg in args {
                check_call_arg(fi, bi, site, arg, locals, globals, temps, errors);
            }
        }
        OxInst::CallByName {
            dst,
            object,
            name,
            calltype,
            args,
        } => {
            check_opt_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, object, locals, globals, temps, errors);
            check_operand(fi, bi, site, name, locals, globals, temps, errors);
            check_operand(fi, bi, site, calltype, locals, globals, temps, errors);
            for arg in args {
                check_call_arg(fi, bi, site, arg, locals, globals, temps, errors);
            }
        }
        OxInst::ComCallEarly {
            dst, recv, args, ..
        }
        | OxInst::ComCallLate {
            dst, recv, args, ..
        } => {
            check_opt_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, recv, locals, globals, temps, errors);
            for arg in args {
                check_call_arg(fi, bi, site, arg, locals, globals, temps, errors);
            }
        }
        OxInst::FieldGet { dst, object, .. } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, object, locals, globals, temps, errors);
        }
        OxInst::FieldSet { object, value, .. } => {
            check_operand(fi, bi, site, object, locals, globals, temps, errors);
            check_operand(fi, bi, site, value, locals, globals, temps, errors);
        }
        OxInst::RecordGet { dst, record, .. } | OxInst::RecordArrayGet { dst, record, .. } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, record, locals, globals, temps, errors);
            if let OxInst::RecordArrayGet { indices, .. } = inst {
                check_operands(fi, bi, site, indices, locals, globals, temps, errors);
            }
        }
        OxInst::RecordSet { record, value, .. }
        | OxInst::RecordLSet { record, value }
        | OxInst::RecordArraySet { record, value, .. } => {
            check_place(fi, bi, site, record, locals, globals, temps, errors);
            check_operand(fi, bi, site, value, locals, globals, temps, errors);
            if let OxInst::RecordArraySet { indices, .. } = inst {
                check_operands(fi, bi, site, indices, locals, globals, temps, errors);
            }
        }
        OxInst::ArrayLiteral {
            dst,
            values,
            aliases,
            ..
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operands(fi, bi, site, values, locals, globals, temps, errors);
            for place in aliases.iter().flatten() {
                check_place(fi, bi, site, place, locals, globals, temps, errors);
            }
        }
        OxInst::ArrayAppend { dst, array, item } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, array, locals, globals, temps, errors);
            check_operand(fi, bi, site, item, locals, globals, temps, errors);
        }
        OxInst::ArrayRedim {
            dst,
            upper_bounds,
            lower_bounds,
            ..
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operands(fi, bi, site, upper_bounds, locals, globals, temps, errors);
            check_operands(fi, bi, site, lower_bounds, locals, globals, temps, errors);
        }
        OxInst::ArrayGet {
            dst,
            array,
            indices,
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, array, locals, globals, temps, errors);
            check_operands(fi, bi, site, indices, locals, globals, temps, errors);
        }
        OxInst::ArraySet {
            array,
            indices,
            value,
        } => {
            check_place(fi, bi, site, array, locals, globals, temps, errors);
            check_operands(fi, bi, site, indices, locals, globals, temps, errors);
            check_operand(fi, bi, site, value, locals, globals, temps, errors);
        }
        OxInst::FieldArrayGet {
            dst,
            object,
            indices,
            ..
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, object, locals, globals, temps, errors);
            check_operands(fi, bi, site, indices, locals, globals, temps, errors);
        }
        OxInst::FieldArraySet {
            object,
            indices,
            value,
            ..
        } => {
            check_operand(fi, bi, site, object, locals, globals, temps, errors);
            check_operands(fi, bi, site, indices, locals, globals, temps, errors);
            check_operand(fi, bi, site, value, locals, globals, temps, errors);
        }
        OxInst::ArrayErase { array, .. } => {
            check_place(fi, bi, site, array, locals, globals, temps, errors);
        }
        OxInst::Bound {
            dst,
            array,
            dimension,
            ..
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, array, locals, globals, temps, errors);
            if let Some(dimension) = dimension {
                check_operand(fi, bi, site, dimension, locals, globals, temps, errors);
            }
        }
        OxInst::ForEachInit { iter, source } => {
            check_place(fi, bi, site, iter, locals, globals, temps, errors);
            check_operand(fi, bi, site, source, locals, globals, temps, errors);
        }
        OxInst::ForEachNext {
            iter,
            item,
            has_value,
        } => {
            check_place(fi, bi, site, iter, locals, globals, temps, errors);
            check_place(fi, bi, site, item, locals, globals, temps, errors);
            check_place(fi, bi, site, has_value, locals, globals, temps, errors);
        }
        OxInst::WithEventsGet { dst, owner, .. }
        | OxInst::WithEventsClearOwner { dst, owner }
        | OxInst::WithEventsFirstOwner {
            dst, source: owner, ..
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, owner, locals, globals, temps, errors);
        }
        OxInst::WithEventsSet {
            dst, owner, value, ..
        } => {
            check_place(fi, bi, site, dst, locals, globals, temps, errors);
            check_operand(fi, bi, site, owner, locals, globals, temps, errors);
            check_operand(fi, bi, site, value, locals, globals, temps, errors);
        }
        OxInst::RaiseEvent { source, args, .. } => {
            check_operand(fi, bi, site, source, locals, globals, temps, errors);
            for arg in args {
                check_arg(fi, bi, site, arg, locals, globals, temps, errors);
            }
        }
        OxInst::StmtBoundary {
            clear_temps_from, ..
        } => {
            if *clear_temps_from > temps {
                errors.push(VerifyError::BadStmtBoundaryTemp {
                    func: fi,
                    block: bi,
                    inst: ii,
                    clear_temps_from: *clear_temps_from,
                    temps,
                });
            }
        }
        OxInst::SetErrorHandler(_)
        | OxInst::ClearErr
        | OxInst::SetLineNumber { .. }
        | OxInst::DrainTerminations => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn check_native_callee(
    fi: usize,
    bi: usize,
    site: Option<usize>,
    callee: &OxNativeCallee,
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    if let OxNativeCallee::Declare { ptr_writebacks, .. } = callee {
        for writeback in ptr_writebacks {
            check_place(
                fi,
                bi,
                site,
                &writeback.target,
                locals,
                globals,
                temps,
                errors,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_arg(
    fi: usize,
    bi: usize,
    site: Option<usize>,
    arg: &OxArg,
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    match arg {
        OxArg::ByVal(value) => check_operand(fi, bi, site, value, locals, globals, temps, errors),
        OxArg::ByRef(place) => check_place(fi, bi, site, place, locals, globals, temps, errors),
        OxArg::Omitted => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn check_call_arg(
    fi: usize,
    bi: usize,
    site: Option<usize>,
    arg: &OxCallArg,
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    match arg {
        OxCallArg::Operand(value) | OxCallArg::Named { value, .. } => {
            check_operand(fi, bi, site, value, locals, globals, temps, errors)
        }
        OxCallArg::ByRef(place) => check_place(fi, bi, site, place, locals, globals, temps, errors),
        OxCallArg::Omitted | OxCallArg::Const(_) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn check_operands(
    fi: usize,
    bi: usize,
    site: Option<usize>,
    operands: &[OxOperand],
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    for operand in operands {
        check_operand(fi, bi, site, operand, locals, globals, temps, errors);
    }
}

#[allow(clippy::too_many_arguments)]
fn check_operand(
    fi: usize,
    bi: usize,
    site: Option<usize>,
    operand: &OxOperand,
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    if let OxOperand::Use(place) = operand {
        check_place(fi, bi, site, place, locals, globals, temps, errors);
    }
}

#[allow(clippy::too_many_arguments)]
fn check_opt_place(
    fi: usize,
    bi: usize,
    site: Option<usize>,
    place: &Option<OxPlace>,
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    if let Some(place) = place {
        check_place(fi, bi, site, place, locals, globals, temps, errors);
    }
}

#[allow(clippy::too_many_arguments)]
fn check_place(
    fi: usize,
    bi: usize,
    site: Option<usize>,
    place: &OxPlace,
    locals: usize,
    globals: usize,
    temps: usize,
    errors: &mut Vec<VerifyError>,
) {
    match place {
        OxPlace::Local(local) if local.0 >= locals => errors.push(VerifyError::BadLocalRef {
            func: fi,
            block: bi,
            inst: site,
            local: local.0,
            locals,
        }),
        OxPlace::Global(global) if global.0 >= globals => errors.push(VerifyError::BadGlobalRef {
            func: fi,
            block: bi,
            inst: site,
            global: global.0,
            globals,
        }),
        OxPlace::Temp(temp) if temp.0 >= temps => errors.push(VerifyError::BadTempRef {
            func: fi,
            block: bi,
            inst: site,
            temp: temp.0,
            temps,
        }),
        _ => {}
    }
}
