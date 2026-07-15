//! `oxvba-vm3` — the typed-CFG reference interpreter for OxIR.
//!
//! VM3 is the sole product interpreter and the JIT parity reference. It uses a
//! typed register/place model, heap frame stack, ByRef aliases, error/Resume
//! routing, arrays/records/objects/classes, project events and shared
//! runtime/evaluation/library/host substrates. The retired Op/Bundle VM2 path is
//! not an execution fallback.
//!
//! Historical note: `vm2` (the `oxvba-vm2` crate) was the predecessor Op/Bundle
//! interpreter and has been **deleted**. Comments below that say "mirrors vm2's
//! X", "matching vm2", or reference a `vm2` op/method name describe the frame,
//! call, lifecycle and error conventions vm3 inherited from that design — the
//! behavior they document is vm3's own; there is no live `vm2` to consult.
//!
//! VM3 is broad but remains in progress for the complete verified OxIR and
//! Windows interop profiles. Its golden snapshot is regression evidence; public
//! VBA specifications and reproducible Excel/VBA behavior remain semantic
//! authority. See `docs/spec/OXVBA_OXIR_AND_IMAGE_CONTRACT_V1.md`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ptr::NonNull;

use oxvba_bundle::{
    ArrayElementType, AssignmentIntent, AssignmentTargetKind, NativeImplId, ProjectMemberKind,
    array_element_type_for_vartype, default_array_element, redim_safearray_from_elements,
    safearray_vartype_for_element, vba_record_layout_for_fields,
};
use oxvba_com::{
    ComMemberToken, ComSubscriptionToken, DynamicCallArg, DynamicCallKind, DynamicCallRequest,
    DynamicMemberSelector, DynamicValue, TypeLibMemberInvokeKind,
};
use oxvba_eval::arith::{self, ArithError};
use oxvba_eval::collection::{CollectionError, CollectionMethod, dispatch_collection};
use oxvba_hal::HostServices;
use oxvba_hal::traits::DynLinkDescriptorView;
use oxvba_oxir::inst::OxAsNew;
use oxvba_oxir::value::{
    ArithOp, BoundWhich, CmpOp, ErrField, LogicalOp, OxArg, OxCallArg, OxCoerceTarget, OxConst,
    OxNativeCallee, OxOperand, OxPlace, PtrKind, PtrWritebackKind,
};
use oxvba_oxir::{
    BlockId, FuncId, ImportId, LocalId, OxBlock, OxInst, OxProgram, OxTerminator, OxTy,
};
use oxvba_rt_abi::{
    ComEventSink, ErrorMode, EventBinding, ExecState, Fault, FaultAction, LoadedProgram,
    MarshalArgRef, ProcInvoker, ResumePoint, ResumeTarget, SavedErrState, apply_byref_writebacks,
    apply_optional_byref_writebacks, marshal_ox_args, marshal_ox_call_args, take_termination_batch,
    variant_changed,
};
use oxvba_runtime::object_ref::{
    ObjectRef, RUNTIME_CLASS_LIFECYCLE_NONE, RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
    RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, RuntimeClassActivationDescriptor,
    RuntimeClassAsNewFieldDescriptor, RuntimeClassDescriptor, RuntimeClassFieldDescriptor,
    RuntimeClassLifecycleDescriptor, RuntimeGuid, RuntimeInterfaceDescriptor, RuntimeInterfaceId,
    RuntimeInterfaceIdentity, RuntimeInterfaceKind, RuntimeMemberDescriptor,
    RuntimeMemberInvokeKind, RuntimeParamDescriptor, RuntimeProjectClassIdentity, RuntimeValueType,
};
use oxvba_runtime::safe_array::{SafeArray, SafeArrayBound};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{
    CallbackExecutor, CallbackRegistration, RuntimeByRefSlot, Variant, VbaRecord, pointer_helpers,
    register_callback,
};

/// `DISP_E_PARAMNOTFOUND` — the sentinel an omitted optional argument carries into a
/// callee slot, so `IsMissing`/`IsError` observe it exactly as vm2 does.
const MISSING_ARG: i32 = 0x8002_0004u32 as i32;

/// The reserved `route_key` stamped on a built-in `Collection` instance (W3). Distinct from
/// every real project class index (those are small non-negative) so dispatch recognises a
/// Collection receiver BEFORE the `program.classes` lookup, and distinct from any instance id
/// so `is_project_instance()` (compat_identity != route_key) still holds.
const VBA_COLLECTION_ROUTE_KEY: i32 = i32::MIN;

/// The runtime QI descriptor for a built-in `Collection` (name + IUnknown). Fully `'static`,
/// so `New Collection` needs no per-instance descriptor leak.
static VBA_COLLECTION_DESCRIPTOR: RuntimeClassDescriptor = RuntimeClassDescriptor {
    name: "Collection",
    project_identity: None,
    predeclared: false,
    lifecycle: RUNTIME_CLASS_LIFECYCLE_NONE,
    fields: &[],
    as_new_fields: &[],
    implements: &[],
    interfaces: &[RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR],
};

fn synthetic_dispatch_id(index: usize, default_member: bool) -> i32 {
    if default_member {
        0
    } else {
        i32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .unwrap_or(i32::MAX)
    }
}

fn runtime_member_invoke_kind(kind: ProjectMemberKind) -> RuntimeMemberInvokeKind {
    match kind {
        ProjectMemberKind::Method => RuntimeMemberInvokeKind::Method,
        ProjectMemberKind::PropertyGet => RuntimeMemberInvokeKind::PropertyGet,
        ProjectMemberKind::PropertyLet => RuntimeMemberInvokeKind::PropertyLet,
        ProjectMemberKind::PropertySet => RuntimeMemberInvokeKind::PropertySet,
    }
}

fn leak_runtime_str(value: &str) -> &'static str {
    Box::leak(value.to_string().into_boxed_str())
}

fn hidden_me_receiver_param_count(func: &oxvba_oxir::OxFunc) -> usize {
    usize::from(
        func.param_count > 0
            && func.locals.first().is_some_and(|local| {
                local.param.is_some() && local.name.eq_ignore_ascii_case("Me")
            }),
    )
}

fn runtime_member_params(func: &oxvba_oxir::OxFunc) -> Vec<RuntimeParamDescriptor> {
    func.locals
        .iter()
        .take(func.param_count)
        .skip(hidden_me_receiver_param_count(func))
        .map(|local| {
            let param = local.param.as_ref();
            RuntimeParamDescriptor {
                name: leak_runtime_str(&local.name),
                value_type: runtime_value_type(&local.ty),
                by_ref: param.map(|p| p.by_ref).unwrap_or(false),
                optional: param.map(|p| p.optional).unwrap_or(false),
                param_array: param.map(|p| p.variadic).unwrap_or(false),
            }
        })
        .collect()
}

struct RuntimeMemberDescriptorInput<'a> {
    program: &'a OxProgram,
    method: &'a oxvba_oxir::program::OxClassMethod,
    display_name: &'a str,
    dispatch_index: usize,
    dispatch_id: Option<i32>,
    vtable_slot: Option<u16>,
    is_default_member: bool,
    is_enumerator_member: bool,
}

fn runtime_member_descriptor(input: RuntimeMemberDescriptorInput<'_>) -> RuntimeMemberDescriptor {
    let RuntimeMemberDescriptorInput {
        program,
        method,
        display_name,
        dispatch_index,
        dispatch_id,
        vtable_slot,
        is_default_member,
        is_enumerator_member,
    } = input;
    let proc = program.funcs.get(method.proc.0);
    let params: &'static [RuntimeParamDescriptor] = Box::leak(
        proc.map(runtime_member_params)
            .unwrap_or_default()
            .into_boxed_slice(),
    );
    RuntimeMemberDescriptor {
        name: leak_runtime_str(display_name),
        dispatch_id: dispatch_id
            .unwrap_or_else(|| synthetic_dispatch_id(dispatch_index, is_default_member)),
        vtable_slot,
        invoke_kind: runtime_member_invoke_kind(method.kind),
        arity: params.len(),
        params,
        return_type: proc.and_then(runtime_return_type),
        is_default_member,
        is_enumerator_member,
    }
}

fn runtime_return_type(func: &oxvba_oxir::OxFunc) -> Option<RuntimeValueType> {
    func.return_local
        .and_then(|local| func.locals.get(local.0))
        .map(|local| runtime_value_type(&local.ty))
}

fn runtime_value_type(ty: &OxTy) -> RuntimeValueType {
    match ty {
        OxTy::Bool => RuntimeValueType::Boolean,
        OxTy::Byte => RuntimeValueType::Byte,
        OxTy::Integer => RuntimeValueType::Integer,
        OxTy::Long => RuntimeValueType::Long,
        OxTy::LongLong => RuntimeValueType::LongLong,
        OxTy::Currency => RuntimeValueType::Currency,
        OxTy::Single => RuntimeValueType::Single,
        OxTy::Double => RuntimeValueType::Double,
        OxTy::Date => RuntimeValueType::Date,
        OxTy::Decimal => RuntimeValueType::Decimal,
        OxTy::Str | OxTy::FixedStr(_) => RuntimeValueType::String,
        OxTy::Object(_) => RuntimeValueType::Object,
        OxTy::Record(_) => RuntimeValueType::Record,
        OxTy::Array(_, _) => RuntimeValueType::Array,
        OxTy::Variant | OxTy::ProcRef => RuntimeValueType::Variant,
    }
}

const PROJECT_INTERFACE_GUID_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PROJECT_INTERFACE_GUID_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a64(seed: u64, input: &str) -> u64 {
    let mut hash = seed;
    for byte in input.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PROJECT_INTERFACE_GUID_PRIME);
    }
    hash
}

fn runtime_project_interface_guid(unit_name: &str, interface_name: &str) -> RuntimeGuid {
    let key = format!(
        "oxvba:project-interface:{}:{}",
        unit_name.to_ascii_lowercase(),
        interface_name.to_ascii_lowercase()
    );
    let hi = fnv1a64(PROJECT_INTERFACE_GUID_OFFSET, &key);
    let lo = fnv1a64(!PROJECT_INTERFACE_GUID_OFFSET, &key);
    let lo_bytes = lo.to_be_bytes();
    RuntimeGuid::new((hi >> 32) as u32, (hi >> 16) as u16, hi as u16, lo_bytes)
}

fn runtime_array_element_type(element: &ArrayElementType) -> RuntimeValueType {
    match element {
        ArrayElementType::Variant => RuntimeValueType::Variant,
        ArrayElementType::Integer => RuntimeValueType::Integer,
        ArrayElementType::Long => RuntimeValueType::Long,
        ArrayElementType::LongLong => RuntimeValueType::LongLong,
        ArrayElementType::Byte => RuntimeValueType::Byte,
        ArrayElementType::Single => RuntimeValueType::Single,
        ArrayElementType::Double => RuntimeValueType::Double,
        ArrayElementType::Currency => RuntimeValueType::Currency,
        ArrayElementType::Date => RuntimeValueType::Date,
        ArrayElementType::String | ArrayElementType::FixedString(_) => RuntimeValueType::String,
        ArrayElementType::Boolean => RuntimeValueType::Boolean,
        ArrayElementType::Record(_) => RuntimeValueType::Record,
        ArrayElementType::FixedArray { .. } => RuntimeValueType::Array,
    }
}

fn runtime_class_field_descriptor(field: &oxvba_oxir::OxClassField) -> RuntimeClassFieldDescriptor {
    RuntimeClassFieldDescriptor {
        name: leak_runtime_str(&field.name),
        token: field.token,
        value_type: runtime_value_type(&field.ty),
        array_element_type: field.array_element.as_ref().map(runtime_array_element_type),
    }
}

fn runtime_class_activation_descriptor(binding: &OxAsNew) -> RuntimeClassActivationDescriptor {
    match binding {
        OxAsNew::ProjectClass { class } => RuntimeClassActivationDescriptor::ProjectClass {
            class_index: class.0,
        },
        OxAsNew::ExternClass { import } => RuntimeClassActivationDescriptor::ExternClass {
            import_index: import.0,
        },
        OxAsNew::ComClass { prog_id } => RuntimeClassActivationDescriptor::ComClass {
            prog_id: leak_runtime_str(prog_id),
        },
    }
}

fn runtime_as_new_field_descriptor(
    field: &oxvba_oxir::program::OxClassAsNewField,
) -> RuntimeClassAsNewFieldDescriptor {
    RuntimeClassAsNewFieldDescriptor {
        field_token: field.field,
        activation: runtime_class_activation_descriptor(&field.binding),
    }
}

fn runtime_project_interface_descriptor(
    program: &OxProgram,
    class: &oxvba_oxir::OxClass,
    interface_name: &str,
) -> Option<RuntimeInterfaceDescriptor> {
    let bare_interface = interface_name.rsplit('.').next().unwrap_or(interface_name);
    let interface_class = program
        .classes
        .iter()
        .find(|candidate| candidate.name.eq_ignore_ascii_case(bare_interface))?;
    let mut members = Vec::new();
    for interface_method in &interface_class.methods {
        let mangled = format!("{bare_interface}_{}", interface_method.name);
        let Some(implementation_method) = class.methods.iter().find(|method| {
            method.kind == interface_method.kind && method.name.eq_ignore_ascii_case(&mangled)
        }) else {
            continue;
        };
        let index = members.len();
        members.push(runtime_member_descriptor(RuntimeMemberDescriptorInput {
            program,
            method: implementation_method,
            display_name: &interface_method.name,
            dispatch_index: index,
            dispatch_id: interface_method.dispid.or(implementation_method.dispid),
            vtable_slot: interface_method
                .vtable_slot
                .or(implementation_method.vtable_slot),
            is_default_member: interface_method.is_default_member
                || implementation_method.is_default_member,
            is_enumerator_member: interface_method.is_enumerator_member
                || implementation_method.is_enumerator_member,
        }));
    }
    let members: &'static [RuntimeMemberDescriptor] = Box::leak(members.into_boxed_slice());
    let qualified_name = if program.unit_name.is_empty() {
        bare_interface.to_string()
    } else {
        format!("{}.{}", program.unit_name, bare_interface)
    };
    let identity_name = leak_runtime_str(&qualified_name);
    Some(RuntimeInterfaceDescriptor {
        id: RuntimeInterfaceId::Unsupported,
        identity: RuntimeInterfaceIdentity::custom(
            runtime_project_interface_guid(&program.unit_name, bare_interface),
            identity_name,
            RuntimeInterfaceKind::Custom,
            None,
            None,
            None,
        ),
        name: leak_runtime_str(bare_interface),
        members,
        dual_dispatch: false,
    })
}

/// A failure to execute an OxIR program on vm3.
#[derive(Debug, Clone)]
pub enum Vm3Error {
    /// An uncaught VBA run-time fault propagated to the top level.
    Fault(Fault),
    /// An OxIR construct vm3 does not yet execute (honest and explicit — never a
    /// silent mis-execution). Lands in a later milestone (calls/builtins: M2-b;
    /// error/`Resume`: M2-c; objects/COM/arrays: M3).
    Unimplemented { what: &'static str },
    /// A structurally invalid program (should not occur for verifier-clean OxIR).
    Malformed(String),
}

impl std::fmt::Display for Vm3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vm3Error::Fault(fault) => write!(f, "uncaught error {}: {}", fault.code, fault.message),
            Vm3Error::Unimplemented { what } => write!(f, "vm3 does not yet execute: {what}"),
            Vm3Error::Malformed(m) => write!(f, "malformed OxIR: {m}"),
        }
    }
}

impl std::error::Error for Vm3Error {}

/// A resolved runtime storage location on the frame stack — what an [`OxPlace`]
/// denotes once ByRef aliasing is applied. `Local`/`Temp` name a specific frame by
/// index so a callee's ByRef parameter can point at one of its caller's cells (which
/// always outlives it, since callers sit below callees and pop later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Loc {
    /// `(program, slot)`: the program tags the slot so a ByRef alias or a call's result dst
    /// captured in one program stays bound to that program's globals regardless of the
    /// executing `cur` (mirrors vm2's `Place::Global(bundle, slot)`).
    Global(usize, usize),
    Local(usize, usize),
    Temp(usize, usize),
}

impl Loc {
    fn frame_index(self) -> Option<usize> {
        match self {
            Loc::Global(_, _) => None,
            Loc::Local(frame, _) | Loc::Temp(frame, _) => Some(frame),
        }
    }
}

fn is_cleared_temp(loc: Loc, frame_index: usize, first_temp: usize) -> bool {
    matches!(loc, Loc::Temp(frame, temp) if frame == frame_index && temp >= first_temp)
}

/// `For Each` iterator state: the snapshot of source elements (taken at loop entry,
/// matching vm2) and the current position. Keyed in [`Vm3::for_each`] by the loop
/// variable's resolved [`Loc`], so concurrent/reentrant loops never alias.
struct ForEachState {
    elements: Vec<Variant>,
    position: usize,
}

/// One procedure activation: its dispatch position, value slots, ByRef aliasing, and
/// the linkage back to its caller. The activation stack holds these so dispatch is an
/// explicit loop (no native recursion → deep VBA recursion is bounded by the frame
/// ceiling, error 28, not a host stack overflow — and matches vm2's iterative model).
struct Frame {
    /// The program (index into `Vm3::programs`) whose `funcs`/`globals` this frame executes
    /// against. `run_loop` sets `cur` from it each iteration, so a cross-program call/return
    /// re-targets globals and instruction fetch automatically. Single-project runs are all `0`.
    prog: usize,
    /// The function this frame is executing.
    func: FuncId,
    /// The current block and the index of the *next* instruction within it.
    block: BlockId,
    ip: usize,
    /// Frame locals (parameters first, then declared locals), indexed by `LocalId`.
    locals: Vec<Variant>,
    /// Single-assignment temporaries, indexed by `TempId` (sparse — written before read).
    temps: HashMap<usize, Variant>,
    /// ByRef parameters: a parameter's frame-local index → the caller location it
    /// aliases, resolved to its ultimate backing at call time (so aliases never chain).
    /// Writes through such a parameter hit the backing live — vm2's true aliasing.
    aliases: HashMap<usize, Loc>,
    /// Where this call's return value is written (resolved in the caller at call time);
    /// `None` for a statement call or the entry/initializer frame.
    dst: Option<Loc>,
    /// The local holding this function's result (`None` for a `Sub`).
    return_local: Option<LocalId>,
    /// The most recent numeric line label executed in this procedure activation.
    /// This is copied to the VM's public `Erl` value only when an error is caught.
    current_line: i32,
    /// The caller's error mode, restored when this frame returns (each callee starts
    /// with no handler).
    saved_error_mode: ErrorMode,
    /// The caller's active-error latch, restored on return (each callee starts with no
    /// active error). Keeping it per-activation is what makes a propagated `Resume`
    /// re-run the *caller's* call-site statement.
    saved_active_error: Option<ResumePoint>,
    /// The `GoSub` Resumption List — a per-activation LIFO stack of return blocks
    /// (MS-VBAL §5.4.2.14). `GoSub` pushes its `ret`; `Return` pops the most recent.
    gosub_stack: Vec<BlockId>,
}

/// The vm3 interpreter over one or more typed OxIR programs.
pub struct Vm3<'h> {
    /// Index into `programs` of the executing program (`0` for single-project activation).
    cur: usize,
    /// Shared runtime state below vm3 and the JIT.
    exec: ExecState<'h>,
    /// The activation stack. `frames[0]` is the entry (`Main`) frame and is never
    /// popped — it backs the result snapshot; deeper frames are `CallProc` callees.
    frames: Vec<Frame>,
    /// `For Each` iterator state, keyed by the loop variable's resolved [`Loc`] (so
    /// reentrant/nested loops that reuse a slot number never alias) — mirrors vm2's
    /// `for_each` map.
    for_each: HashMap<Loc, ForEachState>,
    /// ParamArray packs whose elements alias caller slots. Keyed by the resolved
    /// location that currently stores the ParamArray SAFEARRAY.
    param_array_aliases: HashMap<Loc, Vec<Option<Loc>>>,
    /// `As New` object slots, keyed by the resolved storage location. A read of
    /// one of these slots creates a fresh instance when the slot is Empty/Nothing.
    as_new_slots: HashMap<Loc, OxAsNew>,
}

