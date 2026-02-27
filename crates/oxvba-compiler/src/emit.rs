use std::collections::HashMap;

use crate::{
    bytecode::{Bytecode, Instruction},
    resolve::{
        BoundCond, BoundExpr, BoundModule, BoundParam, BoundProcedure, BoundStmt, CompareOp,
    },
};

#[derive(Debug, Clone)]
struct EmitProcMeta {
    params: Vec<BoundParam>,
    slots: HashMap<String, usize>,
}

pub fn emit_bytecode(module: &BoundModule) -> Bytecode {
    let procedures = if module.procedures.is_empty() {
        vec![BoundProcedure {
            name: "main".to_string(),
            params: Vec::new(),
            declarations: module.declarations.clone(),
            body: module.body.clone(),
        }]
    } else {
        module.procedures.clone()
    };

    let entry_idx = procedures
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case("main"))
        .unwrap_or(0);

    let mut proc_slots: Vec<HashMap<String, usize>> = Vec::new();
    let mut next_slot = 0usize;
    for proc in &procedures {
        let mut map = HashMap::new();
        for name in &proc.declarations {
            map.insert(name.clone(), next_slot);
            next_slot += 1;
        }
        proc_slots.push(map);
    }

    let mut temps = TempSlotAllocator::new(next_slot);
    let mut instructions = Vec::new();
    let mut loop_exit_stack: Vec<Vec<usize>> = Vec::new();
    let mut call_patches: Vec<(usize, String)> = Vec::new();
    let mut proc_labels: HashMap<String, usize> = HashMap::new();
    let mut proc_meta: HashMap<String, EmitProcMeta> = HashMap::new();
    for (idx, proc) in procedures.iter().enumerate() {
        proc_meta.insert(
            proc.name.clone(),
            EmitProcMeta {
                params: proc.params.clone(),
                slots: proc_slots[idx].clone(),
            },
        );
    }
    proc_labels.insert(procedures[entry_idx].name.clone(), 0);

    emit_stmt_list(
        &procedures[entry_idx].body,
        &proc_slots[entry_idx],
        &mut temps,
        &mut instructions,
        &mut loop_exit_stack,
        &mut call_patches,
        &proc_meta,
    );
    instructions.push(Instruction::Halt);

    for (idx, proc) in procedures.iter().enumerate() {
        if idx == entry_idx {
            continue;
        }
        proc_labels.insert(proc.name.clone(), instructions.len());
        emit_stmt_list(
            &proc.body,
            &proc_slots[idx],
            &mut temps,
            &mut instructions,
            &mut loop_exit_stack,
            &mut call_patches,
            &proc_meta,
        );
        instructions.push(Instruction::Return);
    }

    for (patch_idx, proc_name) in call_patches {
        if let Some(target) = proc_labels.get(&proc_name).copied()
            && let Instruction::CallProc { target_pc } = &mut instructions[patch_idx]
        {
            *target_pc = target;
        }
    }

    Bytecode {
        instructions,
        slot_count: temps.total_slots(),
        user_slot_count: procedures[entry_idx].declarations.len(),
    }
}

fn emit_stmt_list(
    stmts: &[BoundStmt],
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    loop_exit_stack: &mut Vec<Vec<usize>>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
) {
    for stmt in stmts {
        emit_stmt(
            stmt,
            slot_map,
            temps,
            instructions,
            loop_exit_stack,
            call_patches,
            proc_meta,
        );
    }
}

