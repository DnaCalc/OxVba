//! Shared runtime ABI substrate for vm3 and the Cranelift JIT.
//!
//! This crate is intentionally free of Cranelift and interpreter-frame state. It hosts
//! the VM-agnostic cells and helper decisions that both engines must share.

use oxvba_bundle::{
    ArrayElementType, NativeImplId, NumericCoerceTarget, NumericMode, ProjectMemberKind,
    StringCompareMode,
};
use oxvba_com::ComSubscriptionToken;
use oxvba_eval::arith::{self, ArithError, CmpOp as EvalCmpOp};
use oxvba_eval::typed;
use oxvba_hal::HostServices;
use oxvba_lib::LibContext;
use oxvba_oxir::value::{OxArg, OxCallArg, OxOperand, OxPlace};
use oxvba_oxir::{
    BlockId, ErrorHandler, FuncId, OxProgram, OxTy, inst::OxAsNew, program::OxClassMethod,
};
use oxvba_runtime::object_ref::{
    ObjectRef, RUNTIME_IDISPATCH_INTERFACE_IDENTITY, RUNTIME_IUNKNOWN_INTERFACE_DESCRIPTOR,
    RuntimeClassActivationDescriptor, RuntimeClassAsNewFieldDescriptor, RuntimeClassDescriptor,
    RuntimeClassFieldDescriptor, RuntimeClassLifecycleDescriptor, RuntimeGuid,
    RuntimeInterfaceDescriptor, RuntimeInterfaceId, RuntimeInterfaceIdentity, RuntimeInterfaceKind,
    RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeParamDescriptor,
    RuntimeProjectClassIdentity, RuntimeValueType,
};
use oxvba_runtime::{VarType, Variant};
use std::collections::HashMap;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::slice;

/// Project-instance ids start above class route keys, matching vm3/vm2 allocation.
pub const INSTANCE_ID_BASE: i32 = 0x1000_0000;

/// A run-time fault carrying the `Err` state it populates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub code: i32,
    pub message: String,
    pub source: Option<String>,
    pub help_file: Option<String>,
    pub help_context: Option<i32>,
}

impl Fault {
    /// A fault with an explicit VBA error code and message.
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
            help_file: None,
            help_context: None,
        }
    }

    /// A fault from the shared arithmetic/coercion kernel.
    pub fn from_arith(e: ArithError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            source: None,
            help_file: None,
            help_context: None,
        }
    }

    /// A bare untyped runtime-helper message: VBA type mismatch (13).
    pub fn from_string(message: String) -> Self {
        Self::new(13, message)
    }

    /// A built-in library error already carries its VBA error code structurally.
    pub fn from_lib(e: oxvba_lib::LibError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            source: None,
            help_file: None,
            help_context: None,
        }
    }

    /// A HAL fault carries the real VBA `Err.Number` recovered from the host when
    /// available, falling back to error 5.
    pub fn from_hal(e: oxvba_hal::HalError) -> Self {
        Self {
            code: e.host_error_code.unwrap_or(5),
            message: e.message,
            source: None,
            help_file: None,
            help_context: None,
        }
    }
}

/// The active `On Error` handler policy of a procedure activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorMode {
    None,
    ResumeNext,
    Goto(BlockId),
}

/// The seeds a `Resume`/`Resume Next` uses from a caught fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumePoint {
    pub resume: BlockId,
    pub resume_next: BlockId,
    pub handler: ErrorMode,
}

/// The `Err` object's observable state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ErrState {
    pub number: i32,
    pub description: String,
    pub source: String,
    pub help_file: String,
    pub help_context: i32,
    pub inherit_fields: bool,
}

/// Error-policy cells saved at an activation boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedErrState {
    pub error_mode: ErrorMode,
    pub active_error: Option<ResumePoint>,
}

/// Result of a landing pad's fault-dispatch decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaultAction {
    Propagate(Fault),
    ResumeNext(BlockId),
    Handle(BlockId),
}

/// The target form requested by a `Resume` terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeTarget {
    Same,
    Next,
    Label(BlockId),
}

/// Shared Err/On Error state and routing decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrEngine {
    pub error_mode: ErrorMode,
    pub active_error: Option<ResumePoint>,
    pub err: ErrState,
    pub erl_line: i32,
    pub last_dll_error: i32,
    pub pending_fault: Option<Fault>,
}

/// A `WithEvents` binding shared by interpreter and JIT event ingress.
pub struct EventBinding {
    pub owner: Variant,
    pub source: Variant,
    pub order: u64,
}

/// A live host event subscription routed back to a project sink handler.
pub struct ComEventSink {
    pub owner: Variant,
    pub handler: usize,
}

/// Host sink registered by COM server/export code for project `RaiseEvent` fan-out.
pub type ProjectEventSink<'h> =
    Box<dyn FnMut(ObjectRef, i32, Vec<Variant>) -> Result<(), String> + 'h>;

/// Shared event fabric state. Procedure execution remains engine-owned; the cells that
/// describe subscriptions, ordering, and host sinks are VM-agnostic session state.
pub struct EventFabric<'h> {
    pub withevents: HashMap<i64, EventBinding>,
    pub next_withevents_order: u64,
    pub com_subscriptions: HashMap<i32, ComEventSink>,
    pub com_subscriptions_by_key: HashMap<i64, Vec<i32>>,
    pub pumping: bool,
    pub withevents_iters: Vec<(Vec<ObjectRef>, usize)>,
    pub project_event_sink: Option<ProjectEventSink<'h>>,
}

impl<'h> Default for EventFabric<'h> {
    fn default() -> Self {
        Self {
            withevents: HashMap::new(),
            next_withevents_order: 0,
            com_subscriptions: HashMap::new(),
            com_subscriptions_by_key: HashMap::new(),
            pumping: false,
            withevents_iters: Vec::new(),
            project_event_sink: None,
        }
    }
}

pub type ProcInvokeFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    target_prog: usize,
    proc: usize,
    me: *const Variant,
    suppress: i32,
) -> i32;

#[derive(Clone, Copy)]
pub struct ProcInvokeBridge {
    pub ctx: *mut c_void,
    pub invoke: ProcInvokeFn,
}

/// One linked VBA project image's mutable runtime tables.
pub struct LoadedProgram<'h> {
    pub program: &'h OxProgram,
    pub globals: Vec<Variant>,
    pub class_descriptors: Vec<&'static RuntimeClassDescriptor>,
    pub predeclared_singletons: HashMap<usize, Variant>,
    pub event_routes: HashMap<(i32, i32), usize>,
}

pub fn runtime_class_descriptors_for_program(
    program: &OxProgram,
) -> Vec<&'static RuntimeClassDescriptor> {
    program
        .classes
        .iter()
        .enumerate()
        .map(|(class_index, class)| build_runtime_class_descriptor(program, class_index, class))
        .collect()
}

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

