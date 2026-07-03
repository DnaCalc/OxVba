//! The OxIR type lattice.
//!
//! Every OxIR local, place, temporary, parameter and value carries an [`OxTy`].
//! This is the static type information the binder already computes (`VarTypeRef`)
//! but the legacy `linearize` pass throws away — recovering it is the whole point
//! of OxIR. A type here is a **permission to take a fast path**, never an
//! obligation the runtime must satisfy: when in doubt the type is [`OxTy::Variant`]
//! and the runtime computes the real value (this mirrors the binder's
//! "safe over-approximation" rule and is what keeps OxIR faithful to the VM oracle).
//!
//! # Machine representation
//!
//! For every non-`Variant` type, the **unboxed machine value is chosen to be
//! bit-identical to the 8-byte payload that the matching `Variant` constructor
//! writes** (see `oxvba_runtime::variant`). That invariant is what makes the
//! `Box`/`Unbox` ops pure tag+payload moves the optimizer can fuse and cancel.
//!
//! | `OxTy` | Machine repr (register / slot) | Boxed payload (`VarType`) |
//! |---|---|---|
//! | `Bool` | `i8` in regs **(diverges: `i16` `-1`/`0` in a Variant)** | `Boolean` |
//! | `Byte` | `u8` | `Byte` |
//! | `Integer` | `i16` | `Integer` |
//! | `Long` | `i32` | `Long` |
//! | `LongLong` | `i64` | `LongLong` |
//! | `LongPtr` | target-width scalar lane; target-specific storage comes from binder metadata | `Long`/`LongLong` by target |
//! | `Single` | `f32` | `Single` |
//! | `Double` | `f64` | `Double` |
//! | `Currency` | `i64` (scaled ×10_000) | `Currency` |
//! | `Date` | `f64` (OLE serial) | `Date` |
//! | `Decimal` | 96-bit struct | `Decimal` |
//! | `Str` / `FixedStr(n)` | `*mut u16` (owned BSTR) | `String` |
//! | `Object(_)` | `*mut IUnknown` (typed) | `Object` |
//! | `Record(_)` | `*const RecordPayload` (value semantics) | `Record` |
//! | `Array(t, _)` | `*mut SAFEARRAY` | `ArrayVariant` |
//! | `ProcRef` | `usize` proc index | `ProcRef` |
//! | `Variant` | the 16-byte `#[repr(C)]` Variant | identity |
//!
//! **`Bool` is the one type whose box/unbox is *not* a pure move** (`i8` register
//! form vs `i16` `-1`/`0` Variant payload); it must be excluded from
//! identity-fusion. [`OxTy::box_unbox_is_identity`] encodes this.

use serde::{Deserialize, Serialize};

/// Index into the program's project-class table (a `New`-allocated instance type).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClassId(pub usize);

/// Index into the program's COM/interface table (a referenced COM interface or a
/// project `Implements` interface). The interface table carries the IID,
/// dual-vs-dispinterface kind, vtable base, and the member → typed-method-descriptor
/// map — the data that keeps COM calls typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IfaceId(pub usize);

/// Index into the program's UDT record-layout table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordLayoutId(pub usize);

/// The shape of a SAFEARRAY type: dynamic (`Dim a()`, `ReDim`-able) or
/// fixed-rank (`Dim a(1 To 3, 1 To 3)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArrayShape {
    /// A dynamic array — rank/bounds established (and re-established) by `ReDim`.
    Dynamic,
    /// A fixed-rank array with a statically known number of dimensions.
    Fixed { rank: u8 },
}

/// The interface identity carried by an object-typed value. An object type is a
/// **typed interface identity**, not a bare class name string — this is what lets
/// the JIT keep a typed interface pointer in a register and make typed member calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjClass {
    /// `Object`/`IDispatch`-only receiver → late-bound, dynamic dispatch.
    Untyped,
    /// A project class instance → internal typed dispatch.
    Class(ClassId),
    /// A project `Implements` interface → typed vtable-like dispatch.
    Iface(IfaceId),
    /// A referenced COM interface → typed vtable dispatch (QueryInterface by IID).
    ComIface(IfaceId),
}

