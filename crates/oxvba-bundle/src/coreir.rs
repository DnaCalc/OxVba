//! The Core IR — a desugared, **fully resolved** tree (the contract's §5 form).
//!
//! This is the structured, in-memory shape of a program: the front-end (`oxvba-bind`) builds it,
//! and the vm3 path (`oxvba-oxir::elaborate`) lowers it to typed OxIR. It has nested values and
//! structured control flow.
//!
//! Invariant: the Core IR carries **no symbol references**. Names are already
//! resolved — callees are a `NativeImplId` / a [`ProcId`] / a typed COM member
//! descriptor / a `Declare` descriptor id; coercion targets use the bundle's enums
//! and `oxvba_runtime::VarType`; variables are logical [`LocalId`]/[`GlobalId`]s
//! that the consumer assigns to slots. `oxvba-bundle` therefore never needs the
//! symbol model or the front-end. It does depend on `oxvba-com` for the **canonical
//! typed COM member descriptor** ([`oxvba_com::TypeLibMemberMetadata`]) that
//! [`CoreCallee::EarlyCom`] carries: that descriptor is the one source of truth for a
//! COM call's typed signature, reused verbatim (never mirrored) so the OxIR
//! elaboration projects a fully typed early-bound call with no typelib re-resolution.

use oxvba_com::TypeLibMemberMetadata;
use oxvba_runtime::variant::VarType;

use crate::native::NativeImplId;
use crate::{
    ArrayElementType, AssignmentIntent, AssignmentTargetKind, ComClassExport, EventRoute,
    ExternalCallDescriptor, NumericCoerceTarget, NumericMode, ProcedureKind, ProjectMemberKind,
    StringCompareMode, VarTypeRef,
};

// ── Resolved logical ids ──────────────────────────────────────────────────────

/// Index into [`CoreProgram::globals`] — a module-level variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalId(pub usize);
/// Index into a procedure's locals (params first, then locals, then the
/// synthetic function-return local).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub usize);
/// Index into [`CoreProgram::procs`] — a resolved call target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcId(pub usize);
/// Index into [`CoreProgram::classes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassId(pub usize);
/// A procedure-local label (for `GoTo`/`GoSub`/`On Error GoTo`/`Resume`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelId(pub usize);

// ── Program / procedure / class ───────────────────────────────────────────────

