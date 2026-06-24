//! A structural verifier for OxIR.
//!
//! It checks CFG well-formedness — block-id consistency, in-range terminator /
//! fault-target / label targets, a fault landing pad for every block that contains a
//! fallible instruction, valid entry/return/param shape, and in-range
//! func/import/class references. Operand-level local/global/temp bounds checking is a
//! planned addition (it needs an operand walk over every instruction); this verifier
//! establishes the graph-integrity invariants the interpreter and backend rely on.

use crate::inst::{ErrorHandler, OxInst, terminator_successors};
use crate::program::{OxFunc, OxProgram};

/// A single structural defect found by [`verify_program`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// `block.id` does not match its position in the block list.
    BlockIdMismatch { func: usize, position: usize, id: usize },
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
    /// An `On Error GoTo <label>` names a block that does not exist.
    BadLabelTarget {
        func: usize,
        block: usize,
        inst: usize,
        target: usize,
        blocks: usize,
    },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::BlockIdMismatch { func, position, id } => write!(
                f,
                "func {func}: block at position {position} has id {id} (must equal its position)"
            ),
            VerifyError::BadEntry { func, entry, blocks } => {
                write!(f, "func {func}: entry block {entry} out of range ({blocks} blocks)")
            }
            VerifyError::BadSuccessor { func, block, target, blocks } => write!(
                f,
                "func {func} block {block}: successor {target} out of range ({blocks} blocks)"
            ),
            VerifyError::BadFaultTarget { func, block, target, blocks } => write!(
                f,
                "func {func} block {block}: fault_target {target} out of range ({blocks} blocks)"
            ),
            VerifyError::MissingFaultTarget { func, block, inst } => write!(
                f,
                "func {func} block {block}: instruction {inst} is fallible but the block has no fault_target"
            ),
            VerifyError::BadParamCount { func, param_count, locals } => write!(
                f,
                "func {func}: param_count {param_count} exceeds local count {locals}"
            ),
            VerifyError::BadReturnLocal { func, local, locals } => write!(
                f,
                "func {func}: return local {local} out of range ({locals} locals)"
            ),
            VerifyError::BadProcRef { func, block, inst, proc, funcs } => write!(
                f,
                "func {func} block {block} inst {inst}: CallProc proc {proc} out of range ({funcs} funcs)"
            ),
            VerifyError::BadImportRef { func, block, inst, import, imports } => write!(
                f,
                "func {func} block {block} inst {inst}: import {import} out of range ({imports} imports)"
            ),
            VerifyError::BadClassRef { func, block, inst, class, classes } => write!(
                f,
                "func {func} block {block} inst {inst}: class {class} out of range ({classes} classes)"
            ),
            VerifyError::BadLabelTarget { func, block, inst, target, blocks } => write!(
                f,
                "func {func} block {block} inst {inst}: On Error label {target} out of range ({blocks} blocks)"
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Verify an entire program. Returns every structural defect found (empty ⇒ valid).
pub fn verify_program(program: &OxProgram) -> Result<(), Vec<VerifyError>> {
    let mut errors = Vec::new();
    for (fi, func) in program.funcs.iter().enumerate() {
        verify_func(program, fi, func, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn verify_func(program: &OxProgram, fi: usize, func: &OxFunc, errors: &mut Vec<VerifyError>) {
    let blocks = func.blocks.len();
    let locals = func.locals.len();
    let funcs = program.funcs.len();
    let imports = program.imports.len();
    let classes = program.classes.len();

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
            verify_inst_refs(fi, bi, ii, inst, funcs, imports, classes, blocks, errors);
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
    blocks: usize,
    errors: &mut Vec<VerifyError>,
) {
    match inst {
        OxInst::CallProc { proc, .. } if proc.0 >= funcs => errors.push(VerifyError::BadProcRef {
            func: fi,
            block: bi,
            inst: ii,
            proc: proc.0,
            funcs,
        }),
        OxInst::CallExtern { import, .. }
        | OxInst::NewExtern { import, .. }
        | OxInst::PredeclaredExtern { import, .. }
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
        OxInst::NewObject { class, .. } | OxInst::Predeclared { class, .. }
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
        OxInst::SetErrorHandler(ErrorHandler::GotoLabel(target)) if target.0 >= blocks => {
            errors.push(VerifyError::BadLabelTarget {
                func: fi,
                block: bi,
                inst: ii,
                target: target.0,
                blocks,
            })
        }
        _ => {}
    }
}