impl<'h> Vm3<'h> {
    /// The currently-executing program's static IR (`programs[cur]`). A `&'h` copy, so callers
    /// can hold it across `&mut self` mutations exactly as the old `self.program` field allowed.
    #[inline]
    fn cur_program(&self) -> &'h OxProgram {
        self.exec.programs[self.cur].program
    }

    /// Initial slot value for a statically typed VBA variable. Dynamic array variables are
    /// arrays before allocation for `IsArray`, but have no SAFEARRAY descriptor/bounds until
    /// `ReDim`.
    fn initial_value_for_slot(ty: &OxTy, array_element: Option<&ArrayElementType>) -> Variant {
        match ty {
            OxTy::Array(element, _) => {
                let element = array_element
                    .cloned()
                    .or_else(|| Self::array_element_type_for_ox_ty(element))
                    .unwrap_or(ArrayElementType::Variant);
                Variant::unallocated_array(safearray_vartype_for_element(&element))
            }
            _ => Variant::empty(),
        }
    }

    fn array_element_type_for_ox_ty(ty: &OxTy) -> Option<ArrayElementType> {
        let element = match ty {
            OxTy::Bool => ArrayElementType::Boolean,
            OxTy::Byte => ArrayElementType::Byte,
            OxTy::Integer => ArrayElementType::Integer,
            OxTy::Long => ArrayElementType::Long,
            OxTy::LongLong => ArrayElementType::LongLong,
            OxTy::Single => ArrayElementType::Single,
            OxTy::Double => ArrayElementType::Double,
            OxTy::Currency => ArrayElementType::Currency,
            OxTy::Date => ArrayElementType::Date,
            OxTy::Str => ArrayElementType::String,
            OxTy::FixedStr(len) => ArrayElementType::FixedString(*len as usize),
            OxTy::Variant => ArrayElementType::Variant,
            _ => ArrayElementType::Variant,
        };
        Some(element)
    }

    /// Build a program's mutable runtime tables: one leaked `&'static` runtime descriptor per
    /// class (the shape `ObjectRef::from_project_instance` requires, as vm2's `LoadedBundle`
    /// leaks them; bounded by class count), the `(binding,event)→handler` route table, and the
    /// zeroed module-global slots.
    fn build_loaded(program: &'h OxProgram) -> LoadedProgram<'h> {
        let class_descriptors: Vec<&'static RuntimeClassDescriptor> = program
            .classes
            .iter()
            .enumerate()
            .map(|(class_index, class)| {
                Self::build_runtime_class_descriptor(program, class_index, class)
            })
            .collect();
        let mut event_routes: HashMap<(i32, i32), usize> = HashMap::new();
        for route in &program.event_routes {
            event_routes.insert((route.binding, route.event), route.handler);
        }
        LoadedProgram {
            program,
            globals: program
                .globals
                .iter()
                .map(|global| {
                    Self::initial_value_for_slot(&global.ty, global.array_element.as_ref())
                })
                .collect(),
            class_descriptors,
            predeclared_singletons: HashMap::new(),
            event_routes,
        }
    }

    fn build_runtime_class_descriptor(
        program: &OxProgram,
        class_index: usize,
        class: &oxvba_oxir::OxClass,
    ) -> &'static RuntimeClassDescriptor {
        let name: &'static str = leak_runtime_str(&class.name);
        let project_identity = RuntimeProjectClassIdentity {
            unit_name: leak_runtime_str(&program.unit_name),
            class_index,
        };
        let fields: &'static [RuntimeClassFieldDescriptor] = Box::leak(
            class
                .fields
                .iter()
                .map(runtime_class_field_descriptor)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let as_new_fields: &'static [RuntimeClassAsNewFieldDescriptor] = Box::leak(
            class
                .as_new_fields
                .iter()
                .map(runtime_as_new_field_descriptor)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let implements: &'static [&'static str] = Box::leak(
            class
                .implements
                .iter()
                .map(|interface| leak_runtime_str(interface))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let lifecycle = RuntimeClassLifecycleDescriptor {
            has_initialize: class.initialize.is_some(),
            has_terminate: class.terminate.is_some(),
        };
        let members: &'static [RuntimeMemberDescriptor] = Box::leak(
            class
                .methods
                .iter()
                .enumerate()
                .map(|(index, method)| {
                    let proc = program.funcs.get(method.proc.0);
                    let params: &'static [RuntimeParamDescriptor] = Box::leak(
                        proc.map(runtime_member_params)
                            .unwrap_or_default()
                            .into_boxed_slice(),
                    );
                    RuntimeMemberDescriptor {
                        name: leak_runtime_str(&method.name),
                        dispatch_id: method.dispid.unwrap_or_else(|| {
                            synthetic_dispatch_id(index, method.is_default_member)
                        }),
                        vtable_slot: method.vtable_slot,
                        invoke_kind: runtime_member_invoke_kind(method.kind),
                        arity: params.len(),
                        params,
                        return_type: proc.and_then(runtime_return_type),
                        is_default_member: method.is_default_member,
                        is_enumerator_member: method.is_enumerator_member,
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let dispatch = RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: "IDispatch",
            members,
            dual_dispatch: true,
        };
        let mut interface_descriptors = vec![RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR, dispatch];
        interface_descriptors.extend(class.implements.iter().filter_map(|interface| {
            runtime_project_interface_descriptor(program, class, interface)
        }));
        let interfaces: &'static [RuntimeInterfaceDescriptor] =
            Box::leak(interface_descriptors.into_boxed_slice());
        &*Box::leak(Box::new(RuntimeClassDescriptor {
            name,
            project_identity: Some(project_identity),
            predeclared: class.predeclared,
            lifecycle,
            fields,
            as_new_fields,
            implements,
            interfaces,
        }))
    }

    /// The index of the loaded program declaring unit `name` (its `unit_name`), for resolving a
    /// cross-project import/reference.
    fn program_index_by_unit(&self, unit: &str) -> Option<usize> {
        self.exec
            .programs
            .iter()
            .position(|lp| lp.program.unit_name.eq_ignore_ascii_case(unit))
    }

    /// Every cross-project import (a non-`VBA` `unit`) must name a loaded program, else the link
    /// is unresolved — reported naming the missing unit (never a silent mis-link). `VBA` imports
    /// are the synthetic built-in library, resolved separately in `call_extern`.
    fn validate_links(&self) -> Result<(), Vm3Error> {
        for lp in &self.exec.programs {
            for imp in &lp.program.imports {
                if imp.unit.eq_ignore_ascii_case("VBA") {
                    continue;
                }
                if self.program_index_by_unit(&imp.unit).is_none() {
                    return Err(Vm3Error::Malformed(format!(
                        "unresolved reference to unit '{}'",
                        imp.unit
                    )));
                }
            }
        }
        Ok(())
    }

    /// Run `program` to completion and return the finished VM (read the result snapshot
    /// with [`Vm3::slot`]). Mirrors vm2: the global initializer runs first, then `Main`
    /// in an entry frame that is never popped.
    pub fn run(program: &'h OxProgram, host: &'h dyn HostServices) -> Result<Self, Vm3Error> {
        let mut vm = Self::activate(program, host)?;
        vm.run_entry()?;
        Ok(vm)
    }

    /// Link `programs` into a runnable image and [`activate`](Self::activate) it (without running
    /// the entry — the host drives that). A single program is the common case: a whole VBA
    /// *project* is exactly ONE `OxProgram` with many classes, so it links trivially and this is
    /// what the single-project product paths (CLI run, single-class COM server) need.
    ///
    /// Multiple programs are cross-project references (project A referencing project B's
    /// procs/classes). The entry is the LAST program (mirrors vm2's `Vm::link`); cross-project
    /// imports are resolved by `unit_name` and must all be present, else the link fails naming
    /// the missing unit. Each program's module-global initializer runs in its own program
    /// context; the entry's `Main` is run later by `run_entry`.
    pub fn link(programs: &[&'h OxProgram], host: &'h dyn HostServices) -> Result<Self, Vm3Error> {
        if programs.is_empty() {
            return Err(Vm3Error::Malformed(
                "Vm3::link requires at least one program".into(),
            ));
        }
        let entry = programs.len() - 1;
        let mut exec = ExecState::new(host);
        exec.programs = programs.iter().map(|p| Self::build_loaded(p)).collect();
        let mut vm = Vm3 {
            cur: entry,
            exec,
            frames: Vec::new(),
            for_each: HashMap::new(),
            param_array_aliases: HashMap::new(),
            as_new_slots: HashMap::new(),
        };
        // Every cross-project reference must resolve before any code runs.
        vm.validate_links()?;
        // Run each program's module-global initializer in ITS OWN program context (cur = i), so
        // a global write lands in that program's globals (the entry's initializer runs last).
        for i in 0..vm.exec.programs.len() {
            if let Some(init) = vm.exec.programs[i].program.global_initializer {
                vm.cur = i;
                let frame = vm.new_frame(init);
                vm.frames.push(frame);
                let r = vm.run_loop(0);
                // The initializer writes module globals; its own frame is discarded.
                if let Some(frame) = vm.frames.pop() {
                    vm.clear_withevents_owners_in_frame_before_drop(&frame);
                    drop(frame);
                }
                vm.prune_param_array_aliases_from_depth(vm.frames.len());
                r?;
            }
        }
        vm.cur = entry;
        // Isolate this activation from any prior run on the shared thread-local termination
        // queue, before any entry/invoke runs — matching vm2's per-run reset.
        oxvba_runtime::reset_pending_terminations();
        Ok(vm)
    }

    /// Host session factory (W7): mint a project-class instance by NAME (running its
    /// `Class_Initialize`) and return it as a held object. The COM-server (`oxvba-comhost`)
    /// activation path calls this. A class the program doesn't declare is "can't create object"
    /// (429). The instance's state lives on the object box exactly as `New`-minted ones.
    pub fn create_project_instance(&mut self, class_name: &str) -> Result<Variant, Vm3Error> {
        let idx = self
            .cur_program()
            .classes
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(class_name))
            .ok_or_else(|| {
                Vm3Error::Fault(Fault::new(429, "ActiveX component can't create object"))
            })?;
        self.new_project_instance(idx)
    }

    /// Host session invoke (W7): call a member by NAME on a held instance with pre-marshaled
    /// by-value args (the COM-server dispatch path). Resolves the member proc by name + accessor
    /// kind (with vm2's get↔method fallback) and runs it via [`Self::run_proc_with_values`]; a
    /// built-in `Collection` receiver routes to the shared keyed dispatch. Missing member → 438.
    pub fn invoke_member_values(
        &mut self,
        object: ObjectRef,
        name: &str,
        kind_hint: Option<ProjectMemberKind>,
        args: Vec<Variant>,
    ) -> Result<Variant, Vm3Error> {
        if object.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return self.dispatch_collection_values(&object, name, args);
        }
        let class_idx = object.route_key() as usize;
        // Resolve + run in the object's OWN program (bundle_id), not the executing cur.
        let obj_bundle = object.bundle_id() as usize;
        let program = self
            .exec
            .programs
            .get(obj_bundle)
            .map(|lp| lp.program)
            .ok_or_else(|| {
                Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
            })?;
        let class = program.classes.get(class_idx).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
        })?;
        let member = class
            .methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name) && kind_hint.is_none_or(|k| m.kind == k))
            .or_else(|| {
                class
                    .methods
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case(name))
            });
        let proc = member.map(|m| m.proc).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(438, format!("Object doesn't support '{name}'")))
        })?;
        // Host args are already by-value (resolution-order is moot), but run in the object's
        // program (target_prog = obj_bundle).
        self.run_proc_with_values(
            obj_bundle,
            proc,
            Variant::from_object_ref(object),
            args,
            false,
        )
    }

    /// A built-in `Collection` member call with already-marshaled by-value args (the
    /// [`Self::invoke_member_values`] counterpart of [`Self::dispatch_collection_method`], whose
    /// args arrive as `OxCallArg`s).
    fn dispatch_collection_values(
        &mut self,
        object: &ObjectRef,
        name: &str,
        args: Vec<Variant>,
    ) -> Result<Variant, Vm3Error> {
        let native = Self::vba_collection_native_method(name).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(
                438,
                format!("Collection doesn't support '{name}'"),
            ))
        })?;
        let method = match native {
            oxvba_bundle::NativeMethodId::CollectionAdd => CollectionMethod::Add,
            oxvba_bundle::NativeMethodId::CollectionItem => CollectionMethod::Item,
            oxvba_bundle::NativeMethodId::CollectionCount => CollectionMethod::Count,
            oxvba_bundle::NativeMethodId::CollectionRemove => CollectionMethod::Remove,
            oxvba_bundle::NativeMethodId::CollectionNewEnum => {
                return self.collection_new_enum_object(object);
            }
        };
        object
            .with_native_collection(|d| dispatch_collection(method, d, &args))
            .ok_or_else(|| Vm3Error::Fault(Fault::new(424, "Object required")))?
            .map_err(Self::collection_fault)
            .map_err(Vm3Error::Fault)
    }

    /// Register a host event sink (W7): every project `RaiseEvent` is delivered to `sink` as
    /// `(source, event_id, args)` after the internal `WithEvents` fan-out. The COM server uses
    /// this to forward project events to its connection-point clients.
    pub fn set_project_event_sink<F>(&mut self, sink: F)
    where
        F: FnMut(ObjectRef, i32, Vec<Variant>) -> Result<(), String> + 'h,
    {
        self.exec.events.project_event_sink = Some(Box::new(sink));
    }

    /// Remove the host event sink (W7).
    pub fn clear_project_event_sink(&mut self) {
        self.exec.events.project_event_sink = None;
    }

    /// Build the VM for a SINGLE program and run its module-global initializer, but do NOT run
    /// the entry (`Main`) — the front half of [`Vm3::run`], split out so a long-lived host
    /// session can activate once and issue many member invokes. A whole VBA project is exactly
    /// one `OxProgram`; this is [`Vm3::link`] over `[program]` (which leaks the `&'static` class
    /// descriptors, builds the event routes, runs the initializer, and resets the drain queue
    /// exactly once).
    pub fn activate(program: &'h OxProgram, host: &'h dyn HostServices) -> Result<Self, Vm3Error> {
        Self::link(&[program], host)
    }

    /// Run the program entry (`Sub Main`) in an entry frame that is never popped — it stays as
    /// `frames[0]` for the result snapshot — then drain any parked `Class_Terminate`s. A no-op
    /// when the program has no entry. The back half of [`Vm3::run`], split out so a session can
    /// `activate()` without running `Main`.
    pub fn run_entry(&mut self) -> Result<(), Vm3Error> {
        if let Some(entry) = self.cur_program().entry {
            let frame = self.new_frame(entry);
            self.frames.push(frame);
            // The entry frame is never popped — it stays as `frames[0]` for the snapshot.
            let r = self.run_loop(0);
            // Run any `Class_Terminate`s parked while the run unwound — including objects an
            // uncaught fault released as it propagated out of called procs (vm2 drains on the
            // fault path; without this a Terminate would be lost on an error exit). On a clean
            // finish this is a no-op (statement boundaries already drained; the entry frame's
            // own locals stay live, un-terminated, exactly as vm2 leaves them).
            self.maybe_drain();
            r?;
        }
        Ok(())
    }

    /// The result snapshot slot `i`: module globals occupy `[0, globals.len())`; higher
    /// indices are the entry (`Main`) frame's locals (the same layout vm2 exposes).
    pub fn slot(&self, i: usize) -> Option<Variant> {
        let global_count = self.exec.programs[self.cur].globals.len();
        if i < global_count {
            self.exec.programs[self.cur].globals.get(i).cloned()
        } else {
            let rel = i - global_count;
            self.frames.first()?.locals.get(rel).cloned()
        }
    }

    /// The final `Err` state (number / description / source) for the error axis.
    pub fn err_number(&self) -> i32 {
        self.exec.err_engine.err.number
    }
    pub fn err_description(&self) -> &str {
        &self.exec.err_engine.err.description
    }
    pub fn err_source(&self) -> &str {
        &self.exec.err_engine.err.source
    }
    pub fn erl(&self) -> i32 {
        self.exec.err_engine.erl_line
    }
    /// `Err.LastDllError` — the OS last-error captured after the most recent `Declare Lib`
    /// call (M3-7); `0` until a Declare runs.
    pub fn last_dll_error(&self) -> i32 {
        self.exec.err_engine.last_dll_error
    }

    fn new_frame(&self, func: FuncId) -> Frame {
        self.new_frame_in(self.cur, func)
    }

    /// Build a fresh frame for `func` in program `prog` (locals sized from that program's
    /// function); the frame carries `prog`, so `run_loop` executes it against that program's
    /// globals/code regardless of the resolving `cur`. Used to run a cross-project callee (an
    /// object method, or a referenced project's proc) whose ARGUMENTS are resolved in the
    /// caller's program (`cur`) BEFORE this frame runs — matching vm2's resolve-then-switch.
    fn new_frame_in(&self, prog: usize, func: FuncId) -> Frame {
        let f = &self.exec.programs[prog].program.funcs[func.0];
        Frame {
            prog,
            func,
            block: f.entry,
            ip: 0,
            locals: f
                .locals
                .iter()
                .map(|local| Self::initial_value_for_slot(&local.ty, local.array_element.as_ref()))
                .collect(),
            temps: HashMap::new(),
            aliases: HashMap::new(),
            dst: None,
            return_local: f.return_local,
            current_line: 0,
            saved_error_mode: ErrorMode::None,
            saved_active_error: None,
            gosub_stack: Vec::new(),
        }
    }

    /// The block-threaded dispatch loop: run the top frame until the frame at depth
    /// `base` returns. A `CallProc` pushes a callee and the loop simply continues with
    /// it; `Return` pops back to the caller. There is no native recursion, so deep VBA
    /// recursion is bounded by the frame ceiling (error 28), never a host stack
    /// overflow — and the model mirrors vm2's iterative dispatch.
    fn run_loop(&mut self, base: usize) -> Result<(), Vm3Error> {
        while self.frames.len() > base {
            let top = self.frames.len() - 1;
            // Re-target `cur` to the executing frame's program each iteration: a cross-program
            // call pushed a frame with a different `prog`, and a return pops back to the caller's,
            // so globals (`programs[cur]`) and instruction fetch follow control flow with no
            // explicit save/restore. `program` is a `'h` copy, independent of the exec borrows.
            self.cur = self.frames[top].prog;
            let program = self.cur_program();
            let (func, block, ip) = {
                let fr = &self.frames[top];
                (fr.func, fr.block, fr.ip)
            };
            let blk: &OxBlock = program.funcs[func.0]
                .blocks
                .get(block.0)
                .ok_or_else(|| Vm3Error::Malformed(format!("block {} out of range", block.0)))?;

            if ip < blk.instrs.len() {
                // Advance past this instruction first, so a `CallProc` it performs
                // resumes the caller at the *next* instruction when the callee returns.
                self.frames[top].ip = ip + 1;
                if let Err(e) = self.exec(&blk.instrs[ip]) {
                    match e {
                        Vm3Error::Fault(fault) => self.route_fault(fault)?,
                        other => return Err(other),
                    }
                }
                continue;
            }

            match &blk.terminator {
                OxTerminator::Jump(b) => self.goto(top, *b),
                OxTerminator::Branch {
                    cond,
                    then_blk,
                    else_blk,
                } => {
                    // `cond` is a pre-computed Boolean: the elaboration emits a `Truthy`
                    // before *every* conditional Branch (a statically-Bool operand is not
                    // a guaranteed runtime Boolean — an unassigned `Dim b As Boolean` is
                    // Empty, `Not b` of an Empty Bool is a Long, a Variant compare is
                    // Null), so the terminator is a pure transfer and any truthiness fault
                    // already routed through the pad at the `Truthy`.
                    let v = self.operand(cond)?;
                    let taken = v.as_bool().ok_or_else(|| {
                        Vm3Error::Malformed("Branch condition is not a pre-computed Boolean".into())
                    })?;
                    self.goto(top, if taken { *then_blk } else { *else_blk });
                }
                OxTerminator::Return => {
                    if self.frames.len() == base + 1 {
                        // The base frame returned — leave it on the stack (the entry frame
                        // backs the result snapshot) and end this run.
                        break;
                    }
                    self.do_return()?;
                }
                // VBA `End`: stop the *entire* program immediately at any call depth — no
                // return to the caller, no finalization. Unwind to the base frame (which
                // stays on the stack to back the snapshot) and end the run.
                OxTerminator::Halt => {
                    self.truncate_frames_with_withevents_cleanup(base + 1);
                    self.prune_param_array_aliases_from_depth(self.frames.len());
                    // Re-derive `cur` to the surviving (base/entry) frame's program: `End` may
                    // fire inside a cross-program callee, and the post-run snapshot reader
                    // (`slot`) reads `programs[cur]`'s globals — which must be the entry
                    // program's, to stay consistent with the entry frame's locals it also reads.
                    self.cur = self.frames[base].prog;
                    break;
                }
                // The landing pad: dispatch the in-flight fault on the activation's
                // handler policy (MS-VBAL §5.4.4; doc rules R4/R9).
                OxTerminator::FaultDispatch {
                    resume,
                    resume_next,
                } => match self
                    .exec
                    .err_engine
                    .dispatch_fault(*resume, *resume_next, self.frames[top].current_line)
                    .map_err(|msg| Vm3Error::Malformed(msg.into()))?
                {
                    FaultAction::Propagate(fault) => self.propagate_fault(fault, base)?,
                    FaultAction::ResumeNext(block) | FaultAction::Handle(block) => {
                        self.goto(top, block);
                    }
                },
                // The three `Resume` forms (R6/R7/R8): with no active error, raise error 20
                // ("Resume without error"); otherwise reset `Err`, clear the latch, RE-ARM
                // the handler that caught the error (so a fault after the resume is caught
                // again), and transfer.
                OxTerminator::Resume => match self.exec.err_engine.resume(ResumeTarget::Same) {
                    Ok(block) => self.goto(top, block),
                    Err(fault) => self.route_fault(fault)?,
                },
                OxTerminator::ResumeNext => match self.exec.err_engine.resume(ResumeTarget::Next) {
                    Ok(block) => self.goto(top, block),
                    Err(fault) => self.route_fault(fault)?,
                },
                OxTerminator::ResumeLabel(b) => {
                    match self.exec.err_engine.resume(ResumeTarget::Label(*b)) {
                        Ok(block) => self.goto(top, block),
                        Err(fault) => self.route_fault(fault)?,
                    }
                }
                // `Err.Raise Number[, Source][, Description]` / `Error n`: build the `Err`
                // state from the number plus any explicit Source/Description, then route
                // through the statement pad so an active `On Error` can catch it (R11).
                //
                // MS-VBAL §9071 (oracle-confirmed): an omitted argument inherits the
                // current `Err` field when the Err fields are inheritable. Actual errors
                // make them inheritable; `Err.Clear` clears that state. Direct
                // `Err.Number = ...` writes do not make the text/help fields inheritable,
                // but writing Description, Source, HelpFile, or HelpContext does, even
                // when `Err.Number` is 0. System faults never inherit — that path is
                // `from_arith`/`route_fault`, which always builds fresh fields.
                OxTerminator::Raise {
                    number,
                    source,
                    description,
                    help_file,
                    help_context,
                    inherit,
                } => {
                    let num_v = self.operand(number)?;
                    match arith::coerce_numeric(&num_v, oxvba_bundle::NumericCoerceTarget::Long) {
                        Ok(code_v) => {
                            let code = code_v.as_i32().unwrap_or(0);
                            // §9071 inherit applies only to `Err.Raise` (inherit=true);
                            // the legacy `Error <n>` statement (inherit=false) never
                            // inherits — oracle-confirmed.
                            let inherit = *inherit && self.exec.err_engine.err.inherit_fields;
                            let message = match description {
                                Some(op) => self.operand_string(op)?,
                                None if inherit => self.exec.err_engine.err.description.clone(),
                                None => default_error_message(code),
                            };
                            let source = match source {
                                Some(op) => Some(self.operand_string(op)?),
                                None if inherit => Some(self.exec.err_engine.err.source.clone()),
                                None => None, // -> project name in `raise`
                            };
                            let help_file = match help_file {
                                Some(op) => Some(self.operand_string(op)?),
                                None if inherit => Some(self.exec.err_engine.err.help_file.clone()),
                                None => None,
                            };
                            let help_context = match help_context {
                                Some(op) => {
                                    let v = self.operand(op)?;
                                    let coerced = arith::coerce_numeric(
                                        &v,
                                        oxvba_bundle::NumericCoerceTarget::Long,
                                    )
                                    .map_err(Fault::from_arith)
                                    .map_err(Vm3Error::Fault)?;
                                    Some(coerced.as_i32().unwrap_or(0))
                                }
                                None if inherit => Some(self.exec.err_engine.err.help_context),
                                None => None,
                            };
                            self.route_fault(Fault {
                                code,
                                message,
                                source,
                                help_file,
                                help_context,
                            })?;
                        }
                        // A non-numeric raise code is itself a coercion fault (e.g. 13).
                        Err(e) => self.route_fault(Fault::from_arith(e))?,
                    }
                }
                // GoSub / Return: a per-activation LIFO resumption list (R12).
                OxTerminator::GoSub { target, ret } => {
                    self.frames[top].gosub_stack.push(*ret);
                    self.goto(top, *target);
                }
                OxTerminator::GoSubReturn => match self.frames[top].gosub_stack.pop() {
                    Some(ret) => self.goto(top, ret),
                    None => self.raise_runtime_error(3)?, // Return without GoSub
                },
                OxTerminator::Unreachable => {
                    return Err(Vm3Error::Malformed(
                        "reached an Unreachable terminator".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Jump frame `top` to the start of `block`.
    fn goto(&mut self, top: usize, block: BlockId) {
        let fr = &mut self.frames[top];
        fr.block = block;
        fr.ip = 0;
    }

    /// Route an in-flight fault to the current frame's block fault pad (intra-frame);
    /// the pad's `FaultDispatch` then consults the error mode.
    fn route_fault(&mut self, fault: Fault) -> Result<(), Vm3Error> {
        let top = self.frames.len() - 1;
        // Re-target `cur` to the faulting frame's program: `propagate_fault` may have popped to a
        // caller in a DIFFERENT program before re-routing here, so the pad lookup + `Err.Source`
        // (raise) must use that frame's program (mirrors vm2's route_fault `cur` restore).
        self.cur = self.frames[top].prog;
        let (func, block) = (self.frames[top].func, self.frames[top].block);
        let pad = self.cur_program().funcs[func.0].blocks[block.0]
            .fault_target
            .ok_or_else(|| {
                Vm3Error::Malformed("fallible instruction in a block with no fault_target".into())
            })?;
        self.raise(fault);
        self.goto(top, pad);
        Ok(())
    }

    /// Propagate an unhandled fault out of the current frame: pop it (restoring the
    /// caller's error mode) and re-route at the caller's call site, or — at the base
    /// frame — surface it as the run's result.
    fn propagate_fault(&mut self, fault: Fault, base: usize) -> Result<(), Vm3Error> {
        if self.frames.len() <= base + 1 {
            return Err(Vm3Error::Fault(fault));
        }
        let callee = self.frames.pop().expect("frame to unwind");
        self.restore_err_from_frame(&callee);
        self.clear_withevents_owners_in_frame_before_drop(&callee);
        drop(callee);
        self.prune_param_array_aliases_from_depth(self.frames.len());
        // The caller's `CallProc` faulted: route to *its* block's fault pad.
        self.route_fault(fault)
    }

    /// Pop a returning callee: restore the caller's error mode and copy out the
    /// function's return value (true aliasing already propagated ByRef writes live).
    fn do_return(&mut self) -> Result<(), Vm3Error> {
        let callee = self.frames.pop().expect("returning frame");
        self.restore_err_from_frame(&callee);
        if let (Some(loc), Some(rl)) = (callee.dst, callee.return_local)
            && let Some(v) = callee.locals.get(rl.0).cloned()
        {
            self.write_loc(loc, v)?;
        }
        // Proc epilogue: drop the callee frame (releasing the objects its locals held) and then
        // run any parked `Class_Terminate`s — vm2's epilogue drain timing.
        self.clear_withevents_owners_in_frame_before_drop(&callee);
        drop(callee);
        self.prune_param_array_aliases_from_depth(self.frames.len());
        self.maybe_drain();
        Ok(())
    }

    fn restore_err_from_frame(&mut self, frame: &Frame) {
        self.exec.err_engine.restore(SavedErrState {
            error_mode: frame.saved_error_mode,
            active_error: frame.saved_active_error,
        });
    }

    /// Populate `Err` from a raised fault and stash it for the landing pad. Number and
    /// Description come from the fault; `Err.Source` is the fault's explicit source if it
    /// carries one (`Err.Raise … Source`), else the **project name** — the VBA default
    /// for any error generated within the project, matching the Excel/VBA 7.1 oracle
    /// (`Err.Source = "VBAProject"`; see `docs/VBA_ERROR_MODEL_ORACLE_FINDINGS.md`).
    fn raise(&mut self, fault: Fault) {
        self.exec
            .err_engine
            .raise(fault, self.cur_program().unit_name.clone());
    }

    /// Evaluate an operand and coerce it to its VBA string form — used for an explicit
    /// `Err.Raise` Source/Description argument.
    fn operand_string(&mut self, op: &OxOperand) -> Result<String, Vm3Error> {
        let v = self.operand(op)?;
        Ok(oxvba_runtime::variant_to_vba_string(&v)
            .map(|b| b.as_str())
            .unwrap_or_default())
    }

    /// Raise a vm3-internal run-time error (a code with its default message) by routing
    /// it through the current statement's fault pad — so an active `On Error` can catch
    /// it (used for error 20 "Resume without error" and error 3 "Return without GoSub").
    fn raise_runtime_error(&mut self, code: i32) -> Result<(), Vm3Error> {
        self.route_fault(Fault {
            code,
            message: default_error_message(code),
            source: None,
            help_file: None,
            help_context: None,
        })
    }

    /// Execute one straight-line instruction against the top frame.
    fn exec(&mut self, inst: &OxInst) -> Result<(), Vm3Error> {
        match inst {
            OxInst::AsNew { place, binding } => {
                let loc = self.resolve(place);
                self.as_new_slots.insert(loc, binding.clone());
            }
            OxInst::Assign { dst, value } => {
                let v = self.operand(value)?;
                self.store(dst, v)?;
            }
            OxInst::Box { dst, src, .. } => {
                let v = self.operand(src)?;
                self.store(dst, v)?;
            }
            OxInst::Unbox {
                dst,
                src,
                to,
                checked,
            } => {
                let v = self.operand(src)?;
                if *checked && !variant_matches_ox_ty(&v, to) {
                    return Err(Vm3Error::Fault(Fault::new(13, "type mismatch")));
                }
                self.store(dst, v)?;
            }
            // `dst := (current != original)` — VBA-`Variant`-equality change detection that
            // guards a compound `ByRef` copy-out (mirrors vm2's `Op::VariantChanged`, which
            // compares with `Variant`'s `PartialEq`).
            OxInst::VariantChanged {
                dst,
                current,
                original,
            } => {
                let changed = variant_changed(&self.operand(current)?, &self.operand(original)?);
                self.store(dst, Variant::from_bool(changed))?;
            }
            OxInst::Arith {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                let out = match op {
                    ArithOp::Add => arith::add(&l, &r, *mode),
                    ArithOp::Sub => arith::sub(&l, &r, *mode),
                    ArithOp::Mul => arith::mul(&l, &r, *mode),
                    ArithOp::IntDiv => arith::int_div(&l, &r, *mode),
                    ArithOp::Mod => arith::modulo(&l, &r, *mode),
                };
                self.store_arith(dst, out)?;
            }
            OxInst::Div { dst, lhs, rhs } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::div(&l, &r))?;
            }
            OxInst::Pow { dst, lhs, rhs } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::pow(&l, &r))?;
            }
            OxInst::Neg { dst, src, mode } => {
                let v = self.operand(src)?;
                self.store_arith(dst, arith::neg(&v, *mode))?;
            }
            OxInst::Concat { dst, lhs, rhs } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::concat(&l, &r))?;
            }
            OxInst::Compare {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                self.store_arith(dst, arith::compare(&l, &r, *mode, cmp_op(*op)))?;
            }
            OxInst::Logical { dst, op, lhs, rhs } => {
                let l = self.operand(lhs)?;
                let r = self.operand(rhs)?;
                let out = match op {
                    LogicalOp::And => arith::and(&l, &r),
                    LogicalOp::Or => arith::or(&l, &r),
                    LogicalOp::Xor => arith::xor(&l, &r),
                    LogicalOp::Eqv => arith::eqv(&l, &r),
                    LogicalOp::Imp => arith::imp(&l, &r),
                };
                self.store_arith(dst, out)?;
            }
            OxInst::Not { dst, src } => {
                let v = self.operand(src)?;
                self.store_arith(dst, arith::not(&v))?;
            }
            // Reduce a condition to a Boolean by VBA truthiness (the elaboration emits
            // this before a conditional `Branch`); the `is_truthy` rule + error code are
            // exactly what vm2's `JumpIfZero` uses.
            OxInst::Truthy { dst, src } => {
                let v = self.operand(src)?;
                let out = arith::is_truthy(&v).map(Variant::from_bool);
                self.store_arith(dst, out)?;
            }
            OxInst::Coerce { dst, src, target } => {
                let v = self.operand(src)?;
                let out = match target {
                    OxCoerceTarget::Numeric(t) => arith::coerce_numeric(&v, *t),
                    OxCoerceTarget::Str => arith::coerce_string(&v),
                    OxCoerceTarget::FixedStr(n) => arith::coerce_fixed_string(&v, *n as usize),
                    // A widen-to-Variant carries no value change.
                    OxCoerceTarget::ImplicitVariant => Ok(v),
                };
                self.store_arith(dst, out)?;
            }
            // A compiled VBA procedure call (intra-unit). The `AddressOf`-reference
            // `CallProcRef` is M3-7.
            OxInst::CallProc { dst, proc, args } => {
                self.call_proc_in(self.cur, *dst, *proc, args)?
            }
            // A cross-bundle call. vm3 links only the synthetic `VBA` library bundle today,
            // so this resolves to a native library function (`Strings.Left`, `Math.Abs`, …)
            // run through the same `invoke_native_lib` bridge as `CallNative { Builtin }`. A
            // reference to another VBA *project* needs a multi-`OxProgram` linker (deferred —
            // surfaced as an explicit `Unimplemented`, never a silent skip).
            OxInst::CallExtern { dst, import, args } => self.call_extern(*dst, *import, args)?,
            // A base-library built-in is classified into the disjoint context-free or
            // contextual `oxvba-lib` dispatcher; `Declare Lib` marshalling is M3.
            OxInst::CallNative { dst, callee, args } => match callee {
                OxNativeCallee::Builtin(id) => {
                    let argv = self.native_args(args)?;
                    let result = self.invoke_native_lib(*id, &argv)?;
                    if let Some(dst) = dst {
                        self.store(dst, result)?;
                    }
                }
                // A `Declare Lib` external call marshals through the host's dynamic-link HAL —
                // the identical `invoke_descriptor_variants` contract vm2 drives — capturing
                // `Err.LastDllError` and applying ByRef + pointer-helper write-backs (M3-7).
                OxNativeCallee::Declare {
                    descriptor_id,
                    ptr_writebacks,
                } => {
                    let result = self.declare_call(*descriptor_id, args, ptr_writebacks)?;
                    if let Some(dst) = dst {
                        self.store(dst, result)?;
                    }
                }
            },
            // `AddressOf <proc>` materializes a procedure reference value (a `ProcRef`
            // Variant carrying the proc index); it is later invoked through `CallProcRef`
            // or marshaled to a native callback slot by a `Declare` (M3-7).
            OxInst::LoadProcRef { dst, proc } => {
                self.store(dst, Variant::from_proc_ref(proc.0))?;
            }
            // Call through a runtime-resolved procedure reference (the `AddressOf` value):
            // resolve the index, validate it, then dispatch through the standard call
            // machinery (ByRef aliasing, frame push, return copy-out — identical to `CallProc`).
            OxInst::CallProcRef { dst, target, args } => {
                let proc = self
                    .operand(target)?
                    .as_proc_ref()
                    .filter(|&p| p < self.cur_program().funcs.len())
                    .ok_or_else(|| {
                        Vm3Error::Fault(Fault::new(490, "invalid procedure reference"))
                    })?;
                let dst = dst.as_ref().copied();
                self.call_proc_in(self.cur, dst, FuncId(proc), args)?;
            }
            // `On Error` sets the activation's handler policy and — per MS-VBAL §5.4.4.1
            // (doc rule R5) — unconditionally resets the `Err` object. (The active-error
            // latch is cleared only by `Resume`/`Exit`, not here.)
            OxInst::SetErrorHandler(handler) => {
                self.exec.err_engine.set_error_handler(handler);
            }
            // Read an `Err` property.
            OxInst::ErrFieldGet { dst, field } => {
                let v = match field {
                    ErrField::Number => Variant::from_i32(self.exec.err_engine.err.number),
                    ErrField::Description => {
                        Variant::from_string(self.exec.err_engine.err.description.clone())
                    }
                    ErrField::Source => {
                        Variant::from_string(self.exec.err_engine.err.source.clone())
                    }
                    ErrField::HelpFile => {
                        Variant::from_string(self.exec.err_engine.err.help_file.clone())
                    }
                    ErrField::HelpContext => {
                        Variant::from_i32(self.exec.err_engine.err.help_context)
                    }
                    ErrField::LastDllError => {
                        Variant::from_i32(self.exec.err_engine.last_dll_error)
                    }
                };
                self.store(dst, v)?;
            }
            OxInst::ErlGet { dst } => {
                self.store(dst, Variant::from_i32(self.exec.err_engine.erl_line))?;
            }
            // Write an `Err` property. `Err.LastDllError` is read-only; accepted user
            // assignments to it should have been rejected by the binder.
            OxInst::ErrFieldSet { field, src } => {
                let value = self.operand(src)?;
                match field {
                    ErrField::Number => {
                        let coerced =
                            arith::coerce_numeric(&value, oxvba_bundle::NumericCoerceTarget::Long)
                                .map_err(Fault::from_arith)
                                .map_err(Vm3Error::Fault)?;
                        self.exec.err_engine.err.number = coerced.as_i32().unwrap_or(0);
                    }
                    ErrField::Description => {
                        let coerced = arith::coerce_string(&value)
                            .map_err(Fault::from_arith)
                            .map_err(Vm3Error::Fault)?;
                        self.exec.err_engine.err.description = arith::as_string(&coerced);
                        self.exec.err_engine.err.inherit_fields = true;
                    }
                    ErrField::Source => {
                        let coerced = arith::coerce_string(&value)
                            .map_err(Fault::from_arith)
                            .map_err(Vm3Error::Fault)?;
                        self.exec.err_engine.err.source = arith::as_string(&coerced);
                        self.exec.err_engine.err.inherit_fields = true;
                    }
                    ErrField::HelpFile => {
                        let coerced = arith::coerce_string(&value)
                            .map_err(Fault::from_arith)
                            .map_err(Vm3Error::Fault)?;
                        self.exec.err_engine.err.help_file = arith::as_string(&coerced);
                        self.exec.err_engine.err.inherit_fields = true;
                    }
                    ErrField::HelpContext => {
                        let coerced =
                            arith::coerce_numeric(&value, oxvba_bundle::NumericCoerceTarget::Long)
                                .map_err(Fault::from_arith)
                                .map_err(Vm3Error::Fault)?;
                        self.exec.err_engine.err.help_context = coerced.as_i32().unwrap_or(0);
                        self.exec.err_engine.err.inherit_fields = true;
                    }
                    ErrField::LastDllError => {
                        return Err(Vm3Error::Malformed("Err.LastDllError is read-only".into()));
                    }
                }
            }
            // `Err.Clear` → reset the `Err` object.
            OxInst::ClearErr => self.exec.err_engine.clear_err(),

            // ── Pointer helpers (M3-7) ───────────────────────────────────────────────
            // `StrPtr`/`VarPtr` pin a cloned cell in the process-global pointer registry and
            // yield its pointer-width address (an `i64`); the `Declare` call that consumes the
            // pointer frees the pin afterwards (see `declare_call`). `ObjPtr` returns a live
            // IUnknown address (no registry cell). Identical to vm2's `Ptr*` ops.
            OxInst::Ptr { dst, kind, src } => {
                let v = self.operand(src)?;
                let pointer = match kind {
                    PtrKind::Str => pointer_helpers::register_utf16_string(&arith::as_string(&v)),
                    PtrKind::Var => pointer_helpers::register_variant_pointer(&v),
                    PtrKind::VarString => pointer_helpers::register_string_variant_pointer(&v),
                    PtrKind::VarVariant => {
                        pointer_helpers::register_variant_var_variant_pointer(&v)
                    }
                    PtrKind::Obj => pointer_helpers::register_object_variant_pointer(&v),
                }
                .map_err(|e| Vm3Error::Fault(Fault::from_string(e)))?;
                self.store(dst, Variant::from_i64(pointer))?;
            }
            // A statement boundary drives finalization timing: run any parked
            // `Class_Terminate`s released by the previous statement (the error model takes its
            // `Resume` seeds from `FaultDispatch`, not from here).
            OxInst::StmtBoundary {
                clear_temps_from, ..
            } => {
                // VBA statement-granular timing: run parked `Class_Terminate`s, then dispatch any
                // inbound host (COM) events (mirrors vm2's statement-boundary drain + pump).
                self.clear_statement_temps(*clear_temps_from);
                self.maybe_drain();
                self.pump_com_events();
            }
            OxInst::SetLineNumber { line } => {
                if let Some(frame) = self.frames.last_mut() {
                    frame.current_line = *line;
                }
            }

            // `Let`/`Set` legality check (M3-4).
            OxInst::ValidateAssignment {
                src,
                intent,
                target_kind,
                target_name,
                target_type_name,
            } => {
                self.validate_assignment(src, *intent, *target_kind, target_name, target_type_name)?
            }

            // ── Arrays / For Each (M3-2) ─────────────────────────────────────────────
            OxInst::ArrayLiteral {
                dst,
                values,
                aliases,
                lower_bound,
            } => {
                let elems = values
                    .iter()
                    .map(|v| self.operand(v))
                    .collect::<Result<Vec<_>, _>>()?;
                let alias_locs = aliases
                    .iter()
                    .map(|alias| alias.as_ref().map(|place| self.resolve(place)))
                    .collect::<Vec<_>>();
                // `Array()` is based at the module's `Option Base` (0 or 1); a
                // `ParamArray` always at 0. A non-zero base needs explicit bounds.
                let array = if *lower_bound == 0 {
                    SafeArray::from_variants(elems)
                } else {
                    let bounds = vec![SafeArrayBound {
                        count: elems.len() as u32,
                        lower: *lower_bound,
                    }];
                    SafeArray::from_variants_nd(bounds, elems)
                };
                self.store(dst, Variant::from_safearray(array))?;
                if alias_locs.iter().any(Option::is_some) {
                    let dst_loc = self.resolve(dst);
                    self.param_array_aliases.insert(dst_loc, alias_locs);
                }
            }
            OxInst::ArrayAppend { dst, array, item } => {
                let mut elems = match self.operand(array)?.as_safearray() {
                    Some(arr) => arr.variant_elements().unwrap_or_default(),
                    None => Vec::new(),
                };
                elems.push(self.operand(item)?);
                self.store(
                    dst,
                    Variant::from_safearray(SafeArray::from_variants(elems)),
                )?;
            }
            OxInst::ArrayRedim {
                dst,
                upper_bounds,
                lower_bounds,
                element,
                preserve,
                fixed,
            } => self.array_redim(dst, upper_bounds, lower_bounds, element, *preserve, *fixed)?,
            OxInst::ArrayGet {
                dst,
                array,
                indices,
            } => {
                // Fast O(1) path: a place-resident array reads one element directly
                // through the SAFEARRAY descriptor (no whole-array clone) → O(N) loops.
                if let Some(value) = self.array_get_fast(array, indices)? {
                    self.store(dst, value)?;
                    return Ok(());
                }
                // `x(i…)` where `x` is a bare `Variant`/`As Object` resolves at run time.
                let recv = self.operand(array)?;
                self.index_value_into(recv, indices, dst)?;
            }
            OxInst::ArraySet {
                array,
                indices,
                value,
            } => {
                let v = self.operand(value)?;
                // Fast O(1) path: a place-resident array mutates one element in place
                // through the SAFEARRAY descriptor (no whole-array clone-and-write-back).
                // The element is written through the resolved (alias-resolved) location,
                // so a ByRef-aliased array sees the change.
                if self.array_set_fast(array, indices, &v)? {
                    return Ok(());
                }
                // General path: a non-place receiver — materialize, set the element, and
                // write the mutated array back to its place.
                let recv = self.read(array)?;
                let arr_v = self.set_index_in_value(recv, indices, &v)?;
                self.store(array, arr_v)?;
            }
            OxInst::FieldArrayGet {
                dst,
                object,
                field,
                indices,
            } => {
                let recv = self.operand(object)?;
                let instance = variant_to_object(&recv)?;
                // Fast O(1): the field holds an array → read one element through the
                // descriptor IN PLACE (no whole-array clone per access). `with_project_field`
                // borrows the field Variant; a non-array field yields `None` → fall back.
                let fast = instance
                    .with_project_field(*field, |stored| {
                        let (arr, (bounds, len)) =
                            stored.and_then(|a| a.safearray_bounds_len().map(|bl| (a, bl)))?;
                        Some((|| -> Result<Variant, Vm3Error> {
                            let flat = self.flat_index(indices, &bounds)?;
                            if flat >= len {
                                return Err(Vm3Error::Fault(Fault::new(
                                    9,
                                    "subscript out of range",
                                )));
                            }
                            if let Some(result) = arr.safearray_i32_element(flat) {
                                match result {
                                    Ok(Some(value)) => return Ok(Variant::from_i32(value)),
                                    Ok(None) => {}
                                    Err(err) => {
                                        return Err(Vm3Error::Fault(Fault::new(13, err)));
                                    }
                                }
                            }
                            arr.safearray_element(flat)
                                .expect("safearray_bounds_len proved this is an array")
                                .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))
                        })())
                    })
                    .flatten();
                match fast {
                    Some(result) => {
                        let value = result?;
                        self.store(dst, value)?;
                    }
                    None => {
                        // Field is absent or not an array (e.g. an object whose default
                        // member is indexed): materialize (cheap for an object ref) and
                        // index generically.
                        let field_val = self.read_project_field_as_new(&instance, *field)?;
                        self.index_value_into(field_val, indices, dst)?;
                    }
                }
            }
            OxInst::FieldArraySet {
                object,
                field,
                indices,
                value,
            } => {
                let v = self.operand(value)?;
                let recv = self.operand(object)?;
                let instance = variant_to_object(&recv)?;
                // Fast O(1): the field holds an array → mutate one element IN PLACE.
                let fast = instance
                    .with_project_field_mut(*field, |arr| {
                        let (bounds, len) = arr.safearray_bounds_len()?;
                        Some((|| -> Result<(), Vm3Error> {
                            let flat = self.flat_index(indices, &bounds)?;
                            if flat >= len {
                                return Err(Vm3Error::Fault(Fault::new(
                                    9,
                                    "subscript out of range",
                                )));
                            }
                            if let Some(value_i32) = v.as_i32()
                                && let Some(result) = arr.set_safearray_i32_element(flat, value_i32)
                            {
                                match result {
                                    Ok(true) => return Ok(()),
                                    Ok(false) => {}
                                    Err(err) => {
                                        return Err(Vm3Error::Fault(Fault::new(13, err)));
                                    }
                                }
                            }
                            arr.set_safearray_element(flat, &v)
                                .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))
                        })())
                    })
                    .flatten();
                match fast {
                    Some(result) => result?,
                    None => {
                        // Not an array field: materialize, set the element, write back.
                        let field_val = self.read_project_field_as_new(&instance, *field)?;
                        let updated = self.set_index_in_value(field_val, indices, &v)?;
                        instance.project_field_set(*field, updated);
                    }
                }
            }
            OxInst::ArrayErase { array, element } => {
                // VBA `Erase` is reset-vs-deallocate decided by the array's *own* storage
                // class, carried on the runtime SAFEARRAY's `FADF_FIXEDSIZE` bit (set at
                // allocation, travelling with copies) — so the dispatch is purely runtime,
                // exactly like real VBA:
                //   • fixed-size (`Dim a(1 To 3)`): reinitialize every element to its type
                //     default and keep the array allocated (reads after `Erase` succeed);
                //   • dynamic (`Dim a()` + `ReDim`): free the storage (becomes
                //     uninitialized; `UBound` raises until re-`ReDim`'d).
                let cur = self.read(array)?;
                let was_array = cur.vtype() == VarType::ArrayVariant;
                let erased_element_vartype = cur
                    .array_element_vartype()
                    .unwrap_or_else(|| safearray_vartype_for_element(&ArrayElementType::Variant));
                let reset = cur.as_safearray().filter(|a| a.is_fixed_size()).map(|arr| {
                    // Rebuild a fresh array of the SAME bounds + element type + fixed flag,
                    // default-initialized — i.e. a `ReDim`-to-current-bounds, which already
                    // default-inits correctly for every element type (scalars, String → "",
                    // Variant → Empty, UDT → recursively zeroed record).
                    //
                    // Pick the reset element type. The bind-site `element` is the authoritative
                    // DECLARED type for a directly-typed array — including a UDT fixed-array
                    // field, whose materialized value is a VT_VARIANT SAFEARRAY that has lost the
                    // declared element type. A bind-site `Variant` is the ambiguous case: either a
                    // genuine `Variant` array, or a typed array materialized into a `Variant` slot
                    // (which keeps its real element vartype on the value but erases the bind-site
                    // type). Only then is the array's OWN runtime vartype the truth.
                    let bounds = arr.bounds().unwrap_or_default();
                    let count = arr.len();
                    let et = match element {
                        ArrayElementType::Variant => {
                            array_element_type_for_vartype(arr.element_vartype())
                        }
                        other => other.clone(),
                    };
                    if Self::element_supports_zeroed_redim(&et) {
                        return SafeArray::from_zeroed_typed_scalars_nd(
                            bounds,
                            safearray_vartype_for_element(&et),
                        )
                        .map(|array| array.with_fixed_size(true));
                    }
                    let elems: Vec<Variant> = (0..count)
                        .map(|_| default_array_element(&et))
                        .collect::<Result<Vec<_>, String>>()?;
                    redim_safearray_from_elements(bounds, &et, elems, true)
                });
                match reset {
                    Some(built) => {
                        let array_value = built.map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                        self.store(array, Variant::from_safearray(array_value))?;
                    }
                    None if was_array => {
                        self.store(array, Variant::unallocated_array(erased_element_vartype))?
                    }
                    None => self.store(array, Variant::empty())?,
                }
            }
            OxInst::Bound {
                dst,
                which,
                array,
                dimension,
            } => {
                let arr = self.array_of(array)?;
                let bounds = arr
                    .bounds()
                    .ok_or_else(|| Vm3Error::Fault(Fault::new(9, "array has no bounds")))?;
                let bound = &bounds[self.array_bound_index(dimension.as_ref(), &bounds)?];
                let value = match which {
                    BoundWhich::Lower => bound.lower,
                    BoundWhich::Upper => bound.lower + bound.count as i32 - 1,
                };
                self.store(dst, Variant::from_i32(value))?;
            }
            OxInst::ForEachInit { iter, source } => {
                let src = self.operand(source)?;
                let elements = self.foreach_elements(src, 0)?;
                let key = self.resolve(iter);
                self.for_each.insert(
                    key,
                    ForEachState {
                        elements,
                        position: 0,
                    },
                );
            }
            OxInst::ForEachNext {
                iter,
                item,
                has_value,
            } => {
                let key = self.resolve(iter);
                let next = self.for_each.get_mut(&key).and_then(|state| {
                    let value = state.elements.get(state.position).cloned();
                    if value.is_some() {
                        state.position += 1;
                    }
                    value
                });
                match next {
                    Some(value) => {
                        self.store(item, value)?;
                        self.store(has_value, Variant::from_bool(true))?;
                    }
                    None => self.store(has_value, Variant::from_bool(false))?,
                }
            }

            // ── Records / UDT (M3-3) — value aggregates with native VBA layout ────────
            OxInst::NewRecord { dst, fields } => {
                let layout = vba_record_layout_for_fields(fields)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                let record = VbaRecord::new_default(layout)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                self.store(dst, Variant::from_vba_record(record))?;
            }
            OxInst::RecordGet { dst, record, index } => {
                let source = self.operand(record)?;
                let value = source
                    .read_record_field_variant(*index)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                self.store(dst, value)?;
            }
            OxInst::RecordArrayGet {
                dst,
                record,
                index,
                indices,
            } => {
                let value = self.record_array_get(record, *index, indices)?;
                self.store(dst, value)?;
            }
            OxInst::RecordSet {
                record,
                index,
                value,
            } => {
                let v = self.operand(value)?;
                // Read the (alias-resolved) record, write the field, store it back — value
                // semantics: the record's data is owned, so a ByRef-aliased record's backing
                // receives the write (equivalent to vm2's in-place `read_place_mut`).
                let mut target = self.read(record)?;
                target
                    .write_record_field_variant(*index, &v)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                self.store(record, target)?;
            }
            OxInst::RecordLSet { record, value } => {
                let source = self.operand(value)?;
                let mut target = self.read(record)?;
                target
                    .lset_record_from(&source)
                    .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                self.store(record, target)?;
            }
            OxInst::RecordArraySet {
                record,
                index,
                indices,
                value,
            } => {
                let v = self.operand(value)?;
                self.record_array_set(record, *index, indices, &v)?;
            }

            // ── Objects / lifecycle / type identity (M3-5) ───────────────────────────
            OxInst::NewObject { dst, class } => {
                let object = self.new_project_instance(class.0)?;
                self.store(dst, object)?;
            }
            OxInst::NewExtern { dst, import } => {
                let object = self.new_extern_instance(*import)?;
                self.store(dst, object)?;
            }
            OxInst::Predeclared { dst, class } => {
                let object = self.predeclared_instance(class.0)?;
                self.store(dst, object)?;
            }
            OxInst::PredeclaredExtern { dst, import } => {
                let object = self.predeclared_extern_instance(*import)?;
                self.store(dst, object)?;
            }
            OxInst::PredeclaredSet { class, value } => {
                let object = self.operand(value)?;
                self.set_predeclared_instance(self.cur, class.0, object)?;
            }
            OxInst::PredeclaredExternSet { import, value } => {
                let object = self.operand(value)?;
                self.set_predeclared_extern_instance(*import, object)?;
            }
            OxInst::FieldGet { dst, object, field } => {
                let recv = self.operand(object)?;
                let instance = variant_to_object(&recv)?;
                let value = self.read_project_field_as_new(&instance, *field)?;
                self.store(dst, value)?;
            }
            OxInst::FieldSet {
                object,
                field,
                value,
            } => {
                let v = self.operand(value)?;
                let recv = self.operand(object)?;
                let instance = variant_to_object(&recv)?;
                instance.project_field_set(*field, v);
            }
            OxInst::CompareObjectIs { dst, lhs, rhs } => {
                let a = object_identity_for_is(&self.operand(lhs)?)?;
                let b = object_identity_for_is(&self.operand(rhs)?)?;
                self.store(dst, Variant::from_bool(a == b))?;
            }
            OxInst::TypeOfIs {
                dst,
                object,
                type_name,
            } => {
                let matches = self.type_of_is(object, type_name)?;
                self.store(dst, Variant::from_bool(matches))?;
            }

            // ── Events / WithEvents (M3-6) ───────────────────────────────────────────
            OxInst::WithEventsGet {
                dst,
                owner,
                binding,
            } => {
                let owner_value = self.operand(owner)?;
                let owner_ref = variant_to_object(&owner_value)?;
                let key = withevents_key(&owner_ref, *binding as i64);
                let value = self
                    .exec
                    .events
                    .withevents
                    .get(&key)
                    .map(|b| b.source.clone())
                    .unwrap_or_else(|| Variant::from_i32(0));
                self.store(dst, value)?;
            }
            OxInst::WithEventsSet {
                dst,
                owner,
                binding,
                value,
            } => {
                let owner_value = self.operand(owner)?;
                let owner_ref = variant_to_object(&owner_value)?;
                let key = withevents_key(&owner_ref, *binding as i64);
                let v = self.operand(value)?;
                // Replacing a binding tears down its old host (COM) subscriptions first.
                self.unsubscribe_com_key(key);
                if is_nothing(&v) {
                    self.exec.events.withevents.remove(&key);
                } else {
                    // A COM/foreign source is wired through the host's connection points (the
                    // shared, live-tested HAL `subscribe_event`); a project source dispatches
                    // internally via `RaiseEvent` (no host subscription). Mirrors vm2's
                    // WithEventsSet.
                    if let Some(source) = v.as_object_ref()
                        && !source.is_project_instance()
                    {
                        self.subscribe_com_events(key, *binding, &owner_value, &source);
                    }
                    let order = self.exec.events.next_withevents_order;
                    self.exec.events.next_withevents_order =
                        self.exec.events.next_withevents_order.wrapping_add(1);
                    self.exec.events.withevents.insert(
                        key,
                        EventBinding {
                            owner: owner_value,
                            source: v.clone(),
                            order,
                        },
                    );
                }
                self.store(dst, v)?;
            }
            OxInst::WithEventsClearOwner { dst, owner } => {
                let owner_ref = variant_to_object(&self.operand(owner)?)?;
                let owner_raw = owner_ref.raw();
                self.unsubscribe_com_owner(owner_raw);
                self.exec
                    .events
                    .withevents
                    .retain(|key, _| withevents_owner_raw(*key) != owner_raw);
                self.store(dst, Variant::from_i32(0))?;
            }
            OxInst::WithEventsFirstOwner {
                dst,
                source,
                binding,
            } => {
                let source = self.operand(source)?;
                let mut owners: Vec<(u64, ObjectRef)> = Vec::new();
                if !is_nothing(&source) {
                    for (key, binding_data) in &self.exec.events.withevents {
                        if withevents_binding(*key) == (*binding as i64 & 0xFFFF_FFFF)
                            && object_identity(&binding_data.source) == object_identity(&source)
                            && let Some(owner) = binding_data.owner.as_object_ref()
                        {
                            owners.push((binding_data.order, owner));
                        }
                    }
                }
                owners.sort_unstable_by_key(|(order, _)| *order);
                let owners: Vec<ObjectRef> = owners.into_iter().map(|(_, owner)| owner).collect();
                match owners.first().cloned() {
                    Some(first) => {
                        self.exec.events.withevents_iters.push((owners, 1));
                        self.store(dst, Variant::from_object_ref(first))?;
                    }
                    None => self.store(dst, Variant::from_i32(0))?,
                }
            }
            OxInst::WithEventsNextOwner { dst } => {
                let next =
                    self.exec
                        .events
                        .withevents_iters
                        .last_mut()
                        .and_then(|(owners, pos)| {
                            let value = owners.get(*pos).cloned();
                            if value.is_some() {
                                *pos += 1;
                            }
                            value
                        });
                match next {
                    Some(owner) => self.store(dst, Variant::from_object_ref(owner))?,
                    None => {
                        self.exec.events.withevents_iters.pop();
                        self.store(dst, Variant::from_i32(0))?;
                    }
                }
            }
            OxInst::RaiseEvent {
                source,
                event,
                args,
            } => {
                let source_object = variant_to_object(&self.operand(source)?)?;
                let source_id = source_object.raw();
                let event_id = *event;
                // Collect subscribers whose binding holds this source and routes this event,
                // then run each handler in VBA subscription order with the sink as `me` and
                // the event args; an unhandled error propagates to the raiser.
                let mut targets: Vec<(u64, Variant, usize, usize)> = Vec::new();
                for (key, binding) in &self.exec.events.withevents {
                    if object_identity(&binding.source) != source_id {
                        continue;
                    }
                    let token = withevents_binding(*key) as i32;
                    // A sink's event routes + handler proc live in the SINK OWNER's program, which
                    // may differ from the raiser's `cur` for a cross-project `WithEvents` (project
                    // A sinks project B's source). Look the route up — and run the handler — there.
                    let owner_bundle = binding
                        .owner
                        .as_object_ref()
                        .map(|o| o.bundle_id() as usize)
                        .unwrap_or(self.cur);
                    if let Some(&handler) = self.exec.programs[owner_bundle]
                        .event_routes
                        .get(&(token, event_id))
                    {
                        targets.push((binding.order, binding.owner.clone(), handler, owner_bundle));
                    }
                }
                targets.sort_by_key(|(order, ..)| *order);
                for (_, sink, handler, owner_bundle) in targets {
                    self.run_proc_with_me(owner_bundle, FuncId(handler), sink, args, false)?;
                }
                // After the internal WithEvents fan-out, deliver to the host event sink (W7):
                // the COM server forwards the event to its connection-point clients. Take the
                // sink out across the call so it does not alias `&mut self` (a host sink calls
                // back into the host, not the VM), then restore it.
                if self.exec.events.project_event_sink.is_some() {
                    let values = self.extern_args(args)?;
                    let mut sink = self
                        .exec
                        .events
                        .project_event_sink
                        .take()
                        .expect("sink present");
                    let outcome = sink(source_object.clone(), event_id, values);
                    self.exec.events.project_event_sink = Some(sink);
                    outcome.map_err(|msg| Vm3Error::Fault(Fault::new(5, msg)))?;
                }
            }

            // ── Early-bound, descriptor-typed COM dispatch (M3-9) ────────────────────
            // The typed descriptor gives the dispid (member token) and the call-site accessor;
            // vm3 dispatches by `TokenNamed{dispid,name}` through the host's COM facet, and the
            // host's `PreferVtable` strategy slot-calls the live object's vtable (driven by the
            // object's own typelib, not by the descriptor's slot) — the identical transport vm2's
            // early-bound (`EarlyCom`→`DispatchIdNamed`) takes, so value + transport counts match.
            // A receiver that is a project instance (a typed call through an `Implements`
            // interface) dispatches internally by the member's name.
            OxInst::ComCallEarly {
                dst,
                method,
                invoke_kind,
                recv,
                args,
            } => {
                let recv_v = self.operand(recv)?;
                let object = variant_to_object(&recv_v)?;
                let descriptor = self.cur_program().com_method(*method).ok_or_else(|| {
                    Vm3Error::Malformed(format!("ComCallEarly: unresolved method ref {method:?}"))
                })?;
                let dispid = descriptor.token;
                let member_name = descriptor.name.clone();
                let ret = if object.is_project_instance() {
                    self.dispatch_project_method(object, recv_v, &member_name, *invoke_kind, args)?
                } else {
                    self.dispatch_com_method(
                        object,
                        DynamicMemberSelector::TokenNamed {
                            token: dispid,
                            name: member_name,
                        },
                        *invoke_kind,
                        args,
                    )?
                };
                if let Some(dst) = dst {
                    self.store(dst, ret)?;
                }
            }

            // ── Late-bound member dispatch (M3-6: project instances; M3-8: COM) ──────
            OxInst::ComCallLate {
                dst,
                recv,
                name,
                default_member,
                invoke_kind,
                args,
            } => {
                let recv_v = self.operand(recv)?;
                let ret = if *default_member {
                    self.dispatch_default_member(recv_v, *invoke_kind, args)?
                } else {
                    self.dispatch_member_by_name(recv_v, name, *invoke_kind, args)?
                };
                if let Some(dst) = dst {
                    self.store(dst, ret)?;
                }
            }
            // `CallByName(obj, "Member", vbMethod|vbGet|vbLet|vbSet, args…)` — the genuinely
            // dynamic by-name dispatch. The runtime `calltype` integer selects the accessor;
            // an out-of-range value raises error 5 (matching vm2).
            OxInst::CallByName {
                dst,
                object,
                name,
                calltype,
                args,
            } => {
                let recv_v = self.operand(object)?;
                let member_name = arith::as_string(&self.operand(name)?);
                let ct = arith::int(&self.operand(calltype)?).map_err(arith_fault)?;
                let invoke_kind = match ct {
                    1 => TypeLibMemberInvokeKind::Method,
                    2 => TypeLibMemberInvokeKind::PropertyGet,
                    4 => TypeLibMemberInvokeKind::PropertyPut,
                    8 => TypeLibMemberInvokeKind::PropertyPutRef,
                    _ => {
                        return Err(Vm3Error::Fault(Fault::new(
                            5,
                            "CallByName: invalid CallType",
                        )));
                    }
                };
                let ret = self.dispatch_member_by_name(recv_v, &member_name, invoke_kind, args)?;
                if let Some(dst) = dst {
                    self.store(dst, ret)?;
                }
            }

            // Everything else is a later milestone (cross-bundle calls / objects / COM /
            // arrays / records M3) — explicit, never a silent no-op.
            other => {
                return Err(Vm3Error::Unimplemented {
                    what: inst_kind(other),
                });
            }
        }
        Ok(())
    }

    /// Call a compiled VBA procedure: evaluate the arguments in the *caller* and push a
    /// callee frame (ByVal copies the value in; ByRef true-aliases the caller's backing
    /// location so writes propagate live; an omitted optional gets the `MISSING_ARG`
    /// sentinel), then hand control to the dispatch loop. The return value is copied out
    /// when the frame returns (see `do_return`). Mirrors vm2's `call_proc`.
    fn call_proc_in(
        &mut self,
        target_prog: usize,
        dst: Option<OxPlace>,
        proc: FuncId,
        args: &[OxArg],
    ) -> Result<(), Vm3Error> {
        // The callee runs against `target_prog` (its own program for a cross-project call); the
        // arguments + result dst are resolved below in the CALLER's context (`cur`), so a ByRef
        // alias / the dst stay bound to the caller's program (see `Loc::Global` tagging).
        let program = self.exec.programs[target_prog].program;
        let callee = program
            .funcs
            .get(proc.0)
            .ok_or_else(|| Vm3Error::Malformed(format!("call to unknown proc {}", proc.0)))?;
        // Resolve the destination + ByRef backings in the caller, before pushing.
        let dst_loc = dst.map(|p| self.resolve(&p));
        let mut locals: Vec<Variant> = callee
            .locals
            .iter()
            .map(|local| Self::initial_value_for_slot(&local.ty, local.array_element.as_ref()))
            .collect();
        let mut aliases = HashMap::new();
        let frame_index = self.frames.len();
        let mut pending_param_array_aliases = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            match arg {
                OxArg::ByVal(op) => {
                    let param_array_aliases = self.param_array_aliases_for_operand(op);
                    let v = self.operand(op)?;
                    if let Some(slot) = locals.get_mut(i) {
                        *slot = v;
                    }
                    if let Some(param_array_aliases) = param_array_aliases {
                        pending_param_array_aliases
                            .push((Loc::Local(frame_index, i), param_array_aliases));
                    }
                }
                OxArg::ByRef(place) => {
                    aliases.insert(i, self.resolve(place));
                }
                OxArg::Omitted => {
                    if let Some(slot) = locals.get_mut(i) {
                        *slot = Variant::from_error_code(MISSING_ARG);
                    }
                }
            }
        }
        let return_local = callee.return_local;
        let entry = callee.entry;

        self.guard_call_depth()?;
        // Push the callee and hand control to it; each procedure starts with no active
        // handler, and the caller's mode is restored from the frame when it returns. The
        // dispatch loop runs the callee and `do_return`/`propagate_fault` pops it — there
        // is no native recursion here, so the call depth is heap-bounded.
        let saved_err = self.exec.err_engine.enter_activation();
        self.frames.push(Frame {
            prog: target_prog,
            func: proc,
            block: entry,
            ip: 0,
            locals,
            temps: HashMap::new(),
            aliases,
            dst: dst_loc,
            return_local,
            current_line: 0,
            saved_error_mode: saved_err.error_mode,
            saved_active_error: saved_err.active_error,
            gosub_stack: Vec::new(),
        });
        for (loc, aliases) in pending_param_array_aliases {
            self.param_array_aliases.insert(loc, aliases);
        }
        Ok(())
    }

    /// A cross-bundle call (`OxInst::CallExtern`). Resolve `import` to a native library
    /// function and run it through the same `invoke_native_lib` bridge a `CallNative`
    /// builtin uses — no frame is pushed (a `NativeBody::Library` body has no VM frame; its
    /// arguments are positional ByVal values), mirroring vm2's `call_extern` short-circuit
    /// and keeping a library function bit-identical however it is routed.
    fn call_extern(
        &mut self,
        dst: Option<OxPlace>,
        import: ImportId,
        args: &[OxArg],
    ) -> Result<(), Vm3Error> {
        let imp = self.cur_program().imports.get(import.0).ok_or_else(|| {
            Vm3Error::Malformed(format!("CallExtern names unknown import {}", import.0))
        })?;
        // A cross-PROJECT reference (a non-`VBA` unit) is a VBA-bodied proc in another loaded
        // program: resolve it by unit name + export token and call INTO that program — the
        // callee frame carries `prog = B` so it runs against B's globals, while the result dst
        // and any ByRef args are resolved in this caller's program (see `Loc::Global` tagging).
        // The synthetic `VBA` unit is the built-in native-library path below.
        if !imp.unit.eq_ignore_ascii_case("VBA") {
            let b = self.program_index_by_unit(&imp.unit).ok_or_else(|| {
                Vm3Error::Malformed(format!("unresolved reference to unit '{}'", imp.unit))
            })?;
            let proc = self.exec.programs[b]
                .program
                .exports
                .iter()
                .find(|e| e.token.matches(&imp.token))
                .ok_or_else(|| {
                    Vm3Error::Malformed(format!(
                        "unit '{}' has no export matching the CallExtern import",
                        imp.unit
                    ))
                })
                .and_then(|export| match export.target {
                    oxvba_bundle::ExportTarget::Proc(p) => Ok(p),
                    _ => Err(Vm3Error::Malformed(
                        "a cross-project CallExtern resolved to a non-procedure export".into(),
                    )),
                })?;
            return self.call_proc_in(b, dst, FuncId(proc), args);
        }
        let (id, string_typed_alias) = self.resolve_library_import(import)?;
        let argv = self.extern_args(args)?;
        let result = self.invoke_native_lib_with_policy(id, &argv, string_typed_alias)?;
        if let Some(dst) = dst {
            self.store(&dst, result)?;
        }
        Ok(())
    }

    /// Resolve a cross-bundle `import` to the native library function it names.
    ///
    /// vm3 links the synthetic `VBA` library bundle
    /// (`oxvba_bundle::vba_library_bundle`) here: the home of every built-in
    /// function (`Strings.Left`, `Math.Abs`, the
    /// `DateTime`/`Conversion`/`Information`/`FileSystem` members, ...), which
    /// the binder lowers to a `CallExtern` rather than a `CallNative`. Non-`VBA`
    /// project references are handled by [`Self::call_extern`] before this helper
    /// is reached.
    fn resolve_library_import(&self, import: ImportId) -> Result<(NativeImplId, bool), Vm3Error> {
        let imp = self.cur_program().imports.get(import.0).ok_or_else(|| {
            Vm3Error::Malformed(format!("CallExtern names unknown import {}", import.0))
        })?;
        debug_assert!(imp.unit.eq_ignore_ascii_case("VBA"));
        let lib = oxvba_bundle::vba_library_bundle();
        let export = lib
            .exports
            .iter()
            .find(|e| e.token.matches(&imp.token))
            .ok_or_else(|| {
                Vm3Error::Malformed(format!(
                    "the VBA library bundle has no export matching import {}",
                    import.0
                ))
            })?;
        let oxvba_bundle::ExportTarget::Proc(proc) = export.target else {
            return Err(Vm3Error::Malformed(
                "a VBA library import resolved to a non-procedure export".into(),
            ));
        };
        match lib.procedures.get(proc).and_then(|p| p.native) {
            Some(oxvba_bundle::NativeBody::Library(id)) => {
                let string_typed_alias = match &imp.token {
                    oxvba_bundle::ExportToken::ModuleFunc { member, .. } => {
                        id.is_string_typed_library_alias(member)
                    }
                    _ => false,
                };
                Ok((id, string_typed_alias))
            }
            Some(oxvba_bundle::NativeBody::Method(_)) => Err(Vm3Error::Malformed(
                "a native object method is not callable via CallExtern".into(),
            )),
            None => Err(Vm3Error::Malformed(
                "a VBA library export has no native library body".into(),
            )),
        }
    }

    /// Marshal a cross-bundle library call's arguments to plain values: a native library
    /// body reads positional values (a ByRef argument by its *value*), and an omitted
    /// optional is `Empty` — matching vm2's `extern_native_args`.
    fn extern_args(&mut self, args: &[OxArg]) -> Result<Vec<Variant>, Vm3Error> {
        marshal_ox_args(
            args,
            |arg| match arg {
                MarshalArgRef::Operand(op) => self.operand(op),
                MarshalArgRef::ByRef(place) => self.read(place),
            },
            Variant::empty,
        )
    }

    /// Build SAFEARRAY bounds from `ReDim` upper/lower-bound operands. Each
    /// bound is a VBA `Long`, so a value outside `Long` range raises Overflow
    /// (6) instead of silently wrapping through `as i32`; `upper < lower` →
    /// subscript out of range (9); and a single dimension above `u32::MAX`
    /// elements → out of memory (7). These guard the bound *shape* only — the
    /// actual element store is allocated fallibly (the zeroed-scalar path and
    /// [`try_build_default_elements`]), so an over-large array raises catchable
    /// error 7 rather than aborting the host on an infallible allocation.
    fn build_bounds(
        &mut self,
        upper_bounds: &[OxOperand],
        lower_bounds: &[OxOperand],
    ) -> Result<Vec<SafeArrayBound>, Vm3Error> {
        let mut bounds = Vec::with_capacity(upper_bounds.len());
        for (i, upper_op) in upper_bounds.iter().enumerate() {
            let lower = if let Some(lower_op) = lower_bounds.get(i) {
                let lower_v = self.operand(lower_op)?;
                subscript_to_long(arith::int(&lower_v).map_err(arith_fault)?)?
            } else {
                0
            };
            let upper_v = self.operand(upper_op)?;
            let upper = subscript_to_long(arith::int(&upper_v).map_err(arith_fault)?)?;
            if upper < lower {
                return Err(Vm3Error::Fault(Fault::new(
                    9,
                    "array upper bound below lower bound",
                )));
            }
            let span = i64::from(upper) - i64::from(lower) + 1;
            if span > i64::from(u32::MAX) {
                return Err(Vm3Error::Fault(Fault::new(
                    7,
                    format!("array dimension too large ({span} elements)"),
                )));
            }
            bounds.push(SafeArrayBound {
                count: span as u32,
                lower,
            });
        }
        Ok(bounds)
    }

    /// Flat element index from VBA (absolute) subscript operands, C-order (first dimension
    /// outermost), bounds-checked → subscript out of range (9).
    fn flat_index(
        &mut self,
        indices: &[OxOperand],
        bounds: &[SafeArrayBound],
    ) -> Result<usize, Vm3Error> {
        if indices.len() != bounds.len() {
            return Err(Vm3Error::Fault(Fault::new(
                9,
                "wrong number of array subscripts",
            )));
        }
        if let ([index_op], [bound]) = (indices, bounds) {
            let index_v = self.operand(index_op)?;
            let raw = subscript_to_long(arith::int(&index_v).map_err(arith_fault)?)?;
            let offset = i64::from(raw) - i64::from(bound.lower);
            if offset < 0 || offset >= i64::from(bound.count) {
                return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
            }
            return Ok(offset as usize);
        }
        let mut flat = 0usize;
        for (i, index_op) in indices.iter().enumerate() {
            let index_v = self.operand(index_op)?;
            let raw = subscript_to_long(arith::int(&index_v).map_err(arith_fault)?)?;
            let bound = &bounds[i];
            let offset = i64::from(raw) - i64::from(bound.lower);
            if offset < 0 || offset >= i64::from(bound.count) {
                return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
            }
            flat = flat * bound.count as usize + offset as usize;
        }
        Ok(flat)
    }

    /// Fast O(1) `arr(i…)` read: when the receiver is a place-resident array, read
    /// the single element directly through the SAFEARRAY descriptor without cloning
    /// the (possibly large) backing store — so an array loop is O(N), not O(N²).
    /// Returns `Ok(None)` when the receiver is not a place-resident array (a `Const`,
    /// an unwritten `Temp`, an object default-member receiver, …), so the caller runs
    /// the general path.
    fn array_get_fast(
        &mut self,
        array: &OxOperand,
        indices: &[OxOperand],
    ) -> Result<Option<Variant>, Vm3Error> {
        let OxOperand::Use(place) = array else {
            return Ok(None);
        };
        let loc = self.resolve(place);
        let Some(arr) = self.read_loc_ref(loc)? else {
            return Ok(None);
        };
        // Only an in-place array takes the fast path; objects/other receivers fall back.
        let Some((bounds, len)) = arr.safearray_bounds_len() else {
            return Ok(None);
        };
        let flat = self.flat_index(indices, &bounds)?;
        if flat >= len {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        let Some(arr) = self.read_loc_ref(loc)? else {
            return Ok(None);
        };
        if let Some(result) = arr.safearray_i32_element(flat) {
            match result {
                Ok(Some(value)) => return Ok(Some(Variant::from_i32(value))),
                Ok(None) => {}
                Err(err) => return Err(Vm3Error::Fault(Fault::new(13, err))),
            }
        }
        let value = arr
            .safearray_element(flat)
            .expect("safearray_bounds_len already proved this is an array")
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
        Ok(Some(value))
    }

    /// Fast O(1) `arr(i…) = v` write: when the receiver is a place-resident array,
    /// mutate the single element through the SAFEARRAY descriptor in place — no
    /// per-write deep-clone-and-write-back of the whole array. A ByRef-aliased array
    /// is mutated through its resolved (aliased) location, so callers see the change.
    /// Returns `Ok(false)` when the receiver is not a place-resident array.
    fn array_set_fast(
        &mut self,
        array: &OxPlace,
        indices: &[OxOperand],
        value: &Variant,
    ) -> Result<bool, Vm3Error> {
        let loc = self.resolve(array);
        // Bounds + flat index under an immutable borrow first (reads index operands),
        // then take the mutable borrow to write the element in place.
        let (bounds, len) = match self.read_loc_ref(loc)? {
            Some(arr) => match arr.safearray_bounds_len() {
                Some(bl) => bl,
                None => return Ok(false),
            },
            None => return Ok(false),
        };
        let flat = self.flat_index(indices, &bounds)?;
        if flat >= len {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        let arr = self
            .read_loc_mut(loc)?
            .expect("location borrowed immutably just above is still present");
        if let Some(value_i32) = value.as_i32()
            && let Some(result) = arr.set_safearray_i32_element(flat, value_i32)
        {
            match result {
                Ok(true) => {
                    self.mirror_param_array_element_write(loc, flat, value)?;
                    return Ok(true);
                }
                Ok(false) => {}
                Err(err) => return Err(Vm3Error::Fault(Fault::new(13, err))),
            }
        }
        arr.set_safearray_element(flat, value)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
        self.mirror_param_array_element_write(loc, flat, value)?;
        Ok(true)
    }

    fn param_array_aliases_for_operand(&self, op: &OxOperand) -> Option<Vec<Option<Loc>>> {
        match op {
            OxOperand::Use(place) => self.param_array_aliases.get(&self.resolve(place)).cloned(),
            _ => None,
        }
    }

    fn mirror_param_array_element_write(
        &mut self,
        array_loc: Loc,
        flat: usize,
        value: &Variant,
    ) -> Result<(), Vm3Error> {
        let Some(aliases) = self.param_array_aliases.get(&array_loc).cloned() else {
            return Ok(());
        };
        let Some(Some(target)) = aliases.get(flat).copied() else {
            return Ok(());
        };

        self.write_loc(target, value.clone())?;

        if let Some(arr) = self.read_loc_mut(array_loc)? {
            for (idx, alias) in aliases.iter().enumerate() {
                if idx != flat && *alias == Some(target) {
                    arr.set_safearray_element(idx, value)
                        .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
                }
            }
        }
        Ok(())
    }

    fn prune_param_array_aliases_from_depth(&mut self, depth: usize) {
        self.param_array_aliases.retain(|loc, aliases| {
            loc.frame_index().is_none_or(|frame| frame < depth)
                && aliases
                    .iter()
                    .flatten()
                    .all(|alias| alias.frame_index().is_none_or(|frame| frame < depth))
        });
        self.as_new_slots
            .retain(|loc, _| loc.frame_index().is_none_or(|frame| frame < depth));
    }

    fn record_array_get(
        &mut self,
        record: &OxOperand,
        index: usize,
        indices: &[OxOperand],
    ) -> Result<Variant, Vm3Error> {
        let OxOperand::Use(place) = record else {
            return Err(Vm3Error::Fault(Fault::new(
                13,
                "record array field requires a place",
            )));
        };
        let loc = self.resolve(place);
        let Some(record) = self.read_loc_ref(loc)? else {
            return Err(Vm3Error::Fault(Fault::new(13, "expected Record Variant")));
        };
        let Some((bounds, len)) = record
            .record_array_field_bounds_len(index)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?
        else {
            return Err(Vm3Error::Fault(Fault::new(13, "Expected array")));
        };
        let flat = self.flat_index(indices, &bounds)?;
        if flat >= len {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        let Some(record) = self.read_loc_ref(loc)? else {
            return Err(Vm3Error::Fault(Fault::new(13, "expected Record Variant")));
        };
        record
            .record_array_field_element(index, flat)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?
            .ok_or_else(|| Vm3Error::Fault(Fault::new(13, "Expected array")))
    }

    fn record_array_set(
        &mut self,
        record: &OxPlace,
        index: usize,
        indices: &[OxOperand],
        value: &Variant,
    ) -> Result<(), Vm3Error> {
        let loc = self.resolve(record);
        let Some(record) = self.read_loc_ref(loc)? else {
            return Err(Vm3Error::Fault(Fault::new(13, "expected Record Variant")));
        };
        let Some((bounds, len)) = record
            .record_array_field_bounds_len(index)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?
        else {
            return Err(Vm3Error::Fault(Fault::new(13, "Expected array")));
        };
        let flat = self.flat_index(indices, &bounds)?;
        if flat >= len {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        let record = self
            .read_loc_mut(loc)?
            .expect("location borrowed immutably just above is still present");
        record
            .set_record_array_field_element(index, flat, value)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?
            .ok_or_else(|| Vm3Error::Fault(Fault::new(13, "Expected array")))
    }

    fn is_unallocated_array_value(value: &Variant) -> bool {
        value.vtype() == VarType::ArrayVariant && value.safearray_bounds_len().is_none()
    }

    /// The array (SAFEARRAY) value of an operand, else type mismatch (13).
    /// A null array marker is still an array value in VBA, but has no descriptor/bounds;
    /// indexing and bounds queries surface subscript out of range (9).
    fn array_of(&mut self, op: &OxOperand) -> Result<SafeArray, Vm3Error> {
        let value = self.operand(op)?;
        if Self::is_unallocated_array_value(&value) {
            return Err(Vm3Error::Fault(Fault::new(9, "array has no bounds")));
        }
        value
            .as_safearray()
            .ok_or_else(|| Vm3Error::Fault(Fault::new(13, "expected an array")))
    }

    /// Index a received receiver VALUE `recv` and store the element into `dst`. The
    /// receiver is either an array (element read) or a built-in `Collection` object
    /// whose default member `Item` is indexed (`c(i)`). Shared by the `ArrayGet`
    /// non-fast path and the `FieldArrayGet` non-array-field fallback.
    fn index_value_into(
        &mut self,
        recv: Variant,
        indices: &[OxOperand],
        dst: &OxPlace,
    ) -> Result<(), Vm3Error> {
        let arr = match recv.as_safearray() {
            Some(arr) => arr,
            None if Self::is_unallocated_array_value(&recv) => {
                return Err(Vm3Error::Fault(Fault::new(9, "array has no bounds")));
            }
            None => {
                if let Some(obj) = recv.as_object_ref() {
                    let argv = self.operands_to_values(indices)?;
                    let value = self.dispatch_default_member_values(
                        Variant::from_object_ref(obj),
                        TypeLibMemberInvokeKind::PropertyGet,
                        argv,
                    )?;
                    self.store(dst, value)?;
                    return Ok(());
                }
                return Err(Vm3Error::Fault(Fault::new(13, "expected an array")));
            }
        };
        let bounds = arr
            .bounds()
            .ok_or_else(|| Vm3Error::Fault(Fault::new(9, "array has no bounds")))?;
        let flat = self.flat_index(indices, &bounds)?;
        if flat >= arr.len() {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        if let Some(result) = recv.safearray_i32_element(flat) {
            match result {
                Ok(Some(value)) => return self.store(dst, Variant::from_i32(value)),
                Ok(None) => {}
                Err(err) => return Err(Vm3Error::Fault(Fault::new(13, err))),
            }
        }
        let value = arr
            .variant_element(flat)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
        self.store(dst, value)
    }

    /// Set one element of a received array VALUE `recv` and return the mutated array
    /// for the caller to write back where `recv` came from. Errors if `recv` is an
    /// object (default-member assignment unsupported) or not an array. Shared by the
    /// `ArraySet` non-fast path and the `FieldArraySet` non-array-field fallback.
    fn set_index_in_value(
        &mut self,
        mut recv: Variant,
        indices: &[OxOperand],
        value: &Variant,
    ) -> Result<Variant, Vm3Error> {
        let arr = match recv.as_safearray() {
            Some(arr) => arr,
            None if Self::is_unallocated_array_value(&recv) => {
                return Err(Vm3Error::Fault(Fault::new(9, "array has no bounds")));
            }
            None => {
                if let Some(obj) = recv.as_object_ref() {
                    let mut argv = self.operands_to_values(indices)?;
                    argv.push(value.clone());
                    let invoke_kind = if value.as_object_ref().is_some() {
                        TypeLibMemberInvokeKind::PropertyPutRef
                    } else {
                        TypeLibMemberInvokeKind::PropertyPut
                    };
                    self.dispatch_default_member_values(
                        Variant::from_object_ref(obj),
                        invoke_kind,
                        argv,
                    )?;
                    return Ok(recv);
                }
                return Err(Vm3Error::Fault(Fault::new(13, "expected an array")));
            }
        };
        let bounds = arr
            .bounds()
            .ok_or_else(|| Vm3Error::Fault(Fault::new(9, "array has no bounds")))?;
        let flat = self.flat_index(indices, &bounds)?;
        if flat >= arr.len() {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        if let Some(value_i32) = value.as_i32()
            && let Some(result) = recv.set_safearray_i32_element(flat, value_i32)
        {
            match result {
                Ok(true) => return Ok(recv),
                Ok(false) => {}
                Err(err) => return Err(Vm3Error::Fault(Fault::new(13, err))),
            }
        }
        recv.set_safearray_element(flat, value)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
        Ok(recv)
    }

    /// The 0-based dimension index for `LBound`/`UBound` from an optional dimension operand
    /// (default dimension 1), validated against the array's rank → subscript out of range (9).
    fn array_bound_index(
        &mut self,
        dimension: Option<&OxOperand>,
        bounds: &[SafeArrayBound],
    ) -> Result<usize, Vm3Error> {
        let dim = match dimension {
            Some(op) => {
                let v = self.operand(op)?;
                arith::int(&v).map_err(arith_fault)?
            }
            None => 1,
        };
        if dim < 1 {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        let index = (dim - 1) as usize;
        if index >= bounds.len() {
            return Err(Vm3Error::Fault(Fault::new(9, "subscript out of range")));
        }
        Ok(index)
    }

    /// `ReDim [Preserve]`: build the new SAFEARRAY shaped by `upper_bounds`/`lower_bounds`,
    /// seeding each element with the declared element type's typed default — except, when
    /// `preserve`, keeping each still-in-range existing element (so a UDT array's populated
    /// records survive and only the grown tail is freshly default-seeded). The element storage
    /// matches the declared element type (typed scalars / native records, not normalized to
    /// VT_VARIANT).
    fn array_redim(
        &mut self,
        dst: &OxPlace,
        upper_bounds: &[OxOperand],
        lower_bounds: &[OxOperand],
        element: &ArrayElementType,
        preserve: bool,
        fixed: bool,
    ) -> Result<(), Vm3Error> {
        // A user `ReDim` (lowered with `fixed = false`) of an already-fixed-size array is
        // illegal in VBA — `Dim a(1 To 3) : ReDim a(...)` is the compile error "Array already
        // dimensioned". vm3 carries fixed-ness only on the runtime value (the symbol model
        // can't tell a fixed top-level `Dim` from a dynamic one), so the faithful surfacing is
        // the runtime analog, error 10 "This array is fixed or temporarily locked". The
        // fixed-`Dim`/UDT-field allocation itself is `fixed = true` against an uninitialized
        // slot, so it is never caught here.
        if !fixed
            && self
                .read(dst)?
                .as_safearray()
                .is_some_and(|a| a.is_fixed_size())
        {
            return Err(Vm3Error::Fault(Fault::new(
                10,
                "This array is fixed or temporarily locked",
            )));
        }
        let bounds = self.build_bounds(upper_bounds, lower_bounds)?;
        // Total element count, guarding the cross-dimension product (`build_bounds` already
        // guards each single dimension's span) → out of memory (7) rather than a usize wrap
        // that would feed a bogus allocation.
        let mut count = 1usize;
        for b in &bounds {
            count = count
                .checked_mul(b.count as usize)
                .ok_or_else(|| Vm3Error::Fault(Fault::new(7, "array too large to allocate")))?;
        }
        if !preserve && Self::element_supports_zeroed_redim(element) {
            let array = SafeArray::from_zeroed_typed_scalars_nd(
                bounds,
                safearray_vartype_for_element(element),
            )
            .map(|array| array.with_fixed_size(fixed))
            .map_err(|_| Vm3Error::Fault(Fault::new(7, "array allocation failed")))?;
            self.store(dst, Variant::from_safearray(array))?;
            return Ok(());
        }
        let elems: Vec<Variant> = if preserve {
            let cur = self.read(dst)?;
            match cur.as_safearray().and_then(|a| {
                a.bounds()
                    .map(|b| (b, a.variant_elements().unwrap_or_default()))
            }) {
                // `ReDim Preserve` over an existing allocation: VBA permits changing ONLY the
                // last dimension's upper bound. Changing the rank, any earlier dimension, or
                // the last dimension's lower bound is subscript out of range (9). Surviving
                // elements are preserved BY COORDINATE (not by flat position), so growing a
                // rank-2 array keeps `a(i, j)` in place.
                Some((old_bounds, old_elems)) => {
                    let rank = bounds.len();
                    let illegal = old_bounds.len() != rank
                        || (0..rank.saturating_sub(1)).any(|i| old_bounds[i] != bounds[i])
                        || match (old_bounds.last(), bounds.last()) {
                            (Some(o), Some(n)) => o.lower != n.lower,
                            _ => false,
                        };
                    if illegal {
                        return Err(Vm3Error::Fault(Fault::new(
                            9,
                            "ReDim Preserve may only resize the last dimension",
                        )));
                    }
                    let mut out = try_build_default_elements(element, count)?;
                    for (old_flat, value) in old_elems.into_iter().enumerate() {
                        if let Some(new_flat) = remap_preserve_index(old_flat, &old_bounds, &bounds)
                        {
                            out[new_flat] = value;
                        }
                    }
                    out
                }
                // `ReDim Preserve` of a not-yet-allocated dynamic array just allocates it.
                None => try_build_default_elements(element, count)?,
            }
        } else {
            try_build_default_elements(element, count)?
        };
        let array = redim_safearray_from_elements(bounds, element, elems, fixed)
            .map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?;
        self.store(dst, Variant::from_safearray(array))?;
        Ok(())
    }

    fn element_supports_zeroed_redim(element: &ArrayElementType) -> bool {
        matches!(
            element,
            ArrayElementType::Integer
                | ArrayElementType::Long
                | ArrayElementType::LongLong
                | ArrayElementType::Byte
                | ArrayElementType::Single
                | ArrayElementType::Double
                | ArrayElementType::Currency
                | ArrayElementType::Date
                | ArrayElementType::Boolean
        )
    }

    /// `Let`/`Set` legality check (mirrors vm2). `Set` requires an object source (else
    /// "Object required" 424); `Let` into an `Object` target requires `Set` when the source
    /// is an object (91) and an object source otherwise (424). The strict `Set` *type* check
    /// (error 13 — a project-instance source must be the target's declared class/interface)
    /// needs the project class tables and lands with the object model (M3-5); until a project
    /// instance can exist, every Set-of-object is a COM/`Nothing` value, which vm2 also
    /// passes — so falling through to `Ok` here is behaviorally exact.
    fn validate_assignment(
        &mut self,
        src: &OxOperand,
        intent: AssignmentIntent,
        target_kind: AssignmentTargetKind,
        target_name: &str,
        target_type_name: &str,
    ) -> Result<(), Vm3Error> {
        use AssignmentIntent as Intent;
        use AssignmentTargetKind as Kind;
        let value = self.operand(src)?;
        // `Nothing` is already a null-object `Variant` (VarType::Object), so the
        // object test is just the type — the old `|| is_nothing(&value)` also
        // matched Empty/Null and numeric 0 (is_nothing's stale scalar-zero
        // sentinel), which let `Set o = 0` silently store a scalar instead of
        // raising "Object required" (424).
        let is_object = matches!(value.vtype(), VarType::Object);
        match intent {
            Intent::Set if !is_object => Err(Vm3Error::Fault(Fault::new(
                424,
                format!("Object required: {target_name}"),
            ))),
            Intent::Let
                if target_kind == Kind::Variant
                    && value.vtype() == VarType::Object
                    && is_nothing(&value) =>
            {
                Err(Vm3Error::Fault(Fault::new(
                    91,
                    "Object variable or With block variable not set",
                )))
            }
            Intent::Let if target_kind == Kind::Object && is_object => Err(Vm3Error::Fault(
                Fault::new(91, format!("Object variable requires Set: {target_name}")),
            )),
            Intent::Let if target_kind == Kind::Object => Err(Vm3Error::Fault(Fault::new(
                424,
                format!("Object required: {target_name}"),
            ))),
            Intent::Set if value.vtype() == VarType::Object && is_nothing(&value) => Ok(()),
            // Strict `Set` type check (error 13): when the target's declared type is a known
            // project class/interface, a project-instance source must be that class or
            // implement that interface. Unconstrained targets (`Object`/`Variant`, or any
            // non-project type) are not checked, and `Nothing` is always allowed. Mirrors
            // vm2's `validate_assignment` (the source object's class + the target type are
            // both resolved in the object's OWN program, against the bare target name).
            Intent::Set if value.vtype() == VarType::Object && !target_type_name.is_empty() => {
                let obj = variant_to_object(&value)?;
                let bare_target = target_type_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(target_type_name);
                if obj.is_project_instance()
                    && let Some(lp) = self.exec.programs.get(obj.bundle_id() as usize)
                {
                    let target_is_project = lp.program.classes.iter().any(|c| {
                        c.name.eq_ignore_ascii_case(bare_target)
                            || c.implements
                                .iter()
                                .any(|i| i.eq_ignore_ascii_case(bare_target))
                    });
                    if target_is_project
                        && let Some(class) = lp.program.classes.get(obj.route_key() as usize)
                    {
                        let compatible = class.name.eq_ignore_ascii_case(bare_target)
                            || class
                                .implements
                                .iter()
                                .any(|i| i.eq_ignore_ascii_case(bare_target));
                        if !compatible {
                            return Err(Vm3Error::Fault(Fault::new(
                                13,
                                format!(
                                    "Type mismatch: `{}` cannot be assigned to `{target_type_name}`",
                                    class.name
                                ),
                            )));
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Allocate a fresh project-class instance (`New <Class>`): a refcounted IUnknown with a
    /// unique identity, then run its `Class_Initialize` (if any). Mirrors vm2's `Op::NewObject`
    /// (bundle id is always 0 — vm3 runs one program). The instance's `has_terminate` flag is
    /// what later parks it for `Class_Terminate` when its last reference drops.
    fn new_project_instance(&mut self, class_idx: usize) -> Result<Variant, Vm3Error> {
        let descriptor = *self.exec.programs[self.cur]
            .class_descriptors
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let program = self.cur_program();
        let class = program
            .classes
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let has_terminate = class.terminate.is_some();
        let initialize = class.initialize;
        let instance_id = self.exec.next_instance_id;
        self.exec.next_instance_id += 1;
        let object = ObjectRef::from_project_instance(
            instance_id,
            class_idx as i32,
            self.cur as i32,
            has_terminate,
            descriptor,
        );
        let value = Variant::from_object_ref(object.clone());
        if let Some(init) = initialize {
            self.run_proc_with_me(self.cur, init, Variant::from_object_ref(object), &[], false)?;
        }
        Ok(value)
    }

    /// A `VB_PredeclaredId` auto-instance (`Class1.Foo` with no explicit `New`): allocate the
    /// singleton + run `Class_Initialize` on first access, then reuse the cached instance.
    /// Mirrors vm2's `predeclared_instance`.
    fn predeclared_instance(&mut self, class_idx: usize) -> Result<Variant, Vm3Error> {
        if let Some(existing) = self.exec.programs[self.cur]
            .predeclared_singletons
            .get(&class_idx)
        {
            return Ok(existing.clone());
        }
        let descriptor = *self.exec.programs[self.cur]
            .class_descriptors
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let program = self.cur_program();
        let class = program
            .classes
            .get(class_idx)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown class {class_idx}")))?;
        let has_terminate = class.terminate.is_some();
        let initialize = class.initialize;
        let instance_id = self.exec.next_instance_id;
        self.exec.next_instance_id += 1;
        let object = ObjectRef::from_project_instance(
            instance_id,
            class_idx as i32,
            self.cur as i32,
            has_terminate,
            descriptor,
        );
        let value = Variant::from_object_ref(object);
        self.exec.programs[self.cur]
            .predeclared_singletons
            .insert(class_idx, value.clone());
        if let Some(init) = initialize
            && let Err(err) = self.run_proc_with_me(self.cur, init, value.clone(), &[], false)
        {
            let failed_identity = object_identity(&value);
            let slot_still_points_to_failed_instance = self.exec.programs[self.cur]
                .predeclared_singletons
                .get(&class_idx)
                .map(|current| object_identity(current) == failed_identity)
                .unwrap_or(false);
            if slot_still_points_to_failed_instance {
                self.exec.programs[self.cur]
                    .predeclared_singletons
                    .remove(&class_idx);
            }
            return Err(err);
        }
        Ok(value)
    }

    /// Assign the storage slot behind a `VB_PredeclaredId` class name. Real VBA treats
    /// `Set ClassName = Nothing` as clearing the default-instance slot, and a later
    /// `ClassName.Member` access creates a fresh default instance. Assigning a non-Nothing
    /// object replaces the slot with that object; the RHS has already been evaluated by the
    /// caller, preserving VBA's `New`-then-release-old ordering.
    fn set_predeclared_instance(
        &mut self,
        program_index: usize,
        class_idx: usize,
        value: Variant,
    ) -> Result<(), Vm3Error> {
        let program = self
            .exec
            .programs
            .get_mut(program_index)
            .ok_or_else(|| Vm3Error::Malformed(format!("unknown program {program_index}")))?;
        if class_idx >= program.program.classes.len() {
            return Err(Vm3Error::Malformed(format!("unknown class {class_idx}")));
        }
        if is_nothing(&value) {
            program.predeclared_singletons.remove(&class_idx);
        } else {
            program.predeclared_singletons.insert(class_idx, value);
        }
        Ok(())
    }

    /// Run a procedure to completion **synchronously** with `me` as its hidden first local —
    /// the lifecycle/event entry point (`Class_Initialize`/`Class_Terminate`/event handlers).
    ///
    /// vm3's dispatch is an explicit loop, so this pushes the callee frame and drives a NESTED
    /// `run_loop(base)` that returns when this frame returns. `run_loop` breaks at
    /// `frames.len() == base + 1` on a normal `Return` *without* popping, and leaves the
    /// faulting frame in place on an uncaught fault — so afterwards we restore the caller's
    /// error state (saved on the frame we pushed) and truncate back to `base` either way.
    /// `suppress` (used for `Class_Terminate`) swallows a raised VBA fault; a structural
    /// `Malformed` always propagates.
    fn run_proc_with_me(
        &mut self,
        target_prog: usize,
        proc: FuncId,
        me: Variant,
        args: &[OxArg],
        suppress: bool,
    ) -> Result<Variant, Vm3Error> {
        self.guard_call_depth()?;
        // Preserve the caller's program across the nested run_loop: it leaves `cur` at the
        // callee's program (run_loop re-derives cur per iteration), so the caller's post-call
        // resolves (store dst / extern_args) would otherwise use the wrong program. The caller
        // sets `cur` to the receiver's program (object dispatch / terminate) before calling.
        let saved_cur = self.cur;
        // Suppressed finalizers/callbacks must not overwrite the caller-visible Err object.
        let suppressed_err_engine = suppress.then(|| self.exec.err_engine.clone());
        let base = self.frames.len();
        let mut frame = self.new_frame_in(target_prog, proc);
        if let Some(slot) = frame.locals.get_mut(0) {
            *slot = me;
        }
        let mut pending_param_array_aliases = Vec::new();
        // Event-handler args follow `me` at locals 1.. (vm2's `run_proc_core` layout),
        // resolved against the CALLER (the still-current top frame) before the push.
        for (i, arg) in args.iter().enumerate() {
            let li = i + 1;
            match arg {
                OxArg::ByVal(op) => {
                    let param_array_aliases = self.param_array_aliases_for_operand(op);
                    let v = self.operand(op)?;
                    if let Some(slot) = frame.locals.get_mut(li) {
                        *slot = v;
                    }
                    if let Some(param_array_aliases) = param_array_aliases {
                        pending_param_array_aliases
                            .push((Loc::Local(base, li), param_array_aliases));
                    }
                }
                OxArg::ByRef(place) => {
                    let loc = self.resolve(place);
                    frame.aliases.insert(li, loc);
                }
                OxArg::Omitted => {
                    if let Some(slot) = frame.locals.get_mut(li) {
                        *slot = Variant::from_error_code(MISSING_ARG);
                    }
                }
            }
        }
        let saved_err = self.exec.err_engine.enter_activation();
        frame.saved_error_mode = saved_err.error_mode;
        frame.saved_active_error = saved_err.active_error;
        self.frames.push(frame);
        for (loc, aliases) in pending_param_array_aliases {
            self.param_array_aliases.insert(loc, aliases);
        }
        let result = self.run_loop(base);
        // The function result is the pushed frame's return local (the nested `run_loop` broke
        // at the frame's `Return` without copying it out). Capture it before unwinding.
        let ret = self
            .frames
            .get(base)
            .and_then(|fr| fr.return_local.and_then(|rl| fr.locals.get(rl.0).cloned()))
            .unwrap_or_else(Variant::empty);
        let saved_err = self.frames.get(base).map(|fr| SavedErrState {
            error_mode: fr.saved_error_mode,
            active_error: fr.saved_active_error,
        });
        if let Some(saved_err) = saved_err {
            self.exec.err_engine.restore(saved_err);
        }
        self.truncate_frames_with_withevents_cleanup(base);
        self.prune_param_array_aliases_from_depth(self.frames.len());
        self.cur = saved_cur;
        // Truncating released the lifecycle frame's object locals (and any an uncaught fault
        // left parked as it unwound) — run their `Class_Terminate`s now, the nested-epilogue /
        // fault-path drain that mirrors vm2 (re-entrant drains fold via the `draining` guard).
        self.maybe_drain();
        if let Some(saved) = suppressed_err_engine {
            self.exec.err_engine = saved;
        }
        match result {
            Ok(()) => Ok(ret),
            Err(Vm3Error::Fault(_)) if suppress => Ok(Variant::empty()),
            Err(e) => Err(e),
        }
    }

    /// Like [`Vm3::run_proc_with_me`] but seeds the arguments as already-evaluated **values**
    /// (a host-supplied `Vec<Variant>`) directly into `locals[1..]`, rather than resolving
    /// `OxArg` operands against a caller frame. Required for the host session API: an
    /// `OxArg::ByVal` carries only a scalar `OxConst`, so an object / array / arbitrary-Variant
    /// argument coming from the host cannot be expressed as an operand — it must be injected by
    /// value. `me` goes to local 0; all args are by-value (the host owns any ByRef itself).
    /// Returns the proc's result; with `suppress`, a `Fault` is swallowed to `Empty` (used by
    /// lifecycle/event callbacks). Wired by W3 (Collection dispatch), W5 (events) and W7
    /// (the host create/invoke session API).
    fn run_proc_with_values(
        &mut self,
        target_prog: usize,
        proc: FuncId,
        me: Variant,
        args: Vec<Variant>,
        suppress: bool,
    ) -> Result<Variant, Vm3Error> {
        self.guard_call_depth()?;
        // Preserve the caller's program across the nested run_loop: it leaves `cur` at the
        // callee's program (run_loop re-derives cur per iteration), so the caller's post-call
        // resolves (store dst / extern_args) would otherwise use the wrong program. The caller
        // sets `cur` to the receiver's program (object dispatch / terminate) before calling.
        let saved_cur = self.cur;
        // Suppressed finalizers/callbacks must not overwrite the caller-visible Err object.
        let suppressed_err_engine = suppress.then(|| self.exec.err_engine.clone());
        let base = self.frames.len();
        let mut frame = self.new_frame_in(target_prog, proc);
        if let Some(slot) = frame.locals.get_mut(0) {
            *slot = me;
        }
        // Host args follow `me` at locals 1.. as direct values — no caller-operand resolution,
        // no ByRef aliasing (the host marshals by value).
        for (i, v) in args.into_iter().enumerate() {
            if let Some(slot) = frame.locals.get_mut(i + 1) {
                *slot = v;
            }
        }
        let saved_err = self.exec.err_engine.enter_activation();
        frame.saved_error_mode = saved_err.error_mode;
        frame.saved_active_error = saved_err.active_error;
        self.frames.push(frame);
        let result = self.run_loop(base);
        // Capture the result from the pushed frame's return local before unwinding (the nested
        // `run_loop` broke at the frame's `Return` without copying it out) — same as
        // `run_proc_with_me`.
        let ret = self
            .frames
            .get(base)
            .and_then(|fr| fr.return_local.and_then(|rl| fr.locals.get(rl.0).cloned()))
            .unwrap_or_else(Variant::empty);
        let saved_err = self.frames.get(base).map(|fr| SavedErrState {
            error_mode: fr.saved_error_mode,
            active_error: fr.saved_active_error,
        });
        if let Some(saved_err) = saved_err {
            self.exec.err_engine.restore(saved_err);
        }
        self.truncate_frames_with_withevents_cleanup(base);
        self.prune_param_array_aliases_from_depth(self.frames.len());
        self.cur = saved_cur;
        self.maybe_drain();
        if let Some(saved) = suppressed_err_engine {
            self.exec.err_engine = saved;
        }
        match result {
            Ok(()) => Ok(ret),
            Err(Vm3Error::Fault(_)) if suppress => Ok(Variant::empty()),
            Err(e) => Err(e),
        }
    }

    /// Run any parked `Class_Terminate`s to a fixpoint. `Release` (an object's last reference
    /// dropping — a frame pop or a slot overwrite) parks a `has_terminate` instance on the
    /// shared `oxvba_runtime` termination queue; this dequeues and runs each `Class_Terminate`
    /// (with errors suppressed), pinned to statement boundaries / proc epilogue — exactly
    /// vm2's `maybe_drain`. The `draining` guard makes a re-entrant release (a Terminate that
    /// drops another object) fold into the same loop rather than nest.
    fn maybe_drain(&mut self) {
        if self.exec.draining {
            return;
        }
        self.exec.draining = true;
        while oxvba_runtime::has_pending_terminations() {
            for work in take_termination_batch(&self.exec) {
                // A terminating object's `Class_Terminate` lives in ITS OWN program (the one that
                // minted it), not necessarily the executing `cur`.
                if let (Some(proc), Some(object)) = (work.terminate, work.object) {
                    // Run the finalizer in the object's program (target_prog = bundle), so its
                    // globals/funcs resolve there. A fault in `Class_Terminate` is swallowed
                    // (suppress); a `Malformed` defect would still surface — drop it to match
                    // vm2's `let _ = …`.
                    let _ = self.run_proc_with_me(work.bundle, proc, object, &[], true);
                }
                oxvba_runtime::finish_pending_termination(work.instance_id);
                // Drop any `WithEvents` bindings + host (COM) subscriptions the terminated
                // instance owned (it can no longer sink events) — mirrors vm2's teardown.
                self.unsubscribe_com_owner(work.instance_id);
                self.exec
                    .events
                    .withevents
                    .retain(|key, _| withevents_owner_raw(*key) != work.instance_id);
            }
        }
        self.exec.draining = false;
    }

    /// Subscribe a `WithEvents` sink (`owner`) to a COM `source`'s events for `binding_token`:
    /// for each event the sink routes, advise the host's connection point (the shared, live-
    /// tested HAL `subscribe_event`) and record the subscription for dispatch + teardown.
    /// Mirrors vm2's `subscribe_com_events`.
    fn subscribe_com_events(
        &mut self,
        key: i64,
        binding_token: i32,
        owner: &Variant,
        source: &ObjectRef,
    ) {
        let routes: Vec<(i32, usize)> = self.exec.programs[self.cur]
            .event_routes
            .iter()
            .filter(|((binding, _), _)| *binding == binding_token)
            .map(|((_, event), handler)| (*event, *handler))
            .collect();
        for (event, handler) in routes {
            if let Ok(subscription) = self
                .exec
                .host
                .com()
                .subscribe_event(source.clone(), ComMemberToken::new(event))
            {
                self.exec.events.com_subscriptions.insert(
                    subscription.raw(),
                    ComEventSink {
                        owner: owner.clone(),
                        handler,
                    },
                );
                self.exec
                    .events
                    .com_subscriptions_by_key
                    .entry(key)
                    .or_default()
                    .push(subscription.raw());
            }
        }
    }

    /// Tear down every host (COM) subscription a `withevents` key holds (rebind / Set Nothing).
    fn unsubscribe_com_key(&mut self, key: i64) {
        if let Some(tokens) = self.exec.events.com_subscriptions_by_key.remove(&key) {
            for raw in tokens {
                let _ = self
                    .exec
                    .host
                    .com()
                    .unsubscribe_event_variant(ComSubscriptionToken::new(raw));
                self.exec.events.com_subscriptions.remove(&raw);
            }
        }
    }

    /// Tear down every host (COM) subscription owned by `owner_raw` (owner cleared / terminated).
    fn unsubscribe_com_owner(&mut self, owner_raw: i32) {
        let keys: Vec<i64> = self
            .exec
            .events
            .com_subscriptions_by_key
            .keys()
            .copied()
            .filter(|key| withevents_owner_raw(*key) == owner_raw)
            .collect();
        for key in keys {
            self.unsubscribe_com_key(key);
        }
    }

    /// Drain inbound host (COM) events: poll the host for delivered callbacks and dispatch each
    /// to the subscribed sink handler (run in the sink owner's program). Re-entrancy-guarded;
    /// handler faults are suppressed (events arrive out-of-band from the raiser). Mirrors vm2's
    /// `pump_com_events`; called at statement boundaries.
    fn pump_com_events(&mut self) {
        if self.exec.events.pumping {
            return;
        }
        self.exec.events.pumping = true;
        loop {
            let payload = match self.exec.host.com().poll_event_callback() {
                Ok(Some(payload)) => payload,
                // `Ok(None)` = nothing pending; `Err` = the host has no event delivery (the null
                // host) — either way, stop pumping.
                _ => break,
            };
            let sink = self
                .exec
                .events
                .com_subscriptions
                .get(&payload.subscription.raw())
                .map(|sink| (sink.owner.clone(), sink.handler));
            if let Some((owner, handler)) = sink {
                let values: Vec<Variant> = payload
                    .args
                    .iter()
                    .map(|arg| arg.variant().clone())
                    .collect();
                // Run the handler in the sink owner's program (its bundle_id), suppressing faults.
                let owner_bundle = owner
                    .as_object_ref()
                    .map(|o| o.bundle_id() as usize)
                    .unwrap_or(self.cur);
                let _ = <Self as ProcInvoker>::invoke_proc_with_values(
                    self,
                    owner_bundle,
                    FuncId(handler),
                    owner,
                    values,
                    true,
                );
            }
            let _ = self
                .exec
                .host
                .com()
                .release_event_callback_variant(payload.callback);
        }
        self.exec.events.pumping = false;
    }

    /// `TypeOf <object> Is <Type>`: for a project instance, match the bare type name against
    /// the instance's class name or any `Implements`ed interface; for a foreign/COM object,
    /// delegate to the host (unreachable until `CreateObject` lands in M3-8, but mirrors vm2).
    fn type_of_is(&mut self, object: &OxOperand, type_name: &str) -> Result<bool, Vm3Error> {
        let v = self.operand(object)?;
        // `TypeOf Nothing Is X` is False, not an error — and so is the same test on an unset
        // or `Set …= Nothing` object variable (a null Object Variant) and on `Empty`/`Null`.
        // Guard before `variant_to_object`, which would otherwise raise 91.
        if is_nothing(&v) {
            return Ok(false);
        }
        let obj = variant_to_object(&v)?;
        let bare = type_name.rsplit('.').next().unwrap_or(type_name);
        // A built-in `Collection` carries the reserved sentinel route key (it indexes no project
        // class), so it must be matched by name before the `classes` lookup.
        if obj.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return Ok(bare.eq_ignore_ascii_case("Collection"));
        }
        if obj.is_project_instance() {
            return Ok(self
                .exec
                .programs
                .get(obj.bundle_id() as usize)
                .and_then(|lp| lp.program.classes.get(obj.route_key() as usize))
                .is_some_and(|class| {
                    class.name.eq_ignore_ascii_case(bare)
                        || class
                            .implements
                            .iter()
                            .any(|i| i.eq_ignore_ascii_case(bare))
                }));
        }
        if let Ok(Some(name)) = self.exec.host.com().object_type_name(obj.clone())
            && (name.eq_ignore_ascii_case(type_name) || name.eq_ignore_ascii_case(bare))
        {
            return Ok(true);
        }
        Ok(false)
    }

    /// The class name of a project instance (else the host's COM name) — the
    /// `TypeName`-of-object resolution mirroring vm2's `object_type_name`.
    fn object_type_name(&self, object: &ObjectRef) -> Option<String> {
        // A built-in `Collection` (reserved sentinel route key) reports `TypeName` "Collection".
        if object.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return Some("Collection".to_string());
        }
        if object.is_project_instance() {
            return self
                .exec
                .programs
                .get(object.bundle_id() as usize)
                .and_then(|lp| lp.program.classes.get(object.route_key() as usize))
                .map(|c| c.name.clone());
        }
        self.exec
            .host
            .com()
            .object_type_name(object.clone())
            .ok()
            .flatten()
    }

    fn project_call_args(
        &self,
        target_prog: usize,
        proc: FuncId,
        args: &[OxCallArg],
    ) -> Result<Vec<OxArg>, Vm3Error> {
        let callee = self
            .exec
            .programs
            .get(target_prog)
            .and_then(|lp| lp.program.funcs.get(proc.0))
            .ok_or_else(|| Vm3Error::Malformed(format!("call to unknown proc {}", proc.0)))?;
        let param_names: Vec<String> = callee
            .locals
            .iter()
            .take(callee.param_count)
            .skip(hidden_me_receiver_param_count(callee))
            .map(|local| local.name.to_ascii_lowercase())
            .collect();
        let mut ordered: Vec<Option<OxArg>> = Vec::new();
        let mut next_positional = 0usize;
        for arg in args {
            let lowered = match arg {
                OxCallArg::ByRef(place) => OxArg::ByRef(*place),
                OxCallArg::Operand(op) => OxArg::ByVal(op.clone()),
                OxCallArg::Named { value, .. } => OxArg::ByVal(value.clone()),
                OxCallArg::Omitted => OxArg::Omitted,
                OxCallArg::Const(_) => {
                    return Err(Vm3Error::Malformed(
                        "a Const argument in a project method call".into(),
                    ));
                }
            };
            if let OxCallArg::Named { name, .. } = arg {
                let folded = name.to_ascii_lowercase();
                let Some(index) = param_names.iter().position(|param| param == &folded) else {
                    return Err(Vm3Error::Fault(Fault::new(
                        448,
                        format!("Named argument not found: {name}"),
                    )));
                };
                if ordered.len() <= index {
                    ordered.resize_with(index + 1, || None);
                }
                if ordered[index].is_some() {
                    return Err(Vm3Error::Fault(Fault::new(
                        448,
                        format!("Named argument not found: {name}"),
                    )));
                }
                ordered[index] = Some(lowered);
            } else {
                while ordered.get(next_positional).is_some_and(Option::is_some) {
                    next_positional += 1;
                }
                if ordered.len() <= next_positional {
                    ordered.resize_with(next_positional + 1, || None);
                }
                ordered[next_positional] = Some(lowered);
                next_positional += 1;
            }
        }
        Ok(ordered
            .into_iter()
            .map(|slot| slot.unwrap_or(OxArg::Omitted))
            .collect())
    }

    fn project_default_member_proc(
        &self,
        obj_bundle: usize,
        class_idx: usize,
        kind: ProjectMemberKind,
    ) -> Result<FuncId, Vm3Error> {
        let program = self
            .exec
            .programs
            .get(obj_bundle)
            .map(|lp| lp.program)
            .ok_or_else(|| {
                Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
            })?;
        let class = program.classes.get(class_idx).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
        })?;
        let exact = class
            .methods
            .iter()
            .find(|m| m.is_default_member && m.kind == kind);
        let member = exact.or_else(|| {
            if kind == ProjectMemberKind::PropertyGet {
                class
                    .methods
                    .iter()
                    .find(|m| m.is_default_member && m.kind == ProjectMemberKind::Method)
            } else if kind == ProjectMemberKind::Method {
                class
                    .methods
                    .iter()
                    .find(|m| m.is_default_member && m.kind == ProjectMemberKind::PropertyGet)
            } else {
                None
            }
        });
        member.map(|m| m.proc).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(438, "Object doesn't support default member"))
        })
    }

    fn dispatch_project_default_member(
        &mut self,
        object: ObjectRef,
        me: Variant,
        invoke_kind: TypeLibMemberInvokeKind,
        args: &[OxCallArg],
    ) -> Result<Variant, Vm3Error> {
        let kind = project_member_kind(invoke_kind);
        let class_idx = object.route_key() as usize;
        let obj_bundle = object.bundle_id() as usize;
        let proc = self.project_default_member_proc(obj_bundle, class_idx, kind)?;
        let proc_args = self.project_call_args(obj_bundle, proc, args)?;
        self.run_proc_with_me(obj_bundle, proc, me, &proc_args, false)
    }

    fn dispatch_project_default_member_values(
        &mut self,
        object: ObjectRef,
        me: Variant,
        invoke_kind: TypeLibMemberInvokeKind,
        args: Vec<Variant>,
    ) -> Result<Variant, Vm3Error> {
        let kind = project_member_kind(invoke_kind);
        let class_idx = object.route_key() as usize;
        let obj_bundle = object.bundle_id() as usize;
        let proc = self.project_default_member_proc(obj_bundle, class_idx, kind)?;
        self.run_proc_with_values(obj_bundle, proc, me, args, false)
    }

    /// Late-bound dispatch on a project instance: resolve the class member by name + accessor
    /// kind (with vm2's get↔method fallback) to its proc, then run it with `me` + the args and
    /// return the function result. Mirrors vm2's `dispatch_project_method` Name path
    /// (`ComCallLate` always names a member; the default-member/`obj(i)` path is M3-2/M3-8). A
    /// missing member is "Object doesn't support this member" (438).
    fn dispatch_project_method(
        &mut self,
        object: ObjectRef,
        me: Variant,
        name: &str,
        invoke_kind: TypeLibMemberInvokeKind,
        args: &[OxCallArg],
    ) -> Result<Variant, Vm3Error> {
        // A built-in `Collection` carries the reserved sentinel route key; its members dispatch
        // to the shared keyed-Collection logic over the box-owned `CollectionData`, not a
        // program class (W3). Checked BEFORE the `program.classes` lookup the sentinel can't index.
        if object.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return self.dispatch_collection_method(&object, name, args);
        }
        let kind = project_member_kind(invoke_kind);
        let class_idx = object.route_key() as usize;
        // The object's class lives in ITS OWN program (stamped into bundle_id when minted), which
        // may differ from the executing `cur` when a cross-project call passed it across a
        // program boundary. Resolve the member there, and run the body in that program.
        let obj_bundle = object.bundle_id() as usize;
        let program = self
            .exec
            .programs
            .get(obj_bundle)
            .map(|lp| lp.program)
            .ok_or_else(|| {
                Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
            })?;
        let class = program.classes.get(class_idx).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
        })?;
        let exact = class
            .methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name) && m.kind == kind);
        // A `PropertyGet` call with no args also resolves a same-named `Method`, and a
        // `Method` call resolves a same-named `PropertyGet` (vm2's accessor fallback).
        let member = exact.or_else(|| {
            if kind == ProjectMemberKind::PropertyGet && args.is_empty() {
                class.methods.iter().find(|m| {
                    m.name.eq_ignore_ascii_case(name) && m.kind == ProjectMemberKind::Method
                })
            } else if kind == ProjectMemberKind::Method {
                class.methods.iter().find(|m| {
                    m.name.eq_ignore_ascii_case(name) && m.kind == ProjectMemberKind::PropertyGet
                })
            } else {
                None
            }
        });
        let proc = member.map(|m| m.proc).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(438, format!("Object doesn't support '{name}'")))
        })?;
        // ByRef args alias the caller's place (write-back); named arguments reorder by
        // the callee parameter names before the frame is seeded.
        let proc_args = self.project_call_args(obj_bundle, proc, args)?;
        // Run the method body in the object's program (target_prog = obj_bundle). The args +
        // result dst are resolved by run_proc_with_me in THIS caller's program (cur unchanged),
        // so a method argument naming a caller global reads/writes the CALLER's global (vm2's
        // resolve-args-then-switch order), not the object program's.
        self.run_proc_with_me(obj_bundle, proc, me, &proc_args, false)
    }

    /// Mint a built-in library-class instance for `New <LibClass>` (`OxInst::NewExtern`). Only
    /// the `VBA.Collection` class is wired today: a native-backed object whose state rides the
    /// object box (`native_state`), so no `Class_Initialize` runs and a reserved sentinel route
    /// key marks it for the Collection dispatch leg.
    fn new_extern_instance(&mut self, import: ImportId) -> Result<Variant, Vm3Error> {
        let imp = self.cur_program().imports.get(import.0).ok_or_else(|| {
            Vm3Error::Malformed(format!("NewExtern names unknown import {}", import.0))
        })?;
        // A cross-PROJECT class `New OtherProj.Class`: resolve it by unit name + class token to a
        // class in the referenced program and mint a project instance THERE — its bundle_id,
        // leaked descriptor, and Class_Initialize, exactly as a local `New` would in that
        // program. The synthetic `VBA` unit is the built-in library path (Collection) below.
        if !imp.unit.eq_ignore_ascii_case("VBA") {
            // Mint in the referenced program (cur=B) so new_project_instance stamps B's
            // bundle_id + descriptor and runs Class_Initialize in B; restore the caller's
            // program after (run_proc_with_me already preserves cur across the init body).
            let (b, class_idx) = self.resolve_cross_project_class(import)?;
            let saved_cur = self.cur;
            self.cur = b;
            let result = self.new_project_instance(class_idx);
            self.cur = saved_cur;
            return result;
        }
        // Built-in library class (Collection): native-backed, reserved sentinel route key.
        let descriptor = self.resolve_extern_class(import)?;
        let instance_id = self.exec.next_instance_id;
        self.exec.next_instance_id += 1;
        let object = ObjectRef::from_project_instance(
            instance_id,
            VBA_COLLECTION_ROUTE_KEY,
            self.cur as i32,
            false,
            descriptor,
        );
        Ok(Variant::from_object_ref(object))
    }

    /// Resolve a cross-project class import (`New`/predeclared of `OtherProj.Class`) to the
    /// referenced program index + the class index within it (by unit name + class export token).
    fn resolve_cross_project_class(&self, import: ImportId) -> Result<(usize, usize), Vm3Error> {
        let imp = self.cur_program().imports.get(import.0).ok_or_else(|| {
            Vm3Error::Malformed(format!("NewExtern names unknown import {}", import.0))
        })?;
        let b = self.program_index_by_unit(&imp.unit).ok_or_else(|| {
            Vm3Error::Malformed(format!("unresolved reference to unit '{}'", imp.unit))
        })?;
        let class_idx = self.exec.programs[b]
            .program
            .exports
            .iter()
            .find(|e| e.token.matches(&imp.token))
            .ok_or_else(|| {
                Vm3Error::Malformed(format!(
                    "unit '{}' has no export matching the class import",
                    imp.unit
                ))
            })
            .and_then(|export| match export.target {
                oxvba_bundle::ExportTarget::Class(c) => Ok(c),
                _ => Err(Vm3Error::Malformed(
                    "a cross-project class import resolved to a non-class export".into(),
                )),
            })?;
        Ok((b, class_idx))
    }

    /// A cross-project `VB_PredeclaredId` singleton (`OtherProj.Class1` with no `New`): the
    /// `PredeclaredExtern` analogue of [`Self::new_extern_instance`] — get-or-create the
    /// singleton in the referenced program (cached there, with that program's bundle_id).
    fn predeclared_extern_instance(&mut self, import: ImportId) -> Result<Variant, Vm3Error> {
        let (b, class_idx) = self.resolve_cross_project_class(import)?;
        let saved_cur = self.cur;
        self.cur = b;
        let result = self.predeclared_instance(class_idx);
        self.cur = saved_cur;
        result
    }

    fn set_predeclared_extern_instance(
        &mut self,
        import: ImportId,
        value: Variant,
    ) -> Result<(), Vm3Error> {
        let (program_index, class_idx) = self.resolve_cross_project_class(import)?;
        self.set_predeclared_instance(program_index, class_idx, value)
    }

    /// Resolve a `New <LibClass>` import to its runtime QI descriptor. The `ExportToken::Class`
    /// analogue of [`Self::resolve_library_import`]. Only `VBA.Collection` is wired today;
    /// another VBA project needs the multi-`OxProgram` linker (deferred), and any other library
    /// class is a clean `Unimplemented` — never a silent mis-run.
    fn resolve_extern_class(
        &self,
        import: ImportId,
    ) -> Result<&'static RuntimeClassDescriptor, Vm3Error> {
        let imp = self.cur_program().imports.get(import.0).ok_or_else(|| {
            Vm3Error::Malformed(format!("NewExtern names unknown import {}", import.0))
        })?;
        if !imp.unit.eq_ignore_ascii_case("VBA") {
            return Err(Vm3Error::Unimplemented {
                what: "cross-project OxProgram link",
            });
        }
        let lib = oxvba_bundle::vba_library_bundle();
        let export = lib
            .exports
            .iter()
            .find(|e| e.token.matches(&imp.token))
            .ok_or_else(|| {
                Vm3Error::Malformed(format!(
                    "the VBA library bundle has no export matching NewExtern import {}",
                    import.0
                ))
            })?;
        let oxvba_bundle::ExportTarget::Class(class_idx) = export.target else {
            return Err(Vm3Error::Malformed(
                "a VBA library NewExtern resolved to a non-class export".into(),
            ));
        };
        let class = lib.classes.get(class_idx).ok_or_else(|| {
            Vm3Error::Malformed(format!("VBA library class {class_idx} out of range"))
        })?;
        if !class.name.eq_ignore_ascii_case("Collection") {
            return Err(Vm3Error::Unimplemented {
                what: "built-in library class other than Collection",
            });
        }
        Ok(&VBA_COLLECTION_DESCRIPTOR)
    }

    /// Dispatch a member call on a built-in `Collection` receiver to the shared keyed logic
    /// (`oxvba_eval::collection::dispatch_collection`) over the box-owned `CollectionData`. The
    /// member name resolves to its `NativeMethodId` via the VBA bundle's Collection class.
    fn dispatch_collection_method(
        &mut self,
        object: &ObjectRef,
        name: &str,
        args: &[OxCallArg],
    ) -> Result<Variant, Vm3Error> {
        let native = Self::vba_collection_native_method(name).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(
                438,
                format!("Collection doesn't support '{name}'"),
            ))
        })?;
        let method = match native {
            oxvba_bundle::NativeMethodId::CollectionAdd => CollectionMethod::Add,
            oxvba_bundle::NativeMethodId::CollectionItem => CollectionMethod::Item,
            oxvba_bundle::NativeMethodId::CollectionCount => CollectionMethod::Count,
            oxvba_bundle::NativeMethodId::CollectionRemove => CollectionMethod::Remove,
            oxvba_bundle::NativeMethodId::CollectionNewEnum => {
                return self.collection_new_enum_object(object);
            }
        };
        let argv = self.collection_args(args)?;
        object
            .with_native_collection(|data| dispatch_collection(method, data, &argv))
            .ok_or_else(|| Vm3Error::Fault(Fault::new(424, "Object required")))?
            .map_err(Self::collection_fault)
            .map_err(Vm3Error::Fault)
    }

    /// Marshal a Collection member's call args to by-value `Variant`s (omitted → `MISSING_ARG`,
    /// the sentinel the shared dispatcher recognises). The built-in Collection never writes
    /// back, so a ByRef arg is read by value.
    fn collection_args(&mut self, args: &[OxCallArg]) -> Result<Vec<Variant>, Vm3Error> {
        args.iter()
            .map(|a| match a {
                OxCallArg::Operand(op) => self.operand(op),
                OxCallArg::ByRef(place) => self.read(place),
                OxCallArg::Named { value, .. } => self.operand(value),
                OxCallArg::Omitted => Ok(Variant::from_error_code(MISSING_ARG)),
                OxCallArg::Const(_) => Err(Vm3Error::Malformed(
                    "a Const argument in a Collection method call".into(),
                )),
            })
            .collect()
    }

    /// Evaluate a list of index operands to by-value `Variant`s (a Collection default-member
    /// `c(i)` call's indices are plain operands, not `OxCallArg`s).
    fn operands_to_values(&mut self, ops: &[OxOperand]) -> Result<Vec<Variant>, Vm3Error> {
        ops.iter().map(|op| self.operand(op)).collect()
    }

    /// Resolve a `Collection` member name to its `NativeMethodId` via the VBA library bundle's
    /// Collection class — the single source of truth for the member set.
    fn vba_collection_native_method(member: &str) -> Option<oxvba_bundle::NativeMethodId> {
        let member = member.trim().trim_start_matches('[').trim_end_matches(']');
        let lib = oxvba_bundle::vba_library_bundle();
        let class = lib.classes.first()?;
        let m = class
            .methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(member))?;
        match lib.procedures.get(m.proc).and_then(|p| p.native) {
            Some(oxvba_bundle::NativeBody::Method(id)) => Some(id),
            _ => None,
        }
    }

    fn collection_new_enum_object(&mut self, object: &ObjectRef) -> Result<Variant, Vm3Error> {
        let snapshot = object
            .with_native_collection(|data| data.clone())
            .ok_or_else(|| Vm3Error::Fault(Fault::new(424, "Object required")))?;
        let instance_id = self.exec.next_instance_id;
        self.exec.next_instance_id += 1;
        let enumerator = ObjectRef::from_project_instance(
            instance_id,
            VBA_COLLECTION_ROUTE_KEY,
            object.bundle_id(),
            false,
            &VBA_COLLECTION_DESCRIPTOR,
        );
        enumerator
            .with_native_collection(|data| *data = snapshot)
            .ok_or_else(|| Vm3Error::Fault(Fault::new(424, "Object required")))?;
        Ok(Variant::from_object_ref(enumerator))
    }

    fn foreach_elements(&mut self, src: Variant, depth: usize) -> Result<Vec<Variant>, Vm3Error> {
        if depth > 8 {
            return Err(Vm3Error::Fault(Fault::new(
                438,
                "Object doesn't support this property or method",
            )));
        }
        if let Some(arr) = src.as_safearray() {
            return Ok(arr.variant_elements().unwrap_or_default());
        }
        let Some(obj) = src.as_object_ref() else {
            return Err(Vm3Error::Fault(Fault::new(
                13,
                "For Each can only iterate over a collection object or an array",
            )));
        };
        if let Some(values) = obj.native_collection_snapshot() {
            return Ok(values);
        }
        if obj.is_compat_object() && obj.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return Ok(Vec::new());
        }
        if obj.is_project_instance() {
            return self.project_class_enumerator_elements(obj, depth + 1);
        }
        self.exec
            .host
            .com()
            .enumerate_object(obj)
            .map_err(Fault::from_hal)
            .map_err(Vm3Error::Fault)
    }

    fn project_class_enumerator_elements(
        &mut self,
        object: ObjectRef,
        depth: usize,
    ) -> Result<Vec<Variant>, Vm3Error> {
        let class_idx = object.route_key() as usize;
        let obj_bundle = object.bundle_id() as usize;
        let program = self
            .exec
            .programs
            .get(obj_bundle)
            .map(|lp| lp.program)
            .ok_or_else(|| {
                Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
            })?;
        let class = program.classes.get(class_idx).ok_or_else(|| {
            Vm3Error::Fault(Fault::new(438, "Object doesn't support this member"))
        })?;
        let member = class
            .methods
            .iter()
            .find(|m| m.is_enumerator_member && m.kind == ProjectMemberKind::PropertyGet)
            .or_else(|| {
                class
                    .methods
                    .iter()
                    .find(|m| m.is_enumerator_member && m.kind == ProjectMemberKind::Method)
            })
            .ok_or_else(|| {
                Vm3Error::Fault(Fault::new(
                    438,
                    "Object doesn't support this property or method",
                ))
            })?;
        let result = self.run_proc_with_me(
            obj_bundle,
            member.proc,
            Variant::from_object_ref(object),
            &[],
            false,
        )?;
        self.foreach_elements(result, depth)
    }

    /// Map a shared `CollectionError` onto its VBA run-time error number (9 / 457 / 5 / 449) —
    /// the vm3 twin of vm2's `collection_fault`.
    fn collection_fault(err: CollectionError) -> Fault {
        match err {
            CollectionError::NotFound => Fault::new(9, "Subscript out of range"),
            CollectionError::DuplicateKey => Fault::new(
                457,
                "This key is already associated with an element of this collection",
            ),
            CollectionError::BadArgument => Fault::new(5, "Invalid procedure call or argument"),
            CollectionError::ArgNotOptional => Fault::new(449, "Argument not optional"),
        }
    }

    /// Late-bound by-name member dispatch shared by `ComCallLate` and `CallByName`: a
    /// project-instance receiver dispatches internally ([`Self::dispatch_project_method`]);
    /// a genuine `Object`/`Variant` (COM/foreign) receiver goes to the host's COM facet
    /// ([`Self::dispatch_com_method`]).
    fn dispatch_default_member(
        &mut self,
        recv_v: Variant,
        invoke_kind: TypeLibMemberInvokeKind,
        args: &[OxCallArg],
    ) -> Result<Variant, Vm3Error> {
        let object = variant_to_object(&recv_v)?;
        if object.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return self.dispatch_collection_method(&object, "Item", args);
        }
        if object.is_project_instance() {
            self.dispatch_project_default_member(object, recv_v, invoke_kind, args)
        } else {
            self.dispatch_com_method(
                object,
                DynamicMemberSelector::DefaultMember,
                invoke_kind,
                args,
            )
        }
    }

    fn dispatch_default_member_values(
        &mut self,
        recv_v: Variant,
        invoke_kind: TypeLibMemberInvokeKind,
        args: Vec<Variant>,
    ) -> Result<Variant, Vm3Error> {
        let object = variant_to_object(&recv_v)?;
        if object.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return self.dispatch_collection_values(&object, "Item", args);
        }
        if object.is_project_instance() {
            self.dispatch_project_default_member_values(object, recv_v, invoke_kind, args)
        } else {
            self.dispatch_com_method_values(
                object,
                DynamicMemberSelector::DefaultMember,
                invoke_kind,
                args,
            )
        }
    }

    fn dispatch_com_method_values(
        &mut self,
        object: ObjectRef,
        member: DynamicMemberSelector,
        invoke_kind: TypeLibMemberInvokeKind,
        args: Vec<Variant>,
    ) -> Result<Variant, Vm3Error> {
        let request = DynamicCallRequest {
            object,
            member,
            args: args
                .into_iter()
                .map(|value| DynamicCallArg {
                    value: Some(DynamicValue::from_variant(value)),
                    name: None,
                    by_ref: None,
                })
                .collect(),
            call_kind_hint: Some(invoke_kind_to_dynamic(invoke_kind)),
        };
        let (ret, _) = self
            .exec
            .host
            .com()
            .dispatch_invoke_dynamic_variant_with_writebacks(&request)
            .map_err(|e| Vm3Error::Fault(Fault::from_hal(e)))?;
        Ok(ret)
    }

    fn dispatch_member_by_name(
        &mut self,
        recv_v: Variant,
        name: &str,
        invoke_kind: TypeLibMemberInvokeKind,
        args: &[OxCallArg],
    ) -> Result<Variant, Vm3Error> {
        let object = variant_to_object(&recv_v)?;
        if object.is_project_instance() {
            self.dispatch_project_method(object, recv_v, name, invoke_kind, args)
        } else {
            self.dispatch_com_method(
                object,
                DynamicMemberSelector::Name(name.to_string()),
                invoke_kind,
                args,
            )
        }
    }

    /// COM dispatch on a genuine COM/foreign receiver: build a dynamic call request (the
    /// `member` selector is `Name` for late-bound and `TokenNamed{dispid,name}` for
    /// early-bound), route it through the host's COM facet (the identical
    /// `dispatch_invoke_dynamic_variant_with_writebacks` contract vm2 drives — the host's
    /// `PreferVtable` strategy picks the transport, the same way for both VMs), and copy any
    /// `[out]`/`[in,out]` write-back to its ByRef call-site place. A dispatch fault carries
    /// the rich VBA `Err.Number` recovered from the HRESULT/EXCEPINFO (via [`Fault::from_hal`],
    /// never the flatten-to-5 default — the must-fix the milestone calls out).
    fn dispatch_com_method(
        &mut self,
        object: ObjectRef,
        member: DynamicMemberSelector,
        invoke_kind: TypeLibMemberInvokeKind,
        args: &[OxCallArg],
    ) -> Result<Variant, Vm3Error> {
        let mut call_args = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let (value, arg_name) = match arg {
                OxCallArg::Operand(op) => (Some(self.operand(op)?), None),
                OxCallArg::ByRef(place) => (Some(self.read(place)?), None),
                OxCallArg::Named { name, value } => {
                    (Some(self.operand(value)?), Some(name.clone()))
                }
                OxCallArg::Omitted => (None, None),
                OxCallArg::Const(n) => (Some(Variant::from_i32(*n)), None),
            };
            call_args.push(DynamicCallArg {
                value: value.map(DynamicValue::from_variant),
                name: arg_name,
                by_ref: matches!(arg, OxCallArg::ByRef(_))
                    .then(|| RuntimeByRefSlot::new(index as u32, None)),
            });
        }
        let request = DynamicCallRequest {
            object,
            member,
            args: call_args,
            call_kind_hint: Some(invoke_kind_to_dynamic(invoke_kind)),
        };
        let (ret, writebacks) = self
            .exec
            .host
            .com()
            .dispatch_invoke_dynamic_variant_with_writebacks(&request)
            .map_err(|e| Vm3Error::Fault(Fault::from_hal(e)))?;
        // Apply COM `[out]`/`[in,out]` write-backs to the ByRef call-site places; only a
        // `ByRef` arg (marked from the typelib's param directions) is written back — a
        // force-ByVal `(x)` / non-l-value is lowered to `Operand` and correctly skipped.
        apply_optional_byref_writebacks(args, &writebacks, |place, value| {
            self.store(place, value)
        })?;
        Ok(ret)
    }

    /// Invoke a base-library built-in. Most builtins use the shared context-free
    /// `oxvba-lib` dispatcher, but a few are host/bundle-aware and the pure body would
    /// return a generically-wrong value: `TypeName` of an object yields the literal
    /// `"Object"` from the pure body, so resolve the real class/COM name here, where the
    /// host COM facet is in reach — never let the generic `"Object"` leak as a
    /// silently-wrong result.
    ///
    /// **This method is the intended `builtin_invoke` boundary of the future
    /// `RuntimeImports` ABI** (plan: M4). The Cranelift JIT does not re-implement builtins
    /// or this object-name special-case — it lowers `CallNative` to a `builtin_invoke`
    /// `extern "C"` shim that recovers `&mut Vm3` from its `ctx` and calls *this* method,
    /// so the interpreter and compiled code share one implementation and cannot drift.
    /// Keep its shape `(ctx, id, &[Variant]) -> Result<Variant, _>` ABI-friendly.
    ///
    /// Current VM3 hosts receive only shared HAL facets, never a VM/session callback.
    /// `StandardHostServices::DoEvents` pumps/marks queued work and returns; VM3 polls
    /// that queue from `StmtBoundary`. A custom host that stores and dereferences a
    /// `Vm3` raw pointer supplies separate unsafe authority not granted by
    /// `HostServices`; same-VM synchronous re-entry needs the future stable session
    /// root rather than pretending this safe API supports it.
    fn invoke_native_lib(
        &mut self,
        id: NativeImplId,
        argv: &[Variant],
    ) -> Result<Variant, Vm3Error> {
        self.invoke_native_lib_with_policy(id, argv, false)
    }

    fn invoke_native_lib_with_policy(
        &mut self,
        id: NativeImplId,
        argv: &[Variant],
        string_typed_alias: bool,
    ) -> Result<Variant, Vm3Error> {
        if string_typed_alias && argv.iter().any(|arg| arg.vtype() == VarType::Null) {
            return Err(Vm3Error::Fault(Fault::new(94, "invalid use of Null")));
        }
        if id == NativeImplId::ErrorText && argv.is_empty() {
            return Ok(Variant::from_string(
                self.exec.err_engine.err.description.clone(),
            ));
        }
        if id == NativeImplId::TypeName
            && let Some(object) = argv.first().and_then(|a| a.as_object_ref())
            && let Some(name) = self.object_type_name(&object)
        {
            // A project instance resolves its class name from the program's class table; a COM
            // object is named by the host. Only if neither names it do we fall through to the
            // pure body (which yields the generic "Object"), exactly as vm2 does.
            return Ok(Variant::from_string(name));
        }
        if oxvba_lib::is_contextual(id) {
            return oxvba_lib::invoke_contextual(id, argv, &mut self.exec.lib)
                .map_err(|e| Vm3Error::Fault(Fault::from_lib(e)));
        }

        match oxvba_lib::invoke_context_free(id, argv, self.exec.host) {
            Ok(value) => Ok(value),
            Err(oxvba_lib::ContextFreeInvokeError::Library(err)) => {
                Err(Vm3Error::Fault(Fault::from_lib(err)))
            }
            Err(oxvba_lib::ContextFreeInvokeError::ContextRequired) => Err(Vm3Error::Malformed(
                format!("contextual built-in {id:?} escaped VM3 dispatch classification"),
            )),
        }
    }

    /// Marshal a native built-in's arguments to plain values — a built-in reads the
    /// *value* of a ByRef argument (matching vm2's `native_args`), and an omitted one is
    /// `Empty`.
    fn native_args(&mut self, args: &[OxCallArg]) -> Result<Vec<Variant>, Vm3Error> {
        marshal_ox_call_args(
            args,
            |arg| match arg {
                MarshalArgRef::Operand(op) => self.operand(op),
                MarshalArgRef::ByRef(place) => self.read(place),
            },
            Variant::empty,
        )
    }

    /// Marshal a `Declare Lib` external call through the host's dynamic-link HAL — the
    /// identical `invoke_descriptor_variants` contract vm2 drives, so a `Declare` behaves
    /// the same whichever VM runs it (the differential gate shares one host). Captures
    /// `Err.LastDllError`, copies ByRef arguments' marshaled-back values to their caller
    /// slots, and applies the pointer-helper write-backs (`StrPtr(x)`/`VarPtr(x)` over an
    /// l-value reads the pinned buffer back into the source). Mirrors vm2's `declare_call`.
    fn declare_call(
        &mut self,
        descriptor_id: u32,
        args: &[OxCallArg],
        ptr_writebacks: &[oxvba_oxir::value::DeclarePtrWriteback],
    ) -> Result<Variant, Vm3Error> {
        // `cur_program()` returns `&'h OxProgram` (a reference independent of the `&mut self`
        // borrow), so the descriptor borrow does not conflict with the mutable calls below.
        let program = self.cur_program();
        let descriptor = program
            .external_calls
            .iter()
            .find(|d| d.descriptor_id == descriptor_id)
            .ok_or_else(|| {
                Vm3Error::Fault(Fault::new(
                    5,
                    format!("unknown Declare descriptor {descriptor_id}"),
                ))
            })?;
        let mut arg_variants = self.native_args(args)?;

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

        let _callback_regs =
            self.prepare_native_callback_args(descriptor, &param_type_strings, &mut arg_variants)?;

        // The pointer-helper pins this call feeds (the `LongLong`-carried registry addresses
        // of `StrPtr`/`VarPtr` args). A pin's life ends with the call it feeds — VBA's "the
        // pointer is valid for the duration of the call" contract — so free them once the
        // call returns and any write-back has read them back, keeping the registry bounded
        // across looping `Declare`s.
        let pin_addrs: Vec<i64> = arg_variants.iter().filter_map(Variant::as_i64).collect();
        let invoke = self
            .exec
            .host
            .dynlink()
            .invoke_descriptor_variants(&view, &arg_variants);
        // VBA refreshes `Err.LastDllError` after every `Declare` call (the OS last-error the
        // HAL captured immediately after the native call); non-native lanes report 0.
        self.exec.err_engine.last_dll_error = self.exec.host.dynlink().last_dll_error();
        let (ret, wb_values) = match invoke {
            Ok(pair) => pair,
            Err(err) => {
                pointer_helpers::free_pins(&pin_addrs);
                return Err(Vm3Error::Fault(Fault::from_hal(err)));
            }
        };
        // Copy each ByRef argument's marshaled-back value to its caller slot. The dynlink host
        // returns `wb_values` aligned to `args`; only `ByRef` args write back (a force-ByVal
        // `(x)` / non-l-value is lowered to `Operand`, so it is correctly left unchanged).
        apply_byref_writebacks(args, &wb_values, |place, value| self.store(place, value))?;
        // Pointer-helper write-back: a `StrPtr(x)`/`VarPtr(x)` argument over an l-value reads
        // the pinned buffer (the native call may have mutated it) back into the source
        // variable. The argument value is the registered pointer; the runtime registry
        // projects it back to a string / byte-array / scalar Variant.
        for wb in ptr_writebacks {
            let pointer = arg_variants
                .get(wb.arg_index)
                .and_then(Variant::as_i64)
                .unwrap_or(0);
            let value = match wb.kind {
                PtrWritebackKind::String => {
                    pointer_helpers::read_back_string_payload_variant(pointer)
                }
                PtrWritebackKind::ByteArray => {
                    pointer_helpers::read_back_byte_array_payload_variant(pointer)
                }
                PtrWritebackKind::Boolean => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Boolean,
                ),
                PtrWritebackKind::Byte => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Byte,
                ),
                PtrWritebackKind::Integer => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Integer,
                ),
                PtrWritebackKind::Long => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Long,
                ),
                PtrWritebackKind::LongLong => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::LongLong,
                ),
                PtrWritebackKind::LongPtr => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::LongPtr,
                ),
                PtrWritebackKind::Single => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Single,
                ),
                PtrWritebackKind::Double => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Double,
                ),
                PtrWritebackKind::Currency => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Currency,
                ),
                PtrWritebackKind::Date => pointer_helpers::read_back_scalar_payload_variant(
                    pointer,
                    pointer_helpers::ScalarPointerKind::Date,
                ),
            }
            .map_err(|e| Vm3Error::Fault(Fault::from_string(e)))?;
            self.store(&wb.target, value)?;
        }
        // The pins are fully consumed (the call ran and any write-back read them back);
        // release them so the registry stays bounded across looping `Declare`s.
        pointer_helpers::free_pins(&pin_addrs);
        Ok(ret)
    }

    fn prepare_native_callback_args(
        &mut self,
        descriptor: &oxvba_bundle::ExternalCallDescriptor,
        param_type_strings: &[String],
        arg_variants: &mut [Variant],
    ) -> Result<Vec<CallbackRegistration>, Vm3Error> {
        let mut registrations = Vec::new();
        for (index, value) in arg_variants.iter_mut().enumerate() {
            let Some(proc_token) = value.as_proc_ref() else {
                continue;
            };
            let is_long_ptr = param_type_strings.get(index).map(String::as_str) == Some("LongPtr");
            let by_ref = descriptor.param_by_ref.get(index).copied().unwrap_or(false);
            if !is_long_ptr || by_ref {
                return Err(Vm3Error::Unimplemented {
                    what: "AddressOf proc passed to an unsupported Declare parameter",
                });
            }
            if !Self::is_synchronous_native_callback_descriptor(descriptor) {
                return Err(Vm3Error::Unimplemented {
                    what: "AddressOf proc passed to a non-synchronous Declare callback parameter",
                });
            }
            let owner = self as *mut Self as usize;
            let executor = NonNull::from(&mut *self);
            // SAFETY: the returned registration is kept alive until the native Declare
            // call returns, and this VM remains the same-thread callback executor for
            // the bounded synchronous CallWindowProc callback shape.
            let registration = unsafe { register_callback(owner, proc_token, executor) }.map_err(
                |err| match err {
                    oxvba_runtime::CallbackThunkError::Exhausted => {
                        Vm3Error::Fault(Fault::new(7, err.to_string()))
                    }
                    oxvba_runtime::CallbackThunkError::UnsupportedPlatform => {
                        Vm3Error::Unimplemented {
                            what: "AddressOf native callback thunks on this platform",
                        }
                    }
                },
            )?;
            *value = Variant::from_i64(registration.address() as i64);
            registrations.push(registration);
        }
        Ok(registrations)
    }

    fn is_synchronous_native_callback_descriptor(
        descriptor: &oxvba_bundle::ExternalCallDescriptor,
    ) -> bool {
        let library = descriptor.library.to_ascii_lowercase();
        let symbol = descriptor.alias.to_ascii_lowercase();
        matches!(library.as_str(), "user32" | "user32.dll")
            && matches!(
                symbol.as_str(),
                "callwindowprocw" | "callwindowproca" | "callwindowproc"
            )
    }

    fn run_native_callback_proc(&mut self, proc_token: usize, raw_args: &[isize]) -> isize {
        let proc = FuncId(proc_token);
        let target_prog = self.cur;
        let Some(callee) = self
            .exec
            .programs
            .get(target_prog)
            .and_then(|program| program.program.funcs.get(proc.0))
        else {
            return 0;
        };
        let args: Vec<Variant> = callee
            .locals
            .iter()
            .take(callee.param_count)
            .zip(raw_args.iter().copied())
            .map(|(local, raw)| Self::callback_arg_for_type(raw, &local.ty))
            .collect();
        match <Self as ProcInvoker>::invoke_callback_proc_with_values(
            self,
            target_prog,
            proc,
            args,
            true,
        ) {
            Ok(ret) => Self::callback_return_to_isize(&ret),
            Err(_) => 0,
        }
    }

    fn callback_arg_for_type(raw: isize, ty: &OxTy) -> Variant {
        match ty {
            OxTy::Byte => Variant::from_u8(raw as u8),
            OxTy::Integer => Variant::from_i16(raw as i16),
            OxTy::Long => Variant::from_i32(raw as i32),
            OxTy::LongLong => Variant::from_i64(raw as i64),
            OxTy::Bool => Variant::from_bool(raw != 0),
            _ => Variant::from_i64(raw as i64),
        }
    }

    fn callback_return_to_isize(value: &Variant) -> isize {
        if let Some(value) = value.as_i64() {
            value as isize
        } else if let Some(value) = value.as_i32() {
            value as isize
        } else if let Some(value) = value.as_i16() {
            value as isize
        } else if let Some(value) = value.as_u8() {
            value as isize
        } else if let Some(value) = value.as_bool() {
            if value { -1 } else { 0 }
        } else {
            0
        }
    }

    fn run_callback_proc_with_values(
        &mut self,
        target_prog: usize,
        proc: FuncId,
        args: Vec<Variant>,
        suppress: bool,
    ) -> Result<Variant, Vm3Error> {
        self.guard_call_depth()?;
        let saved_cur = self.cur;
        // Suppressed native callback handlers must not overwrite the caller-visible Err object.
        let suppressed_err_engine = suppress.then(|| self.exec.err_engine.clone());
        let base = self.frames.len();
        let mut frame = self.new_frame_in(target_prog, proc);
        for (i, v) in args.into_iter().enumerate() {
            if let Some(slot) = frame.locals.get_mut(i) {
                *slot = v;
            }
        }
        let saved_err = self.exec.err_engine.enter_activation();
        frame.saved_error_mode = saved_err.error_mode;
        frame.saved_active_error = saved_err.active_error;
        self.frames.push(frame);
        let result = self.run_loop(base);
        let ret = self
            .frames
            .get(base)
            .and_then(|fr| fr.return_local.and_then(|rl| fr.locals.get(rl.0).cloned()))
            .unwrap_or_else(Variant::empty);
        let saved_err = self.frames.get(base).map(|fr| SavedErrState {
            error_mode: fr.saved_error_mode,
            active_error: fr.saved_active_error,
        });
        if let Some(saved_err) = saved_err {
            self.exec.err_engine.restore(saved_err);
        }
        self.truncate_frames_with_withevents_cleanup(base);
        self.prune_param_array_aliases_from_depth(self.frames.len());
        self.cur = saved_cur;
        self.maybe_drain();
        if let Some(saved) = suppressed_err_engine {
            self.exec.err_engine = saved;
        }
        match result {
            Ok(()) => Ok(ret),
            Err(Vm3Error::Fault(_)) if suppress => Ok(Variant::empty()),
            Err(e) => Err(e),
        }
    }

    /// Bound runaway recursion at vm2's frame ceiling, raising error 28 ("Out of stack
    /// space") as a fault, not a panic. The dispatch loop holds frames on the heap (no
    /// native recursion), so the same ceiling vm2 uses is reachable without overflow.
    fn guard_call_depth(&self) -> Result<(), Vm3Error> {
        const MAX_FRAMES: usize = 50_000;
        if self.frames.len() >= MAX_FRAMES {
            return Err(Vm3Error::Fault(Fault {
                code: 28,
                message: "Out of stack space".into(),
                source: None,
                help_file: None,
                help_context: None,
            }));
        }
        Ok(())
    }

    fn clear_statement_temps(&mut self, first_temp: usize) {
        let Some(frame_index) = self.frames.len().checked_sub(1) else {
            return;
        };
        if let Some(frame) = self.frames.get_mut(frame_index) {
            frame.temps.retain(|temp, _| *temp < first_temp);
        }
        self.for_each
            .retain(|loc, _| !is_cleared_temp(*loc, frame_index, first_temp));
        self.param_array_aliases.retain(|loc, aliases| {
            !is_cleared_temp(*loc, frame_index, first_temp)
                && aliases
                    .iter()
                    .flatten()
                    .all(|alias| !is_cleared_temp(*alias, frame_index, first_temp))
        });
    }

    /// Store the result of a fallible kernel op, raising its fault on error.
    fn store_arith(
        &mut self,
        dst: &OxPlace,
        out: Result<Variant, ArithError>,
    ) -> Result<(), Vm3Error> {
        let v = out.map_err(|e| Vm3Error::Fault(Fault::from_arith(e)))?;
        self.store(dst, v)
    }

    /// Resolve an [`OxPlace`] against the top frame to a concrete frame-stack [`Loc`],
    /// following a ByRef parameter's alias to its caller-side backing.
    fn resolve(&self, place: &OxPlace) -> Loc {
        let top = self.frames.len() - 1;
        match place {
            OxPlace::Global(g) => Loc::Global(self.cur, g.0),
            OxPlace::Local(l) => self.frames[top]
                .aliases
                .get(&l.0)
                .copied()
                .unwrap_or(Loc::Local(top, l.0)),
            OxPlace::Temp(t) => Loc::Temp(top, t.0),
        }
    }

    /// Read a resolved location. Local/Global are dense, program-sized tables, so an
    /// out-of-range index is a structural defect (`Malformed`), never a silent default;
    /// `Temp` absence is the SSA write-before-read contract (sparse map → `Empty`).
    fn read_loc(&self, loc: Loc) -> Result<Variant, Vm3Error> {
        match loc {
            Loc::Global(p, g) => self.exec.programs[p]
                .globals
                .get(g)
                .cloned()
                .ok_or_else(|| Vm3Error::Malformed(format!("global [{p}][{g}] out of range"))),
            Loc::Local(fi, li) => self
                .frames
                .get(fi)
                .and_then(|f| f.locals.get(li))
                .cloned()
                .ok_or_else(|| Vm3Error::Malformed(format!("local [{fi}][{li}] out of range"))),
            Loc::Temp(fi, ti) => Ok(self
                .frames
                .get(fi)
                .and_then(|f| f.temps.get(&ti))
                .cloned()
                .unwrap_or_else(Variant::empty)),
        }
    }

    /// Write a resolved location (same dense/sparse contract as [`Self::read_loc`]).
    fn write_loc(&mut self, loc: Loc, v: Variant) -> Result<(), Vm3Error> {
        self.param_array_aliases.remove(&loc);
        let old = match loc {
            Loc::Global(p, g) => {
                let slot = self.exec.programs[p].globals.get_mut(g).ok_or_else(|| {
                    Vm3Error::Malformed(format!("global [{p}][{g}] out of range"))
                })?;
                std::mem::replace(slot, v)
            }
            Loc::Local(fi, li) => {
                let slot = self
                    .frames
                    .get_mut(fi)
                    .and_then(|f| f.locals.get_mut(li))
                    .ok_or_else(|| {
                        Vm3Error::Malformed(format!("local [{fi}][{li}] out of range"))
                    })?;
                std::mem::replace(slot, v)
            }
            Loc::Temp(fi, ti) => {
                if let Some(f) = self.frames.get_mut(fi) {
                    f.temps.insert(ti, v).unwrap_or_else(Variant::empty)
                } else {
                    return Err(Vm3Error::Malformed(format!("temp frame {fi} out of range")));
                }
            }
        };
        self.clear_withevents_owners_before_releasing_values(std::iter::once(&old));
        drop(old);
        Ok(())
    }

    fn clear_withevents_owners_in_frame_before_drop(&mut self, frame: &Frame) {
        self.clear_withevents_owners_before_releasing_values(
            frame.locals.iter().chain(frame.temps.values()),
        );
    }

    fn truncate_frames_with_withevents_cleanup(&mut self, len: usize) {
        while self.frames.len() > len {
            let frame = self.frames.pop().expect("frame length checked before pop");
            self.clear_withevents_owners_in_frame_before_drop(&frame);
            drop(frame);
        }
    }

    fn clear_withevents_owners_before_releasing_values<'a>(
        &mut self,
        values: impl IntoIterator<Item = &'a Variant>,
    ) {
        let mut candidates: HashMap<i32, (ObjectRef, u32)> = HashMap::new();
        for value in values {
            let Some(owner) = value.as_object_ref() else {
                continue;
            };
            if !owner.is_project_instance() {
                continue;
            }
            let owner_raw = owner.raw();
            match candidates.entry(owner_raw) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert((owner, 1));
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().1 += 1;
                }
            }
        }

        for (owner_raw, (owner, releasing_refs)) in candidates {
            let event_binding_refs = self
                .exec
                .events
                .withevents
                .keys()
                .filter(|key| withevents_owner_raw(**key) == owner_raw)
                .count() as u32;
            if event_binding_refs == 0 {
                continue;
            }
            let com_sink_refs = self
                .exec
                .events
                .com_subscriptions
                .values()
                .filter(|sink| object_identity(&sink.owner) == owner_raw)
                .count() as u32;
            let retained_event_refs = event_binding_refs + com_sink_refs;
            // `owner` is one temporary AddRef taken while inspecting the values. If every
            // remaining non-temporary reference is either in `values` or in the event fabric,
            // break the event-owner cycle before dropping `values` so the object can terminate.
            if owner.strong_count() == retained_event_refs + releasing_refs + 1 {
                self.unsubscribe_com_owner(owner_raw);
                self.exec
                    .events
                    .withevents
                    .retain(|key, _| withevents_owner_raw(*key) != owner_raw);
            }
        }
    }

    /// Borrow a resolved location's `Variant` in place (no clone) — the constant-time
    /// counterpart to [`Self::read_loc`], used by the array fast paths to read an
    /// element without deep-cloning the whole backing store. A `Temp` that has not
    /// been written (SSA write-before-read) has no stable slot to borrow and yields
    /// `None`, so the caller falls back to the cloning path.
    fn read_loc_ref(&self, loc: Loc) -> Result<Option<&Variant>, Vm3Error> {
        match loc {
            Loc::Global(p, g) => self.exec.programs[p]
                .globals
                .get(g)
                .map(Some)
                .ok_or_else(|| Vm3Error::Malformed(format!("global [{p}][{g}] out of range"))),
            Loc::Local(fi, li) => self
                .frames
                .get(fi)
                .and_then(|f| f.locals.get(li))
                .map(Some)
                .ok_or_else(|| Vm3Error::Malformed(format!("local [{fi}][{li}] out of range"))),
            Loc::Temp(fi, ti) => Ok(self.frames.get(fi).and_then(|f| f.temps.get(&ti))),
        }
    }

    /// Mutably borrow a resolved location's `Variant` in place (no clone / writeback)
    /// — the constant-time counterpart to [`Self::write_loc`], used by the array
    /// write fast path to mutate one element through the SAFEARRAY descriptor. A
    /// `Temp` absent from the sparse map yields `None` (caller falls back).
    fn read_loc_mut(&mut self, loc: Loc) -> Result<Option<&mut Variant>, Vm3Error> {
        match loc {
            Loc::Global(p, g) => self.exec.programs[p]
                .globals
                .get_mut(g)
                .map(Some)
                .ok_or_else(|| Vm3Error::Malformed(format!("global [{p}][{g}] out of range"))),
            Loc::Local(fi, li) => self
                .frames
                .get_mut(fi)
                .and_then(|f| f.locals.get_mut(li))
                .map(Some)
                .ok_or_else(|| Vm3Error::Malformed(format!("local [{fi}][{li}] out of range"))),
            Loc::Temp(fi, ti) => Ok(self.frames.get_mut(fi).and_then(|f| f.temps.get_mut(&ti))),
        }
    }

    fn store(&mut self, place: &OxPlace, v: Variant) -> Result<(), Vm3Error> {
        let loc = self.resolve(place);
        self.write_loc(loc, v)
    }

    fn read(&mut self, place: &OxPlace) -> Result<Variant, Vm3Error> {
        let loc = self.resolve(place);
        self.read_loc_as_new(loc)
    }

    fn read_loc_as_new(&mut self, loc: Loc) -> Result<Variant, Vm3Error> {
        let value = self.read_loc(loc)?;
        let Some(binding) = self.as_new_slots.get(&loc).cloned() else {
            return Ok(value);
        };
        if !is_nothing(&value) {
            return Ok(value);
        }
        let object = self.instantiate_as_new(binding)?;
        self.write_loc(loc, object.clone())?;
        Ok(object)
    }

    fn instantiate_as_new(&mut self, binding: OxAsNew) -> Result<Variant, Vm3Error> {
        self.instantiate_as_new_in_bundle(self.cur, binding)
    }

    fn instantiate_as_new_in_bundle(
        &mut self,
        bundle: usize,
        binding: OxAsNew,
    ) -> Result<Variant, Vm3Error> {
        if bundle >= self.exec.programs.len() {
            return Err(Vm3Error::Malformed(format!(
                "unknown As New owner bundle {bundle}"
            )));
        }
        let saved = self.cur;
        self.cur = bundle;
        let result = match binding {
            OxAsNew::ProjectClass { class } => self.new_project_instance(class.0),
            OxAsNew::ExternClass { import } => self.new_extern_instance(import),
            OxAsNew::ComClass { prog_id } => {
                self.invoke_native_lib(NativeImplId::CreateObject, &[Variant::from_string(prog_id)])
            }
        };
        self.cur = saved;
        result
    }

    fn class_field_as_new_binding(&self, object: &ObjectRef, field: i32) -> Option<OxAsNew> {
        if !object.is_project_instance() || object.route_key() == VBA_COLLECTION_ROUTE_KEY {
            return None;
        }
        self.exec
            .programs
            .get(object.bundle_id() as usize)?
            .program
            .classes
            .get(object.route_key() as usize)?
            .as_new_fields
            .iter()
            .find(|candidate| candidate.field == field)
            .map(|candidate| candidate.binding.clone())
    }

    fn read_project_field_as_new(
        &mut self,
        instance: &ObjectRef,
        field: i32,
    ) -> Result<Variant, Vm3Error> {
        let value = instance
            .project_field_get(field)
            .unwrap_or_else(Variant::empty);
        let Some(binding) = self.class_field_as_new_binding(instance, field) else {
            return Ok(value);
        };
        if !is_nothing(&value) {
            return Ok(value);
        }
        let object = self.instantiate_as_new_in_bundle(instance.bundle_id() as usize, binding)?;
        instance.project_field_set(field, object.clone());
        Ok(object)
    }

    fn operand(&mut self, op: &OxOperand) -> Result<Variant, Vm3Error> {
        match op {
            OxOperand::Const(c) => Ok(const_variant(c)),
            OxOperand::Use(p) => self.read(p),
        }
    }
}

