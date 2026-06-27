//! `oxvba-vm2` — the interpreter over the clean [`oxvba_bundle::Bundle`].
//!
//! This is the Phase-1 reference runtime: a slot-machine interpreter with an
//! exhaustive `match` over every [`Op`], so the instruction set is covered by
//! construction. Library built-ins dispatch into [`oxvba_lib::invoke`]; COM
//! member dispatch, `Declare Lib`, and host I/O go through the
//! [`oxvba_hal::HostServices`] facets.
//!
//! ## Execution model
//! - **Globals vs locals.** A slot operand `s` addresses a module global when
//!   `s < bundle.global_count`, otherwise the current frame's local at
//!   `s - global_count`. Globals persist for the whole run.
//! - **Frames.** Each [`Op::CallProc`] pushes a fresh [`Frame`] of `frame_slots`
//!   locals — recursion is automatic (separate storage per activation). The
//!   top-level runs in the entry frame, which is never popped.
//! - **True `ByRef` aliasing.** A `ByRef` argument binds the parameter to the
//!   caller's *place* (a global, or a local of an ancestor frame). Reads and
//!   writes through the parameter operate directly on the caller's storage —
//!   no copy-in/copy-out. Aliases are resolved to their ultimate backing place
//!   at call time, so they never chain or dangle.
//! - **Error state** is per-procedure (`On Error` is saved/restored across
//!   calls). `Resume`/`Resume Next` are statement-granular via
//!   `bundle.statement_starts`.
//!
//! Arithmetic/comparison/coercion are primitive (see [`oxvba_eval::arith`], the
//! shared value-core kernel); they stay opcodes rather than library calls because
//! `Variant` boxing dominates.

use oxvba_eval::arith;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use oxvba_bundle::{
    ArrayElementType, Bundle, CallArg, ClassDescriptor, ClassMethod, ComMemberSelector,
    DeclarePtrWriteback, ExportTarget, NativeBody, NativeCallee, NativeImplId, NativeMethodId,
    NumericMode, Op, ProcArg, ProjectMemberKind, PtrWritebackKind,
};
use oxvba_com::{
    ComMemberToken, ComSubscriptionToken, DynamicCallArg, DynamicCallKind, DynamicCallRequest,
    DynamicMemberSelector, DynamicValue,
};
use oxvba_diagnostics::{Diagnostic, DiagnosticPhase, extract_prefixed_code};
use oxvba_eval::collection::{CollectionData, CollectionError, Selector};
use oxvba_hal::HostServices;
use oxvba_hal::traits::DynLinkDescriptorView;
use oxvba_lib::{LibContext, LibError};
use oxvba_runtime::object_ref::{
    ObjectRef, RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, RuntimeClassDescriptor,
    RuntimeInterfaceDescriptor,
};
use oxvba_runtime::safe_array::{
    SafeArray, SafeArrayBound, VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE,
    VT_I2_VALUE, VT_I4_VALUE, VT_I8_VALUE, VT_R4_VALUE, VT_R8_VALUE, VT_RECORD_VALUE, VT_UI1_VALUE,
    VT_VARIANT_VALUE,
};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{
    Variant, VbaRecord, VbaRecordFieldKind, VbaRecordFieldSpec, VbaRecordLayout, pointer_helpers,
};

/// Project-instance ids start above any class route key (a class's `route_key`
/// is its index, so this keeps `compat_identity != route_key`, i.e. every
/// allocation reads as a true project instance, not a template/compat object).
const INSTANCE_ID_BASE: i32 = 0x1000_0000;

use oxvba_eval::arith::CmpOp;

/// VBA `Missing` (an omitted optional argument): vbError `&H80020004`.
const MISSING_ARG: i32 = 0x8002_0004u32 as i32;
const MAX_NATIVE_CALLBACKS: usize = 32;
static NEXT_NATIVE_CALLBACK_OWNER: AtomicUsize = AtomicUsize::new(1);

type ProjectEventSink<'h> = dyn FnMut(ObjectRef, i32, Vec<Variant>) -> Result<(), String> + 'h;

#[derive(Clone, Copy)]
struct NativeCallbackRegistration {
    owner: usize,
    vm: usize,
    bundle: usize,
    proc: usize,
}

thread_local! {
    static NATIVE_CALLBACKS: RefCell<[Option<NativeCallbackRegistration>; MAX_NATIVE_CALLBACKS]> =
        const { RefCell::new([None; MAX_NATIVE_CALLBACKS]) };
}

fn release_native_callbacks_for_owner(owner: usize) {
    NATIVE_CALLBACKS.with(|callbacks| {
        for slot in callbacks.borrow_mut().iter_mut() {
            if slot.is_some_and(|registration| registration.owner == owner) {
                *slot = None;
            }
        }
    });
}

/// A VBA run-time error surfaced to the embedder (uncaught by any handler).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmError {
    pub code: i32,
    pub message: String,
    pub diagnostic: Option<Box<Diagnostic>>,
}

impl VmError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        if let Some(diagnostic) = &self.diagnostic {
            let diagnostic = diagnostic.as_ref().clone();
            if diagnostic.vba_error_number.is_some() {
                return diagnostic;
            }
            return diagnostic.with_vba_error_number(self.code);
        }
        if let Some(code) = extract_prefixed_code(&self.message, "COM-E-") {
            Diagnostic::error(code, DiagnosticPhase::Com, self.message.clone())
                .with_vba_error_number(self.code)
        } else {
            Diagnostic::error(
                "RUN-E-VBA-ERROR",
                DiagnosticPhase::Runtime,
                self.message.clone(),
            )
            .with_vba_error_number(self.code)
        }
    }
}

/// An in-flight raised error (caught by `On Error` if a handler is active).
#[derive(Debug, Clone)]
struct Fault {
    code: i32,
    message: String,
    diagnostic: Option<Box<Diagnostic>>,
}

impl Fault {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            diagnostic: None,
        }
    }
    fn from_string(message: String) -> Self {
        Fault::new(13, message) // Type mismatch — a bare untyped message
    }
    /// An arithmetic/coercion error carries its VBA code structurally.
    fn from_arith(err: arith::ArithError) -> Self {
        Fault::new(err.code, err.message)
    }
    fn from_lib(err: LibError) -> Self {
        Fault {
            code: err.code,
            message: err.message,
            diagnostic: None,
        }
    }
    fn from_hal(err: oxvba_hal::HalError) -> Self {
        // A COM dispatch fault carries the real VBA Err.Number it recovered from
        // the underlying HRESULT/EXCEPINFO; absent one, fall back to VBA error 5
        // ("invalid procedure call or argument").
        let code = err.host_error_code.unwrap_or(5);
        let diagnostic = err.to_diagnostic().with_vba_error_number(code);
        Fault {
            code,
            message: err.message,
            diagnostic: Some(Box::new(diagnostic)),
        }
    }
}

fn vm_error_from_fault(fault: Fault) -> VmError {
    VmError {
        code: fault.code,
        message: fault.message,
        diagnostic: fault.diagnostic,
    }
}

/// `On Error` handler state for the current procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorMode {
    None,
    ResumeNext,
    Goto(usize),
}

/// The statement to re-enter on `Resume` (`start`) or continue past on
/// `Resume Next` (`next`).
#[derive(Debug, Clone, Copy, Default)]
struct ResumePoint {
    start: usize,
    next: usize,
}

/// The `Err` object (number/description/source).
#[derive(Debug, Clone, Default)]
struct ErrObject {
    number: i32,
    description: String,
    source: String,
}

/// An index into the VM's loaded-bundle table (a ".NET assembly"). Bundle `0` is
/// the entry bundle; single-project runs only ever use `0`.
type BundleId = usize;

/// A storage location: a module global of a specific bundle, or a local of a
/// specific frame. Globals are bundle-qualified so a `ByRef` global aliased across
/// a bundle boundary still resolves to the owning bundle's slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Place {
    Global(BundleId, usize),
    Local(usize, usize), // (frame index, relative local index)
}

/// One activation record: its locals plus the call linkage. `ByRef` parameters
/// are recorded in `aliases` (relative local index → backing place) and have no
/// storage of their own.
struct Frame {
    locals: Vec<Variant>,
    aliases: HashMap<usize, Place>,
    dst: Option<Place>,
    return_slot: Option<usize>,
    return_pc: usize,
    /// The bundle to restore as current (`Vm::cur`) when this frame returns — the
    /// caller's bundle. `Vm::cur` always tracks the top frame's bundle, so this is
    /// the only per-frame bundle linkage needed (set on push, restored on pop).
    return_bundle: BundleId,
    /// When set, this frame's `Return` value is captured into `Vm::captured_return`
    /// rather than written to a caller place (used by nested method invocations).
    capture: bool,
    saved_error_mode: ErrorMode,
    saved_resume: ResumePoint,
}

/// A live `WithEvents` subscription: the sink instance that owns the field
/// (`owner`, the `Me` for handler dispatch) and the bound source object
/// (`source`). Both are strong references, matching VBA's connection-point
/// keep-alive (and so event cycles leak, VBA-consistent).
struct EventBinding {
    owner: Variant,
    source: Variant,
}

/// A live host (COM) event subscription: which sink handler to invoke when the
/// host delivers a callback for this subscription.
struct ComEventSink {
    owner: Variant,
    handler: usize,
}

/// `For Each` enumerator state, keyed by the iterator slot.
struct ForEachState {
    elements: Vec<Variant>,
    position: usize,
}

/// A cross-bundle import resolved to its target at link time:
/// `bundle.imports[i]` resolves to `LoadedBundle::imports[i]`.
#[derive(Debug, Clone, Copy)]
struct ResolvedImport {
    bundle: BundleId,
    target: ExportTarget,
}

/// One loaded bundle (a ".NET assembly"): its code + metadata (`bundle`) plus the
/// per-bundle runtime state the VM owns. The VM holds a `Vec<LoadedBundle>` and a
/// `cur` cursor; the executing frame's `bundle` selects which one `pc` indexes.
struct LoadedBundle<'h> {
    bundle: &'h Bundle,
    /// Module-level globals for this bundle (its "static" storage).
    globals: Vec<Variant>,
    /// Cross-bundle imports resolved at link time (parallel to `bundle.imports`).
    imports: Vec<ResolvedImport>,
    /// `(binding token, event id)` → handler procedure, from this bundle's routes.
    event_routes: HashMap<(i32, i32), usize>,
    /// Leaked `&'static` class descriptors, one per `bundle.classes` entry, for
    /// `ObjectRef::from_project_instance`.
    class_descriptors: Vec<&'static RuntimeClassDescriptor>,
    /// O(1) statement-boundary membership test.
    statement_start_set: std::collections::HashSet<usize>,
    /// `VB_PredeclaredId` singletons of this bundle's classes, created lazily on
    /// first access and cached by class index (so a bare reference to a predeclared
    /// class name yields one persistent instance per run). `New` of the same class
    /// is independent and never consults this.
    predeclared_singletons: HashMap<usize, Variant>,
    /// Whether this bundle's hidden global/static initializer has run.
    global_initialized: bool,
}

impl<'h> LoadedBundle<'h> {
    fn load(bundle: &'h Bundle) -> Self {
        // INVARIANT: each (binding, event) maps to exactly ONE handler. This holds because
        // the binder gives every `WithEvents` field a *bundle-unique* binding token (a
        // bundle-global counter in `oxvba-bind` ids.rs), so two distinct sink fields never
        // share a binding — and VBA permits a single `<field>_<Event>` handler per
        // field+event. (`build_event_routes` additionally collapses exact-duplicate routes
        // from COM shared-dispid events; that dedup does NOT separate two different sink
        // fields — bundle-unique tokens do.) The subscribe path consults the full route Vec
        // (one binding has many events); this map is only the dispatch-time
        // (binding, event) -> handler lookup. If two DIFFERENT handlers ever land on one
        // key, suspect a binding-token allocation regression (e.g. a per-class counter
        // reintroduced), so flag it in debug builds rather than silently keeping the last
        // writer.
        let mut event_routes: HashMap<(i32, i32), usize> = HashMap::new();
        for route in &bundle.event_routes {
            let prev = event_routes.insert((route.binding, route.event), route.handler);
            debug_assert!(
                prev.is_none() || prev == Some(route.handler),
                "bundle event_routes: (binding={}, event={}) bound to two different handlers \
                 ({prev:?} then {}) — two sink fields share a binding token; binding-token \
                 allocation must keep them bundle-unique (see oxvba-bind ids.rs)",
                route.binding,
                route.event,
                route.handler
            );
        }
        // Leak one `&'static RuntimeClassDescriptor` per class (descriptors live
        // for the VM's lifetime; `from_project_instance` requires `'static`).
        let class_descriptors = bundle
            .classes
            .iter()
            .map(|class| {
                let name: &'static str = Box::leak(class.name.clone().into_boxed_str());
                let interfaces: &'static [RuntimeInterfaceDescriptor] =
                    Box::leak(Box::new([RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR]));
                &*Box::leak(Box::new(RuntimeClassDescriptor { name, interfaces }))
            })
            .collect();
        let statement_start_set = bundle.statement_starts.iter().copied().collect();
        Self {
            bundle,
            globals: vec![Variant::empty(); bundle.global_count],
            // Resolved by `Vm::link` once all bundles are loaded (empty for a
            // single bundle, which has no cross-bundle imports).
            imports: Vec::new(),
            event_routes,
            class_descriptors,
            statement_start_set,
            predeclared_singletons: HashMap::new(),
            global_initialized: false,
        }
    }
}

/// The interpreter.
pub struct Vm<'h> {
    native_callback_owner: usize,
    /// Loaded bundles (".NET assemblies"); bundle `0` is the entry bundle.
    bundles: Vec<LoadedBundle<'h>>,
    /// The bundle whose `ops` the `pc` currently indexes (the executing frame's).
    cur: BundleId,
    host: &'h dyn HostServices,
    frames: Vec<Frame>,
    pc: usize,
    next_pc: usize,
    halted: bool,
    error_mode: ErrorMode,
    resume: ResumePoint,
    err: ErrObject,
    /// The OS last-error captured after the most recent native `Declare` call, read
    /// by `Err.LastDllError`. Updated from the HAL after each `declare_call`.
    last_dll_error: i32,
    lib: LibContext,
    /// `For Each` enumerator state, keyed by the iterator's resolved storage
    /// `Place` (bundle- and frame-unique) so concurrent/reentrant/cross-bundle
    /// loops that reuse the same slot *number* never alias.
    for_each: HashMap<Place, ForEachState>,
    withevents: HashMap<i64, EventBinding>,
    withevents_iters: Vec<(Vec<ObjectRef>, usize)>,
    /// Live host event subscriptions: subscription-token raw → sink handler.
    com_subscriptions: HashMap<i32, ComEventSink>,
    /// WithEvents key → host subscription tokens (for teardown on clear/terminate).
    com_subscriptions_by_key: HashMap<i64, Vec<i32>>,
    /// Re-entrancy guard for the inbound COM-event pump.
    pumping: bool,
    /// The most recent captured return value (see [`Frame::capture`]).
    captured_return: Option<Variant>,
    /// Monotonic per-instance identity counter, VM-global across all bundles (the
    /// IUnknown is the true identity; this is the lookup key).
    next_instance_id: i32,
    /// Re-entrancy guard so a `Class_Terminate` drain doesn't recursively drain;
    /// cascades are handled by the drain loop instead.
    draining: bool,
    /// Built-in `VBA.Collection` instance contents, keyed by the instance's
    /// `compat_identity`. Created lazily on first `Add`; never pruned (an entry
    /// outlives its object until VM drop — bounded by `New Collection` count, and
    /// consistent with the object-cycle-leak stance). See `collection.rs`.
    collections: HashMap<i32, CollectionData>,
    /// Optional host/wrapper event publication hook for project `RaiseEvent`.
    project_event_sink: Option<Box<ProjectEventSink<'h>>>,
}

impl Drop for Vm<'_> {
    fn drop(&mut self) {
        release_native_callbacks_for_owner(self.native_callback_owner);
    }
}

/// A cross-bundle link failure (an unresolved/mismatched import).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkError {
    pub message: String,
}

impl LinkError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::error("RUN-E-LINK", DiagnosticPhase::Runtime, self.message.clone())
    }
}

