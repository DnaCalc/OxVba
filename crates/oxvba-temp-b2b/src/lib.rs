//! `oxvba-temp-b2b` — a temporary bridge that lowers the legacy compiler's
//! bytecode/bundle (`oxvba_compiler`) into the clean [`oxvba_bundle::Bundle`] so
//! the existing front-end can drive `oxvba-vm2`. It is scaffolding for Phase 1
//! validation, to be deleted once the new front-end emits the clean bundle
//! directly.
//!
//! ## Totality
//! [`lower_instruction`] is an exhaustive `match` over every legacy
//! `Instruction` variant, so the translation is total by construction (a new
//! legacy opcode won't compile until mapped). Each legacy instruction maps to
//! exactly one clean `Op`, so the program-counter space is preserved 1:1 — every
//! `target_pc`/`entry_pc`/statement-boundary stays valid without relocation.
//!
//! ## Model
//! The legacy uses a flat absolute slot file shared across procedures; the clean
//! bundle is mapped onto that directly by making **all** legacy slots globals
//! (`global_count = slot_count`, every procedure `frame_slots = 0`). The 136
//! `Intrinsic*` library opcodes collapse to `CallNative { Builtin(NativeImplId) }`;
//! `IntrinsicDispatchInvokeHost` → `ComDispatch`; `IntrinsicInvokeSymbolHost` →
//! `Declare`; arrays/pointers/WithEvents map to the dedicated clean `Op`s. `New`
//! and `RaiseEvent` are already lowered to primitive sequences by the legacy
//! front-end, so the clean `classes`/`event_routes` tables stay empty here.
//!
//! ## Known limitation (documented, not silent)
//! The legacy binds `CallProc` arguments via call-site *metadata* (the VM copies
//! source→parameter slots and writes back on return), not via bytecode. This
//! bridge translates `CallProc` as a pure control transfer; **procedure argument
//! binding / return-value copy / ByRef writeback are not yet emitted**. Call-free
//! programs (and the full non-call instruction surface) round-trip correctly;
//! argument binding needs the per-procedure slot-remapping pass (mapping each
//! procedure's locals to the clean frame-relative model), which is the next step
//! of the bridge.

use std::collections::HashMap;

use oxvba_bundle as nb;
use oxvba_bundle::isa::{CallArg, NativeCallee, Op};
use oxvba_bundle::native::NativeImplId as N;
use oxvba_compiler::bundle::OxBundle;
use oxvba_compiler::bytecode::{self as lc, Bytecode, Instruction};

/// Lower a legacy `OxBundle` (bytecode + procedure metadata) to a clean bundle.
pub fn lower(bundle: &OxBundle) -> nb::Bundle {
    // Map procedure entry pcs → clean procedure indices (sorted by entry pc).
    let mut procs: Vec<&oxvba_compiler::emit::ProcedureRuntimeMetadata> =
        bundle.procedure_metadata.values().collect();
    procs.sort_by_key(|p| p.entry_pc);
    let entry_to_proc: HashMap<usize, usize> =
        procs.iter().enumerate().map(|(index, p)| (p.entry_pc, index)).collect();

    let mut out = lower_bytecode_with(&bundle.bytecode, &entry_to_proc);

    out.procedures = procs
        .iter()
        .map(|p| nb::ProcedureDescriptor {
            name: p.procedure_name.clone(),
            entry_pc: p.entry_pc,
            kind: if p.return_slot.is_some() {
                nb::ProcedureKind::Function
            } else {
                nb::ProcedureKind::Sub
            },
            param_count: p.param_slots.len(),
            // Flat-global model: procedures share the global slot file.
            frame_slots: 0,
            return_slot: None,
        })
        .collect();

    let mut statement_starts: Vec<usize> =
        procs.iter().flat_map(|p| p.statement_entry_pcs.iter().copied()).collect();
    statement_starts.sort_unstable();
    statement_starts.dedup();
    out.statement_starts = statement_starts;

    out
}

/// Lower a bare legacy `Bytecode` (no procedure metadata) — useful for tests and
/// call-free programs.
pub fn lower_bytecode(bytecode: &Bytecode) -> nb::Bundle {
    lower_bytecode_with(bytecode, &HashMap::new())
}

fn lower_bytecode_with(bytecode: &Bytecode, entry_to_proc: &HashMap<usize, usize>) -> nb::Bundle {
    let mut declare_writebacks: HashMap<u32, Vec<nb::ExternalCallWriteback>> = HashMap::new();
    let ops: Vec<Op> = bytecode
        .instructions
        .iter()
        .map(|inst| lower_instruction(inst, entry_to_proc, &mut declare_writebacks))
        .collect();
    let external_calls = bytecode
        .external_call_descriptors
        .iter()
        .map(|d| lower_external(d, &declare_writebacks))
        .collect();

    nb::Bundle {
        ops,
        procedures: Vec::new(),
        entry_pc: 0,
        global_count: bytecode.slot_count,
        entry_frame_slots: 0,
        statement_starts: Vec::new(),
        external_calls,
        source_map: Vec::new(),
        com_class_exports: Vec::new(),
        classes: Vec::new(),
        event_routes: Vec::new(),
    }
}