impl CallbackExecutor for Vm3<'_> {
    fn invoke_callback(&mut self, proc_token: usize, args: &[isize]) -> isize {
        self.run_native_callback_proc(proc_token, args)
    }
}

impl<'h> ProcInvoker for Vm3<'h> {
    type Error = Vm3Error;

    fn invoke_proc_with_values(
        &mut self,
        target_prog: usize,
        proc: FuncId,
        me: Variant,
        args: Vec<Variant>,
        suppress: bool,
    ) -> Result<Variant, Self::Error> {
        self.run_proc_with_values(target_prog, proc, me, args, suppress)
    }

    fn invoke_callback_proc_with_values(
        &mut self,
        target_prog: usize,
        proc: FuncId,
        args: Vec<Variant>,
        suppress: bool,
    ) -> Result<Variant, Self::Error> {
        self.run_callback_proc_with_values(target_prog, proc, args, suppress)
    }

    fn maybe_drain(&mut self) {
        Vm3::maybe_drain(self);
    }
}

/// The default VBA message for a run-time error code, used as `Err.Description` when a
/// raised error has no explicit Description.
fn default_error_message(code: i32) -> String {
    oxvba_runtime::default_error_message(code).to_string()
}

fn cmp_op(op: CmpOp) -> arith::CmpOp {
    match op {
        CmpOp::Eq => arith::CmpOp::Eq,
        CmpOp::Ne => arith::CmpOp::Ne,
        CmpOp::Lt => arith::CmpOp::Lt,
        CmpOp::Le => arith::CmpOp::Le,
        CmpOp::Gt => arith::CmpOp::Gt,
        CmpOp::Ge => arith::CmpOp::Ge,
    }
}

