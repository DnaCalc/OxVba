//! The Core IR — a desugared, **fully resolved** tree (the contract's §5 form).
//!
//! This is the structured, in-memory shape of a program; [`crate::linearize`]
//! flattens it to the [`crate::Bundle`] machine ([`crate::isa::Op`]). The two
//! share a vocabulary but differ in shape: this tree has nested values and
//! structured control flow; the bundle is a flat slot machine.
//!
//! Invariant: the Core IR carries **no symbol references**. Names are already
//! resolved — callees are a `NativeImplId` / a [`ProcId`] / a COM dispid / a
//! `Declare` descriptor id; coercion targets use the bundle's enums and
//! `oxvba_runtime::VarType`; variables are logical [`LocalId`]/[`GlobalId`]s
//! that `linearize` assigns to slots. That is what keeps `oxvba-bundle`
//! dependency-light (it never needs the symbol model or the front-end).

use oxvba_runtime::variant::VarType;

use crate::native::NativeImplId;
use crate::{
    ArrayElementType, AssignmentIntent, AssignmentTargetKind, ComClassExport, EventRoute,
    ExternalCallDescriptor, NumericCoerceTarget, NumericMode, ProcedureKind, ProjectMemberKind,
    StringCompareMode,
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
    /// `Some` for a module-level array (informational; element type for `ReDim`).
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
    pub body: Vec<CoreStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreParam {
    pub name: String,
    /// Callee-side declaration (diagnostics only; the caller decides aliasing).
    pub by_ref: bool,
    /// A trailing `ParamArray` parameter: the caller boxes all remaining
    /// positional arguments into a fresh 0-based array bound to this slot.
    pub variadic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLocal {
    pub name: String,
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
    Field { object: Box<CoreValue>, field: i32 },
    /// Array element (`array(i, j, …)`).
    Index { array: Box<CorePlace>, indices: Vec<CoreValue> },
    /// A UDT field at a fixed index into `base`'s record (`p.X`). A value aggregate
    /// (backed by a record at run time), so the field is a positional slot.
    RecordField { base: Box<CorePlace>, index: usize },
    /// A `WithEvents` sink field: read → `WithEventsGet`, assign → `WithEventsSet`.
    WithEvents { owner: Box<CoreValue>, binding: i32 },
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
    FixedString(usize),
    ImplicitVariant(VarType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtrKind {
    Str,
    Var,
    VarString,
    VarVariant,
    Obj,
}

/// The payload a `Declare` pointer-argument write-back reads back from the pinned
/// pointer after the native call: a string (BSTR/UTF-16 cell) or a byte buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtrWritebackKind {
    String,
    ByteArray,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrField {
    Number,
    Description,
    Source,
    /// `Err.LastDllError` — the Win32 last-error code captured after the most recent
    /// `Declare Lib` call (a `Long`).
    LastDllError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    // `num` is the arithmetic numeric regime (the operands' promoted fixed type or
    // `Widening`); it drives `Negate`/arithmetic ops and is `Widening` otherwise.
    Unary { op: CoreUnOp, expr: Box<CoreValue>, num: NumericMode },
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
    Call { callee: CoreCallee, args: Vec<CoreArg> },
    /// `New <Class>` — allocate a project instance and run `Class_Initialize`.
    New(ClassId),
    /// Allocate a default-initialized UDT record of `fields` fields (a value aggregate).
    NewRecord { fields: usize },
    Coerce { value: Box<CoreValue>, to: CoerceTarget },
    TypeOfIs { object: Box<CoreValue>, type_name: String },
    /// A pointer-helper (`VarPtr`/`StrPtr`/`ObjPtr`) over its operand **value**: the
    /// VM pins the value in the pointer registry and yields its address. The operand
    /// is a value (not a place) so r-values like `StrPtr("literal")` work; write-back
    /// into an l-value operand is recorded separately on the `Declare` call.
    Ptr { kind: PtrKind, value: Box<CoreValue> },
    ErrField(ErrField),
    ArrayLiteral(Vec<CoreValue>),
    Bound { which: BoundWhich, array: Box<CorePlace> },
    /// `AddressOf proc` — a reference to a project procedure. Materializes the
    /// procedure index; calling through it (in-VM or via a real OS callback) is the
    /// native-runtime epic's concern.
    AddressOf(ProcId),
    /// `New <referenced class>` — allocate an instance of a class in another bundle.
    /// `import` indexes [`CoreProgram::imports`] (a `Class` token); the instance
    /// carries the target bundle's id for cross-bundle method dispatch.
    NewExtern { import: usize },
    /// A `VB_PredeclaredId` class referenced by its name → its global singleton
    /// instance (created lazily on first access, then persisting for the run).
    /// Distinct from `New`, which always allocates a fresh instance. This form is a
    /// class of the **active** project (same bundle).
    Predeclared { class: ClassId },
    /// A `VB_PredeclaredId` class published by a *referenced project* → its singleton
    /// in that project's bundle. `import` indexes [`CoreProgram::imports`] (a `Class`
    /// token); the returned instance carries the owning bundle's id for cross-bundle
    /// member dispatch (exactly like [`CoreValue::NewExtern`], but a shared singleton).
    PredeclaredExtern { import: usize },
}

/// The resolved target of a [`CoreValue::Call`].
#[derive(Debug, Clone, PartialEq)]
pub enum CoreCallee {
    /// A compiled VBA procedure (active or referenced project).
    VbaProc { proc: ProcId },
    /// A base-library / `Declare` / host primitive native body.
    Native(NativeImplId),
    /// Early-bound COM dispatch (typed receiver). The receiver is `args[0]`.
    EarlyCom { dispid: i32, kind: Option<ProjectMemberKind> },
    /// Late-bound COM dispatch (`Object`/`Variant` receiver, by name).
    LateDispatch { name: String, kind: Option<ProjectMemberKind> },
    /// A `Declare Lib` external call (descriptor in `CoreProgram::external_calls`).
    /// `ptr_writebacks` carries the pointer-helper arguments whose pinned buffer is
    /// read back into a source l-value after the call (see [`PtrWriteback`]).
    Declare { descriptor_id: u32, ptr_writebacks: Vec<PtrWriteback> },
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
    Named { name: String, value: CoreValue },
}

// ── Statements ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct CoreBound {
    pub upper: CoreValue,
    pub lower: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CaseClause {
    Value(CoreValue),
    Range { lo: CoreValue, hi: CoreValue },
    /// `Case Is <op> value`; `op` is a comparison [`CoreBinOp`].
    Is { op: CoreBinOp, value: CoreValue },
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
    OnErrorGotoLabel(LabelId),
    ResumeNext,
    Resume,
    ResumeLabel(LabelId),
    ClearErr,
    Raise { code: i32 },
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
    If { arms: Vec<CoreIfArm>, else_body: Vec<CoreStmt> },
    /// `Do While/Until … Loop` (pre-check) or `Do … Loop While/Until` (post-check).
    DoLoop { condition: CoreValue, until: bool, post_check: bool, body: Vec<CoreStmt> },
    /// `For var = start To end [Step step]`.
    ForRange {
        var: CorePlace,
        start: CoreValue,
        end: CoreValue,
        step: Option<CoreValue>,
        body: Vec<CoreStmt>,
    },
    /// `For Each item In source`.
    ForEach { item: CorePlace, source: CoreValue, body: Vec<CoreStmt> },
    Exit(ExitKind),
    Label(LabelId),
    Goto(LabelId),
    GoSub(LabelId),
    /// `Return` from the most recent `GoSub`.
    GoSubReturn,
    Error(ErrorOp),
    /// `ReDim`/`ReDim Preserve`.
    ReDim { array: CorePlace, bounds: Vec<CoreBound>, element_type: ArrayElementType, preserve: bool },
    Erase { array: CorePlace, element_type: ArrayElementType },
    RaiseEvent { source: CoreValue, event: i32, args: Vec<CoreArg> },
    Select { selector: CoreValue, cases: Vec<CoreCaseBlock>, case_else: Vec<CoreStmt> },
}
