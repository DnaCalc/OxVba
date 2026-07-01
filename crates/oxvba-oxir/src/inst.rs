//! The OxIR instruction set, terminators, and basic blocks.
//!
//! OxIR reuses ~90% of the proven `oxvba_bundle::isa::Op` vocabulary, but **typed,
//! block-structured, and with effects made explicit**. A block holds a list of
//! straight-line instructions plus a single control [`OxTerminator`]; fallible
//! instructions transfer to the block's `fault_target` landing pad on a raised
//! error (the explicit-fault-edge model that replaces the interpreter's implicit
//! per-op fault check and lowers cleanly to a branch on a returned status).
//!
//! COM dispatch is a first-class, *typed* pair of instructions
//! ([`OxInst::ComCallEarly`] / [`OxInst::ComCallLate`]) driven by the typed COM
//! interface + method-descriptor tables ([`crate::com`]) — **not** a `CallNative`
//! variant (which would be exactly the type-erasure this IR exists to undo).
//!
//! Also not yet modelled (added when a consumer needs them): host-injected object
//! handles (`isa::Op::LoadProjectObjectRef`) and the peephole
//! `AddConstI32`/`SubConstI32`/`IncSlot` ops (OxIR favours a general [`OxInst::Arith`]
//! and lets the optimizer introduce strength-reduced forms later).

use serde::{Deserialize, Serialize};

use oxvba_bundle::{
    ArrayElementType, AssignmentIntent, AssignmentTargetKind, NumericMode, StringCompareMode,
};

use oxvba_com::TypeLibMemberInvokeKind;

use crate::com::ComMethodRef;
use crate::ids::{BlockId, FuncId, ImportId};
use crate::ty::{ClassId, OxTy};
use crate::value::{
    ArithOp, BoundWhich, CmpOp, ErrField, LogicalOp, OxArg, OxCallArg, OxCoerceTarget,
    OxNativeCallee, OxOperand, OxPlace, PtrKind,
};

/// How an `On Error` statement sets the active handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorHandler {
    /// `On Error Resume Next`.
    ResumeNext,
    /// `On Error GoTo 0` — clear the handler.
    Goto0,
    /// `On Error GoTo -1` — clear the active-error latch and reset `Err`, but KEEP the
    /// current handler policy (so a re-armed handler can re-catch). Distinct from
    /// `Goto0`, which disables the handler.
    GotoMinus1,
    /// `On Error GoTo <label>` — the handler block.
    GotoLabel(BlockId),
}

