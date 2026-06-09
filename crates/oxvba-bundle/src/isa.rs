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
    NumericMode, ProjectMemberKind, StringCompareMode,
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
    /// bundle's `external_calls` table, keyed by `descriptor_id`. `ptr_writebacks`
    /// are the pointer-helper arguments (`StrPtr(x)`/`VarPtr(x)` over an l-value)
    /// whose pinned buffer is read back into a caller slot after the call.
    Declare { descriptor_id: u32, ptr_writebacks: Vec<DeclarePtrWriteback> },
}

/// A resolved `Declare` pointer-argument write-back: after the native call, read the
/// pinned buffer at the `arg_index`-th argument's pointer and store it into
/// `target_slot`. `kind` selects the read-back projection (string vs byte buffer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclarePtrWriteback {
    pub arg_index: usize,
    pub target_slot: usize,
    pub kind: crate::coreir::PtrWritebackKind,
}

/// A single argument to a `CallNative`. Positional by default; `Omitted`
/// preserves alignment for trailing/optional VBA arguments; `Named` carries a
/// `name := value` argument for late-bound dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallArg {
    Slot(usize),
    /// A `ByRef` argument backed by the caller's slot `s`: the callee writes back
    /// to `s` (a true alias for a project-instance dispatch; a marshaled copy-out
    /// for `Declare`/COM). A non-l-value or parenthesised `(x)` argument lowers to
    /// `Slot`, never `ByRef`, so it gets no write-back (VBA temp semantics).
    ByRef(usize),
    Omitted,
    Named { name: String, slot: usize },
    /// A compile-time integer literal argument (e.g. an `Option Compare` mode
    /// threaded as a trailing argument to `InStr`/`StrComp`/`Like`).
    Const(i32),
}

