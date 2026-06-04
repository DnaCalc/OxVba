//! `oxvba-bundle` — the clean, slim execution target.
//!
//! This crate defines the *ideal* shape of the executable semantic package per
//! `docs/spec/OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md`: a primitive instruction
//! set (`isa::Op`), one native-call form (`isa::CallNative` over
//! `native::NativeImplId`), and the metadata that makes a program runnable and
//! JIT-targetable. It is built on the `oxvba-runtime` value substrate and has no
//! dependency on the legacy compiler crate.
//!
//! Phase 1 runs this in-memory (front-end → legacy bundle → `oxvba-temp-b2b` →
//! this bundle → `oxvba-vm2`); serialization (`.oxb`) is a later concern.

pub mod isa;
pub mod native;

pub use isa::{CallArg, NativeCallee, Op, ProcArg};
pub use native::{LibraryModule, NativeImplId};

use oxvba_runtime::DynLinkSymbol;

// ── Shared scalar enums (the bundle's own clean copies) ──────────────────────

/// String comparison mode for `=`/`<>`/`InStr`/`Like` (`Option Compare`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StringCompareMode {
    Binary,
    Text,
}

/// Target width of a fixed-integer narrowing coercion (`CoerceNumeric`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericCoerceTarget {
    Byte,
    Integer,
    Long,
    LongLong,
}

/// Element type of a runtime array (for `ReDim`/typed element storage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrayElementType {
    Variant,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Byte,
    Single,
    Double,
    Currency,
    Date,
    String,
    Boolean,
}

/// Assignment intent on a `ValidateAssignment` (Let vs Set vs implicit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignmentIntent {
    Implicit,
    Let,
    Set,
}

/// The static kind of an assignment target (drives Let/Set legality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignmentTargetKind {
    Variant,
    Object,
    Scalar,
}

/// How a project/COM member call resolves its accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectMemberKind {
    Method,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

/// A resolved project-procedure member call attached to `CallProc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMemberCall {
    pub lowered_name: String,
    pub kind: ProjectMemberKind,
}

/// Selector for a COM/late-bound member dispatch (`CallNative`'s `ComDispatch`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComMemberSelector {
    DispatchId(i32),
    Name(String),
}

// ── Declare (`Declare Lib`) descriptors ──────────────────────────────────────

/// Parameter / return type of a `Declare Lib` external call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclareParamType {
    Long,
    Integer,
    String,
    Boolean,
    Double,
    Single,
    Currency,
    Date,
    Byte,
    LongLong,
    LongPtr,
    Variant,
    Any,
}

/// How a `Declare` `ByRef`/pointer argument is written back after the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalCallWritebackKind {
    ByRefValue,
    PointerByteArrayPayload,
    PointerStringPayload,
}

/// One `ByRef`/pointer writeback for a `Declare` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalCallWriteback {
    pub arg_index: usize,
    pub source_slot: usize,
    pub kind: ExternalCallWritebackKind,
}

/// A `Declare Lib` external-call descriptor, referenced by `NativeCallee::Declare`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCallDescriptor {
    pub descriptor_id: u32,
    pub declared_name: String,
    pub library: String,
    pub alias: String,
    pub ordinal_alias: bool,
    pub symbol: DynLinkSymbol,
    pub marshal_lane: String,
    pub calling_convention: String,
    pub selection_policy: String,
    pub param_count: usize,
    pub param_types: Vec<DeclareParamType>,
    pub param_by_ref: Vec<bool>,
    pub return_type: Option<DeclareParamType>,
    pub writebacks: Vec<ExternalCallWriteback>,
}

// ── Procedure / source / hosting metadata ────────────────────────────────────

/// The kind of a compiled procedure (a `CallProc` target).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcedureKind {
    Sub,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

/// A compiled procedure: its name, entry pc, kind, arity, and frame layout.
///
/// Each call allocates a fresh frame of `frame_slots` locals (recursion-safe);
/// parameters occupy the first `param_count` locals. Slot operands address
/// either a module global (`slot < Bundle::global_count`) or a current-frame
/// local (`slot - global_count`); see [`Bundle::global_count`]. `ByRef`
/// parameters alias the caller's place rather than copying (true aliasing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcedureDescriptor {
    pub name: String,
    pub entry_pc: usize,
    pub kind: ProcedureKind,
    pub param_count: usize,
    /// Number of local slots in this procedure's frame (params first, then
    /// locals/temporaries).
    pub frame_slots: usize,
    /// Frame-local slot holding the function's result (`None` for a `Sub`).
    pub return_slot: Option<usize>,
}

/// pc → source line, for diagnostics / error reporting / debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLineMapping {
    pub pc: usize,
    pub line: usize,
}

/// COM-server export descriptor (hosting metadata): a project class exposed as a
/// COM coclass. Carried for COM-server build targets; not used by VM execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComClassExport {
    pub class_name: String,
    pub prog_id: Option<String>,
    pub creatable: bool,
}

// ── The bundle ───────────────────────────────────────────────────────────────

/// A complete, runnable program: the instruction stream plus the metadata the VM
/// and (future) JIT need. This is the in-memory executable semantic package.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    /// The flat instruction stream; jumps/calls target indices into this vec.
    pub ops: Vec<Op>,
    /// Compiled procedures (call targets), in deterministic order.
    pub procedures: Vec<ProcedureDescriptor>,
    /// Entry point pc for top-level / `Sub Main` execution.
    pub entry_pc: usize,
    /// Module-level global slot count. A slot operand `s` addresses a global
    /// when `s < global_count`; otherwise it addresses the current frame's
    /// local at `s - global_count`. Globals persist across calls; locals are
    /// per-frame (recursion-safe).
    pub global_count: usize,
    /// Local-slot count of the top-level (entry) frame.
    pub entry_frame_slots: usize,
    /// Sorted pcs at which source statements begin, for statement-granular
    /// `Resume` / `Resume Next`. Empty ⇒ op-granularity (each op is a statement).
    pub statement_starts: Vec<usize>,
    /// `Declare Lib` external-call descriptors, indexed by `descriptor_id`.
    pub external_calls: Vec<ExternalCallDescriptor>,
    /// pc → source-line map.
    pub source_map: Vec<SourceLineMapping>,
    /// COM-server export descriptors (hosting; empty for ordinary programs).
    pub com_class_exports: Vec<ComClassExport>,
}

impl Bundle {
    /// An empty bundle (no program). Useful as a builder seed.
    pub fn empty() -> Self {
        Self {
            ops: Vec::new(),
            procedures: Vec::new(),
            entry_pc: 0,
            global_count: 0,
            entry_frame_slots: 0,
            statement_starts: Vec::new(),
            external_calls: Vec::new(),
            source_map: Vec::new(),
            com_class_exports: Vec::new(),
        }
    }

    /// Look up a `Declare` descriptor by id.
    pub fn external_call(&self, descriptor_id: u32) -> Option<&ExternalCallDescriptor> {
        self.external_calls
            .iter()
            .find(|d| d.descriptor_id == descriptor_id)
    }
}