// ── Small mapping helpers ─────────────────────────────────────────────────────

fn s(slot: usize) -> CallArg {
    CallArg::Slot(slot)
}
fn optarg(slot: Option<usize>) -> CallArg {
    match slot {
        Some(slot) => CallArg::Slot(slot),
        None => CallArg::Omitted,
    }
}
fn mode_arg(mode: lc::StringCompareMode) -> CallArg {
    CallArg::Const(match mode {
        lc::StringCompareMode::Text => 1,
        lc::StringCompareMode::Binary => 0,
    })
}
fn native(dst: usize, id: N, args: Vec<CallArg>) -> Op {
    Op::CallNative { dst: Some(dst), callee: NativeCallee::Builtin(id), args }
}

fn map_compare(mode: lc::StringCompareMode) -> nb::StringCompareMode {
    match mode {
        lc::StringCompareMode::Binary => nb::StringCompareMode::Binary,
        lc::StringCompareMode::Text => nb::StringCompareMode::Text,
    }
}
fn map_coerce(target: lc::NumericCoerceTarget) -> nb::NumericCoerceTarget {
    match target {
        lc::NumericCoerceTarget::Byte => nb::NumericCoerceTarget::Byte,
        lc::NumericCoerceTarget::Integer => nb::NumericCoerceTarget::Integer,
        lc::NumericCoerceTarget::Long => nb::NumericCoerceTarget::Long,
        lc::NumericCoerceTarget::LongLong => nb::NumericCoerceTarget::LongLong,
    }
}
fn map_intent(intent: lc::RuntimeAssignmentIntent) -> nb::AssignmentIntent {
    match intent {
        lc::RuntimeAssignmentIntent::Implicit => nb::AssignmentIntent::Implicit,
        lc::RuntimeAssignmentIntent::Let => nb::AssignmentIntent::Let,
        lc::RuntimeAssignmentIntent::Set => nb::AssignmentIntent::Set,
    }
}
fn map_target_kind(kind: lc::RuntimeAssignmentTargetKind) -> nb::AssignmentTargetKind {
    match kind {
        lc::RuntimeAssignmentTargetKind::Variant => nb::AssignmentTargetKind::Variant,
        lc::RuntimeAssignmentTargetKind::Object => nb::AssignmentTargetKind::Object,
        lc::RuntimeAssignmentTargetKind::Scalar => nb::AssignmentTargetKind::Scalar,
    }
}
fn map_member_kind(kind: lc::ProjectMemberCallKind) -> nb::ProjectMemberKind {
    match kind {
        lc::ProjectMemberCallKind::Method => nb::ProjectMemberKind::Method,
        lc::ProjectMemberCallKind::PropertyGet => nb::ProjectMemberKind::PropertyGet,
        lc::ProjectMemberCallKind::PropertyLet => nb::ProjectMemberKind::PropertyLet,
        lc::ProjectMemberCallKind::PropertySet => nb::ProjectMemberKind::PropertySet,
    }
}
fn map_array_elem(t: lc::RuntimeArrayElementType) -> nb::ArrayElementType {
    use lc::RuntimeArrayElementType as L;
    use nb::ArrayElementType as B;
    match t {
        L::Variant => B::Variant,
        L::Integer => B::Integer,
        L::Long => B::Long,
        L::LongLong => B::LongLong,
        L::LongPtr => B::LongPtr,
        L::Byte => B::Byte,
        L::Single => B::Single,
        L::Double => B::Double,
        L::Currency => B::Currency,
        L::Date => B::Date,
        L::String => B::String,
        L::Boolean => B::Boolean,
    }
}
fn map_declare_param(t: lc::DeclareParamType) -> nb::DeclareParamType {
    use lc::DeclareParamType as L;
    use nb::DeclareParamType as B;
    match t {
        L::Long => B::Long,
        L::Integer => B::Integer,
        L::String => B::String,
        L::Boolean => B::Boolean,
        L::Double => B::Double,
        L::Single => B::Single,
        L::Currency => B::Currency,
        L::Date => B::Date,
        L::Byte => B::Byte,
        L::LongLong => B::LongLong,
        L::LongPtr => B::LongPtr,
        L::Variant => B::Variant,
        L::Any => B::Any,
    }
}
fn map_writeback_kind(kind: lc::ExternalCallWritebackKind) -> nb::ExternalCallWritebackKind {
    match kind {
        lc::ExternalCallWritebackKind::ByRefValue => nb::ExternalCallWritebackKind::ByRefValue,
        lc::ExternalCallWritebackKind::PointerByteArrayPayload => {
            nb::ExternalCallWritebackKind::PointerByteArrayPayload
        }
        lc::ExternalCallWritebackKind::PointerStringPayload => {
            nb::ExternalCallWritebackKind::PointerStringPayload
        }
    }
}