/// Run a single bundle from its entry point, returning the VM (for slot inspection).
pub fn run<'h>(bundle: &'h Bundle, host: &'h dyn HostServices) -> Result<Vm<'h>, VmError> {
    let mut vm = Vm::new(bundle, host);
    vm.run()?;
    Ok(vm)
}

impl<'h> Vm<'h> {
    /// Build a single-bundle VM (the entry bundle is bundle 0). For a cross-bundle
    /// image use [`Vm::link`].
    pub fn new(bundle: &'h Bundle, host: &'h dyn HostServices) -> Self {
        Self::link(&[bundle], host).expect("a single bundle always links")
    }

    /// Link N bundles (".NET assemblies") into one VM image. Imports resolve
    /// against any loaded unit by (case-insensitive) `unit_name`, so slice order
    /// only determines the **entry** unit: the **last** bundle is the entry (its
    /// `entry_pc` / `Sub Main` runs). An unknown unit, a missing export, or a
    /// duplicate `unit_name` is a [`LinkError`].
    pub fn link(bundles: &[&'h Bundle], host: &'h dyn HostServices) -> Result<Self, LinkError> {
        let entry = bundles.len().checked_sub(1).ok_or_else(|| LinkError {
            message: "no bundles to link".into(),
        })?;
        // Always link the built-in `VBA` library so `New Collection` (and, in later
        // phases, the rest of the built-in surface) resolves against a real unit.
        // Appended *after* the entry bundle: every caller-supplied bundle keeps its
        // index and the entry selection is unchanged, and imports resolve it by
        // unit name like any other unit. Skipped if the caller already supplied a
        // unit named "VBA" (avoids a duplicate-unit-name error).
        let mut all: Vec<&'h Bundle> = bundles.to_vec();
        if !bundles
            .iter()
            .any(|b| b.unit_name.eq_ignore_ascii_case("VBA"))
        {
            all.push(oxvba_bundle::vba_library_bundle());
        }
        let bundles: &[&'h Bundle] = &all;
        let mut loaded: Vec<LoadedBundle<'h>> =
            bundles.iter().map(|b| LoadedBundle::load(b)).collect();

        // Unit name (folded) → bundle id, for import resolution. A duplicate unit
        // name is ambiguous (which one does an import mean?) — reject it.
        let mut by_name: HashMap<String, BundleId> = HashMap::new();
        for (id, b) in bundles.iter().enumerate() {
            if !b.unit_name.is_empty()
                && by_name
                    .insert(b.unit_name.to_ascii_lowercase(), id)
                    .is_some()
            {
                return Err(LinkError {
                    message: format!("duplicate unit name `{}`", b.unit_name),
                });
            }
        }
        // Resolve each bundle's imports against the loaded bundles' exports.
        for (id, b) in bundles.iter().enumerate() {
            let mut resolved = Vec::with_capacity(b.imports.len());
            for imp in &b.imports {
                let target_bundle =
                    *by_name
                        .get(&imp.unit.to_ascii_lowercase())
                        .ok_or_else(|| LinkError {
                            message: format!(
                                "bundle `{}` references unknown unit `{}`",
                                b.unit_name, imp.unit
                            ),
                        })?;
                let export = bundles[target_bundle]
                    .exports
                    .iter()
                    .find(|e| e.token.matches(&imp.token))
                    .ok_or_else(|| LinkError {
                        message: format!(
                            "unit `{}` has no export matching a reference from `{}`",
                            imp.unit, b.unit_name
                        ),
                    })?;
                resolved.push(ResolvedImport {
                    bundle: target_bundle,
                    target: export.target,
                });
            }
            loaded[id].imports = resolved;
        }

        let entry_bundle = bundles[entry];
        let frame = Frame {
            locals: vec![Variant::empty(); entry_bundle.entry_frame_slots],
            aliases: HashMap::new(),
            dst: None,
            return_slot: None,
            return_pc: 0,
            return_bundle: entry,
            capture: false,
            saved_error_mode: ErrorMode::None,
            saved_resume: ResumePoint::default(),
        };
        Ok(Self {
            native_callback_owner: NEXT_NATIVE_CALLBACK_OWNER.fetch_add(1, Ordering::Relaxed),
            bundles: loaded,
            cur: entry,
            host,
            frames: vec![frame],
            pc: entry_bundle.entry_pc,
            next_pc: entry_bundle.entry_pc,
            halted: false,
            error_mode: ErrorMode::None,
            resume: ResumePoint::default(),
            err: ErrObject::default(),
            last_dll_error: 0,
            lib: LibContext::default(),
            for_each: HashMap::new(),
            withevents: HashMap::new(),
            withevents_iters: Vec::new(),
            com_subscriptions: HashMap::new(),
            com_subscriptions_by_key: HashMap::new(),
            pumping: false,
            captured_return: None,
            next_instance_id: INSTANCE_ID_BASE,
            draining: false,
            collections: HashMap::new(),
            project_event_sink: None,
        })
    }

    /// The bundle whose code the cursor currently executes. Returns the `&'h`
    /// reference itself, so callers can hold it without borrowing `self`.
    fn cur_bundle(&self) -> &'h Bundle {
        self.bundles[self.cur].bundle
    }

    /// Read a slot's value (resolved against the current top frame).
    pub fn slot(&self, slot: usize) -> Option<&Variant> {
        let place = self.target(slot).ok()?;
        self.read_place(place).ok()
    }

    /// The final `Err` object's number, and `LastDllError`, for the differential error
    /// axis. Valid after `run()` whether it completed or surfaced an uncaught error — the
    /// `Err` object retains the last raise (vm2 keeps the VM alive across `run`). `source`
    /// and `description` are exposed too, but the transitional vm2-vs-vm3 gate compares only
    /// the number / raised-state / `LastDllError` (vm3 deliberately carries richer
    /// Source/Description than vm2, which are validated against the live oracle instead).
    pub fn err_number(&self) -> i32 {
        self.err.number
    }
    pub fn err_source(&self) -> &str {
        &self.err.source
    }
    pub fn err_description(&self) -> &str {
        &self.err.description
    }
    pub fn last_dll_error(&self) -> i32 {
        self.last_dll_error
    }

    /// Allocate a project-class instance in a linked bundle without running the
    /// package entry point. This is the activation primitive used by wrapper
    /// hosts such as an in-process COM class factory.
    pub fn create_project_instance(
        &mut self,
        bundle: usize,
        class_name: &str,
    ) -> Result<ObjectRef, VmError> {
        self.ensure_bundle_initialized(bundle)?;
        let class_idx = self
            .bundles
            .get(bundle)
            .and_then(|loaded| {
                loaded
                    .bundle
                    .classes
                    .iter()
                    .position(|class| class.name.eq_ignore_ascii_case(class_name))
            })
            .ok_or_else(|| VmError {
                code: 5,
                message: format!("unknown project class `{class_name}` in bundle {bundle}"),
                diagnostic: None,
            })?;
        let descriptor = *self.bundles[bundle]
            .class_descriptors
            .get(class_idx)
            .ok_or_else(|| VmError {
                code: 5,
                message: format!("unknown project class `{class_name}` in bundle {bundle}"),
                diagnostic: None,
            })?;
        let meta = self.bundles[bundle]
            .bundle
            .classes
            .get(class_idx)
            .ok_or_else(|| VmError {
                code: 5,
                message: format!("unknown project class `{class_name}` in bundle {bundle}"),
                diagnostic: None,
            })?;
        let has_terminate = meta.terminate.is_some();
        let initialize = meta.initialize;
        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;
        let object = ObjectRef::from_project_instance(
            instance_id,
            class_idx as i32,
            bundle as i32,
            has_terminate,
            descriptor,
        );
        if let Some(init) = initialize {
            let saved_cur = self.cur;
            self.cur = bundle;
            let result = self.run_proc_with_values(
                init,
                Variant::from_object_ref(object.clone()),
                Vec::new(),
                false,
                false,
            );
            self.cur = saved_cur;
            result.map_err(vm_error_from_fault)?;
        }
        Ok(object)
    }

    /// Invoke a member on a project-class object using already-marshaled Variant
    /// values in declaration order. Named/default-argument policy is owned by the
    /// boundary caller; this method is the final VM execution hop.
    pub fn invoke_project_member_values(
        &mut self,
        object: ObjectRef,
        member_name: &str,
        kind_hint: Option<ProjectMemberKind>,
        args: Vec<Variant>,
    ) -> Result<Variant, VmError> {
        if !object.is_project_instance() {
            return Err(VmError {
                code: 438,
                message: "object is not an OxVBA project instance".to_string(),
                diagnostic: None,
            });
        }
        let class_idx = object.route_key() as usize;
        let obj_bundle = object.bundle_id() as usize;
        self.ensure_bundle_initialized(obj_bundle)?;
        let proc = self
            .bundles
            .get(obj_bundle)
            .and_then(|loaded| loaded.bundle.classes.get(class_idx))
            .and_then(|class| {
                let exact = class.methods.iter().find(|method| {
                    method.name.eq_ignore_ascii_case(member_name)
                        && kind_hint.is_none_or(|kind| kind == method.kind)
                });
                exact
                    .or_else(|| {
                        if kind_hint == Some(ProjectMemberKind::PropertyGet) && args.is_empty() {
                            class.methods.iter().find(|method| {
                                method.name.eq_ignore_ascii_case(member_name)
                                    && method.kind == ProjectMemberKind::Method
                            })
                        } else if kind_hint == Some(ProjectMemberKind::Method) {
                            class.methods.iter().find(|method| {
                                method.name.eq_ignore_ascii_case(member_name)
                                    && method.kind == ProjectMemberKind::PropertyGet
                            })
                        } else {
                            None
                        }
                    })
                    .map(|method| method.proc)
            })
            .ok_or_else(|| VmError {
                code: 438,
                message: format!("Object doesn't support `{member_name}`"),
                diagnostic: None,
            })?;
        let saved_cur = self.cur;
        self.cur = obj_bundle;
        let result =
            self.run_proc_with_values(proc, Variant::from_object_ref(object), args, false, true);
        self.cur = saved_cur;
        result.map_err(vm_error_from_fault)
    }

    pub fn set_project_event_sink<F>(&mut self, sink: F)
    where
        F: FnMut(ObjectRef, i32, Vec<Variant>) -> Result<(), String> + 'h,
    {
        self.project_event_sink = Some(Box::new(sink));
    }

    pub fn clear_project_event_sink(&mut self) {
        self.project_event_sink = None;
    }

    /// Drive the instruction stream until `Halt`, a `Return` from the entry
    /// frame, the end of the ops, or an uncaught error.
    pub fn run(&mut self) -> Result<(), VmError> {
        self.run_global_initializers()?;
        // Start from a clean termination queue (the queue is thread-local; the VM
        // is single-threaded, so this isolates one run from the next).
        oxvba_runtime::reset_pending_terminations();
        while !self.halted && self.pc < self.cur_bundle().ops.len() {
            // At a statement boundary, run any `Class_Terminate`s parked by the
            // statement that just completed (VBA statement-granular timing) and
            // dispatch any inbound host (COM) events.
            if self.bundles[self.cur]
                .statement_start_set
                .contains(&self.pc)
            {
                self.maybe_drain();
                self.pump_com_events();
            }
            let op = self.cur_bundle().ops[self.pc].clone();
            self.next_pc = self.pc + 1;
            match self.exec(&op) {
                Ok(()) => self.pc = self.next_pc,
                Err(fault) => self.dispatch_fault(fault)?,
            }
        }
        Ok(())
    }

    pub fn run_global_initializers(&mut self) -> Result<(), VmError> {
        for bundle in 0..self.bundles.len() {
            self.ensure_bundle_initialized(bundle)?;
        }
        Ok(())
    }

    fn ensure_bundle_initialized(&mut self, bundle: usize) -> Result<(), VmError> {
        let Some(initializer) = self
            .bundles
            .get(bundle)
            .and_then(|loaded| loaded.bundle.global_initializer)
        else {
            return Ok(());
        };
        if self
            .bundles
            .get(bundle)
            .map(|loaded| loaded.global_initialized)
            .unwrap_or(true)
        {
            return Ok(());
        }
        if let Some(loaded) = self.bundles.get_mut(bundle) {
            loaded.global_initialized = true;
        }
        let saved_cur = self.cur;
        self.cur = bundle;
        let result =
            self.run_proc_with_values(initializer, Variant::empty(), Vec::new(), false, false);
        self.cur = saved_cur;
        result.map(|_| ()).map_err(vm_error_from_fault)
    }

    // ── Slot / place resolution ────────────────────────────────────────────────
    fn target(&self, slot: usize) -> Result<Place, Fault> {
        let global_count = self.cur_bundle().global_count;
        if slot < global_count {
            return Ok(Place::Global(self.cur, slot));
        }
        let rel = slot - global_count;
        let frame_index = self
            .frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| Fault::new(5, "no active frame"))?;
        match self.frames[frame_index].aliases.get(&rel) {
            Some(place) => Ok(*place),
            None => Ok(Place::Local(frame_index, rel)),
        }
    }

    fn read_place(&self, place: Place) -> Result<&Variant, Fault> {
        match place {
            Place::Global(b, i) => self
                .bundles
                .get(b)
                .and_then(|lb| lb.globals.get(i))
                .ok_or_else(|| Fault::new(9, "global slot out of range")),
            Place::Local(fi, ri) => self
                .frames
                .get(fi)
                .and_then(|f| f.locals.get(ri))
                .ok_or_else(|| Fault::new(9, "local slot out of range")),
        }
    }

    fn read_place_mut(&mut self, place: Place) -> Result<&mut Variant, Fault> {
        match place {
            Place::Global(b, i) => self
                .bundles
                .get_mut(b)
                .and_then(|lb| lb.globals.get_mut(i))
                .ok_or_else(|| Fault::new(9, "global slot out of range")),
            Place::Local(fi, ri) => self
                .frames
                .get_mut(fi)
                .and_then(|f| f.locals.get_mut(ri))
                .ok_or_else(|| Fault::new(9, "local slot out of range")),
        }
    }

    fn write_place(&mut self, place: Place, value: Variant) -> Result<(), Fault> {
        let target = match place {
            Place::Global(b, i) => self.bundles.get_mut(b).and_then(|lb| lb.globals.get_mut(i)),
            Place::Local(fi, ri) => self.frames.get_mut(fi).and_then(|f| f.locals.get_mut(ri)),
        };
        match target {
            Some(cell) => {
                *cell = value;
                Ok(())
            }
            None => Err(Fault::new(9, "slot out of range")),
        }
    }

    fn get(&self, slot: usize) -> Result<&Variant, Fault> {
        let place = self.target(slot)?;
        self.read_place(place)
    }
    fn cloned(&self, slot: usize) -> Result<Variant, Fault> {
        self.get(slot).cloned()
    }
    fn set(&mut self, slot: usize, value: Variant) -> Result<(), Fault> {
        let place = self.target(slot)?;
        self.write_place(place, value)
    }

    fn proc_args_to_values(&self, args: &[ProcArg]) -> Result<Vec<Variant>, Fault> {
        args.iter()
            .map(|arg| match arg {
                ProcArg::ByVal(slot) | ProcArg::ByRef(slot) => self.cloned(*slot),
                ProcArg::Omitted => Ok(Variant::from_error_code(MISSING_ARG)),
            })
            .collect()
    }

    /// The `VB_PredeclaredId` singleton of class `class_idx` in `bundle`, creating it
    /// on first access: allocate the instance (carrying the owning bundle's id, so its
    /// members dispatch against the right class table), cache it, then run
    /// `Class_Initialize` in that bundle. Mirrors `Op::NewObject`/`Op::NewExtern`, but
    /// the instance is cached and shared (one per class per run). The instance is
    /// cached *before* `Class_Initialize` runs, so an access from within
    /// initialization sees the same instance (and the singleton persists for the run,
    /// VBA-consistent — it is never drained/terminated).
    fn predeclared_instance(
        &mut self,
        bundle: BundleId,
        class_idx: usize,
    ) -> Result<Variant, Fault> {
        if let Some(existing) = self.bundles[bundle].predeclared_singletons.get(&class_idx) {
            return Ok(existing.clone());
        }
        let descriptor = *self.bundles[bundle]
            .class_descriptors
            .get(class_idx)
            .ok_or_else(|| Fault::new(5, format!("unknown class {class_idx}")))?;
        let meta = self.bundles[bundle]
            .bundle
            .classes
            .get(class_idx)
            .ok_or_else(|| Fault::new(5, format!("unknown class {class_idx}")))?;
        let has_terminate = meta.terminate.is_some();
        let initialize = meta.initialize;
        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;
        let object = ObjectRef::from_project_instance(
            instance_id,
            class_idx as i32,
            bundle as i32,
            has_terminate,
            descriptor,
        );
        let value = Variant::from_object_ref(object);
        self.bundles[bundle]
            .predeclared_singletons
            .insert(class_idx, value.clone());
        if let Some(init) = initialize {
            let saved_cur = self.cur;
            self.cur = bundle;
            let r = self.run_proc_with_me(init, value.clone(), &[], false, false);
            self.cur = saved_cur;
            r?;
        }
        Ok(value)
    }

