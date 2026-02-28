use std::collections::HashMap;

use oxvba_runtime::safe_array::ARRAY_TAG_BASE;

use crate::{
    bytecode::{Bytecode, Instruction, StringCompareMode},
    resolve::{
        BoundCallArg, BoundCompareMode, BoundCond, BoundExpr, BoundModule, BoundParam,
        BoundProcedure, BoundStmt, CompareOp,
    },
};

fn emit_compare_mode(mode: BoundCompareMode) -> StringCompareMode {
    match mode {
        BoundCompareMode::Binary | BoundCompareMode::Database => StringCompareMode::Binary,
        BoundCompareMode::Text => StringCompareMode::Text,
    }
}

#[derive(Debug, Clone)]
struct EmitProcMeta {
    params: Vec<BoundParam>,
    slots: HashMap<String, usize>,
}

pub fn emit_bytecode(module: &BoundModule) -> Bytecode {
    let compare_mode = emit_compare_mode(module.compare_mode);
    let procedures = if module.procedures.is_empty() {
        vec![BoundProcedure {
            name: "main".to_string(),
            return_type: crate::resolve::BoundType::Variant,
            params: Vec::new(),
            declarations: module.declarations.clone(),
            declaration_types: module.declaration_types.clone(),
            array_descriptors: module.array_descriptors.clone(),
            duplicate_declarations: Vec::new(),
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
    let mut error_handler_patches: Vec<(usize, String)> = Vec::new();
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
    let class_init_proc = procedures
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("class_initialize"))
        .map(|p| p.name.clone());
    let class_terminate_proc = procedures
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("class_terminate"))
        .map(|p| p.name.clone());
    proc_labels.insert(procedures[entry_idx].name.clone(), 0);

    if let Some(name) = class_init_proc {
        let patch_idx = instructions.len();
        instructions.push(Instruction::CallProc { target_pc: 0 });
        call_patches.push((patch_idx, name));
    }
    emit_stmt_list(
        &procedures[entry_idx].body,
        compare_mode,
        &proc_slots[entry_idx],
        &mut temps,
        &mut instructions,
        &mut loop_exit_stack,
        &mut call_patches,
        &mut error_handler_patches,
        &proc_meta,
        &procedures[entry_idx].name,
        &mut proc_labels,
    );
    if let Some(name) = class_terminate_proc {
        let patch_idx = instructions.len();
        instructions.push(Instruction::CallProc { target_pc: 0 });
        call_patches.push((patch_idx, name));
    }
    instructions.push(Instruction::Halt);

    for (idx, proc) in procedures.iter().enumerate() {
        if idx == entry_idx {
            continue;
        }
        proc_labels.insert(proc.name.clone(), instructions.len());
        emit_stmt_list(
            &proc.body,
            compare_mode,
            &proc_slots[idx],
            &mut temps,
            &mut instructions,
            &mut loop_exit_stack,
            &mut call_patches,
            &mut error_handler_patches,
            &proc_meta,
            &proc.name,
            &mut proc_labels,
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

    for (patch_idx, label_name) in error_handler_patches {
        if let Some(target) = proc_labels.get(&label_name).copied()
            && let Instruction::SetOnErrorGotoLabel { target_pc } = &mut instructions[patch_idx]
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

#[allow(clippy::too_many_arguments)]
fn emit_stmt_list(
    stmts: &[BoundStmt],
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    loop_exit_stack: &mut Vec<Vec<usize>>,
    call_patches: &mut Vec<(usize, String)>,
    error_handler_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    current_proc_name: &str,
    proc_labels: &mut HashMap<String, usize>,
) {
    for stmt in stmts {
        emit_stmt(
            stmt,
            compare_mode,
            slot_map,
            temps,
            instructions,
            loop_exit_stack,
            call_patches,
            error_handler_patches,
            proc_meta,
            current_proc_name,
            proc_labels,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_stmt(
    stmt: &BoundStmt,
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    loop_exit_stack: &mut Vec<Vec<usize>>,
    call_patches: &mut Vec<(usize, String)>,
    error_handler_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    current_proc_name: &str,
    proc_labels: &mut HashMap<String, usize>,
) {
    match stmt {
        BoundStmt::Assign { target, expr } => {
            if let Some(target_slot) = slot_map.get(target.as_str()).copied() {
                emit_expr_into(
                    expr,
                    compare_mode,
                    target_slot,
                    slot_map,
                    temps,
                    instructions,
                );
            }
        }
        BoundStmt::MidAssign {
            target,
            start,
            count,
            value,
        } => {
            if let Some(target_slot) = slot_map.get(target.as_str()).copied() {
                let start_slot = temps.alloc_temp();
                emit_expr_into(
                    start,
                    compare_mode,
                    start_slot,
                    slot_map,
                    temps,
                    instructions,
                );
                let count_slot = if let Some(count) = count {
                    let slot = temps.alloc_temp();
                    emit_expr_into(count, compare_mode, slot, slot_map, temps, instructions);
                    Some(slot)
                } else {
                    None
                };
                let value_slot = temps.alloc_temp();
                emit_expr_into(
                    value,
                    compare_mode,
                    value_slot,
                    slot_map,
                    temps,
                    instructions,
                );
                instructions.push(Instruction::IntrinsicMidStmtDigits {
                    target: target_slot,
                    start: start_slot,
                    count: count_slot,
                    value: value_slot,
                });
            }
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            let cond_slot = temps.alloc_temp();
            emit_cond_into(cond, compare_mode, cond_slot, slot_map, temps, instructions);
            let jump_patch = instructions.len();
            instructions.push(Instruction::JumpIfZero {
                cond_slot,
                target_pc: 0,
            });
            emit_stmt_list(
                then_body,
                compare_mode,
                slot_map,
                temps,
                instructions,
                loop_exit_stack,
                call_patches,
                error_handler_patches,
                proc_meta,
                current_proc_name,
                proc_labels,
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
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    loop_exit_stack,
                    call_patches,
                    error_handler_patches,
                    proc_meta,
                    current_proc_name,
                    proc_labels,
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
                emit_expr_into(start, compare_mode, var_slot, slot_map, temps, instructions);
                let end_slot = temps.alloc_temp();
                let cond_slot = temps.alloc_temp();
                emit_expr_into(end, compare_mode, end_slot, slot_map, temps, instructions);

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
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    loop_exit_stack,
                    call_patches,
                    error_handler_patches,
                    proc_meta,
                    current_proc_name,
                    proc_labels,
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
        BoundStmt::ReDim {
            name,
            bounds,
            previous_bounds,
            preserve,
        } => {
            if !preserve {
                reset_array_slots(name, slot_map, instructions);
            } else if let Some(prev) = previous_bounds {
                if let (Some(old_count), Some(new_count)) =
                    (array_element_count(prev), array_element_count(bounds))
                {
                    let overlap = old_count.min(new_count);
                    let tail = old_count.max(new_count);
                    if overlap < tail {
                        reset_array_slots_range(name, overlap, tail, slot_map, instructions);
                    }
                }
            } else {
                reset_array_slots(name, slot_map, instructions);
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
                emit_cond_into(cond, compare_mode, cond_slot, slot_map, temps, instructions);
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
                compare_mode,
                slot_map,
                temps,
                instructions,
                loop_exit_stack,
                call_patches,
                error_handler_patches,
                proc_meta,
                current_proc_name,
                proc_labels,
            );

            emit_cond_into(cond, compare_mode, cond_slot, slot_map, temps, instructions);
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
        BoundStmt::OnErrorGotoLabel { label } => {
            let patch_idx = instructions.len();
            instructions.push(Instruction::SetOnErrorGotoLabel { target_pc: 0 });
            error_handler_patches
                .push((patch_idx, format!("__label::{current_proc_name}::{label}")));
        }
        BoundStmt::ResumeNext => {
            instructions.push(Instruction::ResumeNext);
        }
        BoundStmt::RaiseError(code) => {
            instructions.push(Instruction::RaiseError { code: *code });
        }
        BoundStmt::Label { name } => {
            proc_labels.insert(
                format!("__label::{current_proc_name}::{name}"),
                instructions.len(),
            );
        }
        BoundStmt::GoSub { label } => {
            let patch_idx = instructions.len();
            instructions.push(Instruction::CallProc { target_pc: 0 });
            call_patches.push((patch_idx, format!("__label::{current_proc_name}::{label}")));
        }
        BoundStmt::Return => {
            instructions.push(Instruction::Return);
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            let expr_slot = temps.alloc_temp();
            emit_expr_into(expr, compare_mode, expr_slot, slot_map, temps, instructions);
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
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    loop_exit_stack,
                    call_patches,
                    error_handler_patches,
                    proc_meta,
                    current_proc_name,
                    proc_labels,
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
                compare_mode,
                slot_map,
                temps,
                instructions,
                loop_exit_stack,
                call_patches,
                error_handler_patches,
                proc_meta,
                current_proc_name,
                proc_labels,
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
                let mapped_args = map_call_args_for_emit(args, &meta.params);
                let param_array_idx = meta.params.iter().position(|p| p.param_array);
                for (idx, param) in meta.params.iter().enumerate() {
                    let Some(param_slot) = meta.slots.get(param.name.as_str()).copied() else {
                        continue;
                    };
                    if param.param_array {
                        let extras_start = param_array_idx.unwrap_or(meta.params.len());
                        let extras_count = args.len().saturating_sub(extras_start);
                        instructions.push(Instruction::LoadConstI32 {
                            slot: param_slot,
                            value: ARRAY_TAG_BASE + extras_count as i32,
                        });
                        continue;
                    }
                    let Some(arg) = mapped_args.get(idx).and_then(|arg| *arg) else {
                        if param.optional {
                            emit_optional_default(param, param_slot, instructions);
                        }
                        continue;
                    };

                    if param.by_ref
                        && let BoundExpr::Var(var_name) = &arg.expr
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

                    emit_expr_into(
                        &arg.expr,
                        compare_mode,
                        param_slot,
                        slot_map,
                        temps,
                        instructions,
                    );
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
    compare_mode: StringCompareMode,
    dst: usize,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
) {
    match cond {
        BoundCond::Compare { op, lhs, rhs } => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_expr_into(lhs, compare_mode, lhs_slot, slot_map, temps, instructions);
            emit_expr_into(rhs, compare_mode, rhs_slot, slot_map, temps, instructions);
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
                CompareOp::Like => instructions.push(Instruction::IntrinsicLikeDigits {
                    dst,
                    lhs: lhs_slot,
                    pattern: rhs_slot,
                    mode: compare_mode,
                }),
            }
        }
        BoundCond::Truthy(expr) => {
            let expr_slot = temps.alloc_temp();
            let zero_slot = temps.alloc_temp();
            emit_expr_into(expr, compare_mode, expr_slot, slot_map, temps, instructions);
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
            emit_cond_into(
                inner,
                compare_mode,
                inner_slot,
                slot_map,
                temps,
                instructions,
            );
            instructions.push(Instruction::BoolNot {
                dst,
                src: inner_slot,
            });
        }
        BoundCond::And(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(lhs, compare_mode, lhs_slot, slot_map, temps, instructions);
            emit_cond_into(rhs, compare_mode, rhs_slot, slot_map, temps, instructions);
            instructions.push(Instruction::BoolAnd {
                dst,
                lhs: lhs_slot,
                rhs: rhs_slot,
            });
        }
        BoundCond::Or(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(lhs, compare_mode, lhs_slot, slot_map, temps, instructions);
            emit_cond_into(rhs, compare_mode, rhs_slot, slot_map, temps, instructions);
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
    compare_mode: StringCompareMode,
    dst: usize,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
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
        BoundExpr::IntrinsicCall { name, args } => {
            let mut arg_slots = Vec::with_capacity(args.len());
            for arg in args {
                let slot = temps.alloc_temp();
                emit_expr_into(arg, compare_mode, slot, slot_map, temps, instructions);
                arg_slots.push(slot);
            }

            match (name.as_str(), arg_slots.as_slice()) {
                ("vbnullstring", []) => instructions.push(Instruction::LoadConstI32 {
                    slot: dst,
                    value: 0,
                }),
                ("len", [src]) => {
                    instructions.push(Instruction::IntrinsicLenDigits { dst, src: *src })
                }
                ("left", [src, count]) => instructions.push(Instruction::IntrinsicLeftDigits {
                    dst,
                    src: *src,
                    count: *count,
                }),
                ("right", [src, count]) => instructions.push(Instruction::IntrinsicRightDigits {
                    dst,
                    src: *src,
                    count: *count,
                }),
                ("mid", [src, start]) => instructions.push(Instruction::IntrinsicMidDigits {
                    dst,
                    src: *src,
                    start: *start,
                    count: None,
                }),
                ("mid", [src, start, count]) => {
                    instructions.push(Instruction::IntrinsicMidDigits {
                        dst,
                        src: *src,
                        start: *start,
                        count: Some(*count),
                    })
                }
                ("instr", [haystack, needle]) => {
                    instructions.push(Instruction::IntrinsicInStrDigits {
                        dst,
                        haystack: *haystack,
                        needle: *needle,
                        mode: compare_mode,
                    })
                }
                ("instrrev", [haystack, needle]) => {
                    instructions.push(Instruction::IntrinsicInStrRevDigits {
                        dst,
                        haystack: *haystack,
                        needle: *needle,
                        mode: compare_mode,
                    })
                }
                ("lcase", [src]) => {
                    instructions.push(Instruction::IntrinsicLowerDigits { dst, src: *src })
                }
                ("ucase", [src]) => {
                    instructions.push(Instruction::IntrinsicUpperDigits { dst, src: *src })
                }
                ("split", [src, delimiter]) => {
                    instructions.push(Instruction::IntrinsicSplitCountDigits {
                        dst,
                        src: *src,
                        delimiter: *delimiter,
                    })
                }
                ("join", [src, delimiter]) => instructions.push(Instruction::IntrinsicJoinDigits {
                    dst,
                    src: *src,
                    delimiter: *delimiter,
                }),
                ("replace", [src, find, replace]) => {
                    instructions.push(Instruction::IntrinsicReplaceDigits {
                        dst,
                        src: *src,
                        find: *find,
                        replace: *replace,
                    })
                }
                ("trim", [src]) => {
                    instructions.push(Instruction::IntrinsicTrimDigits { dst, src: *src })
                }
                ("ltrim", [src]) => {
                    instructions.push(Instruction::IntrinsicLTrimDigits { dst, src: *src })
                }
                ("rtrim", [src]) => {
                    instructions.push(Instruction::IntrinsicRTrimDigits { dst, src: *src })
                }
                ("strcomp", [lhs, rhs]) => instructions.push(Instruction::IntrinsicStrCompDigits {
                    dst,
                    lhs: *lhs,
                    rhs: *rhs,
                    mode: compare_mode,
                }),
                ("dateserial", [year, month, day]) => {
                    instructions.push(Instruction::IntrinsicDateSerialDigits {
                        dst,
                        year: *year,
                        month: *month,
                        day: *day,
                    })
                }
                ("timeserial", [hour, minute, second]) => {
                    instructions.push(Instruction::IntrinsicTimeSerialDigits {
                        dst,
                        hour: *hour,
                        minute: *minute,
                        second: *second,
                    })
                }
                ("datevalue", [src]) => {
                    instructions.push(Instruction::IntrinsicDateValueDigits { dst, src: *src })
                }
                ("timevalue", [src]) => {
                    instructions.push(Instruction::IntrinsicTimeValueDigits { dst, src: *src })
                }
                ("dateadd", [interval, number, date]) => {
                    instructions.push(Instruction::IntrinsicDateAddDigits {
                        dst,
                        interval: *interval,
                        number: *number,
                        date: *date,
                    })
                }
                ("datediff", [interval, date1, date2]) => {
                    instructions.push(Instruction::IntrinsicDateDiffDigits {
                        dst,
                        interval: *interval,
                        date1: *date1,
                        date2: *date2,
                    })
                }
                ("abs", [src]) => {
                    instructions.push(Instruction::IntrinsicAbsI32 { dst, src: *src })
                }
                ("int", [src]) => {
                    instructions.push(Instruction::IntrinsicIntI32 { dst, src: *src })
                }
                ("fix", [src]) => {
                    instructions.push(Instruction::IntrinsicFixI32 { dst, src: *src })
                }
                ("sgn", [src]) => {
                    instructions.push(Instruction::IntrinsicSgnI32 { dst, src: *src })
                }
                ("round", [src, digits]) => instructions.push(Instruction::IntrinsicRoundI32 {
                    dst,
                    src: *src,
                    digits: Some(*digits),
                }),
                ("round", [src]) => instructions.push(Instruction::IntrinsicRoundI32 {
                    dst,
                    src: *src,
                    digits: None,
                }),
                ("sqr", [src]) => {
                    instructions.push(Instruction::IntrinsicSqrI32 { dst, src: *src })
                }
                ("sin", [src]) => {
                    instructions.push(Instruction::IntrinsicSinI32 { dst, src: *src })
                }
                ("cos", [src]) => {
                    instructions.push(Instruction::IntrinsicCosI32 { dst, src: *src })
                }
                ("log", [src]) => {
                    instructions.push(Instruction::IntrinsicLogI32 { dst, src: *src })
                }
                ("exp", [src]) => {
                    instructions.push(Instruction::IntrinsicExpI32 { dst, src: *src })
                }
                ("fv", [rate, nper, pmt]) => instructions.push(Instruction::IntrinsicFvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    pv: None,
                    due: None,
                }),
                ("fv", [rate, nper, pmt, pv]) => instructions.push(Instruction::IntrinsicFvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    pv: Some(*pv),
                    due: None,
                }),
                ("fv", [rate, nper, pmt, pv, due]) => {
                    instructions.push(Instruction::IntrinsicFvI32 {
                        dst,
                        rate: *rate,
                        nper: *nper,
                        pmt: *pmt,
                        pv: Some(*pv),
                        due: Some(*due),
                    })
                }
                ("pv", [rate, nper, pmt]) => instructions.push(Instruction::IntrinsicPvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    fv: None,
                    due: None,
                }),
                ("pv", [rate, nper, pmt, fv]) => instructions.push(Instruction::IntrinsicPvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    fv: Some(*fv),
                    due: None,
                }),
                ("pv", [rate, nper, pmt, fv, due]) => {
                    instructions.push(Instruction::IntrinsicPvI32 {
                        dst,
                        rate: *rate,
                        nper: *nper,
                        pmt: *pmt,
                        fv: Some(*fv),
                        due: Some(*due),
                    })
                }
                ("pmt", [rate, nper, pv]) => instructions.push(Instruction::IntrinsicPmtI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pv: *pv,
                    fv: None,
                    due: None,
                }),
                ("pmt", [rate, nper, pv, fv]) => instructions.push(Instruction::IntrinsicPmtI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pv: *pv,
                    fv: Some(*fv),
                    due: None,
                }),
                ("pmt", [rate, nper, pv, fv, due]) => {
                    instructions.push(Instruction::IntrinsicPmtI32 {
                        dst,
                        rate: *rate,
                        nper: *nper,
                        pv: *pv,
                        fv: Some(*fv),
                        due: Some(*due),
                    })
                }
                ("array", args) => {
                    instructions.push(Instruction::LoadConstI32 {
                        slot: dst,
                        value: ARRAY_TAG_BASE + args.len() as i32,
                    });
                }
                ("lbound", [src]) => {
                    instructions.push(Instruction::IntrinsicLBoundArray { dst, src: *src })
                }
                ("ubound", [src]) => {
                    instructions.push(Instruction::IntrinsicUBoundArray { dst, src: *src })
                }
                ("isarray", [src]) => {
                    instructions.push(Instruction::IntrinsicIsArrayTag { dst, src: *src })
                }
                ("vartype", [src]) => {
                    instructions.push(Instruction::IntrinsicVarTypeTag { dst, src: *src })
                }
                ("typename", [src]) => {
                    instructions.push(Instruction::IntrinsicTypeNameTag { dst, src: *src })
                }
                ("isnumeric", [src]) => {
                    instructions.push(Instruction::IntrinsicIsNumericTag { dst, src: *src })
                }
                ("isdate", [src]) => {
                    instructions.push(Instruction::IntrinsicIsDateTag { dst, src: *src })
                }
                ("isobject", [src]) => {
                    instructions.push(Instruction::IntrinsicIsObjectTag { dst, src: *src })
                }
                ("shell", [command]) => instructions.push(Instruction::IntrinsicShellHost {
                    dst,
                    command: *command,
                }),
                ("environ", [key]) => {
                    instructions.push(Instruction::IntrinsicEnvironHost { dst, key: *key })
                }
                ("dir", [path]) => {
                    instructions.push(Instruction::IntrinsicDirHost { dst, path: *path })
                }
                ("collectionadd", [count, item]) => {
                    instructions.push(Instruction::IntrinsicCollectionAdd {
                        dst,
                        count: *count,
                        item: *item,
                    })
                }
                ("collectionadd", [count, item, _key]) => {
                    instructions.push(Instruction::IntrinsicCollectionAdd {
                        dst,
                        count: *count,
                        item: *item,
                    })
                }
                ("collectionitem", [count, index]) => {
                    instructions.push(Instruction::IntrinsicCollectionItem {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectionitem", [count, index, _missing]) => {
                    instructions.push(Instruction::IntrinsicCollectionItem {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectionremove", [count, index]) => {
                    instructions.push(Instruction::IntrinsicCollectionRemove {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectionremove", [count, index, _missing]) => {
                    instructions.push(Instruction::IntrinsicCollectionRemove {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectioncount", [count]) => {
                    instructions.push(Instruction::IntrinsicCollectionCount { dst, count: *count })
                }
                ("createobject", [prog_id]) => {
                    instructions.push(Instruction::IntrinsicCreateObjectHost {
                        dst,
                        prog_id: *prog_id,
                    })
                }
                ("dispatchinvoke", [object, member, arg]) => {
                    instructions.push(Instruction::IntrinsicDispatchInvokeHost {
                        dst,
                        object: *object,
                        member: *member,
                        arg: *arg,
                    })
                }
                _ => {}
            }
        }
    }
}

fn emit_optional_default(param: &BoundParam, dst: usize, instructions: &mut Vec<Instruction>) {
    instructions.push(Instruction::LoadConstI32 {
        slot: dst,
        value: param.default_value.unwrap_or(0),
    });
}

fn map_call_args_for_emit<'a>(
    args: &'a [BoundCallArg],
    params: &[BoundParam],
) -> Vec<Option<&'a BoundCallArg>> {
    let param_array_idx = params.iter().position(|p| p.param_array);
    let fixed_len = param_array_idx.unwrap_or(params.len());
    let mut mapped: Vec<Option<&BoundCallArg>> = vec![None; params.len()];
    let mut next_pos = 0usize;
    let mut seen_named = false;

    for arg in args {
        if param_array_idx.is_some() && arg.name.is_some() {
            continue;
        }
        if let Some(name) = &arg.name {
            seen_named = true;
            if let Some(idx) = params
                .iter()
                .position(|p| p.name.eq_ignore_ascii_case(name))
                && mapped[idx].is_none()
            {
                mapped[idx] = Some(arg);
            }
            continue;
        }

        if seen_named {
            continue;
        }

        while next_pos < params.len() && mapped[next_pos].is_some() {
            next_pos += 1;
        }
        if next_pos < fixed_len {
            mapped[next_pos] = Some(arg);
            next_pos += 1;
        }
    }

    mapped
}

fn reset_array_slots(
    array_name: &str,
    slot_map: &HashMap<String, usize>,
    instructions: &mut Vec<Instruction>,
) {
    let prefix = format!("{array_name}_");
    let mut slots = slot_map
        .iter()
        .filter_map(|(name, slot)| {
            if name.starts_with(&prefix) {
                Some(*slot)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    slots.sort_unstable();
    for slot in slots {
        instructions.push(Instruction::LoadConstI32 { slot, value: 0 });
    }
}

fn reset_array_slots_range(
    array_name: &str,
    start_index: usize,
    end_index_exclusive: usize,
    slot_map: &HashMap<String, usize>,
    instructions: &mut Vec<Instruction>,
) {
    if start_index >= end_index_exclusive {
        return;
    }
    let prefix = format!("{array_name}_");
    let mut slots = slot_map
        .iter()
        .filter_map(|(name, slot)| {
            if !name.starts_with(&prefix) {
                return None;
            }
            let suffix = &name[prefix.len()..];
            let index = suffix.parse::<usize>().ok()?;
            if (start_index..end_index_exclusive).contains(&index) {
                Some(*slot)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    slots.sort_unstable();
    for slot in slots {
        instructions.push(Instruction::LoadConstI32 { slot, value: 0 });
    }
}

fn array_element_count(bounds: &[(i32, i32)]) -> Option<usize> {
    let mut total = 1usize;
    for (lower, upper) in bounds {
        if upper < lower {
            return None;
        }
        let width = (*upper as i64 - *lower as i64 + 1) as usize;
        total = total.checked_mul(width)?;
    }
    Some(total)
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
