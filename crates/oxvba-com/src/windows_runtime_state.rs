#![allow(unsafe_op_in_unsafe_fn)]

use crate::runtime_state::ComEventCallbackValue;
use crate::{
    ComBinding, ComCallbackToken, ComEventPath, ComEventSpec, ComInvokeArg, ComMemberSpec,
    ComMemberToken, ComObjectToken, ComRuntimeState, ComSubscriptionToken, ComValue,
    DispatchEventSinkConfig, RawIDispatch, WindowsConnectionPointTransport,
    binding_from_typelib_metadata, get_dispid_by_name, query_unknown_from_dispatch,
    release_dispatch, release_unknown, source_interface_event_spec_supported,
    try_advise_dispatch_event_sink, try_advise_single_i32_source_interface_event_sink,
    unadvise_connection_point,
};

use oxvba_runtime::ObjectRef;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

const COM_EVENT_DISPATCH_MEMBER_WILDCARD: i32 = i32::MIN + 3_333;

type DispatchEventCallback = Arc<dyn Fn(&[ComValue]) -> bool + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsComSubscriptionTransport {
    Projection,
    NativeConnectionPoint(WindowsConnectionPointTransport),
}

impl WindowsComSubscriptionTransport {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Projection => "projection",
            Self::NativeConnectionPoint(_) => "native-connection-point",
        }
    }

    pub fn is_projection(&self) -> bool {
        matches!(self, Self::Projection)
    }
}

#[derive(Debug, Default)]
pub struct WindowsComClientState {
    inner: ComRuntimeState<WindowsComSubscriptionTransport>,
    host_objects_by_prog_id: BTreeMap<String, ComObjectToken>,
}

impl Deref for WindowsComClientState {
    type Target = ComRuntimeState<WindowsComSubscriptionTransport>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for WindowsComClientState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for WindowsComClientState {
    fn drop(&mut self) {
        for subscription in self.subscriptions.values() {
            if let WindowsComSubscriptionTransport::NativeConnectionPoint(native) =
                subscription.transport
            {
                unsafe {
                    let _ = unadvise_connection_point(native);
                }
            }
        }
        self.subscriptions.clear();
        self.callbacks.clear();
        self.pending_callbacks.clear();
        self.host_objects_by_prog_id.clear();
        for binding in self.bindings.values_mut() {
            if binding.native_dispatch != 0 {
                unsafe {
                    release_dispatch(binding.native_dispatch as *mut RawIDispatch);
                }
                binding.native_dispatch = 0;
            }
            if binding.native_unknown != 0 {
                unsafe {
                    release_unknown(binding.native_unknown as *mut core::ffi::c_void);
                }
                binding.native_unknown = 0;
            }
            binding.runtime_object = None;
        }
        self.bindings.clear();
    }
}

fn normalize_prog_id_name(prog_id_name: &str) -> String {
    prog_id_name.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone)]
pub struct ReleasedWindowsComObject {
    pub transports: Vec<WindowsComSubscriptionTransport>,
    pub stale_callbacks: BTreeSet<ComCallbackToken>,
}

fn callback_sink(
    com_state: Arc<Mutex<WindowsComClientState>>,
    subscription: ComSubscriptionToken,
) -> DispatchEventCallback {
    Arc::new(move |args: &[ComValue]| {
        let mut state = match com_state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.queue_callback_for_subscription(subscription, args)
    })
}