/// Wrap an `arith` coercion error as a routed vm3 fault (it carries its own VBA code).
fn arith_fault(e: ArithError) -> Vm3Error {
    Vm3Error::Fault(Fault::from_arith(e))
}

/// Narrow an evaluated array subscript/bound to a VBA `Long` (i32). VBA array
/// subscripts and `ReDim` bounds are `Long`, so a value outside `Long` range
/// raises Overflow (6) — the arithmetic layer yields the exact `i64`, and a
/// bare `as i32` would silently wrap it (e.g. `a(4294967296#)` reading `a(0)`
/// or `ReDim a(4294967296)` allocating a single element).
fn subscript_to_long(v: i64) -> Result<i32, Vm3Error> {
    i32::try_from(v).map_err(|_| arith_fault(ArithError::overflow()))
}

/// Backing-store ceiling for an element-wise (Variant/String/Object/record)
/// array. Each such element is a `Variant` slot built individually, so a
/// Long-range element count can demand tens of gigabytes and either abort the
/// host on an infallible allocation or, on an over-committing OS where a huge
/// reservation "succeeds", stall/OOM-kill it while the elements are built. VBA
/// itself raises Out of memory (7) for such arrays on ordinary machines, so we
/// reject clearly-pathological sizes up front. The limit is deliberately far
/// above any realistic in-memory VBA array (~350M `Variant` slots).
const MAX_ELEMENTWISE_ARRAY_BYTES: usize = 8 * 1024 * 1024 * 1024;

