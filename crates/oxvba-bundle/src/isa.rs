//! The primitive instruction set (`Op`) of the clean bundle.
//!
//! This is the "machine": loads, arithmetic, comparison, boolean, coercion,
//! control flow, error-state, array/aggregate primitives, with-events plumbing,
//! pointer helpers, object identity — plus exactly two call forms:
//! `CallProc` (a compiled VBA procedure) and `CallNative` (everything natively
//! implemented: the base-library built-ins, COM member dispatch, and `Declare`).
//!
//! There are no per-library opcodes. The 100+ legacy `Intrinsic*` library
//! opcodes are represented as `CallNative { callee: Builtin(NativeImplId), .. }`
//! (see `native.rs`); COM member invocation is `Builtin`-free
//! `NativeCallee::ComDispatch`; `Declare Lib` is `NativeCallee::Declare`.

use crate::native::NativeImplId;
use crate::{
    AssignmentIntent, AssignmentTargetKind, ArrayElementType, ComMemberSelector, NumericCoerceTarget,
    ProjectMemberCall, ProjectMemberKind, StringCompareMode,
};

/// What a `CallNative` targets — the dispatch route for a non-VBA-procedure call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCallee {
    /// A fixed base-library / built-in function with a native body.
    Builtin(NativeImplId),
    /// A member invocation on a COM / late-bound object. `selector` is a dispatch
    /// id (early-bound) or a member name (late-bound).
    ComDispatch {
        selector: ComMemberSelector,
        early_bound: bool,
        kind_hint: Option<ProjectMemberKind>,
    },
    /// A `Declare Lib` external call; the full marshalling descriptor lives in the
    /// bundle's `external_calls` table, keyed by `descriptor_id`.
    Declare { descriptor_id: u32 },
}

/// A single argument to a `CallNative`. Positional by default; `Omitted`
/// preserves alignment for trailing/optional VBA arguments; `Named` carries a
/// `name := value` argument for late-bound dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallArg {
    Slot(usize),
    Omitted,
    Named { name: String, slot: usize },
}

/// One instruction of the clean bundle.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    // ── Loads ────────────────────────────────────────────────
    LoadI32 { slot: usize, value: i32 },
    LoadI64 { slot: usize, value: i64 },
    LoadBool { slot: usize, value: bool },
    LoadString { slot: usize, value: String },
    LoadF64 { slot: usize, bits: u64 },
    LoadF32 { slot: usize, bits: u32 },
    LoadCurrency { slot: usize, scaled: i64 },
    LoadDate { slot: usize, bits: u64 },
    LoadNull { slot: usize },
    LoadEmpty { slot: usize },
    LoadProjectObjectRef { dst: usize, handle: usize },
    LoadErrNumber { slot: usize },
    LoadErrDescription { slot: usize },
    LoadErrSource { slot: usize },

    // ── Arithmetic ───────────────────────────────────────────
    AddConstI32 { slot: usize, value: i32 },
    SubConstI32 { slot: usize, value: i32 },
    IncSlot { slot: usize },
    Add { dst: usize, lhs: usize, rhs: usize },
    Sub { dst: usize, lhs: usize, rhs: usize },
    Mul { dst: usize, lhs: usize, rhs: usize },
    Div { dst: usize, lhs: usize, rhs: usize },
    IntDiv { dst: usize, lhs: usize, rhs: usize },
    Mod { dst: usize, lhs: usize, rhs: usize },
    Pow { dst: usize, lhs: usize, rhs: usize },
    Concat { dst: usize, lhs: usize, rhs: usize },
    Neg { dst: usize, src: usize },
    Copy { dst: usize, src: usize },

    // ── Coercion ─────────────────────────────────────────────
    CoerceNumeric { slot: usize, target: NumericCoerceTarget },
    CoerceFixedString { slot: usize, len: usize },
    ValidateAssignment {
        src: usize,
        intent: AssignmentIntent,
        target_kind: AssignmentTargetKind,
        target_name: String,
        target_type_name: String,
    },

    // ── Comparison ───────────────────────────────────────────
    CmpEq { dst: usize, lhs: usize, rhs: usize, mode: StringCompareMode },
    CmpNe { dst: usize, lhs: usize, rhs: usize, mode: StringCompareMode },
    CmpLt { dst: usize, lhs: usize, rhs: usize, mode: StringCompareMode },
    CmpLe { dst: usize, lhs: usize, rhs: usize, mode: StringCompareMode },
    CmpGt { dst: usize, lhs: usize, rhs: usize, mode: StringCompareMode },
    CmpGe { dst: usize, lhs: usize, rhs: usize, mode: StringCompareMode },
    CmpObjectIs { dst: usize, lhs: usize, rhs: usize },

    // ── Boolean ──────────────────────────────────────────────
    Not { dst: usize, src: usize },
    And { dst: usize, lhs: usize, rhs: usize },
    Or { dst: usize, lhs: usize, rhs: usize },

    // ── Control flow ─────────────────────────────────────────
    Jump { target_pc: usize },
    JumpIfZero { cond_slot: usize, target_pc: usize },
    CallProc { target_pc: usize, member: Option<ProjectMemberCall> },
    CallNative { dst: Option<usize>, callee: NativeCallee, args: Vec<CallArg> },
    Return,
    Halt,

    // ── Error state ──────────────────────────────────────────
    SetOnErrorResumeNext,
    SetOnErrorGoto0,
    SetOnErrorGotoLabel { target_pc: usize },
    ResumeNext,
    Resume,
    ResumeLabel { target_pc: usize },
    RaiseError { code: i32 },
    ClearErr,

    // ── Arrays / aggregates ──────────────────────────────────
    ArrayLiteral { dst: usize, values: Vec<usize> },
    ArrayAppend { dst: usize, array: usize, item: usize },
    ArrayResize { dst: usize, upper_bounds: Vec<usize>, lower_bounds: Vec<i32>, element_type: ArrayElementType },
    ArrayResizePreserve { dst: usize, upper_bounds: Vec<usize>, lower_bounds: Vec<i32>, element_type: ArrayElementType },
    ArrayGet { dst: usize, array: usize, indices: Vec<usize> },
    ArraySet { array: usize, indices: Vec<usize>, src: usize },
    LBound { dst: usize, src: usize },
    UBound { dst: usize, src: usize },
    ForEachInit { iter: usize, src: usize },
    ForEachNext { iter: usize, item: usize, has_value: usize },

    // ── Objects / with-events / type identity ────────────────
    WithEventsGet { dst: usize, owner: usize, binding: usize },
    WithEventsSet { dst: usize, owner: usize, binding: usize, value: usize },
    WithEventsClearOwner { dst: usize, owner: usize },
    WithEventsFirstOwner { dst: usize, source: usize, binding: usize },
    WithEventsNextOwner { dst: usize },
    TypeOfIs { dst: usize, object_slot: usize, type_name: String },

    // ── Pointer helpers ──────────────────────────────────────
    PtrStr { dst: usize, src: usize },
    PtrVar { dst: usize, src: usize },
    PtrVarString { dst: usize, src: usize },
    PtrVarVariant { dst: usize, src: usize },
    PtrObj { dst: usize, src: usize },
}