/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer, `spec` and
/// `connection_point_iid` must describe the event interface implemented by that
/// object, and `com_state` must remain a valid shared runtime container for the
/// lifetime of any transport returned by this call.
pub unsafe fn advise_event_subscription(
    dispatch: *mut RawIDispatch,
    com_state: Arc<Mutex<WindowsComClientState>>,
    subscription: ComSubscriptionToken,
    spec: &ComEventSpec,
    expected_arity: usize,
    connection_point_iid: &str,
) -> Result<Option<WindowsConnectionPointTransport>, String> {
    match spec.path {
        ComEventPath::Dispatch => try_advise_dispatch_event_sink(
            dispatch,
            connection_point_iid,
            DispatchEventSinkConfig {
                event_dispatch_member: spec
                    .dispatch_member_id
                    .unwrap_or(COM_EVENT_DISPATCH_MEMBER_WILDCARD),
                expected_arity,
                connection_point_iid: None,
                on_event: callback_sink(com_state, subscription),
            },
        ),
        ComEventPath::SourceInterface => {
            if !source_interface_event_spec_supported(spec) {
                return Err(format!(
                    "COM-E-EVENT-PATH-UNSUPPORTED: source-interface COM event callbacks (COM-EVT-B) are unsupported for connection-point IID `{connection_point_iid}` in current lane"
                ));
            }
            try_advise_single_i32_source_interface_event_sink(
                dispatch,
                connection_point_iid,
                expected_arity,
                callback_sink(com_state, subscription),
            )
        }
    }
}

/// # Safety
///
/// `transport` must have been produced by `advise_event_subscription` and must
/// still represent a live connection-point subscription owned by the caller.
pub unsafe fn release_subscription_transport(
    transport: WindowsComSubscriptionTransport,
) -> Result<(), String> {
    if let WindowsComSubscriptionTransport::NativeConnectionPoint(native) = transport {
        unadvise_connection_point(native)?;
    }
    Ok(())
}

pub fn event_signature_arity_for_binding(
    binding: &ComBinding,
    event: ComMemberToken,
) -> Option<usize> {
    binding
        .event_specs
        .get(&event)
        .map(|spec| spec.callback_arity)
}

pub fn event_is_source_interface_only(binding: &ComBinding, event: ComMemberToken) -> bool {
    binding
        .event_specs
        .get(&event)
        .is_some_and(|spec| matches!(spec.path, ComEventPath::SourceInterface))
}

pub fn event_callback_args_from_invoke_args(
    binding: &ComBinding,
    member: ComMemberToken,
    args: &[ComInvokeArg],
) -> Result<Option<(ComMemberToken, Vec<ComValue>)>, String> {
    let Some(trigger_spec) = binding.event_trigger_specs.get(&member).copied() else {
        return Ok(None);
    };
    if args.len() < trigger_spec.callback_arity {
        return Ok(None);
    }
    let mut values: Vec<ComValue> = args
        .iter()
        .take(trigger_spec.callback_arity)
        .enumerate()
        .map(|(index, arg)| {
            arg.value
                .as_ref()
                .map(|value| value.to_com_value())
                .ok_or_else(|| {
                    format!(
                        "COM-E-VALUE-TRANSPORT-UNSUPPORTED: projected event trigger `{}` requires concrete callback argument {}",
                        trigger_spec.event_token.raw(),
                        index
                    )
                })
        })
        .collect::<Result<_, _>>()?;
    if trigger_spec.second_arg_is_incremented
        && values.len() >= 2
        && let ComValue::I32(first) = values[0]
    {
        values[1] = ComValue::I32(first.saturating_add(1));
    }
    Ok(Some((trigger_spec.event_token, values)))
}

pub fn collect_stale_callbacks_for_subscription(
    state: &WindowsComClientState,
    subscription: ComSubscriptionToken,
    object: i32,
) -> BTreeSet<ComCallbackToken> {
    state
        .callbacks
        .iter()
        .filter_map(|(callback, payload)| {
            if payload.subscription == subscription && payload.object == object {
                Some(*callback)
            } else {
                None
            }
        })
        .collect()
}

pub fn resolve_bound_native_dispatch(
    state: &WindowsComClientState,
    object: ObjectRef,
) -> Result<*mut RawIDispatch, String> {
    let Some(binding) = state.bindings.get(&ComObjectToken::new(object.raw())) else {
        return Err(format!(
            "COM-E-OBJECT-MISSING: unknown COM object handle {}",
            object.raw()
        ));
    };
    if binding.native_dispatch == 0 {
        return Err(format!(
            "COM-E-OBJECT-MARSHAL-UNSUPPORTED: object handle {} is not backed by native IDispatch",
            object.raw()
        ));
    }
    Ok(binding.native_dispatch as *mut RawIDispatch)
}

