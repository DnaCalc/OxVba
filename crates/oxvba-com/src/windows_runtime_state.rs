#![allow(unsafe_op_in_unsafe_fn)]

use crate::{
    ComBinding, ComCallbackToken, ComEventPath, ComEventSpec, ComMemberSpec, ComMemberToken,
    ComObjectToken, ComRuntimeState, ComSubscriptionToken, ComValue, DispatchEventSinkConfig,
    RawIDispatch, WindowsConnectionPointTransport, binding_from_typelib_metadata,
    get_dispid_by_name, release_dispatch, source_interface_event_spec_supported,
    try_advise_dispatch_event_sink, try_advise_single_i32_source_interface_event_sink,
    unadvise_connection_point,
};

use oxvba_runtime::ObjectHandle;
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
        for binding in self.bindings.values_mut() {
            if binding.native_dispatch != 0 {
                unsafe {
                    release_dispatch(binding.native_dispatch as *mut RawIDispatch);
                }
                binding.native_dispatch = 0;
            }
        }
        self.bindings.clear();
    }
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

pub fn event_callback_args_from_member_token(
    binding: &ComBinding,
    member: ComMemberToken,
    args: &[i32],
) -> Option<(ComMemberToken, Vec<ComValue>)> {
    let trigger_spec = binding.event_trigger_specs.get(&member)?;
    if args.len() < trigger_spec.callback_arity {
        return None;
    }
    let mut values: Vec<ComValue> = args
        .iter()
        .copied()
        .take(trigger_spec.callback_arity)
        .map(ComValue::I32)
        .collect();
    if trigger_spec.second_arg_is_incremented
        && values.len() >= 2
        && let ComValue::I32(first) = values[0]
    {
        values[1] = ComValue::I32(first.saturating_add(1));
    }
    Some((trigger_spec.event_token, values))
}

pub fn collect_stale_callbacks_for_subscription(
    state: &WindowsComClientState,
    subscription: ComSubscriptionToken,
    object: ComObjectToken,
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
    object: ObjectHandle,
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

/// # Safety
///
/// dispatch must be null or carry one retained IDispatch reference owned by the caller.
pub unsafe fn bind_native_dispatch_result(
    state: &mut WindowsComClientState,
    dispatch: *mut RawIDispatch,
    prog_id_hint: &str,
) -> ObjectHandle {
    if dispatch.is_null() {
        return ObjectHandle::new(0);
    }
    if let Some((handle, _)) = state
        .bindings
        .iter()
        .find(|(_, binding)| binding.native_dispatch == dispatch as usize)
    {
        unsafe {
            release_dispatch(dispatch);
        }
        return ObjectHandle::new(handle.raw());
    }
    let handle = state.allocate_handle();
    state.bindings.insert(
        handle,
        binding_from_typelib_metadata(
            format!("{prog_id_hint}::<invoke-result>"),
            dispatch as usize,
            None,
        ),
    );
    ObjectHandle::new(handle.raw())
}

pub fn release_object_binding(
    state: &mut WindowsComClientState,
    object: ObjectHandle,
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
) -> Result<ComValue, String> {
    let Some(payload) = state.callbacks.get(&callback) else {
        return Err(format!(
            "COM-E-EVENT-CALLBACK-MISSING: unknown callback token {}",
            callback.raw()
        ));
    };
    payload.args.get(index).cloned().ok_or_else(|| {
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
    binding: ComBinding,
) -> ObjectHandle {
    let handle = state.allocate_handle();
    state.bindings.insert(handle, binding);
    ObjectHandle::new(handle.raw())
}

pub fn insert_bound_object_binding_at_handle(
    state: &mut WindowsComClientState,
    object: ObjectHandle,
    binding: ComBinding,
) -> ObjectHandle {
    state
        .bindings
        .insert(ComObjectToken::new(object.raw()), binding);
    object
}

pub fn cache_member_dispid(
    state: &mut WindowsComClientState,
    object: ObjectHandle,
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
    object: ObjectHandle,
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
) -> Result<ObjectHandle, String> {
    let mut state = lock_state(com_state, "insert_bound_object_binding")?;
    Ok(insert_bound_object_binding(&mut state, binding))
}

pub fn insert_bound_object_binding_at_handle_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectHandle,
    binding: ComBinding,
) -> Result<ObjectHandle, String> {
    let mut state = lock_state(com_state, "insert_bound_object_binding_at_handle")?;
    Ok(insert_bound_object_binding_at_handle(
        &mut state, object, binding,
    ))
}

pub fn resolve_bound_native_dispatch_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectHandle,
) -> Result<*mut RawIDispatch, String> {
    let state = lock_state(com_state, "resolve_bound_native_dispatch")?;
    resolve_bound_native_dispatch(&state, object)
}

/// # Safety
///
/// dispatch must be null or carry one retained IDispatch reference owned by the caller.
pub unsafe fn bind_native_dispatch_result_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    dispatch: *mut RawIDispatch,
    prog_id_hint: &str,
) -> Result<ObjectHandle, String> {
    let mut state = lock_state(com_state, "bind_native_dispatch_result")?;
    Ok(bind_native_dispatch_result(
        &mut state,
        dispatch,
        prog_id_hint,
    ))
}

pub fn release_object_binding_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectHandle,
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
    object: ObjectHandle,
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
            object: ComObjectToken::new(object.raw()),
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
    let transport = {
        let state = lock_state(com_state, "unsubscribe_event_transport")?;
        resolve_subscription_transport(&state, subscription)?
    };
    unsafe {
        release_subscription_transport(transport)?;
    }
    let mut state = lock_state(com_state, "unsubscribe_event_remove")?;
    let _ = remove_subscription_callbacks(&mut state, subscription)?;
    Ok(())
}

pub fn queue_projection_event_callbacks_shared(
    com_state: &Arc<Mutex<WindowsComClientState>>,
    object: ObjectHandle,
    binding: &ComBinding,
    member: ComMemberToken,
    args: Option<&[i32]>,
) -> Result<usize, String> {
    let Some(trigger_spec) = binding.event_trigger_specs.get(&member).copied() else {
        return Ok(0);
    };
    let Some(args) = args else {
        return Err(format!(
            "COM-E-VALUE-TRANSPORT-UNSUPPORTED: projected event trigger `{}` requires legacy callback argument transport",
            trigger_spec.event_token.raw()
        ));
    };
    let Some((event, args)) = event_callback_args_from_member_token(binding, member, args) else {
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
    Ok(state.queue_callbacks_for_source_event(
        ComObjectToken::new(object.raw()),
        event,
        args.as_slice(),
        |transport| transport.is_projection(),
    ))
}