/// One straight-line OxIR instruction. Whether an instruction can raise (and so
/// transfers to its block's `fault_target`) is given by [`OxInst::is_fallible`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OxInst {
    // ── Moves / representation ───────────────────────────────────────────────
    /// `dst := value` (a typed copy / load of a constant or place).
    Assign { dst: OxPlace, value: OxOperand },
    /// `dst := box(value : from)` — a pure typed→Variant representation change.
    Box {
        dst: OxPlace,
        src: OxOperand,
        from: OxTy,
    },
    /// `dst := unbox(value) : to` — Variant→typed. `checked` re-verifies the tag
    /// (fallible: type mismatch 13); unchecked is emitted only where the tag is
    /// statically proven.
    Unbox {
        dst: OxPlace,
        src: OxOperand,
        to: OxTy,
        checked: bool,
    },
    /// A value-changing coercion (narrowing with banker's rounding, fixed-string
    /// pad/truncate, …). Fallible: overflow (6) / type mismatch (13).
    Coerce {
        dst: OxPlace,
        src: OxOperand,
        target: OxCoerceTarget,
    },
    /// `dst := truthy(src)` — reduce an operand to a Boolean by VBA truthiness (the
    /// `is_truthy` rule a conditional uses: non-zero numeric → `True`, `""`/0 → `False`).
    /// Fallible (type mismatch 13 on a non-coercible operand, e.g. `If "abc" Then`). The
    /// elaboration emits this before a conditional [`OxTerminator::Branch`] whenever the
    /// condition is not already Boolean, so the branch condition is a **pre-computed
    /// Boolean** (the documented `Branch` invariant) and the truthiness fault routes
    /// through the block's normal fault pad like any other fallible instruction — keeping
    /// the terminator a pure control transfer.
    Truthy { dst: OxPlace, src: OxOperand },
    /// `Let`/`Set` legality check (diagnostic metadata; fallible).
    ValidateAssignment {
        src: OxOperand,
        intent: AssignmentIntent,
        target_kind: AssignmentTargetKind,
        target_name: String,
        target_type_name: String,
    },
    /// `AddressOf proc` — materialize a procedure reference into `dst`.
    LoadProcRef { dst: OxPlace, proc: FuncId },
    /// `dst := (current != original)` — VBA-`Variant`-equality change detection. Used to
    /// guard the copy-out of a compound `ByRef` argument: the caller snapshots the copied-in
    /// value (`original`) before the call and writes the (possibly-mutated) `current` temp
    /// back to the place ONLY when this reports a change. Mirrors vm2's `Op::VariantChanged`
    /// then `JumpIfZero` guard, so an unchanged compound `ByRef` param never clobbers an
    /// out-of-band mutation of the same place and never re-runs a `WithEvents` (un)subscribe.
    VariantChanged {
        dst: OxPlace,
        current: OxOperand,
        original: OxOperand,
    },

    // ── Arithmetic / comparison / logic ──────────────────────────────────────
    /// `Checked` arithmetic is fallible (overflow 6); `Widening` never raises.
    Arith {
        dst: OxPlace,
        op: ArithOp,
        lhs: OxOperand,
        rhs: OxOperand,
        mode: NumericMode,
    },
    /// Floating division (`/`) — always `f64`. Fallible: division by zero (11).
    Div {
        dst: OxPlace,
        lhs: OxOperand,
        rhs: OxOperand,
    },
    /// Exponentiation (`^`) — always `f64`.
    Pow {
        dst: OxPlace,
        lhs: OxOperand,
        rhs: OxOperand,
    },
    /// Unary negation (carries the numeric regime).
    Neg {
        dst: OxPlace,
        src: OxOperand,
        mode: NumericMode,
    },
    /// String concatenation (`&`).
    Concat {
        dst: OxPlace,
        lhs: OxOperand,
        rhs: OxOperand,
    },
    Compare {
        dst: OxPlace,
        op: CmpOp,
        lhs: OxOperand,
        rhs: OxOperand,
        mode: StringCompareMode,
    },
    /// Object identity (`Is`).
    CompareObjectIs {
        dst: OxPlace,
        lhs: OxOperand,
        rhs: OxOperand,
    },
    Logical {
        dst: OxPlace,
        op: LogicalOp,
        lhs: OxOperand,
        rhs: OxOperand,
    },
    Not {
        dst: OxPlace,
        src: OxOperand,
    },
    /// `TypeOf obj Is <type>`.
    TypeOfIs {
        dst: OxPlace,
        object: OxOperand,
        type_name: String,
    },

    // ── Calls (all fallible) ─────────────────────────────────────────────────
    /// Call a compiled VBA procedure (this unit).
    CallProc {
        dst: Option<OxPlace>,
        proc: FuncId,
        args: Vec<OxArg>,
    },
    /// Call through an `AddressOf` procedure reference held in `target`.
    CallProcRef {
        dst: Option<OxPlace>,
        target: OxOperand,
        args: Vec<OxArg>,
    },
    /// A cross-bundle call (resolved via the import table at link time).
    CallExtern {
        dst: Option<OxPlace>,
        import: ImportId,
        args: Vec<OxArg>,
    },
    /// A base-library built-in or `Declare Lib` external call.
    CallNative {
        dst: Option<OxPlace>,
        callee: OxNativeCallee,
        args: Vec<OxCallArg>,
    },
    /// `CallByName` — dispatch a member by a runtime name (genuinely dynamic).
    CallByName {
        dst: Option<OxPlace>,
        object: OxOperand,
        name: OxOperand,
        calltype: OxOperand,
        args: Vec<OxCallArg>,
    },
    /// **Early-bound, descriptor-typed COM call.** `method` names the typed member
    /// descriptor (an `oxvba_com::TypeLibMemberMetadata`, resolved via
    /// [`crate::program::OxProgram::com_method`]); `recv` is the interface receiver;
    /// `args` are typed per the descriptor's parameters (interface pointers, scalars,
    /// BSTR, SAFEARRAY / record per each param's wire shape). The hidden `[lcid]` and
    /// omitted-optional trailing arguments are **synthesized from the descriptor** at
    /// lowering/runtime — they are *not* present in `args`. Fallible: on `hr < 0` the
    /// block's `fault_target` receives control with `Err` populated from the rich
    /// HRESULT/EXCEPINFO mapping (not the flatten-to-5 default — the must-fix in M3).
    ///
    /// `invoke_kind` is the **call-site** accessor (a value read coerces to
    /// `PropertyGet`, a `Property Let`/`Set` to `PropertyPut`/`PropertyPutRef`). It
    /// equals the descriptor's own invoke-kind in the common case; the two diverge only
    /// for a member whose dispid is shared across get/put/putref where the binder
    /// resolved one variant in a context that wants another (a default-member read).
    /// It carries exactly the `kind_hint` the legacy Op path used, so vm3/JIT reproduce
    /// vm2's accessor selection — the descriptor gives the typed signature, this gives
    /// the syntactic accessor.
    ComCallEarly {
        dst: Option<OxPlace>,
        method: ComMethodRef,
        invoke_kind: TypeLibMemberInvokeKind,
        recv: OxOperand,
        args: Vec<OxCallArg>,
    },
    /// **Late-bound, by-name COM call** — the *only* dynamic COM path: a genuinely
    /// `Object`/`Variant` receiver dispatched by member `name` with Variant `args`
    /// (correct VBA late-binding semantics, not erasure). `invoke_kind` selects
    /// method / property-get / -put / -putref. Fallible like [`OxInst::ComCallEarly`].
    ComCallLate {
        dst: Option<OxPlace>,
        recv: OxOperand,
        name: String,
        invoke_kind: TypeLibMemberInvokeKind,
        args: Vec<OxCallArg>,
    },

    // ── Objects / fields / instances ─────────────────────────────────────────
    NewObject {
        dst: OxPlace,
        class: ClassId,
    },
    NewExtern {
        dst: OxPlace,
        import: ImportId,
    },
    Predeclared {
        dst: OxPlace,
        class: ClassId,
    },
    PredeclaredExtern {
        dst: OxPlace,
        import: ImportId,
    },
    NewRecord {
        dst: OxPlace,
        fields: Vec<ArrayElementType>,
    },
    /// Read instance field `field` (a resolved field token) of `object`. Fallible
    /// (a COM property accessor can raise).
    FieldGet {
        dst: OxPlace,
        object: OxOperand,
        field: i32,
    },
    FieldSet {
        object: OxOperand,
        field: i32,
        value: OxOperand,
    },
    /// Read positional field `index` of a UDT record (value aggregate).
    RecordGet {
        dst: OxPlace,
        record: OxOperand,
        index: usize,
    },
    RecordSet {
        record: OxPlace,
        index: usize,
        value: OxOperand,
    },

    // ── Arrays ───────────────────────────────────────────────────────────────
    ArrayLiteral {
        dst: OxPlace,
        values: Vec<OxOperand>,
        /// The array's first index (its `LBound`): `Option Base` for `Array()`,
        /// 0 for `ParamArray` boxing.
        lower_bound: i32,
    },
    ArrayAppend {
        dst: OxPlace,
        array: OxOperand,
        item: OxOperand,
    },
    /// `ReDim [Preserve]`.
    ArrayRedim {
        dst: OxPlace,
        upper_bounds: Vec<OxOperand>,
        lower_bounds: Vec<OxOperand>,
        element: ArrayElementType,
        preserve: bool,
        /// Whether the allocated array is fixed-size (set on the runtime
        /// SAFEARRAY's `FADF_FIXEDSIZE` bit); see [`super::OxInst::ArrayErase`].
        fixed: bool,
    },
    /// Array element read (fallible: subscript out of range 9; or default-member
    /// dispatch if the receiver is an object).
    ArrayGet {
        dst: OxPlace,
        array: OxOperand,
        indices: Vec<OxOperand>,
    },
    ArraySet {
        array: OxPlace,
        indices: Vec<OxOperand>,
        value: OxOperand,
    },
    /// Fused read of an element of an array held in `object`'s field `field` —
    /// `obj.field(i…)`. Reads the single element in place through the field's
    /// SAFEARRAY descriptor, so element access into a field-held array is O(1)
    /// instead of cloning the whole field array per access (which `FieldGet` +
    /// `ArrayGet` would do). Falls back to materialize-then-index when the field is
    /// not an array (e.g. an object whose default member is indexed).
    FieldArrayGet {
        dst: OxPlace,
        object: OxOperand,
        field: i32,
        indices: Vec<OxOperand>,
    },
    /// Fused write of an element of an array held in `object`'s field `field` —
    /// `obj.field(i…) = value`. The in-place counterpart to [`OxInst::FieldArrayGet`].
    FieldArraySet {
        object: OxOperand,
        field: i32,
        indices: Vec<OxOperand>,
        value: OxOperand,
    },
    ArrayErase {
        array: OxPlace,
        element: ArrayElementType,
    },
    Bound {
        dst: OxPlace,
        which: BoundWhich,
        array: OxOperand,
        dimension: Option<OxOperand>,
    },
    ForEachInit {
        iter: OxPlace,
        source: OxOperand,
    },
    ForEachNext {
        iter: OxPlace,
        item: OxPlace,
        has_value: OxPlace,
    },

    // ── WithEvents / events ──────────────────────────────────────────────────
    WithEventsGet {
        dst: OxPlace,
        owner: OxOperand,
        binding: i32,
    },
    WithEventsSet {
        dst: OxPlace,
        owner: OxOperand,
        binding: i32,
        value: OxOperand,
    },
    WithEventsClearOwner {
        dst: OxPlace,
        owner: OxOperand,
    },
    WithEventsFirstOwner {
        dst: OxPlace,
        source: OxOperand,
        binding: i32,
    },
    WithEventsNextOwner {
        dst: OxPlace,
    },
    /// Raise a project event to every subscribed `WithEvents` sink (fallible: a
    /// handler can raise).
    RaiseEvent {
        source: OxOperand,
        event: i32,
        args: Vec<OxArg>,
    },

    // ── Pointer helpers / Err fields ─────────────────────────────────────────
    Ptr {
        dst: OxPlace,
        kind: PtrKind,
        src: OxOperand,
    },
    ErrFieldGet {
        dst: OxPlace,
        field: ErrField,
    },
    ErrFieldSet {
        field: ErrField,
        src: OxOperand,
    },

    // ── Error-state setters (no control transfer) ────────────────────────────
    SetErrorHandler(ErrorHandler),
    ClearErr,

    // ── VBA-observable effects (explicit so the optimizer can't reorder/elide) ─
    /// Take a reference (IUnknown AddRef) — pins object liveness.
    AddRef { object: OxOperand },
    /// Release a reference; `may_terminate` carries the "world" effect because
    /// `Class_Terminate` can run arbitrary user code.
    Release {
        object: OxOperand,
        may_terminate: bool,
    },
    /// The start of a VBA source statement (drives `Resume` granularity and
    /// finalization timing). `stmt` is the source-statement index.
    StmtBoundary { stmt: u32 },
    /// Run parked `Class_Terminate`s to a fixpoint (pinned to statement boundaries
    /// / proc epilogue / the post-fault path).
    DrainTerminations,
}