pub fn resolve_bound_runtime_object(
    state: &WindowsComClientState,
    object: ObjectRef,
) -> Result<ObjectRef, String> {
    let Some(binding) = state.bindings.get(&ComObjectToken::new(object.raw())) else {
        return Err(format!(
            "COM-E-OBJECT-MISSING: unknown COM object handle {}",
            object.raw()
        ));
    };
    binding.runtime_object.map_or_else(
        || {
            Err(format!(
                "COM-E-OBJECT-IDENTITY-MISSING: object handle {} is not backed by retained runtime identity",
                object.raw()
            ))
        },
        |raw| {
            Ok(binding.runtime_class_descriptor.map_or_else(
                || ObjectRef::from_compat_identity(raw),
                |descriptor| ObjectRef::from_compat_identity_with_descriptor(raw, descriptor),
            ))
        },
    )
}

fn retained_runtime_object(binding: &mut ComBinding, handle: ComObjectToken) -> ObjectRef {
    if let Some(raw) = binding.runtime_object {
        return binding.runtime_class_descriptor.map_or_else(
            || ObjectRef::from_compat_identity(raw),
            |descriptor| ObjectRef::from_compat_identity_with_descriptor(raw, descriptor),
        );
    }
    let object = binding.runtime_class_descriptor.map_or_else(
        || ObjectRef::from_compat_identity(handle.raw()),
        |descriptor| ObjectRef::from_compat_identity_with_descriptor(handle.raw(), descriptor),
    );
    binding.runtime_object = Some(object.raw());
    object
}

/// # Safety
///
/// dispatch must be null or carry one retained IDispatch reference owned by the caller.
pub unsafe fn bind_native_dispatch_result(
    state: &mut WindowsComClientState,
    dispatch: *mut RawIDispatch,
    prog_id_hint: &str,
) -> ObjectRef {
    if dispatch.is_null() {
        return ObjectRef::from_compat_identity(0);
    }
    let unknown = match unsafe { query_unknown_from_dispatch(dispatch) } {
        Ok(unknown) => unknown,
        Err(_) => {
            unsafe {
                release_dispatch(dispatch);
            }
            return ObjectRef::from_compat_identity(0);
        }
    };
    if let Some((handle, binding)) = state
        .bindings
        .iter_mut()
        .find(|(_, binding)| binding.native_unknown == unknown as usize)
    {
        unsafe {
            release_dispatch(dispatch);
            release_unknown(unknown.cast());
        }
        return retained_runtime_object(binding, *handle);
    }
    let handle = state.allocate_handle();
    let mut binding = binding_from_typelib_metadata(
        format!("{prog_id_hint}::<invoke-result>"),
        dispatch as usize,
        None,
    );
    binding.native_unknown = unknown as usize;
    let object = retained_runtime_object(&mut binding, handle);
    state.bindings.insert(handle, binding);
    object
}

pub fn release_object_binding(
    state: &mut WindowsComClientState,
    object: ObjectRef,
) -> Result<ReleasedWindowsComObject, String> {
    let Some((binding, transports, stale_callbacks)) =
        state.release_object_state(ComObjectToken::new(object.raw()))
    else {
        return Err(format!(
            "COM-E-OBJECT-MISSING: unknown COM object token {}",
            object.raw()
        ));
    };
    if binding.native_dispatch != 0 {
        unsafe {
            release_dispatch(binding.native_dispatch as *mut RawIDispatch);
        }
    }
    if binding.native_unknown != 0 {
        unsafe {
            release_unknown(binding.native_unknown as *mut core::ffi::c_void);
        }
    }
    Ok(ReleasedWindowsComObject {
        transports,
        stale_callbacks,
    })
}

