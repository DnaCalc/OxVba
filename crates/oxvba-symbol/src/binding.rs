//! The result of resolving a name/member: a `Binding` carrying a `DispatchRoute`
//! that holds exactly enough for the binder to emit a `coreir::CoreCallee`.
//! Folds the legacy `frontend_member_dispatch::MemberDispatchClass`.

use oxvba_bundle::coreir::CoreConst;
use oxvba_bundle::{NativeImplId, ProjectMemberKind};
use oxvba_com::{
    TypeLibEventDispatchPath, TypeLibMemberInvokeKind, TypeLibMemberMetadata, TypeLibParamType,
};

use crate::model::{PredeclaredObjectId, SymbolId};
use crate::structural::StructuralIntrinsic;

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// The declared symbol, when the route came from a stored symbol (source or
    /// eagerly-populated provider). `None` for lazily-resolved COM members.
    pub symbol: Option<SymbolId>,
    /// Set when this resolution is via a default member (`obj` used as a value).
    pub is_default: bool,
    pub route: DispatchRoute,
}

impl Binding {
    pub fn new(symbol: Option<SymbolId>, route: DispatchRoute) -> Self {
        Self {
            symbol,
            is_default: false,
            route,
        }
    }
}

/// Every variant maps onto a `coreir::CoreCallee` (or a value/structural form)
/// the binder will emit.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchRoute {
    /// A constant / variable / field read — not a call.
    Value,
    /// A folded constant literal carried in the route: a `Public Const`/`Enum`
    /// member published by a *referenced project's* export surface **and** the
    /// `vb*` base-library value constants (which resolve through this same shared
    /// route — see `providers::vba_library`). Surface-driven: no dependence on
    /// shared symbol identity across bundles.
    ConstValue(CoreConst),
    /// A reference to a predeclared object (`Debug`/`Err`/`Collection`); the
    /// binder uses it as the receiver type for member access.
    PredeclaredObject(PredeclaredObjectId),
    /// A project proc/property/event → `CoreCallee::VbaProc { member }`.
    ProjectMember { kind: ProjectMemberKind },
    /// A member published by a *referenced project's* export surface — bound
    /// through that project's contract (its bundle is a separate ".NET assembly").
    /// The binder lowers it to a cross-bundle extern by context:
    ///   * `has_receiver: false` (a hidden-module free function) → a `BundleImport`
    ///     keyed by `(unit, ExportToken::ModuleFunc{owner, member, kind})` +
    ///     `CoreCallee::ExternProc`;
    ///   * `has_receiver: true` (a coclass member) → `CoreCallee::LateDispatch` by
    ///     `member` name on the receiver (whose `bundle_id` selects the class table).
    ExternMember {
        /// The referenced unit (project) name — the bundle key for the import.
        unit: String,
        /// Owning type: a hidden module's name (free function) or the coclass name.
        owner: String,
        /// The member name (the import-token member / the receiver-dispatch name).
        member: String,
        kind: ProjectMemberKind,
        /// Per-parameter typelib types — the binder coerces each argument to these
        /// and reads `is_by_ref()` for ByRef write-back, exactly as for `ComMember`.
        param_types: Vec<TypeLibParamType>,
        /// Per-parameter names (declaration order). For a no-receiver `ExternProc`
        /// call the binder reorders named arguments into positional slots by name
        /// (the cross-bundle callee is positional); the coclass `LateDispatch` path
        /// keeps names for runtime name-dispatch.
        param_names: Vec<String>,
        /// Per-parameter optional flags. Missing required arguments remain omitted
        /// so the callee/runtime reports the normal arity error; only declared
        /// optional slots receive synthesized defaults.
        param_optional: Vec<bool>,
        /// Per-parameter explicit optional defaults. `None` means there is no
        /// folded/default literal in the surface; callers still synthesize VBA's
        /// declared-type default for omitted typed optional parameters.
        param_optional_defaults: Vec<Option<CoreConst>>,
        /// True when the exported member has a trailing `ParamArray` slot.
        variadic: bool,
        /// `true` for a coclass member (dispatched on a receiver); `false` for a
        /// global-namespace / hidden-module function (no receiver).
        has_receiver: bool,
    },
    /// A base-library intrinsic → `CoreCallee::Native(id)`.
    Native(NativeImplId),
    /// A `Declare Lib` external call → `CoreCallee::Declare { descriptor_id }`.
    Declare { descriptor_id: u32 },
    /// A COM member, fully typed. The binder picks early vs late by the receiver's
    /// static type: typed coclass → `EarlyCom`; `Object`/`Variant` →
    /// `LateDispatch { name }`. Carrying both makes one route serve both.
    ///
    /// The broken-out `dispid`/`member_name`/`member_kind`/`vtable_slot`/
    /// `invoke_kind`/`is_default_member`/`param_by_ref` are convenience projections
    /// of `member` that the binder's call-shaping logic reads heavily (e.g. coercing
    /// a value read to `PropertyGet`); they are derivable from `member` and exist for
    /// ergonomics on the legacy call-binding paths. `member` itself is the canonical
    /// typed descriptor the de-erased `CoreCallee::EarlyCom` carries into OxIR.
    ComMember {
        member_name: String,
        dispid: i32,
        vtable_slot: Option<u16>,
        invoke_kind: TypeLibMemberInvokeKind,
        member_kind: ProjectMemberKind,
        is_default_member: bool,
        /// Per-parameter by-ref (`[out]`/`[in,out]`) directions from the typelib;
        /// the binder emits `CoreArg::ByRef` for these so writes propagate back.
        param_by_ref: Vec<bool>,
        /// The declared receiver COM type (e.g. `"Excel.Range"`) — carried so the
        /// de-erased early-bound call records its typed-interface grouping key.
        interface_name: String,
        /// The full canonical typed member descriptor (boxed to keep the route small).
        member: Box<TypeLibMemberMetadata>,
    },
    /// A COM source-interface event (for `WithEvents` / `RaiseEvent` wiring).
    ComEvent {
        event_name: String,
        token: i32,
        dispatch_path: TypeLibEventDispatchPath,
        dispatch_member_id: Option<i32>,
        connection_point_iid: Option<String>,
        callback_arity: u8,
    },
    /// A root object supplied by the host object model (`Application`,
    /// `ThisWorkbook`, …). The current Core IR has no host-root value form, so
    /// the binder lowers this through the existing COM activation path; the route
    /// remains distinct so that later host-identity injection has one replacement
    /// point.
    ComObjectRoot {
        type_name: String,
        prog_id: Option<String>,
    },
    /// A compiler-internal structural intrinsic.
    Structural(StructuralIntrinsic),
    /// A VBA special form with a non-ordinary evaluation rule.
    SpecialForm(SpecialForm),
    /// A member of the predeclared `Err` object (no `NativeImplId`; the binder
    /// lowers these to `coreir` error-state ops / `ErrField` reads).
    ErrMember(ErrMember),
    /// The name did not resolve to anything callable/value.
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrMember {
    Raise,
    Number,
    Description,
    Source,
    Clear,
    HelpFile,
    HelpContext,
    LastDllError,
}

/// VBA forms whose evaluation is not an ordinary call and which are neither a
/// `NativeImplId` body nor a structural intrinsic: the binder lowers each with a
/// dedicated rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialForm {
    Array,
    IIf,
    Choose,
    Switch,
    CallByName,
    TypeOf,
    AddressOf,
    UBound,
    LBound,
    Erl,
}

/// Map a typelib invoke kind to the project-member kind the binder/coreir uses.
pub fn member_kind_from_invoke(invoke: TypeLibMemberInvokeKind) -> ProjectMemberKind {
    match invoke {
        TypeLibMemberInvokeKind::Method => ProjectMemberKind::Method,
        TypeLibMemberInvokeKind::PropertyGet => ProjectMemberKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyPut => ProjectMemberKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPutRef => ProjectMemberKind::PropertySet,
    }
}