fn runtime_class_field_descriptor(
    field: &oxvba_oxir::program::OxClassField,
) -> RuntimeClassFieldDescriptor {
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

fn runtime_member_descriptor(
    program: &OxProgram,
    method: &OxClassMethod,
    display_name: &str,
    dispatch_index: usize,
    dispatch_id: Option<i32>,
    vtable_slot: Option<u16>,
    is_default_member: bool,
    is_enumerator_member: bool,
) -> RuntimeMemberDescriptor {
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
    RuntimeGuid::new(
        (hi >> 32) as u32,
        (hi >> 16) as u16,
        hi as u16,
        lo.to_be_bytes(),
    )
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
        members.push(runtime_member_descriptor(
            program,
            implementation_method,
            &interface_method.name,
            index,
            interface_method.dispid.or(implementation_method.dispid),
            interface_method
                .vtable_slot
                .or(implementation_method.vtable_slot),
            interface_method.is_default_member || implementation_method.is_default_member,
            interface_method.is_enumerator_member || implementation_method.is_enumerator_member,
        ));
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
                runtime_member_descriptor(
                    program,
                    method,
                    &method.name,
                    index,
                    method.dispid,
                    method.vtable_slot,
                    method.is_default_member,
                    method.is_enumerator_member,
                )
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
    interface_descriptors.extend(
        class.implements.iter().filter_map(|interface| {
            runtime_project_interface_descriptor(program, class, interface)
        }),
    );
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

/// Shared procedure invocation seam used by host/event ingress and future JIT tiering.
///
/// The implementor owns frames and instruction dispatch. The ABI layer owns only the
/// call boundary shape: target project, procedure id, optional `Me`/argument values,
/// fault suppression policy, and result carrier.
pub trait ProcInvoker {
    type Error;

    fn invoke_proc_with_values(
        &mut self,
        target_prog: usize,
        proc: FuncId,
        me: Variant,
        args: Vec<Variant>,
        suppress: bool,
    ) -> Result<Variant, Self::Error>;

    fn invoke_callback_proc_with_values(
        &mut self,
        target_prog: usize,
        proc: FuncId,
        args: Vec<Variant>,
        suppress: bool,
    ) -> Result<Variant, Self::Error>;

    fn maybe_drain(&mut self);
}

/// VBA `VariantChanged` equality predicate used by compound ByRef copy-out guards.
pub fn variant_changed(current: &Variant, original: &Variant) -> bool {
    current != original
}

pub enum MarshalArgRef<'a> {
    Operand(&'a OxOperand),
    ByRef(&'a OxPlace),
}

/// Collect an `OxArg` list into by-value `Variant` arguments using engine-owned readers.
pub fn marshal_ox_args<E>(
    args: &[OxArg],
    mut read: impl FnMut(MarshalArgRef<'_>) -> Result<Variant, E>,
    omitted: impl Fn() -> Variant,
) -> Result<Vec<Variant>, E> {
    args.iter()
        .map(|arg| match arg {
            OxArg::ByVal(op) => read(MarshalArgRef::Operand(op)),
            OxArg::ByRef(place) => read(MarshalArgRef::ByRef(place)),
            OxArg::Omitted => Ok(omitted()),
        })
        .collect()
}

/// Collect an `OxCallArg` list into by-value native/library arguments.
pub fn marshal_ox_call_args<E>(
    args: &[OxCallArg],
    mut read: impl FnMut(MarshalArgRef<'_>) -> Result<Variant, E>,
    omitted: impl Fn() -> Variant,
) -> Result<Vec<Variant>, E> {
    args.iter()
        .map(|arg| match arg {
            OxCallArg::Operand(op) => read(MarshalArgRef::Operand(op)),
            OxCallArg::ByRef(place) => read(MarshalArgRef::ByRef(place)),
            OxCallArg::Omitted => Ok(omitted()),
            OxCallArg::Named { value, .. } => read(MarshalArgRef::Operand(value)),
            OxCallArg::Const(value) => Ok(Variant::from_i32(*value)),
        })
        .collect()
}

/// Apply write-back values aligned to an `OxCallArg` list, writing only ByRef places.
pub fn apply_byref_writebacks<E>(
    args: &[OxCallArg],
    values: &[Variant],
    mut store: impl FnMut(&OxPlace, Variant) -> Result<(), E>,
) -> Result<(), E> {
    for (index, arg) in args.iter().enumerate() {
        if let OxCallArg::ByRef(place) = arg
            && let Some(value) = values.get(index)
        {
            store(place, value.clone())?;
        }
    }
    Ok(())
}

/// Apply optional write-back values aligned to an `OxCallArg` list.
pub fn apply_optional_byref_writebacks<E>(
    args: &[OxCallArg],
    values: &[Option<Variant>],
    mut store: impl FnMut(&OxPlace, Variant) -> Result<(), E>,
) -> Result<(), E> {
    for (index, arg) in args.iter().enumerate() {
        if let OxCallArg::ByRef(place) = arg
            && let Some(Some(value)) = values.get(index)
        {
            store(place, value.clone())?;
        }
    }
    Ok(())
}

/// One pending `Class_Terminate` callback resolved against the shared loaded-image tables.
pub struct TerminationWork {
    pub instance_id: i32,
    pub bundle: usize,
    pub terminate: Option<FuncId>,
    pub object: Option<Variant>,
}

/// Take the current pending-termination batch and resolve each item to its owning proc/object.
pub fn take_termination_batch(exec: &ExecState<'_>) -> Vec<TerminationWork> {
    oxvba_runtime::take_pending_terminations()
        .into_iter()
        .map(|(instance_id, bundle_id, route_key)| {
            let bundle = bundle_id as usize;
            let terminate = exec
                .programs
                .get(bundle)
                .and_then(|lp| lp.program.classes.get(route_key as usize))
                .and_then(|class| class.terminate);
            let object = oxvba_runtime::retained_parked_termination_object(instance_id)
                .map(Variant::from_object_ref);
            TerminationWork {
                instance_id,
                bundle,
                terminate,
                object,
            }
        })
        .collect()
}

fn withevents_owner_raw(key: i64) -> i32 {
    (key >> 32) as i32
}

fn unsubscribe_com_key(exec: &mut ExecState<'_>, key: i64) {
    if let Some(tokens) = exec.events.com_subscriptions_by_key.remove(&key) {
        for raw in tokens {
            let _ = exec
                .host
                .com()
                .unsubscribe_event_variant(ComSubscriptionToken::new(raw));
            exec.events.com_subscriptions.remove(&raw);
        }
    }
}

fn cleanup_terminated_owner(exec: &mut ExecState<'_>, owner_raw: i32) {
    let keys: Vec<i64> = exec
        .events
        .com_subscriptions_by_key
        .keys()
        .copied()
        .filter(|key| withevents_owner_raw(*key) == owner_raw)
        .collect();
    for key in keys {
        unsubscribe_com_key(exec, key);
    }
    exec.events
        .withevents
        .retain(|key, _| withevents_owner_raw(*key) != owner_raw);
}

pub fn maybe_drain_with_bridge(exec: &mut ExecState<'_>) -> Result<(), Fault> {
    if exec.draining {
        return Ok(());
    }
    let Some(bridge) = exec.proc_invoker else {
        if oxvba_runtime::has_pending_terminations() {
            return Err(Fault::new(
                5,
                "rt_maybe_drain requires an installed ProcInvoker",
            ));
        }
        return Ok(());
    };
    exec.draining = true;
    while oxvba_runtime::has_pending_terminations() {
        for work in take_termination_batch(exec) {
            if let (Some(proc), Some(object)) = (work.terminate, work.object) {
                // SAFETY: the installed bridge owns the opaque context and accepts a borrowed
                // Variant for the duration of the call. Terminate faults are suppressed.
                let _ = unsafe { (bridge.invoke)(bridge.ctx, work.bundle, proc.0, &object, 1) };
            }
            oxvba_runtime::finish_pending_termination(work.instance_id);
            cleanup_terminated_owner(exec, work.instance_id);
        }
    }
    exec.draining = false;
    Ok(())
}

/// VM-agnostic observable execution state shared by vm3 and the JIT.
///
/// This is deliberately not an interpreter frame. Stack slots, instruction pointers,
/// `For Each` cursors, and other engine-private execution mechanics stay in the owning
/// engine. Cross-engine runtime state lives here.
pub struct ExecState<'h> {
    pub programs: Vec<LoadedProgram<'h>>,
    pub default_error_source: String,
    pub err_engine: ErrEngine,
    pub events: EventFabric<'h>,
    pub lib: LibContext,
    pub host: &'h dyn HostServices,
    pub proc_invoker: Option<ProcInvokeBridge>,
    pub next_instance_id: i32,
    pub draining: bool,
    _not_send: PhantomData<Rc<()>>,
}

impl<'h> ExecState<'h> {
    pub fn new(host: &'h dyn HostServices) -> Self {
        Self {
            programs: Vec::new(),
            default_error_source: "VBAProject".to_string(),
            err_engine: ErrEngine::default(),
            events: EventFabric::default(),
            lib: LibContext::default(),
            host,
            proc_invoker: None,
            next_instance_id: INSTANCE_ID_BASE,
            draining: false,
            _not_send: PhantomData,
        }
    }
}

/// C-ABI status code: operation completed.
pub const ST_OK: i32 = 0;
/// C-ABI status code: VBA fault payload was seated into [`ExecState`].
pub const ST_FAULT: i32 = 1;
/// C-ABI status code: VBA `End`/halt propagation.
pub const ST_HALT: i32 = 2;

pub const RT_ARITH_ADD: u32 = 1;
pub const RT_ARITH_SUB: u32 = 2;
pub const RT_ARITH_MUL: u32 = 3;
pub const RT_ARITH_DIV: u32 = 4;
pub const RT_ARITH_INT_DIV: u32 = 5;
pub const RT_ARITH_MOD: u32 = 6;
pub const RT_ARITH_POW: u32 = 7;

pub const RT_LOGIC_AND: u32 = 1;
pub const RT_LOGIC_OR: u32 = 2;
pub const RT_LOGIC_XOR: u32 = 3;
pub const RT_LOGIC_EQV: u32 = 4;
pub const RT_LOGIC_IMP: u32 = 5;

pub const RT_COMPARE_EQ: u32 = 1;
pub const RT_COMPARE_NE: u32 = 2;
pub const RT_COMPARE_LT: u32 = 3;
pub const RT_COMPARE_LE: u32 = 4;
pub const RT_COMPARE_GT: u32 = 5;
pub const RT_COMPARE_GE: u32 = 6;

pub const RT_STRING_COMPARE_BINARY: u32 = 0;
pub const RT_STRING_COMPARE_TEXT: u32 = 1;

pub const RT_NUMERIC_WIDENING: u32 = 0;
pub const RT_NUMERIC_CHECKED_BYTE: u32 = 1;
pub const RT_NUMERIC_CHECKED_INTEGER: u32 = 2;
pub const RT_NUMERIC_CHECKED_LONG: u32 = 3;
pub const RT_NUMERIC_CHECKED_LONGLONG: u32 = 4;
pub const RT_NUMERIC_CHECKED_SINGLE: u32 = 5;
pub const RT_NUMERIC_CHECKED_DOUBLE: u32 = 6;
pub const RT_NUMERIC_CHECKED_CURRENCY: u32 = 7;
pub const RT_NUMERIC_CHECKED_DATE: u32 = 8;
pub const RT_NUMERIC_CHECKED_BOOLEAN: u32 = 9;

pub const RT_FAULT_DISP_UNWIND: i32 = 0;
pub const RT_FAULT_DISP_RESUME_NEXT: i32 = 1;
pub const RT_FAULT_DISP_HANDLER: i32 = 2;

pub const RT_ERROR_HANDLER_GOTO_0: u32 = 0;
pub const RT_ERROR_HANDLER_RESUME_NEXT: u32 = 1;
pub const RT_ERROR_HANDLER_GOTO_LABEL: u32 = 2;
pub const RT_ERROR_HANDLER_GOTO_MINUS_1: u32 = 3;

pub const RT_ERR_FIELD_NUMBER: u32 = 0;
pub const RT_ERR_FIELD_DESCRIPTION: u32 = 1;
pub const RT_ERR_FIELD_SOURCE: u32 = 2;
pub const RT_ERR_FIELD_HELP_FILE: u32 = 3;
pub const RT_ERR_FIELD_HELP_CONTEXT: u32 = 4;
pub const RT_ERR_FIELD_LAST_DLL_ERROR: u32 = 5;

pub const RT_RESUME_SAME: u32 = 0;
pub const RT_RESUME_NEXT: u32 = 1;
pub const RT_RESUME_LABEL: u32 = 2;

const RT_ERROR_MODE_NONE: u32 = 0;
const RT_ERROR_MODE_RESUME_NEXT: u32 = 1;
const RT_ERROR_MODE_GOTO: u32 = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RtSavedErrState {
    pub error_mode_kind: u32,
    pub error_mode_block: u32,
    pub active_error_present: u32,
    pub active_resume: u32,
    pub active_resume_next: u32,
    pub active_handler_kind: u32,
    pub active_handler_block: u32,
}

/// Opaque raw pointer type used by C-ABI shims.
pub enum RawExecState {}

pub fn exec_state_as_raw(state: &mut ExecState<'_>) -> *mut RawExecState {
    state as *mut ExecState<'_> as *mut RawExecState
}

fn with_status(work: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or(ST_FAULT)
}

unsafe fn state_from_raw<'a>(state: *mut RawExecState) -> Option<&'a mut ExecState<'a>> {
    if state.is_null() {
        None
    } else {
        // SAFETY: callers pass pointers produced from a live `ExecState`; the ABI treats
        // the state as opaque and same-thread.
        Some(unsafe { &mut *(state as *mut ExecState<'a>) })
    }
}

fn seat_fault(state: *mut RawExecState, fault: Fault) -> i32 {
    // SAFETY: the raw pointer is validated by `state_from_raw`.
    if let Some(state) = unsafe { state_from_raw(state) } {
        state
            .err_engine
            .raise(fault, state.default_error_source.clone());
    }
    ST_FAULT
}

fn write_out<T>(state: *mut RawExecState, out: *mut T, value: T) -> i32 {
    if out.is_null() {
        return seat_fault(state, Fault::new(5, "runtime ABI output pointer is null"));
    }
    // SAFETY: null was rejected and the ABI requires `out` to name writable storage for `T`.
    unsafe {
        *out = value;
    }
    ST_OK
}

fn read_in<'a, T>(state: *mut RawExecState, input: *const T, name: &str) -> Result<&'a T, i32> {
    if input.is_null() {
        Err(seat_fault(
            state,
            Fault::new(5, format!("runtime ABI {name} pointer is null")),
        ))
    } else {
        // SAFETY: null was rejected and the ABI requires `input` to name a live `T`.
        Ok(unsafe { &*input })
    }
}

fn numeric_mode_from_raw(raw: u32) -> Result<NumericMode, Fault> {
    let mode = match raw {
        RT_NUMERIC_WIDENING => NumericMode::Widening,
        RT_NUMERIC_CHECKED_BYTE => NumericMode::Checked(NumericCoerceTarget::Byte),
        RT_NUMERIC_CHECKED_INTEGER => NumericMode::Checked(NumericCoerceTarget::Integer),
        RT_NUMERIC_CHECKED_LONG => NumericMode::Checked(NumericCoerceTarget::Long),
        RT_NUMERIC_CHECKED_LONGLONG => NumericMode::Checked(NumericCoerceTarget::LongLong),
        RT_NUMERIC_CHECKED_SINGLE => NumericMode::Checked(NumericCoerceTarget::Single),
        RT_NUMERIC_CHECKED_DOUBLE => NumericMode::Checked(NumericCoerceTarget::Double),
        RT_NUMERIC_CHECKED_CURRENCY => NumericMode::Checked(NumericCoerceTarget::Currency),
        RT_NUMERIC_CHECKED_DATE => NumericMode::Checked(NumericCoerceTarget::Date),
        RT_NUMERIC_CHECKED_BOOLEAN => NumericMode::Checked(NumericCoerceTarget::Boolean),
        _ => return Err(Fault::new(5, format!("unknown numeric mode {raw}"))),
    };
    Ok(mode)
}

fn numeric_target_from_raw(raw: u32) -> Result<NumericCoerceTarget, Fault> {
    let target = match raw {
        RT_NUMERIC_CHECKED_BYTE => NumericCoerceTarget::Byte,
        RT_NUMERIC_CHECKED_INTEGER => NumericCoerceTarget::Integer,
        RT_NUMERIC_CHECKED_LONG => NumericCoerceTarget::Long,
        RT_NUMERIC_CHECKED_LONGLONG => NumericCoerceTarget::LongLong,
        RT_NUMERIC_CHECKED_SINGLE => NumericCoerceTarget::Single,
        RT_NUMERIC_CHECKED_DOUBLE => NumericCoerceTarget::Double,
        RT_NUMERIC_CHECKED_CURRENCY => NumericCoerceTarget::Currency,
        RT_NUMERIC_CHECKED_DATE => NumericCoerceTarget::Date,
        RT_NUMERIC_CHECKED_BOOLEAN => NumericCoerceTarget::Boolean,
        _ => return Err(Fault::new(5, format!("unknown numeric target {raw}"))),
    };
    Ok(target)
}

fn compare_op_from_raw(raw: u32) -> Result<EvalCmpOp, Fault> {
    let op = match raw {
        RT_COMPARE_EQ => EvalCmpOp::Eq,
        RT_COMPARE_NE => EvalCmpOp::Ne,
        RT_COMPARE_LT => EvalCmpOp::Lt,
        RT_COMPARE_LE => EvalCmpOp::Le,
        RT_COMPARE_GT => EvalCmpOp::Gt,
        RT_COMPARE_GE => EvalCmpOp::Ge,
        _ => return Err(Fault::new(5, format!("unknown compare op {raw}"))),
    };
    Ok(op)
}

fn string_compare_mode_from_raw(raw: u32) -> Result<StringCompareMode, Fault> {
    let mode = match raw {
        RT_STRING_COMPARE_BINARY => StringCompareMode::Binary,
        RT_STRING_COMPARE_TEXT => StringCompareMode::Text,
        _ => return Err(Fault::new(5, format!("unknown string compare mode {raw}"))),
    };
    Ok(mode)
}

fn error_mode_to_raw(mode: ErrorMode) -> (u32, u32) {
    match mode {
        ErrorMode::None => (RT_ERROR_MODE_NONE, 0),
        ErrorMode::ResumeNext => (RT_ERROR_MODE_RESUME_NEXT, 0),
        ErrorMode::Goto(block) => (RT_ERROR_MODE_GOTO, block.0 as u32),
    }
}

fn error_mode_from_raw(kind: u32, block: u32) -> Result<ErrorMode, Fault> {
    match kind {
        RT_ERROR_MODE_NONE => Ok(ErrorMode::None),
        RT_ERROR_MODE_RESUME_NEXT => Ok(ErrorMode::ResumeNext),
        RT_ERROR_MODE_GOTO => Ok(ErrorMode::Goto(BlockId(block as usize))),
        _ => Err(Fault::new(5, format!("unknown error mode {kind}"))),
    }
}

fn saved_err_to_raw(saved: SavedErrState) -> RtSavedErrState {
    let (error_mode_kind, error_mode_block) = error_mode_to_raw(saved.error_mode);
    let mut raw = RtSavedErrState {
        error_mode_kind,
        error_mode_block,
        ..RtSavedErrState::default()
    };
    if let Some(active) = saved.active_error {
        let (active_handler_kind, active_handler_block) = error_mode_to_raw(active.handler);
        raw.active_error_present = 1;
        raw.active_resume = active.resume.0 as u32;
        raw.active_resume_next = active.resume_next.0 as u32;
        raw.active_handler_kind = active_handler_kind;
        raw.active_handler_block = active_handler_block;
    }
    raw
}

fn saved_err_from_raw(raw: RtSavedErrState) -> Result<SavedErrState, Fault> {
    let error_mode = error_mode_from_raw(raw.error_mode_kind, raw.error_mode_block)?;
    let active_error = match raw.active_error_present {
        0 => None,
        1 => Some(ResumePoint {
            resume: BlockId(raw.active_resume as usize),
            resume_next: BlockId(raw.active_resume_next as usize),
            handler: error_mode_from_raw(raw.active_handler_kind, raw.active_handler_block)?,
        }),
        other => {
            return Err(Fault::new(
                5,
                format!("unknown active error presence flag {other}"),
            ));
        }
    };
    Ok(SavedErrState {
        error_mode,
        active_error,
    })
}

fn release_variant_slot(value: *mut Variant, expected: Option<VarType>) -> i32 {
    if value.is_null() {
        return ST_OK;
    }
    // SAFETY: null was rejected and the ABI requires unique access to the slot.
    unsafe {
        if expected.is_none_or(|ty| (*value).vtype() == ty) {
            *value = Variant::empty();
        }
    }
    ST_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_add_i32(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_add(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i32)
        {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_sub_i32(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_sub(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i32)
        {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_mul_i32(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_mul(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i32)
        {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_div_i32(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_binop(typed::CheckedIntBinOp::Div, i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i32)
        {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_rem_i32(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_binop(typed::CheckedIntBinOp::Rem, i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i32)
        {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_add_i16(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_add(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i16)
        {
            Ok(value) => write_out(state, out, i32::from(value)),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_sub_i16(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_sub(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i16)
        {
            Ok(value) => write_out(state, out, i32::from(value)),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_mul_i16(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_mul(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_i16)
        {
            Ok(value) => write_out(state, out, i32::from(value)),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_add_u8(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_add(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_u8)
        {
            Ok(value) => write_out(state, out, i32::from(value)),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_sub_u8(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_sub(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_u8)
        {
            Ok(value) => write_out(state, out, i32::from(value)),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_mul_u8(state: *mut RawExecState, lhs: i32, rhs: i32, out: *mut i32) -> i32 {
    with_status(|| {
        match typed::checked_i64_mul(i64::from(lhs), i64::from(rhs))
            .and_then(typed::narrow_i64_to_u8)
        {
            Ok(value) => write_out(state, out, i32::from(value)),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_add_i64(state: *mut RawExecState, lhs: i64, rhs: i64, out: *mut i64) -> i32 {
    with_status(|| match typed::checked_i64_add(lhs, rhs) {
        Ok(value) => write_out(state, out, value),
        Err(err) => seat_fault(state, Fault::from_arith(err)),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_sub_i64(state: *mut RawExecState, lhs: i64, rhs: i64, out: *mut i64) -> i32 {
    with_status(|| match typed::checked_i64_sub(lhs, rhs) {
        Ok(value) => write_out(state, out, value),
        Err(err) => seat_fault(state, Fault::from_arith(err)),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_mul_i64(state: *mut RawExecState, lhs: i64, rhs: i64, out: *mut i64) -> i32 {
    with_status(|| match typed::checked_i64_mul(lhs, rhs) {
        Ok(value) => write_out(state, out, value),
        Err(err) => seat_fault(state, Fault::from_arith(err)),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_div_i64(state: *mut RawExecState, lhs: i64, rhs: i64, out: *mut i64) -> i32 {
    with_status(
        || match typed::checked_i64_binop(typed::CheckedIntBinOp::Div, lhs, rhs) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_rem_i64(state: *mut RawExecState, lhs: i64, rhs: i64, out: *mut i64) -> i32 {
    with_status(
        || match typed::checked_i64_binop(typed::CheckedIntBinOp::Rem, lhs, rhs) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_currency_add(
    state: *mut RawExecState,
    lhs_scaled: i64,
    rhs_scaled: i64,
    out_scaled: *mut i64,
) -> i32 {
    with_status(
        || match typed::currency_add_scaled(lhs_scaled, rhs_scaled) {
            Ok(value) => write_out(state, out_scaled, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_currency_sub(
    state: *mut RawExecState,
    lhs_scaled: i64,
    rhs_scaled: i64,
    out_scaled: *mut i64,
) -> i32 {
    with_status(
        || match typed::currency_sub_scaled(lhs_scaled, rhs_scaled) {
            Ok(value) => write_out(state, out_scaled, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_currency_mul(
    state: *mut RawExecState,
    lhs_scaled: i64,
    rhs_scaled: i64,
    out_scaled: *mut i64,
) -> i32 {
    with_status(|| {
        match typed::currency_mul_scaled_i128(i128::from(lhs_scaled), i128::from(rhs_scaled)) {
            Ok(value) => write_out(state, out_scaled, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_arith_v(
    state: *mut RawExecState,
    op: u32,
    mode: u32,
    lhs: *const Variant,
    rhs: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let lhs = match read_in(state, lhs, "lhs") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let rhs = match read_in(state, rhs, "rhs") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let mode = match numeric_mode_from_raw(mode) {
            Ok(mode) => mode,
            Err(fault) => return seat_fault(state, fault),
        };
        let result = match op {
            RT_ARITH_ADD => arith::add(lhs, rhs, mode),
            RT_ARITH_SUB => arith::sub(lhs, rhs, mode),
            RT_ARITH_MUL => arith::mul(lhs, rhs, mode),
            RT_ARITH_DIV => arith::div(lhs, rhs),
            RT_ARITH_INT_DIV => arith::int_div(lhs, rhs, mode),
            RT_ARITH_MOD => arith::modulo(lhs, rhs, mode),
            RT_ARITH_POW => arith::pow(lhs, rhs),
            _ => return seat_fault(state, Fault::new(5, format!("unknown arithmetic op {op}"))),
        };
        match result {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_neg_v(
    state: *mut RawExecState,
    mode: u32,
    src: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let src = match read_in(state, src, "src") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let mode = match numeric_mode_from_raw(mode) {
            Ok(mode) => mode,
            Err(fault) => return seat_fault(state, fault),
        };
        match arith::neg(src, mode) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_logical_v(
    state: *mut RawExecState,
    op: u32,
    lhs: *const Variant,
    rhs: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let lhs = match read_in(state, lhs, "lhs") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let rhs = match read_in(state, rhs, "rhs") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let result = match op {
            RT_LOGIC_AND => arith::and(lhs, rhs),
            RT_LOGIC_OR => arith::or(lhs, rhs),
            RT_LOGIC_XOR => arith::xor(lhs, rhs),
            RT_LOGIC_EQV => arith::eqv(lhs, rhs),
            RT_LOGIC_IMP => arith::imp(lhs, rhs),
            _ => return seat_fault(state, Fault::new(5, format!("unknown logical op {op}"))),
        };
        match result {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_not_v(
    state: *mut RawExecState,
    src: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let src = match read_in(state, src, "src") {
            Ok(value) => value,
            Err(status) => return status,
        };
        match arith::not(src) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_truthy_v(state: *mut RawExecState, src: *const Variant, out: *mut i32) -> i32 {
    with_status(|| {
        let src = match read_in(state, src, "src") {
            Ok(value) => value,
            Err(status) => return status,
        };
        match arith::is_truthy(src) {
            Ok(value) => write_out(state, out, i32::from(u8::from(value))),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_compare_v(
    state: *mut RawExecState,
    op: u32,
    mode: u32,
    lhs: *const Variant,
    rhs: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let lhs = match read_in(state, lhs, "lhs") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let rhs = match read_in(state, rhs, "rhs") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let op = match compare_op_from_raw(op) {
            Ok(op) => op,
            Err(fault) => return seat_fault(state, fault),
        };
        let mode = match string_compare_mode_from_raw(mode) {
            Ok(mode) => mode,
            Err(fault) => return seat_fault(state, fault),
        };
        match arith::compare(lhs, rhs, mode, op) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_coerce_numeric_v(
    state: *mut RawExecState,
    target: u32,
    src: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let src = match read_in(state, src, "src") {
            Ok(value) => value,
            Err(status) => return status,
        };
        let target = match numeric_target_from_raw(target) {
            Ok(target) => target,
            Err(fault) => return seat_fault(state, fault),
        };
        match arith::coerce_numeric(src, target) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_coerce_string_v(
    state: *mut RawExecState,
    src: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let src = match read_in(state, src, "src") {
            Ok(value) => value,
            Err(status) => return status,
        };
        match arith::coerce_string(src) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_coerce_fixed_string_v(
    state: *mut RawExecState,
    len: u32,
    src: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let src = match read_in(state, src, "src") {
            Ok(value) => value,
            Err(status) => return status,
        };
        match arith::coerce_fixed_string(src, len as usize) {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_arith(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_variant_clone(
    state: *mut RawExecState,
    src: *const Variant,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        if src.is_null() {
            return seat_fault(state, Fault::new(5, "runtime ABI source pointer is null"));
        }
        // SAFETY: null was rejected and the ABI requires `src` to name a live `Variant`.
        let value = unsafe { (*src).clone() };
        write_out(state, out, value)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_variant_release(state: *mut RawExecState, value: *mut Variant) -> i32 {
    with_status(|| {
        let _ = state;
        release_variant_slot(value, None)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_bstr_release(state: *mut RawExecState, value: *mut Variant) -> i32 {
    with_status(|| {
        let _ = state;
        release_variant_slot(value, Some(VarType::String))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_object_release(state: *mut RawExecState, value: *mut Variant) -> i32 {
    with_status(|| {
        let _ = state;
        release_variant_slot(value, Some(VarType::Object))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_array_release(state: *mut RawExecState, value: *mut Variant) -> i32 {
    with_status(|| {
        let _ = state;
        release_variant_slot(value, Some(VarType::ArrayVariant))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_record_release(state: *mut RawExecState, value: *mut Variant) -> i32 {
    with_status(|| {
        let _ = state;
        release_variant_slot(value, Some(VarType::Record))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_lib_invoke(
    state: *mut RawExecState,
    native_id: u32,
    args: *const Variant,
    argc: usize,
    out: *mut Variant,
) -> i32 {
    rt_lib_invoke_with_policy(state, native_id, args, argc, 0, out)
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_lib_invoke_with_policy(
    state: *mut RawExecState,
    native_id: u32,
    args: *const Variant,
    argc: usize,
    string_typed_alias: i32,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let id = match NativeImplId::ALL.get(native_id as usize).copied() {
            Some(id) => id,
            None => {
                return seat_fault(
                    state,
                    Fault::new(5, format!("unknown library id {native_id}")),
                );
            }
        };
        if args.is_null() && argc != 0 {
            return seat_fault(state, Fault::new(5, "runtime ABI args pointer is null"));
        }
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        // SAFETY: null with nonzero length was rejected; zero-length null is permitted.
        let argv = unsafe { slice::from_raw_parts(args, argc) };
        if string_typed_alias != 0 && argv.iter().any(|arg| arg.vtype() == VarType::Null) {
            return seat_fault(state, Fault::new(94, "invalid use of Null"));
        }
        let result = oxvba_lib::invoke(id, argv, exec.host, &mut exec.lib);
        match result {
            Ok(value) => write_out(state, out, value),
            Err(err) => seat_fault(state, Fault::from_lib(err)),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_maybe_drain(state: *mut RawExecState) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        maybe_drain_with_bridge(exec)
            .map(|_| ST_OK)
            .unwrap_or_else(|fault| seat_fault(state, fault))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_install_proc_invoker(
    state: *mut RawExecState,
    ctx: *mut c_void,
    invoke: Option<ProcInvokeFn>,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        exec.proc_invoker = invoke.map(|invoke| ProcInvokeBridge { ctx, invoke });
        ST_OK
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_clear_proc_invoker(state: *mut RawExecState) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        exec.proc_invoker = None;
        ST_OK
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_project_new_object(
    state: *mut RawExecState,
    program_index: usize,
    class_index: usize,
    out: *mut Variant,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let Some(loaded) = exec.programs.get(program_index) else {
            return seat_fault(
                state,
                Fault::new(5, format!("unknown program {program_index}")),
            );
        };
        let Some(descriptor) = loaded.class_descriptors.get(class_index).copied() else {
            return seat_fault(state, Fault::new(5, format!("unknown class {class_index}")));
        };
        let Some(class) = loaded.program.classes.get(class_index) else {
            return seat_fault(state, Fault::new(5, format!("unknown class {class_index}")));
        };
        let instance_id = exec.next_instance_id;
        exec.next_instance_id += 1;
        let object = ObjectRef::from_project_instance(
            instance_id,
            class_index as i32,
            program_index as i32,
            class.terminate.is_some(),
            descriptor,
        );
        let value = Variant::from_object_ref(object.clone());
        if let Some(init) = class.initialize {
            let Some(bridge) = exec.proc_invoker else {
                return seat_fault(
                    state,
                    Fault::new(
                        5,
                        "rt_project_new_object requires an installed ProcInvoker for Class_Initialize",
                    ),
                );
            };
            // SAFETY: the installed bridge owns its opaque context for this run. The `Me`
            // Variant is borrowed only for the duration of the synchronous initializer call.
            let status = unsafe { (bridge.invoke)(bridge.ctx, program_index, init.0, &value, 0) };
            if status != ST_OK {
                return status;
            }
        }
        write_out(state, out, value)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_err_clear(state: *mut RawExecState) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        exec.err_engine.clear_err();
        ST_OK
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_err_number(state: *mut RawExecState, out: *mut i32) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        write_out(state, out, exec.err_engine.err.number)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_err_i32_field(state: *mut RawExecState, field: u32, out: *mut i32) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let value = match field {
            RT_ERR_FIELD_NUMBER => exec.err_engine.err.number,
            RT_ERR_FIELD_HELP_CONTEXT => exec.err_engine.err.help_context,
            RT_ERR_FIELD_LAST_DLL_ERROR => exec.err_engine.last_dll_error,
            _ => return seat_fault(state, Fault::new(5, "Err field is not numeric")),
        };
        write_out(state, out, value)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_err_string_field_utf8(
    state: *mut RawExecState,
    field: u32,
    out_ptr: *mut *const u8,
    out_len: *mut i32,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let value = match field {
            RT_ERR_FIELD_DESCRIPTION => &exec.err_engine.err.description,
            RT_ERR_FIELD_SOURCE => &exec.err_engine.err.source,
            RT_ERR_FIELD_HELP_FILE => &exec.err_engine.err.help_file,
            _ => return seat_fault(state, Fault::new(5, "Err field is not string")),
        };
        let Ok(len) = i32::try_from(value.len()) else {
            return seat_fault(state, Fault::new(5, "Err field string is too long"));
        };
        let status = write_out(state, out_ptr, value.as_ptr());
        if status != ST_OK {
            return status;
        }
        write_out(state, out_len, len)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_set_error_handler(
    state: *mut RawExecState,
    handler_kind: u32,
    block: u32,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let handler = match handler_kind {
            RT_ERROR_HANDLER_GOTO_0 => ErrorHandler::Goto0,
            RT_ERROR_HANDLER_RESUME_NEXT => ErrorHandler::ResumeNext,
            RT_ERROR_HANDLER_GOTO_LABEL => ErrorHandler::GotoLabel(BlockId(block as usize)),
            RT_ERROR_HANDLER_GOTO_MINUS_1 => ErrorHandler::GotoMinus1,
            _ => {
                return seat_fault(
                    state,
                    Fault::new(5, format!("unknown error handler kind {handler_kind}")),
                );
            }
        };
        exec.err_engine.set_error_handler(&handler);
        ST_OK
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_last_dll_error(state: *mut RawExecState, out: *mut i32) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        write_out(state, out, exec.err_engine.last_dll_error)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_set_last_dll_error(state: *mut RawExecState, value: i32) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        exec.err_engine.last_dll_error = value;
        ST_OK
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_err_enter_activation(
    state: *mut RawExecState,
    out_saved: *mut RtSavedErrState,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let saved = saved_err_to_raw(exec.err_engine.enter_activation());
        write_out(state, out_saved, saved)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_err_restore_activation(
    state: *mut RawExecState,
    saved: *const RtSavedErrState,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let Ok(saved) = read_in(state, saved, "saved error state") else {
            return ST_FAULT;
        };
        let saved = match saved_err_from_raw(*saved) {
            Ok(saved) => saved,
            Err(fault) => return seat_fault(state, fault),
        };
        exec.err_engine.restore(saved);
        ST_OK
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_out_of_stack(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(28, default_error_message(28))))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_out_of_memory(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(7, default_error_message(7))))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_invalid_proc_ref(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(490, "invalid procedure reference")))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_type_mismatch(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(13, default_error_message(13))))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_error_number(
    state: *mut RawExecState,
    number: i32,
    inherit_fields: i32,
    source_ptr: *const u8,
    source_len: i32,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let source_len = match usize::try_from(source_len) {
            Ok(value) => value,
            Err(_) => return seat_fault(state, Fault::new(5, "invalid Err.Raise source length")),
        };
        let default_source = if source_len == 0 {
            "VBAProject"
        } else {
            if source_ptr.is_null() {
                return seat_fault(state, Fault::new(5, "invalid Err.Raise source pointer"));
            }
            // SAFETY: the JIT passes a pointer/length pair from the live OxProgram unit name.
            let bytes = unsafe { std::slice::from_raw_parts(source_ptr, source_len) };
            match std::str::from_utf8(bytes) {
                Ok(value) => value,
                Err(_) => return seat_fault(state, Fault::new(5, "invalid Err.Raise source utf8")),
            }
        };
        let inherit = inherit_fields != 0 && exec.err_engine.err.inherit_fields;
        let (message, source, help_file, help_context) = if inherit {
            (
                exec.err_engine.err.description.clone(),
                Some(exec.err_engine.err.source.clone()),
                Some(exec.err_engine.err.help_file.clone()),
                Some(exec.err_engine.err.help_context),
            )
        } else {
            (
                default_error_message(number),
                Some(default_source.to_owned()),
                None,
                None,
            )
        };
        seat_fault(
            state,
            Fault {
                code: number,
                message,
                source,
                help_file,
                help_context,
            },
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_runtime_error_number(state: *mut RawExecState, number: i32) -> i32 {
    with_status(|| seat_fault(state, Fault::new(number, default_error_message(number))))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_expected_array(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(13, "expected an array")))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_array_has_no_bounds(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(9, "array has no bounds")))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_fixed_or_temporarily_locked_array(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(10, default_error_message(10))))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_raise_subscript_out_of_range(state: *mut RawExecState) -> i32 {
    with_status(|| seat_fault(state, Fault::new(9, "subscript out of range")))
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_route_fault(
    state: *mut RawExecState,
    resume: u32,
    resume_next: u32,
    current_line: i32,
    out_dispatch: *mut i32,
    out_block: *mut u32,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let action = match exec.err_engine.dispatch_fault(
            BlockId(resume as usize),
            BlockId(resume_next as usize),
            current_line,
        ) {
            Ok(action) => action,
            Err(msg) => return seat_fault(state, Fault::new(5, msg)),
        };
        match action {
            FaultAction::Propagate(fault) => {
                exec.err_engine.pending_fault = Some(fault);
                write_out(state, out_dispatch, RT_FAULT_DISP_UNWIND)
            }
            FaultAction::ResumeNext(block) => {
                if write_out(state, out_block, block.0 as u32) != ST_OK {
                    return ST_FAULT;
                }
                write_out(state, out_dispatch, RT_FAULT_DISP_RESUME_NEXT)
            }
            FaultAction::Handle(block) => {
                if write_out(state, out_block, block.0 as u32) != ST_OK {
                    return ST_FAULT;
                }
                write_out(state, out_dispatch, RT_FAULT_DISP_HANDLER)
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_resume(
    state: *mut RawExecState,
    target_kind: u32,
    label: u32,
    out_block: *mut u32,
) -> i32 {
    with_status(|| {
        let Some(exec) = (unsafe { state_from_raw(state) }) else {
            return ST_FAULT;
        };
        let target = match target_kind {
            RT_RESUME_SAME => ResumeTarget::Same,
            RT_RESUME_NEXT => ResumeTarget::Next,
            RT_RESUME_LABEL => ResumeTarget::Label(BlockId(label as usize)),
            _ => {
                return seat_fault(
                    state,
                    Fault::new(5, format!("unknown resume target {target_kind}")),
                );
            }
        };
        match exec.err_engine.resume(target) {
            Ok(block) => write_out(state, out_block, block.0 as u32),
            Err(fault) => seat_fault(state, fault),
        }
    })
}

impl Default for ErrEngine {
    fn default() -> Self {
        Self {
            error_mode: ErrorMode::None,
            active_error: None,
            err: ErrState::default(),
            erl_line: 0,
            last_dll_error: 0,
            pending_fault: None,
        }
    }
}

impl ErrEngine {
    pub fn save(&self) -> SavedErrState {
        SavedErrState {
            error_mode: self.error_mode,
            active_error: self.active_error,
        }
    }

    pub fn restore(&mut self, saved: SavedErrState) {
        self.error_mode = saved.error_mode;
        self.active_error = saved.active_error;
    }

    pub fn enter_activation(&mut self) -> SavedErrState {
        let saved = self.save();
        self.error_mode = ErrorMode::None;
        self.active_error = None;
        saved
    }

    pub fn clear_err(&mut self) {
        self.err = ErrState::default();
    }

    pub fn set_error_handler(&mut self, handler: &ErrorHandler) {
        self.clear_err();
        match handler {
            ErrorHandler::GotoMinus1 => self.active_error = None,
            ErrorHandler::ResumeNext => self.error_mode = ErrorMode::ResumeNext,
            ErrorHandler::Goto0 => self.error_mode = ErrorMode::None,
            ErrorHandler::GotoLabel(block) => self.error_mode = ErrorMode::Goto(*block),
        }
    }

    /// Populate `Err` from a raised fault and stash it for the landing pad.
    pub fn raise(&mut self, mut fault: Fault, default_source: impl Into<String>) {
        self.err.number = fault.code;
        self.err.description = fault.message.clone();
        self.err.inherit_fields = true;

        let source = fault
            .source
            .clone()
            .unwrap_or_else(|| default_source.into());
        self.err.source = source.clone();
        fault.source = Some(source);

        let help_file = fault
            .help_file
            .clone()
            .unwrap_or_else(default_error_help_file);
        self.err.help_file = help_file.clone();
        fault.help_file = Some(help_file);

        let help_context = fault
            .help_context
            .unwrap_or_else(|| default_error_help_context(fault.code));
        self.err.help_context = help_context;
        fault.help_context = Some(help_context);

        self.pending_fault = Some(fault);
    }

    pub fn dispatch_fault(
        &mut self,
        resume: BlockId,
        resume_next: BlockId,
        current_line: i32,
    ) -> Result<FaultAction, &'static str> {
        let rp = ResumePoint {
            resume,
            resume_next,
            handler: self.error_mode,
        };
        match self.error_mode {
            ErrorMode::None => self
                .pending_fault
                .take()
                .map(FaultAction::Propagate)
                .ok_or("FaultDispatch reached with no pending fault"),
            ErrorMode::ResumeNext => {
                self.pending_fault = None;
                self.erl_line = current_line;
                self.active_error = None;
                Ok(FaultAction::ResumeNext(resume_next))
            }
            ErrorMode::Goto(handler) => {
                self.pending_fault = None;
                self.error_mode = ErrorMode::None;
                self.erl_line = current_line;
                self.active_error = Some(rp);
                Ok(FaultAction::Handle(handler))
            }
        }
    }

    pub fn resume(&mut self, target: ResumeTarget) -> Result<BlockId, Fault> {
        let Some(rp) = self.active_error.take() else {
            return Err(Fault::new(20, default_error_message(20)));
        };
        self.clear_err();
        self.error_mode = rp.handler;
        Ok(match target {
            ResumeTarget::Same => rp.resume,
            ResumeTarget::Next => rp.resume_next,
            ResumeTarget::Label(block) => block,
        })
    }
}

/// The default VBA message for a run-time error code.
pub fn default_error_message(code: i32) -> String {
    oxvba_runtime::default_error_message(code).to_string()
}

pub fn default_error_help_file() -> String {
    "C:\\Program Files\\Common Files\\Microsoft Shared\\VBA\\VBA7.1\\1033\\VbLR6.chm".to_string()
}

pub fn default_error_help_context(code: i32) -> i32 {
    let message = oxvba_runtime::default_error_message(code);
    if message == "Application-defined or object-defined error" {
        1_000_095
    } else {
        1_000_000 + code
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_hal::HostPolicy;
    use oxvba_hal::adapters::null::NullHostServices;

    #[test]
    fn goto_dispatch_demotes_and_resume_rearms() {
        let mut err = ErrEngine {
            error_mode: ErrorMode::Goto(BlockId(7)),
            pending_fault: Some(Fault::new(11, "division by zero")),
            ..ErrEngine::default()
        };
        let action = err
            .dispatch_fault(BlockId(1), BlockId(2), 100)
            .expect("dispatch");
        assert_eq!(action, FaultAction::Handle(BlockId(7)));
        assert_eq!(err.error_mode, ErrorMode::None);
        assert!(err.active_error.is_some());
        assert_eq!(err.erl_line, 100);

        let resume = err.resume(ResumeTarget::Next).expect("resume next");
        assert_eq!(resume, BlockId(2));
        assert_eq!(err.error_mode, ErrorMode::Goto(BlockId(7)));
    }

    #[test]
    fn resume_without_active_error_raises_20() {
        let mut err = ErrEngine::default();
        let fault = err
            .resume(ResumeTarget::Next)
            .expect_err("missing active error");
        assert_eq!(fault.code, 20);
    }

    #[test]
    fn checked_i32_shim_writes_output() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_i32(exec_state_as_raw(&mut state), 2, 5, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 7);
    }

    #[test]
    fn checked_i32_shim_seats_overflow_fault() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_i32(exec_state_as_raw(&mut state), i32::MAX, 1, &mut out);
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 6);
    }

    #[test]
    fn raise_error_number_respects_legacy_error_defaults_and_err_raise_inheritance() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let source = b"Main";

        state.err_engine.err = ErrState {
            number: 100,
            description: "previous description".to_string(),
            source: "PreviousSource".to_string(),
            help_file: "previous.chm".to_string(),
            help_context: 77,
            inherit_fields: true,
        };

        let status = rt_raise_error_number(
            exec_state_as_raw(&mut state),
            5,
            0,
            source.as_ptr(),
            source.len() as i32,
        );
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 5);
        assert_eq!(state.err_engine.err.description, default_error_message(5));
        assert_eq!(state.err_engine.err.source, "Main");
        assert_eq!(state.err_engine.err.help_file, default_error_help_file());
        assert_eq!(
            state.err_engine.err.help_context,
            default_error_help_context(5)
        );

        state.err_engine.err = ErrState {
            number: 100,
            description: "previous description".to_string(),
            source: "PreviousSource".to_string(),
            help_file: "previous.chm".to_string(),
            help_context: 77,
            inherit_fields: true,
        };

        let status = rt_raise_error_number(
            exec_state_as_raw(&mut state),
            42,
            1,
            source.as_ptr(),
            source.len() as i32,
        );
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 42);
        assert_eq!(state.err_engine.err.description, "previous description");
        assert_eq!(state.err_engine.err.source, "PreviousSource");
        assert_eq!(state.err_engine.err.help_file, "previous.chm");
        assert_eq!(state.err_engine.err.help_context, 77);
    }

    #[test]
    fn err_field_getters_project_current_and_cleared_state() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        state
            .err_engine
            .raise(Fault::new(9, default_error_message(9)), "VBAProject");

        let mut number = 0;
        let status = rt_err_i32_field(
            exec_state_as_raw(&mut state),
            RT_ERR_FIELD_NUMBER,
            &mut number,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(number, 9);

        let mut help_context = 0;
        let status = rt_err_i32_field(
            exec_state_as_raw(&mut state),
            RT_ERR_FIELD_HELP_CONTEXT,
            &mut help_context,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(help_context, default_error_help_context(9));

        let mut ptr = std::ptr::null();
        let mut len = 0;
        let status = rt_err_string_field_utf8(
            exec_state_as_raw(&mut state),
            RT_ERR_FIELD_DESCRIPTION,
            &mut ptr,
            &mut len,
        );
        assert_eq!(status, ST_OK);
        // SAFETY: the ABI returned a pointer/length to the live state's UTF-8 string.
        let description =
            unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len as usize)) };
        assert_eq!(description, default_error_message(9));

        let status = rt_err_clear(exec_state_as_raw(&mut state));
        assert_eq!(status, ST_OK);

        let status = rt_err_i32_field(
            exec_state_as_raw(&mut state),
            RT_ERR_FIELD_NUMBER,
            &mut number,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(number, 0);

        let status = rt_err_string_field_utf8(
            exec_state_as_raw(&mut state),
            RT_ERR_FIELD_DESCRIPTION,
            &mut ptr,
            &mut len,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(len, 0);
        assert!(!ptr.is_null());
    }

    #[test]
    fn checked_i32_div_rem_shims_write_output() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_div_i32(exec_state_as_raw(&mut state), 17, 5, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 3);

        let status = rt_rem_i32(exec_state_as_raw(&mut state), 17, 5, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 2);
    }

    #[test]
    fn checked_i32_div_rem_shims_seat_division_by_zero_fault() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_div_i32(exec_state_as_raw(&mut state), 17, 0, &mut out);
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 11);

        state.err_engine.clear_err();
        let status = rt_rem_i32(exec_state_as_raw(&mut state), 17, 0, &mut out);
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 11);
    }

    #[test]
    fn checked_i16_shim_writes_output() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_i16(exec_state_as_raw(&mut state), 32_000, 12, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 32_012);
    }

    #[test]
    fn checked_i16_shim_seats_overflow_fault() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_i16(
            exec_state_as_raw(&mut state),
            i32::from(i16::MAX),
            1,
            &mut out,
        );
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 6);
    }

    #[test]
    fn checked_u8_shim_writes_output() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_u8(exec_state_as_raw(&mut state), 12, 5, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 17);
    }

    #[test]
    fn checked_u8_shim_seats_overflow_fault() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_u8(
            exec_state_as_raw(&mut state),
            i32::from(u8::MAX),
            1,
            &mut out,
        );
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 6);
    }

    #[test]
    fn checked_i64_shim_writes_output() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_i64(exec_state_as_raw(&mut state), 5_000_000_000, 12, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 5_000_000_012);
    }

    #[test]
    fn checked_i64_shim_seats_overflow_fault() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_add_i64(exec_state_as_raw(&mut state), i64::MAX, 1, &mut out);
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 6);
    }

    #[test]
    fn checked_i64_div_rem_shims_write_output() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_div_i64(exec_state_as_raw(&mut state), 5_000_000_017, 5, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 1_000_000_003);

        let status = rt_rem_i64(exec_state_as_raw(&mut state), 5_000_000_017, 5, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 2);
    }

    #[test]
    fn out_of_stack_shim_seats_vba_error_28() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let status = rt_raise_out_of_stack(exec_state_as_raw(&mut state));
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 28);
        assert_eq!(state.err_engine.err.description, "Out of stack space");
    }

    #[test]
    fn invalid_proc_ref_shim_seats_vba_error_490() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let status = rt_raise_invalid_proc_ref(exec_state_as_raw(&mut state));
        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 490);
        assert_eq!(
            state.err_engine.err.description,
            "invalid procedure reference"
        );
    }

    #[test]
    fn activation_shims_clear_and_restore_error_policy() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        state.err_engine.error_mode = ErrorMode::Goto(BlockId(7));
        state.err_engine.active_error = Some(ResumePoint {
            resume: BlockId(2),
            resume_next: BlockId(3),
            handler: ErrorMode::ResumeNext,
        });

        let mut saved = RtSavedErrState::default();
        let status = rt_err_enter_activation(exec_state_as_raw(&mut state), &mut saved);
        assert_eq!(status, ST_OK);
        assert_eq!(state.err_engine.error_mode, ErrorMode::None);
        assert_eq!(state.err_engine.active_error, None);

        let status = rt_err_restore_activation(exec_state_as_raw(&mut state), &saved);
        assert_eq!(status, ST_OK);
        assert_eq!(state.err_engine.error_mode, ErrorMode::Goto(BlockId(7)));
        assert_eq!(
            state.err_engine.active_error,
            Some(ResumePoint {
                resume: BlockId(2),
                resume_next: BlockId(3),
                handler: ErrorMode::ResumeNext,
            })
        );
    }

    #[test]
    fn currency_shim_uses_scaled_i128_kernel() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let mut out = 0;
        let status = rt_currency_mul(exec_state_as_raw(&mut state), 12_345, 67_891, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 83_811);
    }

    #[test]
    fn variant_arith_shim_writes_variant_result() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let lhs = Variant::from_i32(20);
        let rhs = Variant::from_i32(22);
        let mut out = Variant::empty();
        let status = rt_arith_v(
            exec_state_as_raw(&mut state),
            RT_ARITH_ADD,
            RT_NUMERIC_CHECKED_LONG,
            &lhs,
            &rhs,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_i32(), Some(42));
    }

    #[test]
    fn variant_div_pow_shim_write_double_results() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let lhs = Variant::from_f64(9.0);
        let rhs = Variant::from_f64(2.0);
        let mut out = Variant::empty();
        let status = rt_arith_v(
            exec_state_as_raw(&mut state),
            RT_ARITH_DIV,
            RT_NUMERIC_WIDENING,
            &lhs,
            &rhs,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_f64(), Some(4.5));

        let status = rt_arith_v(
            exec_state_as_raw(&mut state),
            RT_ARITH_POW,
            RT_NUMERIC_WIDENING,
            &lhs,
            &rhs,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_f64(), Some(81.0));
    }

    #[test]
    fn variant_neg_shim_writes_variant_result() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let src = Variant::from_f64(2.5);
        let mut out = Variant::empty();
        let status = rt_neg_v(
            exec_state_as_raw(&mut state),
            RT_NUMERIC_WIDENING,
            &src,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_f64(), Some(-2.5));
    }

    #[test]
    fn variant_logical_shim_uses_shared_null_logic() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let lhs = Variant::null();
        let rhs = Variant::from_bool(false);
        let mut out = Variant::empty();
        let status = rt_logical_v(
            exec_state_as_raw(&mut state),
            RT_LOGIC_AND,
            &lhs,
            &rhs,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_bool(), Some(false));
    }

    #[test]
    fn variant_not_shim_preserves_null() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let src = Variant::null();
        let mut out = Variant::empty();
        let status = rt_not_v(exec_state_as_raw(&mut state), &src, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out.vtype(), VarType::Null);
    }

    #[test]
    fn variant_truthy_shim_treats_null_as_false() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let src = Variant::null();
        let mut out = -1;
        let status = rt_truthy_v(exec_state_as_raw(&mut state), &src, &mut out);
        assert_eq!(status, ST_OK);
        assert_eq!(out, 0);
    }

    #[test]
    fn variant_compare_shim_preserves_null() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let lhs = Variant::null();
        let rhs = Variant::from_i32(1);
        let mut out = Variant::empty();
        let status = rt_compare_v(
            exec_state_as_raw(&mut state),
            RT_COMPARE_EQ,
            RT_STRING_COMPARE_BINARY,
            &lhs,
            &rhs,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.vtype(), VarType::Null);
    }

    #[test]
    fn variant_numeric_coerce_shim_writes_variant_result() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let src = Variant::from_f64(6.5);
        let mut out = Variant::empty();
        let status = rt_coerce_numeric_v(
            exec_state_as_raw(&mut state),
            RT_NUMERIC_CHECKED_DOUBLE,
            &src,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_f64(), Some(6.5));
    }

    #[test]
    fn variant_numeric_coerce_shim_writes_boolean_result() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let src = Variant::from_i32(2);
        let mut out = Variant::empty();
        let status = rt_coerce_numeric_v(
            exec_state_as_raw(&mut state),
            RT_NUMERIC_CHECKED_BOOLEAN,
            &src,
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_bool(), Some(true));
    }

    #[test]
    fn type_specific_releases_are_null_safe_and_match_only_their_type() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        assert_eq!(
            rt_bstr_release(exec_state_as_raw(&mut state), std::ptr::null_mut()),
            ST_OK
        );

        let mut text = Variant::from_string("abc");
        let status = rt_bstr_release(exec_state_as_raw(&mut state), &mut text);
        assert_eq!(status, ST_OK);
        assert_eq!(text.vtype(), VarType::Empty);

        let mut number = Variant::from_i32(7);
        let status = rt_bstr_release(exec_state_as_raw(&mut state), &mut number);
        assert_eq!(status, ST_OK);
        assert_eq!(number.as_i32(), Some(7));
    }

    #[test]
    fn lib_invoke_shim_calls_shared_library_context() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        let len_id = NativeImplId::ALL
            .iter()
            .position(|id| *id == NativeImplId::Len)
            .expect("Len id") as u32;
        let args = [Variant::from_string("abcd")];
        let mut out = Variant::empty();
        let status = rt_lib_invoke(
            exec_state_as_raw(&mut state),
            len_id,
            args.as_ptr(),
            args.len(),
            &mut out,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(out.as_i32(), Some(4));
    }

    #[test]
    fn err_and_fault_shims_route_through_err_engine() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        state.err_engine.error_mode = ErrorMode::Goto(BlockId(9));
        state
            .err_engine
            .raise(Fault::new(11, "division by zero"), "TestProject");

        let mut dispatch = -1;
        let mut block = 0;
        let status = rt_route_fault(
            exec_state_as_raw(&mut state),
            1,
            2,
            77,
            &mut dispatch,
            &mut block,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(dispatch, RT_FAULT_DISP_HANDLER);
        assert_eq!(block, 9);

        let mut resumed = 0;
        let status = rt_resume(
            exec_state_as_raw(&mut state),
            RT_RESUME_NEXT,
            0,
            &mut resumed,
        );
        assert_eq!(status, ST_OK);
        assert_eq!(resumed, 2);

        let mut number = -1;
        assert_eq!(
            rt_err_number(exec_state_as_raw(&mut state), &mut number),
            ST_OK
        );
        assert_eq!(number, 0);
        assert_eq!(
            rt_set_last_dll_error(exec_state_as_raw(&mut state), 1234),
            ST_OK
        );
        let mut last = 0;
        assert_eq!(
            rt_last_dll_error(exec_state_as_raw(&mut state), &mut last),
            ST_OK
        );
        assert_eq!(last, 1234);
    }

    #[test]
    fn rt_resume_without_active_error_uses_state_default_source() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        state.default_error_source = "Main".to_string();
        state.err_engine.error_mode = ErrorMode::ResumeNext;

        let mut resumed = 0;
        let status = rt_resume(
            exec_state_as_raw(&mut state),
            RT_RESUME_NEXT,
            0,
            &mut resumed,
        );

        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 20);
        assert_eq!(state.err_engine.err.source, "Main");
    }

    #[test]
    fn type_mismatch_shim_seats_error_13() {
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);

        let status = rt_raise_type_mismatch(exec_state_as_raw(&mut state));

        assert_eq!(status, ST_FAULT);
        assert_eq!(state.err_engine.err.number, 13);
        assert_eq!(state.err_engine.err.description, "Type mismatch");
    }

    #[test]
    fn maybe_drain_shim_is_ok_when_queue_is_empty() {
        oxvba_runtime::reset_pending_terminations();
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        assert_eq!(rt_maybe_drain(exec_state_as_raw(&mut state)), ST_OK);
    }
}