/// A complete compilation unit (one project = one ".NET assembly"), ready to
/// linearize into its own [`crate::Bundle`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CoreProgram {
    /// Module-level variables; index == [`GlobalId`].
    pub globals: Vec<CoreGlobal>,
    /// Procedures; index == [`ProcId`].
    pub procs: Vec<CoreProc>,
    /// Project classes; index == [`ClassId`].
    pub classes: Vec<CoreClass>,
    /// WithEvents event routes (already resolved; copied verbatim to the bundle).
    pub event_routes: Vec<EventRoute>,
    /// `Declare Lib` external-call descriptors (copied verbatim).
    pub external_calls: Vec<ExternalCallDescriptor>,
    /// COM-server export descriptors (copied verbatim).
    pub com_class_exports: Vec<ComClassExport>,
    /// Entry procedure; `None` ⇒ `Main` (case-insensitive) or the first proc.
    pub entry: Option<ProcId>,
    /// Hidden once-per-run initializer for module fixed arrays and Static storage.
    pub global_initializer: Option<ProcId>,
    /// This unit's name (its project name) — the key cross-bundle imports use.
    pub unit_name: String,
    /// Public members exported for cross-bundle references (the bundle manifest).
    pub exports: Vec<crate::BundleExport>,
    /// Cross-bundle references this unit makes (resolved at link time).
    pub imports: Vec<crate::BundleImport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreGlobal {
    pub name: String,
    /// The declared static type (the binder's resolved `VarTypeRef`, recovered by the
    /// OxIR elaboration pass). `Variant` when undeclared. Object/UDT cases carry the
    /// folded type *name*; elaboration resolves the name to a typed identity.
    pub ty: VarTypeRef,
    /// `Some` for a module-level array (informational; element type for `ReDim`).
    /// Overlaps with [`Self::ty`] for arrays — it is the legacy `linearize`/`ReDim`
    /// element contract and will be unified into `ty` when `linearize` is retired.
    pub array_element: Option<ArrayElementType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreProc {
    pub name: String,
    pub kind: ProcedureKind,
    /// Parameters; occupy the first frame slots in order.
    pub params: Vec<CoreParam>,
    /// Locals (block scoping flattened) + the synthetic return local if any.
    pub locals: Vec<CoreLocal>,
    /// The local holding the function/property-get result (`None` for a `Sub`).
    pub return_local: Option<LocalId>,
    /// Procedure-local label metadata. `label_lines[id.0]` is `Some(n)` for a numeric
    /// line label (`10`, `20:`, ...), and `None` for named labels. The statement list
    /// still carries `CoreStmt::Label(id)` as the branch target; this side table lets
    /// the runtime update Erl's current line without changing label identity.
    pub label_lines: Vec<Option<i32>>,
    pub body: Vec<CoreStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreParam {
    pub name: String,
    /// The declared static type of the parameter (see [`CoreGlobal::ty`]).
    pub ty: VarTypeRef,
    /// Callee-side declaration (diagnostics only; the caller decides aliasing).
    pub by_ref: bool,
    /// A trailing `ParamArray` parameter: the caller boxes all remaining
    /// positional arguments into a fresh 0-based array bound to this slot.
    pub variadic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLocal {
    pub name: String,
    /// The declared static type of the local (see [`CoreGlobal::ty`]). For a
    /// function/property-get return local, this is the procedure's return type.
    pub ty: VarTypeRef,
    pub array_element: Option<ArrayElementType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreClass {
    pub name: String,
    pub initialize: Option<ProcId>,
    pub terminate: Option<ProcId>,
    pub methods: Vec<CoreClassMethod>,
    /// Display names of the interfaces this class `Implements` (for `TypeOf` and
    /// `Set` type checking). Members dispatch through the mangled `Interface_Member`
    /// names already present in `methods`.
    pub implements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreClassMethod {
    pub name: String,
    pub kind: ProjectMemberKind,
    pub proc: ProcId,
    pub is_default_member: bool,
    pub is_enumerator_member: bool,
}

// ── Places (l-values) ─────────────────────────────────────────────────────────

/// An assignable / addressable location. `ByRef`-ness of a parameter is NOT a
/// place kind — it is a [`CoreArg`] property, because in the frame model a
/// `ByRef` parameter is just a local slot the caller aliased.
#[derive(Debug, Clone, PartialEq)]
pub enum CorePlace {
    Local(LocalId),
    Global(GlobalId),
    /// Instance field access (`obj.field`); `field` is the resolved field token.
    Field {
        object: Box<CoreValue>,
        field: i32,
    },
    /// Array element (`array(i, j, …)`).
    Index {
        array: Box<CorePlace>,
        indices: Vec<CoreValue>,
    },
    /// A UDT field at a fixed index into `base`'s record (`p.X`). A value aggregate
    /// (backed by a record at run time), so the field is a positional slot.
    RecordField {
        base: Box<CorePlace>,
        index: usize,
    },
    /// A `WithEvents` sink field: read → `WithEventsGet`, assign → `WithEventsSet`.
    WithEvents {
        owner: Box<CoreValue>,
        binding: i32,
    },
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// A typed literal. Floats/date/currency are stored as bit patterns to mirror
/// the corresponding `Op::Load*` and keep the IR `PartialEq`.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreConst {
    Empty,
    Null,
    Nothing,
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    /// IEEE-754 `f64` bit pattern.
    F64(u64),
    /// IEEE-754 `f32` bit pattern.
    F32(u32),
    /// Currency scaled by 10_000.
    Currency(i64),
    /// `Date` serial as `f64` bit pattern.
    Date(u64),
    Str(String),
}

impl CoreConst {
    /// Parse a VBA decimal integer-literal token into its VBA-visible runtime
    /// carrier. A trailing `%`/`&`/`^` fixes Integer/Long/LongLong. Without a
    /// suffix, Excel/VBA uses Integer for the signed 16-bit range, Long for the
    /// signed 32-bit range, and Double beyond Long.
    pub fn from_int_literal(text: &str) -> Option<CoreConst> {
        let trimmed = text.trim();
        let suffix = match trimmed.as_bytes().last() {
            Some(&b @ (b'%' | b'&' | b'^')) => Some(b),
            _ => None,
        };
        let digits = trimmed.trim_end_matches(['%', '&', '^']);
        let n: i64 = digits.parse().ok()?;
        match suffix {
            Some(b'%') => i16::try_from(n).ok().map(CoreConst::I16),
            Some(b'&') => i32::try_from(n).ok().map(CoreConst::I32),
            Some(b'^') => Some(CoreConst::I64(n)),
            _ => {
                if let Ok(v) = i16::try_from(n) {
                    Some(CoreConst::I16(v))
                } else if let Ok(v) = i32::try_from(n) {
                    Some(CoreConst::I32(v))
                } else {
                    let value: f64 = digits.parse().ok()?;
                    value.is_finite().then_some(CoreConst::F64(value.to_bits()))
                }
            }
        }
    }

    /// Parse a VBA hex (`&H…`) or octal (`&O…`) integer-literal token into a
    /// typed constant, applying the width-based two's-complement sign rule
    /// (MS-VBAL §3.3.2; see [`oxvba_runtime::parse_vba_radix_with_width`]).
    /// `radix` is 16 or 8. The carrier follows the literal width:
    /// Integer/Long/LongLong. Excel/VBA rejects unsuffixed radix literals beyond
    /// Long width; LongLong radix requires an explicit `^` suffix. Returns `None`
    /// on malformed digits, syntax rejection, or a type-character width overflow.
    pub fn from_vba_radix(text: &str, radix: u32) -> Option<CoreConst> {
        let (value, width, suffix) = oxvba_runtime::parse_vba_radix_with_width(text, radix)?;
        use oxvba_runtime::VbaRadixWidth;
        match width {
            VbaRadixWidth::Integer => Some(CoreConst::I16(value as i16)),
            VbaRadixWidth::Long => Some(CoreConst::I32(value as i32)),
            VbaRadixWidth::LongLong if suffix == Some(b'^') => Some(CoreConst::I64(value)),
            VbaRadixWidth::LongLong => None,
        }
    }

    /// Parse a VBA floating-point literal token into its typed constant by the
    /// trailing type-declaration character: `@` → `Currency`, `!` → `Single`,
    /// `#` (or none) → `Double`. Without a `@`/`!` suffix this is a plain `Double`,
    /// so `1.5` and `1.5#` fold identically. The numeric body is parsed as `f64`;
    /// the Currency carrier is `value * 10_000` rounded to the nearest scaled unit
    /// (the same f64-based scaling the rest of the Currency system uses, e.g.
    /// `CCur`/`Const … As Currency`). Returns `None` on malformed digits or when a
    /// `Single`/`Currency` magnitude overflows its type. Used by both the binder
    /// and `Const` folding so the two agree.
    pub fn from_float_literal(text: &str) -> Option<CoreConst> {
        let value: f64 = text.trim_end_matches(['!', '#', '@']).parse().ok()?;
        match text.as_bytes().last() {
            Some(b'@') => {
                let scaled = (value * 10_000.0).round_ties_even();
                (scaled.is_finite() && scaled >= i64::MIN as f64 && scaled <= i64::MAX as f64)
                    .then_some(CoreConst::Currency(scaled as i64))
            }
            Some(b'!') => (value.is_finite() && value.abs() <= f64::from(f32::MAX))
                .then_some(CoreConst::F32((value as f32).to_bits())),
            _ => value.is_finite().then_some(CoreConst::F64(value.to_bits())),
        }
    }
}

#[cfg(test)]
mod core_const_tests {
    use super::CoreConst;

    #[test]
    fn decimal_integer_literal_keeps_vba_visible_carrier() {
        assert_eq!(CoreConst::from_int_literal("7"), Some(CoreConst::I16(7)));
        assert_eq!(
            CoreConst::from_int_literal("32767"),
            Some(CoreConst::I16(32767))
        );
        assert_eq!(CoreConst::from_int_literal("7%"), Some(CoreConst::I16(7)));
        assert_eq!(
            CoreConst::from_int_literal("32768"),
            Some(CoreConst::I32(32768))
        );
        assert_eq!(CoreConst::from_int_literal("7&"), Some(CoreConst::I32(7)));
        assert_eq!(CoreConst::from_int_literal("7^"), Some(CoreConst::I64(7)));
        assert!(matches!(
            CoreConst::from_int_literal("2147483648"),
            Some(CoreConst::F64(_))
        ));
    }

    #[test]
    fn radix_integer_literal_keeps_vba_visible_carrier() {
        assert_eq!(
            CoreConst::from_vba_radix("&HFFFF", 16),
            Some(CoreConst::I16(-1))
        );
        assert_eq!(
            CoreConst::from_vba_radix("&O177777", 8),
            Some(CoreConst::I16(-1))
        );
        assert_eq!(
            CoreConst::from_vba_radix("&HFFFF&", 16),
            Some(CoreConst::I32(65535))
        );
        assert_eq!(
            CoreConst::from_vba_radix("&HFFFFFFFFFFFFFFFF^", 16),
            Some(CoreConst::I64(-1))
        );
        assert_eq!(CoreConst::from_vba_radix("&H100000000", 16), None);
        assert_eq!(CoreConst::from_vba_radix("&O40000000000", 8), None);
    }
}

// ── Operators ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreUnOp {
    Negate,
    Not,
}

/// Binary operators. `Is` lowers to object identity; `Like` lowers to the
/// library; the rest lower to primitive ops (the §6.2 pragmatic exception).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreBinOp {
    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,
    Pow,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Xor,
    Eqv,
    Imp,
    Is,
    Like,
}

/// Where an explicit `Coerce` lands. `ImplicitVariant` is a runtime-implicit
/// widening that needs no instruction (the VM promotes Variants internally).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoerceTarget {
    Numeric(NumericCoerceTarget),
    String,
    FixedString(usize),
    ImplicitVariant(VarType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PtrKind {
    Str,
    Var,
    VarString,
    VarVariant,
    Obj,
}

/// The payload a `Declare` pointer-argument write-back reads back from the pinned
/// pointer after the native call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PtrWritebackKind {
    String,
    ByteArray,
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
}

/// A `Declare` argument that is `StrPtr(x)` / `VarPtr(x)` over an l-value: after
/// the native call, the pinned buffer at the `arg_index`-th argument's pointer is
/// read back into `target` (the source variable). This is VBA's expression-shape
/// driven write-back — an r-value pointer operand records no write-back.
#[derive(Debug, Clone, PartialEq)]
pub struct PtrWriteback {
    pub arg_index: usize,
    pub target: CorePlace,
    pub kind: PtrWritebackKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ErrField {
    Number,
    Description,
    Source,
    HelpFile,
    HelpContext,
    /// `Err.LastDllError` — the Win32 last-error code captured after the most recent
    /// `Declare Lib` call (a `Long`).
    LastDllError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BoundWhich {
    Lower,
    Upper,
}

// ── Values ────────────────────────────────────────────────────────────────────

/// A value-producing expression. Member access, default-member application, and
/// property reads are already desugared into `Call`/`Load`.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreValue {
    Const(CoreConst),
    Load(CorePlace),
    /// Procedure-local scratch slot holding a `With` receiver. The binder assigns
    /// ids; the linearizer maps each id to the slot where the enclosing `With`
    /// receiver was evaluated once.
    WithTemp(usize),
    // `num` is the arithmetic numeric regime (the operands' promoted fixed type or
    // `Widening`); it drives `Negate`/arithmetic ops and is `Widening` otherwise.
    Unary {
        op: CoreUnOp,
        expr: Box<CoreValue>,
        num: NumericMode,
    },
    // `mode` is the string-comparison mode (comparison ops); `num` is the arithmetic
    // numeric regime (arithmetic ops). Each op uses one.
    Binary {
        op: CoreBinOp,
        lhs: Box<CoreValue>,
        rhs: Box<CoreValue>,
        mode: StringCompareMode,
        num: NumericMode,
    },
    /// The one call node — project proc, library, COM, or `Declare`.
    Call {
        callee: CoreCallee,
        args: Vec<CoreArg>,
    },
    /// `New <Class>` — allocate a project instance and run `Class_Initialize`.
    New(ClassId),
    /// Allocate a default-initialized UDT record using its recursive field layout.
    NewRecord {
        fields: Vec<ArrayElementType>,
    },
    Coerce {
        value: Box<CoreValue>,
        to: CoerceTarget,
    },
    TypeOfIs {
        object: Box<CoreValue>,
        type_name: String,
    },
    /// A pointer-helper (`VarPtr`/`StrPtr`/`ObjPtr`) over its operand **value**: the
    /// VM pins the value in the pointer registry and yields its address. The operand
    /// is a value (not a place) so r-values like `StrPtr("literal")` work; write-back
    /// into an l-value operand is recorded separately on the `Declare` call.
    Ptr {
        kind: PtrKind,
        value: Box<CoreValue>,
    },
    ErrField(ErrField),
    /// The standalone VBA `Erl` function: reads the line number recorded by the most
    /// recent trapped error in the current run (0 if none).
    Erl,
    /// A `Variant` array materialized from element values (VBA's `Array()` and
    /// `ParamArray` boxing). `lower_bound` is the array's first index: the
    /// module's `Option Base` for `Array()`, always 0 for a `ParamArray`.
    /// `aliases` is populated only for `ParamArray` elements that VBA treats as
    /// caller-backed slots; ordinary `Array()` literals leave it empty.
    ArrayLiteral {
        elems: Vec<CoreValue>,
        lower_bound: i32,
        aliases: Vec<Option<CorePlace>>,
    },
    Bound {
        which: BoundWhich,
        array: Box<CorePlace>,
        dimension: Option<Box<CoreValue>>,
    },
    /// `AddressOf proc` — a reference to a project procedure. Materializes the
    /// procedure index; calling through it (in-VM or via a real OS callback) is the
    /// native-runtime epic's concern.
    AddressOf(ProcId),
    /// `New <referenced class>` — allocate an instance of a class in another bundle.
    /// `import` indexes [`CoreProgram::imports`] (a `Class` token); the instance
    /// carries the target bundle's id for cross-bundle method dispatch.
    NewExtern {
        import: usize,
    },
    /// A `VB_PredeclaredId` class referenced by its name → its global singleton
    /// instance (created lazily on first access, then persisting for the run).
    /// Distinct from `New`, which always allocates a fresh instance. This form is a
    /// class of the **active** project (same bundle).
    Predeclared {
        class: ClassId,
    },
    /// A `VB_PredeclaredId` class published by a *referenced project* → its singleton
    /// in that project's bundle. `import` indexes [`CoreProgram::imports`] (a `Class`
    /// token); the returned instance carries the owning bundle's id for cross-bundle
    /// member dispatch (exactly like [`CoreValue::NewExtern`], but a shared singleton).
    PredeclaredExtern {
        import: usize,
    },
}

/// The resolved target of a [`CoreValue::Call`].
#[derive(Debug, Clone, PartialEq)]
pub enum CoreCallee {
    /// A compiled VBA procedure (active or referenced project).
    VbaProc { proc: ProcId },
    /// A base-library / `Declare` / host primitive native body.
    Native(NativeImplId),
    /// Early-bound COM dispatch (typed receiver). The receiver is `args[0]`.
    ///
    /// Carries the **full canonical typed member signature** ([`member`]) — the
    /// per-parameter semantic types + ABI wire shapes + IIDs, the `QueryInterface`
    /// target, the x64 vtable slot (+ bound), dual/dispinterface kind, return
    /// type/wire, and optional/`[lcid]` rules — so the OxIR elaboration projects a
    /// fully typed early-bound call (and persists it) with no typelib re-resolution.
    /// This is the de-erasure: the legacy pipeline kept only a `(dispid, name)`
    /// selector and re-resolved the live object at run time.
    ///
    /// [`name`] and [`kind`] are the **call-site** selector name and dispatch
    /// accessor, which are *not* always the member's own (a value read of a
    /// get/put-sharing property coerces `kind` to `PropertyGet`; a default-member
    /// access selects by the receiver label), so they are carried distinctly from
    /// `member`. `dispid` is **not** stored — it is `member.token`.
    ///
    /// [`member`]: CoreCallee::EarlyCom::member
    /// [`name`]: CoreCallee::EarlyCom::name
    /// [`kind`]: CoreCallee::EarlyCom::kind
    EarlyCom {
        /// Call-site selector name (the syntactic member, or a default-member
        /// receiver label) — the `linearize` selector name and the late-resolution key.
        name: String,
        /// Call-site dispatch accessor (read context coerces to `PropertyGet`, etc.).
        kind: Option<ProjectMemberKind>,
        /// The declared receiver COM type (e.g. `"Excel.Range"`) — the elaboration's
        /// typed-interface-table grouping key.
        interface_name: String,
        /// The full canonical typed member descriptor (boxed to keep `CoreCallee` small).
        member: Box<TypeLibMemberMetadata>,
    },
    /// Late-bound COM dispatch (`Object`/`Variant` receiver), either by member
    /// name or by the receiver's default member (`obj(...)` / dispid 0).
    LateDispatch {
        name: String,
        kind: Option<ProjectMemberKind>,
        default_member: bool,
    },
    /// A `Declare Lib` external call (descriptor in `CoreProgram::external_calls`).
    /// `ptr_writebacks` carries the pointer-helper arguments whose pinned buffer is
    /// read back into a source l-value after the call (see [`PtrWriteback`]).
    Declare {
        descriptor_id: u32,
        ptr_writebacks: Vec<PtrWriteback>,
    },
    /// `CallByName(obj, name, calltype, args…)` — dispatch by a *runtime* member
    /// name through the same machinery as `LateDispatch`. The operands are carried
    /// in `args`: `[0]` = object, `[1]` = name string, `[2]` = calltype
    /// (`vbMethod`/`vbGet`/`vbLet`/`vbSet`), `[3..]` = the forwarded call arguments.
    DynamicByName,
    /// A cross-bundle (".NET assembly") call to a referenced project's hidden-module
    /// / free function. `import` indexes [`CoreProgram::imports`]; the linker
    /// resolves it to a `(bundle, proc)` pair at load time.
    ExternProc { import: usize },
}

/// A call argument. `ByRef` passes a place (true alias for slot places;
/// copy-in/out for field/array-element places).
#[derive(Debug, Clone, PartialEq)]
pub enum CoreArg {
    ByVal(CoreValue),
    ByRef(CorePlace),
    Omitted,
    /// A `name := value` argument (late-bound / COM dispatch).
    Named {
        name: String,
        value: CoreValue,
    },
}

// ── Statements ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CoreBound {
    pub upper: CoreValue,
    pub lower: CoreValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaseClause {
    Value(CoreValue),
    Range {
        lo: CoreValue,
        hi: CoreValue,
    },
    /// `Case Is <op> value`; `op` is a comparison [`CoreBinOp`].
    Is {
        op: CoreBinOp,
        value: CoreValue,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    Do,
    For,
    Proc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorOp {
    OnErrorResumeNext,
    OnErrorGoto0,
    /// `On Error GoTo -1` — clears the active-error latch (so the current handler can
    /// re-catch) and resets `Err`, but KEEPS the current handler policy (unlike
    /// `GoTo 0`, which disables it).
    OnErrorGotoMinus1,
    OnErrorGotoLabel(LabelId),
    ResumeNext,
    Resume,
    ResumeLabel(LabelId),
    ClearErr,
    /// `Err.Raise Number[, Source][, Description][, HelpFile][, HelpContext]`
    /// and the legacy `Error Number`.
    /// `number` is folded to a `Const(I32)` when statically known, so vm2's linearizer
    /// keeps its immediate `RaiseError` path; otherwise it is a runtime operand.
    /// `source`/`description`/`help_file`/`help_context` are the optional explicit
    /// fields; a missing one falls back to the VBA default for the number at raise
    /// time, or inherits the current `Err` state when `inherit` applies.
    ///
    /// `inherit` selects the omitted-argument semantics. `true` for `Err.Raise`: an
    /// omitted field inherits the un-cleared `Err` field (MS-VBAL §9071).
    /// `false` for the legacy `Error <n>` statement: omitted fields ALWAYS take their
    /// defaults (project name / derived message) — `Error <n>` does NOT inherit, an
    /// oracle-confirmed divergence from §2841's "as if Err.Raise(number)".
    Raise {
        number: CoreValue,
        source: Option<Box<CoreValue>>,
        description: Option<Box<CoreValue>>,
        help_file: Option<Box<CoreValue>>,
        help_context: Option<Box<CoreValue>>,
        inherit: bool,
    },
    /// `Err.Number = ...`, `Err.Description = ...`, `Err.Source = ...`,
    /// `Err.HelpFile = ...`, and `Err.HelpContext = ...`.
    /// `Err.LastDllError` is intentionally read-only; the binder rejects it before
    /// this IR is produced.
    SetErrField {
        field: ErrField,
        value: CoreValue,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreIfArm {
    pub condition: CoreValue,
    pub body: Vec<CoreStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreCaseBlock {
    pub clauses: Vec<CaseClause>,
    pub body: Vec<CoreStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreStmt {
    /// `Let`/`Set`/implicit assignment. Carries the `ValidateAssignment` metadata.
    Assign {
        place: CorePlace,
        value: CoreValue,
        intent: AssignmentIntent,
        target_kind: AssignmentTargetKind,
        target_name: String,
        target_type_name: String,
    },
    /// A statement-form call (the value is evaluated, the result discarded). All
    /// file/console/`Debug` I/O statements desugar to `Eval(Call(Native(..)))`.
    Eval(CoreValue),
    /// `If` / `ElseIf` chain (`arms[0]` is the `If`); `else_body` may be empty.
    If {
        arms: Vec<CoreIfArm>,
        else_body: Vec<CoreStmt>,
    },
    /// `Do While/Until … Loop` (pre-check) or `Do … Loop While/Until` (post-check).
    DoLoop {
        condition: CoreValue,
        until: bool,
        post_check: bool,
        body: Vec<CoreStmt>,
    },
    /// `For var = start To end [Step step]`.
    ForRange {
        var: CorePlace,
        start: CoreValue,
        end: CoreValue,
        step: Option<CoreValue>,
        body: Vec<CoreStmt>,
    },
    /// `For Each item In source`.
    ForEach {
        item: CorePlace,
        source: CoreValue,
        body: Vec<CoreStmt>,
    },
    /// `With receiver ... End With`; the receiver is evaluated once.
    With {
        id: usize,
        receiver: CoreValue,
        body: Vec<CoreStmt>,
    },
    Exit(ExitKind),
    Label(LabelId),
    Goto(LabelId),
    GoSub(LabelId),
    /// `Return` from the most recent `GoSub`.
    GoSubReturn,
    /// Bare `End`: terminate the whole program immediately.
    End,
    /// `On <selector> GoTo L1, L2, …` / `On <selector> GoSub S1, S2, …` — the computed
    /// branch. `selector` is 1-based: `targets[selector - 1]` is taken; `0` or a value
    /// past the end falls through to the next statement. `is_gosub` distinguishes the
    /// `GoSub` form (each taken target returns to the statement after this one).
    ComputedGoto {
        selector: CoreValue,
        targets: Vec<LabelId>,
        is_gosub: bool,
    },
    Error(ErrorOp),
    /// `ReDim`/`ReDim Preserve`.
    ReDim {
        array: CorePlace,
        bounds: Vec<CoreBound>,
        element_type: ArrayElementType,
        preserve: bool,
        /// Whether the allocated array is fixed-size (`Dim a(1 To 3)` / a UDT
        /// fixed-array field) rather than a dynamic user `ReDim`. Carried onto
        /// the runtime SAFEARRAY's `FADF_FIXEDSIZE` bit so `Erase` can reset a
        /// fixed array (vs deallocate a dynamic one) by reading the value's own
        /// flag.
        fixed: bool,
    },
    Erase {
        array: CorePlace,
        element_type: ArrayElementType,
    },
    RaiseEvent {
        source: CoreValue,
        event: i32,
        args: Vec<CoreArg>,
    },
    Select {
        selector: CoreValue,
        cases: Vec<CoreCaseBlock>,
        case_else: Vec<CoreStmt>,
        /// The enclosing module's `Option Compare`, applied to string `Case`
        /// comparisons (so `Select Case "a" / Case "A"` matches under `Text`).
        compare_mode: StringCompareMode,
    },
}