impl OxInst {
    /// Whether this instruction can raise a VBA error and therefore transfers to its
    /// block's `fault_target` on failure. The verifier requires a `fault_target` for
    /// any block that contains a fallible instruction.
    ///
    /// This is a **safe over-approximation**: an op is treated as fallible unless it
    /// is *provably* pure. Under-approximating would be unsound — a faulting op in a
    /// block with no landing pad has nowhere to route `On Error`. Note in particular
    /// that even `Widening` arithmetic and `Compare`/`Logical`/`Not`/`Concat` can
    /// raise type mismatch (13) on a non-coercible operand (they go through the same
    /// `arith` functions that return `Result` in the interpreter), and `Bound` can
    /// raise subscript-out-of-range (9) on a non-array.
    pub fn is_fallible(&self) -> bool {
        match self {
            // Provably pure — cannot raise a VBA run-time error.
            OxInst::Assign { .. }
            | OxInst::Box { .. }
            | OxInst::LoadProcRef { .. }
            | OxInst::VariantChanged { .. }
            | OxInst::CompareObjectIs { .. }
            | OxInst::TypeOfIs { .. }
            | OxInst::ErrFieldGet { .. }
            | OxInst::SetErrorHandler(_)
            | OxInst::ClearErr
            | OxInst::StmtBoundary { .. }
            | OxInst::Ptr { .. }
            | OxInst::AddRef { .. }
            // `Release` only enqueues; the user code in `Class_Terminate` runs at
            // `DrainTerminations`, whose error handling is bespoke (not the fault
            // edge), so neither routes through `fault_target`.
            | OxInst::Release { .. }
            | OxInst::DrainTerminations
            | OxInst::NewRecord { .. }
            | OxInst::WithEventsClearOwner { .. }
            | OxInst::WithEventsFirstOwner { .. }
            | OxInst::WithEventsNextOwner { .. } => false,

            // `Unbox` is fallible only when it re-checks the tag.
            OxInst::Unbox { checked, .. } => *checked,

            // Everything else may raise (coercion, all arithmetic/comparison/logic,
            // division, calls, COM/field accessors, array access, object
            // construction running `Class_Initialize`, events, `For Each` over a COM
            // enumerator, …).
            _ => true,
        }
    }
}

