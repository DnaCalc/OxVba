//! Program structure: parameters, locals, globals, functions, classes, and the
//! whole compilation unit ([`OxProgram`]).
//!
//! Cross-bundle, event, declare, and COM-export metadata are reused verbatim from
//! `oxvba_bundle` — these are stable, name-keyed contracts that OxIR does not need
//! to re-model. (The *typed COM interface + method-descriptor tables* are the one
//! piece OxIR adds; they land with the COM instructions in the next sub-section.)

use serde::{Deserialize, Serialize};

use oxvba_bundle::{
    BundleExport, BundleImport, ComClassExport, EventRoute, ExternalCallDescriptor, ProcedureKind,
    ProjectMemberKind,
};

use crate::ids::{BlockId, FuncId, LocalId};
use crate::inst::OxBlock;
use crate::ty::OxTy;

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
}

/// A project class: lifecycle hooks, late-bound member table, implemented interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxClass {
    pub name: String,
    pub initialize: Option<FuncId>,
    pub terminate: Option<FuncId>,
    pub methods: Vec<OxClassMethod>,
    /// Display names of `Implements`ed interfaces (for `TypeOf`/`Set` and typed
    /// interface dispatch).
    pub implements: Vec<String>,
}

/// A complete compilation unit in OxIR — the typed, CFG-structured analogue of
/// `oxvba_bundle::Bundle`. vm3 interprets it; the Cranelift backend lowers it.
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
}