/// A single argument to a `CallProc` (a compiled VBA procedure). VBA passes
/// `ByRef` by default: the callee operates on the caller's storage.
/// `ByRef(slot)` binds the parameter as a true alias of the caller's place —
/// writes through the parameter are immediately visible to the caller (and the
/// reverse), for the duration of the call. `ByVal(slot)` copies the value in;
/// `Omitted` leaves an optional parameter unset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcArg {
    ByVal(usize),
    ByRef(usize),
    Omitted,
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
    /// `AddressOf proc` — materialize a procedure reference (the procedure index)
    /// into `dst`. In-VM call-through and real OS-callback thunking are deferred to
    /// the native-runtime epic; the value marshals to a `Declare` as an integer.
    LoadProcRef { dst: usize, proc: usize },
    LoadErrNumber { slot: usize },
    LoadErrDescription { slot: usize },
    LoadErrSource { slot: usize },
    /// `Err.LastDllError` — the Win32 last-error captured after the most recent
    /// native `Declare` call (0 if none / non-native).
    LoadErrLastDllError { slot: usize },

    // ── Arithmetic ───────────────────────────────────────────
    AddConstI32 { slot: usize, value: i32 },
    SubConstI32 { slot: usize, value: i32 },
    IncSlot { slot: usize },
    // `mode` carries the numeric regime (the operands' promoted fixed type, or
    // `Widening` for a Variant operand) — the VM computes the result in that type and
    // raises Overflow (6) on out-of-range; see [`NumericMode`].
    Add { dst: usize, lhs: usize, rhs: usize, mode: NumericMode },
    Sub { dst: usize, lhs: usize, rhs: usize, mode: NumericMode },
    Mul { dst: usize, lhs: usize, rhs: usize, mode: NumericMode },
    Div { dst: usize, lhs: usize, rhs: usize },
    IntDiv { dst: usize, lhs: usize, rhs: usize, mode: NumericMode },
    Mod { dst: usize, lhs: usize, rhs: usize, mode: NumericMode },
    Pow { dst: usize, lhs: usize, rhs: usize },
    Concat { dst: usize, lhs: usize, rhs: usize },
    Neg { dst: usize, src: usize, mode: NumericMode },
    Copy { dst: usize, src: usize },

    // ── UDT records (value aggregates, backed by a record store) ───────────
    /// Allocate a default-initialized record of `fields` fields.
    NewRecord { dst: usize, fields: usize },
    /// Read field `index` of the record in slot `record`.
    RecordGet { dst: usize, record: usize, index: usize },
    /// Set field `index` of the record in slot `record` (updates the record in place).
    RecordSet { record: usize, index: usize, src: usize },

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
    Xor { dst: usize, lhs: usize, rhs: usize },
    Eqv { dst: usize, lhs: usize, rhs: usize },
    Imp { dst: usize, lhs: usize, rhs: usize },

    // ── Control flow ─────────────────────────────────────────
    Jump { target_pc: usize },
    JumpIfZero { cond_slot: usize, target_pc: usize },
    /// Call a compiled VBA procedure. `proc` indexes `Bundle::procedures` (which
    /// carries the callee's frame layout and return slot); `dst` receives the
    /// return value (`None` for a `Sub`); `args` are bound to the parameter slots
    /// per the copy-in/copy-out convention (see [`ProcArg`]).
    CallProc {
        proc: usize,
        dst: Option<usize>,
        args: Vec<ProcArg>,
    },
    CallNative { dst: Option<usize>, callee: NativeCallee, args: Vec<CallArg> },
    /// Call through a procedure reference (an `AddressOf` value) held in `target`,
    /// resolved to a procedure index at runtime. This is the VM-resident
    /// call-through that a real OS-callback trampoline re-enters; `args` bind to
    /// the callee frame exactly as [`Op::CallProc`].
    CallProcRef { dst: Option<usize>, target: usize, args: Vec<ProcArg> },
    /// A cross-bundle call (".NET assembly" reference). `import` indexes the
    /// current bundle's resolved import table (→ a `(bundle, proc)` pair); the
    /// callee runs in — and returns to — its own bundle. `args`/`dst` bind exactly
    /// as [`Op::CallProc`] (resolved in the caller's bundle before the switch).
    CallExtern { import: usize, dst: Option<usize>, args: Vec<ProcArg> },
    /// `CallByName` — dispatch a member call by a runtime name on a runtime object,
    /// routing through the same late-dispatch path as `CallNative`/`ComDispatch`.
    /// `object`/`name`/`calltype` are operand slots; `calltype` is the VbCallType
    /// (`vbMethod`=1/`vbGet`=2/`vbLet`=4/`vbSet`=8); `args` are the forwarded method
    /// arguments (the receiver is *not* in `args`).
    CallByName {
        dst: Option<usize>,
        object: usize,
        name: usize,
        calltype: usize,
        args: Vec<CallArg>,
    },
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

    // ── Project objects (instances + fields) ─────────────────
    /// Allocate a fresh instance of `class` (index into `Bundle::classes`), run
    /// its `Class_Initialize`, and store the object reference in `dst`. The
    /// instance is a refcounted IUnknown; release drives `Class_Terminate`.
    NewObject { dst: usize, class: usize },
    /// Allocate a fresh instance of a class in ANOTHER bundle (`import` indexes the
    /// current bundle's import table → a `(bundle, class)` pair); the instance
    /// carries that bundle's id so its methods dispatch against the right class
    /// table. Runs `Class_Initialize` in the owning bundle.
    NewExtern { dst: usize, import: usize },
    /// Load (creating on first access) the `VB_PredeclaredId` singleton of `class`
    /// (index into this bundle's `classes`) into `dst`. The instance persists for the
    /// whole run; `Class_Initialize` runs once, on creation. Unlike [`Op::NewObject`],
    /// repeated executions yield the *same* instance.
    PredeclaredInstance { dst: usize, class: usize },
    /// Load (creating on first access) the predeclared singleton of a class in
    /// ANOTHER bundle (`import` → a `(bundle, class)` pair); the instance carries that
    /// bundle's id so its members dispatch against the right class table. The
    /// cross-bundle analogue of [`Op::PredeclaredInstance`].
    PredeclaredInstanceExtern { dst: usize, import: usize },
    /// Read instance field `field` (a field token) of the object in `object`.
    FieldGet { dst: usize, object: usize, field: i32 },
    /// Write `src` into instance field `field` of the object in `object`.
    FieldSet { object: usize, field: i32, src: usize },
    /// Raise project event `event` from the source instance in `source`,
    /// dispatching to every subscribed `WithEvents` sink's handler procedure
    /// (resolved via the bundle's event routes). Arguments follow the event's
    /// `ByVal`/`ByRef` signature (see [`ProcArg`]).
    RaiseEvent { source: usize, event: i32, args: Vec<ProcArg> },

    // ── Pointer helpers ──────────────────────────────────────
    PtrStr { dst: usize, src: usize },
    PtrVar { dst: usize, src: usize },
    PtrVarString { dst: usize, src: usize },
    PtrVarVariant { dst: usize, src: usize },
    PtrObj { dst: usize, src: usize },
}
