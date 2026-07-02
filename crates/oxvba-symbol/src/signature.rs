//! VBA procedure/member signatures + the call-shape classification that tells
//! the binder how a given callable's *syntax* works (ordinary call, statement
//! form, the quirky intrinsics, etc.).

// The static type lattice (`VarTypeRef`/`BuiltinType`) is defined once, in
// `oxvba-bundle` (the lowest crate the symbol model, the binder, and the typed IR
// all share), and re-exported here so the symbol/binder code keeps its established
// `signature::VarTypeRef` paths. See `oxvba_bundle::vartype`.
pub use oxvba_bundle::{BuiltinType, FixedArrayBound, VarTypeRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassingMode {
    ByRef,
    ByVal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DefaultValue {
    I16(i16),
    I32(i32),
    I64(i64),
    /// IEEE-754 `f64` bit pattern.
    F64(u64),
    /// IEEE-754 `f32` bit pattern.
    F32(u32),
    Bool(bool),
    Str(String),
    /// Currency scaled ×10_000.
    CurrencyScaledI64(i64),
    /// `Date` serial as `f64` bit pattern.
    DateSerialF64(u64),
    /// `Optional … As <type>` with no explicit default → the type's zero value.
    DeclaredTypeDefault,
    /// `Optional` Variant with no default → the `Missing` marker.
    VariantMissing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: VarTypeRef,
    pub mode: PassingMode,
    pub optional: bool,
    pub param_array: bool,
    pub default: Option<DefaultValue>,
}

/// How a callable is written at the call site — drives the binder's desugaring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallShape {
    /// `f(a, b)` / `f a, b` — positional or named, normal.
    Ordinary,
    /// A `Sub` invoked statement-style with no parentheses.
    NoParens,
    /// `Mid(s, i, n) = rhs` — the call is the target of an assignment (`MidStmt`).
    AssignmentTarget,
    /// `Open`/`Close`/`Print #`/`Write #`/`Input #`/`Line Input #`/`Get`/`Put`/`Seek`/`Width`.
    FileStatement,
    /// `Debug.Print` / `Debug.Assert`.
    DebugMember,
    /// `Spc(n)` / `Tab(n)` — only legal inside a `Print` clause.
    PrintClause,
    /// `Array()` / `IIf` / `Choose` / `Switch` / `CallByName` — special eval rule.
    SpecialForm,
    /// `TypeOf x Is T` — the argument is a type, not a value.
    TypeArgument,
    /// `AddressOf proc` — a procedure reference, not a call.
    AddressOf,
    /// `VarPtr`/`StrPtr`/`ObjPtr` — receiver is a place, not a value.
    PtrIntrinsic,
    /// `Err.Raise` / `Err.Number` / `.Description` / `.Source`.
    ErrMember,
    /// `RaiseEvent E(args)`.
    EventRaise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignatureId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub params: Vec<Param>,
    pub return_type: Option<VarTypeRef>,
    pub call_shape: CallShape,
}

#[derive(Debug, Clone, Default)]
pub struct SignatureTable {
    sigs: Vec<Signature>,
}

impl SignatureTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&mut self, signature: Signature) -> SignatureId {
        let id = SignatureId(self.sigs.len() as u32);
        self.sigs.push(signature);
        id
    }

    pub fn get(&self, id: SignatureId) -> Option<&Signature> {
        self.sigs.get(id.0 as usize)
    }
}