/// Build `count` default elements for a (re)dimensioned array. A
/// `map(..).collect()` reserves capacity infallibly, so a large `ReDim` of a
/// Variant/String/Object/record element (e.g. `ReDim v(0 To 2000000000)`) would
/// abort the whole host process. A pathological size is rejected up front, and
/// the remaining allocation is reserved fallibly, so both surface as a catchable
/// Out of memory (7), matching VBA. (Scalar numeric elements take the separate
/// zeroed-SAFEARRAY path, which is a single fallible buffer allocation.)
fn try_build_default_elements(
    element: &ArrayElementType,
    count: usize,
) -> Result<Vec<Variant>, Vm3Error> {
    let bytes = count.saturating_mul(std::mem::size_of::<Variant>());
    if bytes > MAX_ELEMENTWISE_ARRAY_BYTES {
        return Err(Vm3Error::Fault(Fault::new(7, "Out of memory")));
    }
    let mut out: Vec<Variant> = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| Vm3Error::Fault(Fault::new(7, "Out of memory")))?;
    for _ in 0..count {
        out.push(default_array_element(element).map_err(|e| Vm3Error::Fault(Fault::new(13, e)))?);
    }
    Ok(out)
}

/// Map an old flat element index to its position in a `ReDim Preserve`'d shape, preserving
/// the element's absolute n-dimensional coordinate (C-order, first dimension outermost — the
/// same convention [`Vm3::flat_index`] uses). Returns `None` when the coordinate falls outside
/// the new bounds (a shrunk dimension drops those elements). Both bound slices have equal rank
/// (the caller enforces the VBA `ReDim Preserve` rule before calling).
fn remap_preserve_index(
    old_flat: usize,
    old_bounds: &[SafeArrayBound],
    new_bounds: &[SafeArrayBound],
) -> Option<usize> {
    let rank = old_bounds.len();
    // Decode the C-order flat index into per-dimension offsets (last dimension fastest).
    let mut offsets = vec![0usize; rank];
    let mut rem = old_flat;
    for d in (0..rank).rev() {
        let c = old_bounds[d].count as usize;
        if c == 0 {
            return None;
        }
        offsets[d] = rem % c;
        rem /= c;
    }
    // Re-encode the same absolute coordinate against the new bounds.
    let mut new_flat = 0usize;
    for d in 0..rank {
        let new_count = new_bounds[d].count as i64;
        let abs = i64::from(old_bounds[d].lower) + offsets[d] as i64;
        let new_off = abs - i64::from(new_bounds[d].lower);
        if new_off < 0 || new_off >= new_count {
            return None;
        }
        new_flat = new_flat * new_count as usize + new_off as usize;
    }
    Some(new_flat)
}

