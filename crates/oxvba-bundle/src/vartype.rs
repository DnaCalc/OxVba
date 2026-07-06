//! `VarTypeRef` — the resolved static type of a VBA value, variable, parameter, or
//! expression.
//!
//! This is the binder's type lattice, but it lives here (the lowest crate the binder,
//! the symbol model, and the typed IR all depend on) so it is the **one** type truth:
//! `oxvba-symbol` resolves into it, the binder records it on the Core IR bindings it
//! produces ([`crate::coreir::CoreLocal`] etc.), and `oxvba-oxir` resolves it into its
//! `OxTy` lattice. It is name-based (objects/UDTs by folded name, not by id), so it
//! carries no symbol-model references and keeps this crate front-end-free — the same
//! discipline as the rest of the Core IR.

/// A VBA scalar/declared builtin type. Self-contained (not the runtime's
/// `VarType`, which carries COM/array encodings irrelevant to resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BuiltinType {
    Boolean,
    Byte,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Single,
    Double,
    Currency,
    Date,
    String,
}

/// One declared dimension of a fixed-size array. Single-bound array
/// declarations, including inline UDT fields, inherit their module's
/// `Option Base` in scanner metadata. Explicit `lower To upper` bounds preserve
/// their written lower bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct FixedArrayBound {
    pub lower: i32,
    pub len: usize,
}

/// A resolved type reference. `Object(name)` is the object-reference discriminator
/// the binder uses to pick early vs late COM dispatch. A known project class name
/// binds early; bare `Object` and foreign/COM object names bind late.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VarTypeRef {
    Builtin(BuiltinType),
    /// A typed object/class/coclass receiver, by (folded) type name.
    Object(String),
    /// A user-defined `Type` (UDT) value, by (folded) type name. A value aggregate
    /// (copied by value, fields accessed by index) — distinct from `Object` (a
    /// reference). Produced by the binder when an `Object`-named type resolves to a
    /// declared `Type`.
    Udt(String),
    /// Untyped `Variant` — forces late binding for member access.
    Variant,
    Array(Box<VarTypeRef>),
    /// A fixed-size array field inside a UDT, carrying its element type and total
    /// declared shape. This is distinct from `Array` because UDT fixed arrays are
    /// inline record payload, not a SAFEARRAY slot.
    FixedArray {
        element: Box<VarTypeRef>,
        bounds: Vec<FixedArrayBound>,
    },
    /// A fixed-length string `String * N` (length in characters). Behaves like
    /// `String` in the type lattice, but assignment pads/truncates to `N` and file
    /// records use exactly `N` ANSI bytes with no length prefix.
    FixedString(u32),
}
