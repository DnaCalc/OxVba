//! Program structure: parameters, locals, globals, functions, classes, and the
//! whole compilation unit ([`OxProgram`]).
//!
//! Cross-bundle, event, declare, and COM-export metadata are reused verbatim from
//! `oxvba_bundle`/`oxvba_com` — these are stable contracts OxIR does not re-model. The
//! *typed COM interface table* ([`crate::com`]) is the one organizing structure OxIR
//! adds, carried here as [`OxProgram::com_interfaces`].

use serde::{Deserialize, Serialize};

use oxvba_bundle::{
    ArrayElementType, BundleExport, BundleImport, ComClassExport, EventRoute,
    ExternalCallDescriptor, ProcedureKind, ProjectMemberKind,
};

use oxvba_com::TypeLibMemberMetadata;

use crate::com::{ComInterface, ComMethodRef};
use crate::ids::{BlockId, FuncId, LocalId};
use crate::inst::{OxAsNew, OxBlock};
use crate::ty::{IfaceId, OxTy, RecordLayoutId};

/// Parameter-specific facts for a [`OxLocal`] that is a procedure parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxParamInfo {
    /// Source-level `Optional` marker. Omitted defaults are bound by the caller,
    /// but package/runtime descriptor consumers still need the signature fact.
    #[serde(default)]
    pub optional: bool,
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
    /// Element layout for declared array slots. This comes from the binder's
    /// target-aware metadata and preserves source-level array facts that are not
    /// otherwise represented by a scalar `OxTy`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub array_element: Option<ArrayElementType>,
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
    /// Element layout for declared array slots; see [`OxLocal::array_element`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub array_element: Option<ArrayElementType>,
}

/// A compiled procedure: its typed frame and its basic-block CFG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxFunc {
    pub name: String,
    pub kind: ProcedureKind,
    /// Frame locals (parameters first), indexed by [`LocalId`].
    pub locals: Vec<OxLocal>,
    /// Compiler temporaries, indexed by [`crate::ids::TempId`].
    #[serde(default)]
    pub temps: Vec<OxTy>,
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
    /// COM/VBA dispatch id assigned by the project export surface, when this
    /// class member is part of that surface.
    #[serde(default)]
    pub dispid: Option<i32>,
    /// Dual-interface vtable slot assigned by the project export surface, when
    /// this class member is part of that surface.
    #[serde(default)]
    pub vtable_slot: Option<u16>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxClassField {
    pub name: String,
    /// Stable per-class instance-field token used by field get/set instructions.
    pub token: i32,
    pub ty: OxTy,
    /// Element layout for declared array fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub array_element: Option<ArrayElementType>,
}

/// A project class: lifecycle hooks, late-bound member table, implemented interfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OxClass {
    pub name: String,
    /// True for a `VB_PredeclaredId = True` class whose name denotes a singleton
    /// default instance.
    #[serde(default)]
    pub predeclared: bool,
    pub initialize: Option<FuncId>,
    pub terminate: Option<FuncId>,
    #[serde(default)]
    pub fields: Vec<OxClassField>,
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
    /// UDT record layouts, indexed by [`RecordLayoutId`]. Each entry is the recursive
    /// field layout used by the runtime `VbaRecord` carrier.
    #[serde(default)]
    pub record_layouts: Vec<Vec<ArrayElementType>>,
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

    /// The recursive field layout for a UDT record type.
    pub fn record_layout(&self, id: RecordLayoutId) -> Option<&[ArrayElementType]> {
        self.record_layouts.get(id.0).map(Vec::as_slice)
    }
}