fn emit_stmt(
    stmt: &BoundStmt,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    loop_exit_stack: &mut Vec<Vec<usize>>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
) {
    match stmt {
        BoundStmt::Assign { target, expr } => {
            if let Some(target_slot) = slot_map.get(target.as_str()).copied() {
                emit_expr_into(expr, target_slot, slot_map, instructions);
            }
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            let cond_slot = temps.alloc_temp();
            emit_cond_into(cond, cond_slot, slot_map, temps, instructions);
            let jump_patch = instructions.len();
            instructions.push(Instruction::JumpIfZero {
                cond_slot,
                target_pc: 0,
            });
            emit_stmt_list(
                then_body,
                slot_map,
                temps,
                instructions,
                loop_exit_stack,
                call_patches,
                proc_meta,
            );
            if else_body.is_empty() {
                let target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[jump_patch] {
                    *target_pc = target;
                }
            } else {
                let end_patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                let else_target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[jump_patch] {
                    *target_pc = else_target;
                }
                emit_stmt_list(
                    else_body,
                    slot_map,
                    temps,
                    instructions,
                    loop_exit_stack,
                    call_patches,
                    proc_meta,
                );
                let end_target = instructions.len();
                if let Instruction::Jump { target_pc } = &mut instructions[end_patch] {
                    *target_pc = end_target;
                }
            }
        }
        BoundStmt::ForRange {
            var,
            start,
            end,
            body,
        } => {
            if let Some(var_slot) = slot_map.get(var.as_str()).copied() {
                emit_expr_into(start, var_slot, slot_map, instructions);
                let end_slot = temps.alloc_temp();
                let cond_slot = temps.alloc_temp();
                emit_expr_into(end, end_slot, slot_map, instructions);

                let loop_head = instructions.len();
                instructions.push(Instruction::CmpLeSlots {
                    dst: cond_slot,
                    lhs: var_slot,
                    rhs: end_slot,
                });
                let exit_patch = instructions.len();
                instructions.push(Instruction::JumpIfZero {
                    cond_slot,
                    target_pc: 0,
                });
                emit_stmt_list(
                    body,
                    slot_map,
                    temps,
                    instructions,
                    loop_exit_stack,
                    call_patches,
                    proc_meta,
                );
                instructions.push(Instruction::IncSlot { slot: var_slot });
                instructions.push(Instruction::Jump {
                    target_pc: loop_head,
                });
                let exit_target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[exit_patch] {
                    *target_pc = exit_target;
                }
            }
        }
        BoundStmt::DoWhile {
            cond,
            body,
            post_check,
        } => {
            let loop_head = instructions.len();
            let cond_slot = temps.alloc_temp();
            let mut entry_exit_patch: Option<usize> = None;

            if !post_check {
                emit_cond_into(cond, cond_slot, slot_map, temps, instructions);
                let exit_patch = instructions.len();
                instructions.push(Instruction::JumpIfZero {
                    cond_slot,
                    target_pc: 0,
                });
                entry_exit_patch = Some(exit_patch);
            }

            loop_exit_stack.push(Vec::new());
            emit_stmt_list(
                body,
                slot_map,
                temps,
                instructions,
                loop_exit_stack,
                call_patches,
                proc_meta,
            );

            emit_cond_into(cond, cond_slot, slot_map, temps, instructions);
            let post_exit_patch = instructions.len();
            instructions.push(Instruction::JumpIfZero {
                cond_slot,
                target_pc: 0,
            });
            instructions.push(Instruction::Jump {
                target_pc: loop_head,
            });

            let exit_target = instructions.len();
            if let Some(entry_patch) = entry_exit_patch
                && let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[entry_patch]
            {
                *target_pc = exit_target;
            }
            if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[post_exit_patch] {
                *target_pc = exit_target;
            }

            if let Some(exit_patches) = loop_exit_stack.pop() {
                for patch in exit_patches {
                    if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                        *target_pc = exit_target;
                    }
                }
            }
        }
        BoundStmt::ExitDo => {
            if let Some(exit_patches) = loop_exit_stack.last_mut() {
                let patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                exit_patches.push(patch);
            }
        }
        BoundStmt::OnErrorResumeNext => {
            instructions.push(Instruction::SetOnErrorResumeNext);
        }
        BoundStmt::OnErrorGoto0 => {
            instructions.push(Instruction::SetOnErrorGoto0);
        }
        BoundStmt::ResumeNext => {
            instructions.push(Instruction::ResumeNext);
        }
        BoundStmt::RaiseError(code) => {
            instructions.push(Instruction::RaiseError { code: *code });
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            let expr_slot = temps.alloc_temp();
            emit_expr_into(expr, expr_slot, slot_map, instructions);
            let mut end_patches: Vec<usize> = Vec::new();

            for (values, body) in arms {
                let aggregate_slot = temps.alloc_temp();
                instructions.push(Instruction::LoadConstI32 {
                    slot: aggregate_slot,
                    value: 0,
                });

                for value in values {
                    let const_slot = temps.alloc_temp();
                    let cmp_slot = temps.alloc_temp();
                    instructions.push(Instruction::LoadConstI32 {
                        slot: const_slot,
                        value: *value,
                    });
                    instructions.push(Instruction::CmpEqSlots {
                        dst: cmp_slot,
                        lhs: expr_slot,
                        rhs: const_slot,
                    });
                    instructions.push(Instruction::BoolOr {
                        dst: aggregate_slot,
                        lhs: aggregate_slot,
                        rhs: cmp_slot,
                    });
                }

                let next_patch = instructions.len();
                instructions.push(Instruction::JumpIfZero {
                    cond_slot: aggregate_slot,
                    target_pc: 0,
                });
                emit_stmt_list(
                    body,
                    slot_map,
                    temps,
                    instructions,
                    loop_exit_stack,
                    call_patches,
                    proc_meta,
                );
                let end_patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                end_patches.push(end_patch);
                let next_target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[next_patch] {
                    *target_pc = next_target;
                }
            }

            emit_stmt_list(
                else_body,
                slot_map,
                temps,
                instructions,
                loop_exit_stack,
                call_patches,
                proc_meta,
            );
            let end_target = instructions.len();
            for patch in end_patches {
                if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                    *target_pc = end_target;
                }
            }
        }
        BoundStmt::Call { name, args } => {
            let mut byref_copyback: Vec<(usize, usize)> = Vec::new();
            if let Some(meta) = proc_meta.get(name) {
                for (idx, param) in meta.params.iter().enumerate() {
                    let Some(arg) = args.get(idx) else {
                        continue;
                    };
                    let Some(param_slot) = meta.slots.get(param.name.as_str()).copied() else {
                        continue;
                    };

                    if param.by_ref
                        && let BoundExpr::Var(var_name) = arg
                        && let Some(src_slot) = slot_map.get(var_name.as_str()).copied()
                    {
                        if src_slot != param_slot {
                            instructions.push(Instruction::CopySlot {
                                dst: param_slot,
                                src: src_slot,
                            });
                        }
                        byref_copyback.push((src_slot, param_slot));
                        continue;
                    }

                    emit_expr_into(arg, param_slot, slot_map, instructions);
                }
            }

            let patch_idx = instructions.len();
            instructions.push(Instruction::CallProc { target_pc: 0 });
            call_patches.push((patch_idx, name.clone()));

            for (dst_slot, src_slot) in byref_copyback {
                if dst_slot != src_slot {
                    instructions.push(Instruction::CopySlot {
                        dst: dst_slot,
                        src: src_slot,
                    });
                }
            }
        }
        BoundStmt::Unsupported { .. } => {}
    }
}