pub fn resolve_subscription_transport(
    state: &WindowsComClientState,
    subscription: ComSubscriptionToken,
) -> Result<WindowsComSubscriptionTransport, String> {
    let Some(entry) = state.subscriptions.get(&subscription) else {
        return Err(format!(
            "COM-E-EVENT-ADVISE-FAILED: unknown COM event subscription token {}",
            subscription.raw()
        ));
    };
    Ok(entry.transport)
}

pub fn take_polled_callback_payload(
    state: &mut WindowsComClientState,
) -> Option<crate::ComCallbackPayload> {
    state.take_polled_callback()
}

pub fn callback_subscription_token(
    state: &WindowsComClientState,
    callback: ComCallbackToken,
) -> Result<ComSubscriptionToken, String> {
    let Some(payload) = state.callbacks.get(&callback) else {
        return Err(format!(
            "COM-E-EVENT-CALLBACK-MISSING: unknown callback token {}",
            callback.raw()
        ));
    };
    Ok(payload.subscription)
}

pub fn callback_arity(
    state: &WindowsComClientState,
    callback: ComCallbackToken,
) -> Result<usize, String> {
    let Some(payload) = state.callbacks.get(&callback) else {
        return Err(format!(
            "COM-E-EVENT-CALLBACK-MISSING: unknown callback token {}",
            callback.raw()
        ));
    };
    Ok(payload.args.len())
}

pub fn callback_arg(
    state: &WindowsComClientState,
    callback: ComCallbackToken,
    index: usize,
) -> Result<ComEventCallbackValue, String> {
    let Some(payload) = state.callbacks.get(&callback) else {
        return Err(format!(
            "COM-E-EVENT-CALLBACK-MISSING: unknown callback token {}",
            callback.raw()
        ));
    };
    payload
        .args
        .get(index)
        .cloned()
        .ok_or_else(|| {
            format!(
                "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: callback argument index {} exceeds callback arity {}",
                index,
                payload.args.len()
            )
        })
}

pub fn release_callback(
    state: &mut WindowsComClientState,
    callback: ComCallbackToken,
) -> Result<(), String> {
    if state.callbacks.remove(&callback).is_none() {
        return Err(format!(
            "COM-E-EVENT-CALLBACK-MISSING: unknown callback token {}",
            callback.raw()
        ));
    }
    state.pending_callbacks.retain(|token| *token != callback);
    if state.last_pumped_callback == Some(callback) {
        state.last_pumped_callback = None;
    }
    Ok(())
}

pub fn remove_subscription_callbacks(
    state: &mut WindowsComClientState,
    subscription: ComSubscriptionToken,
) -> Result<BTreeSet<ComCallbackToken>, String> {
    let Some(entry) = state.subscriptions.remove(&subscription) else {
        return Err(format!(
            "COM-E-EVENT-ADVISE-FAILED: unknown COM event subscription token {}",
            subscription.raw()
        ));
    };
    let stale_callbacks =
        collect_stale_callbacks_for_subscription(state, subscription, entry.object);
    for callback in &stale_callbacks {
        state.callbacks.remove(callback);
    }
    state
        .pending_callbacks
        .retain(|callback| !stale_callbacks.contains(callback));
    if state
        .last_pumped_callback
        .is_some_and(|callback| stale_callbacks.contains(&callback))
    {
        state.last_pumped_callback = None;
    }
    Ok(stale_callbacks)
}

pub fn insert_bound_object_binding(
    state: &mut WindowsComClientState,
    mut binding: ComBinding,
) -> ObjectRef {
    let handle = state.allocate_handle();
    let object = retained_runtime_object(&mut binding, handle);
    state.bindings.insert(handle, binding);
    object
}

pub fn bind_host_dispatch_object(
    state: &mut WindowsComClientState,
    prog_id_name: &str,
    mut binding: ComBinding,
) -> ObjectRef {
    let normalized_prog_id = normalize_prog_id_name(prog_id_name);
    if let Some(handle) = state
        .host_objects_by_prog_id
        .get(&normalized_prog_id)
        .copied()
        && let Some(existing) = state.bindings.get_mut(&handle)
    {
        return retained_runtime_object(existing, handle);
    }
    let handle = state.allocate_handle();
    let object = retained_runtime_object(&mut binding, handle);
    state
        .host_objects_by_prog_id
        .insert(normalized_prog_id, handle);
    state.bindings.insert(handle, binding);
    object
}

