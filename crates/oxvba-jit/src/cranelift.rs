use std::collections::HashMap;
use std::convert::TryFrom;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{AbiParam, InstBuilder, MemFlags, types};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use oxvba_compiler::{Bytecode, Instruction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentTerminal {
    Halt,
    Return,
}

fn slot_offset(slot: usize) -> Result<i32, String> {
    let slot_i32 = i32::try_from(slot).map_err(|_| format!("slot index out of range: {slot}"))?;
    slot_i32
        .checked_mul(4)
        .ok_or_else(|| format!("slot offset overflow: {slot}"))
}

fn supports_core_instruction(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::LoadConstI32 { .. }
            | Instruction::AddConstI32 { .. }
            | Instruction::AddSlots { .. }
            | Instruction::SubConstI32 { .. }
            | Instruction::CopySlot { .. }
            | Instruction::IntrinsicAbsI32 { .. }
            | Instruction::IntrinsicIntI32 { .. }
            | Instruction::IntrinsicFixI32 { .. }
            | Instruction::IntrinsicSgnI32 { .. }
            | Instruction::CmpEqSlots { .. }
            | Instruction::CmpNeSlots { .. }
            | Instruction::CmpLtSlots { .. }
            | Instruction::CmpLeSlots { .. }
            | Instruction::CmpGtSlots { .. }
            | Instruction::CmpGeSlots { .. }
            | Instruction::BoolNot { .. }
            | Instruction::BoolAnd { .. }
            | Instruction::BoolOr { .. }
            | Instruction::ClearErr
            | Instruction::JumpIfZero { .. }
            | Instruction::Jump { .. }
            | Instruction::IncSlot { .. }
            | Instruction::Halt
    )
}