/// The control-flow terminator that ends every basic block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OxTerminator {
    /// Unconditional branch.
    Jump(BlockId),
    /// Conditional branch. `cond` is a **pre-computed** Boolean/numeric-truthy
    /// operand: any fallible truthiness coercion (e.g. of a `String` condition) is a
    /// preceding fallible instruction in the block, so the branch itself never
    /// faults — keeping the "terminators are pure control transfers" invariant that
    /// makes the fault model sound.
    Branch {
        cond: OxOperand,
        then_blk: BlockId,
        else_blk: BlockId,
    },
    /// Return from the procedure (the return value is the function's return local).
    Return,
    /// End top-level execution.
    Halt,
    /// The **landing pad** of a faulting statement: a block whose `fault_target`
    /// edges from that statement's fallible instructions all converge here. Control
    /// arrives after the runtime has populated `Err`. The pad records the resume
    /// target `(resume, resume_next)` and dispatches on the active error mode:
    /// - no active handler → propagate the error as an early return;
    /// - `On Error Resume Next` → continue at `resume_next`;
    /// - `On Error GoTo h` → transfer to the (runtime-held) handler block `h`.
    ///
    /// `resume` is the faulting statement's own start block (the seed a later
    /// `Resume` jumps to); `resume_next` is the following statement's start block (the
    /// seed for `Resume Next`, and the direct target when the mode is `Resume Next`).
    /// The handler block is not a static operand — it lives in the runtime error-mode
    /// cell set by [`OxInst::SetErrorHandler`].
    FaultDispatch {
        resume: BlockId,
        resume_next: BlockId,
    },
    /// `Resume` — re-enter the faulting statement (jumps to the resume target's
    /// `resume` block, captured by the [`OxTerminator::FaultDispatch`] that fired).
    Resume,
    /// `Resume Next` — continue past the faulting statement (the resume target's
    /// `resume_next` block).
    ResumeNext,
    /// `Resume <label>`.
    ResumeLabel(BlockId),
    /// `Err.Raise Number[, Source][, Description]` / `Error <n>`. `number` is the error
    /// code operand (a `Const` when statically known); `source`/`description` are the
    /// optional explicit fields — a missing one falls back to the project name /
    /// the standard message for the number at raise time.
    ///
    /// `inherit` selects omitted-argument semantics: `true` (`Err.Raise`) inherits the
    /// un-cleared `Err` field per MS-VBAL §9071; `false` (the legacy `Error <n>`
    /// statement) always takes the defaults (oracle-confirmed — `Error <n>` does not
    /// inherit).
    Raise {
        number: OxOperand,
        source: Option<OxOperand>,
        description: Option<OxOperand>,
        inherit: bool,
    },
    /// `GoSub <label>` — push a return point and branch.
    GoSub { target: BlockId, ret: BlockId },
    /// `Return` from the most recent `GoSub`.
    GoSubReturn,
    /// Statically unreachable (e.g. after an unconditional `Exit`).
    Unreachable,
}