    // ── Error handling ─────────────────────────────────────────────────────────
    fn set_err(&mut self, code: i32, message: &str) {
        self.err = ErrObject {
            number: code,
            description: message.to_string(),
            source: String::new(),
        };
    }

    /// Statement bounds for a pc: `(start, next)` from `bundle.statement_starts`
    /// (op-granularity if the table is empty).
    fn statement_bounds(&self, pc: usize) -> ResumePoint {
        let bundle = self.cur_bundle();
        let starts = &bundle.statement_starts;
        if starts.is_empty() {
            return ResumePoint {
                start: pc,
                next: pc + 1,
            };
        }
        let idx = match starts.binary_search(&pc) {
            Ok(i) => i,
            Err(0) => {
                return ResumePoint {
                    start: pc,
                    next: pc + 1,
                };
            }
            Err(i) => i - 1,
        };
        let start = starts[idx];
        let next = starts.get(idx + 1).copied().unwrap_or(bundle.ops.len());
        ResumePoint { start, next }
    }

    /// Route a raised fault for the top-level loop: handle it (Resume/handler) or
    /// unwind to an ancestor handler; an uncaught fault becomes a [`VmError`].
    /// Object locals dropped during the unwind are released and their
    /// `Class_Terminate`s drained (fixing the legacy error-path gap).
    fn dispatch_fault(&mut self, fault: Fault) -> Result<(), VmError> {
        let (code, message) = (fault.code, fault.message.clone());
        let diagnostic = fault.diagnostic.clone();
        let handled = self.route_fault(fault, 1);
        self.maybe_drain();
        if handled {
            Ok(())
        } else {
            Err(VmError {
                code,
                message,
                diagnostic,
            })
        }
    }

    /// Walk handlers / unwind frames until the fault is handled or it would
    /// escape `floor` frames (1 = the entry frame for the top level; `base` for a
    /// nested `Class_Initialize`/`Terminate` run). Returns whether it was handled.
    fn route_fault(&mut self, fault: Fault, floor: usize) -> bool {
        self.set_err(fault.code, &fault.message);
        let mut errored_pc = self.pc;
        loop {
            match self.error_mode {
                ErrorMode::ResumeNext => {
                    self.resume = self.statement_bounds(errored_pc);
                    self.pc = self.resume.next;
                    return true;
                }
                ErrorMode::Goto(handler) => {
                    self.resume = self.statement_bounds(errored_pc);
                    self.pc = handler;
                    return true;
                }
                ErrorMode::None => {
                    if self.frames.len() <= floor {
                        return false;
                    }
                    // Unwind one frame; dropping its locals releases object locals
                    // on every exit path. The error now "occurs" at the call site.
                    let frame = self.frames.pop().unwrap();
                    errored_pc = frame.return_pc.saturating_sub(1);
                    self.error_mode = frame.saved_error_mode;
                    self.resume = frame.saved_resume;
                    // Restore the caller's bundle so `statement_bounds` and the
                    // resumed/handler pc index the right bundle's ops (a fault can
                    // unwind across a cross-bundle `CallExtern` boundary).
                    self.cur = frame.return_bundle;
                }
            }
        }
    }