pub fn host_dispatch_object_for_prog_id(
    state: &mut WindowsComClientState,
    prog_id_name: &str,
) -> Option<ObjectRef> {
    let handle = state
        .host_objects_by_prog_id
        .get(&normalize_prog_id_name(prog_id_name))
        .copied()?;
    state
        .bindings
        .get_mut(&handle)
        .map(|binding| retained_runtime_object(binding, handle))
}

pub fn insert_bound_object_binding_at_handle(
    state: &mut WindowsComClientState,
    object: ObjectRef,
    mut binding: ComBinding,
) -> ObjectRef {
    let runtime_object = if object.raw() == 0 {
        object
    } else {
        if binding.runtime_object.is_none() {
            binding.runtime_object = Some(object.raw());
        }
        let raw = binding
            .runtime_object
            .expect("non-zero object binding must retain a runtime object");
        binding.runtime_class_descriptor.map_or_else(
            || ObjectRef::from_compat_identity(raw),
            |descriptor| ObjectRef::from_compat_identity_with_descriptor(raw, descriptor),
        )
    };
    state
        .bindings
        .insert(ComObjectToken::new(runtime_object.raw()), binding);
    runtime_object
}

pub fn cache_member_dispid(
    state: &mut WindowsComClientState,
    object: ObjectRef,
    member: ComMemberToken,
    dispid: i32,
) {
    if let Some(binding) = state.bindings.get_mut(&ComObjectToken::new(object.raw())) {
        binding.member_dispids.insert(member, dispid);
    }
}

/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer for the duration of the lookup.
pub unsafe fn resolve_member_dispid_cached(
    state: &mut WindowsComClientState,
    dispatch: *mut RawIDispatch,
    object: ObjectRef,
    binding: &ComBinding,
    member: ComMemberToken,
    fallback_spec: Option<ComMemberSpec>,
) -> Result<Option<(i32, ComMemberSpec)>, String> {
    let spec = if let Some(spec) = binding.member_specs.get(&member).cloned() {
        spec
    } else if let Some(spec) = fallback_spec {
        spec
    } else {
        return Ok(None);
    };
    if let Some(dispid) = binding.member_dispids.get(&member).copied() {
        return Ok(Some((dispid, spec)));
    }
    let dispid = unsafe { get_dispid_by_name(dispatch, &spec.name) }?;
    cache_member_dispid(state, object, member, dispid);
    Ok(Some((dispid, spec)))
}

/// # Safety
///
/// `dispatch` must be a valid live `IDispatch` pointer for the duration of the resolve/advise path.
pub unsafe fn resolve_event_subscription_transport(
    binding: &ComBinding,
    event: ComMemberToken,
    dispatch: *mut RawIDispatch,
    com_state: Arc<Mutex<WindowsComClientState>>,
    subscription: ComSubscriptionToken,
    expected_arity: usize,
) -> Result<WindowsComSubscriptionTransport, String> {
    if binding.native_dispatch == 0 {
        return Ok(WindowsComSubscriptionTransport::Projection);
    }
    let Some(spec) = binding.event_specs.get(&event) else {
        return Ok(WindowsComSubscriptionTransport::Projection);
    };
    let Some(connection_point_iid) = spec.connection_point_iid.as_deref() else {
        if matches!(spec.path, ComEventPath::SourceInterface) {
            return Err(
                "COM-E-EVENT-PATH-UNSUPPORTED: source-interface COM event callbacks (COM-EVT-B) require connection-point metadata in current lane".to_string(),
            );
        }
        return Ok(WindowsComSubscriptionTransport::Projection);
    };
    let advised = unsafe {
        advise_event_subscription(
            dispatch,
            com_state,
            subscription,
            spec,
            expected_arity,
            connection_point_iid,
        )
    }?;
    Ok(match advised {
        Some(native) => WindowsComSubscriptionTransport::NativeConnectionPoint(native),
        None => WindowsComSubscriptionTransport::Projection,
    })
}