/// The static type of an OxIR value, local, place, parameter or temporary.
///
/// `T <: Variant` for every `T`: [`OxTy::Variant`] is the dynamic top. There is no
/// other subtyping except that a typed object is-a untyped object
/// ([`ObjClass::Class`]/[`ObjClass::Iface`]/[`ObjClass::ComIface`] `<:`
/// [`ObjClass::Untyped`]). The numeric widening order is **not** modelled here — it
/// rides on `NumericMode` stamped on each arithmetic op.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OxTy {
    // ── Scalars — unboxed machine values, bit-identical to the Variant payload ──
    Bool,
    Byte,
    Integer,
    Long,
    LongLong,
    /// Target-width integer. OxIR itself carries no platform target, so target-specific
    /// storage decisions must arrive from binder/elaboration metadata.
    LongPtr,
    Single,
    Double,
    /// Scaled `i64` (×10_000).
    Currency,
    /// OLE `f64` serial.
    Date,
    /// 96-bit decimal (uses the Variant reserved words when boxed).
    Decimal,

    // ── Strings ──
    /// Owned BSTR (`*mut u16`).
    Str,
    /// `String * N` — same machine repr as [`OxTy::Str`]; `N` drives the store
    /// coercion (space-pad / truncate), it is not part of the runtime value.
    FixedStr(u32),

    // ── References / aggregates ──
    /// A typed interface pointer (`*mut IUnknown`); see [`ObjClass`].
    Object(ObjClass),
    /// A value-semantics UDT record (`*const RecordPayload`).
    Record(RecordLayoutId),
    /// A `SAFEARRAY` of element type `T`.
    Array(Box<OxTy>, ArrayShape),
    /// An `AddressOf` procedure reference.
    ProcRef,

    // ── The dynamic top ──
    /// The literal 16-byte `#[repr(C)]` Variant — the dynamic top type. Empty /
    /// Null / Error inhabit only this type; `Nothing` is an `Object` with a null
    /// pointer, and is therefore *not* a separate type.
    Variant,
}

impl OxTy {
    /// Whether this is the dynamic top type.
    pub fn is_variant(&self) -> bool {
        matches!(self, OxTy::Variant)
    }

    /// Whether the unboxed machine value is a pointer (BSTR / object / record /
    /// array / proc-ref). Such values cross the runtime ABI as an 8-byte pointer.
    pub fn is_pointer_repr(&self) -> bool {
        matches!(
            self,
            OxTy::Str
                | OxTy::FixedStr(_)
                | OxTy::Object(_)
                | OxTy::Record(_)
                | OxTy::Array(_, _)
                | OxTy::ProcRef
        )
    }

    /// Whether the value owns a heap resource that must be released / deep-cloned by
    /// VBA value semantics (drives the ownership/move analysis behind copy-elision).
    /// `Variant` is conservatively owning (it may hold any of these).
    pub fn is_owning(&self) -> bool {
        matches!(
            self,
            OxTy::Str
                | OxTy::FixedStr(_)
                | OxTy::Object(_)
                | OxTy::Record(_)
                | OxTy::Array(_, _)
                | OxTy::Variant
        )
    }

    /// Whether `Box`/`Unbox` between this type and `Variant` is a pure tag+payload
    /// move (so the optimizer may cancel `Unbox(Box(x)) == x`). This is true for
    /// every type **except [`OxTy::Bool`]**, whose register form (`i8`) differs from
    /// its Variant payload (`i16` `-1`/`0`), so its box/unbox carries a conversion.
    pub fn box_unbox_is_identity(&self) -> bool {
        !matches!(self, OxTy::Bool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_is_top() {
        assert!(OxTy::Variant.is_variant());
        assert!(!OxTy::Long.is_variant());
    }

    #[test]
    fn bool_box_unbox_is_not_identity() {
        // The one documented divergence (R-Box): i8 register form vs i16 -1/0 payload.
        assert!(!OxTy::Bool.box_unbox_is_identity());
        assert!(OxTy::Long.box_unbox_is_identity());
        assert!(OxTy::Str.box_unbox_is_identity());
    }

    #[test]
    fn pointer_and_owning_classification() {
        assert!(OxTy::Str.is_pointer_repr());
        assert!(OxTy::Object(ObjClass::Untyped).is_pointer_repr());
        assert!(OxTy::Array(Box::new(OxTy::Long), ArrayShape::Dynamic).is_pointer_repr());
        assert!(!OxTy::Long.is_pointer_repr());
        assert!(!OxTy::Variant.is_pointer_repr()); // 16-byte struct, not a pointer

        assert!(OxTy::Str.is_owning());
        assert!(OxTy::Variant.is_owning());
        assert!(!OxTy::Long.is_owning());
    }

    #[test]
    fn nested_array_element_type() {
        let t = OxTy::Array(
            Box::new(OxTy::Record(RecordLayoutId(3))),
            ArrayShape::Fixed { rank: 2 },
        );
        match t {
            OxTy::Array(el, ArrayShape::Fixed { rank }) => {
                assert_eq!(rank, 2);
                assert_eq!(*el, OxTy::Record(RecordLayoutId(3)));
            }
            _ => panic!("expected fixed-rank array"),
        }
    }
}
