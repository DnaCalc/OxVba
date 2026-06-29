//! Operands, places, constants, and the small operator enums.
//!
//! OxIR keeps a slot-like value model — simple **places** (locals/globals/temps)
//! that ops read and write — exactly like the proven `oxvba_bundle::isa` machine,
//! but typed. Complex/fallible accesses (a COM field, an array element, a record
//! field) are *not* hidden inside an operand; they are explicit instructions
//! ([`crate::inst::OxInst`]) that read into / write out of a simple place. This
//! mirrors the bundle ISA and keeps effects and faults visible.

use serde::{Deserialize, Serialize};

use crate::ids::{GlobalId, LocalId, TempId};

/// A typed literal. Floats/date/currency are stored as bit patterns so the IR stays
/// `Eq`/`Hash` and round-trips losslessly (mirrors `oxvba_bundle::coreir::CoreConst`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OxConst {
    Empty,
    Null,
    Nothing,
    Bool(bool),
    I32(i32),
    I64(i64),
    /// IEEE-754 `f32` bit pattern.
    F32(u32),
    /// IEEE-754 `f64` bit pattern.
    F64(u64),
    /// Currency scaled ×10_000.
    Currency(i64),
    /// `Date` serial as an `f64` bit pattern.
    Date(u64),
    Str(String),
}

/// A simple, directly-addressable place: a local, a global, or a temporary. These
/// are the read/write targets of ops; everything else (fields, array elements,
/// record fields, WithEvents sinks) is reached through an explicit instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OxPlace {
    Local(LocalId),
    Global(GlobalId),
    Temp(TempId),
}

/// An operand: a constant, or a pure read of a simple place.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OxOperand {
    Const(OxConst),
    Use(OxPlace),
}

impl OxOperand {
    /// Convenience: a `Use` of a local.
    pub fn local(id: LocalId) -> Self {
        OxOperand::Use(OxPlace::Local(id))
    }
    /// Convenience: a `Use` of a temporary.
    pub fn temp(id: TempId) -> Self {
        OxOperand::Use(OxPlace::Temp(id))
    }
}

/// Binary arithmetic operators that carry a `NumericMode` (the `Checked`/`Widening`
/// regime). `Div`/`Pow` are separate instructions because they are always `f64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    IntDiv,
    Mod,
}

/// Ordered/equality comparison operators (carry a `StringCompareMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Bitwise/logical operators (VBA `And`/`Or`/… are bitwise on numbers, logical on
/// Booleans — the runtime decides by operand type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogicalOp {
    And,
    Or,
    Xor,
    Eqv,
    Imp,
}

/// Where an explicit value-changing coercion lands. Pure representation changes
/// (typed ↔ Variant) are [`crate::inst::OxInst::Box`]/[`crate::inst::OxInst::Unbox`],
/// not coercions. `ImplicitVariant` widens to the [`crate::ty::OxTy`] `Variant` top
/// type without changing the value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OxCoerceTarget {
    Numeric(oxvba_bundle::NumericCoerceTarget),
    Str,
    FixedStr(u32),
    ImplicitVariant,
}

/// An argument to a compiled VBA procedure call (`CallProc`/`CallProcRef`/
/// `CallExtern`). VBA passes `ByRef` by default — a true alias of the caller's
/// place; `ByVal` copies the value in; `Omitted` leaves an optional parameter unset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OxArg {
    ByVal(OxOperand),
    ByRef(OxPlace),
    Omitted,
}

/// An argument to a `CallNative` / `CallByName` (the late/native dispatch path).
/// `ByRef` carries a place for marshaled copy-out; `Named` carries a `name := value`
/// argument; `Const` is a compile-time integer threaded as a trailing argument
/// (e.g. an `Option Compare` mode).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OxCallArg {
    Operand(OxOperand),
    ByRef(OxPlace),
    Omitted,
    Named { name: String, value: OxOperand },
    Const(i32),
}

/// What a `CallNative` targets. COM dispatch is **not** here — it becomes a
/// first-class, typed instruction added with the typed COM interface +
/// method-descriptor tables (next sub-section); `CallNative` covers only the
/// base-library built-ins and `Declare Lib` external calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OxNativeCallee {
    /// A base-library built-in with a native body.
    Builtin(oxvba_bundle::NativeImplId),
    /// A `Declare Lib` external call; the marshalling descriptor lives in the
    /// program's `external_calls` table, keyed by `descriptor_id`.
    Declare {
        descriptor_id: u32,
        ptr_writebacks: Vec<DeclarePtrWriteback>,
    },
}

/// A resolved `Declare` pointer-argument write-back. The typed analogue of
/// `oxvba_bundle::isa::DeclarePtrWriteback`: same concept, but the target is a typed
/// [`OxPlace`] rather than the erased flat slot index (genuinely different data, not a
/// serde-driven copy). The `kind` projection is reused from `coreir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarePtrWriteback {
    pub arg_index: usize,
    pub target: OxPlace,
    pub kind: PtrWritebackKind,
}

// ── Small leaf enums — reused from `oxvba_bundle::coreir` (the one truth) ──────
//
// `BoundWhich` (`LBound`/`UBound`), `PtrKind` (`VarPtr`/`StrPtr`/…), `ErrField`
// (`Err.Number`/…) and `PtrWritebackKind` (a `Declare` pointer write-back's read-back
// projection) are defined once in `coreir` and reused here verbatim — OxIR does not
// re-model them. They carry `serde` (added on the canonical definitions), so they
// serialize into the `.oxi` image directly.
pub use oxvba_bundle::coreir::{BoundWhich, ErrField, PtrKind, PtrWritebackKind};
