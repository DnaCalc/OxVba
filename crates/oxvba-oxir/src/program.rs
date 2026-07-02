//! Program structure: parameters, locals, globals, functions, classes, and the
//! whole compilation unit ([`OxProgram`]).
//!
//! Cross-bundle, event, declare, and COM-export metadata are reused verbatim from
//! `oxvba_bundle`/`oxvba_com` — these are stable contracts OxIR does not re-model. The
//! *typed COM interface table* ([`crate::com`]) is the one organizing structure OxIR
//! adds, carried here as [`OxProgram::com_interfaces`].

use serde::{Deserialize, Serialize};

use oxvba_bundle::{
    BundleExport, BundleImport, ComClassExport, EventRoute, ExternalCallDescriptor, ProcedureKind,
    ProjectMemberKind,
};

use oxvba_com::TypeLibMemberMetadata;

use crate::com::{ComInterface, ComMethodRef};
use crate::ids::{BlockId, FuncId, LocalId};
use crate::inst::{OxAsNew, OxBlock};
use crate::ty::{IfaceId, OxTy};

/// Parameter-specific facts for a [`OxLocal`] that is a procedure parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxParamInfo {
    /// Callee-side `ByRef` declaration (the *caller* decides actual aliasing).
    pub by_ref: bool,
    /// A trailing `ParamArray`.
    pub variadic: bool,
}

/// A frame-local typed variable (indexed by [`LocalId`]). Parameters occupy the
/// first `param_count` locals of a function, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxLocal {
    pub name: String,
    pub ty: OxTy,
    /// `Some` if this local is a parameter.
    pub param: Option<OxParamInfo>,
    /// Address-taken (`VarPtr`/`StrPtr`) or aliased ByRef into a project proc — must
    /// stay in an addressable memory cell (a backend must not SSA-promote it). Set
    /// by the elaboration pass.
    pub escaped: bool,
}

/// A module-level typed global (indexed by [`crate::ids::GlobalId`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxGlobal {
    pub name: String,
    pub ty: OxTy,
}

/// A compiled procedure: its typed frame and its basic-block CFG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxFunc {
    pub name: String,
    pub kind: ProcedureKind,
    /// Frame locals (parameters first), indexed by [`LocalId`].
    pub locals: Vec<OxLocal>,
    /// How many of `locals` are parameters.
    pub param_count: usize,
    /// The local holding the function/property-get result (`None` for a `Sub`).
    pub return_local: Option<LocalId>,
    /// Basic blocks, indexed by [`BlockId`].
    pub blocks: Vec<OxBlock>,
    /// The entry block.
    pub entry: BlockId,
}

/// A late-bound-callable member of a project class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxClassMethod {
    pub name: String,
    pub kind: ProjectMemberKind,
    pub proc: FuncId,
    pub is_default_member: bool,
    #[serde(default)]
    pub is_enumerator_member: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxClassAsNewField {
    /// Stable per-class instance-field token.
    pub field: i32,
    /// Activation target for this field's `As New` declaration.
    pub binding: OxAsNew,
}

/// A project class: lifecycle hooks, late-bound member table, implemented interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxClass {
    pub name: String,
    pub initialize: Option<FuncId>,
    pub terminate: Option<FuncId>,
    pub methods: Vec<OxClassMethod>,
    #[serde(default)]
    pub as_new_fields: Vec<OxClassAsNewField>,
    /// Display names of `Implements`ed interfaces (for `TypeOf`/`Set` and typed
    /// interface dispatch).
    pub implements: Vec<String>,
}

/// A complete compilation unit in OxIR — the typed, CFG-structured program shape.
/// vm3 interprets it; the Cranelift backend lowers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct OxProgram {
    /// Procedures, indexed by [`FuncId`].
    pub funcs: Vec<OxFunc>,
    /// Module globals, indexed by [`crate::ids::GlobalId`].
    pub globals: Vec<OxGlobal>,
    /// Project classes, indexed by [`crate::ty::ClassId`].
    pub classes: Vec<OxClass>,
    /// Entry procedure (`None` ⇒ `Main` or the first proc).
    pub entry: Option<FuncId>,
    /// Hidden once-per-run global/static initializer.
    pub global_initializer: Option<FuncId>,
    /// This unit's name (its project name) — the key cross-bundle imports use.
    pub unit_name: String,
    /// WithEvents event routes (reused verbatim from the bundle).
    pub event_routes: Vec<EventRoute>,
    /// `Declare Lib` external-call descriptors, keyed by `descriptor_id`.
    pub external_calls: Vec<ExternalCallDescriptor>,
    /// COM-server export descriptors (hosting metadata).
    pub com_class_exports: Vec<ComClassExport>,
    /// Typed COM interface table, indexed by [`crate::ty::IfaceId`]: one entry per
    /// referenced/served COM interface (the canonical `oxvba_com::TypeLibInterfaceMetadata`
    /// grouping, with its full typed member descriptors) and per project `Implements`
    /// interface a typed object value can name. Persisted in the `.oxi` image so vm3/JIT make
    /// typed COM calls with no typelib re-resolution. An early-bound call
    /// ([`crate::inst::OxInst::ComCallEarly`]) names a member here via
    /// [`crate::com::ComMethodRef`]; resolve it with [`OxProgram::com_method`].
    pub com_interfaces: Vec<ComInterface>,
    /// Public members exported for cross-bundle references.
    pub exports: Vec<BundleExport>,
    /// Cross-bundle references this unit makes, indexed by [`crate::ids::ImportId`].
    pub imports: Vec<BundleImport>,
}

impl OxProgram {
    /// An empty program.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The interface-table entry named by an [`IfaceId`], if in range.
    pub fn com_interface(&self, id: IfaceId) -> Option<&ComInterface> {
        self.com_interfaces.get(id.0)
    }

    /// Resolve a [`ComMethodRef`] to its typed member descriptor. Returns `None` if
    /// the interface index is out of range, the entry is a project (non-COM)
    /// interface, or the member index is out of range.
    pub fn com_method(&self, method: ComMethodRef) -> Option<&TypeLibMemberMetadata> {
        self.com_interface(method.iface)?
            .com_members()?
            .get(method.member)
    }
}