fn lock_state<'a>(
    com_state: &'a Arc<Mutex<WindowsComClientState>>,
    op: &'static str,
) -> Result<MutexGuard<'a, WindowsComClientState>, String> {
    com_state
        .lock()
        .map_err(|_| format!("COM-E-STATE-LOCK-POISONED: {op} state lock poisoned"))
}

pub fn insert_bound_object_binding_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    binding: ComBinding,
) -> Result<ObjectRef, String> {
    let mut state = lock_state(com_state, "insert_bound_object_binding")?;
    Ok(insert_bound_object_binding(&mut state, binding))
}

pub fn bind_host_dispatch_object_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    prog_id_name: &str,
    binding: ComBinding,
) -> Result<ObjectRef, String> {
    let mut state = lock_state(com_state, "bind_host_dispatch_object")?;
    Ok(bind_host_dispatch_object(&mut state, prog_id_name, binding))
}

pub fn host_dispatch_object_for_prog_id_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    prog_id_name: &str,
) -> Result<Option<ObjectRef>, String> {
    let mut state = lock_state(com_state, "host_dispatch_object_for_prog_id")?;
    Ok(host_dispatch_object_for_prog_id(&mut state, prog_id_name))
}

pub fn insert_bound_object_binding_at_handle_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectRef,
    binding: ComBinding,
) -> Result<ObjectRef, String> {
    let mut state = lock_state(com_state, "insert_bound_object_binding_at_handle")?;
    Ok(insert_bound_object_binding_at_handle(
        &mut state, object, binding,
    ))
}

pub fn resolve_bound_native_dispatch_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectRef,
) -> Result<*mut RawIDispatch, String> {
    let state = lock_state(com_state, "resolve_bound_native_dispatch")?;
    resolve_bound_native_dispatch(&state, object)
}

pub fn resolve_bound_runtime_object_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectRef,
) -> Result<ObjectRef, String> {
    let state = lock_state(com_state, "resolve_bound_runtime_object")?;
    resolve_bound_runtime_object(&state, object)
}

/// # Safety
///
/// dispatch must be null or carry one retained IDispatch reference owned by the caller.
pub unsafe fn bind_native_dispatch_result_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    dispatch: *mut RawIDispatch,
    prog_id_hint: &str,
) -> Result<ObjectRef, String> {
    let mut state = lock_state(com_state, "bind_native_dispatch_result")?;
    Ok(bind_native_dispatch_result(
        &mut state,
        dispatch,
        prog_id_hint,
    ))
}

/// # Safety
///
/// dispatch must be null or carry one retained IDispatch reference owned by the caller.
pub unsafe fn bind_native_runtime_object_result_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    dispatch: *mut RawIDispatch,
    prog_id_hint: &str,
) -> Result<ObjectRef, String> {
    let handle = unsafe { bind_native_dispatch_result_shared(com_state, dispatch, prog_id_hint) }?;
    if handle.raw() == 0 {
        return Ok(handle);
    }
    resolve_bound_runtime_object_shared(com_state, handle)
}

pub fn release_object_binding_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectRef,
) -> Result<ReleasedWindowsComObject, String> {
    let mut state = lock_state(com_state, "release_object_binding")?;
    release_object_binding(&mut state, object)
}

pub fn mark_next_callback_pumped_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
) -> Result<Option<ComCallbackToken>, String> {
    let mut state = lock_state(com_state, "mark_next_callback_pumped")?;
    Ok(state.mark_next_callback_pumped())
}