fn supports_core(bytecode: &Bytecode) -> bool {
    let len = bytecode.instructions.len();
    if len == 0 {
        return true;
    }

    for instruction in &bytecode.instructions {
        if !supports_core_instruction(instruction) {
            return false;
        }
        match instruction {
            Instruction::Jump { target_pc } | Instruction::JumpIfZero { target_pc, .. } => {
                if *target_pc > len {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn find_first_halt(instructions: &[Instruction]) -> Option<usize> {
    instructions
        .iter()
        .position(|inst| matches!(inst, Instruction::Halt))
}

fn find_first_return(instructions: &[Instruction], start_pc: usize) -> Option<usize> {
    let tail = instructions.get(start_pc..)?;
    let rel = tail
        .iter()
        .position(|inst| matches!(inst, Instruction::Return))?;
    Some(start_pc + rel)
}

fn inline_segment(
    instructions: &[Instruction],
    start_pc: usize,
    end_pc: usize,
    terminal: SegmentTerminal,
    call_stack: &mut Vec<usize>,
) -> Result<Vec<Instruction>, String> {
    if end_pc >= instructions.len() || start_pc > end_pc {
        return Err("invalid bytecode segment bounds".to_string());
    }

    match (terminal, &instructions[end_pc]) {
        (SegmentTerminal::Halt, Instruction::Halt) => {}
        (SegmentTerminal::Return, Instruction::Return) => {}
        _ => return Err("segment terminal marker mismatch".to_string()),
    }

    if call_stack.contains(&start_pc) {
        return Err(format!(
            "recursive call is not supported by cranelift jit subset (pc={start_pc})"
        ));
    }
    call_stack.push(start_pc);

    let mut out = Vec::new();
    let mut mapping: HashMap<usize, usize> = HashMap::new();
    let mut jump_patches: Vec<(usize, usize)> = Vec::new();

    for old_pc in start_pc..=end_pc {
        mapping.insert(old_pc, out.len());
        let inst = &instructions[old_pc];

        if old_pc == end_pc {
            if terminal == SegmentTerminal::Halt {
                out.push(Instruction::Halt);
            }
            continue;
        }

        match inst {
            Instruction::CallProc { target_pc } => {
                let proc_end = find_first_return(instructions, *target_pc).ok_or_else(|| {
                    format!("cannot locate callee return for call target {target_pc}")
                })?;
                let nested = inline_segment(
                    instructions,
                    *target_pc,
                    proc_end,
                    SegmentTerminal::Return,
                    call_stack,
                )?;
                out.extend(nested);
            }
            Instruction::Jump { target_pc } => {
                let patch_idx = out.len();
                out.push(Instruction::Jump { target_pc: 0 });
                jump_patches.push((patch_idx, *target_pc));
            }
            Instruction::JumpIfZero {
                cond_slot,
                target_pc,
            } => {
                let patch_idx = out.len();
                out.push(Instruction::JumpIfZero {
                    cond_slot: *cond_slot,
                    target_pc: 0,
                });
                jump_patches.push((patch_idx, *target_pc));
            }
            Instruction::Return | Instruction::Halt => {
                return Err("unexpected terminal inside inlined segment".to_string());
            }
            other => out.push(other.clone()),
        }
    }

    let segment_exit = out.len();
    let end_plus_one = end_pc.checked_add(1);
    for (patch_idx, old_target) in jump_patches {
        let new_target = if let Some(mapped) = mapping.get(&old_target) {
            *mapped
        } else if end_plus_one == Some(old_target) {
            segment_exit
        } else {
            return Err(format!(
                "cross-segment jump is not supported by cranelift jit subset (target={old_target})"
            ));
        };

        match &mut out[patch_idx] {
            Instruction::Jump { target_pc } => *target_pc = new_target,
            Instruction::JumpIfZero { target_pc, .. } => *target_pc = new_target,
            _ => return Err("invalid jump patch target".to_string()),
        }
    }

    call_stack.pop();
    Ok(out)
}

fn inline_bytecode(bytecode: &Bytecode) -> Result<Bytecode, String> {
    let Some(main_end) = find_first_halt(&bytecode.instructions) else {
        return Err("bytecode entry procedure missing halt".to_string());
    };

    let mut call_stack = Vec::new();
    let instructions = inline_segment(
        &bytecode.instructions,
        0,
        main_end,
        SegmentTerminal::Halt,
        &mut call_stack,
    )?;

    Ok(Bytecode {
        instructions,
        external_call_descriptors: bytecode.external_call_descriptors.clone(),
        slot_count: bytecode.slot_count,
        user_slot_count: bytecode.user_slot_count,
    })
}

pub fn supports_bytecode(bytecode: &Bytecode) -> bool {
    let Ok(inlined) = inline_bytecode(bytecode) else {
        return false;
    };
    supports_core(&inlined)
}

pub fn execute_bytecode(bytecode: &Bytecode) -> Result<Vec<i32>, String> {
    let inlined = inline_bytecode(bytecode)?;
    if !supports_core(&inlined) {
        return Err("unsupported bytecode for cranelift execution".to_string());
    }

    let jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| format!("jit builder error: {e}"))?;
    let mut module = JITModule::new(jit_builder);

    let mut context = module.make_context();
    let ptr_ty = module.target_config().pointer_type();
    context.func.signature.params.push(AbiParam::new(ptr_ty));
    context
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I32));

    let function_id = module
        .declare_function("oxvba_jit_run", Linkage::Local, &context.func.signature)
        .map_err(|e| format!("declare function error: {e}"))?;

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_ctx);

    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    let exit_block = builder.create_block();

    let mut pc_blocks = Vec::with_capacity(inlined.instructions.len());
    for _ in 0..inlined.instructions.len() {
        pc_blocks.push(builder.create_block());
    }

    let slots_var = builder.declare_var(ptr_ty);

    builder.switch_to_block(entry_block);
    let slots_ptr = builder.block_params(entry_block)[0];
    builder.def_var(slots_var, slots_ptr);
    if let Some(first_block) = pc_blocks.first() {
        builder.ins().jump(*first_block, &[]);
    } else {
        builder.ins().jump(exit_block, &[]);
    }

    for (pc, instruction) in inlined.instructions.iter().enumerate() {
        builder.switch_to_block(pc_blocks[pc]);
        let slots_ptr = builder.use_var(slots_var);

        let read_slot = |builder: &mut FunctionBuilder,
                         slots_ptr,
                         slot: usize|
         -> Result<cranelift_codegen::ir::Value, String> {
            let offset = i64::from(slot_offset(slot)?);
            let addr = builder.ins().iadd_imm(slots_ptr, offset);
            Ok(builder.ins().load(types::I32, MemFlags::new(), addr, 0))
        };

        let write_slot = |builder: &mut FunctionBuilder,
                          slots_ptr,
                          slot: usize,
                          value: cranelift_codegen::ir::Value|
         -> Result<(), String> {
            let offset = i64::from(slot_offset(slot)?);
            let addr = builder.ins().iadd_imm(slots_ptr, offset);
            builder.ins().store(MemFlags::new(), value, addr, 0);
            Ok(())
        };

        let bool_to_i32 = |builder: &mut FunctionBuilder, cond: cranelift_codegen::ir::Value| {
            let one = builder.ins().iconst(types::I32, 1);
            let zero = builder.ins().iconst(types::I32, 0);
            builder.ins().select(cond, one, zero)
        };

        let next_block = if pc + 1 < pc_blocks.len() {
            pc_blocks[pc + 1]
        } else {
            exit_block
        };

        match instruction {
            Instruction::LoadConstI32 { slot, value } => {
                let out = builder.ins().iconst(types::I32, i64::from(*value));
                write_slot(&mut builder, slots_ptr, *slot, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::AddConstI32 { slot, value } => {
                let lhs = read_slot(&mut builder, slots_ptr, *slot)?;
                let rhs = builder.ins().iconst(types::I32, i64::from(*value));
                let out = builder.ins().iadd(lhs, rhs);
                write_slot(&mut builder, slots_ptr, *slot, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::AddSlots { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let out = builder.ins().iadd(lhs, rhs);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::SubConstI32 { slot, value } => {
                let lhs = read_slot(&mut builder, slots_ptr, *slot)?;
                let rhs = builder.ins().iconst(types::I32, i64::from(*value));
                let out = builder.ins().isub(lhs, rhs);
                write_slot(&mut builder, slots_ptr, *slot, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CopySlot { dst, src } => {
                let value = read_slot(&mut builder, slots_ptr, *src)?;
                write_slot(&mut builder, slots_ptr, *dst, value)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::IntrinsicAbsI32 { dst, src } => {
                let value = read_slot(&mut builder, slots_ptr, *src)?;
                let is_neg = builder.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
                let negated = builder.ins().ineg(value);
                let abs_value = builder.ins().select(is_neg, negated, value);
                let is_min = builder
                    .ins()
                    .icmp_imm(IntCC::Equal, value, i64::from(i32::MIN));
                let max_i32 = builder.ins().iconst(types::I32, i64::from(i32::MAX));
                let saturated = builder.ins().select(is_min, max_i32, abs_value);
                write_slot(&mut builder, slots_ptr, *dst, saturated)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::IntrinsicIntI32 { dst, src }
            | Instruction::IntrinsicFixI32 { dst, src } => {
                let value = read_slot(&mut builder, slots_ptr, *src)?;
                write_slot(&mut builder, slots_ptr, *dst, value)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::IntrinsicSgnI32 { dst, src } => {
                let value = read_slot(&mut builder, slots_ptr, *src)?;
                let is_positive = builder.ins().icmp_imm(IntCC::SignedGreaterThan, value, 0);
                let is_negative = builder.ins().icmp_imm(IntCC::SignedLessThan, value, 0);
                let one = builder.ins().iconst(types::I32, 1);
                let minus_one = builder.ins().iconst(types::I32, -1);
                let zero = builder.ins().iconst(types::I32, 0);
                let pos_or_zero = builder.ins().select(is_positive, one, zero);
                let signed = builder.ins().select(is_negative, minus_one, pos_or_zero);
                write_slot(&mut builder, slots_ptr, *dst, signed)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpEqSlots { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::Equal, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpNeSlots { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::NotEqual, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpLtSlots { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::SignedLessThan, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpLeSlots { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::SignedLessThanOrEqual, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpGtSlots { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::SignedGreaterThan, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpGeSlots { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThanOrEqual, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::BoolNot { dst, src } => {
                let value = read_slot(&mut builder, slots_ptr, *src)?;
                let is_zero = builder.ins().icmp_imm(IntCC::Equal, value, 0);
                let out = bool_to_i32(&mut builder, is_zero);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::BoolAnd { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let lhs_truth = builder.ins().icmp_imm(IntCC::NotEqual, lhs, 0);
                let rhs_truth = builder.ins().icmp_imm(IntCC::NotEqual, rhs, 0);
                let out_truth = builder.ins().band(lhs_truth, rhs_truth);
                let out = bool_to_i32(&mut builder, out_truth);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::BoolOr { dst, lhs, rhs } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let lhs_truth = builder.ins().icmp_imm(IntCC::NotEqual, lhs, 0);
                let rhs_truth = builder.ins().icmp_imm(IntCC::NotEqual, rhs, 0);
                let out_truth = builder.ins().bor(lhs_truth, rhs_truth);
                let out = bool_to_i32(&mut builder, out_truth);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::JumpIfZero {
                cond_slot,
                target_pc,
            } => {
                let cond = read_slot(&mut builder, slots_ptr, *cond_slot)?;
                let is_zero = builder.ins().icmp_imm(IntCC::Equal, cond, 0);
                let true_block = if *target_pc < pc_blocks.len() {
                    pc_blocks[*target_pc]
                } else {
                    exit_block
                };
                builder
                    .ins()
                    .brif(is_zero, true_block, &[], next_block, &[]);
            }
            Instruction::Jump { target_pc } => {
                let jump_block = if *target_pc < pc_blocks.len() {
                    pc_blocks[*target_pc]
                } else {
                    exit_block
                };
                builder.ins().jump(jump_block, &[]);
            }
            Instruction::IncSlot { slot } => {
                let value = read_slot(&mut builder, slots_ptr, *slot)?;
                let out = builder.ins().iadd_imm(value, 1);
                write_slot(&mut builder, slots_ptr, *slot, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::ClearErr => {
                builder.ins().jump(next_block, &[]);
            }
            Instruction::Halt => {
                builder.ins().jump(exit_block, &[]);
            }
            _ => {
                return Err("unsupported bytecode for cranelift execution".to_string());
            }
        }
    }

    builder.switch_to_block(exit_block);
    let ok = builder.ins().iconst(types::I32, 0);
    builder.ins().return_(&[ok]);
    builder.seal_all_blocks();
    builder.finalize();

    module
        .define_function(function_id, &mut context)
        .map_err(|e| format!("define function error: {e}"))?;
    module.clear_context(&mut context);
    module.finalize_definitions().map_err(|e| format!("{e}"))?;

    let code_ptr = module.get_finalized_function(function_id);
    type JitFn = unsafe extern "C" fn(*mut i32) -> i32;
    let jit_fn: JitFn = unsafe { std::mem::transmute(code_ptr) };

    let total_slots = inlined.slot_count.max(inlined.user_slot_count);
    let storage_len = total_slots.max(1);
    let mut storage = vec![0_i32; storage_len];
    let rc = unsafe { jit_fn(storage.as_mut_ptr()) };
    if rc != 0 {
        return Err(format!("jit returned non-zero status: {rc}"));
    }

    Ok(storage[..inlined.user_slot_count].to_vec())
}
