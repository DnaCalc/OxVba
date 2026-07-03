//! Shared runtime ABI substrate for vm3 and the Cranelift JIT.
//!
//! This crate is intentionally free of Cranelift and interpreter-frame state. It hosts
//! the VM-agnostic cells and helper decisions that both engines must share.

use oxvba_bundle::{NativeImplId, NumericCoerceTarget, NumericMode};
use oxvba_com::ComSubscriptionToken;
use oxvba_eval::arith::{self, ArithError};
use oxvba_eval::typed;
use oxvba_hal::HostServices;
use oxvba_lib::LibContext;
use oxvba_oxir::value::{OxArg, OxCallArg, OxOperand, OxPlace};
use oxvba_oxir::{BlockId, ErrorHandler, FuncId, OxProgram};
use oxvba_runtime::object_ref::{ObjectRef, RuntimeClassDescriptor};
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

pub const RT_NUMERIC_WIDENING: u32 = 0;
pub const RT_NUMERIC_CHECKED_BYTE: u32 = 1;
pub const RT_NUMERIC_CHECKED_INTEGER: u32 = 2;
pub const RT_NUMERIC_CHECKED_LONG: u32 = 3;
pub const RT_NUMERIC_CHECKED_LONGLONG: u32 = 4;
pub const RT_NUMERIC_CHECKED_SINGLE: u32 = 5;
pub const RT_NUMERIC_CHECKED_DOUBLE: u32 = 6;
pub const RT_NUMERIC_CHECKED_CURRENCY: u32 = 7;
pub const RT_NUMERIC_CHECKED_DATE: u32 = 8;

pub const RT_FAULT_DISP_UNWIND: i32 = 0;
pub const RT_FAULT_DISP_RESUME_NEXT: i32 = 1;
pub const RT_FAULT_DISP_HANDLER: i32 = 2;

pub const RT_RESUME_SAME: u32 = 0;
pub const RT_RESUME_NEXT: u32 = 1;
pub const RT_RESUME_LABEL: u32 = 2;

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
        state.err_engine.raise(fault, "VBAProject");
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
        _ => return Err(Fault::new(5, format!("unknown numeric mode {raw}"))),
    };
    Ok(mode)
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
            _ => return seat_fault(state, Fault::new(5, format!("unknown arithmetic op {op}"))),
        };
        match result {
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
    fn maybe_drain_shim_is_ok_when_queue_is_empty() {
        oxvba_runtime::reset_pending_terminations();
        let host = NullHostServices::new(HostPolicy::default());
        let mut state = ExecState::new(&host);
        assert_eq!(rt_maybe_drain(exec_state_as_raw(&mut state)), ST_OK);
    }
}