/// # Safety
///
/// The caller must ensure the current thread is in the required COM apartment
/// before native connection-point subscription work is attempted.
pub unsafe fn subscribe_event_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectRef,
    event: ComMemberToken,
) -> Result<(ComSubscriptionToken, WindowsComSubscriptionTransport, usize), String> {
    let (binding, expected_arity, subscription) = {
        let mut state = lock_state(com_state, "subscribe_event")?;
        let Some(binding) = state
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .cloned()
        else {
            return Err(format!(
                "COM-E-EVENT-CONNECTIONPOINT-MISSING: unknown COM object token {}",
                object.raw()
            ));
        };
        let Some(expected_arity) = event_signature_arity_for_binding(&binding, event) else {
            return Err(format!(
                "COM-E-EVENT-CONNECTIONPOINT-MISSING: object `{}` does not expose event token {}",
                binding.prog_id_name,
                event.raw()
            ));
        };
        let subscription = state.allocate_subscription();
        (binding, expected_arity, subscription)
    };
    let transport = unsafe {
        resolve_event_subscription_transport(
            &binding,
            event,
            binding.native_dispatch as *mut RawIDispatch,
            Arc::clone(com_state),
            subscription,
            expected_arity,
        )
    }?;
    let mut state = lock_state(com_state, "subscribe_event_insert")?;
    if !state
        .bindings
        .contains_key(&ComObjectToken::new(object.raw()))
    {
        if let WindowsComSubscriptionTransport::NativeConnectionPoint(native) = transport {
            unsafe {
                let _ = release_subscription_transport(
                    WindowsComSubscriptionTransport::NativeConnectionPoint(native),
                );
            }
        }
        return Err(format!(
            "COM-E-EVENT-CONNECTIONPOINT-MISSING: unknown COM object token {}",
            object.raw()
        ));
    }
    state.subscriptions.insert(
        subscription,
        crate::ComEventSubscription {
            object: object.raw(),
            event,
            transport,
        },
    );
    Ok((subscription, transport, expected_arity))
}

/// # Safety
///
/// The caller must ensure the current thread is in the required COM apartment
/// before native connection-point unsubscription work is attempted.
pub unsafe fn unsubscribe_event_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    subscription: ComSubscriptionToken,
) -> Result<(), String> {
    // Remove the subscription from the state BEFORE releasing the transport:
    // release_subscription_transport drops our only connection-point
    // reference even when Unadvise reports failure, so a transport left in
    // the map would be Unadvised/Released a second time by release_object or
    // the state Drop — through an interface we no longer own (W1-com-004).
    // Resolving and removing under one lock also closes the gap where a
    // concurrent path could observe the subscription mid-teardown.
    let transport = {
        let mut state = lock_state(com_state, "unsubscribe_event_transport")?;
        let transport = resolve_subscription_transport(&state, subscription)?;
        let _ = remove_subscription_callbacks(&mut state, subscription)?;
        transport
    };
    unsafe { release_subscription_transport(transport) }
}

