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

type DispatchEventCallback = Arc<dyn Fn(&[ComValue], &[(usize, u32)]) -> bool + Send + Sync>;

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
    /// Per-interface-IID cache of "is this interface marshaled by PSOAInterface?"
    /// (`HKCR\Interface\{iid}\ProxyStubClsid32 == {00020424-…}`, the oleaut
    /// universal marshaler whose proxy is a typelib-aligned, vtable-callable slot
    /// table). The vtable dispatch site reads this when an object is a marshaling
    /// proxy, to decide whether an out-of-process slot call is ABI-safe (PSOA) or
    /// must fall back to IDispatch (PSDispatch / any other / missing). Keyed by the
    /// braced uppercase IID string so the registry read happens once per IID, not
    /// once per call. Holds no COM references — needs no teardown in `Drop`.
    psoa_interface_iid_cache: BTreeMap<String, bool>,
}

impl WindowsComClientState {
    /// Look up the cached "interface IID is marshaled by PSOAInterface" decision.
    pub fn psoa_interface_cache_get(&self, iid_braces: &str) -> Option<bool> {
        self.psoa_interface_iid_cache.get(iid_braces).copied()
    }

    /// Record the "interface IID is marshaled by PSOAInterface" decision so the
    /// registry probe runs once per interface IID for the bridge's lifetime.
    pub fn psoa_interface_cache_put(&mut self, iid_braces: String, is_psoa: bool) {
        self.psoa_interface_iid_cache.insert(iid_braces, is_psoa);
    }
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
                // SAFETY: every transport still present in `subscriptions` was produced by
                // `advise_event_subscription` and has not been released: unsubscribe and
                // release_object always remove a subscription from this map before
                // releasing its transport (W1-com-004), so this Drop owns the one final
                // Unadvise/Release. The result is ignored because guest teardown must
                // never abort the host.
                unsafe {
                    let _ = unadvise_connection_point(native);
                }
            }
        }
        self.subscriptions.clear();
        // Revoke GIT registrations of any queued-but-unpumped object event args so a
        // bridge discarded with live callbacks does not leave the source objects pinned
        // in the process Global Interface Table (the map is cleared immediately after, so
        // each cookie is revoked exactly once).
        let orphan_cookies: Vec<u32> = self
            .callbacks
            .values()
            .flat_map(|callback| callback.pending_marshals.iter().map(|(_, cookie)| *cookie))
            .collect();
        revoke_marshal_cookies(orphan_cookies);
        self.callbacks.clear();
        self.pending_callbacks.clear();
        self.host_objects_by_prog_id.clear();
        for binding in self.bindings.values_mut() {
            if binding.native_dispatch != 0 {
                // SAFETY: the bindings map owns exactly one retained `IDispatch` reference
                // per native binding, established when the binding was created; the
                // non-zero check guarantees the reference exists and zeroing the field
                // right after ensures it is released exactly once.
                unsafe {
                    release_dispatch(binding.native_dispatch as *mut RawIDispatch);
                }
                binding.native_dispatch = 0;
            }
            if binding.native_unknown != 0 {
                // SAFETY: the bindings map owns exactly one retained `IUnknown` identity
                // reference per binding, obtained via `query_unknown_from_dispatch` at bind
                // time; the non-zero check guarantees the reference exists and zeroing the
                // field right after ensures it is released exactly once.
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
    // Hold the shared state WEAKLY: the native connection point owns this
    // closure until Unadvise, and the only implicit Unadvise lives in
    // WindowsComClientState::Drop — a strong Arc here therefore formed a
    // cycle (state → binding → server connection point → sink → state) that
    // kept the state, every retained IDispatch/IUnknown reference, and the
    // sinks alive forever once a bridge was discarded with live
    // subscriptions (W1-com-008). An event arriving after the last strong
    // reference is gone is reported unconsumed — the runtime that would
    // drain it no longer exists.
    let com_state = Arc::downgrade(&com_state);
    Arc::new(move |args: &[ComValue], marshals: &[(usize, u32)]| {
        let Some(com_state) = com_state.upgrade() else {
            // The runtime that would drain this event is gone; revoke the object-arg GIT
            // registrations this delivery just made rather than leak them.
            revoke_marshal_cookies(marshals.iter().map(|(_, cookie)| *cookie));
            return false;
        };
        let mut state = match com_state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let queued =
            state.queue_callback_for_subscription_with_marshals(subscription, args, marshals);
        if !queued {
            // The subscription was torn down between this delivery's GIT registration and
            // the queue attempt (the callback was never stored, so no double-revoke);
            // revoke the now-orphaned object-arg cookies instead of leaking them.
            revoke_marshal_cookies(marshals.iter().map(|(_, cookie)| *cookie));
        }
        queued
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
    // SAFETY: `dispatch` was checked non-null above, and per this function's `# Safety`
    // the caller transferred one retained reference to us, so it is a live `IDispatch`
    // whose `IUnknown` vtable can be queried.
    let unknown = match unsafe { query_unknown_from_dispatch(dispatch) } {
        Ok(unknown) => unknown,
        Err(_) => {
            // SAFETY: QueryInterface failed so no IUnknown reference was retained; we own
            // the caller's single retained `IDispatch` reference (non-null, checked above)
            // and release it exactly once before returning Nothing.
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
        // SAFETY: an existing binding already owns retained dispatch/unknown references for
        // this COM identity, so the caller's incoming `IDispatch` reference (ours per this
        // function's `# Safety`) and the `IUnknown` reference just AddRef'd by
        // `query_unknown_from_dispatch` are surplus duplicates; each is released exactly
        // once here.
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

/// Revoke the Global Interface Table registrations of one or more event-callback
/// object arguments that will never be poll-revived. EVERY teardown path that
/// discards a callback (or a just-delivered marshal set) before the VM pump revives
/// it — unsubscribe ([`remove_subscription_callbacks`]), release
/// ([`release_callback`] / [`release_object_binding`]), a declined queue
/// ([`callback_sink`]), or client [`WindowsComClientState`] drop — must funnel its
/// `pending_marshals` cookies through here. Otherwise the GIT keeps its own
/// AddRef'd reference to each source object forever, pinning it and potentially
/// preventing an out-of-process source (Excel) from shutting down.
///
/// Safe against double-revoke: a revived callback has its `pending_marshals` cleared
/// by [`resolve_pending_event_marshals_for_next`], and every teardown path removes
/// the callback from `state.callbacks` after (or instead of) revoking, so each cookie
/// reaches `revoke_git_cookie` at most once.
fn revoke_marshal_cookies(cookies: impl IntoIterator<Item = u32>) {
    for cookie in cookies {
        // SAFETY: cookies are revoked on a COM-initialized thread (the VM/STA thread for
        // teardown, the COM-servicing delivery thread for a declined queue); each was
        // registered by our own sink and is revoked at most once (see the doc above).
        unsafe { crate::windows_connection_point::revoke_git_cookie(cookie) };
    }
}

pub fn release_object_binding(
    state: &mut WindowsComClientState,
    object: ObjectRef,
) -> Result<ReleasedWindowsComObject, String> {
    // Revoke GIT registrations for any of this object's event callbacks that were
    // queued but never pumped, so a purged callback does not pin its object arguments
    // in the Global Interface Table (which would hold a COM reference forever and can
    // keep an out-of-process source from shutting down). The callbacks themselves are
    // removed by `release_object_state` below, so a revoked cookie is never revived.
    let orphan_cookies: Vec<u32> = state
        .callbacks
        .values()
        .filter(|callback| callback.object == object.raw())
        .flat_map(|callback| callback.pending_marshals.iter().map(|(_, cookie)| *cookie))
        .collect();
    revoke_marshal_cookies(orphan_cookies);
    let Some((binding, transports, stale_callbacks)) =
        state.release_object_state(ComObjectToken::new(object.raw()))
    else {
        return Err(format!(
            "COM-E-OBJECT-MISSING: unknown COM object token {}",
            object.raw()
        ));
    };
    if binding.native_dispatch != 0 {
        // SAFETY: `release_object_state` just removed this binding from the map, so this
        // frame is the exclusive owner of the map's single retained `IDispatch` reference
        // (established at bind time); the non-zero check guarantees the reference exists
        // and the binding is dropped after, so it is released exactly once.
        unsafe {
            release_dispatch(binding.native_dispatch as *mut RawIDispatch);
        }
    }
    if binding.native_unknown != 0 {
        // SAFETY: same exclusivity as the dispatch release above — the binding was removed
        // from the map, so this frame owns the single retained `IUnknown` identity
        // reference obtained at bind time, released exactly once here.
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
    // Revive any cross-apartment object event arguments for the callback about to be
    // dispatched: the agile sink registered them in the GIT on its (MTA) delivery
    // thread and queued `Nothing` placeholders; here, on the VM/STA thread, we
    // unmarshal each into a thread-correct binding so the handler receives the real
    // object (e.g. the new `Workbook`, the changed `Range`).
    resolve_pending_event_marshals_for_next(state);
    state.take_polled_callback()
}

/// The callback token `take_polled_callback` will next consume: the one already
/// marked pumped (by a prior `DoEvents`) if any, otherwise the front of the queue.
fn next_pollable_callback(state: &WindowsComClientState) -> Option<ComCallbackToken> {
    state
        .last_pumped_callback
        .or_else(|| state.pending_callbacks.front().copied())
}

/// Unmarshals the GIT-registered object arguments of the next pollable callback into
/// thread-correct bindings on the current (VM/STA) apartment, overwriting their
/// `Nothing` placeholders. A cookie that fails to revive leaves `Nothing` in place
/// (the handler then sees `Nothing` for that argument rather than the event failing).
fn resolve_pending_event_marshals_for_next(state: &mut WindowsComClientState) {
    let Some(token) = next_pollable_callback(state) else {
        return;
    };
    let marshals = match state.callbacks.get(&token) {
        Some(callback) if !callback.pending_marshals.is_empty() => {
            callback.pending_marshals.clone()
        }
        _ => return,
    };
    for (arg_index, cookie) in marshals {
        // SAFETY: the VM thread is COM-initialized before any event poll; the cookie was
        // registered by our own sink and is consumed (revoked) exactly once here.
        let dispatch = unsafe { crate::windows_connection_point::take_dispatch_from_git(cookie) };
        let Some(dispatch) = dispatch else {
            continue;
        };
        // SAFETY: `take_dispatch_from_git` returned a live `IDispatch` carrying one retained
        // reference owned by us; `bind_native_dispatch_result` takes ownership of it.
        let object = unsafe { bind_native_dispatch_result(state, dispatch, "<com-event-arg>") };
        let value = ComEventCallbackValue::from_com_value(&ComValue::Object(object));
        if let Some(callback) = state.callbacks.get_mut(&token)
            && let Some(slot) = callback.args.get_mut(arg_index)
        {
            *slot = value;
        }
    }
    if let Some(callback) = state.callbacks.get_mut(&token) {
        callback.pending_marshals.clear();
    }
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
    let Some(removed) = state.callbacks.remove(&callback) else {
        return Err(format!(
            "COM-E-EVENT-CALLBACK-MISSING: unknown callback token {}",
            callback.raw()
        ));
    };
    // A callback released before it is poll-revived (e.g. `DoEvents` marked it pumped,
    // the handler read its args, then released it without ever polling) still owns its
    // object-arg GIT registrations; revoke them so the source objects are not pinned.
    revoke_marshal_cookies(removed.pending_marshals.iter().map(|(_, cookie)| *cookie));
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
        if let Some(removed) = state.callbacks.remove(callback) {
            // Unsubscribing (WithEvents = Nothing) before a queued event is pumped must
            // revoke the dropped callback's object-arg GIT registrations, or the source's
            // object arguments stay pinned in the Global Interface Table forever.
            revoke_marshal_cookies(removed.pending_marshals.iter().map(|(_, cookie)| *cookie));
        }
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
    intended_kind: crate::TypeLibMemberInvokeKind,
    fallback_spec: Option<ComMemberSpec>,
) -> Result<Option<(i32, ComMemberSpec)>, String> {
    // Select the spec for the intended access kind so a read/write property's
    // get / let / set FUNCDESCs (which share a memid) each resolve to their OWN
    // vtable slot and ABI param shape rather than collapsing to one.
    let spec = if let Some(spec) = binding.lookup_member_spec(member, intended_kind).cloned() {
        spec
    } else if let Some(spec) = fallback_spec {
        spec
    } else {
        return Ok(None);
    };
    if let Some(dispid) = binding.member_dispids.get(&member).copied() {
        return Ok(Some((dispid, spec)));
    }
    // SAFETY: forwarded caller contract — this function's `# Safety` requires `dispatch` to
    // be a valid live `IDispatch` for the duration of the lookup, which is exactly what
    // `get_dispid_by_name` requires.
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
    // SAFETY: `dispatch` is live per this function's `# Safety` contract; `spec` and
    // `connection_point_iid` come from this binding's own `event_specs`, so they describe
    // the event interface implemented by that object; and the sink holds `com_state` only
    // weakly (W1-com-008), so the shared-state-outlives-transport requirement is met by the
    // bridge that owns the strong Arc.
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
    // SAFETY: forwarded caller contract — this function's `# Safety` requires `dispatch` to
    // be null or carry one retained `IDispatch` reference owned by the caller, the exact
    // precondition of the shared binding path (which takes ownership of that reference).
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
        let token = ComObjectToken::new(object.raw());
        if !state.bindings.contains_key(&token) {
            return Err(format!(
                "COM-E-EVENT-CONNECTIONPOINT-MISSING: unknown COM object token {}",
                object.raw()
            ));
        }
        // Lazily recover event metadata from the live object when this binding carries
        // none for the requested event. An object returned by a method call (e.g. a
        // Workbook from `Workbooks.Add`) is bound with no typelib metadata, so its
        // `event_specs` start empty; recover them from the object's own type
        // information (the same path a `CreateObject`'d source uses) so `WithEvents`
        // on the returned object can subscribe.
        let needs_recovery = state.bindings.get(&token).is_some_and(|binding| {
            binding.native_dispatch != 0
                && event_signature_arity_for_binding(binding, event).is_none()
        });
        if needs_recovery {
            let (dispatch, prog_id) = {
                let binding = state.bindings.get(&token).expect("binding present");
                (binding.native_dispatch, binding.prog_id_name.clone())
            };
            // SAFETY: `dispatch` is the live `IDispatch` this binding retains for its
            // lifetime (released only on the VM thread, which is the thread inside this
            // call); recovery only reads its type information.
            let recovered = unsafe {
                crate::windows_typelib_loader::build_metadata_blob_from_dispatch(
                    dispatch as *mut RawIDispatch,
                    &prog_id,
                )
            }
            .map(|blob| binding_from_typelib_metadata(prog_id, dispatch, Some(&blob)));
            if let Some(recovered) = recovered
                && let Some(stored) = state.bindings.get_mut(&token)
            {
                // Merge ONLY the recovered event specs. `build_metadata_blob_from_dispatch`
                // recovers events-only (its `members` is always empty, by design — walking a
                // marshalled out-of-process typelib per member is catastrophically slow), so
                // member calls keep going through the independent live-recovery path. There
                // is deliberately no `member_specs` merge here: it could only ever copy
                // empty-over-empty, and a real merge would silently route OOP member calls
                // onto the slow declined-vtable path this whole design avoids.
                for (event_token, spec) in recovered.event_specs {
                    stored.event_specs.entry(event_token).or_insert(spec);
                }
            }
        }
        let binding = state
            .bindings
            .get(&token)
            .cloned()
            .expect("binding present");
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
    // SAFETY: the state lock is deliberately dropped here because Advise can re-enter; the
    // dispatch pointer stays live across this unlocked window because the bindings map owns
    // one retained `IDispatch` reference for the handle and bindings are only released from
    // the VM thread that is currently inside this subscribe (cross-thread state access is
    // limited to event sinks, which only queue callback payloads). A zero
    // `native_dispatch` is fine: the callee returns the projection transport before
    // touching the pointer. The COM apartment is supplied by this function's `# Safety`.
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
            // SAFETY: this transport was just produced by the advise above and was never
            // inserted into the subscriptions map (the binding vanished mid-subscribe), so
            // this rollback path is the sole owner of its one final Unadvise/Release; the
            // COM apartment is supplied by this function's `# Safety` contract.
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
    // SAFETY: the transport was resolved and removed from the subscriptions map under one
    // lock above, so no other path (release_object, state Drop) can release it again
    // (W1-com-004); this call therefore owns the one final Unadvise/Release, and the COM
    // apartment is supplied by this function's `# Safety` contract.
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
// Test-support code exercising the documented production binding paths.
#[allow(clippy::undocumented_unsafe_blocks)]
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
    fn callback_sink_does_not_keep_state_alive() {
        // W1-com-008: the sink closure is owned by the native connection
        // point until Unadvise, and the only implicit Unadvise lives in
        // WindowsComClientState::Drop — so a strong capture formed a cycle
        // that pinned the state (and its retained COM references) forever
        // once a bridge was discarded with live subscriptions.
        let state = Arc::new(Mutex::new(WindowsComClientState::default()));
        let weak = Arc::downgrade(&state);
        let sink = super::callback_sink(Arc::clone(&state), crate::ComSubscriptionToken::new(1));
        drop(state);
        assert!(
            weak.upgrade().is_none(),
            "the event sink must not keep the shared state alive"
        );
        // A late event after teardown is reported unconsumed, not a panic.
        assert!(!sink(&[], &[]));
    }

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