impl OxTerminator {
    /// Whether ending a block with this terminator can raise a VBA run-time error and
    /// must therefore route through the enclosing statement's fault pad (so `On Error`
    /// can catch it). `Branch` is a pure transfer (its condition is a pre-computed
    /// operand) and `Jump`/`Return`/`GoSub`/`FaultDispatch`/… are control moves; the
    /// fallible terminators are the raises plus the resume/return forms that can fail:
    /// - `Raise` — `Err.Raise` / `Error n`;
    /// - `Resume`/`ResumeNext`/`ResumeLabel` — error 20 ("Resume without error") when
    ///   there is no active error (MS-VBAL §5.4.4.2);
    /// - `GoSubReturn` — error 3 ("Return without GoSub") on an empty resumption list
    ///   (MS-VBAL §5.4.2.15).
    pub fn is_fallible(&self) -> bool {
        matches!(
            self,
            OxTerminator::Raise { .. }
                | OxTerminator::Resume
                | OxTerminator::ResumeNext
                | OxTerminator::ResumeLabel(_)
                | OxTerminator::GoSubReturn
        )
    }
}

/// One basic block: a list of straight-line instructions, an optional fault landing
/// pad for the fallible ones, and a single control terminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxBlock {
    pub id: BlockId,
    pub instrs: Vec<OxInst>,
    /// The landing pad that this block's fallible instructions transfer to on a
    /// raised error (the enclosing statement's handler-dispatch block). Required
    /// when any instruction in `instrs` is fallible.
    pub fault_target: Option<BlockId>,
    pub terminator: OxTerminator,
}