fn emit_cond_into(
    cond: &BoundCond,
    dst: usize,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
) {
    match cond {
        BoundCond::Compare { op, lhs, rhs } => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_expr_into(lhs, lhs_slot, slot_map, instructions);
            emit_expr_into(rhs, rhs_slot, slot_map, instructions);
            match op {
                CompareOp::Eq => instructions.push(Instruction::CmpEqSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Ne => instructions.push(Instruction::CmpNeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Lt => instructions.push(Instruction::CmpLtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Le => instructions.push(Instruction::CmpLeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Gt => instructions.push(Instruction::CmpGtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
                CompareOp::Ge => instructions.push(Instruction::CmpGeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                }),
            }
        }
        BoundCond::Truthy(expr) => {
            let expr_slot = temps.alloc_temp();
            let zero_slot = temps.alloc_temp();
            emit_expr_into(expr, expr_slot, slot_map, instructions);
            instructions.push(Instruction::LoadConstI32 {
                slot: zero_slot,
                value: 0,
            });
            instructions.push(Instruction::CmpNeSlots {
                dst,
                lhs: expr_slot,
                rhs: zero_slot,
            });
        }
        BoundCond::Not(inner) => {
            let inner_slot = temps.alloc_temp();
            emit_cond_into(inner, inner_slot, slot_map, temps, instructions);
            instructions.push(Instruction::BoolNot {
                dst,
                src: inner_slot,
            });
        }
        BoundCond::And(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(lhs, lhs_slot, slot_map, temps, instructions);
            emit_cond_into(rhs, rhs_slot, slot_map, temps, instructions);
            instructions.push(Instruction::BoolAnd {
                dst,
                lhs: lhs_slot,
                rhs: rhs_slot,
            });
        }
        BoundCond::Or(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(lhs, lhs_slot, slot_map, temps, instructions);
            emit_cond_into(rhs, rhs_slot, slot_map, temps, instructions);
            instructions.push(Instruction::BoolOr {
                dst,
                lhs: lhs_slot,
                rhs: rhs_slot,
            });
        }
    }
}

fn emit_expr_into(
    expr: &BoundExpr,
    dst: usize,
    slot_map: &HashMap<String, usize>,
    instructions: &mut Vec<Instruction>,
) {
    match expr {
        BoundExpr::IntConst(value) => instructions.push(Instruction::LoadConstI32 {
            slot: dst,
            value: *value,
        }),
        BoundExpr::Var(name) => {
            if let Some(src) = slot_map.get(name.as_str()).copied()
                && src != dst
            {
                instructions.push(Instruction::CopySlot { dst, src });
            } else if name.eq_ignore_ascii_case("err_number") {
                instructions.push(Instruction::LoadErrNumber { slot: dst });
            }
        }
        BoundExpr::AddConst { var, delta } => {
            if let Some(src) = slot_map.get(var.as_str()).copied() {
                if src != dst {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
                instructions.push(Instruction::AddConstI32 {
                    slot: dst,
                    value: *delta,
                });
            } else if var.eq_ignore_ascii_case("err_number") {
                instructions.push(Instruction::LoadErrNumber { slot: dst });
                instructions.push(Instruction::AddConstI32 {
                    slot: dst,
                    value: *delta,
                });
            }
        }
        BoundExpr::SubConst { var, delta } => {
            if let Some(src) = slot_map.get(var.as_str()).copied() {
                if src != dst {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
                instructions.push(Instruction::SubConstI32 {
                    slot: dst,
                    value: *delta,
                });
            } else if var.eq_ignore_ascii_case("err_number") {
                instructions.push(Instruction::LoadErrNumber { slot: dst });
                instructions.push(Instruction::SubConstI32 {
                    slot: dst,
                    value: *delta,
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
struct TempSlotAllocator {
    declared_count: usize,
    next_temp: usize,
}

impl TempSlotAllocator {
    fn new(declared_count: usize) -> Self {
        Self {
            declared_count,
            next_temp: declared_count,
        }
    }

    fn alloc_temp(&mut self) -> usize {
        let slot = self.next_temp;
        self.next_temp += 1;
        slot
    }

    fn total_slots(&self) -> usize {
        self.next_temp.max(self.declared_count)
    }
}

#[cfg(test)]
mod tests {
    use super::{Instruction, TempSlotAllocator, emit_bytecode};
    use crate::resolve::resolve_symbols;

    #[test]
    fn temp_slot_allocator_starts_after_declarations() {
        let mut alloc = TempSlotAllocator::new(2);
        let a = alloc.alloc_temp();
        let b = alloc.alloc_temp();
        assert_eq!(a, 2);
        assert_eq!(b, 3);
        assert_eq!(alloc.total_slots(), 4);
    }

    #[test]
    fn emits_if_and_for_control_flow() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nIf x = 0 Then\nx = 5\nEnd If\nFor i = 1 To 2\nx = x + 1\nNext i\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpEqSlots { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLeSlots { .. }))
        );
    }

    #[test]
    fn emits_do_while_loop_and_exit_do() {
        let source = "Sub Main()\nDim x\nDo While x < 5\nx = x + 1\nIf x = 3 Then\nExit Do\nEnd If\nLoop\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        let jump_count = code
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Jump { .. }))
            .count();
        let jump_if_count = code
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::JumpIfZero { .. }))
            .count();
        assert!(jump_count >= 2);
        assert!(jump_if_count >= 2);
    }

    #[test]
    fn emits_select_case_dispatch_jumps() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase 1\nx = 10\nCase 2, 3\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolOr { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn emits_callproc_and_return_for_named_sub() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall Foo\nEnd Sub\nSub Foo()\nDim y\ny = 2\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Return))
        );
    }

    #[test]
    fn emits_on_error_resume_next_and_raise_ops() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SetOnErrorResumeNext))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::RaiseError { code: 5 }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrNumber { .. }))
        );
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::TempSlotAllocator;

    #[kani::proof]
    fn temp_slots_do_not_overlap_declared_slots() {
        let declared: usize = kani::any();
        kani::assume(declared < 1024);
        let mut alloc = TempSlotAllocator::new(declared);
        let a = alloc.alloc_temp();
        let b = alloc.alloc_temp();
        assert!(a >= declared);
        assert!(b >= declared);
        assert!(b > a);
    }
}