pub fn queue_projection_event_callbacks_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectRef,
    binding: &ComBinding,
    member: ComMemberToken,
    args: &[ComInvokeArg],
) -> Result<usize, String> {
    if !binding.event_trigger_specs.contains_key(&member) {
        return Ok(0);
    }
    let Some((event, args)) = event_callback_args_from_invoke_args(binding, member, args)? else {
        return Ok(0);
    };
    let Some(expected_arity) = event_signature_arity_for_binding(binding, event) else {
        return Err(format!(
            "COM-E-EVENT-CONNECTIONPOINT-MISSING: object `{}` does not expose event token {}",
            binding.prog_id_name,
            event.raw()
        ));
    };
    if event_is_source_interface_only(binding, event) {
        return Ok(0);
    }
    if args.len() != expected_arity {
        return Err(format!(
            "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: event token {} expected {} argument(s), queued {}",
            event.raw(),
            expected_arity,
            args.len()
        ));
    }
    let mut state = lock_state(com_state, "queue_projection_event_callbacks")?;
    Ok(
        state.queue_callbacks_for_source_event(&object, event, args.as_slice(), |transport| {
            transport.is_projection()
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        WindowsComClientState, WindowsComSubscriptionTransport,
        bind_native_runtime_object_result_shared, event_callback_args_from_invoke_args,
        queue_projection_event_callbacks_shared,
    };
    use crate::{
        ComBinding, ComEventPath, ComEventSpec, ComEventSubscription, ComEventTriggerSpec,
        ComInvokeArg, ComMemberToken, ComValue,
    };
    use oxvba_runtime::{ObjectRef, bstr::BStr};
    use std::sync::{Arc, Mutex};

    #[test]
    fn null_native_runtime_object_result_preserves_nothing_identity() {
        let state = Arc::new(Mutex::new(WindowsComClientState::default()));

        let object = unsafe {
            bind_native_runtime_object_result_shared(
                &state,
                std::ptr::null_mut(),
                "Excel.Range.Find",
            )
        }
        .expect("null native runtime object should bind as Nothing");

        assert_eq!(object.raw(), 0);
        assert!(
            state.lock().expect("state").bindings.is_empty(),
            "Nothing should not allocate a COM binding"
        );
    }

    #[test]
    fn projection_event_callback_args_accept_non_legacy_com_values() {
        let mut binding = ComBinding::new("Test.Object".to_string(), 0);
        binding.event_trigger_specs.insert(
            ComMemberToken::new(7),
            ComEventTriggerSpec {
                event_token: ComMemberToken::new(11),
                callback_arity: 1,
                second_arg_is_incremented: false,
            },
        );
        binding.event_specs.insert(
            ComMemberToken::new(11),
            ComEventSpec {
                callback_arity: 1,
                path: ComEventPath::Dispatch,
                connection_point_iid: None,
                dispatch_member_id: Some(11),
            },
        );

        let values = event_callback_args_from_invoke_args(
            &binding,
            ComMemberToken::new(7),
            &[ComInvokeArg::positional_value(ComValue::String(
                BStr::from("payload"),
            ))],
        )
        .expect("projection callback args should be widened from invoke args")
        .expect("event trigger should be recognized");

        assert_eq!(values.0.raw(), 11);
        assert_eq!(values.1, vec![ComValue::String(BStr::from("payload"))]);
    }

    #[test]
    fn projection_callback_queue_preserves_runtime_object_identity_token() {
        let object = ObjectRef::from_compat_identity(20_111);
        let subscription = crate::ComSubscriptionToken::new(40_111);
        let state = Arc::new(Mutex::new(WindowsComClientState::default()));
        {
            let mut locked = state.lock().expect("state");
            locked.subscriptions.insert(
                subscription,
                ComEventSubscription {
                    object: object.raw(),
                    event: ComMemberToken::new(11),
                    transport: WindowsComSubscriptionTransport::Projection,
                },
            );
        }

        let mut binding = ComBinding::new("Test.Object".to_string(), 0);
        binding.event_trigger_specs.insert(
            ComMemberToken::new(7),
            ComEventTriggerSpec {
                event_token: ComMemberToken::new(11),
                callback_arity: 1,
                second_arg_is_incremented: false,
            },
        );
        binding.event_specs.insert(
            ComMemberToken::new(11),
            ComEventSpec {
                callback_arity: 1,
                path: ComEventPath::Dispatch,
                connection_point_iid: None,
                dispatch_member_id: Some(11),
            },
        );

        let queued = queue_projection_event_callbacks_shared(
            &state,
            object.clone(),
            &binding,
            ComMemberToken::new(7),
            &[ComInvokeArg::positional_value(ComValue::String(
                BStr::from("payload"),
            ))],
        )
        .expect("projection callback queue should succeed");
        assert_eq!(queued, 1);

        let payload = state
            .lock()
            .expect("state")
            .take_polled_callback()
            .expect("queued payload");
        assert_eq!(payload.subscription, subscription);
        assert_eq!(payload.object.raw(), object.raw());
        assert_eq!(payload.event.raw(), 11);
        assert_eq!(
            payload
                .args
                .iter()
                .map(|value| value.to_com_value())
                .collect::<Vec<_>>(),
            vec![ComValue::String(BStr::from("payload"))]
        );
    }
}