fn lower_external(
    d: &lc::ExternalCallDescriptor,
    writebacks: &HashMap<u32, Vec<nb::ExternalCallWriteback>>,
) -> nb::ExternalCallDescriptor {
    nb::ExternalCallDescriptor {
        descriptor_id: d.descriptor_id,
        declared_name: d.declared_name.clone(),
        library: d.library.clone(),
        alias: d.alias.clone(),
        ordinal_alias: d.ordinal_alias,
        symbol: d.symbol,
        marshal_lane: d.marshal_lane.clone(),
        calling_convention: d.calling_convention.clone(),
        selection_policy: d.selection_policy.clone(),
        param_count: d.param_count,
        param_types: d.param_types.iter().copied().map(map_declare_param).collect(),
        param_by_ref: d.param_by_ref.clone(),
        return_type: d.return_type.map(map_declare_param),
        writebacks: writebacks.get(&d.descriptor_id).cloned().unwrap_or_default(),
    }
}

/// Translate one legacy instruction to exactly one clean `Op`.
fn lower_instruction(
    inst: &Instruction,
    entry_to_proc: &HashMap<usize, usize>,
    declare_writebacks: &mut HashMap<u32, Vec<nb::ExternalCallWriteback>>,
) -> Op {
    match inst {
        // ── Loads / constants ──
        Instruction::LoadConstI32 { slot, value } => Op::LoadI32 { slot: *slot, value: *value },
        Instruction::LoadConstI64 { slot, value } => Op::LoadI64 { slot: *slot, value: *value },
        Instruction::LoadConstBool { slot, value } => Op::LoadBool { slot: *slot, value: *value },
        Instruction::LoadConstString { slot, value } => {
            Op::LoadString { slot: *slot, value: value.clone() }
        }
        Instruction::LoadConstF64 { slot, bits } => Op::LoadF64 { slot: *slot, bits: *bits },
        Instruction::LoadConstF32 { slot, bits } => Op::LoadF32 { slot: *slot, bits: *bits },
        Instruction::LoadConstCurrency { slot, scaled } => {
            Op::LoadCurrency { slot: *slot, scaled: *scaled }
        }
        Instruction::LoadConstDate { slot, bits } => Op::LoadDate { slot: *slot, bits: *bits },
        Instruction::LoadNull { slot } => Op::LoadNull { slot: *slot },
        Instruction::LoadEmpty { slot } => Op::LoadEmpty { slot: *slot },
        Instruction::LoadProjectObjectRef { dst, handle } => {
            Op::LoadProjectObjectRef { dst: *dst, handle: *handle }
        }
        Instruction::LoadErrNumber { slot } => Op::LoadErrNumber { slot: *slot },
        Instruction::LoadErrDescription { slot } => Op::LoadErrDescription { slot: *slot },
        Instruction::LoadErrSource { slot } => Op::LoadErrSource { slot: *slot },

        // ── Arithmetic ──
        Instruction::AddConstI32 { slot, value } => Op::AddConstI32 { slot: *slot, value: *value },
        Instruction::SubConstI32 { slot, value } => Op::SubConstI32 { slot: *slot, value: *value },
        Instruction::IncSlot { slot } => Op::IncSlot { slot: *slot },
        Instruction::AddSlots { dst, lhs, rhs } => Op::Add { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::SubSlots { dst, lhs, rhs } => Op::Sub { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::MulSlots { dst, lhs, rhs } => Op::Mul { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::DivSlots { dst, lhs, rhs } => Op::Div { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::IntDivSlots { dst, lhs, rhs } => Op::IntDiv { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::ModSlots { dst, lhs, rhs } => Op::Mod { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::PowSlots { dst, lhs, rhs } => Op::Pow { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::ConcatSlots { dst, lhs, rhs } => Op::Concat { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::NegSlot { dst, src } => Op::Neg { dst: *dst, src: *src },
        Instruction::CopySlot { dst, src } => Op::Copy { dst: *dst, src: *src },

        // ── Coercion / validation ──
        Instruction::CoerceNumeric { slot, target } => {
            Op::CoerceNumeric { slot: *slot, target: map_coerce(*target) }
        }
        Instruction::CoerceFixedString { slot, len } => {
            Op::CoerceFixedString { slot: *slot, len: *len }
        }
        Instruction::ValidateRuntimeAssignment {
            src,
            intent,
            target_kind,
            target_name,
            target_type_name,
        } => Op::ValidateAssignment {
            src: *src,
            intent: map_intent(*intent),
            target_kind: map_target_kind(*target_kind),
            target_name: target_name.clone(),
            target_type_name: target_type_name.clone(),
        },

        // ── Boolean ──
        Instruction::BoolNot { dst, src } => Op::Not { dst: *dst, src: *src },
        Instruction::BoolAnd { dst, lhs, rhs } => Op::And { dst: *dst, lhs: *lhs, rhs: *rhs },
        Instruction::BoolOr { dst, lhs, rhs } => Op::Or { dst: *dst, lhs: *lhs, rhs: *rhs },

        // ── Comparison ──
        Instruction::CmpEqSlots { dst, lhs, rhs, mode } => {
            Op::CmpEq { dst: *dst, lhs: *lhs, rhs: *rhs, mode: map_compare(*mode) }
        }
        Instruction::CmpNeSlots { dst, lhs, rhs, mode } => {
            Op::CmpNe { dst: *dst, lhs: *lhs, rhs: *rhs, mode: map_compare(*mode) }
        }
        Instruction::CmpLtSlots { dst, lhs, rhs, mode } => {
            Op::CmpLt { dst: *dst, lhs: *lhs, rhs: *rhs, mode: map_compare(*mode) }
        }
        Instruction::CmpLeSlots { dst, lhs, rhs, mode } => {
            Op::CmpLe { dst: *dst, lhs: *lhs, rhs: *rhs, mode: map_compare(*mode) }
        }
        Instruction::CmpGtSlots { dst, lhs, rhs, mode } => {
            Op::CmpGt { dst: *dst, lhs: *lhs, rhs: *rhs, mode: map_compare(*mode) }
        }
        Instruction::CmpGeSlots { dst, lhs, rhs, mode } => {
            Op::CmpGe { dst: *dst, lhs: *lhs, rhs: *rhs, mode: map_compare(*mode) }
        }
        Instruction::CmpObjectIsSlots { dst, lhs, rhs } => {
            Op::CmpObjectIs { dst: *dst, lhs: *lhs, rhs: *rhs }
        }

        // ── Control flow ──
        Instruction::Jump { target_pc } => Op::Jump { target_pc: *target_pc },
        Instruction::JumpIfZero { cond_slot, target_pc } => {
            Op::JumpIfZero { cond_slot: *cond_slot, target_pc: *target_pc }
        }
        Instruction::CallProc { target_pc, project_member } => Op::CallProc {
            proc: entry_to_proc.get(target_pc).copied().unwrap_or(0),
            // Argument binding / return / writeback is the documented bridge gap.
            dst: None,
            args: Vec::new(),
            member: project_member.as_ref().map(|m| nb::ProjectMemberCall {
                lowered_name: m.lowered_name.clone(),
                kind: map_member_kind(m.kind),
            }),
        },
        Instruction::Return => Op::Return,
        Instruction::Halt => Op::Halt,

        // ── Error state ──
        Instruction::SetOnErrorResumeNext => Op::SetOnErrorResumeNext,
        Instruction::SetOnErrorGoto0 => Op::SetOnErrorGoto0,
        Instruction::SetOnErrorGotoLabel { target_pc } => {
            Op::SetOnErrorGotoLabel { target_pc: *target_pc }
        }
        Instruction::ResumeNext => Op::ResumeNext,
        Instruction::Resume => Op::Resume,
        Instruction::ResumeLabel { target_pc } => Op::ResumeLabel { target_pc: *target_pc },
        Instruction::RaiseError { code } => Op::RaiseError { code: *code },
        Instruction::ClearErr => Op::ClearErr,

        // ── Arrays (dedicated clean ops) ──
        Instruction::IntrinsicArrayLiteral { dst, values } => {
            Op::ArrayLiteral { dst: *dst, values: values.clone() }
        }
        Instruction::IntrinsicArrayAppend { dst, array, item } => {
            Op::ArrayAppend { dst: *dst, array: *array, item: *item }
        }
        Instruction::IntrinsicArrayResize { dst, upper_bounds, lower_bounds, element_type } => {
            Op::ArrayResize {
                dst: *dst,
                upper_bounds: upper_bounds.clone(),
                lower_bounds: lower_bounds.clone(),
                element_type: map_array_elem(*element_type),
            }
        }
        Instruction::IntrinsicArrayResizePreserve {
            dst,
            upper_bounds,
            lower_bounds,
            element_type,
        } => Op::ArrayResizePreserve {
            dst: *dst,
            upper_bounds: upper_bounds.clone(),
            lower_bounds: lower_bounds.clone(),
            element_type: map_array_elem(*element_type),
        },
        Instruction::IntrinsicArrayGet { dst, array, indices } => {
            Op::ArrayGet { dst: *dst, array: *array, indices: indices.clone() }
        }
        Instruction::IntrinsicArraySet { array, indices, src } => {
            Op::ArraySet { array: *array, indices: indices.clone(), src: *src }
        }
        Instruction::IntrinsicLBoundArray { dst, src } => Op::LBound { dst: *dst, src: *src },
        Instruction::IntrinsicUBoundArray { dst, src } => Op::UBound { dst: *dst, src: *src },
        Instruction::IntrinsicForEachInit { iter, src } => {
            Op::ForEachInit { iter: *iter, src: *src }
        }
        Instruction::IntrinsicForEachNext { iter, item, has_value } => {
            Op::ForEachNext { iter: *iter, item: *item, has_value: *has_value }
        }

        // ── Pointers (dedicated clean ops) ──
        Instruction::IntrinsicStrPtr { dst, src } => Op::PtrStr { dst: *dst, src: *src },
        Instruction::IntrinsicVarPtr { dst, src } => Op::PtrVar { dst: *dst, src: *src },
        Instruction::IntrinsicVarPtrStringVar { dst, src } => {
            Op::PtrVarString { dst: *dst, src: *src }
        }
        Instruction::IntrinsicVarPtrVariantVar { dst, src } => {
            Op::PtrVarVariant { dst: *dst, src: *src }
        }
        Instruction::IntrinsicObjPtr { dst, src } => Op::PtrObj { dst: *dst, src: *src },

        // ── Type identity ──
        Instruction::IntrinsicTypeOfIs { dst, object_slot, type_name } => {
            Op::TypeOfIs { dst: *dst, object_slot: *object_slot, type_name: type_name.clone() }
        }
        Instruction::IntrinsicIsArrayTag { dst, src } => native(*dst, N::IsArray, vec![s(*src)]),
        Instruction::IntrinsicIsNull { dst, src } => native(*dst, N::IsNull, vec![s(*src)]),
        Instruction::IntrinsicIsEmpty { dst, src } => native(*dst, N::IsEmpty, vec![s(*src)]),
        Instruction::IntrinsicVarTypeTag { dst, src } => native(*dst, N::VarType, vec![s(*src)]),
        Instruction::IntrinsicVarType { dst, src } => native(*dst, N::VarType, vec![s(*src)]),
        Instruction::IntrinsicIsNumericTag { dst, src } => {
            native(*dst, N::IsNumeric, vec![s(*src)])
        }
        Instruction::IntrinsicIsNumeric { dst, src } => native(*dst, N::IsNumeric, vec![s(*src)]),
        Instruction::IntrinsicIsError { dst, src } => native(*dst, N::IsError, vec![s(*src)]),
        Instruction::IntrinsicIsDateTag { dst, src } => native(*dst, N::IsDate, vec![s(*src)]),
        Instruction::IntrinsicIsObjectTag { dst, src } => native(*dst, N::IsObject, vec![s(*src)]),
        Instruction::IntrinsicTypeNameTag { dst, src } => native(*dst, N::TypeName, vec![s(*src)]),

        // ── Strings ──
        Instruction::IntrinsicLenDigits { dst, src } => native(*dst, N::Len, vec![s(*src)]),
        Instruction::IntrinsicLeftDigits { dst, src, count } => {
            native(*dst, N::Left, vec![s(*src), s(*count)])
        }
        Instruction::IntrinsicRightDigits { dst, src, count } => {
            native(*dst, N::Right, vec![s(*src), s(*count)])
        }
        Instruction::IntrinsicMidDigits { dst, src, start, count } => {
            native(*dst, N::Mid, vec![s(*src), s(*start), optarg(*count)])
        }
        Instruction::IntrinsicMidStmtDigits { target, start, count, value } => {
            native(*target, N::MidStmt, vec![s(*target), s(*start), optarg(*count), s(*value)])
        }
        Instruction::IntrinsicInStrDigits { dst, haystack, needle, mode } => {
            native(*dst, N::InStr, vec![s(*haystack), s(*needle), mode_arg(*mode)])
        }
        Instruction::IntrinsicInStrRevDigits { dst, haystack, needle, mode } => {
            native(*dst, N::InStrRev, vec![s(*haystack), s(*needle), mode_arg(*mode)])
        }
        Instruction::IntrinsicLowerDigits { dst, src } => native(*dst, N::LCase, vec![s(*src)]),
        Instruction::IntrinsicUpperDigits { dst, src } => native(*dst, N::UCase, vec![s(*src)]),
        Instruction::IntrinsicSplitCountDigits { dst, src, delimiter } => {
            native(*dst, N::Split, vec![s(*src), s(*delimiter)])
        }
        Instruction::IntrinsicJoinDigits { dst, src, delimiter } => {
            native(*dst, N::Join, vec![s(*src), s(*delimiter)])
        }
        Instruction::IntrinsicReplaceDigits { dst, src, find, replace } => {
            native(*dst, N::Replace, vec![s(*src), s(*find), s(*replace)])
        }
        Instruction::IntrinsicTrimDigits { dst, src } => native(*dst, N::Trim, vec![s(*src)]),
        Instruction::IntrinsicLTrimDigits { dst, src } => native(*dst, N::LTrim, vec![s(*src)]),
        Instruction::IntrinsicRTrimDigits { dst, src } => native(*dst, N::RTrim, vec![s(*src)]),
        Instruction::IntrinsicStrCompDigits { dst, lhs, rhs, mode } => {
            native(*dst, N::StrComp, vec![s(*lhs), s(*rhs), mode_arg(*mode)])
        }
        Instruction::IntrinsicLikeDigits { dst, lhs, pattern, mode } => {
            native(*dst, N::Like, vec![s(*lhs), s(*pattern), mode_arg(*mode)])
        }
        Instruction::IntrinsicChrDigits { dst, src } => native(*dst, N::Chr, vec![s(*src)]),
        Instruction::IntrinsicAscDigits { dst, src } => native(*dst, N::Asc, vec![s(*src)]),
        Instruction::IntrinsicSpaceDigits { dst, count } => native(*dst, N::Space, vec![s(*count)]),
        Instruction::IntrinsicStringRepeatDigits { dst, count, ch } => {
            native(*dst, N::StringRepeat, vec![s(*count), s(*ch)])
        }
        Instruction::IntrinsicHexDigits { dst, src } => native(*dst, N::Hex, vec![s(*src)]),
        Instruction::IntrinsicOctDigits { dst, src } => native(*dst, N::Oct, vec![s(*src)]),
        Instruction::IntrinsicCStrDigits { dst, src } => native(*dst, N::CStr, vec![s(*src)]),
        Instruction::IntrinsicStrFuncDigits { dst, src } => native(*dst, N::Str, vec![s(*src)]),
        Instruction::IntrinsicValDigits { dst, src } => native(*dst, N::Val, vec![s(*src)]),
        Instruction::IntrinsicStrReverseDigits { dst, src } => {
            native(*dst, N::StrReverse, vec![s(*src)])
        }
        Instruction::IntrinsicStrConvDigits { dst, src, conversion } => {
            native(*dst, N::StrConv, vec![s(*src), s(*conversion)])
        }
        Instruction::IntrinsicFormatDigits { dst, value, format_string } => {
            native(*dst, N::Format, vec![s(*value), optarg(*format_string)])
        }

        // ── Date / time ──
        Instruction::IntrinsicDateSerialDigits { dst, year, month, day } => {
            native(*dst, N::DateSerial, vec![s(*year), s(*month), s(*day)])
        }
        Instruction::IntrinsicTimeSerialDigits { dst, hour, minute, second } => {
            native(*dst, N::TimeSerial, vec![s(*hour), s(*minute), s(*second)])
        }
        Instruction::IntrinsicDateValueDigits { dst, src } => {
            native(*dst, N::DateValue, vec![s(*src)])
        }
        Instruction::IntrinsicTimeValueDigits { dst, src } => {
            native(*dst, N::TimeValue, vec![s(*src)])
        }
        Instruction::IntrinsicDateAddDigits { dst, interval, number, date } => {
            native(*dst, N::DateAdd, vec![s(*interval), s(*number), s(*date)])
        }
        Instruction::IntrinsicDateDiffDigits { dst, interval, date1, date2 } => {
            native(*dst, N::DateDiff, vec![s(*interval), s(*date1), s(*date2)])
        }
        Instruction::IntrinsicYearDigits { dst, src } => native(*dst, N::Year, vec![s(*src)]),
        Instruction::IntrinsicMonthDigits { dst, src } => native(*dst, N::Month, vec![s(*src)]),
        Instruction::IntrinsicDayDigits { dst, src } => native(*dst, N::Day, vec![s(*src)]),
        Instruction::IntrinsicWeekdayDigits { dst, src } => native(*dst, N::Weekday, vec![s(*src)]),
        Instruction::IntrinsicMonthNameDigits { dst, src } => {
            native(*dst, N::MonthName, vec![s(*src)])
        }
        Instruction::IntrinsicDateNowHost { dst } => native(*dst, N::DateNow, vec![]),
        Instruction::IntrinsicTimeNowHost { dst } => native(*dst, N::TimeNow, vec![]),
        Instruction::IntrinsicNowHost { dst } => native(*dst, N::Now, vec![]),
        Instruction::IntrinsicTimerHost { dst } => native(*dst, N::Timer, vec![]),

        // ── Math ──
        Instruction::IntrinsicAbsI32 { dst, src } => native(*dst, N::Abs, vec![s(*src)]),
        Instruction::IntrinsicIntI32 { dst, src } => native(*dst, N::Int, vec![s(*src)]),
        Instruction::IntrinsicFixI32 { dst, src } => native(*dst, N::Fix, vec![s(*src)]),
        Instruction::IntrinsicSgnI32 { dst, src } => native(*dst, N::Sgn, vec![s(*src)]),
        Instruction::IntrinsicRoundI32 { dst, src, digits } => {
            native(*dst, N::Round, vec![s(*src), optarg(*digits)])
        }
        Instruction::IntrinsicSqrI32 { dst, src } => native(*dst, N::Sqr, vec![s(*src)]),
        Instruction::IntrinsicSinI32 { dst, src } => native(*dst, N::Sin, vec![s(*src)]),
        Instruction::IntrinsicCosI32 { dst, src } => native(*dst, N::Cos, vec![s(*src)]),
        Instruction::IntrinsicTanI32 { dst, src } => native(*dst, N::Tan, vec![s(*src)]),
        Instruction::IntrinsicLogI32 { dst, src } => native(*dst, N::Log, vec![s(*src)]),
        Instruction::IntrinsicExpI32 { dst, src } => native(*dst, N::Exp, vec![s(*src)]),
        Instruction::IntrinsicAtnI32 { dst, src } => native(*dst, N::Atn, vec![s(*src)]),
        Instruction::IntrinsicRndDigits { dst, seed } => native(*dst, N::Rnd, vec![optarg(*seed)]),
        Instruction::IntrinsicRandomizeDigits { dst, seed } => {
            native(*dst, N::Randomize, vec![optarg(*seed)])
        }

        // ── Financial ──
        Instruction::IntrinsicFvI32 { dst, rate, nper, pmt, pv, due } => {
            native(*dst, N::Fv, vec![s(*rate), s(*nper), s(*pmt), optarg(*pv), optarg(*due)])
        }
        Instruction::IntrinsicPvI32 { dst, rate, nper, pmt, fv, due } => {
            native(*dst, N::Pv, vec![s(*rate), s(*nper), s(*pmt), optarg(*fv), optarg(*due)])
        }
        Instruction::IntrinsicPmtI32 { dst, rate, nper, pv, fv, due } => {
            native(*dst, N::Pmt, vec![s(*rate), s(*nper), s(*pv), optarg(*fv), optarg(*due)])
        }
        Instruction::IntrinsicNpvI32 { dst, rate, values } => {
            let mut args = vec![s(*rate)];
            args.extend(values.iter().map(|v| s(*v)));
            native(*dst, N::Npv, args)
        }
        Instruction::IntrinsicIrrI32 { dst, value, guess } => {
            native(*dst, N::Irr, vec![s(*value), optarg(*guess)])
        }
        Instruction::IntrinsicMirrI32 { dst, value, finance_rate, reinvest_rate } => {
            native(*dst, N::Mirr, vec![s(*value), s(*finance_rate), s(*reinvest_rate)])
        }
        Instruction::IntrinsicRateI32 { dst, nper, pmt, pv, fv, due, guess } => native(
            *dst,
            N::Rate,
            vec![s(*nper), s(*pmt), s(*pv), optarg(*fv), optarg(*due), optarg(*guess)],
        ),
        Instruction::IntrinsicNPerI32 { dst, rate, pmt, pv, fv, due } => {
            native(*dst, N::NPer, vec![s(*rate), s(*pmt), s(*pv), optarg(*fv), optarg(*due)])
        }

        // ── Conversion ──
        Instruction::IntrinsicCDateValue { dst, src } => native(*dst, N::CDate, vec![s(*src)]),
        Instruction::IntrinsicCVErr { dst, src } => native(*dst, N::CVErr, vec![s(*src)]),

        // ── Collection ──
        Instruction::IntrinsicCollectionAdd { dst, count, item } => {
            native(*dst, N::CollectionAdd, vec![s(*count), s(*item)])
        }
        Instruction::IntrinsicCollectionItem { dst, count, index } => {
            native(*dst, N::CollectionItem, vec![s(*count), s(*index)])
        }
        Instruction::IntrinsicCollectionRemove { dst, count, index } => {
            native(*dst, N::CollectionRemove, vec![s(*count), s(*index)])
        }
        Instruction::IntrinsicCollectionCount { dst, count } => {
            native(*dst, N::CollectionCount, vec![s(*count)])
        }

        // ── File / console / UI / process / debug host ──
        Instruction::IntrinsicFreeFileHost { dst, range_selector } => {
            native(*dst, N::FreeFile, vec![optarg(*range_selector)])
        }
        Instruction::IntrinsicFileOpenHost { dst, path, mode, file_number } => {
            native(*dst, N::FileOpen, vec![s(*path), s(*mode), s(*file_number)])
        }
        Instruction::IntrinsicFileCloseHost { dst, handle } => {
            native(*dst, N::FileClose, vec![s(*handle)])
        }
        Instruction::IntrinsicFileKillHost { dst, path } => {
            native(*dst, N::FileKill, vec![s(*path)])
        }
        Instruction::IntrinsicFileReadHost { dst, handle, count } => {
            native(*dst, N::FileRead, vec![s(*handle), s(*count)])
        }
        Instruction::IntrinsicFileWriteHost { dst, handle, data } => {
            native(*dst, N::FileWrite, vec![s(*handle), s(*data)])
        }
        Instruction::IntrinsicFilePrintHost { dst, handle, data } => {
            native(*dst, N::FilePrint, vec![s(*handle), s(*data)])
        }
        Instruction::IntrinsicFileInputHost { dst, handle, count } => {
            native(*dst, N::FileInput, vec![s(*handle), s(*count)])
        }
        Instruction::IntrinsicFileLineInputHost { dst, handle } => {
            native(*dst, N::FileLineInput, vec![s(*handle)])
        }
        Instruction::IntrinsicFileEofHost { dst, handle } => {
            native(*dst, N::FileEof, vec![s(*handle)])
        }
        Instruction::IntrinsicFileLofHost { dst, handle } => {
            native(*dst, N::FileLof, vec![s(*handle)])
        }
        Instruction::IntrinsicFileSeekHost { dst, handle } => {
            native(*dst, N::FileSeek, vec![s(*handle)])
        }
        Instruction::IntrinsicFileLocHost { dst, handle } => {
            native(*dst, N::FileLoc, vec![s(*handle)])
        }
        Instruction::IntrinsicConsolePrintHost { dst, data } => {
            native(*dst, N::ConsolePrint, vec![s(*data)])
        }
        Instruction::IntrinsicConsoleInputHost { dst, count } => {
            native(*dst, N::ConsoleInput, vec![s(*count)])
        }
        Instruction::IntrinsicConsoleLineInputHost { dst } => {
            native(*dst, N::ConsoleLineInput, vec![])
        }
        Instruction::IntrinsicMsgBoxHost { dst, prompt, style } => {
            native(*dst, N::MsgBox, vec![s(*prompt), optarg(*style)])
        }
        Instruction::IntrinsicInputBoxHost { dst, prompt, default_value } => {
            native(*dst, N::InputBox, vec![s(*prompt), optarg(*default_value)])
        }
        Instruction::IntrinsicBeepHost { dst } => native(*dst, N::Beep, vec![]),
        Instruction::IntrinsicShellHost { dst, command } => {
            native(*dst, N::Shell, vec![s(*command)])
        }
        Instruction::IntrinsicEnvironHost { dst, key } => native(*dst, N::Environ, vec![s(*key)]),
        Instruction::IntrinsicDirHost { dst, path } => native(*dst, N::Dir, vec![s(*path)]),
        Instruction::IntrinsicCreateObjectHost { dst, prog_id } => {
            native(*dst, N::CreateObject, vec![s(*prog_id)])
        }
        Instruction::IntrinsicDoEventsHost { dst } => native(*dst, N::DoEvents, vec![]),
        Instruction::IntrinsicDebugPrintHost { dst, data } => {
            native(*dst, N::DebugPrint, vec![s(*data)])
        }

        // ── COM events (native bodies) ──
        Instruction::IntrinsicComSubscribeEventHost { dst, object, event } => {
            native(*dst, N::ComSubscribeEvent, vec![s(*object), s(*event)])
        }
        Instruction::IntrinsicComUnsubscribeEventHost { dst, subscription } => {
            native(*dst, N::ComUnsubscribeEvent, vec![s(*subscription)])
        }
        Instruction::IntrinsicComEventCallbackSubscriptionHost { dst, callback } => {
            native(*dst, N::ComEventCallbackSubscription, vec![s(*callback)])
        }
        Instruction::IntrinsicComEventCallbackArgHost { dst, callback, index } => {
            native(*dst, N::ComEventCallbackArg, vec![s(*callback), s(*index)])
        }
        Instruction::IntrinsicComReleaseEventCallbackHost { dst, callback } => {
            native(*dst, N::ComReleaseEventCallback, vec![s(*callback)])
        }

        // ── COM / late-bound member dispatch ──
        Instruction::IntrinsicDispatchInvokeHost {
            dst,
            object,
            member: _,
            args,
            early_bound,
            com_member,
            call_kind_hint,
        } => {
            let selector = match com_member {
                Some(cm) => match &cm.selector {
                    lc::ComMemberSelectorDescriptor::DispatchId(id) => {
                        nb::ComMemberSelector::DispatchId(*id)
                    }
                    lc::ComMemberSelectorDescriptor::Name(name) => {
                        nb::ComMemberSelector::Name(name.clone())
                    }
                },
                // A runtime member name (in the `member` slot) can't be carried as
                // a compile-time selector; bridged best-effort.
                None => nb::ComMemberSelector::DispatchId(0),
            };
            let mut call_args = vec![s(*object)];
            for arg in args {
                call_args.push(match (&arg.name, arg.slot) {
                    (Some(name), Some(slot)) => CallArg::Named { name: name.clone(), slot },
                    (None, Some(slot)) => CallArg::Slot(slot),
                    _ => CallArg::Omitted,
                });
            }
            Op::CallNative {
                dst: Some(*dst),
                callee: NativeCallee::ComDispatch {
                    selector,
                    early_bound: *early_bound,
                    kind_hint: call_kind_hint.map(map_member_kind),
                },
                args: call_args,
            }
        }

        // ── Declare Lib external call ──
        Instruction::IntrinsicInvokeSymbolHost {
            dst,
            descriptor_id,
            symbol: _,
            args,
            writeback_slots,
        } => {
            declare_writebacks.entry(*descriptor_id).or_insert_with(|| {
                writeback_slots
                    .iter()
                    .map(|w| nb::ExternalCallWriteback {
                        arg_index: w.arg_index,
                        source_slot: w.source_slot,
                        kind: map_writeback_kind(w.kind),
                    })
                    .collect()
            });
            Op::CallNative {
                dst: Some(*dst),
                callee: NativeCallee::Declare { descriptor_id: *descriptor_id },
                args: args.iter().map(|a| s(*a)).collect(),
            }
        }

        // ── WithEvents (dedicated clean ops) ──
        Instruction::IntrinsicWithEventsGet { dst, owner, binding } => {
            Op::WithEventsGet { dst: *dst, owner: *owner, binding: *binding }
        }
        Instruction::IntrinsicWithEventsSet { dst, owner, binding, value } => {
            Op::WithEventsSet { dst: *dst, owner: *owner, binding: *binding, value: *value }
        }
        Instruction::IntrinsicWithEventsClearOwner { dst, owner } => {
            Op::WithEventsClearOwner { dst: *dst, owner: *owner }
        }
        Instruction::IntrinsicWithEventsFirstOwner { dst, source, binding } => {
            Op::WithEventsFirstOwner { dst: *dst, source: *source, binding: *binding }
        }
        Instruction::IntrinsicWithEventsNextOwner { dst } => {
            Op::WithEventsNextOwner { dst: *dst }
        }
    }
}

#[cfg(test)]
mod tests;