/// VBA `Nothing`/empty test (mirrors vm2): a null object reference, `Empty`/`Null`, or a
/// numeric zero (the literal-0-as-Nothing representation). Used by `Let`/`Set` validation.
fn is_nothing(value: &Variant) -> bool {
    match value.vtype() {
        VarType::Object => value.as_object_ref().map(|o| o.raw()).unwrap_or(0) == 0,
        VarType::Empty | VarType::Null => true,
        _ => value.as_i16() == Some(0) || value.as_i32() == Some(0),
    }
}

/// The raw identity (an `i32`) of an object value, or 0 for a non-object/`Nothing`.
fn object_identity(value: &Variant) -> i32 {
    value.as_object_ref().map(|o| o.raw()).unwrap_or(0)
}

/// VBA's `Is` operator accepts only object references at run time. A Variant
/// that currently holds a scalar compiles, but evaluating the comparison raises
/// error 424 instead of treating the scalar as the null object identity.
fn object_identity_for_is(value: &Variant) -> Result<i32, Vm3Error> {
    if value.vtype() == VarType::Object {
        Ok(object_identity(value))
    } else {
        Err(Vm3Error::Fault(Fault::new(424, "Object required")))
    }
}

/// The `withevents` map key: the sink owner's identity in the high 32 bits, the binding token
/// in the low 32 (mirrors vm2). One sink can hold several `WithEvents` sources, one per token.
fn withevents_key(owner: &ObjectRef, binding: i64) -> i64 {
    (i64::from(owner.raw()) << 32) | (binding & 0xFFFF_FFFF)
}
/// The sink owner identity recovered from a `withevents` key.
fn withevents_owner_raw(key: i64) -> i32 {
    (key >> 32) as i32
}
/// The binding token recovered from a `withevents` key.
fn withevents_binding(key: i64) -> i64 {
    key & 0xFFFF_FFFF
}

/// The project accessor kind a late-bound call's COM invoke-kind selects (the inverse of the
/// elaboration's `invoke_kind_from_member_kind`).
fn project_member_kind(k: TypeLibMemberInvokeKind) -> ProjectMemberKind {
    match k {
        TypeLibMemberInvokeKind::PropertyGet => ProjectMemberKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyPut => ProjectMemberKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPutRef => ProjectMemberKind::PropertySet,
        TypeLibMemberInvokeKind::Method => ProjectMemberKind::Method,
    }
}

/// The dynamic COM dispatch kind (the late-bound `IDispatch::Invoke` flag hint) a call-site
/// accessor selects. Mirrors vm2's `member_kind_to_dynamic` composed with the invoke-kind→
/// member-kind mapping: `Put`→`Let`, `PutRef`→`Set`.
fn invoke_kind_to_dynamic(k: TypeLibMemberInvokeKind) -> DynamicCallKind {
    match k {
        TypeLibMemberInvokeKind::Method => DynamicCallKind::Method,
        TypeLibMemberInvokeKind::PropertyGet => DynamicCallKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyPut => DynamicCallKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPutRef => DynamicCallKind::PropertySet,
    }
}