    // ── Procedure calls ──────────────────────────────────────────────────────
    /// Guard against unbounded recursion: a VBA program that recurses without
    /// terminating must raise "Out of stack space" (error 28), never abort the host
    /// by exhausting memory growing the frame stack. Frames are heap-backed (not the
    /// OS stack), so the limit is generous — far above any legitimate recursion that
    /// real VBA would itself run — yet bounded so an infinite cycle fails cleanly.
    fn guard_call_depth(&self) -> Result<(), Fault> {
        const MAX_CALL_DEPTH: usize = 50_000;
        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(Fault::new(28, "Out of stack space"));
        }
        Ok(())
    }

    fn call_proc(
        &mut self,
        proc: usize,
        dst: Option<usize>,
        args: &[ProcArg],
    ) -> Result<(), Fault> {
        let desc = self
            .cur_bundle()
            .procedures
            .get(proc)
            .ok_or_else(|| Fault::new(5, format!("unknown procedure {proc}")))?;
        let (frame_slots, return_slot, entry, native) = (
            desc.frame_slots,
            desc.return_slot,
            desc.entry_pc,
            desc.native,
        );
        // Native-bodied procs live only in the VBA library bundle and are reached
        // by cross-bundle dispatch (late member dispatch → `run_proc_core`), never
        // by an intra-bundle `CallProc`. Guard rather than push a frame into the
        // placeholder body (which would silently return Empty).
        if native.is_some() {
            return Err(Fault::new(
                5,
                "native-bodied procedure is not callable via CallProc",
            ));
        }

        // Resolve everything in the *caller* context before pushing the callee.
        let dst_place = match dst {
            Some(slot) => Some(self.target(slot)?),
            None => None,
        };
        let mut locals = vec![Variant::empty(); frame_slots];
        let mut aliases = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            match arg {
                ProcArg::ByVal(s) => {
                    let value = self.cloned(*s)?;
                    if i < locals.len() {
                        locals[i] = value;
                    }
                }
                ProcArg::ByRef(s) => {
                    let place = self.target(*s)?;
                    aliases.insert(i, place);
                }
                ProcArg::Omitted => {
                    if i < locals.len() {
                        locals[i] = Variant::from_error_code(MISSING_ARG);
                    }
                }
            }
        }

        self.guard_call_depth()?;
        self.frames.push(Frame {
            locals,
            aliases,
            dst: dst_place,
            return_slot,
            return_pc: self.pc + 1,
            // An ordinary `CallProc` is intra-bundle (cross-bundle calls go through
            // `Op::CallExtern`), so the callee runs in — and returns to — the
            // caller's bundle.
            return_bundle: self.cur,
            capture: false,
            saved_error_mode: self.error_mode,
            saved_resume: self.resume,
        });
        self.error_mode = ErrorMode::None; // each procedure starts with no handler
        self.next_pc = entry;
        Ok(())
    }

    /// A cross-bundle call: resolve `import` (an index into the current bundle's
    /// resolved import table) to a `(target_bundle, proc)`, bind args/dst in the
    /// *caller's* bundle, then push a frame that runs in — and returns to — the
    /// target bundle. Mirrors [`Self::call_proc`] but switches `cur`.
    fn call_extern(
        &mut self,
        import: usize,
        dst: Option<usize>,
        args: &[ProcArg],
    ) -> Result<(), Fault> {
        let resolved = self.bundles[self.cur]
            .imports
            .get(import)
            .copied()
            .ok_or_else(|| Fault::new(5, format!("unresolved import {import}")))?;
        let ExportTarget::Proc(proc) = resolved.target else {
            return Err(Fault::new(5, "cross-bundle reference is not a procedure"));
        };
        let target_bundle = resolved.bundle;
        let desc = self.bundles[target_bundle]
            .bundle
            .procedures
            .get(proc)
            .ok_or_else(|| {
                Fault::new(
                    5,
                    format!("unknown procedure {proc} in unit {target_bundle}"),
                )
            })?;
        let (frame_slots, return_slot, entry, native) = (
            desc.frame_slots,
            desc.return_slot,
            desc.entry_pc,
            desc.native,
        );
        // A native-bodied **library function** (e.g. `Strings.Left`) called
        // cross-bundle runs through `oxvba-lib` directly — no frame is pushed, since
        // its arguments are positional ByVal values and its body needs no VM frame
        // (the `entry_pc` body is the bundle's lone `Return` placeholder). This is
        // the same dispatch as `Op::CallNative { NativeCallee::Builtin(..) }`, so the
        // function behaves identically whether resolved via the `Native` route or
        // (for `Strings`) via this `ExternProc` route. Object *methods*
        // (`NativeBody::Method`) never reach `CallExtern` — they arrive by late
        // member dispatch (`run_proc_core`) — so that arm is rejected.
        match native {
            None => {}
            Some(NativeBody::Library(id)) => {
                let argv = self.extern_native_args(args)?;
                let value = self.invoke_native_lib(id, &argv)?;
                if let Some(slot) = dst {
                    self.set(slot, value)?;
                }
                return Ok(());
            }
            Some(NativeBody::Method(_)) => {
                return Err(Fault::new(
                    5,
                    "native object method is not callable via CallExtern",
                ));
            }
        }

        // Resolve dst + args in the CALLER's bundle (current `cur`) before switching.
        let dst_place = match dst {
            Some(slot) => Some(self.target(slot)?),
            None => None,
        };
        let mut locals = vec![Variant::empty(); frame_slots];
        let mut aliases = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            match arg {
                ProcArg::ByVal(s) => {
                    let value = self.cloned(*s)?;
                    if i < locals.len() {
                        locals[i] = value;
                    }
                }
                ProcArg::ByRef(s) => {
                    let place = self.target(*s)?;
                    aliases.insert(i, place);
                }
                ProcArg::Omitted => {
                    if i < locals.len() {
                        locals[i] = Variant::from_error_code(MISSING_ARG);
                    }
                }
            }
        }
        self.guard_call_depth()?;
        self.frames.push(Frame {
            locals,
            aliases,
            dst: dst_place,
            return_slot,
            return_pc: self.pc + 1,
            return_bundle: self.cur, // restore the caller's bundle on return
            capture: false,
            saved_error_mode: self.error_mode,
            saved_resume: self.resume,
        });
        self.error_mode = ErrorMode::None;
        self.cur = target_bundle; // execute the callee against its own bundle
        self.next_pc = entry;
        Ok(())
    }

    fn ret(&mut self) -> Result<(), Fault> {
        if self.frames.len() <= 1 {
            // Return from the entry frame ends execution.
            self.halted = true;
            return Ok(());
        }
        let frame = self.frames.pop().unwrap();
        let return_value = frame
            .return_slot
            .and_then(|rs| frame.locals.get(rs).cloned());
        self.error_mode = frame.saved_error_mode;
        self.resume = frame.saved_resume;
        self.next_pc = frame.return_pc;
        self.cur = frame.return_bundle;
        // Writes during the call already hit the backing places (true aliasing),
        // so the only copy-out is the function's return value: captured for a
        // nested invocation, otherwise written into the caller place.
        if frame.capture {
            self.captured_return = return_value;
        } else if let (Some(place), Some(value)) = (frame.dst, return_value) {
            self.write_place(place, value)?;
        }
        // Object locals dropped with the frame above may have hit refcount 0;
        // run their `Class_Terminate`s at this procedure epilogue.
        drop(frame);
        self.maybe_drain();
        Ok(())
    }

    /// Run all parked `Class_Terminate`s to completion. Re-entrancy-guarded: a
    /// Terminate that releases children parks them, and the loop picks them up on
    /// the next iteration, so we never recurse into a nested drain.
    fn maybe_drain(&mut self) {
        if self.draining {
            return;
        }
        self.draining = true;
        while oxvba_runtime::has_pending_terminations() {
            for (instance_id, bundle_id, route_key) in oxvba_runtime::take_pending_terminations() {
                let bundle_id = bundle_id as usize;
                let terminate = self
                    .bundles
                    .get(bundle_id)
                    .and_then(|lb| lb.bundle.classes.get(route_key as usize))
                    .and_then(|class| class.terminate);
                if let (Some(proc), Some(object)) = (
                    terminate,
                    oxvba_runtime::retained_parked_termination_object(instance_id),
                ) {
                    // Terminate runs under a fresh error scope in the instance's own
                    // bundle; an unhandled error in it is swallowed (VBA teardown).
                    let saved_cur = self.cur;
                    self.cur = bundle_id;
                    let _ = self.run_proc_with_me(
                        proc,
                        Variant::from_object_ref(object),
                        &[],
                        true,
                        false,
                    );
                    self.cur = saved_cur;
                }
                oxvba_runtime::finish_pending_termination(instance_id);
                // Drop any WithEvents bindings + host subscriptions owned by the
                // terminated instance.
                self.unsubscribe_com_owner(instance_id);
                self.withevents
                    .retain(|key, _| Self::withevents_owner(*key).raw() != instance_id);
            }
        }
        self.draining = false;
    }

    /// Run a project procedure to completion with `me` bound as the hidden first
    /// local and `args` bound at locals `1..`, isolated from the main cursor.
    /// `Class_Initialize`/`Terminate` pass no args; event handlers pass the event
    /// args; late-bound methods pass the call args and set `capture` to recover
    /// the function result. With `suppress`, an unhandled fault is swallowed
    /// (Terminate teardown); otherwise it propagates out.
    /// Resolve call arguments against the caller (current top frame + `cur`): ByVal
    /// args are cloned to values, ByRef args to self-describing `Place`s. The result
    /// is bundle-independent, so the caller may switch `cur` before running the body.
    #[allow(clippy::type_complexity)]
    fn resolve_proc_args(
        &self,
        args: &[ProcArg],
    ) -> Result<(Vec<(usize, Variant)>, HashMap<usize, Place>), Fault> {
        let mut byval: Vec<(usize, Variant)> = Vec::new();
        let mut aliases = HashMap::new();
        for (i, arg) in args.iter().enumerate() {
            let local = 1 + i; // local 0 is `Me`
            match arg {
                ProcArg::ByVal(s) => byval.push((local, self.cloned(*s)?)),
                ProcArg::ByRef(s) => {
                    aliases.insert(local, self.target(*s)?);
                }
                ProcArg::Omitted => byval.push((local, Variant::from_error_code(MISSING_ARG))),
            }
        }
        Ok((byval, aliases))
    }

    fn run_proc_with_me(
        &mut self,
        proc: usize,
        me: Variant,
        args: &[ProcArg],
        suppress: bool,
        capture: bool,
    ) -> Result<Variant, Fault> {
        // Resolve arguments in the caller (current top frame) before pushing.
        let (byval, aliases) = self.resolve_proc_args(args)?;
        self.run_proc_core(proc, me, byval, aliases, suppress, capture)
    }

    /// Like `run_proc_with_me`, but runs the body against `target_bundle` (the
    /// object's / sink's own bundle for a cross-bundle method or event dispatch).
    /// Arguments are still resolved in the **caller's** bundle (current `cur`) before
    /// the switch, so a global-slot argument resolves against the caller's globals.
    fn run_proc_in_bundle(
        &mut self,
        proc: usize,
        me: Variant,
        args: &[ProcArg],
        target_bundle: usize,
        suppress: bool,
        capture: bool,
    ) -> Result<Variant, Fault> {
        let (byval, aliases) = self.resolve_proc_args(args)?;
        let saved_cur = self.cur;
        self.cur = target_bundle;
        let result = self.run_proc_core(proc, me, byval, aliases, suppress, capture);
        self.cur = saved_cur;
        result
    }

    /// Invoke a procedure with `me` and pre-resolved by-value arguments. Used for
    /// inbound COM event handlers, whose args arrive from the host rather than
    /// from caller slots.
    fn run_proc_with_values(
        &mut self,
        proc: usize,
        me: Variant,
        values: Vec<Variant>,
        suppress: bool,
        capture: bool,
    ) -> Result<Variant, Fault> {
        self.run_proc_with_values_from_slot(proc, me, values, 1, suppress, capture)
    }

    fn run_proc_with_values_from_slot(
        &mut self,
        proc: usize,
        me: Variant,
        values: Vec<Variant>,
        first_slot: usize,
        suppress: bool,
        capture: bool,
    ) -> Result<Variant, Fault> {
        let byval = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (first_slot + i, v))
            .collect();
        self.run_proc_core(proc, me, byval, HashMap::new(), suppress, capture)
    }

    fn run_proc_core(
        &mut self,
        proc: usize,
        me: Variant,
        byval: Vec<(usize, Variant)>,
        aliases: HashMap<usize, Place>,
        suppress: bool,
        capture: bool,
    ) -> Result<Variant, Fault> {
        let (native, entry, return_slot, desc_frame_slots) = {
            let desc = self
                .cur_bundle()
                .procedures
                .get(proc)
                .ok_or_else(|| Fault::new(5, format!("unknown procedure {proc}")))?;
            (
                desc.native,
                desc.entry_pc,
                desc.return_slot,
                desc.frame_slots,
            )
        };
        // A native-bodied procedure runs directly with its positional arguments
        // instead of pushing a bytecode frame. `resolve_proc_args` placed args at
        // slots `1..` (slot 0 is `Me`); recover them in order. ByRef args (none
        // today) would arrive via `aliases` and are not yet threaded through.
        //
        // A `Method` body (a built-in object's method, e.g. `Collection.Add`) runs
        // inside the VM via `run_native_method` so it can mutate VM-held instance
        // state. A `Library` body (a base-library function) runs through `oxvba-lib`
        // with no receiver — `Strings` functions reach this path only if dispatched
        // as a member (they are not), but it is handled for completeness/uniformity.
        match native {
            Some(NativeBody::Method(id)) => {
                let mut ordered = byval;
                ordered.sort_by_key(|(slot, _)| *slot);
                let args: Vec<Variant> = ordered.into_iter().map(|(_, value)| value).collect();
                return self.run_native_method(id, me, args);
            }
            Some(NativeBody::Library(id)) => {
                let mut ordered = byval;
                ordered.sort_by_key(|(slot, _)| *slot);
                let args: Vec<Variant> = ordered.into_iter().map(|(_, value)| value).collect();
                return self.invoke_native_lib(id, &args);
            }
            None => {}
        }
        let max_local = byval
            .iter()
            .map(|(l, _)| *l)
            .chain(aliases.keys().copied())
            .max()
            .unwrap_or(0);
        let frame_slots = desc_frame_slots.max(max_local + 1);
        let base = self.frames.len();
        let saved = (
            self.pc,
            self.next_pc,
            self.halted,
            self.error_mode,
            self.resume,
            self.captured_return.take(),
            self.cur,
        );

        let mut locals = vec![Variant::empty(); frame_slots];
        locals[0] = me;
        for (local, value) in byval {
            if local < locals.len() {
                locals[local] = value;
            }
        }
        self.guard_call_depth()?;
        self.frames.push(Frame {
            locals,
            aliases,
            dst: None,
            return_slot,
            return_pc: 0,
            // The proc runs in whatever bundle the caller set as current (the
            // object's bundle for a cross-bundle method dispatch); `cur` is saved
            // above and restored below, so `return_bundle` is informational here.
            return_bundle: self.cur,
            capture,
            saved_error_mode: self.error_mode,
            saved_resume: self.resume,
        });
        self.error_mode = ErrorMode::None;
        self.captured_return = None;
        self.pc = entry;
        self.halted = false;

        let mut result: Result<Variant, Fault> = Ok(Variant::empty());
        while self.frames.len() > base && !self.halted && self.pc < self.cur_bundle().ops.len() {
            if self.bundles[self.cur]
                .statement_start_set
                .contains(&self.pc)
            {
                self.maybe_drain();
                self.pump_com_events();
            }
            let op = self.cur_bundle().ops[self.pc].clone();
            self.next_pc = self.pc + 1;
            match self.exec(&op) {
                Ok(()) => self.pc = self.next_pc,
                Err(fault) => {
                    if !self.route_fault(fault.clone(), base) {
                        if !suppress {
                            result = Err(fault);
                        }
                        break;
                    }
                }
            }
        }
        // Discard any frames left above the baseline (suppressed/forced exit),
        // releasing their object locals.
        while self.frames.len() > base {
            self.frames.pop();
        }
        let captured = self.captured_return.take().unwrap_or_else(Variant::empty);
        self.pc = saved.0;
        self.next_pc = saved.1;
        self.halted = saved.2;
        self.error_mode = saved.3;
        self.resume = saved.4;
        self.captured_return = saved.5;
        self.cur = saved.6;
        result.map(|_| captured)
    }

    /// Invoke a native-bodied library method (a built-in object's member). `me` is
    /// the receiver object; `args` are the positional arguments in declaration
    /// order (omitted optionals arrive as the `MISSING_ARG` sentinel). The built-in
    /// object's mutable state is reached via the receiver's `compat_identity`.
    /// Errors map to VBA run-time error numbers (9 = bad index/key, 13 = type
    /// mismatch). This is the native counterpart of pushing a bytecode frame.
    fn run_native_method(
        &mut self,
        id: NativeMethodId,
        me: Variant,
        args: Vec<Variant>,
    ) -> Result<Variant, Fault> {
        let id_key = variant_to_object(&me)?.compat_identity();
        match id {
            NativeMethodId::CollectionCount => Ok(Variant::from_i32(
                self.collections
                    .get(&id_key)
                    .map_or(0, CollectionData::count),
            )),
            NativeMethodId::CollectionAdd => {
                let value = args.first().cloned().unwrap_or_else(Variant::empty);
                let key = optional_key(args.get(1));
                let before = optional_selector(args.get(2));
                let after = optional_selector(args.get(3));
                self.collections
                    .entry(id_key)
                    .or_default()
                    .add(value, key, before, after)
                    .map_err(collection_fault)?;
                Ok(Variant::empty())
            }
            NativeMethodId::CollectionItem => {
                let sel = required_selector(args.first())?;
                self.collections
                    .get(&id_key)
                    .map_or(Err(CollectionError::NotFound), |c| c.item(&sel))
                    .map_err(collection_fault)
            }
            NativeMethodId::CollectionRemove => {
                let sel = required_selector(args.first())?;
                self.collections
                    .get_mut(&id_key)
                    .map_or(Err(CollectionError::NotFound), |c| c.remove(&sel))
                    .map_err(collection_fault)?;
                Ok(Variant::empty())
            }
        }
    }

    /// Drain inbound host (COM) events: poll the host for delivered callbacks and
    /// dispatch each to the subscribed sink handler. Re-entrancy-guarded; handler
    /// faults are suppressed (events arrive out-of-band from the raiser).
    fn pump_com_events(&mut self) {
        if self.pumping {
            return;
        }
        self.pumping = true;
        loop {
            let payload = match self.host.com().poll_event_callback() {
                Ok(Some(payload)) => payload,
                // `Ok(None)` = nothing pending; `Err` = the host has no event
                // delivery (e.g. the null host) — either way, stop pumping.
                _ => break,
            };
            let sink = self
                .com_subscriptions
                .get(&payload.subscription.raw())
                .map(|sink| (sink.owner.clone(), sink.handler));
            if let Some((owner, handler)) = sink {
                // The polled payload is self-contained: `poll_event_callback`
                // consumes the queued callback and returns its event args inline, so
                // read them straight from the payload. (The per-arg
                // `event_callback_arity`/`event_callback_variant` accessors would
                // miss — the queue entry is already gone after the poll.)
                let values: Vec<Variant> = payload
                    .args
                    .iter()
                    .map(|arg| arg.variant().clone())
                    .collect();
                // The handler is a proc in the sink's (owner's) bundle — run it there.
                let owner_bundle = owner
                    .as_object_ref()
                    .map(|o| o.bundle_id() as usize)
                    .unwrap_or(self.cur);
                let saved_cur = self.cur;
                self.cur = owner_bundle;
                let _ = self.run_proc_with_values(handler, owner, values, true, false);
                self.cur = saved_cur;
            }
            let _ = self
                .host
                .com()
                .release_event_callback_variant(payload.callback);
        }
        self.pumping = false;
    }

    /// Subscribe a WithEvents sink (`owner`) to a COM `source`'s events for the
    /// given binding token, recording each subscription for dispatch + teardown.
    fn subscribe_com_events(
        &mut self,
        key: i64,
        binding_token: i32,
        owner: &Variant,
        source: &ObjectRef,
    ) {
        let routes: Vec<(i32, usize)> = self
            .cur_bundle()
            .event_routes
            .iter()
            .filter(|route| route.binding == binding_token)
            .map(|route| (route.event, route.handler))
            .collect();
        for (event, handler) in routes {
            if let Ok(subscription) = self
                .host
                .com()
                .subscribe_event(source.clone(), ComMemberToken::new(event))
            {
                self.com_subscriptions.insert(
                    subscription.raw(),
                    ComEventSink {
                        owner: owner.clone(),
                        handler,
                    },
                );
                self.com_subscriptions_by_key
                    .entry(key)
                    .or_default()
                    .push(subscription.raw());
            }
        }
    }

    fn unsubscribe_com_key(&mut self, key: i64) {
        if let Some(tokens) = self.com_subscriptions_by_key.remove(&key) {
            for raw in tokens {
                let _ = self
                    .host
                    .com()
                    .unsubscribe_event_variant(ComSubscriptionToken::new(raw));
                self.com_subscriptions.remove(&raw);
            }
        }
    }

    fn unsubscribe_com_owner(&mut self, owner_raw: i32) {
        let keys: Vec<i64> = self
            .com_subscriptions_by_key
            .keys()
            .copied()
            .filter(|key| Self::withevents_owner(*key).raw() == owner_raw)
            .collect();
        for key in keys {
            self.unsubscribe_com_key(key);
        }
    }

    // ── Native dispatch ──────────────────────────────────────────────────────
    fn native_args(&self, args: &[CallArg]) -> Result<Vec<Variant>, Fault> {
        args.iter()
            .map(|a| match a {
                CallArg::Slot(s) | CallArg::ByRef(s) => self.cloned(*s),
                CallArg::Named { slot, .. } => self.cloned(*slot),
                CallArg::Omitted => Ok(Variant::empty()),
                CallArg::Const(value) => Ok(Variant::from_i32(*value)),
            })
            .collect()
    }

    fn native_callback_pointer(&mut self, proc: usize) -> Result<i64, Fault> {
        if proc >= self.cur_bundle().procedures.len() {
            return Err(Fault::new(490, "invalid procedure reference"));
        }
        let registration = NativeCallbackRegistration {
            owner: self.native_callback_owner,
            vm: (self as *mut Vm<'h>) as usize,
            bundle: self.cur,
            proc,
        };
        NATIVE_CALLBACKS.with(|callbacks| {
            let mut callbacks = callbacks.borrow_mut();
            if let Some((index, existing)) =
                callbacks.iter_mut().enumerate().find(|(_, existing)| {
                    existing
                        .map(|existing| {
                            existing.owner == registration.owner
                                && existing.bundle == registration.bundle
                                && existing.proc == registration.proc
                        })
                        .unwrap_or(false)
                })
            {
                *existing = Some(registration);
                return native_callback_thunk(index)
                    .map(|ptr| ptr as isize as i64)
                    .ok_or_else(|| Fault::new(5, "unsupported native callback slot"));
            }
            if let Some(index) = callbacks.iter().position(Option::is_none) {
                callbacks[index] = Some(registration);
                return native_callback_thunk(index)
                    .map(|ptr| ptr as isize as i64)
                    .ok_or_else(|| Fault::new(5, "unsupported native callback slot"));
            }
            Err(Fault::new(7, "native callback table exhausted"))
        })
    }

    /// Assemble the argument Variants for a native-bodied library function reached
    /// cross-bundle (an `Op::CallExtern` whose target proc has a
    /// [`NativeBody::Library`] body). The args are positional ByVal values in the
    /// *caller's* bundle (no ByRef — the binder binds these as ByVal), gathered in
    /// order with **no truncation** to any frame size. An omitted optional slot
    /// becomes `Empty`, exactly as `native_args` maps `CallArg::Omitted`, so a
    /// `Strings` call binds equivalently however it is routed.
    fn extern_native_args(&self, args: &[ProcArg]) -> Result<Vec<Variant>, Fault> {
        args.iter()
            .map(|a| match a {
                ProcArg::ByVal(s) | ProcArg::ByRef(s) => self.cloned(*s),
                ProcArg::Omitted => Ok(Variant::empty()),
            })
            .collect()
    }

    /// Run a base-library function (`NativeImplId`) on already-assembled argument
    /// Variants through `oxvba-lib`, mapping a library error to a VBA fault. The
    /// single dispatch point shared by `Op::CallNative { Builtin }` and a
    /// `NativeBody::Library` proc reached cross-bundle (`call_extern`): both forms
    /// run the identical body, so a library function behaves the same however it is
    /// routed.
    fn invoke_native_lib(&mut self, id: NativeImplId, argv: &[Variant]) -> Result<Variant, Fault> {
        // `TypeName` of an object is host-aware (BUG 5): the pure body cannot read
        // a COM class name and a project instance's name lives in the bundle, so a
        // bare `oxvba_lib::invoke` would return the generic "Object". Resolve the
        // real name here, where both the host COM facet and the class table are in
        // reach; every non-object subtype (numbers/strings/arrays/Nothing/etc.)
        // and the unresolved-object fallback run the unchanged pure body.
        if id == NativeImplId::TypeName
            && let Some(arg) = argv.first()
            && let Some(object) = arg.as_object_ref()
            && let Some(name) = self.object_type_name(object)
        {
            return Ok(Variant::from_string(name));
        }
        oxvba_lib::invoke(id, argv, self.host, &mut self.lib).map_err(Fault::from_lib)
    }

    /// The class/type name `TypeName` reports for an object handle, or `None` to
    /// fall through to the pure library body (which yields "Object"). A project
    /// instance reports its VBA class name (from its own bundle); a foreign COM
    /// object is named by the host (`ComHal::object_type_name`).
    fn object_type_name(&self, object: ObjectRef) -> Option<String> {
        if object.is_project_instance() {
            return self
                .bundles
                .get(object.bundle_id() as usize)
                .and_then(|lb| lb.bundle.classes.get(object.route_key() as usize))
                .map(|class| class.name.clone());
        }
        self.host.com().object_type_name(object).ok().flatten()
    }

    fn arg_object(&self, arg: Option<&CallArg>) -> Result<ObjectRef, Fault> {
        let slot = match arg {
            Some(CallArg::Slot(s)) | Some(CallArg::Named { slot: s, .. }) => *s,
            _ => return Err(Fault::new(424, "Object required")),
        };
        variant_to_object(self.get(slot)?)
    }

    fn com_dispatch(
        &mut self,
        selector: &ComMemberSelector,
        kind_hint: Option<ProjectMemberKind>,
        args: &[CallArg],
    ) -> Result<Variant, Fault> {
        // The receiver is `args[0]`; the rest are the method arguments.
        let object = self.arg_object(args.first())?;
        let method_args = args.split_first().map(|(_, rest)| rest).unwrap_or(&[]);
        self.dispatch_by_object(object, selector, kind_hint, method_args)
    }

    /// Dispatch a member call on an already-resolved object (used by both static
    /// `ComDispatch` lowering and runtime `CallByName`). `method_args` excludes the
    /// receiver. A project-instance receiver dispatches internally (resolve the
    /// class member → procedure, call with `Me`); a COM receiver goes to the host.
    fn dispatch_by_object(
        &mut self,
        object: ObjectRef,
        selector: &ComMemberSelector,
        kind_hint: Option<ProjectMemberKind>,
        method_args: &[CallArg],
    ) -> Result<Variant, Fault> {
        if object.is_project_instance() {
            return self.dispatch_project_method(object, selector, kind_hint, method_args);
        }
        let member = match selector {
            ComMemberSelector::DispatchId(id) => DynamicMemberSelector::Token(*id),
            ComMemberSelector::DispatchIdNamed { id, name } => DynamicMemberSelector::TokenNamed {
                token: *id,
                name: name.clone(),
            },
            ComMemberSelector::Name(name) => DynamicMemberSelector::Name(name.clone()),
        };
        let mut call_args = Vec::new();
        for arg in method_args {
            let (value, name) = match arg {
                CallArg::Slot(s) | CallArg::ByRef(s) => (Some(self.cloned(*s)?), None),
                CallArg::Named { name, slot } => (Some(self.cloned(*slot)?), Some(name.clone())),
                CallArg::Omitted => (None, None),
                CallArg::Const(value) => (Some(Variant::from_i32(*value)), None),
            };
            call_args.push(DynamicCallArg {
                value: value.map(DynamicValue::from_variant),
                name,
            });
        }
        let request = DynamicCallRequest {
            object,
            member,
            args: call_args,
            call_kind_hint: kind_hint.map(member_kind_to_dynamic),
        };
        let (ret, writebacks) = self
            .host
            .com()
            .dispatch_invoke_dynamic_variant_with_writebacks(&request)
            .map_err(Fault::from_hal)?;
        // Apply COM `[out]`/`[in,out]` write-backs to the ByRef call-site slots;
        // only a `CallArg::ByRef` arg (marked from the typelib's param directions)
        // is written back.
        for (j, wb) in writebacks.into_iter().enumerate() {
            if let Some(value) = wb
                && let Some(CallArg::ByRef(slot)) = method_args.get(j)
            {
                self.set(*slot, value)?;
            }
        }
        Ok(ret)
    }

    /// Late-bound dispatch on a project instance: resolve the class member by
    /// name (and accessor kind) → procedure, call it with `Me` + the positional
    /// arguments, and return the function result. `method_args` excludes `Me`.
    fn dispatch_project_method(
        &mut self,
        object: ObjectRef,
        selector: &ComMemberSelector,
        kind_hint: Option<ProjectMemberKind>,
        method_args: &[CallArg],
    ) -> Result<Variant, Fault> {
        let class_idx = object.route_key() as usize;
        // The object's class table lives in the bundle that created it (a cross-
        // bundle dispatch resolves the method by name there, and the method body
        // runs against that bundle — exactly like a COM call into another image).
        let obj_bundle = object.bundle_id() as usize;
        let class = self
            .bundles
            .get(obj_bundle)
            .and_then(|lb| lb.bundle.classes.get(class_idx))
            .ok_or_else(|| Fault::new(438, "Object doesn't support this member"))?;
        let member = match selector {
            ComMemberSelector::Name(name) | ComMemberSelector::DispatchIdNamed { name, .. } => {
                let exact = class.methods.iter().find(|m| {
                    m.name.eq_ignore_ascii_case(name) && kind_hint.is_none_or(|k| k == m.kind)
                });
                exact.or_else(|| {
                    if kind_hint == Some(ProjectMemberKind::PropertyGet) && method_args.is_empty() {
                        class.methods.iter().find(|m| {
                            m.name.eq_ignore_ascii_case(name) && m.kind == ProjectMemberKind::Method
                        })
                    } else if kind_hint == Some(ProjectMemberKind::Method) {
                        class.methods.iter().find(|m| {
                            m.name.eq_ignore_ascii_case(name)
                                && m.kind == ProjectMemberKind::PropertyGet
                        })
                    } else {
                        None
                    }
                })
            }
            ComMemberSelector::DispatchId(0) => {
                if kind_hint.is_none() {
                    unique_read_default_member(class)?
                } else if matches!(
                    kind_hint,
                    Some(ProjectMemberKind::Method | ProjectMemberKind::PropertyGet)
                ) {
                    let exact = class
                        .methods
                        .iter()
                        .find(|m| m.is_default_member && kind_hint == Some(m.kind));
                    match exact {
                        Some(member) => Some(member),
                        None => unique_read_default_member(class)?,
                    }
                } else {
                    let mut exact_matches = class.methods.iter().filter(|m| {
                        m.is_default_member && kind_hint.is_none_or(|kind| kind == m.kind)
                    });
                    let exact = exact_matches.next();
                    if exact_matches.next().is_some() {
                        let names = class
                            .methods
                            .iter()
                            .filter(|m| {
                                m.is_default_member && kind_hint.is_none_or(|kind| kind == m.kind)
                            })
                            .map(|member| format!("{}:{:?}", member.name, member.kind))
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(Fault::new(
                            438,
                            format!(
                                "Object `{}` doesn't support a unique default member for this call ({names})",
                                class.name
                            ),
                        ));
                    }
                    exact
                }
            }
            ComMemberSelector::DispatchId(_) => None,
        };
        let member = member.ok_or_else(|| match selector {
            ComMemberSelector::Name(name) | ComMemberSelector::DispatchIdNamed { name, .. } => {
                Fault::new(438, format!("Object doesn't support '{name}'"))
            }
            ComMemberSelector::DispatchId(0) => Fault::new(
                438,
                format!(
                    "Object `{}` doesn't support a unique default member for this call",
                    class.name
                ),
            ),
            ComMemberSelector::DispatchId(id) => {
                Fault::new(438, format!("Object doesn't support dispatch id {id}"))
            }
        })?;
        let proc = member.proc;
        let proc_args: Vec<ProcArg> = method_args
            .iter()
            .map(|a| match a {
                // ByRef args alias the caller's slot; ByVal/Named copy in.
                // (Parenthesised `(x)` and non-l-values were lowered to `Slot`, so
                // they correctly do not write back.)
                CallArg::ByRef(s) => ProcArg::ByRef(*s),
                CallArg::Slot(s) => ProcArg::ByVal(*s),
                CallArg::Named { slot, .. } => ProcArg::ByVal(*slot),
                // A `Const` arg only arises for library built-ins (e.g. compare
                // mode), never project methods, so it has no place here.
                CallArg::Omitted | CallArg::Const(_) => ProcArg::Omitted,
            })
            .collect();
        self.run_proc_in_bundle(
            proc,
            Variant::from_object_ref(object),
            &proc_args,
            obj_bundle,
            false,
            true,
        )
    }

    fn declare_call(
        &mut self,
        descriptor_id: u32,
        args: &[CallArg],
        ptr_writebacks: &[DeclarePtrWriteback],
    ) -> Result<Variant, Fault> {
        let descriptor = self
            .cur_bundle()
            .external_call(descriptor_id)
            .ok_or_else(|| Fault::new(5, format!("unknown Declare descriptor {descriptor_id}")))?
            .clone();
        let arg_variants = self.native_args(args)?;

        let param_type_strings: Vec<String> = descriptor
            .param_types
            .iter()
            .map(|pt| format!("{pt:?}"))
            .collect();
        let view = DynLinkDescriptorView {
            descriptor_id: descriptor.descriptor_id,
            declared_name: &descriptor.declared_name,
            library: &descriptor.library,
            alias: &descriptor.alias,
            ordinal_alias: descriptor.ordinal_alias,
            symbol: descriptor.symbol,
            marshal_lane: &descriptor.marshal_lane,
            calling_convention: &descriptor.calling_convention,
            selection_policy: &descriptor.selection_policy,
            param_count: descriptor.param_count,
            param_types: &param_type_strings,
            param_by_ref: &descriptor.param_by_ref,
            return_type: descriptor
                .return_type
                .as_ref()
                .map(|rt| Cow::Owned(format!("{rt:?}"))),
        };
        // Pointer-helper pins this call feeds (the `LongLong`-carried registry
        // addresses of `StrPtr`/`VarPtr` args). A pin's life ends with the call it
        // feeds — the VBA "the pointer is valid for the duration of the call"
        // contract — so we free it once the call returns and any write-back has
        // read it back, instead of leaking one cell per helper call. Non-pin integer
        // args are not in the registry and are ignored by `free_pins`. (A pin never
        // passed to a `Declare` is not reclaimed here; that degenerate case is the
        // documented residual.)
        let mut arg_variants = arg_variants;
        for (index, value) in arg_variants.iter_mut().enumerate() {
            let param_type = descriptor
                .param_types
                .get(index)
                .map(|ty| format!("{ty:?}"))
                .unwrap_or_else(|| "Long".to_string());
            if param_type == "LongPtr"
                && !descriptor.param_by_ref.get(index).copied().unwrap_or(false)
                && let Some(proc) = decode_proc_ref(value)
            {
                let pointer = self.native_callback_pointer(proc)?;
                *value = Variant::from_i64(pointer);
            }
        }
        let pin_addrs: Vec<i64> = arg_variants.iter().filter_map(Variant::as_i64).collect();
        let invoke = self
            .host
            .dynlink()
            .invoke_descriptor_variants(&view, &arg_variants);
        // VBA updates `Err.LastDllError` after every `Declare` call (the OS last-error
        // the HAL captured immediately after the native call); non-native lanes report 0.
        self.last_dll_error = self.host.dynlink().last_dll_error();
        let (ret, wb_values) = match invoke {
            Ok(pair) => pair,
            Err(err) => {
                pointer_helpers::free_pins(&pin_addrs);
                return Err(Fault::from_hal(err));
            }
        };
        // Copy each ByRef argument's marshaled-back value to its caller slot. The
        // dynlink host returns `wb_values` aligned to `args`; only `CallArg::ByRef`
        // args write back (a force-ByVal `(x)`/non-l-value lowered to `Slot`, so it
        // is correctly left unchanged).
        for (i, arg) in args.iter().enumerate() {
            if let CallArg::ByRef(slot) = arg
                && let Some(value) = wb_values.get(i)
            {
                self.set(*slot, value.clone())?;
            }
        }
        // Pointer-helper write-back: a `StrPtr(x)`/`VarPtr(x)` argument over an
        // l-value reads the pinned buffer (the native call may have mutated it) back
        // into the source variable. The argument value is the registered pointer; the
        // runtime registry projects it back to a string or byte-array Variant.
        for wb in ptr_writebacks {
            let pointer = arg_variants
                .get(wb.arg_index)
                .and_then(Variant::as_i64)
                .unwrap_or(0);
            let value = match wb.kind {
                PtrWritebackKind::String => {
                    pointer_helpers::read_back_string_payload_variant(pointer)
                        .map_err(Fault::from_string)?
                }
                PtrWritebackKind::ByteArray => {
                    pointer_helpers::read_back_byte_array_payload_variant(pointer)
                        .map_err(Fault::from_string)?
                }
                PtrWritebackKind::Boolean => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Boolean,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::Byte => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Byte,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::Integer => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Integer,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::Long => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Long,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::LongLong => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::LongLong,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::LongPtr => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::LongPtr,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::Single => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Single,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::Double => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Double,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::Currency => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Currency,
                )
                .map_err(Fault::from_string)?,
                PtrWritebackKind::Date => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Date,
                )
                .map_err(Fault::from_string)?,
            };
            self.set(wb.target_slot, value)?;
        }
        // The pins are fully consumed (the call ran and any write-back read them
        // back); release them so the registry stays bounded across looping Declares.
        pointer_helpers::free_pins(&pin_addrs);
        Ok(ret)
    }

    // ── Arrays ───────────────────────────────────────────────────────────────
    fn build_bounds(
        &self,
        upper_bounds: &[usize],
        lower_bounds: &[i32],
    ) -> Result<Vec<SafeArrayBound>, Fault> {
        let mut bounds = Vec::with_capacity(upper_bounds.len());
        for (i, upper_slot) in upper_bounds.iter().enumerate() {
            let lower = lower_bounds.get(i).copied().unwrap_or(0);
            let upper = arith::int(self.get(*upper_slot)?).map_err(Fault::from_arith)? as i32;
            if upper < lower {
                return Err(Fault::new(9, "array upper bound below lower bound"));
            }
            // Bound the element count so a garbage/huge upper raises VBA "Out of
            // memory" (7) instead of attempting an unbounded host allocation that
            // would abort the process (guest code must never crash the VM).
            let span = i64::from(upper) - i64::from(lower) + 1;
            if span > i64::from(u32::MAX) {
                return Err(Fault::new(
                    7,
                    format!("array dimension too large ({span} elements)"),
                ));
            }
            let count = span as u32;
            bounds.push(SafeArrayBound { count, lower });
        }
        Ok(bounds)
    }

    /// Flat element index from VBA (absolute) indices, C-order (first dimension
    /// outermost). Bounds-checked → error 9 on a subscript out of range.
    fn flat_index(&self, indices: &[usize], bounds: &[SafeArrayBound]) -> Result<usize, Fault> {
        if indices.len() != bounds.len() {
            return Err(Fault::new(9, "wrong number of array subscripts"));
        }
        let mut flat = 0usize;
        for (i, index_slot) in indices.iter().enumerate() {
            let raw = arith::int(self.get(*index_slot)?).map_err(Fault::from_arith)? as i32;
            let bound = &bounds[i];
            let offset = i64::from(raw) - i64::from(bound.lower);
            if offset < 0 || offset >= i64::from(bound.count) {
                return Err(Fault::new(9, "subscript out of range"));
            }
            flat = flat * bound.count as usize + offset as usize;
        }
        Ok(flat)
    }

    fn array_of(&self, slot: usize) -> Result<SafeArray, Fault> {
        self.get(slot)?
            .as_safearray()
            .ok_or_else(|| Fault::new(13, "expected an array"))
    }

    fn array_bound_index(
        &self,
        dimension: Option<usize>,
        bounds: &[SafeArrayBound],
    ) -> Result<usize, Fault> {
        let dim = match dimension {
            Some(slot) => arith::int(self.get(slot)?).map_err(Fault::from_arith)?,
            None => 1,
        };
        if dim < 1 {
            return Err(Fault::new(9, "subscript out of range"));
        }
        let index = (dim - 1) as usize;
        if index >= bounds.len() {
            return Err(Fault::new(9, "subscript out of range"));
        }
        Ok(index)
    }

    // ── WithEvents ───────────────────────────────────────────────────────────
    fn withevents_key(owner: &ObjectRef, binding: i64) -> i64 {
        (i64::from(owner.raw()) << 32) | (binding & 0xFFFF_FFFF)
    }
    fn withevents_owner(key: i64) -> ObjectRef {
        ObjectRef::from_compat_identity((key >> 32) as i32)
    }
    fn withevents_binding(key: i64) -> i64 {
        key & 0xFFFF_FFFF
    }

    // ── The instruction dispatch ───────────────────────────────────────────────
    fn exec(&mut self, op: &Op) -> Result<(), Fault> {
        match op {
            // ── Loads ──
            Op::LoadI32 { slot, value } => self.set(*slot, Variant::from_i32(*value))?,
            Op::LoadI64 { slot, value } => self.set(*slot, Variant::from_i64(*value))?,
            Op::LoadBool { slot, value } => self.set(*slot, Variant::from_bool(*value))?,
            Op::LoadString { slot, value } => {
                self.set(*slot, Variant::from_string(value.clone()))?
            }
            Op::LoadF64 { slot, bits } => {
                self.set(*slot, Variant::from_f64(f64::from_bits(*bits)))?
            }
            Op::LoadF32 { slot, bits } => {
                self.set(*slot, Variant::from_f32(f32::from_bits(*bits)))?
            }
            Op::LoadCurrency { slot, scaled } => {
                self.set(*slot, Variant::from_currency_scaled_i64(*scaled))?
            }
            Op::LoadDate { slot, bits } => {
                self.set(*slot, Variant::from_date_f64(f64::from_bits(*bits)))?
            }
            Op::LoadNull { slot } => self.set(*slot, Variant::null())?,
            Op::LoadEmpty { slot } => self.set(*slot, Variant::empty())?,
            Op::LoadProjectObjectRef { dst, handle } => self.set(
                *dst,
                Variant::from_object_ref(ObjectRef::from_compat_identity(*handle as i32)),
            )?,
            // A procedure reference is intentionally not a normal numeric pointer:
            // keep it tagged until a Declare parameter expects a callback pointer,
            // then register a real native thunk for that proc reference.
            Op::LoadProcRef { dst, proc } => self.set(*dst, Variant::from_proc_ref(*proc))?,
            Op::LoadErrLastDllError { slot } => {
                self.set(*slot, Variant::from_i32(self.last_dll_error))?
            }
            Op::LoadErrNumber { slot } => self.set(*slot, Variant::from_i32(self.err.number))?,
            Op::LoadErrDescription { slot } => {
                self.set(*slot, Variant::from_string(self.err.description.clone()))?
            }
            Op::LoadErrSource { slot } => {
                self.set(*slot, Variant::from_string(self.err.source.clone()))?
            }

            // ── Arithmetic ──
            // The fast-path const/inc ops are loop-counter helpers — the widening regime.
            Op::AddConstI32 { slot, value } => {
                let v = arith::add(
                    self.get(*slot)?,
                    &Variant::from_i32(*value),
                    NumericMode::Widening,
                )
                .map_err(Fault::from_arith)?;
                self.set(*slot, v)?;
            }
            Op::SubConstI32 { slot, value } => {
                let v = arith::sub(
                    self.get(*slot)?,
                    &Variant::from_i32(*value),
                    NumericMode::Widening,
                )
                .map_err(Fault::from_arith)?;
                self.set(*slot, v)?;
            }
            Op::IncSlot { slot } => {
                let v = arith::add(
                    self.get(*slot)?,
                    &Variant::from_i32(1),
                    NumericMode::Widening,
                )
                .map_err(Fault::from_arith)?;
                self.set(*slot, v)?;
            }
            Op::Add {
                dst,
                lhs,
                rhs,
                mode,
            } => self.binop(*dst, *lhs, *rhs, |l, r| arith::add(l, r, *mode))?,
            Op::Sub {
                dst,
                lhs,
                rhs,
                mode,
            } => self.binop(*dst, *lhs, *rhs, |l, r| arith::sub(l, r, *mode))?,
            Op::Mul {
                dst,
                lhs,
                rhs,
                mode,
            } => self.binop(*dst, *lhs, *rhs, |l, r| arith::mul(l, r, *mode))?,
            Op::Div { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::div)?,
            Op::IntDiv {
                dst,
                lhs,
                rhs,
                mode,
            } => self.binop(*dst, *lhs, *rhs, |l, r| arith::int_div(l, r, *mode))?,
            Op::Mod {
                dst,
                lhs,
                rhs,
                mode,
            } => self.binop(*dst, *lhs, *rhs, |l, r| arith::modulo(l, r, *mode))?,
            Op::Pow { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::pow)?,
            Op::Concat { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::concat)?,
            Op::Neg { dst, src, mode } => {
                let v = arith::neg(self.get(*src)?, *mode).map_err(Fault::from_arith)?;
                self.set(*dst, v)?;
            }
            Op::Copy { dst, src } => {
                let v = self.cloned(*src)?;
                self.set(*dst, v)?;
            }
            Op::VariantChanged {
                dst,
                current,
                original,
            } => {
                let changed = self.get(*current)? != self.get(*original)?;
                self.set(*dst, Variant::from_bool(changed))?;
            }

            // ── Coercion ──
            Op::CoerceNumeric { slot, target } => {
                let v =
                    arith::coerce_numeric(self.get(*slot)?, *target).map_err(Fault::from_arith)?;
                self.set(*slot, v)?;
            }
            Op::CoerceString { slot } => {
                let v = arith::coerce_string(self.get(*slot)?).map_err(Fault::from_arith)?;
                self.set(*slot, v)?;
            }
            Op::CoerceFixedString { slot, len } => {
                let v = arith::coerce_fixed_string(self.get(*slot)?, *len)
                    .map_err(Fault::from_arith)?;
                self.set(*slot, v)?;
            }
            Op::ValidateAssignment {
                src,
                intent,
                target_kind,
                target_name,
                target_type_name,
            } => {
                self.validate_assignment(
                    *src,
                    *intent,
                    *target_kind,
                    target_name,
                    target_type_name,
                )?;
            }

            // ── Comparison ──
            Op::CmpEq {
                dst,
                lhs,
                rhs,
                mode,
            } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Eq)?,
            Op::CmpNe {
                dst,
                lhs,
                rhs,
                mode,
            } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Ne)?,
            Op::CmpLt {
                dst,
                lhs,
                rhs,
                mode,
            } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Lt)?,
            Op::CmpLe {
                dst,
                lhs,
                rhs,
                mode,
            } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Le)?,
            Op::CmpGt {
                dst,
                lhs,
                rhs,
                mode,
            } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Gt)?,
            Op::CmpGe {
                dst,
                lhs,
                rhs,
                mode,
            } => self.cmp(*dst, *lhs, *rhs, *mode, CmpOp::Ge)?,
            Op::CmpObjectIs { dst, lhs, rhs } => {
                let a = object_identity(self.get(*lhs)?);
                let b = object_identity(self.get(*rhs)?);
                self.set(*dst, Variant::from_bool(a == b))?;
            }

            // ── Boolean ──
            Op::Not { dst, src } => {
                let v = arith::not(self.get(*src)?).map_err(Fault::from_arith)?;
                self.set(*dst, v)?;
            }
            Op::And { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::and)?,
            Op::Or { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::or)?,
            Op::Xor { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::xor)?,
            Op::Eqv { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::eqv)?,
            Op::Imp { dst, lhs, rhs } => self.binop(*dst, *lhs, *rhs, arith::imp)?,

            // ── Control flow ──
            Op::Jump { target_pc } => self.next_pc = *target_pc,
            Op::JumpIfZero {
                cond_slot,
                target_pc,
            } => {
                if !arith::is_truthy(self.get(*cond_slot)?).map_err(Fault::from_arith)? {
                    self.next_pc = *target_pc;
                }
            }
            Op::CallProc { proc, dst, args } => self.call_proc(*proc, *dst, args)?,
            Op::CallExtern { import, dst, args } => self.call_extern(*import, *dst, args)?,
            Op::CallProcRef { dst, target, args } => {
                // Resolve the procedure reference (the AddressOf value) to an index
                // at runtime, then dispatch through the standard call machinery.
                let proc = self
                    .get(*target)?
                    .as_proc_ref()
                    .filter(|&p| p < self.cur_bundle().procedures.len())
                    .ok_or_else(|| Fault::new(490, "invalid procedure reference"))?;
                self.call_proc(proc, *dst, args)?
            }
            Op::CallNative { dst, callee, args } => {
                let value = match callee {
                    NativeCallee::Builtin(id) => {
                        let argv = self.native_args(args)?;
                        self.invoke_native_lib(*id, &argv)?
                    }
                    NativeCallee::ComDispatch {
                        selector,
                        kind_hint,
                        ..
                    } => self.com_dispatch(selector, *kind_hint, args)?,
                    NativeCallee::Declare {
                        descriptor_id,
                        ptr_writebacks,
                    } => self.declare_call(*descriptor_id, args, ptr_writebacks)?,
                };
                if let Some(dst) = dst {
                    self.set(*dst, value)?;
                }
            }
            Op::CallByName {
                dst,
                object,
                name,
                calltype,
                args,
            } => {
                let obj = variant_to_object(self.get(*object)?)?;
                let member_name = arith::as_string(self.get(*name)?);
                let ct = arith::int(self.get(*calltype)?).map_err(Fault::from_arith)?;
                let kind = match ct {
                    1 => Some(ProjectMemberKind::Method),
                    2 => Some(ProjectMemberKind::PropertyGet),
                    4 => Some(ProjectMemberKind::PropertyLet),
                    8 => Some(ProjectMemberKind::PropertySet),
                    _ => return Err(Fault::new(5, "CallByName: invalid CallType")),
                };
                let selector = ComMemberSelector::Name(member_name);
                let value = self.dispatch_by_object(obj, &selector, kind, args)?;
                if let Some(dst) = dst {
                    self.set(*dst, value)?;
                }
            }
            Op::Return => self.ret()?,
            Op::Halt => self.halted = true,

            // ── Error state ──
            Op::SetOnErrorResumeNext => self.error_mode = ErrorMode::ResumeNext,
            Op::SetOnErrorGoto0 => self.error_mode = ErrorMode::None,
            Op::SetOnErrorGotoLabel { target_pc } => self.error_mode = ErrorMode::Goto(*target_pc),
            Op::ResumeNext => self.next_pc = self.resume.next,
            Op::Resume => self.next_pc = self.resume.start,
            Op::ResumeLabel { target_pc } => self.next_pc = *target_pc,
            Op::RaiseError { code } => {
                return Err(Fault::new(*code, default_error_message(*code)));
            }
            Op::RaiseErrorFromSlot { slot } => {
                let code = variant_to_error_code(self.get(*slot)?)?;
                return Err(Fault::new(code, default_error_message(code)));
            }
            Op::ClearErr => self.err = ErrObject::default(),

            // ── Arrays / aggregates ──
            Op::ArrayLiteral { dst, values } => {
                let elems = values
                    .iter()
                    .map(|s| self.cloned(*s))
                    .collect::<Result<Vec<_>, _>>()?;
                self.set(
                    *dst,
                    Variant::from_safearray(SafeArray::from_variants(elems)),
                )?;
            }
            Op::ArrayAppend { dst, array, item } => {
                let mut elems = match self.get(*array)?.as_safearray() {
                    Some(arr) => arr.variant_elements().unwrap_or_default(),
                    None => Vec::new(),
                };
                elems.push(self.cloned(*item)?);
                self.set(
                    *dst,
                    Variant::from_safearray(SafeArray::from_variants(elems)),
                )?;
            }
            Op::ArrayResize {
                dst,
                upper_bounds,
                lower_bounds,
                element_type,
            } => {
                let bounds = self.build_bounds(upper_bounds, lower_bounds)?;
                let count: usize = bounds.iter().map(|b| b.count as usize).product();
                // `from_shape` alone leaves a null payload (no element storage); a
                // freshly `ReDim`-ed array must hold `count` default elements so a
                // following `ArraySet`/`ArrayGet` lands in range. Use the declared
                // element type for the SAFEARRAY storage instead of normalizing typed
                // arrays to VT_VARIANT.
                let elems = (0..count)
                    .map(|_| default_array_element(element_type))
                    .collect();
                let array = redim_safearray_from_elements(bounds, element_type, elems)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_safearray(array))?;
            }
            Op::ArrayResizePreserve {
                dst,
                upper_bounds,
                lower_bounds,
                element_type,
            } => {
                let old = self
                    .get(*dst)?
                    .as_safearray()
                    .and_then(|a| a.variant_elements())
                    .unwrap_or_default();
                let bounds = self.build_bounds(upper_bounds, lower_bounds)?;
                let count: usize = bounds.iter().map(|b| b.count as usize).product();
                // Keep each existing element (its record, for a UDT array); only the
                // grown tail is freshly default-seeded — so `ReDim Preserve` never
                // overwrites a populated element with a blank default record.
                let mut elems: Vec<Variant> = Vec::with_capacity(count);
                for i in 0..count {
                    match old.get(i) {
                        Some(value) => elems.push(value.clone()),
                        None => elems.push(default_array_element(element_type)),
                    }
                }
                let array = redim_safearray_from_elements(bounds, element_type, elems)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_safearray(array))?;
            }
            Op::ArrayGet {
                dst,
                array,
                indices,
            } => {
                // `x(i…)` where `x` is statically a bare `Variant`/`As Object` is
                // lowered as an array subscript, but VBA resolves it at run time: if
                // the value is an OBJECT (not an array) the parentheses are a
                // DEFAULT-MEMBER call (`x.Item(i…)` / dispid 0). Bind-time handles the
                // typed-`Object(name)` receiver; this is the late-bound leg, where
                // `Dim As Object` carries no static class to resolve a default member.
                let value = self.get(*array)?;
                if value.as_safearray().is_none() && value.as_object_ref().is_some() {
                    let object = variant_to_object(self.get(*array)?)?;
                    let method_args: Vec<CallArg> =
                        indices.iter().map(|s| CallArg::Slot(*s)).collect();
                    let result = self.dispatch_by_object(
                        object,
                        &ComMemberSelector::DispatchId(0),
                        Some(ProjectMemberKind::Method),
                        &method_args,
                    )?;
                    self.set(*dst, result)?;
                } else {
                    let arr = self.array_of(*array)?;
                    let bounds = arr
                        .bounds()
                        .ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                    let flat = self.flat_index(indices, &bounds)?;
                    if flat >= arr.len() {
                        return Err(Fault::new(9, "subscript out of range"));
                    }
                    let value = arr.variant_element(flat).map_err(Fault::from_string)?;
                    self.set(*dst, value)?;
                }
            }
            Op::ArraySet {
                array,
                indices,
                src,
            } => {
                // `x(i…) = v` where `x` is statically a bare `Variant`/`As Object`
                // follows the same runtime split as `ArrayGet`: a real array writes
                // an element, while an object receiver writes its default member
                // (`Item`, dispid 0). This also covers ByRef temporary write-back
                // after passing `dict(key)` to a default-ByRef Variant parameter.
                let value = self.get(*array)?;
                if value.as_safearray().is_none() && value.as_object_ref().is_some() {
                    let object = variant_to_object(value)?;
                    let mut method_args: Vec<CallArg> =
                        indices.iter().map(|s| CallArg::Slot(*s)).collect();
                    method_args.push(CallArg::Slot(*src));
                    let kind = if self.get(*src)?.vtype() == VarType::Object {
                        ProjectMemberKind::PropertySet
                    } else {
                        ProjectMemberKind::PropertyLet
                    };
                    let _ = self.dispatch_by_object(
                        object,
                        &ComMemberSelector::DispatchId(0),
                        Some(kind),
                        &method_args,
                    )?;
                    return Ok(());
                }
                let arr = self.array_of(*array)?;
                let bounds = arr
                    .bounds()
                    .ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                let flat = self.flat_index(indices, &bounds)?;
                if flat >= arr.len() {
                    return Err(Fault::new(9, "subscript out of range"));
                }
                let value = self.cloned(*src)?;
                let place = self.target(*array)?;
                self.read_place_mut(place)?
                    .set_safearray_element(flat, &value)
                    .map_err(Fault::from_string)?;
            }

            // ── UDT records (a value aggregate backed by a record store) ──
            // Records use native VBA layout storage when the package carries the
            // recursive field layout. Legacy SAFEARRAY-backed record bags remain
            // readable/writable below for old or hand-built internal values.
            Op::NewRecord { dst, fields } => {
                let layout = vba_record_layout_for_fields(fields).map_err(Fault::from_string)?;
                let record = VbaRecord::new_default(layout).map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_vba_record(record))?;
            }
            Op::RecordGet { dst, record, index } => {
                let source = self.get(*record)?;
                let value = if let Some(rec) = source.as_safearray() {
                    if *index >= rec.len() {
                        return Err(Fault::new(9, "record field out of range"));
                    }
                    rec.variant_element(*index).map_err(Fault::from_string)?
                } else {
                    source
                        .read_record_field_variant(*index)
                        .map_err(Fault::from_string)?
                };
                self.set(*dst, value)?;
            }
            Op::RecordSet { record, index, src } => {
                let value = self.cloned(*src)?;
                let place = self.target(*record)?;
                let target = self.read_place_mut(place)?;
                if let Some(rec) = target.as_safearray() {
                    if *index >= rec.len() {
                        return Err(Fault::new(9, "record field out of range"));
                    }
                    target
                        .set_safearray_element(*index, &value)
                        .map_err(Fault::from_string)?;
                } else {
                    target
                        .write_record_field_variant(*index, &value)
                        .map_err(Fault::from_string)?;
                }
            }
            Op::LBound {
                dst,
                src,
                dimension,
            } => {
                let arr = self.array_of(*src)?;
                let bounds = arr
                    .bounds()
                    .ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                let bound = &bounds[self.array_bound_index(*dimension, &bounds)?];
                let lower = bound.lower;
                self.set(*dst, Variant::from_i32(lower))?;
            }
            Op::UBound {
                dst,
                src,
                dimension,
            } => {
                let arr = self.array_of(*src)?;
                let bounds = arr
                    .bounds()
                    .ok_or_else(|| Fault::new(9, "array has no bounds"))?;
                let bound = &bounds[self.array_bound_index(*dimension, &bounds)?];
                let upper = bound.lower + bound.count as i32 - 1;
                self.set(*dst, Variant::from_i32(upper))?;
            }
            Op::ForEachInit { iter, src } => {
                let source = self.get(*src)?.clone();
                // A built-in `Collection` enumerates its values in insertion order;
                // otherwise the source is an array (SafeArray). The snapshot is taken
                // at loop entry, matching the existing array behaviour.
                let elements = if let Some(obj) = source.as_object_ref() {
                    if let Some(collection) = self.collections.get(&obj.compat_identity()) {
                        // A VBA built-in `Collection` (or a project instance backed by
                        // the in-VM store) enumerates its values in insertion order.
                        collection.values()
                    } else if obj.is_project_instance() {
                        // A project instance with no Collection backing has nothing to
                        // enumerate; preserve the existing empty-iteration behaviour.
                        Vec::new()
                    } else {
                        // A foreign COM collection: snapshot its elements through the
                        // host's IEnumVARIANT bridge (BUG 3). A host that cannot
                        // enumerate (no COM transport, or the object exposes no
                        // enumerator) yields an empty iteration, as before.
                        self.host.com().enumerate_object(obj).unwrap_or_default()
                    }
                } else if let Some(arr) = source.as_safearray() {
                    arr.variant_elements().unwrap_or_default()
                } else {
                    Vec::new()
                };
                let key = self.target(*iter)?;
                self.for_each.insert(
                    key,
                    ForEachState {
                        elements,
                        position: 0,
                    },
                );
            }
            Op::ForEachNext {
                iter,
                item,
                has_value,
            } => {
                let key = self.target(*iter)?;
                let next = self.for_each.get_mut(&key).and_then(|state| {
                    let value = state.elements.get(state.position).cloned();
                    if value.is_some() {
                        state.position += 1;
                    }
                    value
                });
                match next {
                    Some(value) => {
                        self.set(*item, value)?;
                        self.set(*has_value, Variant::from_bool(true))?;
                    }
                    None => self.set(*has_value, Variant::from_bool(false))?,
                }
            }

            // ── Objects / WithEvents / type identity ──
            Op::WithEventsGet {
                dst,
                owner,
                binding,
            } => {
                let key = self.withevents_lookup_key(*owner, *binding)?;
                let value = self
                    .withevents
                    .get(&key)
                    .map(|binding| binding.source.clone())
                    .unwrap_or_else(|| Variant::from_i32(0));
                self.set(*dst, value)?;
            }
            Op::WithEventsSet {
                dst,
                owner,
                binding,
                value,
            } => {
                let owner_value = self.cloned(*owner)?;
                let owner_ref = variant_to_object(&owner_value)?;
                let binding_tok = arith::int(self.get(*binding)?).map_err(Fault::from_arith)?;
                let key = Self::withevents_key(&owner_ref, binding_tok);
                let v = self.cloned(*value)?;
                // Replacing a binding tears down its old host subscriptions first.
                self.unsubscribe_com_key(key);
                if is_nothing(&v) {
                    self.withevents.remove(&key);
                } else {
                    // A COM/foreign source is wired through the host's connection
                    // points; a project source is dispatched internally by
                    // RaiseEvent (no host subscription).
                    if let Some(source) = v.as_object_ref()
                        && !source.is_project_instance()
                    {
                        self.subscribe_com_events(key, binding_tok as i32, &owner_value, &source);
                    }
                    self.withevents.insert(
                        key,
                        EventBinding {
                            owner: owner_value,
                            source: v.clone(),
                        },
                    );
                }
                self.set(*dst, v)?;
            }
            Op::WithEventsClearOwner { dst, owner } => {
                let owner_ref = variant_to_object(self.get(*owner)?)?;
                self.unsubscribe_com_owner(owner_ref.raw());
                self.withevents
                    .retain(|key, _| Self::withevents_owner(*key).raw() != owner_ref.raw());
                self.set(*dst, Variant::from_i32(0))?;
            }
            Op::WithEventsFirstOwner {
                dst,
                source,
                binding,
            } => {
                let source = self.cloned(*source)?;
                let binding = arith::int(self.get(*binding)?).map_err(Fault::from_arith)?;
                let mut owners: Vec<ObjectRef> = Vec::new();
                if !is_nothing(&source) {
                    for (key, binding_data) in &self.withevents {
                        if Self::withevents_binding(*key) == (binding & 0xFFFF_FFFF)
                            && object_identity(&binding_data.source) == object_identity(&source)
                            && let Some(owner) = binding_data.owner.as_object_ref()
                        {
                            owners.push(owner);
                        }
                    }
                }
                owners.sort_unstable_by_key(ObjectRef::raw);
                match owners.first().cloned() {
                    Some(first) => {
                        self.withevents_iters.push((owners, 1));
                        self.set(*dst, Variant::from_object_ref(first))?;
                    }
                    None => self.set(*dst, Variant::from_i32(0))?,
                }
            }
            Op::WithEventsNextOwner { dst } => {
                let next = self.withevents_iters.last_mut().and_then(|(owners, pos)| {
                    let value = owners.get(*pos).cloned();
                    if value.is_some() {
                        *pos += 1;
                    }
                    value
                });
                match next {
                    Some(owner) => self.set(*dst, Variant::from_object_ref(owner))?,
                    None => {
                        self.withevents_iters.pop();
                        self.set(*dst, Variant::from_i32(0))?;
                    }
                }
            }
            Op::TypeOfIs {
                dst,
                object_slot,
                type_name,
            } => {
                let matches = self.type_of_is(*object_slot, type_name)?;
                self.set(*dst, Variant::from_bool(matches))?;
            }

            // ── Project objects (New / fields) ──
            Op::NewObject { dst, class } => {
                let class_idx = *class;
                let descriptor = *self.bundles[self.cur]
                    .class_descriptors
                    .get(class_idx)
                    .ok_or_else(|| Fault::new(5, format!("unknown class {class_idx}")))?;
                let meta = self
                    .cur_bundle()
                    .classes
                    .get(class_idx)
                    .ok_or_else(|| Fault::new(5, format!("unknown class {class_idx}")))?;
                let has_terminate = meta.terminate.is_some();
                let initialize = meta.initialize;
                let instance_id = self.next_instance_id;
                self.next_instance_id += 1;
                let object = ObjectRef::from_project_instance(
                    instance_id,
                    class_idx as i32,
                    self.cur as i32,
                    has_terminate,
                    descriptor,
                );
                if let Some(init) = initialize {
                    self.run_proc_with_me(
                        init,
                        Variant::from_object_ref(object.clone()),
                        &[],
                        false,
                        false,
                    )?;
                }
                self.set(*dst, Variant::from_object_ref(object))?;
            }
            Op::NewExtern { dst, import } => {
                // Allocate an instance of a class in another bundle. The object
                // carries the target bundle's id, so its methods dispatch against
                // that bundle's class table (see `dispatch_project_method`).
                let resolved = self.bundles[self.cur]
                    .imports
                    .get(*import)
                    .copied()
                    .ok_or_else(|| Fault::new(5, format!("unresolved import {import}")))?;
                let ExportTarget::Class(class_idx) = resolved.target else {
                    return Err(Fault::new(5, "cross-bundle reference is not a class"));
                };
                let target_bundle = resolved.bundle;
                let descriptor = *self.bundles[target_bundle]
                    .class_descriptors
                    .get(class_idx)
                    .ok_or_else(|| Fault::new(5, format!("unknown class {class_idx}")))?;
                let meta = self.bundles[target_bundle]
                    .bundle
                    .classes
                    .get(class_idx)
                    .ok_or_else(|| Fault::new(5, format!("unknown class {class_idx}")))?;
                let has_terminate = meta.terminate.is_some();
                let initialize = meta.initialize;
                let instance_id = self.next_instance_id;
                self.next_instance_id += 1;
                let object = ObjectRef::from_project_instance(
                    instance_id,
                    class_idx as i32,
                    target_bundle as i32,
                    has_terminate,
                    descriptor,
                );
                if let Some(init) = initialize {
                    let saved_cur = self.cur;
                    self.cur = target_bundle;
                    let r = self.run_proc_with_me(
                        init,
                        Variant::from_object_ref(object.clone()),
                        &[],
                        false,
                        false,
                    );
                    self.cur = saved_cur;
                    r?;
                }
                self.set(*dst, Variant::from_object_ref(object))?;
            }
            Op::PredeclaredInstance { dst, class } => {
                let value = self.predeclared_instance(self.cur, *class)?;
                self.set(*dst, value)?;
            }
            Op::PredeclaredInstanceExtern { dst, import } => {
                let resolved = self.bundles[self.cur]
                    .imports
                    .get(*import)
                    .copied()
                    .ok_or_else(|| Fault::new(5, format!("unresolved import {import}")))?;
                let ExportTarget::Class(class_idx) = resolved.target else {
                    return Err(Fault::new(
                        5,
                        "cross-bundle predeclared reference is not a class",
                    ));
                };
                let value = self.predeclared_instance(resolved.bundle, class_idx)?;
                self.set(*dst, value)?;
            }
            Op::FieldGet { dst, object, field } => {
                let instance = variant_to_object(self.get(*object)?)?;
                let value = instance
                    .project_field_get(*field)
                    .unwrap_or_else(Variant::empty);
                self.set(*dst, value)?;
            }
            Op::FieldSet { object, field, src } => {
                let value = self.cloned(*src)?;
                let instance = variant_to_object(self.get(*object)?)?;
                instance.project_field_set(*field, value);
            }
            Op::RaiseEvent {
                source,
                event,
                args,
            } => {
                let source_object = variant_to_object(&self.cloned(*source)?)?;
                let source_id = source_object.raw();
                let event_id = *event;
                let external_args = if self.project_event_sink.is_some() {
                    Some(self.proc_args_to_values(args)?)
                } else {
                    None
                };
                // Collect subscribers (sink Me + handler + sink bundle) whose binding
                // holds this source and routes this event. The route + handler live in
                // the SINK's bundle (a cross-bundle WithEvents sink), and the raised
                // `event_id` equals the surface id the sink's routes were built from.
                let mut targets: Vec<(i32, Variant, usize, usize)> = Vec::new();
                for (key, binding) in &self.withevents {
                    if object_identity(&binding.source) != source_id {
                        continue;
                    }
                    let token = Self::withevents_binding(*key) as i32;
                    let sink_bundle = binding
                        .owner
                        .as_object_ref()
                        .map(|o| o.bundle_id() as usize)
                        .unwrap_or(self.cur);
                    // Bounds-checked like every other runtime-`bundle_id` lookup
                    // (`dispatch_project_method`, `type_of_is`, …): a stale/foreign
                    // owner bundle id degrades to "no route" rather than panicking.
                    if let Some(&handler) = self
                        .bundles
                        .get(sink_bundle)
                        .and_then(|lb| lb.event_routes.get(&(token, event_id)))
                    {
                        let sink_id = binding.owner.as_object_ref().map(|o| o.raw()).unwrap_or(0);
                        targets.push((sink_id, binding.owner.clone(), handler, sink_bundle));
                    }
                }
                targets.sort_by_key(|(sink_id, ..)| *sink_id);
                for (_, sink, handler, sink_bundle) in targets {
                    // Handlers run in sequence in the sink's bundle; an unhandled error
                    // propagates to the raiser. `args` follow the event signature and
                    // resolve in the raiser's bundle before the dispatch.
                    self.run_proc_in_bundle(handler, sink, args, sink_bundle, false, false)?;
                }
                if let (Some(sink), Some(external_args)) =
                    (self.project_event_sink.as_mut(), external_args)
                {
                    sink(source_object, event_id, external_args).map_err(Fault::from_string)?;
                }
            }

            // ── Pointer helpers ──
            // `StrPtr`/`VarPtr` pin a cloned cell in the process-global registry;
            // the native `declare_call` that consumes the pointer frees the pin
            // afterwards (see `declare_call`). `PtrObj` returns a live IUnknown
            // address (no registry cell), so there is nothing to free.
            Op::PtrStr { dst, src } => {
                let p = pointer_helpers::register_utf16_string(&arith::as_string(self.get(*src)?))
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrVar { dst, src } => {
                let p = pointer_helpers::register_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrVarString { dst, src } => {
                let p = pointer_helpers::register_string_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrVarVariant { dst, src } => {
                let p = pointer_helpers::register_variant_var_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
            Op::PtrObj { dst, src } => {
                let p = pointer_helpers::register_object_variant_pointer(self.get(*src)?)
                    .map_err(Fault::from_string)?;
                self.set(*dst, Variant::from_i64(p))?;
            }
        }
        Ok(())
    }

    // ── Small op helpers ───────────────────────────────────────────────────────
    fn binop(
        &mut self,
        dst: usize,
        lhs: usize,
        rhs: usize,
        f: impl Fn(&Variant, &Variant) -> Result<Variant, arith::ArithError>,
    ) -> Result<(), Fault> {
        let value = f(self.get(lhs)?, self.get(rhs)?).map_err(Fault::from_arith)?;
        self.set(dst, value)
    }

    fn cmp(
        &mut self,
        dst: usize,
        lhs: usize,
        rhs: usize,
        mode: oxvba_bundle::StringCompareMode,
        op: CmpOp,
    ) -> Result<(), Fault> {
        let value =
            arith::compare(self.get(lhs)?, self.get(rhs)?, mode, op).map_err(Fault::from_arith)?;
        self.set(dst, value)
    }

    fn validate_assignment(
        &mut self,
        src: usize,
        intent: oxvba_bundle::AssignmentIntent,
        target_kind: oxvba_bundle::AssignmentTargetKind,
        target_name: &str,
        target_type_name: &str,
    ) -> Result<(), Fault> {
        use oxvba_bundle::{AssignmentIntent as Intent, AssignmentTargetKind as Kind};
        let value = self.get(src)?;
        let is_object = matches!(value.vtype(), VarType::Object) || is_nothing(value);
        match intent {
            Intent::Set if !is_object => {
                Err(Fault::new(424, format!("Object required: {target_name}")))
            }
            Intent::Let if target_kind == Kind::Object && is_object => Err(Fault::new(
                91,
                format!("Object variable requires Set: {target_name}"),
            )),
            Intent::Let if target_kind == Kind::Object => {
                Err(Fault::new(424, format!("Object required: {target_name}")))
            }
            // Strict `Set` type check (error 13): when the target's declared type is
            // a known project class/interface, a project-instance source must be
            // that class or implement that interface. Unconstrained targets
            // (`Object`/`Variant`, or any non-project type) are not checked, and
            // `Nothing` is always allowed.
            Intent::Set if value.vtype() == VarType::Object && !target_type_name.is_empty() => {
                let obj = variant_to_object(value)?;
                // Both the target type and the source's class are resolved in the
                // source object's **own** bundle, against the bare target name (a
                // referenced type may be `Lib.Widget`) — unambiguous in a DAG.
                let bare_target = target_type_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(target_type_name);
                if obj.is_project_instance()
                    && let Some(lb) = self.bundles.get(obj.bundle_id() as usize)
                {
                    let target_is_project = lb.bundle.classes.iter().any(|c| {
                        c.name.eq_ignore_ascii_case(bare_target)
                            || c.implements
                                .iter()
                                .any(|i| i.eq_ignore_ascii_case(bare_target))
                    });
                    if target_is_project
                        && let Some(class) = lb.bundle.classes.get(obj.route_key() as usize)
                    {
                        let compatible = class.name.eq_ignore_ascii_case(bare_target)
                            || class
                                .implements
                                .iter()
                                .any(|i| i.eq_ignore_ascii_case(bare_target));
                        if !compatible {
                            return Err(Fault::new(
                                13,
                                format!(
                                    "Type mismatch: `{}` cannot be assigned to `{target_type_name}`",
                                    class.name
                                ),
                            ));
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn withevents_lookup_key(&self, owner: usize, binding: usize) -> Result<i64, Fault> {
        let owner_ref = variant_to_object(self.get(owner)?)?;
        let binding = arith::int(self.get(binding)?).map_err(Fault::from_arith)?;
        Ok(Self::withevents_key(&owner_ref, binding))
    }

    fn type_of_is(&self, object_slot: usize, type_name: &str) -> Result<bool, Fault> {
        let object = variant_to_object(self.get(object_slot)?)?;
        // A project instance is its own class and every interface it `Implements`;
        // a foreign/COM object is described by the host's typelib (`prog_id_name`).
        // The class is looked up in the object's **own** bundle and matched against
        // the bare type name (a referenced type may be written `Lib.IShape`); the
        // lookup-in-its-own-bundle makes the bare match unambiguous in a DAG.
        if object.is_project_instance() {
            let bare = type_name.rsplit('.').next().unwrap_or(type_name);
            return Ok(self
                .bundles
                .get(object.bundle_id() as usize)
                .and_then(|lb| lb.bundle.classes.get(object.route_key() as usize))
                .is_some_and(|class| {
                    class.name.eq_ignore_ascii_case(bare)
                        || class
                            .implements
                            .iter()
                            .any(|i| i.eq_ignore_ascii_case(bare))
                }));
        }
        let bare = type_name.rsplit('.').next().unwrap_or(type_name);
        if let Ok(Some(name)) = self.host.com().object_type_name(object.clone())
            && (name.eq_ignore_ascii_case(type_name) || name.eq_ignore_ascii_case(bare))
        {
            return Ok(true);
        }
        match self.host.com().describe_object(object) {
            Ok(Some(descriptor)) => {
                let descriptor_bare = descriptor
                    .prog_id_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(&descriptor.prog_id_name);
                Ok(descriptor.prog_id_name.eq_ignore_ascii_case(type_name)
                    || descriptor_bare.eq_ignore_ascii_case(bare))
            }
            _ => Ok(false),
        }
    }
}

fn decode_proc_ref(value: &Variant) -> Option<usize> {
    value.as_proc_ref()
}

fn native_callback_thunk(index: usize) -> Option<*const c_void> {
    Some(match index {
        0 => native_timer_callback_0 as *const c_void,
        1 => native_timer_callback_1 as *const c_void,
        2 => native_timer_callback_2 as *const c_void,
        3 => native_timer_callback_3 as *const c_void,
        4 => native_timer_callback_4 as *const c_void,
        5 => native_timer_callback_5 as *const c_void,
        6 => native_timer_callback_6 as *const c_void,
        7 => native_timer_callback_7 as *const c_void,
        8 => native_timer_callback_8 as *const c_void,
        9 => native_timer_callback_9 as *const c_void,
        10 => native_timer_callback_10 as *const c_void,
        11 => native_timer_callback_11 as *const c_void,
        12 => native_timer_callback_12 as *const c_void,
        13 => native_timer_callback_13 as *const c_void,
        14 => native_timer_callback_14 as *const c_void,
        15 => native_timer_callback_15 as *const c_void,
        16 => native_timer_callback_16 as *const c_void,
        17 => native_timer_callback_17 as *const c_void,
        18 => native_timer_callback_18 as *const c_void,
        19 => native_timer_callback_19 as *const c_void,
        20 => native_timer_callback_20 as *const c_void,
        21 => native_timer_callback_21 as *const c_void,
        22 => native_timer_callback_22 as *const c_void,
        23 => native_timer_callback_23 as *const c_void,
        24 => native_timer_callback_24 as *const c_void,
        25 => native_timer_callback_25 as *const c_void,
        26 => native_timer_callback_26 as *const c_void,
        27 => native_timer_callback_27 as *const c_void,
        28 => native_timer_callback_28 as *const c_void,
        29 => native_timer_callback_29 as *const c_void,
        30 => native_timer_callback_30 as *const c_void,
        31 => native_timer_callback_31 as *const c_void,
        _ => return None,
    })
}

unsafe fn invoke_native_timer_callback(
    index: usize,
    hwnd: *mut c_void,
    message: u32,
    timer_id: usize,
    time: u32,
) {
    let registration =
        NATIVE_CALLBACKS.with(|callbacks| callbacks.borrow().get(index).copied().flatten());
    let Some(registration) = registration else {
        return;
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: native timer registrations are created from a live `Vm` and are
        // removed before that VM is dropped; the callback table stores the same
        // pointer only for the duration of that registration.
        let vm = unsafe { &mut *(registration.vm as *mut Vm<'static>) };
        let saved_cur = vm.cur;
        vm.cur = registration.bundle;
        let _ = vm.run_proc_with_values_from_slot(
            registration.proc,
            Variant::empty(),
            vec![
                Variant::from_i64(hwnd as isize as i64),
                Variant::from_i32(message as i32),
                Variant::from_i64(timer_id as i64),
                Variant::from_i32(time as i32),
            ],
            0,
            true,
            false,
        );
        vm.cur = saved_cur;
    }));
}

macro_rules! native_timer_callback {
    ($name:ident, $index:expr) => {
        unsafe extern "system" fn $name(
            hwnd: *mut c_void,
            message: u32,
            timer_id: usize,
            time: u32,
        ) {
            // SAFETY: Windows calls this thunk with the timer callback ABI; the
            // index is a compile-time slot that maps to the thread-local
            // registration table used by `invoke_native_timer_callback`.
            unsafe { invoke_native_timer_callback($index, hwnd, message, timer_id, time) };
        }
    };
}

native_timer_callback!(native_timer_callback_0, 0);
native_timer_callback!(native_timer_callback_1, 1);
native_timer_callback!(native_timer_callback_2, 2);
native_timer_callback!(native_timer_callback_3, 3);
native_timer_callback!(native_timer_callback_4, 4);
native_timer_callback!(native_timer_callback_5, 5);
native_timer_callback!(native_timer_callback_6, 6);
native_timer_callback!(native_timer_callback_7, 7);
native_timer_callback!(native_timer_callback_8, 8);
native_timer_callback!(native_timer_callback_9, 9);
native_timer_callback!(native_timer_callback_10, 10);
native_timer_callback!(native_timer_callback_11, 11);
native_timer_callback!(native_timer_callback_12, 12);
native_timer_callback!(native_timer_callback_13, 13);
native_timer_callback!(native_timer_callback_14, 14);
native_timer_callback!(native_timer_callback_15, 15);
native_timer_callback!(native_timer_callback_16, 16);
native_timer_callback!(native_timer_callback_17, 17);
native_timer_callback!(native_timer_callback_18, 18);
native_timer_callback!(native_timer_callback_19, 19);
native_timer_callback!(native_timer_callback_20, 20);
native_timer_callback!(native_timer_callback_21, 21);
native_timer_callback!(native_timer_callback_22, 22);
native_timer_callback!(native_timer_callback_23, 23);
native_timer_callback!(native_timer_callback_24, 24);
native_timer_callback!(native_timer_callback_25, 25);
native_timer_callback!(native_timer_callback_26, 26);
native_timer_callback!(native_timer_callback_27, 27);
native_timer_callback!(native_timer_callback_28, 28);
native_timer_callback!(native_timer_callback_29, 29);
native_timer_callback!(native_timer_callback_30, 30);
native_timer_callback!(native_timer_callback_31, 31);

// ── Free helpers ──────────────────────────────────────────────────────────────

fn redim_safearray_from_elements(
    bounds: Vec<SafeArrayBound>,
    element_type: &ArrayElementType,
    elems: Vec<Variant>,
) -> Result<SafeArray, String> {
    if let ArrayElementType::Record(fields) = element_type {
        let layout = vba_record_layout_for_fields(fields)?;
        let records = elems
            .into_iter()
            .map(|value| {
                value
                    .as_vba_record()
                    .ok_or_else(|| "UDT array element default was not a VBA record".to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        return SafeArray::from_vba_records_nd(bounds, layout, records);
    }
    SafeArray::from_typed_variants_nd(bounds, safearray_vartype_for_element(element_type), elems)
}

fn safearray_vartype_for_element(element_type: &ArrayElementType) -> u16 {
    match element_type {
        ArrayElementType::Variant => VT_VARIANT_VALUE,
        ArrayElementType::Record(_) => VT_RECORD_VALUE,
        ArrayElementType::FixedArray { .. } => VT_VARIANT_VALUE,
        ArrayElementType::Integer => VT_I2_VALUE,
        ArrayElementType::Long => VT_I4_VALUE,
        ArrayElementType::LongPtr => {
            if core::mem::size_of::<usize>() == 8 {
                VT_I8_VALUE
            } else {
                VT_I4_VALUE
            }
        }
        ArrayElementType::LongLong => VT_I8_VALUE,
        ArrayElementType::Byte => VT_UI1_VALUE,
        ArrayElementType::Single => VT_R4_VALUE,
        ArrayElementType::Double => VT_R8_VALUE,
        ArrayElementType::Currency => VT_CY_VALUE,
        ArrayElementType::Date => VT_DATE_VALUE,
        ArrayElementType::String => VT_BSTR_VALUE,
        ArrayElementType::Boolean => VT_BOOL_VALUE,
    }
}

/// The default value for a freshly-`ReDim`-ed array element. A UDT-record element
/// is a native VBA record with descriptor-backed field offsets, built recursively
/// so nested scalar-UDT subfields are themselves default records — mirroring the
/// binder's `emit_udt_record_init` so `Lines(i).Words(j).Rect.X` reads back.
/// Scalar defaults are typed zero values so typed SAFEARRAY storage can encode
/// them directly instead of relying on a boundary-time Variant projection.
fn default_array_element(element_type: &ArrayElementType) -> Variant {
    match element_type {
        ArrayElementType::Variant => Variant::empty(),
        ArrayElementType::Integer => Variant::from_i16(0),
        ArrayElementType::Long => Variant::from_i32(0),
        ArrayElementType::LongPtr => {
            if core::mem::size_of::<usize>() == 8 {
                Variant::from_i64(0)
            } else {
                Variant::from_i32(0)
            }
        }
        ArrayElementType::LongLong => Variant::from_i64(0),
        ArrayElementType::Byte => Variant::from_u8(0),
        ArrayElementType::Single => Variant::from_f32(0.0),
        ArrayElementType::Double => Variant::from_f64(0.0),
        ArrayElementType::Currency => Variant::from_currency_scaled_i64(0),
        ArrayElementType::Date => Variant::from_date_f64(0.0),
        ArrayElementType::String => Variant::from_string(""),
        ArrayElementType::Boolean => Variant::from_bool(false),
        ArrayElementType::Record(fields) => {
            let layout = vba_record_layout_for_fields(fields)
                .expect("bundle UDT array element layout should map to VBA record layout");
            let record = VbaRecord::new_default(layout)
                .expect("default VBA record allocation should succeed");
            Variant::from_vba_record(record)
        }
        ArrayElementType::FixedArray { .. } => Variant::empty(),
    }
}

fn vba_record_layout_for_fields(
    fields: &[ArrayElementType],
) -> Result<Arc<VbaRecordLayout>, String> {
    let specs = fields
        .iter()
        .map(|field| Ok(VbaRecordFieldSpec::anonymous(vba_record_field_kind(field)?)))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Arc::new(VbaRecordLayout::new(specs)?))
}

fn vba_record_field_kind(element_type: &ArrayElementType) -> Result<VbaRecordFieldKind, String> {
    let kind = match element_type {
        ArrayElementType::Variant => VbaRecordFieldKind::Variant,
        ArrayElementType::Integer => VbaRecordFieldKind::Integer,
        ArrayElementType::Long => VbaRecordFieldKind::Long,
        ArrayElementType::LongLong => VbaRecordFieldKind::LongLong,
        ArrayElementType::LongPtr => VbaRecordFieldKind::LongPtr,
        ArrayElementType::Byte => VbaRecordFieldKind::Byte,
        ArrayElementType::Single => VbaRecordFieldKind::Single,
        ArrayElementType::Double => VbaRecordFieldKind::Double,
        ArrayElementType::Currency => VbaRecordFieldKind::Currency,
        ArrayElementType::Date => VbaRecordFieldKind::Date,
        ArrayElementType::String => VbaRecordFieldKind::String,
        ArrayElementType::Boolean => VbaRecordFieldKind::Boolean,
        ArrayElementType::Record(fields) => {
            VbaRecordFieldKind::Record(vba_record_layout_for_fields(fields)?)
        }
        ArrayElementType::FixedArray { element, len } => VbaRecordFieldKind::FixedArray {
            element: Box::new(vba_record_field_kind(element)?),
            len: *len,
        },
    };
    Ok(kind)
}

fn member_kind_to_dynamic(kind: ProjectMemberKind) -> DynamicCallKind {
    match kind {
        ProjectMemberKind::Method => DynamicCallKind::Method,
        ProjectMemberKind::PropertyGet => DynamicCallKind::PropertyGet,
        ProjectMemberKind::PropertyLet => DynamicCallKind::PropertyLet,
        ProjectMemberKind::PropertySet => DynamicCallKind::PropertySet,
    }
}

fn unique_read_default_member(class: &ClassDescriptor) -> Result<Option<&ClassMethod>, Fault> {
    let read_matches = class
        .methods
        .iter()
        .filter(|m| {
            m.is_default_member
                && matches!(
                    m.kind,
                    ProjectMemberKind::Method | ProjectMemberKind::PropertyGet
                )
        })
        .collect::<Vec<_>>();
    match read_matches.as_slice() {
        [] => Ok(None),
        [member] => Ok(Some(*member)),
        matches => {
            let names = matches
                .iter()
                .map(|member| format!("{}:{:?}", member.name, member.kind))
                .collect::<Vec<_>>()
                .join(", ");
            Err(Fault::new(
                438,
                format!(
                    "Object `{}` doesn't support a unique default member for this call ({names})",
                    class.name
                ),
            ))
        }
    }
}

/// A Variant carrying a COM/project object handle: an object ref, or an `i32`/
/// `i64` handle.
fn variant_to_object(value: &Variant) -> Result<ObjectRef, Fault> {
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    // An unset object variable — a typed object reference holding `Nothing`
    // (vtype Object, null payload) or a `Dim x As Object` that was never `Set`
    // (defaults to `Empty`) — is "Object variable or With block variable not
    // set" (91), distinct from a non-object value (424). Dereferencing an unset
    // object (`Dim x As Object` / `Set x = Nothing` then `x.Foo`) raises 91.
    if matches!(
        value.vtype(),
        VarType::Object | VarType::Empty | VarType::Null
    ) {
        return Err(Fault::new(
            91,
            "Object variable or With block variable not set",
        ));
    }
    if let Some(raw) = value.as_i32() {
        return Ok(ObjectRef::from_compat_identity(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ObjectRef::from_compat_identity)
            .map_err(|_| Fault::new(13, "object handle exceeds i32 range"));
    }
    Err(Fault::new(424, "Object required"))
}

fn variant_to_error_code(value: &Variant) -> Result<i32, Fault> {
    if let Some(code) = value.as_i32() {
        return Ok(code);
    }
    if let Some(code) = value.as_i64() {
        return i32::try_from(code).map_err(|_| Fault::new(13, "Type mismatch"));
    }
    if let Some(code) = value.as_f64() {
        if code.is_finite()
            && code.fract() == 0.0
            && code >= i32::MIN as f64
            && code <= i32::MAX as f64
        {
            return Ok(code as i32);
        }
    }
    Err(Fault::new(13, "Type mismatch"))
}

/// True for the omitted-optional-argument sentinel placed by `resolve_proc_args`.
fn is_missing_arg(v: &Variant) -> bool {
    v.as_error_code() == Some(MISSING_ARG)
}

/// A `Collection`/`Item`/`Remove`/`Add before|after` selector from a Variant: a
/// string argument is a (folded) key, anything else a 1-based index (numeric
/// coercion is lenient; a non-numeric, non-string value yields index 0, i.e. out
/// of range → error 9).
fn variant_selector(v: &Variant) -> Selector {
    if let Some(s) = v.as_bstr() {
        Selector::Key(s.as_str().to_ascii_lowercase())
    } else {
        let index = v
            .as_i32()
            .or_else(|| v.as_i64().map(|n| n as i32))
            .or_else(|| v.as_f64().map(|n| n.round() as i32))
            .unwrap_or(0);
        Selector::Index(index)
    }
}

/// The folded string key from an `Add` key argument, or `None` if omitted.
fn optional_key(arg: Option<&Variant>) -> Option<String> {
    let v = arg?;
    (!is_missing_arg(v))
        .then(|| v.as_bstr().map(|s| s.as_str().to_ascii_lowercase()))
        .flatten()
}

/// A `Selector` from an optional `before`/`after` argument, or `None` if omitted.
fn optional_selector(arg: Option<&Variant>) -> Option<Selector> {
    let v = arg?;
    (!is_missing_arg(v)).then(|| variant_selector(v))
}

/// A required `Item`/`Remove` selector. Omitted ⇒ error 449.
fn required_selector(arg: Option<&Variant>) -> Result<Selector, Fault> {
    match arg {
        Some(v) if !is_missing_arg(v) => Ok(variant_selector(v)),
        _ => Err(Fault::new(449, "Argument not optional")),
    }
}

/// Map a collection data-model error onto its VBA run-time error number.
fn collection_fault(err: CollectionError) -> Fault {
    match err {
        CollectionError::NotFound => Fault::new(9, "Subscript out of range"),
        CollectionError::DuplicateKey => Fault::new(
            457,
            "This key is already associated with an element of this collection",
        ),
        CollectionError::BadArgument => Fault::new(5, "Invalid procedure call or argument"),
    }
}

/// Object-identity key for `Is`: the object raw, or 0 for Nothing/non-object.
fn object_identity(value: &Variant) -> i32 {
    value.as_object_ref().map(|o| o.raw()).unwrap_or(0)
}

fn is_nothing(value: &Variant) -> bool {
    match value.vtype() {
        VarType::Object => value.as_object_ref().map(|o| o.raw()).unwrap_or(0) == 0,
        VarType::Empty | VarType::Null => true,
        _ => value.as_i32() == Some(0),
    }
}

fn default_error_message(code: i32) -> String {
    match code {
        6 => "Overflow".to_string(),
        9 => "Subscript out of range".to_string(),
        11 => "Division by zero".to_string(),
        13 => "Type mismatch".to_string(),
        91 => "Object variable or With block variable not set".to_string(),
        424 => "Object required".to_string(),
        _ => "Application-defined or object-defined error".to_string(),
    }
}

#[cfg(test)]
mod tests;
