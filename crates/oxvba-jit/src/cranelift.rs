#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{AbiParam, BlockArg, InstBuilder, MemFlags, types};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use oxvba_compiler::bytecode::StringCompareMode;
use oxvba_compiler::{Bytecode, Instruction};
use oxvba_hal::traits::HostServices;
use oxvba_runtime::{RuntimeValue, Variant};

use crate::jit_context::JitContextOwned;
use crate::runtime_helpers;
use crate::slot_abi::{SLOT_PAYLOAD_OFFSET, SLOT_SIZE, SLOT_VTYPE_OFFSET, VT_I4};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentTerminal {
    Halt,
    Return,
}

// ── Legacy path helpers (4-byte i32 slots) ────────────────────────────

fn slot_offset(slot: usize) -> Result<i32, String> {
    let slot_i32 = i32::try_from(slot).map_err(|_| format!("slot index out of range: {slot}"))?;
    slot_i32
        .checked_mul(4)
        .ok_or_else(|| format!("slot offset overflow: {slot}"))
}

// ── RtSlot path helpers (16-byte VARIANT slots) ───────────────────────

fn rtslot_vtype_offset(slot: usize) -> Result<i64, String> {
    let slot_i32 = i32::try_from(slot).map_err(|_| format!("slot index out of range: {slot}"))?;
    let base = slot_i32
        .checked_mul(SLOT_SIZE)
        .ok_or_else(|| format!("rtslot offset overflow: {slot}"))?;
    Ok(i64::from(base + SLOT_VTYPE_OFFSET))
}

fn rtslot_payload_offset(slot: usize) -> Result<i64, String> {
    let slot_i32 = i32::try_from(slot).map_err(|_| format!("slot index out of range: {slot}"))?;
    let base = slot_i32
        .checked_mul(SLOT_SIZE)
        .ok_or_else(|| format!("rtslot offset overflow: {slot}"))?;
    Ok(i64::from(base + SLOT_PAYLOAD_OFFSET))
}

// ── Instruction support gates ─────────────────────────────────────────

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