/// Coerce a value to an object reference (mirrors vm2's `variant_to_object`): an unset object
/// reference (`Object`/`Empty`/`Null` with no instance) is "Object variable not set" (91),
/// distinct from a non-object value (424); a bare integer is a legacy compat-identity handle.
fn variant_to_object(value: &Variant) -> Result<ObjectRef, Vm3Error> {
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    if matches!(
        value.vtype(),
        VarType::Object | VarType::Empty | VarType::Null
    ) {
        return Err(Vm3Error::Fault(Fault::new(
            91,
            "Object variable or With block variable not set",
        )));
    }
    if let Some(raw) = value.as_i16().map(i32::from).or_else(|| value.as_i32()) {
        return Ok(ObjectRef::from_compat_identity(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ObjectRef::from_compat_identity)
            .map_err(|_| Vm3Error::Fault(Fault::new(13, "object handle exceeds i32 range")));
    }
    Err(Vm3Error::Fault(Fault::new(424, "Object required")))
}

fn const_variant(c: &OxConst) -> Variant {
    match c {
        OxConst::Empty => Variant::empty(),
        OxConst::Null => Variant::null(),
        OxConst::Nothing => Variant::nothing(),
        OxConst::Bool(b) => Variant::from_bool(*b),
        OxConst::I16(n) => Variant::from_i16(*n),
        OxConst::I32(n) => Variant::from_i32(*n),
        OxConst::I64(n) => Variant::from_i64(*n),
        OxConst::F32(bits) => Variant::from_f32(f32::from_bits(*bits)),
        OxConst::F64(bits) => Variant::from_f64(f64::from_bits(*bits)),
        OxConst::Currency(scaled) => Variant::from_currency_scaled_i64(*scaled),
        OxConst::Date(bits) => Variant::from_date_f64(f64::from_bits(*bits)),
        OxConst::Str(s) => Variant::from_string(s.clone()),
    }
}

fn variant_matches_ox_ty(value: &Variant, ty: &OxTy) -> bool {
    match ty {
        OxTy::Variant => true,
        OxTy::Bool => value.vtype() == VarType::Boolean,
        OxTy::Byte => value.vtype() == VarType::Byte,
        OxTy::Integer => value.vtype() == VarType::Integer,
        OxTy::Long => value.vtype() == VarType::Long,
        OxTy::LongLong => value.vtype() == VarType::LongLong,
        OxTy::Single => value.vtype() == VarType::Single,
        OxTy::Double => value.vtype() == VarType::Double,
        OxTy::Currency => value.vtype() == VarType::Currency,
        OxTy::Date => value.vtype() == VarType::Date,
        OxTy::Decimal => value.vtype() == VarType::Decimal,
        OxTy::Str | OxTy::FixedStr(_) => value.vtype() == VarType::String,
        OxTy::Object(_) => value.vtype() == VarType::Object,
        OxTy::Record(_) => value.vtype() == VarType::Record,
        OxTy::Array(_, _) => value.vtype() == VarType::ArrayVariant,
        OxTy::ProcRef => value.vtype() == VarType::ProcRef,
    }
}

/// A short label for an instruction kind (for the `Unimplemented` message).
fn inst_kind(inst: &OxInst) -> &'static str {
    match inst {
        OxInst::CallProc { .. } => "CallProc",
        OxInst::CallProcRef { .. } => "CallProcRef",
        OxInst::CallExtern { .. } => "CallExtern",
        OxInst::CallNative { .. } => "CallNative (builtin/Declare)",
        OxInst::CallByName { .. } => "CallByName",
        OxInst::ComCallEarly { .. } => "ComCallEarly",
        OxInst::ComCallLate { .. } => "ComCallLate",
        OxInst::Box { .. } => "Box",
        OxInst::Unbox { .. } => "Unbox",
        OxInst::ValidateAssignment { .. } => "ValidateAssignment",
        OxInst::LoadProcRef { .. } => "LoadProcRef",
        OxInst::CompareObjectIs { .. } => "CompareObjectIs",
        OxInst::TypeOfIs { .. } => "TypeOfIs",
        OxInst::NewObject { .. } | OxInst::NewExtern { .. } => "New",
        OxInst::Predeclared { .. }
        | OxInst::PredeclaredExtern { .. }
        | OxInst::PredeclaredSet { .. }
        | OxInst::PredeclaredExternSet { .. } => "Predeclared",
        OxInst::NewRecord { .. } => "NewRecord",
        OxInst::FieldGet { .. } | OxInst::FieldSet { .. } => "object field access",
        OxInst::RecordGet { .. }
        | OxInst::RecordSet { .. }
        | OxInst::RecordLSet { .. }
        | OxInst::RecordArrayGet { .. }
        | OxInst::RecordArraySet { .. } => "record field access",
        OxInst::ArrayLiteral { .. }
        | OxInst::ArrayAppend { .. }
        | OxInst::ArrayRedim { .. }
        | OxInst::ArrayGet { .. }
        | OxInst::ArraySet { .. }
        | OxInst::ArrayErase { .. }
        | OxInst::Bound { .. } => "array op",
        OxInst::ForEachInit { .. } | OxInst::ForEachNext { .. } => "For Each",
        OxInst::WithEventsGet { .. }
        | OxInst::WithEventsSet { .. }
        | OxInst::WithEventsClearOwner { .. }
        | OxInst::WithEventsFirstOwner { .. }
        | OxInst::WithEventsNextOwner { .. } => "WithEvents",
        OxInst::RaiseEvent { .. } => "RaiseEvent",
        OxInst::Ptr { .. } => "pointer helper",
        OxInst::ErrFieldGet { .. } => "Err field read",
        OxInst::ErlGet { .. } => "Erl read",
        OxInst::ErrFieldSet { .. } => "Err field write",
        OxInst::SetErrorHandler(_) => "On Error",
        OxInst::SetLineNumber { .. } => "line number label",
        OxInst::AddRef { .. } | OxInst::Release { .. } => "refcount effect",
        OxInst::DrainTerminations => "DrainTerminations",
        // The handled instructions never reach here.
        _ => "instruction",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    };

    use oxvba_bundle::coreir::{
        ClassId as CoreClassId, CoreArg, CoreAsNew, CoreBinOp, CoreCallee, CoreClass,
        CoreClassAsNewField, CoreClassField, CoreClassMethod, CoreConst, CoreGlobal, CoreLocal,
        CoreParam, CorePlace, CoreProc, CoreProgram, CoreStmt, CoreValue, ErrorOp, ExitKind,
        LabelId, LocalId as CoreLocalId, ProcId, PtrWriteback,
    };
    use oxvba_bundle::{
        AssignmentIntent, AssignmentTargetKind, BuiltinType, DeclareParamType,
        ExternalCallDescriptor, NativeImplId, NumericCoerceTarget, NumericMode, ProcedureKind,
        ProjectMemberKind, StringCompareMode, VarTypeRef,
    };
    use oxvba_com::{ComCallbackPayload, ComCallbackToken, ComMemberToken, ComSubscriptionToken};
    use oxvba_hal::HostPolicy;
    use oxvba_hal::adapters::null::NullHostServices;
    use oxvba_hal::error::{HalError, HalResult};
    use oxvba_hal::model::{CapabilityId, HalDescriptor, HalProfileId};
    use oxvba_hal::traits::{
        ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal,
        ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
    };
    use oxvba_oxir::program::{OxFunc, OxLocal, OxParamInfo};
    use oxvba_oxir::ty::OxTy;
    use oxvba_runtime::DynLinkSymbol;
    use oxvba_runtime::object_ref::{
        RUNTIME_E_NOINTERFACE, RawRuntimeIUnknown, RawRuntimeIUnknownVtbl, RuntimeGuid,
    };
    use oxvba_runtime::variant::VarType;

    #[test]
    fn subscript_to_long_overflows_beyond_long_range() {
        // In-range Long values pass through unchanged.
        assert_eq!(subscript_to_long(0).unwrap(), 0);
        assert_eq!(subscript_to_long(i64::from(i32::MAX)).unwrap(), i32::MAX);
        assert_eq!(subscript_to_long(i64::from(i32::MIN)).unwrap(), i32::MIN);
        // Anything outside Long range is Overflow (6), never a silent wrap.
        for v in [
            i64::from(i32::MAX) + 1,
            4_294_967_296, // 2^32 used to wrap to 0
            4_294_967_299, // 2^32 + 3 used to wrap to 3
            i64::from(i32::MIN) - 1,
            i64::MAX,
            i64::MIN,
        ] {
            match subscript_to_long(v) {
                Err(Vm3Error::Fault(f)) => assert_eq!(f.code, 6, "value {v} should overflow"),
                other => panic!("expected Overflow(6) for {v}, got {other:?}"),
            }
        }
    }

    #[test]
    fn try_build_default_elements_reports_oom_not_abort() {
        // A count whose backing store cannot be allocated must surface a
        // catchable Out of memory (7) rather than an infallible allocation that
        // aborts the host. `usize::MAX` overflows the byte size deterministically
        // on every platform without attempting a real large allocation.
        match try_build_default_elements(&ArrayElementType::Variant, usize::MAX) {
            Err(Vm3Error::Fault(f)) => assert_eq!(f.code, 7),
            other => panic!("expected Out of memory (7), got {other:?}"),
        }
        // A small count still succeeds and yields the requested number of defaults.
        let ok = try_build_default_elements(&ArrayElementType::Long, 4).unwrap();
        assert_eq!(ok.len(), 4);
    }

    /// Bind-free: hand-build a `CoreProgram`, elaborate it to OxIR, run it on vm3, and
    /// read back a snapshot slot.
    fn run_core(prog: &CoreProgram) -> Vm3<'_> {
        // Leak the elaborated program so the returned VM can borrow it for the test.
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        Vm3::run(oxp, host).expect("vm3 run")
    }

    fn assign(place: CorePlace, value: CoreValue) -> CoreStmt {
        CoreStmt::Assign {
            place,
            value,
            intent: AssignmentIntent::Let,
            target_kind: AssignmentTargetKind::Scalar,
            target_name: String::new(),
            target_type_name: String::new(),
        }
    }

    fn local(name: &str, ty: VarTypeRef) -> CoreLocal {
        CoreLocal {
            name: name.into(),
            ty,
            array_element: None,
        }
    }

    fn main_proc(locals: Vec<CoreLocal>, body: Vec<CoreStmt>) -> CoreProgram {
        CoreProgram {
            long_ptr_width: Default::default(),
            procs: vec![CoreProc {
                name: "Main".into(),
                kind: ProcedureKind::Sub,
                params: Vec::new(),
                locals,
                return_local: None,
                label_lines: Vec::new(),
                body,
            }],
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            ..Default::default()
        }
    }

    /// A multi-proc program whose `procs[0]` is the entry (`Main`).
    fn procs_program(procs: Vec<CoreProc>) -> CoreProgram {
        CoreProgram {
            long_ptr_width: Default::default(),
            procs,
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            ..Default::default()
        }
    }

    fn proc(
        name: &str,
        kind: ProcedureKind,
        params: Vec<CoreParam>,
        locals: Vec<CoreLocal>,
        return_local: Option<CoreLocalId>,
        body: Vec<CoreStmt>,
    ) -> CoreProc {
        CoreProc {
            name: name.into(),
            kind,
            params,
            locals,
            return_local,
            label_lines: Vec::new(),
            body,
        }
    }

    fn long_param(name: &str) -> CoreParam {
        CoreParam {
            name: name.into(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            optional: false,
            by_ref: true,
            variadic: false,
        }
    }

    fn byval_long_param(name: &str) -> CoreParam {
        CoreParam {
            name: name.into(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            optional: false,
            by_ref: false,
            variadic: false,
        }
    }

    fn optional_long_param(name: &str) -> CoreParam {
        CoreParam {
            name: name.into(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            optional: true,
            by_ref: true,
            variadic: false,
        }
    }

    fn ox_param_local(name: &str) -> OxLocal {
        OxLocal {
            name: name.into(),
            ty: OxTy::Long,
            array_element: None,
            param: Some(OxParamInfo {
                optional: false,
                by_ref: false,
                variadic: false,
            }),
            escaped: false,
        }
    }

    fn ox_func_with_params(name: &str, locals: Vec<OxLocal>, param_count: usize) -> OxFunc {
        OxFunc {
            name: name.into(),
            kind: ProcedureKind::Sub,
            locals,
            temps: Vec::new(),
            param_count,
            return_local: None,
            blocks: Vec::new(),
            entry: BlockId(0),
        }
    }

    #[test]
    fn runtime_member_params_skips_only_explicit_hidden_me_receiver() {
        let with_me = ox_func_with_params(
            "Touch",
            vec![ox_param_local("Me"), ox_param_local("value")],
            2,
        );
        let params = runtime_member_params(&with_me);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "value");

        let without_me = ox_func_with_params("Touch", vec![ox_param_local("value")], 1);
        let params = runtime_member_params(&without_me);
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "value");

        let mut non_param_me = ox_param_local("Me");
        non_param_me.param = None;
        let params = runtime_member_params(&ox_func_with_params("Touch", vec![non_param_me], 1));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "Me");
    }

    #[test]
    fn project_member_named_args_skip_only_explicit_hidden_me_receiver() {
        let twice = proc(
            "Twice",
            ProcedureKind::Function,
            vec![long_param("me"), long_param("x")],
            vec![local("Twice", VarTypeRef::Builtin(BuiltinType::Long))],
            Some(CoreLocalId(2)),
            Vec::new(),
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        );
        let prog = CoreProgram {
            long_ptr_width: Default::default(),
            procs: vec![main, twice],
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            classes: vec![CoreClass {
                name: "Widget".into(),
                predeclared: false,
                initialize: None,
                terminate: None,
                fields: Vec::new(),
                methods: vec![CoreClassMethod {
                    name: "Twice".into(),
                    kind: ProjectMemberKind::Method,
                    proc: ProcId(1),
                    dispid: None,
                    vtable_slot: None,
                    is_default_member: false,
                    is_enumerator_member: false,
                }],
                as_new_fields: Vec::new(),
                implements: Vec::new(),
            }],
            ..Default::default()
        };
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(&prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        let vm = Vm3::activate(oxp, host).expect("activate");
        let args = vm
            .project_call_args(
                0,
                FuncId(1),
                &[OxCallArg::Named {
                    name: "x".into(),
                    value: OxOperand::Const(OxConst::I32(21)),
                }],
            )
            .expect("named arg should map to visible parameter after Me");
        assert_eq!(args.len(), 1);
        assert!(matches!(
            &args[0],
            OxArg::ByVal(OxOperand::Const(OxConst::I32(21)))
        ));
    }

    /// `CorePlace::Local(i)`.
    fn lc(i: usize) -> CorePlace {
        CorePlace::Local(CoreLocalId(i))
    }
    /// `CoreValue::Load(Local(i))`.
    fn load(i: usize) -> CoreValue {
        CoreValue::Load(lc(i))
    }
    /// A `Checked(Long)` addition (the regime the binder picks for `Long` operands).
    fn long_add(l: CoreValue, r: CoreValue) -> CoreValue {
        CoreValue::Binary {
            op: CoreBinOp::Add,
            lhs: Box::new(l),
            rhs: Box::new(r),
            mode: StringCompareMode::Binary,
            num: NumericMode::Checked(NumericCoerceTarget::Long),
        }
    }

    #[test]
    fn checked_long_arithmetic_matches() {
        // Sub Main(): n = (10 + 5) * 2  →  n is Long 30.
        let n = || CorePlace::Local(CoreLocalId(0));
        let long = NumericMode::Checked(NumericCoerceTarget::Long);
        let bin = |op, l, r| CoreValue::Binary {
            op,
            lhs: Box::new(l),
            rhs: Box::new(r),
            mode: StringCompareMode::Binary,
            num: long,
        };
        let expr = bin(
            CoreBinOp::Mul,
            bin(
                CoreBinOp::Add,
                CoreValue::Const(CoreConst::I32(10)),
                CoreValue::Const(CoreConst::I32(5)),
            ),
            CoreValue::Const(CoreConst::I32(2)),
        );
        let prog = main_proc(
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            vec![assign(n(), expr)],
        );
        let vm = run_core(&prog);
        let v = vm.slot(0).expect("slot n"); // global_count is 0, so slot 0 = Main local 0
        assert_eq!(v.vtype(), VarType::Long);
        assert_eq!(v.as_i32(), Some(30));
    }

    #[test]
    fn string_concat_matches() {
        // Sub Main(): s = "ab" & "cd"  →  s is "abcd".
        let s = || CorePlace::Local(CoreLocalId(0));
        let expr = CoreValue::Binary {
            op: CoreBinOp::Concat,
            lhs: Box::new(CoreValue::Const(CoreConst::Str("ab".into()))),
            rhs: Box::new(CoreValue::Const(CoreConst::Str("cd".into()))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        let prog = main_proc(
            vec![local("s", VarTypeRef::Builtin(BuiltinType::String))],
            vec![assign(s(), expr)],
        );
        let vm = run_core(&prog);
        let v = vm.slot(0).expect("slot s");
        let s = oxvba_runtime::variant_to_vba_string(&v)
            .map(|b| b.as_str())
            .unwrap_or_default();
        assert_eq!(s, "abcd");
    }

    #[test]
    fn comparison_into_boolean_matches() {
        // Sub Main(): x = 3.5 : b = (x > 1)  →  b is True.
        let x = || CorePlace::Local(CoreLocalId(0));
        let b = || CorePlace::Local(CoreLocalId(1));
        let cmp = CoreValue::Binary {
            op: CoreBinOp::Gt,
            lhs: Box::new(CoreValue::Load(x())),
            rhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        let prog = main_proc(
            vec![
                local("x", VarTypeRef::Variant),
                local("b", VarTypeRef::Builtin(BuiltinType::Boolean)),
            ],
            vec![
                assign(x(), CoreValue::Const(CoreConst::F64(3.5f64.to_bits()))),
                assign(b(), cmp),
            ],
        );
        let vm = run_core(&prog);
        let bv = vm.slot(1).expect("slot b");
        assert_eq!(bv.as_bool(), Some(true));
    }

    #[test]
    fn module_globals_lead_the_snapshot() {
        // A module global `g As Long` then `Sub Main(): g = 7`.
        let g = CorePlace::Global(oxvba_bundle::coreir::GlobalId(0));
        let mut prog = main_proc(
            Vec::new(),
            vec![assign(g, CoreValue::Const(CoreConst::I32(7)))],
        );
        prog.globals = vec![CoreGlobal {
            name: "g".into(),
            ty: VarTypeRef::Builtin(BuiltinType::Long),
            array_element: None,
        }];
        let vm = run_core(&prog);
        // Slot 0 is the global (globals lead), and there are no Main locals after it.
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(7));
    }

    #[test]
    fn if_else_control_flow_executes() {
        // `If <c> Then n = 5 Else n = 9` — a non-Boolean condition flows through the
        // elaboration's Truthy coercion and the Branch terminator.
        use oxvba_bundle::coreir::CoreIfArm;
        let run_if = |cond: i32| -> Option<i32> {
            let n = || CorePlace::Local(CoreLocalId(0));
            let prog = main_proc(
                vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
                vec![CoreStmt::If {
                    arms: vec![CoreIfArm {
                        condition: CoreValue::Const(CoreConst::I32(cond)),
                        body: vec![assign(n(), CoreValue::Const(CoreConst::I32(5)))],
                    }],
                    else_body: vec![assign(n(), CoreValue::Const(CoreConst::I32(9)))],
                }],
            );
            run_core(&prog).slot(0).and_then(|v| v.as_i32())
        };
        assert_eq!(
            run_if(1),
            Some(5),
            "a truthy condition takes the Then branch"
        );
        assert_eq!(
            run_if(0),
            Some(9),
            "a falsy condition takes the Else branch"
        );
    }

    #[test]
    fn entry_falls_back_to_the_first_proc_when_no_main() {
        // No `Sub Main` (CoreProgram.entry == None): vm3 must still run the only proc,
        // matching vm2's select_entry fallback (else nothing runs and `g` stays Empty).
        let g = CorePlace::Global(oxvba_bundle::coreir::GlobalId(0));
        let prog = CoreProgram {
            long_ptr_width: Default::default(),
            procs: vec![CoreProc {
                name: "Helper".into(), // deliberately not "Main"
                kind: ProcedureKind::Sub,
                params: Vec::new(),
                locals: Vec::new(),
                return_local: None,
                label_lines: Vec::new(),
                body: vec![assign(g, CoreValue::Const(CoreConst::I32(42)))],
            }],
            globals: vec![CoreGlobal {
                name: "g".into(),
                ty: VarTypeRef::Builtin(BuiltinType::Long),
                array_element: None,
            }],
            entry: None,
            unit_name: "T".into(),
            ..Default::default()
        };
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
    }

    #[test]
    fn unassigned_boolean_condition_is_false_not_an_error() {
        // `Dim b As Boolean : If b Then n = 5 Else n = 9` — b is unassigned, so Empty at
        // runtime (not a Boolean tag). vm2's is_truthy(Empty) = False, so the Else branch
        // (n = 9); vm3 must match, not error on the non-Boolean tag. (Regression guard:
        // before the Truthy-always fix this returned Malformed.)
        use oxvba_bundle::coreir::CoreIfArm;
        let b = || CorePlace::Local(CoreLocalId(0)); // Dim b As Boolean (unassigned)
        let n = || CorePlace::Local(CoreLocalId(1)); // Dim n As Long
        let prog = main_proc(
            vec![
                local("b", VarTypeRef::Builtin(BuiltinType::Boolean)),
                local("n", VarTypeRef::Builtin(BuiltinType::Long)),
            ],
            vec![CoreStmt::If {
                arms: vec![CoreIfArm {
                    condition: CoreValue::Load(b()),
                    body: vec![assign(n(), CoreValue::Const(CoreConst::I32(5)))],
                }],
                else_body: vec![assign(n(), CoreValue::Const(CoreConst::I32(9)))],
            }],
        );
        let vm = run_core(&prog);
        assert_eq!(
            vm.slot(1).and_then(|v| v.as_i32()),
            Some(9),
            "unassigned Boolean is False -> Else branch, not an error"
        );
    }

    #[test]
    fn select_case_runs_including_a_null_selector() {
        // A Select matching a case, and one whose selector is Null (no case matches ->
        // Case Else) — the Null selector must fall through, not error (vm2 parity).
        use oxvba_bundle::coreir::{CaseClause, CoreCaseBlock};
        let run_select = |sel: CoreValue, sel_ty: VarTypeRef| -> Option<i32> {
            let s = || CorePlace::Local(CoreLocalId(0));
            let x = || CorePlace::Local(CoreLocalId(1));
            let prog = main_proc(
                vec![
                    local("s", sel_ty),
                    local("x", VarTypeRef::Builtin(BuiltinType::Long)),
                ],
                vec![
                    assign(s(), sel),
                    CoreStmt::Select {
                        selector: CoreValue::Load(s()),
                        cases: vec![CoreCaseBlock {
                            clauses: vec![CaseClause::Value(CoreValue::Const(CoreConst::I32(1)))],
                            body: vec![assign(x(), CoreValue::Const(CoreConst::I32(5)))],
                        }],
                        case_else: vec![assign(x(), CoreValue::Const(CoreConst::I32(9)))],
                        compare_mode: oxvba_bundle::StringCompareMode::Binary,
                    },
                ],
            );
            run_core(&prog).slot(1).and_then(|v| v.as_i32())
        };
        // Selector 1 matches `Case 1` -> x = 5.
        assert_eq!(
            run_select(
                CoreValue::Const(CoreConst::I32(1)),
                VarTypeRef::Builtin(BuiltinType::Long)
            ),
            Some(5)
        );
        // A Null selector matches nothing (is_truthy(Null) = False) -> Case Else, x = 9.
        assert_eq!(
            run_select(CoreValue::Const(CoreConst::Null), VarTypeRef::Variant),
            Some(9),
            "a Null Select selector falls through to Case Else, not an error"
        );
    }

    #[test]
    fn for_loop_accumulates() {
        // `For i = 1 To 3 : s = s + i : Next` -> s = 6 (exercises the For counter-test
        // Branch + its Truthy coercion end-to-end).
        let i = || CorePlace::Local(CoreLocalId(0));
        let s = || CorePlace::Local(CoreLocalId(1));
        let prog = main_proc(
            vec![
                local("i", VarTypeRef::Builtin(BuiltinType::Long)),
                local("s", VarTypeRef::Builtin(BuiltinType::Long)),
            ],
            vec![
                assign(s(), CoreValue::Const(CoreConst::I32(0))),
                CoreStmt::ForRange {
                    var: i(),
                    start: CoreValue::Const(CoreConst::I32(1)),
                    end: CoreValue::Const(CoreConst::I32(3)),
                    step: None,
                    body: vec![assign(
                        s(),
                        CoreValue::Binary {
                            op: CoreBinOp::Add,
                            lhs: Box::new(CoreValue::Load(s())),
                            rhs: Box::new(CoreValue::Load(i())),
                            mode: StringCompareMode::Binary,
                            num: NumericMode::Widening,
                        },
                    )],
                },
            ],
        );
        let vm = run_core(&prog);
        assert_eq!(vm.slot(1).and_then(|v| v.as_i32()), Some(6));
    }

    #[test]
    fn call_proc_returns_a_function_value() {
        // Function Add(a As Long, b As Long) As Long : Add = a + b
        // Sub Main() : n = Add(10, 5)   ->  n = 15
        let add = proc(
            "Add",
            ProcedureKind::Function,
            vec![long_param("a"), long_param("b")], // LocalId 0, 1
            vec![local("Add", VarTypeRef::Builtin(BuiltinType::Long))], // the return local, LocalId 2
            Some(CoreLocalId(2)),
            vec![assign(lc(2), long_add(load(0), load(1)))],
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))], // LocalId 0
            None,
            vec![assign(
                lc(0),
                CoreValue::Call {
                    callee: CoreCallee::VbaProc { proc: ProcId(1) },
                    args: vec![
                        CoreArg::ByVal(CoreValue::Const(CoreConst::I32(10))),
                        CoreArg::ByVal(CoreValue::Const(CoreConst::I32(5))),
                    ],
                },
            )],
        );
        let prog = procs_program(vec![main, add]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(15));
    }

    #[test]
    fn link_over_a_single_program_runs_like_run() {
        // W2: Vm3::link over a single program activates the same image Vm3::run does; a
        // multi-program link now activates every program (cross-project EXECUTION — calls, fault
        // unwind, unresolved-reference rejection — is covered in tests/cross_program.rs).
        let prog = main_proc(
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            vec![assign(lc(0), CoreValue::Const(CoreConst::I32(42)))],
        );
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(&prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        let mut vm = Vm3::link(&[oxp], host).expect("link single program");
        vm.run_entry().expect("run_entry");
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));

        // A multi-program link activates every program and runs the entry (the last program);
        // two independent copies link cleanly and the entry's `n` slot is 42.
        let mut multi = Vm3::link(&[oxp, oxp], host).expect("multi-program link");
        multi.run_entry().expect("run_entry");
        assert_eq!(multi.slot(0).and_then(|v| v.as_i32()), Some(42));
    }

    #[test]
    fn host_session_creates_a_class_and_invokes_a_member() {
        // W7: the host session API — create a project-class instance by name (activate without
        // running Main), then invoke a member with pre-marshaled value args. Class Widget with
        // Function Twice(x) = x + x; Widget.Twice(21) -> 42.
        let twice = proc(
            "Twice",
            ProcedureKind::Function,
            vec![long_param("me"), long_param("x")], // me LocalId 0, x LocalId 1
            vec![local("Twice", VarTypeRef::Builtin(BuiltinType::Long))], // return local, LocalId 2
            Some(CoreLocalId(2)),
            vec![assign(lc(2), long_add(load(1), load(1)))],
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        );
        let prog = CoreProgram {
            long_ptr_width: Default::default(),
            procs: vec![main, twice],
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            classes: vec![CoreClass {
                name: "Widget".into(),
                predeclared: false,
                initialize: None,
                terminate: None,
                fields: Vec::new(),
                methods: vec![CoreClassMethod {
                    name: "Twice".into(),
                    kind: ProjectMemberKind::Method,
                    proc: ProcId(1),
                    dispid: None,
                    vtable_slot: None,
                    is_default_member: false,
                    is_enumerator_member: false,
                }],
                as_new_fields: Vec::new(),
                implements: Vec::new(),
            }],
            ..Default::default()
        };
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(&prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        let mut vm = Vm3::activate(oxp, host).expect("activate");
        let widget = vm.create_project_instance("Widget").expect("create Widget");
        let obj = widget.as_object_ref().expect("Widget is an object");
        let r = vm
            .invoke_member_values(
                obj,
                "Twice",
                Some(ProjectMemberKind::Method),
                vec![Variant::from_i32(21)],
            )
            .expect("invoke Twice");
        assert_eq!(
            r.as_i32(),
            Some(42),
            "create + invoke a member with value args"
        );
        // An unknown class is "can't create object" (429).
        assert!(matches!(
            vm.create_project_instance("Nope"),
            Err(Vm3Error::Fault(_))
        ));
    }

    #[test]
    fn project_instance_runtime_descriptor_carries_dispatch_members() {
        let get_value = proc(
            "GetValue",
            ProcedureKind::PropertyGet,
            vec![long_param("me")],
            vec![local("GetValue", VarTypeRef::Builtin(BuiltinType::Long))],
            Some(CoreLocalId(1)),
            vec![assign(lc(1), CoreValue::Const(CoreConst::I32(7)))],
        );
        let let_value = proc(
            "LetValue",
            ProcedureKind::PropertyLet,
            vec![long_param("me"), byval_long_param("newValue")],
            Vec::new(),
            None,
            Vec::new(),
        );
        let reset = proc(
            "Reset",
            ProcedureKind::Sub,
            vec![long_param("me"), optional_long_param("count")],
            Vec::new(),
            None,
            Vec::new(),
        );
        let new_enum = proc(
            "NewEnum",
            ProcedureKind::PropertyGet,
            vec![long_param("me")],
            vec![local("NewEnum", VarTypeRef::Object("Object".into()))],
            Some(CoreLocalId(1)),
            Vec::new(),
        );
        let initialize = proc(
            "Class_Initialize",
            ProcedureKind::Sub,
            vec![long_param("me")],
            Vec::new(),
            None,
            Vec::new(),
        );
        let terminate = proc(
            "Class_Terminate",
            ProcedureKind::Sub,
            vec![long_param("me")],
            Vec::new(),
            None,
            Vec::new(),
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        );
        let prog = CoreProgram {
            long_ptr_width: Default::default(),
            procs: vec![
                main, get_value, let_value, reset, initialize, terminate, new_enum,
            ],
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            classes: vec![CoreClass {
                name: "Widget".into(),
                predeclared: true,
                initialize: Some(ProcId(4)),
                terminate: Some(ProcId(5)),
                fields: vec![
                    CoreClassField {
                        name: "mCount".into(),
                        token: 11,
                        ty: VarTypeRef::Builtin(BuiltinType::Long),
                        array_element: None,
                    },
                    CoreClassField {
                        name: "Items".into(),
                        token: 12,
                        ty: VarTypeRef::Array(Box::new(VarTypeRef::Builtin(BuiltinType::Long))),
                        array_element: Some(ArrayElementType::Long),
                    },
                    CoreClassField {
                        name: "Child".into(),
                        token: 13,
                        ty: VarTypeRef::Object("Widget".into()),
                        array_element: None,
                    },
                ],
                methods: vec![
                    CoreClassMethod {
                        name: "Value".into(),
                        kind: ProjectMemberKind::PropertyGet,
                        proc: ProcId(1),
                        dispid: Some(0),
                        vtable_slot: Some(7),
                        is_default_member: true,
                        is_enumerator_member: false,
                    },
                    CoreClassMethod {
                        name: "Value".into(),
                        kind: ProjectMemberKind::PropertyLet,
                        proc: ProcId(2),
                        dispid: Some(0),
                        vtable_slot: Some(8),
                        is_default_member: true,
                        is_enumerator_member: false,
                    },
                    CoreClassMethod {
                        name: "Reset".into(),
                        kind: ProjectMemberKind::Method,
                        proc: ProcId(3),
                        dispid: None,
                        vtable_slot: None,
                        is_default_member: false,
                        is_enumerator_member: false,
                    },
                    CoreClassMethod {
                        name: "NewEnum".into(),
                        kind: ProjectMemberKind::PropertyGet,
                        proc: ProcId(6),
                        dispid: Some(-4),
                        vtable_slot: Some(12),
                        is_default_member: false,
                        is_enumerator_member: true,
                    },
                ],
                as_new_fields: vec![CoreClassAsNewField {
                    field: 13,
                    binding: CoreAsNew::ProjectClass {
                        class: CoreClassId(0),
                    },
                }],
                implements: vec!["IWidget".into(), "IDisposable".into()],
            }],
            ..Default::default()
        };
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(&prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        let mut vm = Vm3::activate(oxp, host).expect("activate");
        let widget = vm.create_project_instance("Widget").expect("create Widget");
        let obj = widget.as_object_ref().expect("Widget object");
        let class_descriptor = obj.class_descriptor();

        assert_eq!(class_descriptor.name, "Widget");
        assert_eq!(
            class_descriptor.project_identity,
            Some(RuntimeProjectClassIdentity {
                unit_name: "T",
                class_index: 0,
            })
        );
        assert!(class_descriptor.predeclared);
        assert!(class_descriptor.lifecycle.has_initialize);
        assert!(class_descriptor.lifecycle.has_terminate);
        assert_eq!(class_descriptor.implements, &["IWidget", "IDisposable"]);
        assert_eq!(class_descriptor.fields.len(), 3);
        assert_eq!(class_descriptor.fields[0].name, "mCount");
        assert_eq!(class_descriptor.fields[0].token, 11);
        assert_eq!(
            class_descriptor.fields[0].value_type,
            RuntimeValueType::Long
        );
        assert_eq!(class_descriptor.fields[0].array_element_type, None);
        assert_eq!(class_descriptor.fields[1].name, "Items");
        assert_eq!(class_descriptor.fields[1].token, 12);
        assert_eq!(
            class_descriptor.fields[1].value_type,
            RuntimeValueType::Array
        );
        assert_eq!(
            class_descriptor.fields[1].array_element_type,
            Some(RuntimeValueType::Long)
        );
        assert_eq!(class_descriptor.fields[2].name, "Child");
        assert_eq!(class_descriptor.fields[2].token, 13);
        assert_eq!(
            class_descriptor.fields[2].value_type,
            RuntimeValueType::Object
        );
        assert_eq!(class_descriptor.fields[2].array_element_type, None);
        assert_eq!(class_descriptor.as_new_fields.len(), 1);
        assert_eq!(class_descriptor.as_new_fields[0].field_token, 13);
        assert_eq!(
            class_descriptor.as_new_fields[0].activation,
            RuntimeClassActivationDescriptor::ProjectClass { class_index: 0 }
        );

        assert!(
            obj.query_interface_descriptor(RuntimeInterfaceId::IUnknown)
                .is_some()
        );
        let dispatch = obj
            .query_interface_descriptor(RuntimeInterfaceId::IDispatch)
            .expect("project instances should advertise IDispatch");
        assert_eq!(dispatch.name, "IDispatch");
        assert!(dispatch.dual_dispatch);
        assert_eq!(dispatch.members.len(), 4);

        let get = &dispatch.members[0];
        assert_eq!(get.name, "Value");
        assert_eq!(get.dispatch_id, 0);
        assert_eq!(get.vtable_slot, Some(7));
        assert_eq!(get.invoke_kind, RuntimeMemberInvokeKind::PropertyGet);
        assert!(get.is_default_member);
        assert!(!get.is_enumerator_member);
        assert_eq!(get.arity, 0);
        assert_eq!(get.params, &[]);
        assert_eq!(get.return_type, Some(RuntimeValueType::Long));

        let put = &dispatch.members[1];
        assert_eq!(put.name, "Value");
        assert_eq!(put.dispatch_id, 0);
        assert_eq!(put.vtable_slot, Some(8));
        assert_eq!(put.invoke_kind, RuntimeMemberInvokeKind::PropertyLet);
        assert!(put.is_default_member);
        assert!(!put.is_enumerator_member);
        assert_eq!(put.arity, 1);
        assert_eq!(put.params[0].name, "newValue");
        assert_eq!(put.params[0].value_type, RuntimeValueType::Long);
        assert!(!put.params[0].by_ref);
        assert_eq!(put.return_type, None);

        let method = &dispatch.members[2];
        assert_eq!(method.name, "Reset");
        assert_eq!(method.dispatch_id, 3);
        assert_eq!(method.vtable_slot, None);
        assert_eq!(method.invoke_kind, RuntimeMemberInvokeKind::Method);
        assert!(!method.is_default_member);
        assert!(!method.is_enumerator_member);
        assert_eq!(method.arity, 1);
        assert_eq!(method.params[0].name, "count");
        assert_eq!(method.params[0].value_type, RuntimeValueType::Long);
        assert!(method.params[0].by_ref);
        assert!(method.params[0].optional);
        assert!(!method.params[0].param_array);
        assert_eq!(method.return_type, None);

        let new_enum = &dispatch.members[3];
        assert_eq!(new_enum.name, "NewEnum");
        assert_eq!(new_enum.dispatch_id, -4);
        assert_eq!(new_enum.vtable_slot, Some(12));
        assert_eq!(new_enum.invoke_kind, RuntimeMemberInvokeKind::PropertyGet);
        assert!(!new_enum.is_default_member);
        assert!(new_enum.is_enumerator_member);
        assert_eq!(new_enum.arity, 0);
        assert_eq!(new_enum.return_type, Some(RuntimeValueType::Object));

        drop(obj);
        drop(widget);
        vm.maybe_drain();
    }

    #[test]
    fn project_instance_runtime_descriptor_carries_implemented_interfaces() {
        let iface_get_size = proc(
            "Size",
            ProcedureKind::PropertyGet,
            vec![long_param("me")],
            vec![local("Size", VarTypeRef::Builtin(BuiltinType::Long))],
            Some(CoreLocalId(1)),
            Vec::new(),
        );
        let iface_let_size = proc(
            "Size",
            ProcedureKind::PropertyLet,
            vec![long_param("me"), byval_long_param("value")],
            Vec::new(),
            None,
            Vec::new(),
        );
        let impl_get_size = proc(
            "IShape_Size",
            ProcedureKind::PropertyGet,
            vec![long_param("me")],
            vec![local("IShape_Size", VarTypeRef::Builtin(BuiltinType::Long))],
            Some(CoreLocalId(1)),
            Vec::new(),
        );
        let impl_let_size = proc(
            "IShape_Size",
            ProcedureKind::PropertyLet,
            vec![long_param("me"), byval_long_param("newSize")],
            Vec::new(),
            None,
            Vec::new(),
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        );
        let prog = CoreProgram {
            long_ptr_width: Default::default(),
            procs: vec![
                main,
                iface_get_size,
                iface_let_size,
                impl_get_size,
                impl_let_size,
            ],
            entry: Some(ProcId(0)),
            unit_name: "T".into(),
            classes: vec![
                CoreClass {
                    name: "CBox".into(),
                    predeclared: false,
                    initialize: None,
                    terminate: None,
                    fields: Vec::new(),
                    methods: vec![
                        CoreClassMethod {
                            name: "IShape_Size".into(),
                            kind: ProjectMemberKind::PropertyGet,
                            proc: ProcId(3),
                            dispid: None,
                            vtable_slot: None,
                            is_default_member: false,
                            is_enumerator_member: false,
                        },
                        CoreClassMethod {
                            name: "IShape_Size".into(),
                            kind: ProjectMemberKind::PropertyLet,
                            proc: ProcId(4),
                            dispid: None,
                            vtable_slot: None,
                            is_default_member: false,
                            is_enumerator_member: false,
                        },
                    ],
                    as_new_fields: Vec::new(),
                    implements: vec!["IShape".into()],
                },
                CoreClass {
                    name: "IShape".into(),
                    predeclared: false,
                    initialize: None,
                    terminate: None,
                    fields: Vec::new(),
                    methods: vec![
                        CoreClassMethod {
                            name: "Size".into(),
                            kind: ProjectMemberKind::PropertyGet,
                            proc: ProcId(1),
                            dispid: Some(0),
                            vtable_slot: Some(11),
                            is_default_member: true,
                            is_enumerator_member: false,
                        },
                        CoreClassMethod {
                            name: "Size".into(),
                            kind: ProjectMemberKind::PropertyLet,
                            proc: ProcId(2),
                            dispid: Some(0),
                            vtable_slot: Some(12),
                            is_default_member: true,
                            is_enumerator_member: false,
                        },
                    ],
                    as_new_fields: Vec::new(),
                    implements: Vec::new(),
                },
            ],
            ..Default::default()
        };
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(&prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        let mut vm = Vm3::activate(oxp, host).expect("activate");
        let cbox = vm.create_project_instance("CBox").expect("create CBox");
        let obj = cbox.as_object_ref().expect("CBox object");
        let class_descriptor = obj.class_descriptor();

        assert_eq!(class_descriptor.implements, &["IShape"]);
        assert_eq!(
            class_descriptor.project_identity,
            Some(RuntimeProjectClassIdentity {
                unit_name: "T",
                class_index: 0,
            })
        );
        assert_eq!(class_descriptor.interfaces.len(), 3);
        assert!(
            obj.query_interface_descriptor(RuntimeInterfaceId::IUnknown)
                .is_some()
        );
        assert!(
            obj.query_interface_descriptor(RuntimeInterfaceId::IDispatch)
                .is_some()
        );

        let shape = class_descriptor
            .interfaces
            .iter()
            .find(|interface| interface.name == "IShape")
            .expect("implemented project interface descriptor");
        assert_eq!(shape.id, RuntimeInterfaceId::Unsupported);
        assert_eq!(shape.identity.id, RuntimeInterfaceId::Unsupported);
        assert_eq!(shape.identity.name, "T.IShape");
        assert_eq!(shape.identity.kind, RuntimeInterfaceKind::Custom);
        assert!(!shape.dual_dispatch);
        assert_eq!(shape.members.len(), 2);
        assert_eq!(
            obj.query_interface_descriptor_by_guid(shape.identity.guid)
                .map(|descriptor| descriptor.name),
            Some("IShape")
        );

        let get = &shape.members[0];
        assert_eq!(get.name, "Size");
        assert_eq!(get.dispatch_id, 0);
        assert_eq!(get.vtable_slot, Some(11));
        assert_eq!(get.invoke_kind, RuntimeMemberInvokeKind::PropertyGet);
        assert!(get.is_default_member);
        assert_eq!(get.arity, 0);
        assert_eq!(get.return_type, Some(RuntimeValueType::Long));

        let put = &shape.members[1];
        assert_eq!(put.name, "Size");
        assert_eq!(put.dispatch_id, 0);
        assert_eq!(put.vtable_slot, Some(12));
        assert_eq!(put.invoke_kind, RuntimeMemberInvokeKind::PropertyLet);
        assert!(put.is_default_member);
        assert_eq!(put.arity, 1);
        assert_eq!(put.params[0].name, "newSize");
        assert_eq!(put.params[0].value_type, RuntimeValueType::Long);
        assert!(!put.params[0].by_ref);
        assert_eq!(put.return_type, None);

        drop(obj);
        drop(cbox);
        vm.maybe_drain();
    }

    #[test]
    fn run_proc_with_values_seeds_host_value_args_across_repeated_invokes() {
        // W0: a host session activates a program ONCE (no Main run), then invokes a function
        // directly with already-evaluated Variant args (the path object/array args need, since
        // an OxArg::ByVal carries only a scalar OxConst). Invoking twice against the one
        // activation proves the session lifecycle: activate() is separate from run_entry(), so
        // the descriptor-leak + termination-reset happen once, not per invoke.
        // run_proc_with_values mirrors run_proc_with_me's frame layout: local 0 is the
        // receiver `me` (as for a class method / event handler), and the value args land at
        // locals 1.. — so the callable is shaped like a method whose first slot is `me`.
        let add = proc(
            "Add",
            ProcedureKind::Function,
            vec![long_param("me"), long_param("a"), long_param("b")], // LocalId 0=me, 1=a, 2=b
            vec![local("Add", VarTypeRef::Builtin(BuiltinType::Long))], // return local, LocalId 3
            Some(CoreLocalId(3)),
            vec![assign(lc(3), long_add(load(1), load(2)))],
        );
        // Main is a no-op entry (procs_program requires procs[0] to be the entry); we never run it.
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
        );
        let prog = procs_program(vec![main, add]);
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(&prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        let mut vm = Vm3::activate(oxp, host).expect("activate"); // NOT run() — Main never executes

        let first = vm
            .run_proc_with_values(
                0,
                FuncId(1),
                Variant::empty(),
                vec![Variant::from_i32(20), Variant::from_i32(22)],
                false,
            )
            .expect("invoke Add #1");
        assert_eq!(
            first.as_i32(),
            Some(42),
            "value-seeded args reach the proc and return their sum"
        );

        let second = vm
            .run_proc_with_values(
                0,
                FuncId(1),
                Variant::empty(),
                vec![Variant::from_i32(1), Variant::from_i32(2)],
                false,
            )
            .expect("invoke Add #2");
        assert_eq!(
            second.as_i32(),
            Some(3),
            "a second invoke against the same activation works"
        );
    }

    #[test]
    fn call_proc_byref_mutates_the_caller() {
        // Sub Bump(ByRef x As Long) : x = x + 1
        // Sub Main() : v = 41 : Bump(v)   ->  v = 42 (true aliasing through the frame stack)
        let bump = proc(
            "Bump",
            ProcedureKind::Sub,
            vec![long_param("x")], // LocalId 0
            Vec::new(),
            None,
            vec![assign(
                lc(0),
                long_add(load(0), CoreValue::Const(CoreConst::I32(1))),
            )],
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("v", VarTypeRef::Builtin(BuiltinType::Long))], // LocalId 0
            None,
            vec![
                assign(lc(0), CoreValue::Const(CoreConst::I32(41))),
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::VbaProc { proc: ProcId(1) },
                    args: vec![CoreArg::ByRef(lc(0))],
                }),
            ],
        );
        let prog = procs_program(vec![main, bump]);
        let vm = run_core(&prog);
        assert_eq!(
            vm.slot(0).and_then(|v| v.as_i32()),
            Some(42),
            "a ByRef write must propagate to the caller's backing slot"
        );
    }

    #[test]
    fn call_proc_byval_does_not_mutate_the_caller() {
        // The same Bump, but Main passes `v` ByVal -> the callee mutates a copy, v stays 41.
        let bump = proc(
            "Bump",
            ProcedureKind::Sub,
            vec![long_param("x")],
            Vec::new(),
            None,
            vec![assign(
                lc(0),
                long_add(load(0), CoreValue::Const(CoreConst::I32(1))),
            )],
        );
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("v", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![
                assign(lc(0), CoreValue::Const(CoreConst::I32(41))),
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::VbaProc { proc: ProcId(1) },
                    args: vec![CoreArg::ByVal(load(0))],
                }),
            ],
        );
        let prog = procs_program(vec![main, bump]);
        let vm = run_core(&prog);
        assert_eq!(
            vm.slot(0).and_then(|v| v.as_i32()),
            Some(41),
            "a ByVal copy must NOT propagate back to the caller"
        );
    }

    #[test]
    fn call_native_builtin_invokes_the_shared_library() {
        // Sub Main() : n = Len("abc")   ->  n = 3 (through the context-free dispatcher).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![assign(
                lc(0),
                CoreValue::Call {
                    callee: CoreCallee::Native(NativeImplId::Len),
                    args: vec![CoreArg::ByVal(CoreValue::Const(CoreConst::Str(
                        "abc".into(),
                    )))],
                },
            )],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(3));
    }

    /// A bounded host probe for the current VM3 event-pump contract. The host
    /// receives no VM/session callback; `DoEvents` returns first, and VM3 polls
    /// queued COM callbacks only from its following `StmtBoundary`.
    struct QueuedDoEventsHost {
        inner: NullHostServices,
        in_do_events: AtomicBool,
        callback_ready: AtomicBool,
        order: Mutex<Vec<&'static str>>,
    }

    impl QueuedDoEventsHost {
        fn new() -> Self {
            Self {
                inner: NullHostServices::new(HostPolicy::deterministic_runtime()),
                in_do_events: AtomicBool::new(false),
                callback_ready: AtomicBool::new(false),
                order: Mutex::new(Vec::new()),
            }
        }

        fn push(&self, event: &'static str) {
            self.order.lock().expect("order lock").push(event);
        }
    }

    impl HostServices for QueuedDoEventsHost {
        fn profile(&self) -> HalProfileId {
            self.inner.profile()
        }
        fn descriptor(&self) -> HalDescriptor {
            self.inner.descriptor()
        }
        fn policy(&self) -> &HostPolicy {
            self.inner.policy()
        }
        fn console(&self) -> &dyn ConsoleHal {
            self.inner.console()
        }
        fn ui(&self) -> &dyn UiInteractionHal {
            self.inner.ui()
        }
        fn events(&self) -> &dyn EventPumpHal {
            self
        }
        fn fs(&self) -> &dyn FileSystemHal {
            self.inner.fs()
        }
        fn process(&self) -> &dyn ProcessEnvHal {
            self.inner.process()
        }
        fn com(&self) -> &dyn ComHal {
            self
        }
        fn time_locale(&self) -> &dyn TimeLocaleHal {
            self.inner.time_locale()
        }
        fn dynlink(&self) -> &dyn DynamicLinkHal {
            self.inner.dynlink()
        }
        fn diag(&self) -> &dyn DiagnosticsHal {
            self.inner.diag()
        }
    }

    impl EventPumpHal for QueuedDoEventsHost {
        fn do_events_variant(&self) -> HalResult<Variant> {
            assert!(
                !self.in_do_events.swap(true, Ordering::AcqRel),
                "DoEvents must not recursively enter this host probe"
            );
            self.push("do-events-enter");
            // StandardHostServices performs a bounded OS pump and marks a queued
            // COM callback here. It has no safe VM3 callback authority.
            self.callback_ready.store(true, Ordering::Release);
            self.push("do-events-exit");
            self.in_do_events.store(false, Ordering::Release);
            Ok(Variant::from_i32(0))
        }
    }

    impl ComHal for QueuedDoEventsHost {
        fn describe_object(
            &self,
            object: ObjectRef,
        ) -> HalResult<Option<oxvba_com::ComObjectDescriptor>> {
            self.inner.com().describe_object(object)
        }

        fn subscribe_event(
            &self,
            object: ObjectRef,
            event: ComMemberToken,
        ) -> HalResult<ComSubscriptionToken> {
            self.inner.com().subscribe_event(object, event)
        }

        fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>> {
            assert!(
                !self.in_do_events.load(Ordering::Acquire),
                "VM3 must not poll or dispatch a callback inside the host DoEvents call"
            );
            self.push("stmt-boundary-poll");
            if self.callback_ready.swap(false, Ordering::AcqRel) {
                return Ok(Some(ComCallbackPayload {
                    callback: ComCallbackToken::new(60_001),
                    subscription: ComSubscriptionToken::new(40_001),
                    object: ObjectRef::from_compat_identity(20_001),
                    event: ComMemberToken::new(1),
                    args: Vec::new(),
                }));
            }
            Ok(None)
        }

        fn event_callback_subscription(
            &self,
            callback: ComCallbackToken,
        ) -> HalResult<ComSubscriptionToken> {
            self.inner.com().event_callback_subscription(callback)
        }

        fn event_callback_arity(&self, callback: ComCallbackToken) -> HalResult<usize> {
            self.inner.com().event_callback_arity(callback)
        }

        fn release_event_callback_variant(&self, callback: ComCallbackToken) -> HalResult<Variant> {
            assert_eq!(callback, ComCallbackToken::new(60_001));
            self.push("callback-release");
            Ok(Variant::from_i32(1))
        }

        fn resolve_typelib_reference(
            &self,
            request: &TypeLibResolveRequest,
        ) -> HalResult<TypeLibResolvedIdentity> {
            self.inner.com().resolve_typelib_reference(request)
        }

        fn load_typelib_metadata(
            &self,
            identity: &TypeLibResolvedIdentity,
        ) -> HalResult<TypeLibMetadataBlob> {
            self.inner.com().load_typelib_metadata(identity)
        }

        fn invalidate_typelib_cache(
            &self,
            scope: TypeLibCacheScope,
            reference_name: Option<&str>,
        ) -> HalResult<Variant> {
            self.inner
                .com()
                .invalidate_typelib_cache(scope, reference_name)
        }
    }

    #[test]
    fn do_events_returns_before_vm3_polls_callbacks_and_rng_context_remains_usable() {
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![
                local("before", VarTypeRef::Builtin(BuiltinType::Single)),
                local("after", VarTypeRef::Builtin(BuiltinType::Single)),
            ],
            None,
            vec![
                assign(
                    lc(0),
                    CoreValue::Call {
                        callee: CoreCallee::Native(NativeImplId::Rnd),
                        args: Vec::new(),
                    },
                ),
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::Native(NativeImplId::DoEvents),
                    args: Vec::new(),
                }),
                assign(
                    lc(1),
                    CoreValue::Call {
                        callee: CoreCallee::Native(NativeImplId::Rnd),
                        args: Vec::new(),
                    },
                ),
            ],
        );
        let oxp = oxvba_oxir::elaborate::elaborate(&procs_program(vec![main]))
            .expect("elaborate DoEvents probe");
        let host = QueuedDoEventsHost::new();
        let vm = Vm3::run(&oxp, &host).expect("VM3 DoEvents probe");

        assert!(vm.slot(0).and_then(|value| value.as_f32()).is_some());
        assert!(vm.slot(1).and_then(|value| value.as_f32()).is_some());

        let order = host.order.lock().expect("order lock");
        let enter = order
            .iter()
            .position(|event| *event == "do-events-enter")
            .expect("DoEvents entered host");
        assert_eq!(order.get(enter + 1), Some(&"do-events-exit"));
        assert!(
            order[enter + 2..].contains(&"stmt-boundary-poll"),
            "VM3 must poll queued callbacks only after host DoEvents returns: {order:?}"
        );
        assert!(
            order[enter + 2..].contains(&"callback-release"),
            "VM3 must own and release the queued payload after host return: {order:?}"
        );
    }

    // ── Native interop (M3-7): Declare Lib + pointer helpers + AddressOf ──────────
    //
    // The corpus differential gate exercises the `Declare`/`StrPtr`/`VarPtr` *reachable*
    // ops end-to-end against the deterministic host (vm3 matches vm2). These unit tests
    // pin the marshalling under a *controlled* mock dynlink — proving the ByRef write-back
    // targets the caller slot, `Err.LastDllError` refreshes, and a pointer pin round-trips
    // — independent of whatever a real OS `Declare` would do.

    /// A host whose dynlink echoes each argument incremented by 100 (a fake native
    /// "Bump") and reports a fixed `last_dll_error`, to exercise per-call-site `Declare`
    /// ByRef write-back and the `Err.LastDllError` refresh. Mirrors vm2's test host.
    struct MockDynlinkHost {
        inner: NullHostServices,
        last_dll_error: i32,
    }
    impl MockDynlinkHost {
        fn new(last_dll_error: i32) -> Self {
            Self {
                inner: NullHostServices::new(HostPolicy::deterministic_runtime()),
                last_dll_error,
            }
        }
    }
    impl HostServices for MockDynlinkHost {
        fn profile(&self) -> HalProfileId {
            self.inner.profile()
        }
        fn descriptor(&self) -> HalDescriptor {
            self.inner.descriptor()
        }
        fn policy(&self) -> &HostPolicy {
            self.inner.policy()
        }
        fn console(&self) -> &dyn ConsoleHal {
            self.inner.console()
        }
        fn ui(&self) -> &dyn UiInteractionHal {
            self.inner.ui()
        }
        fn events(&self) -> &dyn EventPumpHal {
            self.inner.events()
        }
        fn fs(&self) -> &dyn FileSystemHal {
            self.inner.fs()
        }
        fn process(&self) -> &dyn ProcessEnvHal {
            self.inner.process()
        }
        fn com(&self) -> &dyn ComHal {
            self.inner.com()
        }
        fn time_locale(&self) -> &dyn TimeLocaleHal {
            self.inner.time_locale()
        }
        fn dynlink(&self) -> &dyn DynamicLinkHal {
            self
        }
        fn diag(&self) -> &dyn DiagnosticsHal {
            self.inner.diag()
        }
    }
    impl DynamicLinkHal for MockDynlinkHost {
        fn invoke_descriptor_variants(
            &self,
            _descriptor: &DynLinkDescriptorView<'_>,
            args: &[Variant],
        ) -> HalResult<(Variant, Vec<Variant>)> {
            let wb = args
                .iter()
                .map(|a| {
                    a.as_i32()
                        .map(|n| Variant::from_i32(n + 100))
                        .unwrap_or_else(|| a.clone())
                })
                .collect();
            Ok((Variant::empty(), wb))
        }
        fn last_dll_error(&self) -> i32 {
            self.last_dll_error
        }
    }

    struct MockComEnumerateFailHost {
        inner: NullHostServices,
    }

    impl MockComEnumerateFailHost {
        fn new() -> Self {
            Self {
                inner: NullHostServices::new(HostPolicy::deterministic_runtime()),
            }
        }
    }

    impl HostServices for MockComEnumerateFailHost {
        fn profile(&self) -> HalProfileId {
            self.inner.profile()
        }
        fn descriptor(&self) -> HalDescriptor {
            self.inner.descriptor()
        }
        fn policy(&self) -> &HostPolicy {
            self.inner.policy()
        }
        fn console(&self) -> &dyn ConsoleHal {
            self.inner.console()
        }
        fn ui(&self) -> &dyn UiInteractionHal {
            self.inner.ui()
        }
        fn events(&self) -> &dyn EventPumpHal {
            self.inner.events()
        }
        fn fs(&self) -> &dyn FileSystemHal {
            self.inner.fs()
        }
        fn process(&self) -> &dyn ProcessEnvHal {
            self.inner.process()
        }
        fn com(&self) -> &dyn ComHal {
            self
        }
        fn time_locale(&self) -> &dyn TimeLocaleHal {
            self.inner.time_locale()
        }
        fn dynlink(&self) -> &dyn DynamicLinkHal {
            self.inner.dynlink()
        }
        fn diag(&self) -> &dyn DiagnosticsHal {
            self.inner.diag()
        }
    }

    impl ComHal for MockComEnumerateFailHost {
        fn create_object_variant(&self, _prog_id: Variant) -> HalResult<Variant> {
            Ok(Variant::from_object_ref(fake_foreign_object()))
        }

        fn describe_object(
            &self,
            _object: ObjectRef,
        ) -> HalResult<Option<oxvba_com::ComObjectDescriptor>> {
            Ok(None)
        }

        fn enumerate_object(&self, _object: ObjectRef) -> HalResult<Vec<Variant>> {
            Err(HalError::adapter_fault(
                HalProfileId::Windows,
                CapabilityId::ComActivationDispatch,
                "enumerate_object",
                "mock IEnumVARIANT failure",
            )
            .with_host_error_code(438))
        }

        fn subscribe_event(
            &self,
            object: ObjectRef,
            event: ComMemberToken,
        ) -> HalResult<ComSubscriptionToken> {
            self.inner.com().subscribe_event(object, event)
        }

        fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>> {
            self.inner.com().poll_event_callback()
        }

        fn event_callback_subscription(
            &self,
            callback: ComCallbackToken,
        ) -> HalResult<ComSubscriptionToken> {
            self.inner.com().event_callback_subscription(callback)
        }

        fn event_callback_arity(&self, callback: ComCallbackToken) -> HalResult<usize> {
            self.inner.com().event_callback_arity(callback)
        }

        fn resolve_typelib_reference(
            &self,
            request: &TypeLibResolveRequest,
        ) -> HalResult<TypeLibResolvedIdentity> {
            self.inner.com().resolve_typelib_reference(request)
        }

        fn load_typelib_metadata(
            &self,
            identity: &TypeLibResolvedIdentity,
        ) -> HalResult<TypeLibMetadataBlob> {
            self.inner.com().load_typelib_metadata(identity)
        }

        fn invalidate_typelib_cache(
            &self,
            scope: TypeLibCacheScope,
            reference_name: Option<&str>,
        ) -> HalResult<Variant> {
            self.inner
                .com()
                .invalidate_typelib_cache(scope, reference_name)
        }
    }

    #[repr(C)]
    struct FakeForeignObject {
        unknown: RawRuntimeIUnknown,
        ref_count: AtomicU32,
        drop_count: Arc<AtomicU32>,
    }

    impl Drop for FakeForeignObject {
        fn drop(&mut self) {
            self.drop_count.fetch_add(1, Ordering::AcqRel);
        }
    }

    static FAKE_FOREIGN_VTBL: RawRuntimeIUnknownVtbl = RawRuntimeIUnknownVtbl {
        query_interface: fake_foreign_query_interface,
        add_ref: fake_foreign_add_ref,
        release: fake_foreign_release,
    };

    unsafe extern "C" fn fake_foreign_query_interface(
        _this: *mut c_void,
        _iid: RuntimeGuid,
        ppv: *mut *mut c_void,
    ) -> i32 {
        if !ppv.is_null() {
            // SAFETY: COM QueryInterface receives a caller-owned out pointer.
            unsafe {
                *ppv = std::ptr::null_mut();
            }
        }
        RUNTIME_E_NOINTERFACE
    }

    unsafe extern "C" fn fake_foreign_add_ref(this: *mut c_void) -> u32 {
        let object = this.cast::<FakeForeignObject>();
        // SAFETY: the fake vtable is installed only on FakeForeignObject boxes.
        unsafe { (*object).ref_count.fetch_add(1, Ordering::AcqRel) + 1 }
    }

    unsafe extern "C" fn fake_foreign_release(this: *mut c_void) -> u32 {
        let object = this.cast::<FakeForeignObject>();
        // SAFETY: the fake vtable is installed only on FakeForeignObject boxes.
        let previous = unsafe { (*object).ref_count.fetch_sub(1, Ordering::AcqRel) };
        let remaining = previous.saturating_sub(1);
        if remaining == 0 {
            // SAFETY: refcount reached zero for the box allocated in fake_foreign_object.
            unsafe {
                drop(Box::from_raw(object));
            }
        }
        remaining
    }

    fn fake_foreign_object_with_drop_count(drop_count: Arc<AtomicU32>) -> ObjectRef {
        let boxed = Box::new(FakeForeignObject {
            unknown: RawRuntimeIUnknown {
                vtbl: &FAKE_FOREIGN_VTBL,
            },
            ref_count: AtomicU32::new(1),
            drop_count,
        });
        let raw = Box::into_raw(boxed);
        // SAFETY: `FakeForeignObject` is `repr(C)` with `unknown` first. Casting the
        // complete allocation pointer gives the field's exact address while retaining
        // the full-box provenance that AddRef/Release need to access the adjacent
        // refcount and eventually reconstruct the Box. The initial reference transfers
        // to the returned ObjectRef.
        unsafe {
            ObjectRef::from_raw_iunknown_owned(raw.cast::<RawRuntimeIUnknown>())
                .expect("fake object pointer is non-null")
        }
    }

    fn fake_foreign_object() -> ObjectRef {
        fake_foreign_object_with_drop_count(Arc::new(AtomicU32::new(0)))
    }

    unsafe fn fake_foreign_object_ref_count(raw: *mut RawRuntimeIUnknown) -> u32 {
        let object = raw.cast::<FakeForeignObject>();
        // SAFETY: the caller supplies the live complete-allocation-derived interface
        // pointer minted by `fake_foreign_object_with_drop_count`.
        unsafe { (*object).ref_count.load(Ordering::Acquire) }
    }

    #[test]
    fn fake_foreign_object_addref_release_preserves_complete_allocation_provenance() {
        let drop_count = Arc::new(AtomicU32::new(0));
        let object = fake_foreign_object_with_drop_count(Arc::clone(&drop_count));
        let raw = object.raw_iunknown();

        // SAFETY: `object` owns the initial live reference to the fake allocation.
        assert_eq!(unsafe { fake_foreign_object_ref_count(raw) }, 1);
        let clone = object.clone();
        // SAFETY: the original and clone keep the same fake allocation live.
        assert_eq!(unsafe { fake_foreign_object_ref_count(raw) }, 2);
        // SAFETY: `raw` is a borrowed live IUnknown; this constructor takes one
        // additional reference and returns the matching owned ObjectRef.
        let retained = unsafe { ObjectRef::from_raw_iunknown_addref(raw) }
            .expect("borrowed fake object pointer is non-null");
        // SAFETY: three ObjectRef owners now keep the fake allocation live.
        assert_eq!(unsafe { fake_foreign_object_ref_count(raw) }, 3);

        drop(object);
        // SAFETY: `clone` and `retained` still keep the allocation live.
        assert_eq!(unsafe { fake_foreign_object_ref_count(raw) }, 2);
        assert_eq!(drop_count.load(Ordering::Acquire), 0);
        drop(clone);
        // SAFETY: `retained` still owns the final live reference.
        assert_eq!(unsafe { fake_foreign_object_ref_count(raw) }, 1);
        assert_eq!(drop_count.load(Ordering::Acquire), 0);
        drop(retained);
        assert_eq!(drop_count.load(Ordering::Acquire), 1);
        assert_eq!(Arc::strong_count(&drop_count), 1);
    }

    /// Elaborate a hand-built `CoreProgram` and run it on vm3 with a chosen host.
    fn run_core_with_host<'h>(prog: &CoreProgram, host: &'h dyn HostServices) -> Vm3<'h> {
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(prog).expect("elaborate"),
        ));
        Vm3::run(oxp, host).expect("vm3 run")
    }

    /// A one-arg `Declare` descriptor (`descriptor_id` 0) whose single parameter is `ty`,
    /// `ByRef` per `by_ref`. The deterministic mock host ignores most of these fields.
    fn declare_descriptor(ty: DeclareParamType, by_ref: bool) -> ExternalCallDescriptor {
        ExternalCallDescriptor {
            descriptor_id: 0,
            declared_name: "Bump".into(),
            library: "t".into(),
            alias: "Bump".into(),
            ordinal_alias: false,
            symbol: DynLinkSymbol::new(0),
            marshal_lane: "m0-deterministic".into(),
            calling_convention: "platform-default".into(),
            selection_policy: "case-insensitive-canonical".into(),
            param_count: 1,
            param_types: vec![ty],
            param_by_ref: vec![by_ref],
            return_type: None,
        }
    }

    #[test]
    fn foreach_over_foreign_object_surfaces_enumeration_failure() {
        let obj = || CorePlace::Local(CoreLocalId(0));
        let item = || CorePlace::Local(CoreLocalId(1));
        let touched = || CorePlace::Local(CoreLocalId(2));
        let err_number = || CorePlace::Local(CoreLocalId(3));
        let prog = main_proc(
            vec![
                local("obj", VarTypeRef::Variant),
                local("item", VarTypeRef::Variant),
                local("touched", VarTypeRef::Builtin(BuiltinType::Long)),
                local("err_number", VarTypeRef::Builtin(BuiltinType::Long)),
            ],
            vec![
                assign(
                    obj(),
                    CoreValue::Call {
                        callee: CoreCallee::Native(NativeImplId::CreateObject),
                        args: vec![CoreArg::ByVal(CoreValue::Const(CoreConst::Str(
                            "Probe.Object".into(),
                        )))],
                    },
                ),
                assign(touched(), CoreValue::Const(CoreConst::I32(0))),
                CoreStmt::Error(ErrorOp::OnErrorResumeNext),
                CoreStmt::ForEach {
                    item: item(),
                    source: CoreValue::Load(obj()),
                    body: vec![assign(touched(), CoreValue::Const(CoreConst::I32(1)))],
                },
                assign(err_number(), CoreValue::ErrField(ErrField::Number)),
            ],
        );
        let host = MockComEnumerateFailHost::new();
        let vm = run_core_with_host(&prog, &host);
        assert_eq!(
            vm.slot(2).and_then(|v| v.as_i32()),
            Some(0),
            "the failed For Each body must not run"
        );
        assert_eq!(
            vm.slot(3).and_then(|v| v.as_i32()),
            Some(438),
            "host enumeration failure must populate Err.Number, not become an empty loop"
        );
    }

    #[test]
    fn declare_byref_writes_back_to_the_call_site_slot() {
        // Declare Sub Bump Lib "t" (ByRef n As Long); r = 5; Bump r  ->  r = 105 (the mock
        // dynlink echoes +100). Proves the write-back targets the caller's `ByRef` slot.
        let mut prog = main_proc(
            vec![local("r", VarTypeRef::Builtin(BuiltinType::Long))],
            vec![
                assign(lc(0), CoreValue::Const(CoreConst::I32(5))),
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::Declare {
                        descriptor_id: 0,
                        ptr_writebacks: Vec::new(),
                    },
                    args: vec![CoreArg::ByRef(lc(0))],
                }),
            ],
        );
        prog.external_calls = vec![declare_descriptor(DeclareParamType::Long, true)];
        let host = MockDynlinkHost::new(0);
        let vm = run_core_with_host(&prog, &host);
        assert_eq!(
            vm.slot(0).and_then(|v| v.as_i32()),
            Some(105),
            "a Declare ByRef arg must write the marshaled-back value to the caller slot"
        );
    }

    #[test]
    fn a_declare_refreshes_err_last_dll_error() {
        // After a Declare call, Err.LastDllError reads the OS last-error the HAL captured.
        let mut prog = main_proc(
            vec![local("e", VarTypeRef::Builtin(BuiltinType::Long))],
            vec![
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::Declare {
                        descriptor_id: 0,
                        ptr_writebacks: Vec::new(),
                    },
                    args: vec![CoreArg::ByVal(CoreValue::Const(CoreConst::I32(1)))],
                }),
                assign(lc(0), CoreValue::ErrField(ErrField::LastDllError)),
            ],
        );
        prog.external_calls = vec![declare_descriptor(DeclareParamType::Long, false)];
        let host = MockDynlinkHost::new(2026);
        let vm = run_core_with_host(&prog, &host);
        assert_eq!(
            vm.slot(0).and_then(|v| v.as_i32()),
            Some(2026),
            "Err.LastDllError must reflect the dynlink HAL's last-error after a Declare"
        );
    }

    #[test]
    fn strptr_pins_and_a_declare_writeback_round_trips_the_string() {
        // s = "hi"; Declare Sub Poke Lib "t" (ByVal p As LongPtr); Poke StrPtr(s) with a
        // String pointer-helper write-back into `s`. The mock makes no native mutation, so
        // the pinned UTF-16 buffer reads back unchanged — proving StrPtr registration + the
        // pointer read-back path (and that the pin survives until after the write-back).
        let mut prog = main_proc(
            vec![local("s", VarTypeRef::Builtin(BuiltinType::String))],
            vec![
                assign(lc(0), CoreValue::Const(CoreConst::Str("hi".into()))),
                CoreStmt::Eval(CoreValue::Call {
                    callee: CoreCallee::Declare {
                        descriptor_id: 0,
                        ptr_writebacks: vec![PtrWriteback {
                            arg_index: 0,
                            target: lc(0),
                            kind: PtrWritebackKind::String,
                        }],
                    },
                    args: vec![CoreArg::ByVal(CoreValue::Ptr {
                        kind: PtrKind::Str,
                        value: Box::new(load(0)),
                    })],
                }),
            ],
        );
        prog.external_calls = vec![declare_descriptor(DeclareParamType::LongPtr, false)];
        let host = MockDynlinkHost::new(0);
        let vm = run_core_with_host(&prog, &host);
        let s = oxvba_runtime::variant_to_vba_string(&vm.slot(0).expect("s"))
            .map(|b| b.as_str().to_string())
            .unwrap_or_default();
        assert_eq!(
            s, "hi",
            "the StrPtr pin must read back the source string unchanged"
        );
    }

    #[test]
    fn call_proc_ref_dispatches_through_address_of() {
        // A procedure reference (AddressOf, materialized by LoadProcRef) is called through
        // CallProcRef: Double(21) = 42 via a runtime-resolved proc index. These ops are
        // latent in the front-end (no producer yet) but consumed by the JIT, so the program
        // is built directly in OxIR. Mirrors vm2's `call_proc_ref_dispatches_through_address_of`.
        fn ox_local(name: &str, ty: OxTy, param: Option<OxParamInfo>) -> OxLocal {
            OxLocal {
                name: name.into(),
                ty,
                array_element: None,
                param,
                escaped: false,
            }
        }
        let main = OxFunc {
            name: "Main".into(),
            kind: ProcedureKind::Sub,
            locals: vec![
                ox_local("arg", OxTy::Long, None),  // Local 0
                ox_local("f", OxTy::ProcRef, None), // Local 1 (the AddressOf value)
                ox_local("n", OxTy::Long, None),    // Local 2 (result, snapshot slot 2)
            ],
            temps: Vec::new(),
            param_count: 0,
            return_local: None,
            blocks: vec![OxBlock {
                id: BlockId(0),
                instrs: vec![
                    OxInst::Assign {
                        dst: OxPlace::Local(LocalId(0)),
                        value: OxOperand::Const(OxConst::I32(21)),
                    },
                    OxInst::LoadProcRef {
                        dst: OxPlace::Local(LocalId(1)),
                        proc: FuncId(1),
                    },
                    OxInst::CallProcRef {
                        dst: Some(OxPlace::Local(LocalId(2))),
                        target: OxOperand::local(LocalId(1)),
                        args: vec![OxArg::ByVal(OxOperand::local(LocalId(0)))],
                    },
                ],
                fault_target: None,
                terminator: OxTerminator::Return,
            }],
            entry: BlockId(0),
        };
        let double = OxFunc {
            name: "Double".into(),
            kind: ProcedureKind::Function,
            locals: vec![
                ox_local(
                    "x",
                    OxTy::Long,
                    Some(OxParamInfo {
                        optional: false,
                        by_ref: false,
                        variadic: false,
                    }),
                ), // Local 0 (param)
                ox_local("Double", OxTy::Long, None), // Local 1 (return)
            ],
            temps: Vec::new(),
            param_count: 1,
            return_local: Some(LocalId(1)),
            blocks: vec![OxBlock {
                id: BlockId(0),
                instrs: vec![OxInst::Arith {
                    dst: OxPlace::Local(LocalId(1)),
                    op: ArithOp::Add,
                    lhs: OxOperand::local(LocalId(0)),
                    rhs: OxOperand::local(LocalId(0)),
                    mode: NumericMode::Widening,
                }],
                fault_target: None,
                terminator: OxTerminator::Return,
            }],
            entry: BlockId(0),
        };
        let program = OxProgram {
            funcs: vec![main, double],
            entry: Some(FuncId(0)),
            unit_name: "T".into(),
            ..Default::default()
        };
        let oxp: &'static OxProgram = Box::leak(Box::new(program));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        let vm = Vm3::run(oxp, host).expect("vm3 run");
        assert_eq!(
            vm.slot(2).and_then(|v| v.as_i32()),
            Some(42),
            "CallProcRef must dispatch through the AddressOf proc reference"
        );
    }

    #[test]
    fn call_native_typename_routes_through_the_veneer() {
        // Sub Main() : n = TypeName("hi")  ->  "String". Proves CallNative now goes
        // through the `invoke_native_lib` veneer (which only intercepts an *object*
        // argument, mirroring vm2): a non-object argument skips the interception and the
        // pure library body still answers correctly.
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Variant)],
            None,
            vec![assign(
                lc(0),
                CoreValue::Call {
                    callee: CoreCallee::Native(NativeImplId::TypeName),
                    args: vec![CoreArg::ByVal(CoreValue::Const(CoreConst::Str(
                        "hi".into(),
                    )))],
                },
            )],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        let s = oxvba_runtime::variant_to_vba_string(&vm.slot(0).expect("n"))
            .map(|b| b.as_str())
            .unwrap_or_default();
        assert_eq!(s, "String");
    }

    #[test]
    fn recursion_is_bounded_not_a_stack_overflow() {
        // Sub Spin() : Spin()  — unbounded self-recursion must surface as VBA error 28
        // ("Out of stack space"), not a native stack overflow / panic.
        let spin = proc(
            "Spin",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            vec![CoreStmt::Eval(CoreValue::Call {
                callee: CoreCallee::VbaProc { proc: ProcId(0) },
                args: Vec::new(),
            })],
        );
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(&procs_program(vec![spin])).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        match Vm3::run(oxp, host) {
            Err(Vm3Error::Fault(f)) => assert_eq!(f.code, 28, "deep recursion is error 28"),
            Err(other) => panic!("expected an Out-of-stack fault, got error: {other}"),
            Ok(_) => panic!("expected an Out-of-stack fault, but the run completed"),
        }
    }

    // ── M2-c: error / Resume / Err / GoSub model ────────────────────────────────

    /// Run a single-proc program and expect it to end with an uncaught fault of `code`.
    fn run_expecting_fault(prog: &CoreProgram, code: i32) {
        let oxp: &'static OxProgram = Box::leak(Box::new(
            oxvba_oxir::elaborate::elaborate(prog).expect("elaborate"),
        ));
        let host: &'static NullHostServices =
            Box::leak(Box::new(NullHostServices::new(HostPolicy::default())));
        match Vm3::run(oxp, host) {
            Err(Vm3Error::Fault(f)) => assert_eq!(f.code, code, "expected uncaught error {code}"),
            Err(other) => panic!("expected uncaught error {code}, got error: {other}"),
            Ok(_) => panic!("expected uncaught error {code}, but the run completed"),
        }
    }

    /// `1 / 0` — a division-by-zero (error 11) expression.
    fn div_by_zero() -> CoreValue {
        CoreValue::Binary {
            op: CoreBinOp::Div,
            lhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
            rhs: Box::new(CoreValue::Const(CoreConst::I32(0))),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        }
    }

    #[test]
    fn on_error_resume_next_continues_and_reads_err() {
        // On Error Resume Next : n = 1/0 : n = Err.Number  ->  n = 11 (Resume Next skips
        // past the faulting statement; Err carries the division-by-zero code).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![
                CoreStmt::Error(ErrorOp::OnErrorResumeNext),
                assign(lc(0), div_by_zero()),
                assign(lc(0), CoreValue::ErrField(ErrField::Number)),
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(11));
    }

    #[test]
    fn err_raise_is_caught_by_on_error_goto() {
        // On Error GoTo H : Err.Raise 5 : Exit Sub : H: n = Err.Number  ->  n = 5
        // (proves Err.Raise routes through the statement pad so On Error catches it).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("n", VarTypeRef::Builtin(BuiltinType::Long))],
            None,
            vec![
                CoreStmt::Error(ErrorOp::OnErrorGotoLabel(LabelId(0))),
                CoreStmt::Error(ErrorOp::Raise {
                    number: CoreValue::Const(CoreConst::I32(5)),
                    source: None,
                    description: None,
                    help_file: None,
                    help_context: None,
                    inherit: true,
                }),
                CoreStmt::Exit(ExitKind::Proc),
                CoreStmt::Label(LabelId(0)),
                assign(lc(0), CoreValue::ErrField(ErrField::Number)),
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(5));
    }

    #[test]
    fn resume_re_runs_the_faulting_statement() {
        // On Error GoTo H : k = k+1 : m = 1/(k-1) [faults when k=1] : n = 42 : Exit Sub
        // H: k = k+1 : Resume   ->  the division statement re-runs with k=2 (1/1 ok), so
        // control reaches `n = 42`. `Resume` re-enters the *faulting* statement, and the
        // handler's k-bump prevents an infinite re-fault. (`n` is a clean Long literal so
        // the assertion is unaffected by the Double division result, which lands in `m`.)
        let k = || lc(2);
        let div = CoreValue::Binary {
            op: CoreBinOp::Div,
            lhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
            rhs: Box::new(CoreValue::Binary {
                op: CoreBinOp::Sub,
                lhs: Box::new(load(2)),
                rhs: Box::new(CoreValue::Const(CoreConst::I32(1))),
                mode: StringCompareMode::Binary,
                num: NumericMode::Widening,
            }),
            mode: StringCompareMode::Binary,
            num: NumericMode::Widening,
        };
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![
                local("n", VarTypeRef::Builtin(BuiltinType::Long)), // 0
                local("m", VarTypeRef::Variant),                    // 1
                local("k", VarTypeRef::Builtin(BuiltinType::Long)), // 2
            ],
            None,
            vec![
                CoreStmt::Error(ErrorOp::OnErrorGotoLabel(LabelId(0))),
                assign(k(), long_add(load(2), CoreValue::Const(CoreConst::I32(1)))),
                assign(lc(1), div), // faulting statement: 1/(k-1)
                assign(lc(0), CoreValue::Const(CoreConst::I32(42))),
                CoreStmt::Exit(ExitKind::Proc),
                CoreStmt::Label(LabelId(0)),
                assign(k(), long_add(load(2), CoreValue::Const(CoreConst::I32(1)))),
                CoreStmt::Error(ErrorOp::Resume),
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        assert_eq!(vm.slot(0).and_then(|v| v.as_i32()), Some(42));
    }

    #[test]
    fn resume_without_active_error_raises_20() {
        // Resume Next with no active error -> runtime error 20 (Resume without error).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            vec![CoreStmt::Error(ErrorOp::ResumeNext)],
        );
        run_expecting_fault(&procs_program(vec![main]), 20);
    }

    #[test]
    fn return_without_gosub_raises_3() {
        // A bare Return (no GoSub on the stack) -> runtime error 3 (Return without GoSub).
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            Vec::new(),
            None,
            vec![CoreStmt::GoSubReturn],
        );
        run_expecting_fault(&procs_program(vec![main]), 3);
    }

    #[test]
    fn gosub_resumption_list_is_lifo() {
        // GoSub A : Exit Sub : A: GoSub B : s = s & "A" : Return : B: s = s & "B" : Return
        // -> s = "BA": B's Return pops the inner ret, A's Return pops the outer (LIFO).
        let s = || lc(0);
        let cat = |suffix: &str| {
            assign(
                s(),
                CoreValue::Binary {
                    op: CoreBinOp::Concat,
                    lhs: Box::new(load(0)),
                    rhs: Box::new(CoreValue::Const(CoreConst::Str(suffix.into()))),
                    mode: StringCompareMode::Binary,
                    num: NumericMode::Widening,
                },
            )
        };
        let main = proc(
            "Main",
            ProcedureKind::Sub,
            Vec::new(),
            vec![local("s", VarTypeRef::Builtin(BuiltinType::String))],
            None,
            vec![
                CoreStmt::GoSub(LabelId(0)),
                CoreStmt::Exit(ExitKind::Proc),
                CoreStmt::Label(LabelId(0)), // A
                CoreStmt::GoSub(LabelId(1)),
                cat("A"),
                CoreStmt::GoSubReturn,
                CoreStmt::Label(LabelId(1)), // B
                cat("B"),
                CoreStmt::GoSubReturn,
            ],
        );
        let prog = procs_program(vec![main]);
        let vm = run_core(&prog);
        let s = oxvba_runtime::variant_to_vba_string(&vm.slot(0).expect("s"))
            .map(|b| b.as_str())
            .unwrap_or_default();
        assert_eq!(s, "BA");
    }
}