impl OxBlock {
    /// A block with no instructions and the given terminator.
    pub fn new(id: BlockId, terminator: OxTerminator) -> Self {
        Self {
            id,
            instrs: Vec::new(),
            fault_target: None,
            terminator,
        }
    }
}

/// The successor blocks named by a terminator (for CFG traversal / verification).
/// Note: this does **not** include `fault_target` (a block property) nor the
/// runtime-resolved `Resume`/`GoSubReturn` targets. For
/// [`OxTerminator::FaultDispatch`] both `resume` and `resume_next` are reported (both
/// are blocks the fault/resume machinery can transfer to); the *runtime-held* handler
/// block is dynamic and therefore not a static successor (like `Resume`'s target).
pub fn terminator_successors(t: &OxTerminator) -> Vec<BlockId> {
    match t {
        OxTerminator::Jump(b) | OxTerminator::ResumeLabel(b) => vec![*b],
        OxTerminator::Branch {
            then_blk, else_blk, ..
        } => vec![*then_blk, *else_blk],
        OxTerminator::GoSub { target, ret } => vec![*target, *ret],
        OxTerminator::FaultDispatch {
            resume,
            resume_next,
        } => vec![*resume, *resume_next],
        OxTerminator::Return
        | OxTerminator::Halt
        | OxTerminator::Resume
        | OxTerminator::ResumeNext
        | OxTerminator::Raise { .. }
        | OxTerminator::GoSubReturn
        | OxTerminator::Unreachable => Vec::new(),
    }
}

/// The operands this terminator reads (for use/liveness checks). `Branch` reads its
/// condition; `Raise` reads its number plus any explicit Source/Description operands;
/// every other terminator is a pure control transfer.
pub fn terminator_operands(t: &OxTerminator) -> Vec<&OxOperand> {
    match t {
        OxTerminator::Branch { cond, .. } => vec![cond],
        OxTerminator::Raise {
            number,
            source,
            description,
            ..
        } => {
            let mut ops = vec![number];
            ops.extend(source.iter());
            ops.extend(description.iter());
            ops
        }
        _ => Vec::new(),
    }
}