/// Check whether an instruction is supported by the RtSlot JIT path.
/// This is a superset of `supports_core_instruction` — it grows as we
/// implement more instruction handlers.
fn supports_rtslot_instruction(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        // Phase 0: same 23 as legacy
        Instruction::LoadConstI32 { .. }
            | Instruction::LoadConstBool { .. }
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
            // Phase 1 additions
            | Instruction::SubSlots { .. }
            | Instruction::MulSlots { .. }
            | Instruction::DivSlots { .. }
            | Instruction::IntDivSlots { .. }
            | Instruction::ModSlots { .. }
            | Instruction::PowSlots { .. }
            | Instruction::NegSlot { .. }
            | Instruction::ConcatSlots { .. }
            | Instruction::LoadConstF64 { .. }
            | Instruction::LoadConstString { .. }
            | Instruction::LoadNull { .. }
            // Phase 2: String ops
            | Instruction::IntrinsicLenDigits { .. }
            | Instruction::IntrinsicLeftDigits { .. }
            | Instruction::IntrinsicRightDigits { .. }
            | Instruction::IntrinsicMidDigits { .. }
            | Instruction::IntrinsicMidStmtDigits { .. }
            | Instruction::IntrinsicInStrDigits { .. }
            | Instruction::IntrinsicInStrRevDigits { .. }
            | Instruction::IntrinsicLowerDigits { .. }
            | Instruction::IntrinsicUpperDigits { .. }
            | Instruction::IntrinsicSplitCountDigits { .. }
            | Instruction::IntrinsicJoinDigits { .. }
            | Instruction::IntrinsicReplaceDigits { .. }
            | Instruction::IntrinsicTrimDigits { .. }
            | Instruction::IntrinsicLTrimDigits { .. }
            | Instruction::IntrinsicRTrimDigits { .. }
            | Instruction::IntrinsicStrCompDigits { .. }
            | Instruction::IntrinsicLikeDigits { .. }
            | Instruction::IntrinsicStrConvDigits { .. }
            // Phase 2: Char/format
            | Instruction::IntrinsicChrDigits { .. }
            | Instruction::IntrinsicAscDigits { .. }
            | Instruction::IntrinsicSpaceDigits { .. }
            | Instruction::IntrinsicStringRepeatDigits { .. }
            | Instruction::IntrinsicHexDigits { .. }
            | Instruction::IntrinsicOctDigits { .. }
            | Instruction::IntrinsicFormatDigits { .. }
            | Instruction::IntrinsicStrReverseDigits { .. }
            // Phase 2: Date/time
            | Instruction::IntrinsicDateSerialDigits { .. }
            | Instruction::IntrinsicTimeSerialDigits { .. }
            | Instruction::IntrinsicDateValueDigits { .. }
            | Instruction::IntrinsicTimeValueDigits { .. }
            | Instruction::IntrinsicDateAddDigits { .. }
            | Instruction::IntrinsicDateDiffDigits { .. }
            | Instruction::IntrinsicCDateValue { .. }
            | Instruction::IntrinsicYearDigits { .. }
            | Instruction::IntrinsicMonthDigits { .. }
            | Instruction::IntrinsicDayDigits { .. }
            | Instruction::IntrinsicWeekdayDigits { .. }
            | Instruction::IntrinsicMonthNameDigits { .. }
            // Phase 2: Math
            | Instruction::IntrinsicRoundI32 { .. }
            | Instruction::IntrinsicSqrI32 { .. }
            | Instruction::IntrinsicSinI32 { .. }
            | Instruction::IntrinsicCosI32 { .. }
            | Instruction::IntrinsicLogI32 { .. }
            | Instruction::IntrinsicExpI32 { .. }
            | Instruction::IntrinsicAtnI32 { .. }
            | Instruction::IntrinsicTanI32 { .. }
            // Phase 2: Type checking
            | Instruction::IntrinsicVarTypeTag { .. }
            | Instruction::IntrinsicVarType { .. }
            | Instruction::IntrinsicTypeNameTag { .. }
            | Instruction::IntrinsicStrPtr { .. }
            | Instruction::IntrinsicVarPtr { .. }
            | Instruction::IntrinsicVarPtrStringVar { .. }
            | Instruction::IntrinsicVarPtrVariantVar { .. }
            | Instruction::IntrinsicObjPtr { .. }
            | Instruction::IntrinsicIsNumericTag { .. }
            | Instruction::IntrinsicIsNumeric { .. }
            | Instruction::IntrinsicIsError { .. }
            | Instruction::IntrinsicIsDateTag { .. }
            | Instruction::IntrinsicIsObjectTag { .. }
            | Instruction::IntrinsicIsNull { .. }
            | Instruction::IntrinsicIsEmpty { .. }
            | Instruction::IntrinsicIsArrayTag { .. }
            // Phase 2: Financial
            | Instruction::IntrinsicFvI32 { .. }
            | Instruction::IntrinsicPvI32 { .. }
            | Instruction::IntrinsicPmtI32 { .. }
            | Instruction::IntrinsicNpvI32 { .. }
            | Instruction::IntrinsicIrrI32 { .. }
            | Instruction::IntrinsicMirrI32 { .. }
            | Instruction::IntrinsicRateI32 { .. }
            | Instruction::IntrinsicNPerI32 { .. }
            // Phase 2: Array
            | Instruction::IntrinsicArrayLiteral { .. }
            | Instruction::IntrinsicArrayAppend { .. }
            | Instruction::IntrinsicArrayResize { .. }
            | Instruction::IntrinsicArrayResizePreserve { .. }
            | Instruction::IntrinsicArrayGet { .. }
            | Instruction::IntrinsicArraySet { .. }
            | Instruction::IntrinsicLBoundArray { .. }
            | Instruction::IntrinsicUBoundArray { .. }
            // Phase 2: Collection
            | Instruction::IntrinsicCollectionAdd { .. }
            | Instruction::IntrinsicCollectionItem { .. }
            | Instruction::IntrinsicCollectionRemove { .. }
            | Instruction::IntrinsicCollectionCount { .. }
            // Phase 2: Random
            | Instruction::IntrinsicRndDigits { .. }
            | Instruction::IntrinsicRandomizeDigits { .. }
            // Phase 2: Assignment
            | Instruction::ValidateRuntimeAssignment { .. }
            // Phase 3: Error handling
            | Instruction::SetOnErrorResumeNext
            | Instruction::SetOnErrorGoto0
            | Instruction::SetOnErrorGotoLabel { .. }
            | Instruction::LoadErrNumber { .. }
            | Instruction::LoadErrDescription { .. }
            | Instruction::LoadErrSource { .. }
            | Instruction::RaiseError { .. }
            // Phase 4: Host services
            | Instruction::IntrinsicFreeFileHost { .. }
            | Instruction::IntrinsicFileOpenHost { .. }
            | Instruction::IntrinsicFileCloseHost { .. }
            | Instruction::IntrinsicFileKillHost { .. }
            | Instruction::IntrinsicFileReadHost { .. }
            | Instruction::IntrinsicFileWriteHost { .. }
            | Instruction::IntrinsicFilePrintHost { .. }
            | Instruction::IntrinsicConsolePrintHost { .. }
            | Instruction::IntrinsicFileInputHost { .. }
            | Instruction::IntrinsicConsoleInputHost { .. }
            | Instruction::IntrinsicFileLineInputHost { .. }
            | Instruction::IntrinsicConsoleLineInputHost { .. }
            | Instruction::IntrinsicFileEofHost { .. }
            | Instruction::IntrinsicFileLofHost { .. }
            | Instruction::IntrinsicFileSeekHost { .. }
            | Instruction::IntrinsicFileLocHost { .. }
            // COM dispatch
            | Instruction::IntrinsicCreateObjectHost { .. }
            | Instruction::IntrinsicDispatchInvokeHost { .. }
            // COM events
            | Instruction::IntrinsicComSubscribeEventHost { .. }
            | Instruction::IntrinsicComUnsubscribeEventHost { .. }
            | Instruction::IntrinsicComEventCallbackSubscriptionHost { .. }
            | Instruction::IntrinsicComEventCallbackArgHost { .. }
            | Instruction::IntrinsicComReleaseEventCallbackHost { .. }
            // WithEvents
            | Instruction::IntrinsicWithEventsGet { .. }
            | Instruction::IntrinsicWithEventsSet { .. }
            | Instruction::IntrinsicWithEventsClearOwner { .. }
            | Instruction::IntrinsicWithEventsFirstOwner { .. }
            | Instruction::IntrinsicWithEventsNextOwner { .. }
            // UI
            | Instruction::IntrinsicMsgBoxHost { .. }
            | Instruction::IntrinsicInputBoxHost { .. }
            | Instruction::IntrinsicBeepHost { .. }
            | Instruction::IntrinsicDebugPrintHost { .. }
            | Instruction::IntrinsicDoEventsHost { .. }
            | Instruction::IntrinsicShellHost { .. }
            | Instruction::IntrinsicEnvironHost { .. }
            | Instruction::IntrinsicDirHost { .. }
            | Instruction::IntrinsicDateNowHost { .. }
            | Instruction::IntrinsicTimeNowHost { .. }
            | Instruction::IntrinsicNowHost { .. }
            | Instruction::IntrinsicTimerHost { .. }
            // DynLink
            | Instruction::IntrinsicInvokeSymbolHost { .. }
            // Phase 5: Resume
            | Instruction::Resume
            | Instruction::ResumeNext
            | Instruction::ResumeLabel { .. } // Note: IntrinsicTypeOfIs not supported — requires project_dynamic_objects in
                                              // JitContext (tracked gap). Functions using TypeOf...Is fall back to interpreter.
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

fn supports_rtslot(bytecode: &Bytecode) -> bool {
    let len = bytecode.instructions.len();
    if len == 0 {
        return true;
    }

    for instruction in &bytecode.instructions {
        if !supports_rtslot_instruction(instruction) {
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

// ── Inlining ──────────────────────────────────────────────────────────

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
            Instruction::SetOnErrorGotoLabel { target_pc } => {
                let patch_idx = out.len();
                out.push(Instruction::SetOnErrorGotoLabel { target_pc: 0 });
                jump_patches.push((patch_idx, *target_pc));
            }
            Instruction::ResumeLabel { target_pc } => {
                let patch_idx = out.len();
                out.push(Instruction::ResumeLabel { target_pc: 0 });
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
            Instruction::SetOnErrorGotoLabel { target_pc } => *target_pc = new_target,
            Instruction::ResumeLabel { target_pc } => *target_pc = new_target,
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

// ── Public API ────────────────────────────────────────────────────────

pub fn supports_bytecode(bytecode: &Bytecode) -> bool {
    let Ok(inlined) = inline_bytecode(bytecode) else {
        return false;
    };
    supports_core(&inlined)
}

/// Returns true if the bytecode can run on the RtSlot JIT path.
pub fn supports_bytecode_rtslot(bytecode: &Bytecode) -> bool {
    let Ok(inlined) = inline_bytecode(bytecode) else {
        return false;
    };
    supports_rtslot(&inlined)
}

pub fn execute_bytecode(bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, String> {
    execute_bytecode_legacy(bytecode).map(|values| {
        values
            .into_iter()
            .map(RuntimeValue::from_compat_slot_i32)
            .collect()
    })
}

/// Execute bytecode through the legacy 4-byte slot path and project results
/// directly to exact Variant carriers.
pub fn execute_bytecode_variants(bytecode: &Bytecode) -> Result<Vec<Variant>, String> {
    execute_bytecode_legacy(bytecode).map(|values| {
        values
            .into_iter()
            .map(Variant::from_compat_slot_i32)
            .collect()
    })
}

/// Execute bytecode through the RtSlot path. Returns typed RuntimeValues.
pub fn execute_bytecode_rtslot(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<Vec<RuntimeValue>, String> {
    execute_bytecode_rtslot_variants(bytecode, host_services)?
        .into_iter()
        .map(|value| value.to_runtime_value())
        .collect()
}

/// Execute bytecode through the RtSlot path. Returns exact VARIANT carriers.
pub fn execute_bytecode_rtslot_variants(
    bytecode: &Bytecode,
    host_services: Arc<dyn HostServices>,
) -> Result<Vec<Variant>, String> {
    let inlined = inline_bytecode(bytecode)?;
    if !supports_rtslot(&inlined) {
        return Err("unsupported bytecode for rtslot cranelift execution".to_string());
    }

    // Register runtime helper symbols.
    let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| format!("jit builder error: {e}"))?;
    runtime_helpers::register_symbols(&mut jit_builder);

    let mut module = JITModule::new(jit_builder);
    let mut context = module.make_context();
    let ptr_ty = module.target_config().pointer_type();

    // JIT function signature: fn(*mut JitContext) -> i32
    context.func.signature.params.push(AbiParam::new(ptr_ty));
    context
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I32));

    let function_id = module
        .declare_function("oxvba_jit_rtslot", Linkage::Local, &context.func.signature)
        .map_err(|e| format!("declare function error: {e}"))?;

    // ── Declare helper function signatures ────────────────────────────
    // Helper signature: fn(ctx: ptr, dst: i32, lhs: i32, rhs: i32) -> i32
    let sig_3slot = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_ty)); // ctx
        sig.params.push(AbiParam::new(types::I32)); // dst
        sig.params.push(AbiParam::new(types::I32)); // lhs
        sig.params.push(AbiParam::new(types::I32)); // rhs
        sig
    };

    // Helper: fn(ctx, dst, src) -> i32
    let sig_2slot = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig
    };

    // Helper: fn(ctx, slot, value) -> i32
    let sig_slot_i32 = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig
    };

    // Helper: fn(ctx, slot) -> i32
    let sig_1slot = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        sig
    };

    // Comparison helper: fn(ctx, dst, lhs, rhs, mode) -> i32
    let sig_cmp = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32)); // mode
        sig
    };

    // String load helper: fn(ctx, dst, ptr, len) -> i32
    let sig_load_str = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(ptr_ty)); // string data ptr
        sig.params.push(AbiParam::new(types::I32)); // string len
        sig
    };

    // F64 load helper: fn(ctx, dst, bits) -> i32
    let sig_load_f64 = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_ty));
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I64)); // f64 bits
        sig
    };

    // Import helper functions
    let helpers = HelperFuncIds::declare(
        &mut module,
        &sig_3slot,
        &sig_2slot,
        &sig_slot_i32,
        &sig_1slot,
        &sig_cmp,
        &sig_load_str,
        &sig_load_f64,
    )?;

    // ── Build IR ──────────────────────────────────────────────────────
    let mut builder_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_ctx);

    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    let exit_block = builder.create_block();
    let fatal_block = builder.create_block();

    // error_dispatch_block(failing_pc: I32, error_code: I32)
    let error_dispatch_block = builder.create_block();
    builder.append_block_param(error_dispatch_block, types::I32); // failing_pc
    builder.append_block_param(error_dispatch_block, types::I32); // error_code

    // pc_dispatch_block(target_pc: I32) — shared Switch jump table
    let pc_dispatch_block = builder.create_block();
    builder.append_block_param(pc_dispatch_block, types::I32); // target_pc

    let mut pc_blocks = Vec::with_capacity(inlined.instructions.len());
    for _ in 0..inlined.instructions.len() {
        pc_blocks.push(builder.create_block());
    }

    // ctx_ptr variable (pointer to JitContext)
    let ctx_var = builder.declare_var(ptr_ty);

    builder.switch_to_block(entry_block);
    let ctx_ptr = builder.block_params(entry_block)[0];
    builder.def_var(ctx_var, ctx_ptr);

    // Load slots_ptr from JitContext (first field at offset 0)
    let slots_ptr_var = builder.declare_var(ptr_ty);
    let slots_ptr = builder.ins().load(ptr_ty, MemFlags::new(), ctx_ptr, 0);
    builder.def_var(slots_ptr_var, slots_ptr);

    if let Some(first_block) = pc_blocks.first() {
        builder.ins().jump(*first_block, &[]);
    } else {
        builder.ins().jump(exit_block, &[]);
    }

    // Collect string constants for later use (we need their addresses).
    // We'll store them on the Rust heap and pass pointers to the helpers.
    let mut string_constants: Vec<String> = Vec::new();

    // First pass: collect string constants.
    for instruction in &inlined.instructions {
        if let Instruction::LoadConstString { value, .. } = instruction {
            string_constants.push(value.clone());
        }
    }

    // We need stable addresses for string constants.
    // Box them so their addresses remain valid through JIT execution.
    let string_boxes: Vec<Box<[u8]>> = string_constants
        .iter()
        .map(|s| s.as_bytes().to_vec().into_boxed_slice())
        .collect();

    let mut string_idx = 0usize;

    for (pc, instruction) in inlined.instructions.iter().enumerate() {
        builder.switch_to_block(pc_blocks[pc]);
        let ctx_ptr = builder.use_var(ctx_var);
        let slots_ptr = builder.use_var(slots_ptr_var);

        let next_block = if pc + 1 < pc_blocks.len() {
            pc_blocks[pc + 1]
        } else {
            exit_block
        };

        // Closures for reading/writing RtSlot VARTYPE and payload inline.
        let read_vtype = |builder: &mut FunctionBuilder,
                          slots_ptr: cranelift_codegen::ir::Value,
                          slot: usize|
         -> Result<cranelift_codegen::ir::Value, String> {
            let offset = rtslot_vtype_offset(slot)?;
            let addr = builder.ins().iadd_imm(slots_ptr, offset);
            Ok(builder.ins().load(types::I32, MemFlags::new(), addr, 0))
        };

        let read_payload_i64 = |builder: &mut FunctionBuilder,
                                slots_ptr: cranelift_codegen::ir::Value,
                                slot: usize|
         -> Result<cranelift_codegen::ir::Value, String> {
            let offset = rtslot_payload_offset(slot)?;
            let addr = builder.ins().iadd_imm(slots_ptr, offset);
            Ok(builder.ins().load(types::I64, MemFlags::new(), addr, 0))
        };

        let write_vtype_payload = |builder: &mut FunctionBuilder,
                                   slots_ptr: cranelift_codegen::ir::Value,
                                   slot: usize,
                                   vtype: i64,
                                   payload: cranelift_codegen::ir::Value|
         -> Result<(), String> {
            let vtype_off = rtslot_vtype_offset(slot)?;
            let pay_off = rtslot_payload_offset(slot)?;
            let vtype_addr = builder.ins().iadd_imm(slots_ptr, vtype_off);
            let pay_addr = builder.ins().iadd_imm(slots_ptr, pay_off);
            let vtype_val = builder.ins().iconst(types::I32, vtype);
            builder
                .ins()
                .store(MemFlags::new(), vtype_val, vtype_addr, 0);
            builder.ins().store(MemFlags::new(), payload, pay_addr, 0);
            Ok(())
        };

        // Helper to emit a call to a runtime helper and check the return code.
        // On error (rc != 0), branches to error_dispatch_block with (pc, rc).
        let emit_helper_call_and_check =
            |builder: &mut FunctionBuilder,
             func_ref: cranelift_codegen::ir::FuncRef,
             args: &[cranelift_codegen::ir::Value],
             next: cranelift_codegen::ir::Block| {
                let call = builder.ins().call(func_ref, args);
                let rc = builder.inst_results(call)[0];
                let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
                let pc_imm = builder.ins().iconst(types::I32, pc as i64);
                let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
                builder
                    .ins()
                    .brif(is_ok, next, &[], error_dispatch_block, &err_args);
            };

        match instruction {
            Instruction::LoadConstI32 { slot, value } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_load_i32"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *slot as i64);
                let value_val = builder.ins().iconst(types::I32, i64::from(*value));
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, value_val],
                    next_block,
                );
            }
            Instruction::LoadConstBool { slot, value } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_load_bool"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *slot as i64);
                let value_val = builder
                    .ins()
                    .iconst(types::I32, i64::from(i32::from(*value)));
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, value_val],
                    next_block,
                );
            }
            Instruction::LoadConstF64 { slot, bits } => {
                let func_ref = module.declare_func_in_func(helpers.load_f64, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *slot as i64);
                let bits_val = builder.ins().iconst(types::I64, *bits as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, bits_val],
                    next_block,
                );
            }
            Instruction::LoadConstString { slot, .. } => {
                let func_ref = module.declare_func_in_func(helpers.load_string, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *slot as i64);
                let str_data = &string_boxes[string_idx];
                let str_ptr = builder.ins().iconst(ptr_ty, str_data.as_ptr() as i64);
                let str_len = builder.ins().iconst(types::I32, str_data.len() as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, str_ptr, str_len],
                    next_block,
                );
                string_idx += 1;
            }
            Instruction::LoadNull { slot } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_load_null"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *slot as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::AddConstI32 { slot, value } => {
                // Inline i32 fast path: check VT_I4, then add directly.
                let vtype = read_vtype(&mut builder, slots_ptr, *slot)?;
                let is_i32 = builder.ins().icmp_imm(IntCC::Equal, vtype, VT_I4 as i64);

                let fast_block = builder.create_block();
                let slow_block = builder.create_block();

                builder.ins().brif(is_i32, fast_block, &[], slow_block, &[]);

                // Fast path: inline i32 add
                builder.switch_to_block(fast_block);
                let slots_ptr_f = builder.use_var(slots_ptr_var);
                let payload = read_payload_i64(&mut builder, slots_ptr_f, *slot)?;
                let payload_i32 = builder.ins().ireduce(types::I32, payload);
                let result = builder.ins().iadd_imm(payload_i32, i64::from(*value));
                let result_i64 = builder.ins().uextend(types::I64, result);
                write_vtype_payload(&mut builder, slots_ptr_f, *slot, VT_I4 as i64, result_i64)?;
                builder.ins().jump(next_block, &[]);

                // Slow path: call helper
                builder.switch_to_block(slow_block);
                let ctx_ptr_s = builder.use_var(ctx_var);
                let func_ref = module.declare_func_in_func(helpers.add_const, builder.func);
                let slot_val = builder.ins().iconst(types::I32, *slot as i64);
                let val = builder.ins().iconst(types::I32, i64::from(*value));
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr_s, slot_val, val],
                    next_block,
                );
            }
            Instruction::SubConstI32 { slot, value } => {
                let vtype = read_vtype(&mut builder, slots_ptr, *slot)?;
                let is_i32 = builder.ins().icmp_imm(IntCC::Equal, vtype, VT_I4 as i64);

                let fast_block = builder.create_block();
                let slow_block = builder.create_block();

                builder.ins().brif(is_i32, fast_block, &[], slow_block, &[]);

                builder.switch_to_block(fast_block);
                let slots_ptr_f = builder.use_var(slots_ptr_var);
                let payload = read_payload_i64(&mut builder, slots_ptr_f, *slot)?;
                let payload_i32 = builder.ins().ireduce(types::I32, payload);
                let delta = builder.ins().iconst(types::I32, i64::from(*value));
                let result = builder.ins().isub(payload_i32, delta);
                let result_i64 = builder.ins().uextend(types::I64, result);
                write_vtype_payload(&mut builder, slots_ptr_f, *slot, VT_I4 as i64, result_i64)?;
                builder.ins().jump(next_block, &[]);

                builder.switch_to_block(slow_block);
                let ctx_ptr_s = builder.use_var(ctx_var);
                let func_ref = module.declare_func_in_func(helpers.sub_const, builder.func);
                let slot_val = builder.ins().iconst(types::I32, *slot as i64);
                let val = builder.ins().iconst(types::I32, i64::from(*value));
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr_s, slot_val, val],
                    next_block,
                );
            }
            Instruction::AddSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.add_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::SubSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.sub_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::MulSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.mul_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::DivSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.div_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::IntDivSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.intdiv_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::ModSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.mod_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::PowSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.pow_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::NegSlot { dst, src } => {
                let func_ref = module.declare_func_in_func(helpers.neg_slot, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let src_val = builder.ins().iconst(types::I32, *src as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, src_val],
                    next_block,
                );
            }
            Instruction::ConcatSlots { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.concat_slots, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::CopySlot { dst, src } => {
                let func_ref = module.declare_func_in_func(helpers.copy_slot, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let src_val = builder.ins().iconst(types::I32, *src as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, src_val],
                    next_block,
                );
            }
            Instruction::IntrinsicAbsI32 { dst, src } => {
                let func_ref = module.declare_func_in_func(helpers.abs, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let src_val = builder.ins().iconst(types::I32, *src as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, src_val],
                    next_block,
                );
            }
            Instruction::IntrinsicSgnI32 { dst, src } => {
                let func_ref = module.declare_func_in_func(helpers.sgn, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let src_val = builder.ins().iconst(types::I32, *src as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, src_val],
                    next_block,
                );
            }
            Instruction::IntrinsicIntI32 { dst, src }
            | Instruction::IntrinsicFixI32 { dst, src } => {
                let func_ref = module.declare_func_in_func(helpers.int_fix, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let src_val = builder.ins().iconst(types::I32, *src as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, src_val],
                    next_block,
                );
            }
            Instruction::IncSlot { slot } => {
                let func_ref = module.declare_func_in_func(helpers.inc_slot, builder.func);
                let slot_val = builder.ins().iconst(types::I32, *slot as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, slot_val],
                    next_block,
                );
            }
            Instruction::CmpEqSlots {
                dst,
                lhs,
                rhs,
                mode,
            } => {
                emit_cmp_helper(
                    &mut builder,
                    &mut module,
                    &helpers,
                    ctx_ptr,
                    *dst,
                    *lhs,
                    *rhs,
                    *mode,
                    "oxrt_cmp_eq",
                    pc,
                    next_block,
                    error_dispatch_block,
                )?;
            }
            Instruction::CmpNeSlots {
                dst,
                lhs,
                rhs,
                mode,
            } => {
                emit_cmp_helper(
                    &mut builder,
                    &mut module,
                    &helpers,
                    ctx_ptr,
                    *dst,
                    *lhs,
                    *rhs,
                    *mode,
                    "oxrt_cmp_ne",
                    pc,
                    next_block,
                    error_dispatch_block,
                )?;
            }
            Instruction::CmpLtSlots {
                dst,
                lhs,
                rhs,
                mode,
            } => {
                emit_cmp_helper(
                    &mut builder,
                    &mut module,
                    &helpers,
                    ctx_ptr,
                    *dst,
                    *lhs,
                    *rhs,
                    *mode,
                    "oxrt_cmp_lt",
                    pc,
                    next_block,
                    error_dispatch_block,
                )?;
            }
            Instruction::CmpLeSlots {
                dst,
                lhs,
                rhs,
                mode,
            } => {
                emit_cmp_helper(
                    &mut builder,
                    &mut module,
                    &helpers,
                    ctx_ptr,
                    *dst,
                    *lhs,
                    *rhs,
                    *mode,
                    "oxrt_cmp_le",
                    pc,
                    next_block,
                    error_dispatch_block,
                )?;
            }
            Instruction::CmpGtSlots {
                dst,
                lhs,
                rhs,
                mode,
            } => {
                emit_cmp_helper(
                    &mut builder,
                    &mut module,
                    &helpers,
                    ctx_ptr,
                    *dst,
                    *lhs,
                    *rhs,
                    *mode,
                    "oxrt_cmp_gt",
                    pc,
                    next_block,
                    error_dispatch_block,
                )?;
            }
            Instruction::CmpGeSlots {
                dst,
                lhs,
                rhs,
                mode,
            } => {
                emit_cmp_helper(
                    &mut builder,
                    &mut module,
                    &helpers,
                    ctx_ptr,
                    *dst,
                    *lhs,
                    *rhs,
                    *mode,
                    "oxrt_cmp_ge",
                    pc,
                    next_block,
                    error_dispatch_block,
                )?;
            }
            Instruction::BoolNot { dst, src } => {
                let func_ref = module.declare_func_in_func(helpers.bool_not, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let src_val = builder.ins().iconst(types::I32, *src as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, src_val],
                    next_block,
                );
            }
            Instruction::BoolAnd { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.bool_and, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::BoolOr { dst, lhs, rhs } => {
                let func_ref = module.declare_func_in_func(helpers.bool_or, builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let lhs_val = builder.ins().iconst(types::I32, *lhs as i64);
                let rhs_val = builder.ins().iconst(types::I32, *rhs as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, lhs_val, rhs_val],
                    next_block,
                );
            }
            Instruction::JumpIfZero {
                cond_slot,
                target_pc,
            } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_jump_if_zero"), builder.func);
                let src_val = builder.ins().iconst(types::I32, *cond_slot as i64);
                let call = builder.ins().call(func_ref, &[ctx_ptr, src_val]);
                let rc = builder.inst_results(call)[0];
                let is_error = builder.ins().icmp_imm(IntCC::Equal, rc, -1);
                let pc_imm = builder.ins().iconst(types::I32, pc as i64);
                let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
                let dispatch_block = builder.create_block();
                builder.ins().brif(
                    is_error,
                    error_dispatch_block,
                    &err_args,
                    dispatch_block,
                    &[],
                );
                builder.switch_to_block(dispatch_block);
                let is_zero = builder.ins().icmp_imm(IntCC::Equal, rc, 1);
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
            Instruction::ClearErr => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_clear_err"), builder.func);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr], next_block);
            }
            Instruction::Halt => {
                builder.ins().jump(exit_block, &[]);
            }
            // ── Phase 2: String ops ──────────────────────────────────
            Instruction::IntrinsicLenDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_len",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicLeftDigits { dst, src, count } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_left",
                    ctx_ptr,
                    *dst,
                    *src,
                    *count,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicRightDigits { dst, src, count } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_right",
                    ctx_ptr,
                    *dst,
                    *src,
                    *count,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicMidDigits {
                dst,
                src,
                start,
                count,
            } => {
                let count_val = count.map(|c| c as i64).unwrap_or(u32::MAX as i64);
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_mid",
                    ctx_ptr,
                    *dst,
                    *src,
                    *start,
                    count_val,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicMidStmtDigits {
                target,
                start,
                count,
                value,
            } => {
                let count_val = count.map(|c| c as i64).unwrap_or(u32::MAX as i64);
                emit_extra_4slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_mid_stmt",
                    ctx_ptr,
                    *target as i64,
                    *start as i64,
                    count_val,
                    *value as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicInStrDigits {
                dst,
                haystack,
                needle,
                mode,
            } => {
                let mode_val = match mode {
                    StringCompareMode::Text => 1,
                    _ => 0,
                };
                emit_extra_4slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_instr",
                    ctx_ptr,
                    *dst as i64,
                    *haystack as i64,
                    *needle as i64,
                    mode_val,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicInStrRevDigits {
                dst,
                haystack,
                needle,
                mode,
            } => {
                let mode_val = match mode {
                    StringCompareMode::Text => 1,
                    _ => 0,
                };
                emit_extra_4slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_instrrev",
                    ctx_ptr,
                    *dst as i64,
                    *haystack as i64,
                    *needle as i64,
                    mode_val,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicLowerDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_lower",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicUpperDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_upper",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicSplitCountDigits {
                dst,
                src,
                delimiter,
            } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_split",
                    ctx_ptr,
                    *dst,
                    *src,
                    *delimiter,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicJoinDigits {
                dst,
                src,
                delimiter,
            } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_join",
                    ctx_ptr,
                    *dst,
                    *src,
                    *delimiter,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicReplaceDigits {
                dst,
                src,
                find,
                replace,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_replace",
                    ctx_ptr,
                    *dst,
                    *src,
                    *find,
                    *replace as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicTrimDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_trim",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicLTrimDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_ltrim",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicRTrimDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_rtrim",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicStrCompDigits {
                dst,
                lhs,
                rhs,
                mode,
            } => {
                let mode_val = match mode {
                    StringCompareMode::Text => 1,
                    _ => 0,
                };
                emit_extra_4slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_strcomp",
                    ctx_ptr,
                    *dst as i64,
                    *lhs as i64,
                    *rhs as i64,
                    mode_val,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicLikeDigits {
                dst,
                lhs,
                pattern,
                mode,
            } => {
                let mode_val = match mode {
                    StringCompareMode::Text => 1,
                    _ => 0,
                };
                emit_extra_4slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_like",
                    ctx_ptr,
                    *dst as i64,
                    *lhs as i64,
                    *pattern as i64,
                    mode_val,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicStrConvDigits {
                dst,
                src,
                conversion,
            } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_strconv",
                    ctx_ptr,
                    *dst,
                    *src,
                    *conversion,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Char/format ─────────────────────────────────
            Instruction::IntrinsicChrDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_chr",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicAscDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_asc",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicSpaceDigits { dst, count } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_space",
                    ctx_ptr,
                    *dst,
                    *count,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicStringRepeatDigits { dst, count, ch } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_string_repeat",
                    ctx_ptr,
                    *dst,
                    *count,
                    *ch,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicHexDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_hex",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicOctDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_oct",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFormatDigits {
                dst,
                value,
                format_string,
            } => {
                let fmt_slot = format_string.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_3slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_format",
                    ctx_ptr,
                    *dst as i64,
                    *value as i64,
                    fmt_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicStrReverseDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_strreverse",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Date/time ───────────────────────────────────
            Instruction::IntrinsicDateSerialDigits {
                dst,
                year,
                month,
                day,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_date_serial",
                    ctx_ptr,
                    *dst,
                    *year,
                    *month,
                    *day as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicTimeSerialDigits {
                dst,
                hour,
                minute,
                second,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_time_serial",
                    ctx_ptr,
                    *dst,
                    *hour,
                    *minute,
                    *second as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDateValueDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_date_value",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicCDateValue { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_cdate",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicTimeValueDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_time_value",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDateAddDigits {
                dst,
                interval,
                number,
                date,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_date_add",
                    ctx_ptr,
                    *dst,
                    *interval,
                    *number,
                    *date as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDateDiffDigits {
                dst,
                interval,
                date1,
                date2,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_date_diff",
                    ctx_ptr,
                    *dst,
                    *interval,
                    *date1,
                    *date2 as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicYearDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_year",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicMonthDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_month",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDayDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_day",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicWeekdayDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_weekday",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicMonthNameDigits { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_month_name",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Math ────────────────────────────────────────
            Instruction::IntrinsicRoundI32 { dst, src, digits } => {
                let dig_slot = digits.map(|d| d as i64).unwrap_or(u32::MAX as i64);
                emit_extra_3slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_round",
                    ctx_ptr,
                    *dst as i64,
                    *src as i64,
                    dig_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicSqrI32 { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_sqr",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicSinI32 { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_sin",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicCosI32 { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_cos",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicLogI32 { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_log",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicExpI32 { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_exp",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicAtnI32 { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_atn",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicTanI32 { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_tan",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Type checking ───────────────────────────────
            Instruction::IntrinsicVarTypeTag { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_vartype_tag",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicVarType { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_vartype",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicTypeNameTag { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_typename_tag",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsNumericTag { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_numeric_tag",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicStrPtr { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_strptr",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicVarPtr { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_varptr",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicVarPtrStringVar { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_varptr_string_var",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicVarPtrVariantVar { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_varptr_variant_var",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicObjPtr { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_objptr",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsNumeric { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_numeric",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsError { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_error",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsDateTag { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_date_tag",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsObjectTag { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_object_tag",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsNull { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_null",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsEmpty { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_empty",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicIsArrayTag { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_is_array_tag",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Financial ───────────────────────────────────
            Instruction::IntrinsicFvI32 {
                dst,
                rate,
                nper,
                pmt,
                pv,
                due,
            } => {
                let pv_slot = pv.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                let due_slot = due.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_nslot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_fv",
                    ctx_ptr,
                    &[
                        *dst as i64,
                        *rate as i64,
                        *nper as i64,
                        *pmt as i64,
                        pv_slot,
                        due_slot,
                    ],
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicPvI32 {
                dst,
                rate,
                nper,
                pmt,
                fv,
                due,
            } => {
                let fv_slot = fv.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                let due_slot = due.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_nslot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_pv",
                    ctx_ptr,
                    &[
                        *dst as i64,
                        *rate as i64,
                        *nper as i64,
                        *pmt as i64,
                        fv_slot,
                        due_slot,
                    ],
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicPmtI32 {
                dst,
                rate,
                nper,
                pv,
                fv,
                due,
            } => {
                let fv_slot = fv.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                let due_slot = due.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_nslot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_pmt",
                    ctx_ptr,
                    &[
                        *dst as i64,
                        *rate as i64,
                        *nper as i64,
                        *pv as i64,
                        fv_slot,
                        due_slot,
                    ],
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicNpvI32 { dst, rate, values } => {
                // Pass slot indices as a boxed array on the heap
                let slot_indices: Vec<u32> = values.iter().map(|s| *s as u32).collect();
                let boxed = slot_indices.into_boxed_slice();
                let ptr_val = builder.ins().iconst(ptr_ty, boxed.as_ptr() as i64);
                let len_val = builder.ins().iconst(types::I32, boxed.len() as i64);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let rate_val = builder.ins().iconst(types::I32, *rate as i64);
                let func_ref = module.declare_func_in_func(helpers.extra("oxrt_npv"), builder.func);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, rate_val, ptr_val, len_val],
                    next_block,
                );
                std::mem::forget(boxed); // Keep alive; leaked for simplicity
            }
            Instruction::IntrinsicIrrI32 { dst, value, guess } => {
                let guess_slot = guess.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_3slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_irr",
                    ctx_ptr,
                    *dst as i64,
                    *value as i64,
                    guess_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicMirrI32 {
                dst,
                value,
                finance_rate,
                reinvest_rate,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_mirr",
                    ctx_ptr,
                    *dst,
                    *value,
                    *finance_rate,
                    *reinvest_rate as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicRateI32 {
                dst,
                nper,
                pmt,
                pv,
                fv,
                due,
                guess,
            } => {
                let fv_slot = fv.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                let due_slot = due.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                let guess_slot = guess.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_nslot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_rate",
                    ctx_ptr,
                    &[
                        *dst as i64,
                        *nper as i64,
                        *pmt as i64,
                        *pv as i64,
                        fv_slot,
                        due_slot,
                        guess_slot,
                    ],
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicNPerI32 {
                dst,
                rate,
                pmt,
                pv,
                fv,
                due,
            } => {
                let fv_slot = fv.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                let due_slot = due.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_nslot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_nper",
                    ctx_ptr,
                    &[
                        *dst as i64,
                        *rate as i64,
                        *pmt as i64,
                        *pv as i64,
                        fv_slot,
                        due_slot,
                    ],
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Array ───────────────────────────────────────
            Instruction::IntrinsicArrayLiteral { dst, values } => {
                let slot_indices: Vec<u32> = values.iter().map(|s| *s as u32).collect();
                let boxed = slot_indices.into_boxed_slice();
                let ptr_val = builder.ins().iconst(ptr_ty, boxed.as_ptr() as i64);
                let len_val = builder.ins().iconst(types::I32, boxed.len() as i64);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_array_literal"), builder.func);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, ptr_val, len_val],
                    next_block,
                );
                std::mem::forget(boxed);
            }
            Instruction::IntrinsicLBoundArray { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_lbound",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicUBoundArray { dst, src } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_ubound",
                    ctx_ptr,
                    *dst,
                    *src,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicArrayGet {
                dst,
                array,
                indices,
            } => {
                let boxed = indices
                    .iter()
                    .map(|slot| *slot as u32)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let ptr_val = builder.ins().iconst(ptr_ty, boxed.as_ptr() as i64);
                let len_val = builder.ins().iconst(types::I32, boxed.len() as i64);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let array_val = builder.ins().iconst(types::I32, *array as i64);
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_array_get"), builder.func);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, array_val, ptr_val, len_val],
                    next_block,
                );
                std::mem::forget(boxed);
            }
            Instruction::IntrinsicArraySet {
                array,
                indices,
                src,
            } => {
                let boxed = indices
                    .iter()
                    .map(|slot| *slot as u32)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let ptr_val = builder.ins().iconst(ptr_ty, boxed.as_ptr() as i64);
                let len_val = builder.ins().iconst(types::I32, boxed.len() as i64);
                let array_val = builder.ins().iconst(types::I32, *array as i64);
                let src_val = builder.ins().iconst(types::I32, *src as i64);
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_array_set"), builder.func);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, array_val, ptr_val, len_val, src_val],
                    next_block,
                );
                std::mem::forget(boxed);
            }
            Instruction::IntrinsicArrayAppend { dst, array, item } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_array_append",
                    ctx_ptr,
                    *dst,
                    *array,
                    *item,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicArrayResize {
                dst,
                upper_bounds,
                lower_bounds,
                element_type,
            } => {
                let upper_boxed = upper_bounds
                    .iter()
                    .map(|slot| *slot as u32)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let lower_boxed = lower_bounds.clone().into_boxed_slice();
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let upper_ptr = builder.ins().iconst(ptr_ty, upper_boxed.as_ptr() as i64);
                let lower_ptr = builder.ins().iconst(ptr_ty, lower_boxed.as_ptr() as i64);
                let len_val = builder.ins().iconst(types::I32, upper_boxed.len() as i64);
                let elem_val = builder.ins().iconst(types::I32, *element_type as i64);
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_array_resize"), builder.func);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, upper_ptr, lower_ptr, len_val, elem_val],
                    next_block,
                );
                std::mem::forget(upper_boxed);
                std::mem::forget(lower_boxed);
            }
            Instruction::IntrinsicArrayResizePreserve {
                dst,
                upper_bounds,
                lower_bounds,
                element_type,
            } => {
                let upper_boxed = upper_bounds
                    .iter()
                    .map(|slot| *slot as u32)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let lower_boxed = lower_bounds.clone().into_boxed_slice();
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let upper_ptr = builder.ins().iconst(ptr_ty, upper_boxed.as_ptr() as i64);
                let lower_ptr = builder.ins().iconst(ptr_ty, lower_boxed.as_ptr() as i64);
                let len_val = builder.ins().iconst(types::I32, upper_boxed.len() as i64);
                let elem_val = builder.ins().iconst(types::I32, *element_type as i64);
                let func_ref = module.declare_func_in_func(
                    helpers.extra("oxrt_array_resize_preserve"),
                    builder.func,
                );
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, upper_ptr, lower_ptr, len_val, elem_val],
                    next_block,
                );
                std::mem::forget(upper_boxed);
                std::mem::forget(lower_boxed);
            }
            // ── Phase 2: Collection ──────────────────────────────────
            Instruction::IntrinsicCollectionAdd { dst, count, item } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_collection_add",
                    ctx_ptr,
                    *dst,
                    *count,
                    *item,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicCollectionItem { dst, count, index } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_collection_item",
                    ctx_ptr,
                    *dst,
                    *count,
                    *index,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicCollectionRemove { dst, count, index } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_collection_remove",
                    ctx_ptr,
                    *dst,
                    *count,
                    *index,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicCollectionCount { dst, count } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_collection_count",
                    ctx_ptr,
                    *dst,
                    *count,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Random ──────────────────────────────────────
            Instruction::IntrinsicRndDigits { dst, seed } => {
                let seed_slot = seed.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_2slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_rnd",
                    ctx_ptr,
                    *dst as i64,
                    seed_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicRandomizeDigits { dst, seed } => {
                let seed_slot = seed.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_2slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_randomize",
                    ctx_ptr,
                    *dst as i64,
                    seed_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            // ── Phase 2: Assignment ──────────────────────────────────
            Instruction::ValidateRuntimeAssignment {
                src: _,
                intent: _,
                target_kind: _,
                target_name: _,
                target_type_name: _,
            } => {
                // This is a complex helper - skip it in JIT (no-op pass through)
                // The VM handles validation; JIT trusts the compiler output.
                builder.ins().jump(next_block, &[]);
            }
            // ── Phase 3: Error handling ──────────────────────────────
            Instruction::SetOnErrorResumeNext => {
                let func_ref = module.declare_func_in_func(
                    helpers.extra("oxrt_set_on_error_resume_next"),
                    builder.func,
                );
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr], next_block);
            }
            Instruction::SetOnErrorGoto0 => {
                let func_ref = module
                    .declare_func_in_func(helpers.extra("oxrt_set_on_error_goto0"), builder.func);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr], next_block);
            }
            Instruction::SetOnErrorGotoLabel { target_pc } => {
                let func_ref = module.declare_func_in_func(
                    helpers.extra("oxrt_set_on_error_goto_label"),
                    builder.func,
                );
                let pc_val = builder.ins().iconst(types::I32, *target_pc as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, pc_val], next_block);
            }
            Instruction::LoadErrNumber { slot } => {
                let func_ref = module
                    .declare_func_in_func(helpers.extra("oxrt_load_err_number"), builder.func);
                let slot_val = builder.ins().iconst(types::I32, *slot as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, slot_val],
                    next_block,
                );
            }
            Instruction::LoadErrDescription { slot } => {
                let func_ref = module
                    .declare_func_in_func(helpers.extra("oxrt_load_err_description"), builder.func);
                let slot_val = builder.ins().iconst(types::I32, *slot as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, slot_val],
                    next_block,
                );
            }
            Instruction::LoadErrSource { slot } => {
                let func_ref = module
                    .declare_func_in_func(helpers.extra("oxrt_load_err_source"), builder.func);
                let slot_val = builder.ins().iconst(types::I32, *slot as i64);
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, slot_val],
                    next_block,
                );
            }
            Instruction::RaiseError { code } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_raise_error"), builder.func);
                let code_val = builder.ins().iconst(types::I32, i64::from(*code));
                let pc_val = builder.ins().iconst(types::I32, pc as i64);
                let call = builder.ins().call(func_ref, &[ctx_ptr, code_val, pc_val]);
                let rc = builder.inst_results(call)[0];
                // 0 = OERN handled → next_block
                // positive = GoTo target → pc_dispatch_block
                // negative = fatal → fatal_block
                let is_oern = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
                let nonzero_block = builder.create_block();
                builder
                    .ins()
                    .brif(is_oern, next_block, &[], nonzero_block, &[]);
                builder.switch_to_block(nonzero_block);
                let is_goto = builder.ins().icmp_imm(IntCC::SignedGreaterThan, rc, 0);
                builder.ins().brif(
                    is_goto,
                    pc_dispatch_block,
                    &[BlockArg::Value(rc)],
                    fatal_block,
                    &[],
                );
            }
            // ── Phase 4: Host services ───────────────────────────────
            Instruction::IntrinsicFreeFileHost {
                dst,
                range_selector,
            } => {
                let range_slot = range_selector.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_2slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_free_file",
                    ctx_ptr,
                    *dst as i64,
                    range_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileOpenHost {
                dst,
                path,
                mode,
                file_number,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_open",
                    ctx_ptr,
                    *dst,
                    *path,
                    *mode,
                    *file_number as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileCloseHost { dst, handle } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_close",
                    ctx_ptr,
                    *dst,
                    *handle,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileKillHost { dst, path } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_kill",
                    ctx_ptr,
                    *dst,
                    *path,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileReadHost { dst, handle, count } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_read",
                    ctx_ptr,
                    *dst,
                    *handle,
                    *count,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileWriteHost { dst, handle, data } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_write",
                    ctx_ptr,
                    *dst,
                    *handle,
                    *data,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFilePrintHost { dst, handle, data } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_print",
                    ctx_ptr,
                    *dst,
                    *handle,
                    *data,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicConsolePrintHost { dst, data } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_console_print",
                    ctx_ptr,
                    *dst,
                    *data,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileInputHost { dst, handle, count } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_input",
                    ctx_ptr,
                    *dst,
                    *handle,
                    *count,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicConsoleInputHost { dst, count } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_console_input",
                    ctx_ptr,
                    *dst,
                    *count,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileLineInputHost { dst, handle } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_line_input",
                    ctx_ptr,
                    *dst,
                    *handle,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicConsoleLineInputHost { dst } => {
                let func_ref = module.declare_func_in_func(
                    helpers.extra("oxrt_host_console_line_input"),
                    builder.func,
                );
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicFileEofHost { dst, handle } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_eof",
                    ctx_ptr,
                    *dst,
                    *handle,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileLofHost { dst, handle } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_lof",
                    ctx_ptr,
                    *dst,
                    *handle,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileSeekHost { dst, handle } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_seek",
                    ctx_ptr,
                    *dst,
                    *handle,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicFileLocHost { dst, handle } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_file_loc",
                    ctx_ptr,
                    *dst,
                    *handle,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicCreateObjectHost { dst, prog_id } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_create_object",
                    ctx_ptr,
                    *dst,
                    *prog_id,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDispatchInvokeHost {
                dst,
                object,
                member,
                args,
            } => {
                let func_ref = module
                    .declare_func_in_func(helpers.extra("oxrt_host_dispatch_invoke"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let obj_val = builder.ins().iconst(types::I32, *object as i64);
                let mem_val = builder.ins().iconst(types::I32, *member as i64);
                let (aptr, alen) = if args.is_empty() {
                    (
                        builder.ins().iconst(ptr_ty, 0i64),
                        builder.ins().iconst(types::I32, 0i64),
                    )
                } else {
                    (
                        builder.ins().iconst(ptr_ty, args.as_ptr() as i64),
                        builder.ins().iconst(types::I32, args.len() as i64),
                    )
                };
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, obj_val, mem_val, aptr, alen],
                    next_block,
                );
            }
            Instruction::IntrinsicComSubscribeEventHost { dst, object, event } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_com_subscribe",
                    ctx_ptr,
                    *dst,
                    *object,
                    *event,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicComUnsubscribeEventHost { dst, subscription } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_com_unsubscribe",
                    ctx_ptr,
                    *dst,
                    *subscription,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicComEventCallbackSubscriptionHost { dst, callback } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_com_event_callback_sub",
                    ctx_ptr,
                    *dst,
                    *callback,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicComEventCallbackArgHost {
                dst,
                callback,
                index,
            } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_com_event_callback_arg",
                    ctx_ptr,
                    *dst,
                    *callback,
                    *index,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicComReleaseEventCallbackHost { dst, callback } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_com_release_event_callback",
                    ctx_ptr,
                    *dst,
                    *callback,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicWithEventsGet {
                dst,
                owner,
                binding,
            } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_withevents_get",
                    ctx_ptr,
                    *dst,
                    *owner,
                    *binding,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicWithEventsSet {
                dst,
                owner,
                binding,
                value,
            } => {
                emit_extra_4slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_withevents_set",
                    ctx_ptr,
                    *dst,
                    *owner,
                    *binding,
                    *value as i64,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicWithEventsClearOwner { dst, owner } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_withevents_clear_owner",
                    ctx_ptr,
                    *dst,
                    *owner,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicWithEventsFirstOwner {
                dst,
                source,
                binding,
            } => {
                emit_extra_3slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_withevents_first_owner",
                    ctx_ptr,
                    *dst,
                    *source,
                    *binding,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicWithEventsNextOwner { dst } => {
                let func_ref = module.declare_func_in_func(
                    helpers.extra("oxrt_host_withevents_next_owner"),
                    builder.func,
                );
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicMsgBoxHost { dst, prompt, style } => {
                let style_slot = style.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_3slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_msgbox",
                    ctx_ptr,
                    *dst as i64,
                    *prompt as i64,
                    style_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicInputBoxHost {
                dst,
                prompt,
                default_value,
            } => {
                let default_slot = default_value.map(|s| s as i64).unwrap_or(u32::MAX as i64);
                emit_extra_3slot_i(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_inputbox",
                    ctx_ptr,
                    *dst as i64,
                    *prompt as i64,
                    default_slot,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicBeepHost { dst } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_host_beep"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicDebugPrintHost { dst, data } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_debug_print",
                    ctx_ptr,
                    *dst,
                    *data,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDoEventsHost { dst } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_host_do_events"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicShellHost { dst, command } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_shell",
                    ctx_ptr,
                    *dst,
                    *command,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicEnvironHost { dst, key } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_environ",
                    ctx_ptr,
                    *dst,
                    *key,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDirHost { dst, path } => {
                emit_extra_2slot(
                    &mut builder,
                    &mut module,
                    &helpers,
                    "oxrt_host_dir",
                    ctx_ptr,
                    *dst,
                    *path,
                    pc,
                    next_block,
                    error_dispatch_block,
                );
            }
            Instruction::IntrinsicDateNowHost { dst } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_host_date_now"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicTimeNowHost { dst } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_host_time_now"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicNowHost { dst } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_host_now"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicTimerHost { dst } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_host_timer"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                emit_helper_call_and_check(&mut builder, func_ref, &[ctx_ptr, dst_val], next_block);
            }
            Instruction::IntrinsicInvokeSymbolHost {
                dst,
                descriptor_id,
                symbol,
                args,
                writeback_slots,
            } => {
                let func_ref = module
                    .declare_func_in_func(helpers.extra("oxrt_host_invoke_symbol"), builder.func);
                let dst_val = builder.ins().iconst(types::I32, *dst as i64);
                let sym_val = builder.ins().iconst(types::I32, i64::from(symbol.raw()));
                let desc_id = builder.ins().iconst(types::I32, i64::from(*descriptor_id));
                let (aptr, alen) = if args.is_empty() {
                    (
                        builder.ins().iconst(ptr_ty, 0i64),
                        builder.ins().iconst(types::I32, 0i64),
                    )
                } else {
                    (
                        builder.ins().iconst(ptr_ty, args.as_ptr() as i64),
                        builder.ins().iconst(types::I32, args.len() as i64),
                    )
                };
                let (wptr, wlen) = if writeback_slots.is_empty() {
                    (
                        builder.ins().iconst(ptr_ty, 0i64),
                        builder.ins().iconst(types::I32, 0i64),
                    )
                } else {
                    (
                        builder
                            .ins()
                            .iconst(ptr_ty, writeback_slots.as_ptr() as i64),
                        builder
                            .ins()
                            .iconst(types::I32, writeback_slots.len() as i64),
                    )
                };
                emit_helper_call_and_check(
                    &mut builder,
                    func_ref,
                    &[ctx_ptr, dst_val, sym_val, desc_id, aptr, alen, wptr, wlen],
                    next_block,
                );
            }
            // ── Phase 5: Resume instructions ────────────────────────
            Instruction::Resume => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_resume"), builder.func);
                let call = builder.ins().call(func_ref, &[ctx_ptr]);
                let rc = builder.inst_results(call)[0];
                // rc >= 0: target PC → pc_dispatch_block
                // rc < 0 (-20): "Resume without error" → route error 20
                let is_ok = builder
                    .ins()
                    .icmp_imm(IntCC::SignedGreaterThanOrEqual, rc, 0);
                let err20_block = builder.create_block();
                builder.ins().brif(
                    is_ok,
                    pc_dispatch_block,
                    &[BlockArg::Value(rc)],
                    err20_block,
                    &[],
                );
                builder.switch_to_block(err20_block);
                let err20_pc = builder.ins().iconst(types::I32, pc as i64);
                let err20_code = builder.ins().iconst(types::I32, 20);
                builder.ins().jump(
                    error_dispatch_block,
                    &[BlockArg::Value(err20_pc), BlockArg::Value(err20_code)],
                );
            }
            Instruction::ResumeNext => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_resume_next"), builder.func);
                let call = builder.ins().call(func_ref, &[ctx_ptr]);
                let rc = builder.inst_results(call)[0];
                let is_ok = builder
                    .ins()
                    .icmp_imm(IntCC::SignedGreaterThanOrEqual, rc, 0);
                let err20_block = builder.create_block();
                builder.ins().brif(
                    is_ok,
                    pc_dispatch_block,
                    &[BlockArg::Value(rc)],
                    err20_block,
                    &[],
                );
                builder.switch_to_block(err20_block);
                let err20_pc = builder.ins().iconst(types::I32, pc as i64);
                let err20_code = builder.ins().iconst(types::I32, 20);
                builder.ins().jump(
                    error_dispatch_block,
                    &[BlockArg::Value(err20_pc), BlockArg::Value(err20_code)],
                );
            }
            Instruction::ResumeLabel { target_pc } => {
                let func_ref =
                    module.declare_func_in_func(helpers.extra("oxrt_resume_label"), builder.func);
                let target_val = builder.ins().iconst(types::I32, *target_pc as i64);
                let call = builder.ins().call(func_ref, &[ctx_ptr, target_val]);
                let rc = builder.inst_results(call)[0];
                let is_ok = builder
                    .ins()
                    .icmp_imm(IntCC::SignedGreaterThanOrEqual, rc, 0);
                let err20_block = builder.create_block();
                builder.ins().brif(
                    is_ok,
                    pc_dispatch_block,
                    &[BlockArg::Value(rc)],
                    err20_block,
                    &[],
                );
                builder.switch_to_block(err20_block);
                let err20_pc = builder.ins().iconst(types::I32, pc as i64);
                let err20_code = builder.ins().iconst(types::I32, 20);
                builder.ins().jump(
                    error_dispatch_block,
                    &[BlockArg::Value(err20_pc), BlockArg::Value(err20_code)],
                );
            }
            _ => {
                return Err(format!(
                    "unsupported instruction for rtslot cranelift execution: {:?}",
                    instruction
                ));
            }
        }
    }

    // Exit block: return 0 (success)
    builder.switch_to_block(exit_block);
    let ok = builder.ins().iconst(types::I32, 0);
    builder.ins().return_(&[ok]);

    // Fatal block: return -1 (unhandled error)
    builder.switch_to_block(fatal_block);
    let err = builder.ins().iconst(types::I32, -1);
    builder.ins().return_(&[err]);

    // Error dispatch block: route errors through oxrt_route_error
    builder.switch_to_block(error_dispatch_block);
    {
        let failing_pc_param = builder.block_params(error_dispatch_block)[0];
        let error_code_param = builder.block_params(error_dispatch_block)[1];
        let ctx2 = builder.use_var(ctx_var);
        let route_func =
            module.declare_func_in_func(helpers.extra("oxrt_route_error"), builder.func);
        let call = builder
            .ins()
            .call(route_func, &[ctx2, error_code_param, failing_pc_param]);
        let result = builder.inst_results(call)[0];
        let is_handled = builder.ins().icmp_imm(IntCC::SignedGreaterThan, result, 0);
        builder.ins().brif(
            is_handled,
            pc_dispatch_block,
            &[BlockArg::Value(result)],
            fatal_block,
            &[],
        );
    }

    // PC dispatch block: Switch jump table mapping target_pc → pc_blocks[target_pc]
    builder.switch_to_block(pc_dispatch_block);
    {
        let target_pc_param = builder.block_params(pc_dispatch_block)[0];
        let mut switch = cranelift_frontend::Switch::new();
        for (i, block) in pc_blocks.iter().enumerate() {
            switch.set_entry(i as u128, *block);
        }
        switch.emit(&mut builder, target_pc_param, fatal_block);
    }

    builder.seal_all_blocks();
    builder.finalize();

    module
        .define_function(function_id, &mut context)
        .map_err(|e| format!("define function error: {e}"))?;
    module.clear_context(&mut context);
    module.finalize_definitions().map_err(|e| format!("{e}"))?;

    let code_ptr = module.get_finalized_function(function_id);
    type JitFn = unsafe extern "C" fn(*mut crate::jit_context::JitContext) -> i32;
    let jit_fn: JitFn = unsafe { std::mem::transmute(code_ptr) };

    // Create JitContext with RtSlot storage.
    let total_slots = inlined.slot_count.max(inlined.user_slot_count);
    let mut ctx_owned = JitContextOwned::new(
        total_slots,
        inlined.user_slot_count,
        host_services,
        &inlined.external_call_descriptors,
    );
    let rc = unsafe { jit_fn(ctx_owned.context_ptr()) };
    if rc != 0 {
        return Err(format!("jit (rtslot) returned non-zero status: {rc}"));
    }

    // Keep string_boxes alive until after JIT execution.
    drop(string_boxes);

    Ok(ctx_owned.extract_user_variants())
}

// ── Helper function ID management ─────────────────────────────────────

struct HelperFuncIds {
    add_slots: cranelift_module::FuncId,
    sub_slots: cranelift_module::FuncId,
    mul_slots: cranelift_module::FuncId,
    div_slots: cranelift_module::FuncId,
    intdiv_slots: cranelift_module::FuncId,
    mod_slots: cranelift_module::FuncId,
    pow_slots: cranelift_module::FuncId,
    neg_slot: cranelift_module::FuncId,
    concat_slots: cranelift_module::FuncId,
    add_const: cranelift_module::FuncId,
    sub_const: cranelift_module::FuncId,
    inc_slot: cranelift_module::FuncId,
    copy_slot: cranelift_module::FuncId,
    abs: cranelift_module::FuncId,
    sgn: cranelift_module::FuncId,
    int_fix: cranelift_module::FuncId,
    bool_not: cranelift_module::FuncId,
    bool_and: cranelift_module::FuncId,
    bool_or: cranelift_module::FuncId,
    cmp_eq: cranelift_module::FuncId,
    cmp_ne: cranelift_module::FuncId,
    cmp_lt: cranelift_module::FuncId,
    cmp_le: cranelift_module::FuncId,
    cmp_gt: cranelift_module::FuncId,
    cmp_ge: cranelift_module::FuncId,
    load_string: cranelift_module::FuncId,
    load_f64: cranelift_module::FuncId,
    // Dynamic lookup for Phase 2-4 helpers
    extras: HashMap<String, cranelift_module::FuncId>,
}

impl HelperFuncIds {
    fn declare(
        module: &mut JITModule,
        sig_3slot: &cranelift_codegen::ir::Signature,
        sig_2slot: &cranelift_codegen::ir::Signature,
        sig_slot_i32: &cranelift_codegen::ir::Signature,
        sig_1slot: &cranelift_codegen::ir::Signature,
        sig_cmp: &cranelift_codegen::ir::Signature,
        sig_load_str: &cranelift_codegen::ir::Signature,
        sig_load_f64: &cranelift_codegen::ir::Signature,
    ) -> Result<Self, String> {
        let map_err = |e| format!("declare helper error: {e}");
        // All signatures need return type added
        let mut s3 = sig_3slot.clone();
        s3.returns.push(AbiParam::new(types::I32));
        let mut s2 = sig_2slot.clone();
        s2.returns.push(AbiParam::new(types::I32));
        let mut s_si = sig_slot_i32.clone();
        s_si.returns.push(AbiParam::new(types::I32));
        let mut s1 = sig_1slot.clone();
        s1.returns.push(AbiParam::new(types::I32));
        let mut sc = sig_cmp.clone();
        sc.returns.push(AbiParam::new(types::I32));
        let mut sls = sig_load_str.clone();
        sls.returns.push(AbiParam::new(types::I32));
        let mut sf = sig_load_f64.clone();
        sf.returns.push(AbiParam::new(types::I32));

        let ptr_ty = module.target_config().pointer_type();

        // Build various signature patterns for phase 2-4 helpers
        let make_sig = |module: &mut JITModule, param_types: &[cranelift_codegen::ir::Type]| {
            let mut sig = module.make_signature();
            for &ty in param_types {
                sig.params.push(AbiParam::new(ty));
            }
            sig.returns.push(AbiParam::new(types::I32));
            sig
        };

        let sig_ctx_only = make_sig(module, &[ptr_ty]); // fn(ctx) -> i32
        let sig_ctx_1 = make_sig(module, &[ptr_ty, types::I32]); // fn(ctx, slot) -> i32
        let sig_ctx_2 = make_sig(module, &[ptr_ty, types::I32, types::I32]); // fn(ctx, dst, src) -> i32
        let sig_ctx_3 = make_sig(module, &[ptr_ty, types::I32, types::I32, types::I32]); // fn(ctx, dst, a, b) -> i32
        let sig_ctx_4 = make_sig(
            module,
            &[ptr_ty, types::I32, types::I32, types::I32, types::I32],
        ); // fn(ctx, dst, a, b, c) -> i32
        let _sig_ctx_5 = make_sig(
            module,
            &[
                ptr_ty,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
            ],
        ); // fn(ctx, dst, a, b, c, d) -> i32
        let sig_ctx_6 = make_sig(
            module,
            &[
                ptr_ty,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
            ],
        ); // fn(ctx, dst, a, b, c, d, e) -> i32
        let sig_ctx_7 = make_sig(
            module,
            &[
                ptr_ty,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
                types::I32,
            ],
        ); // 7 i32 params
        // NPV/ArrayLiteral: fn(ctx, dst, rate, ptr, len) -> i32
        let sig_ctx_2_ptr_1 = make_sig(
            module,
            &[ptr_ty, types::I32, types::I32, ptr_ty, types::I32],
        );
        let sig_ctx_2_ptrptr_2 = make_sig(
            module,
            &[ptr_ty, types::I32, ptr_ty, ptr_ty, types::I32, types::I32],
        );
        let sig_ctx_2_ptr_2 = make_sig(
            module,
            &[ptr_ty, types::I32, types::I32, ptr_ty, types::I32],
        );
        let sig_ctx_1_ptr_2 = make_sig(
            module,
            &[ptr_ty, types::I32, ptr_ty, types::I32, types::I32],
        );
        // ValidateAssignment: fn(ctx, src, intent, kind, name_ptr, name_len, type_ptr, type_len) -> i32
        let sig_validate = make_sig(
            module,
            &[
                ptr_ty,
                types::I32,
                types::I32,
                types::I32,
                ptr_ty,
                types::I32,
                ptr_ty,
                types::I32,
            ],
        );

        let mut extras = HashMap::new();

        // Declare all phase 2-4 helpers with appropriate signatures
        let helper_sigs: &[(&str, &cranelift_codegen::ir::Signature)] = &[
            // VARIANT slot load/branch helpers
            ("oxrt_load_i32", &s_si),
            ("oxrt_load_bool", &s_si),
            ("oxrt_load_null", &sig_ctx_1),
            ("oxrt_jump_if_zero", &sig_ctx_1),
            // Phase 2: String ops (18)
            ("oxrt_len", &sig_ctx_2),
            ("oxrt_left", &sig_ctx_3),
            ("oxrt_right", &sig_ctx_3),
            ("oxrt_mid", &sig_ctx_4),
            ("oxrt_mid_stmt", &sig_ctx_4),
            ("oxrt_instr", &sig_ctx_4),
            ("oxrt_instrrev", &sig_ctx_4),
            ("oxrt_lower", &sig_ctx_2),
            ("oxrt_upper", &sig_ctx_2),
            ("oxrt_split", &sig_ctx_3),
            ("oxrt_join", &sig_ctx_3),
            ("oxrt_replace", &sig_ctx_4),
            ("oxrt_trim", &sig_ctx_2),
            ("oxrt_ltrim", &sig_ctx_2),
            ("oxrt_rtrim", &sig_ctx_2),
            ("oxrt_strcomp", &sig_ctx_4),
            ("oxrt_like", &sig_ctx_4),
            ("oxrt_strconv", &sig_ctx_3),
            // Phase 2: Char/format (8)
            ("oxrt_chr", &sig_ctx_2),
            ("oxrt_asc", &sig_ctx_2),
            ("oxrt_space", &sig_ctx_2),
            ("oxrt_string_repeat", &sig_ctx_3),
            ("oxrt_hex", &sig_ctx_2),
            ("oxrt_oct", &sig_ctx_2),
            ("oxrt_format", &sig_ctx_3),
            ("oxrt_strreverse", &sig_ctx_2),
            // Phase 2: Date/time (11)
            ("oxrt_date_serial", &sig_ctx_4),
            ("oxrt_time_serial", &sig_ctx_4),
            ("oxrt_date_value", &sig_ctx_2),
            ("oxrt_cdate", &sig_ctx_2),
            ("oxrt_time_value", &sig_ctx_2),
            ("oxrt_date_add", &sig_ctx_4),
            ("oxrt_date_diff", &sig_ctx_4),
            ("oxrt_year", &sig_ctx_2),
            ("oxrt_month", &sig_ctx_2),
            ("oxrt_day", &sig_ctx_2),
            ("oxrt_weekday", &sig_ctx_2),
            ("oxrt_month_name", &sig_ctx_2),
            // Phase 2: Math (8)
            ("oxrt_round", &sig_ctx_3),
            ("oxrt_sqr", &sig_ctx_2),
            ("oxrt_sin", &sig_ctx_2),
            ("oxrt_cos", &sig_ctx_2),
            ("oxrt_log", &sig_ctx_2),
            ("oxrt_exp", &sig_ctx_2),
            ("oxrt_atn", &sig_ctx_2),
            ("oxrt_tan", &sig_ctx_2),
            // Phase 2: Type checking (11)
            ("oxrt_vartype_tag", &sig_ctx_2),
            ("oxrt_vartype", &sig_ctx_2),
            ("oxrt_typename_tag", &sig_ctx_2),
            ("oxrt_is_numeric_tag", &sig_ctx_2),
            ("oxrt_is_numeric", &sig_ctx_2),
            ("oxrt_is_error", &sig_ctx_2),
            ("oxrt_is_date_tag", &sig_ctx_2),
            ("oxrt_is_object_tag", &sig_ctx_2),
            ("oxrt_is_null", &sig_ctx_2),
            ("oxrt_is_empty", &sig_ctx_2),
            ("oxrt_is_array_tag", &sig_ctx_2),
            // Phase 2: Financial (8)
            ("oxrt_fv", &sig_ctx_6),
            ("oxrt_pv", &sig_ctx_6),
            ("oxrt_pmt", &sig_ctx_6),
            ("oxrt_npv", &sig_ctx_2_ptr_1),
            ("oxrt_irr", &sig_ctx_3),
            ("oxrt_mirr", &sig_ctx_4),
            ("oxrt_rate", &sig_ctx_7),
            ("oxrt_nper", &sig_ctx_6),
            // Phase 2: Array (3)
            ("oxrt_array_literal", &sig_ctx_2_ptr_1),
            ("oxrt_array_append", &sig_ctx_3),
            ("oxrt_array_resize", &sig_ctx_2_ptrptr_2),
            ("oxrt_array_resize_preserve", &sig_ctx_2_ptrptr_2),
            ("oxrt_array_get", &sig_ctx_2_ptr_2),
            ("oxrt_array_set", &sig_ctx_1_ptr_2),
            ("oxrt_lbound", &sig_ctx_2),
            ("oxrt_ubound", &sig_ctx_2),
            // Phase 2: Collection (4)
            ("oxrt_collection_add", &sig_ctx_3),
            ("oxrt_collection_item", &sig_ctx_3),
            ("oxrt_collection_remove", &sig_ctx_3),
            ("oxrt_collection_count", &sig_ctx_2),
            // Phase 2: Random (2)
            ("oxrt_rnd", &sig_ctx_2),
            ("oxrt_randomize", &sig_ctx_2),
            // Phase 2: Assignment (1)
            ("oxrt_validate_assignment", &sig_validate),
            // Phase 3: Error handling
            ("oxrt_set_on_error_resume_next", &sig_ctx_only),
            ("oxrt_set_on_error_goto0", &sig_ctx_only),
            ("oxrt_set_on_error_goto_label", &sig_ctx_1),
            ("oxrt_load_err_number", &sig_ctx_1),
            ("oxrt_load_err_description", &sig_ctx_1),
            ("oxrt_load_err_source", &sig_ctx_1),
            ("oxrt_raise_error", &sig_ctx_2),
            ("oxrt_clear_err", &sig_ctx_only),
            ("oxrt_route_error", &sig_ctx_2), // fn(ctx, error_code, failing_pc) -> i32
            ("oxrt_resume", &sig_ctx_only),
            ("oxrt_resume_next", &sig_ctx_only),
            ("oxrt_resume_label", &sig_ctx_1),
            // Phase 4: File I/O
            ("oxrt_host_free_file", &sig_ctx_2),
            ("oxrt_host_file_open", &sig_ctx_4),
            ("oxrt_host_file_close", &sig_ctx_2),
            ("oxrt_host_file_kill", &sig_ctx_2),
            ("oxrt_host_file_read", &sig_ctx_3),
            ("oxrt_host_file_write", &sig_ctx_3),
            ("oxrt_host_file_print", &sig_ctx_3),
            ("oxrt_host_file_input", &sig_ctx_3),
            ("oxrt_host_file_line_input", &sig_ctx_2),
            ("oxrt_host_file_eof", &sig_ctx_2),
            ("oxrt_host_file_lof", &sig_ctx_2),
            ("oxrt_host_file_seek", &sig_ctx_2),
            ("oxrt_host_file_loc", &sig_ctx_2),
            ("oxrt_host_console_print", &sig_ctx_3),
            ("oxrt_host_console_input", &sig_ctx_3),
            ("oxrt_host_console_line_input", &sig_ctx_2),
            ("oxrt_host_beep", &sig_ctx_1),
            // Phase 4: COM
            ("oxrt_host_create_object", &sig_ctx_2),
            (
                "oxrt_host_dispatch_invoke",
                &make_sig(
                    module,
                    &[
                        ptr_ty,
                        types::I32,
                        types::I32,
                        types::I32,
                        ptr_ty,
                        types::I32,
                    ],
                ),
            ),
            ("oxrt_host_com_subscribe", &sig_ctx_3),
            ("oxrt_host_com_unsubscribe", &sig_ctx_2),
            ("oxrt_host_com_event_callback_sub", &sig_ctx_2),
            ("oxrt_host_com_event_callback_arg", &sig_ctx_3),
            ("oxrt_host_com_release_event_callback", &sig_ctx_2),
            // Phase 4: WithEvents
            ("oxrt_host_withevents_get", &sig_ctx_3),
            ("oxrt_host_withevents_set", &sig_ctx_4),
            ("oxrt_host_withevents_clear_owner", &sig_ctx_2),
            ("oxrt_host_withevents_first_owner", &sig_ctx_3),
            ("oxrt_host_withevents_next_owner", &sig_ctx_1),
            // Phase 4: UI
            ("oxrt_host_msgbox", &sig_ctx_3),
            ("oxrt_host_inputbox", &sig_ctx_3),
            ("oxrt_host_debug_print", &sig_ctx_3),
            ("oxrt_strptr", &sig_ctx_3),
            ("oxrt_varptr", &sig_ctx_3),
            ("oxrt_varptr_string_var", &sig_ctx_3),
            ("oxrt_varptr_variant_var", &sig_ctx_3),
            ("oxrt_objptr", &sig_ctx_3),
            ("oxrt_host_do_events", &sig_ctx_1),
            // Phase 4: Process/Env
            ("oxrt_host_shell", &sig_ctx_2),
            ("oxrt_host_environ", &sig_ctx_2),
            ("oxrt_host_dir", &sig_ctx_2),
            // Phase 4: Time
            ("oxrt_host_date_now", &sig_ctx_1),
            ("oxrt_host_time_now", &sig_ctx_1),
            ("oxrt_host_now", &sig_ctx_1),
            ("oxrt_host_timer", &sig_ctx_1),
            // Phase 4: DynLink
            (
                "oxrt_host_invoke_symbol",
                &make_sig(
                    module,
                    &[
                        ptr_ty,
                        types::I32,
                        types::I32,
                        types::I32,
                        ptr_ty,
                        types::I32,
                        ptr_ty,
                        types::I32,
                    ],
                ),
            ),
        ];

        for &(name, sig) in helper_sigs {
            let func_id = module
                .declare_function(name, Linkage::Import, sig)
                .map_err(map_err)?;
            extras.insert(name.to_string(), func_id);
        }

        Ok(Self {
            add_slots: module
                .declare_function("oxrt_add_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            sub_slots: module
                .declare_function("oxrt_sub_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            mul_slots: module
                .declare_function("oxrt_mul_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            div_slots: module
                .declare_function("oxrt_div_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            intdiv_slots: module
                .declare_function("oxrt_intdiv_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            mod_slots: module
                .declare_function("oxrt_mod_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            pow_slots: module
                .declare_function("oxrt_pow_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            neg_slot: module
                .declare_function("oxrt_neg_slot", Linkage::Import, &s2)
                .map_err(map_err)?,
            concat_slots: module
                .declare_function("oxrt_concat_slots", Linkage::Import, &s3)
                .map_err(map_err)?,
            add_const: module
                .declare_function("oxrt_add_const", Linkage::Import, &s_si)
                .map_err(map_err)?,
            sub_const: module
                .declare_function("oxrt_sub_const", Linkage::Import, &s_si)
                .map_err(map_err)?,
            inc_slot: module
                .declare_function("oxrt_inc_slot", Linkage::Import, &s1)
                .map_err(map_err)?,
            copy_slot: module
                .declare_function("oxrt_copy_slot", Linkage::Import, &s2)
                .map_err(map_err)?,
            abs: module
                .declare_function("oxrt_abs", Linkage::Import, &s2)
                .map_err(map_err)?,
            sgn: module
                .declare_function("oxrt_sgn", Linkage::Import, &s2)
                .map_err(map_err)?,
            int_fix: module
                .declare_function("oxrt_int_fix", Linkage::Import, &s2)
                .map_err(map_err)?,
            bool_not: module
                .declare_function("oxrt_bool_not", Linkage::Import, &s2)
                .map_err(map_err)?,
            bool_and: module
                .declare_function("oxrt_bool_and", Linkage::Import, &s3)
                .map_err(map_err)?,
            bool_or: module
                .declare_function("oxrt_bool_or", Linkage::Import, &s3)
                .map_err(map_err)?,
            cmp_eq: module
                .declare_function("oxrt_cmp_eq", Linkage::Import, &sc)
                .map_err(map_err)?,
            cmp_ne: module
                .declare_function("oxrt_cmp_ne", Linkage::Import, &sc)
                .map_err(map_err)?,
            cmp_lt: module
                .declare_function("oxrt_cmp_lt", Linkage::Import, &sc)
                .map_err(map_err)?,
            cmp_le: module
                .declare_function("oxrt_cmp_le", Linkage::Import, &sc)
                .map_err(map_err)?,
            cmp_gt: module
                .declare_function("oxrt_cmp_gt", Linkage::Import, &sc)
                .map_err(map_err)?,
            cmp_ge: module
                .declare_function("oxrt_cmp_ge", Linkage::Import, &sc)
                .map_err(map_err)?,
            load_string: module
                .declare_function("oxrt_load_string", Linkage::Import, &sls)
                .map_err(map_err)?,
            load_f64: module
                .declare_function("oxrt_load_f64", Linkage::Import, &sf)
                .map_err(map_err)?,
            extras,
        })
    }

    /// Get a helper function by name from the extras map.
    fn extra(&self, name: &str) -> cranelift_module::FuncId {
        *self
            .extras
            .get(name)
            .unwrap_or_else(|| panic!("missing helper: {name}"))
    }
}

fn emit_cmp_helper(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    ctx_ptr: cranelift_codegen::ir::Value,
    dst: usize,
    lhs: usize,
    rhs: usize,
    mode: StringCompareMode,
    _name: &str,
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) -> Result<(), String> {
    let func_id = match _name {
        "oxrt_cmp_eq" => helpers.cmp_eq,
        "oxrt_cmp_ne" => helpers.cmp_ne,
        "oxrt_cmp_lt" => helpers.cmp_lt,
        "oxrt_cmp_le" => helpers.cmp_le,
        "oxrt_cmp_gt" => helpers.cmp_gt,
        "oxrt_cmp_ge" => helpers.cmp_ge,
        _ => return Err(format!("unknown comparison helper: {_name}")),
    };
    let func_ref = module.declare_func_in_func(func_id, builder.func);
    let dst_val = builder.ins().iconst(types::I32, dst as i64);
    let lhs_val = builder.ins().iconst(types::I32, lhs as i64);
    let rhs_val = builder.ins().iconst(types::I32, rhs as i64);
    let mode_val = builder.ins().iconst(
        types::I32,
        match mode {
            StringCompareMode::Binary => 0,
            StringCompareMode::Text => 1,
        },
    );
    let call = builder
        .ins()
        .call(func_ref, &[ctx_ptr, dst_val, lhs_val, rhs_val, mode_val]);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
    Ok(())
}

// ── Emission helpers for Phase 2-4 instructions ───────────────────────

/// Emit a call to a 2-slot helper: fn(ctx, dst, src) -> i32
fn emit_extra_2slot(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    name: &str,
    ctx_ptr: cranelift_codegen::ir::Value,
    dst: usize,
    src: usize,
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) {
    let func_ref = module.declare_func_in_func(helpers.extra(name), builder.func);
    let dst_val = builder.ins().iconst(types::I32, dst as i64);
    let src_val = builder.ins().iconst(types::I32, src as i64);
    let call = builder.ins().call(func_ref, &[ctx_ptr, dst_val, src_val]);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
}

/// Emit a call with 2 i64 immediate args
fn emit_extra_2slot_i(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    name: &str,
    ctx_ptr: cranelift_codegen::ir::Value,
    a: i64,
    b: i64,
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) {
    let func_ref = module.declare_func_in_func(helpers.extra(name), builder.func);
    let a_val = builder.ins().iconst(types::I32, a);
    let b_val = builder.ins().iconst(types::I32, b);
    let call = builder.ins().call(func_ref, &[ctx_ptr, a_val, b_val]);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
}

/// Emit a call to a 3-slot helper: fn(ctx, dst, a, b) -> i32
fn emit_extra_3slot(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    name: &str,
    ctx_ptr: cranelift_codegen::ir::Value,
    dst: usize,
    a: usize,
    b: usize,
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) {
    let func_ref = module.declare_func_in_func(helpers.extra(name), builder.func);
    let dst_val = builder.ins().iconst(types::I32, dst as i64);
    let a_val = builder.ins().iconst(types::I32, a as i64);
    let b_val = builder.ins().iconst(types::I32, b as i64);
    let call = builder
        .ins()
        .call(func_ref, &[ctx_ptr, dst_val, a_val, b_val]);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
}

/// Emit a call with 3 i64 immediate args
fn emit_extra_3slot_i(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    name: &str,
    ctx_ptr: cranelift_codegen::ir::Value,
    a: i64,
    b: i64,
    c: i64,
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) {
    let func_ref = module.declare_func_in_func(helpers.extra(name), builder.func);
    let a_val = builder.ins().iconst(types::I32, a);
    let b_val = builder.ins().iconst(types::I32, b);
    let c_val = builder.ins().iconst(types::I32, c);
    let call = builder
        .ins()
        .call(func_ref, &[ctx_ptr, a_val, b_val, c_val]);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
}

/// Emit a call to a 4-slot helper: fn(ctx, dst, a, b, c) -> i32
fn emit_extra_4slot(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    name: &str,
    ctx_ptr: cranelift_codegen::ir::Value,
    dst: usize,
    a: usize,
    b: usize,
    c: i64,
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) {
    let func_ref = module.declare_func_in_func(helpers.extra(name), builder.func);
    let dst_val = builder.ins().iconst(types::I32, dst as i64);
    let a_val = builder.ins().iconst(types::I32, a as i64);
    let b_val = builder.ins().iconst(types::I32, b as i64);
    let c_val = builder.ins().iconst(types::I32, c);
    let call = builder
        .ins()
        .call(func_ref, &[ctx_ptr, dst_val, a_val, b_val, c_val]);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
}

/// Emit a call with 4 i64 immediate args
fn emit_extra_4slot_i(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    name: &str,
    ctx_ptr: cranelift_codegen::ir::Value,
    a: i64,
    b: i64,
    c: i64,
    d: i64,
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) {
    let func_ref = module.declare_func_in_func(helpers.extra(name), builder.func);
    let a_val = builder.ins().iconst(types::I32, a);
    let b_val = builder.ins().iconst(types::I32, b);
    let c_val = builder.ins().iconst(types::I32, c);
    let d_val = builder.ins().iconst(types::I32, d);
    let call = builder
        .ins()
        .call(func_ref, &[ctx_ptr, a_val, b_val, c_val, d_val]);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
}

/// Emit a call with N i32 args (from a slice of i64 values)
fn emit_extra_nslot_i(
    builder: &mut FunctionBuilder,
    module: &mut JITModule,
    helpers: &HelperFuncIds,
    name: &str,
    ctx_ptr: cranelift_codegen::ir::Value,
    args: &[i64],
    pc: usize,
    next_block: cranelift_codegen::ir::Block,
    error_dispatch_block: cranelift_codegen::ir::Block,
) {
    let func_ref = module.declare_func_in_func(helpers.extra(name), builder.func);
    let mut call_args: Vec<cranelift_codegen::ir::Value> = vec![ctx_ptr];
    for &arg in args {
        call_args.push(builder.ins().iconst(types::I32, arg));
    }
    let call = builder.ins().call(func_ref, &call_args);
    let rc = builder.inst_results(call)[0];
    let is_ok = builder.ins().icmp_imm(IntCC::Equal, rc, 0);
    let pc_imm = builder.ins().iconst(types::I32, pc as i64);
    let err_args = [BlockArg::Value(pc_imm), BlockArg::Value(rc)];
    builder
        .ins()
        .brif(is_ok, next_block, &[], error_dispatch_block, &err_args);
}

// ── Legacy path (unchanged) ───────────────────────────────────────────

pub fn execute_bytecode_legacy(bytecode: &Bytecode) -> Result<Vec<i32>, String> {
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
            Instruction::LoadConstBool { .. } => {
                return Err("unsupported bytecode for cranelift execution".to_string());
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
            Instruction::CmpEqSlots { dst, lhs, rhs, .. } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::Equal, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpNeSlots { dst, lhs, rhs, .. } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::NotEqual, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpLtSlots { dst, lhs, rhs, .. } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::SignedLessThan, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpLeSlots { dst, lhs, rhs, .. } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::SignedLessThanOrEqual, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpGtSlots { dst, lhs, rhs, .. } => {
                let lhs = read_slot(&mut builder, slots_ptr, *lhs)?;
                let rhs = read_slot(&mut builder, slots_ptr, *rhs)?;
                let pred = builder.ins().icmp(IntCC::SignedGreaterThan, lhs, rhs);
                let out = bool_to_i32(&mut builder, pred);
                write_slot(&mut builder, slots_ptr, *dst, out)?;
                builder.ins().jump(next_block, &[]);
            }
            Instruction::CmpGeSlots { dst, lhs, rhs, .. } => {
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
