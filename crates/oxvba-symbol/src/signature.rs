//! VBA procedure/member signatures + the call-shape classification that tells
//! the binder how a given callable's *syntax* works (ordinary call, statement
//! form, the quirky intrinsics, etc.).

/// A VBA scalar/declared builtin type. Self-contained (not the runtime's
/// `VarType`, which carries COM/array encodings irrelevant to resolution).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// A resolved type reference. `Object(name)` is the static receiver-type
/// discriminator the binder uses to pick early vs late COM dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarTypeRef {
    Builtin(BuiltinType),
    /// A typed object/class/coclass receiver, by (folded) type name.
    Object(String),
    /// A user-defined `Type` (UDT) value, by (folded) type name. A value aggregate
    /// (copied by value, fields accessed by index) — distinct from `Object` (a
    /// reference). Produced by the binder when an `Object`-named type resolves to a
    /// declared `Type`.
    Udt(String),
    /// Untyped `Variant`/`Object` — forces late binding for member access.
    Variant,
    Array(Box<VarTypeRef>),
    /// A fixed-length string `String * N` (length in characters). Behaves like
    /// `String` in the type lattice, but assignment pads/truncates to `N` and file
    /// records use exactly `N` ANSI bytes with no length prefix.
    FixedString(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassingMode {
    ByRef,
    ByVal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DefaultValue {
    I32(i32),
    I64(i64),
    /// IEEE-754 `f64` bit pattern.
    F64(u64),
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
