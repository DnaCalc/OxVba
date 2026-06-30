#![allow(clippy::result_large_err)]

use crate::{
    ComBinding, ComCallbackPayload, ComCallbackToken, ComInvokeArg, ComInvokeFailure,
    ComInvokeRequest, ComMemberSpec, ComMemberToken, ComObjectDescriptor, ComObjectToken,
    ComSubscriptionToken, DynamicCallKind, DynamicCallRequest, DynamicMemberSelector, RawIDispatch,
    ReleasedWindowsComObject, TypeLibCacheScope, TypeLibMetadataBlob, TypeLibMetadataCacheState,
    TypeLibResolveRequest, TypeLibResolvedIdentity, WindowsComClientState,
    activate_runtime_dispatch, activate_runtime_object_binding_shared,
    bind_host_dispatch_object_shared, bind_native_dispatch_result_shared,
    binding_from_typelib_metadata, build_typelib_metadata, callback_arg, callback_arity,
    callback_subscription_token, execute_bound_runtime_call_result_with_shared_state,
    execute_bound_variant_with_shared_state, host_dispatch_object_for_prog_id_shared,
    insert_bound_object_binding_at_handle_shared, invoke_dispatch_variant_with_shared_state,
    legacy_runtime_arg_values, member_spec_from_typelib_metadata,
    member_token_and_spec_from_typelib_metadata_name, query_unknown_from_dispatch,
    release_callback, release_object_binding_shared, release_subscription_transport,
    resolve_bound_native_dispatch_shared, resolve_known_typelib_identity,
    resolve_named_argument_dispids, resolve_typelib_identity_for_prog_id_name,
    subscribe_event_shared, take_polled_callback_payload, unsubscribe_event_shared,
    validate_named_arg_order,
};
use oxvba_diagnostics::{Diagnostic, DiagnosticPhase, extract_prefixed_code};
use oxvba_runtime::{ObjectRef, RuntimeCallResult, Variant};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Clone)]
pub struct WindowsComBridge {
    state: Arc<Mutex<WindowsComClientState>>,
    typelib_state: Arc<Mutex<TypeLibMetadataCacheState>>,
    force_registered_test_dispatch: bool,
    /// Count of early-bound member calls that dispatched through the COM vtable
    /// slot (the `prefer_vtable` fast path). Mirrors the `last_dll_error` atomic
    /// pattern so a host test can observe which transport carried a member.
    vtable_call_count: Arc<AtomicU64>,
    /// Count of early-bound member calls that dispatched through
    /// `IDispatch::Invoke` (the default path and the vtable fallback).
    idispatch_call_count: Arc<AtomicU64>,
}

#[derive(Debug, Clone)]
pub enum WindowsComBridgeDispatchError {
    Message(String),
    InvokeFailure(ComInvokeFailure),
}

impl WindowsComBridgeDispatchError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            WindowsComBridgeDispatchError::Message(message) => {
                let code = extract_prefixed_code(message, "COM-E-")
                    .unwrap_or_else(|| "COM-E-DISPATCH".to_string());
                Diagnostic::error(code, DiagnosticPhase::Com, message.clone())
            }
            WindowsComBridgeDispatchError::InvokeFailure(failure) => failure.to_diagnostic(),
        }
    }
}

impl WindowsComBridge {
    pub fn new(force_registered_test_dispatch: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(WindowsComClientState::default())),
            typelib_state: Arc::new(Mutex::new(TypeLibMetadataCacheState::default())),
            force_registered_test_dispatch,
            vtable_call_count: Arc::new(AtomicU64::new(0)),
            idispatch_call_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Number of early-bound member calls dispatched through the COM vtable slot
    /// so far (the `prefer_vtable` fast path). A host test reads this to prove a
    /// supported member went through the vtable.
    pub fn vtable_call_count(&self) -> u64 {
        self.vtable_call_count.load(Ordering::Relaxed)
    }

    /// Number of early-bound member calls dispatched through `IDispatch::Invoke`
    /// so far (the default path and the vtable fallback).
    pub fn idispatch_call_count(&self) -> u64 {
        self.idispatch_call_count.load(Ordering::Relaxed)
    }

    pub fn shared_state(&self) -> &Arc<Mutex<WindowsComClientState>> {
        &self.state
    }

    pub fn shared_typelib_state(&self) -> &Arc<Mutex<TypeLibMetadataCacheState>> {
        &self.typelib_state
    }

    pub fn lock_state(
        &self,
        op: &'static str,
    ) -> Result<MutexGuard<'_, WindowsComClientState>, String> {
        self.state
            .lock()
            .map_err(|_| format!("COM-E-STATE-POISONED: COM state lock poisoned during {op}"))
    }

    pub fn lock_typelib_state(
        &self,
        op: &'static str,
    ) -> Result<MutexGuard<'_, TypeLibMetadataCacheState>, String> {
        self.typelib_state.lock().map_err(|_| {
            format!("COM-E-TYPELIB-STATE-POISONED: typelib state lock poisoned during {op}")
        })
    }

    pub fn resolve_typelib_reference(
        &self,
        request: &TypeLibResolveRequest,
    ) -> Result<TypeLibResolvedIdentity, String> {
        resolve_known_typelib_identity(request).ok_or_else(|| {
            let request_key = request
                .importlib_hint
                .as_deref()
                .or(request.libid_hint.as_deref())
                .unwrap_or(request.reference_name.as_str());
            format!("COM-E-TYPELIB-IDENTITY-UNRESOLVED: no typelib identity could be resolved for `{request_key}`")
        })
    }

    pub fn load_typelib_metadata(
        &self,
        identity: &TypeLibResolvedIdentity,
    ) -> Result<TypeLibMetadataBlob, String> {
        let mut state = self.lock_typelib_state("load_typelib_metadata")?;
        Ok(state.load_or_build(identity, build_typelib_metadata))
    }

    pub fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> Result<usize, String> {
        let mut state = self.lock_typelib_state("invalidate_typelib_cache")?;
        state
            .invalidate(scope, reference_name)
            .map_err(str::to_string)
    }

    pub fn load_typelib_metadata_for_prog_id_name(
        &self,
        prog_id_name: &str,
    ) -> Result<Option<TypeLibMetadataBlob>, String> {
        let Some(identity) = resolve_typelib_identity_for_prog_id_name(prog_id_name) else {
            return Ok(None);
        };
        self.load_typelib_metadata(&identity).map(Some)
    }

    pub fn known_member_spec_for_prog_id_name(
        &self,
        prog_id_name: &str,
        member: ComMemberToken,
    ) -> Result<Option<ComMemberSpec>, String> {
        Ok(self
            .load_typelib_metadata_for_prog_id_name(prog_id_name)?
            .as_ref()
            .and_then(|blob| member_spec_from_typelib_metadata(blob, member)))
    }

    pub fn known_member_spec_for_prog_id_name_by_name(
        &self,
        prog_id_name: &str,
        member_name: &str,
    ) -> Result<Option<(ComMemberToken, ComMemberSpec)>, String> {
        Ok(self
            .load_typelib_metadata_for_prog_id_name(prog_id_name)?
            .as_ref()
            .and_then(|blob| member_token_and_spec_from_typelib_metadata_name(blob, member_name)))
    }

    pub fn activate_runtime_dispatch(&self, prog_id: &str) -> Result<*mut RawIDispatch, String> {
        activate_runtime_dispatch(prog_id, self.force_registered_test_dispatch)
    }

    /// # Safety
    /// `dispatch` must be a valid live `IDispatch` pointer for the duration of the lookup.
    pub unsafe fn resolve_named_argument_dispids(
        &self,
        dispatch: *mut RawIDispatch,
        member_name: &str,
        args: &[ComInvokeArg],
    ) -> Result<Vec<i32>, String> {
        // SAFETY: forwarded caller contract — this method's `# Safety` requires `dispatch`
        // to be a valid live `IDispatch` for the duration of the lookup, which is exactly
        // what the free function requires.
        unsafe { resolve_named_argument_dispids(dispatch, member_name, args) }
    }

    pub fn resolve_native_dispatch_for_object(
        &self,
        object: ObjectRef,
    ) -> Result<*mut RawIDispatch, String> {
        resolve_bound_native_dispatch_shared(&self.state, object)
    }

    pub fn activate_runtime_object_binding<F>(
        &self,
        prog_id_name: &str,
        mut configure_binding: F,
    ) -> Result<ObjectRef, String>
    where
        F: FnMut(&mut ComBinding) -> Result<(), String>,
    {
        let metadata = self.load_typelib_metadata_for_prog_id_name(prog_id_name)?;
        activate_runtime_object_binding_shared(
            &self.state,
            prog_id_name,
            metadata.as_ref(),
            self.force_registered_test_dispatch,
            |binding| configure_binding(binding),
        )
    }

    /// `GetObject(, "<ProgID>")` — bind to the currently-running registered instance of a
    /// ProgID (`GetActiveObject`), registering the recovered `IDispatch` as a runtime object
    /// binding exactly like an activated one. Errors when no instance is running.
    pub fn get_active_object(&self, prog_id_name: &str) -> Result<ObjectRef, String> {
        let dispatch = crate::get_active_dispatch_by_prog_id(prog_id_name)?;
        // SAFETY: `get_active_dispatch_by_prog_id` returns one retained IDispatch reference
        // (or errors before returning), and `bind_host_dispatch_object` takes ownership of
        // that single reference on both success and failure.
        unsafe { self.bind_host_dispatch_object(prog_id_name, dispatch) }
    }

    /// `GetObject("<pathname>"[, class])` — bind to the object a file path names
    /// (`CoGetObject`), registering the recovered `IDispatch` as a runtime object binding.
    /// `class`, when given, only labels the object's metadata (the moniker bind uses the
    /// path); without it the object binds late (dispatch-only).
    pub fn bind_file_object(
        &self,
        path: &str,
        class_name: Option<&str>,
    ) -> Result<ObjectRef, String> {
        let dispatch = crate::bind_dispatch_by_path(path)?;
        let label = class_name.unwrap_or(path);
        // SAFETY: `bind_dispatch_by_path` returns one retained IDispatch reference (or errors
        // before returning), and `bind_host_dispatch_object` takes ownership of that single
        // reference on both success and failure.
        unsafe { self.bind_host_dispatch_object(label, dispatch) }
    }

    pub fn bind_projection_object(
        &self,
        object: ObjectRef,
        prog_id_name: &str,
    ) -> Result<ObjectRef, String> {
        let metadata = self.load_typelib_metadata_for_prog_id_name(prog_id_name)?;
        let binding = binding_from_typelib_metadata(prog_id_name.to_string(), 0, metadata.as_ref());
        insert_bound_object_binding_at_handle_shared(&self.state, object, binding)
    }

    /// # Safety
    ///
    /// `dispatch` must be null or carry one retained `IDispatch` reference
    /// owned by the caller. On success or failure this method takes ownership
    /// of that reference.
    pub unsafe fn bind_host_dispatch_object(
        &self,
        prog_id_name: &str,
        dispatch: *mut RawIDispatch,
    ) -> Result<ObjectRef, String> {
        if dispatch.is_null() {
            return Ok(ObjectRef::from_compat_identity(0));
        }
        let metadata = match self.load_typelib_metadata_for_prog_id_name(prog_id_name) {
            Ok(metadata) => metadata,
            Err(message) => {
                // SAFETY: per this method's `# Safety` we took ownership of the caller's one
                // retained `IDispatch` reference, `dispatch` was checked non-null above, and
                // this error return is the sole exit on this path, so the reference is
                // released exactly once.
                unsafe {
                    crate::release_dispatch(dispatch);
                }
                return Err(message);
            }
        };
        let mut binding = binding_from_typelib_metadata(
            prog_id_name.to_string(),
            dispatch as usize,
            metadata.as_ref(),
        );
        // SAFETY: `dispatch` was checked non-null above and per this method's `# Safety`
        // carries one retained reference owned by us, so it is a live `IDispatch` whose
        // `IUnknown` vtable can be queried.
        match unsafe { query_unknown_from_dispatch(dispatch) } {
            Ok(unknown) => {
                binding.native_unknown = unknown as usize;
            }
            Err(message) => {
                // SAFETY: QueryInterface failed, so no IUnknown reference was retained; we
                // still own the caller's single retained `IDispatch` reference (non-null,
                // checked above) and release it exactly once before erroring out.
                unsafe {
                    crate::release_dispatch(dispatch);
                }
                return Err(message);
            }
        }
        bind_host_dispatch_object_shared(&self.state, prog_id_name, binding)
    }

    pub fn host_dispatch_object_for_prog_id(
        &self,
        prog_id_name: &str,
    ) -> Result<Option<ObjectRef>, String> {
        host_dispatch_object_for_prog_id_shared(&self.state, prog_id_name)
    }

    pub fn describe_object(
        &self,
        object: ObjectRef,
    ) -> Result<Option<ComObjectDescriptor>, String> {
        let state = self.lock_state("describe_object")?;
        Ok(state
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .map(|binding| {
                binding.descriptor(
                    object,
                    resolve_typelib_identity_for_prog_id_name(&binding.prog_id_name)
                        .map(|identity| identity.cache_key),
                )
            }))
    }

    pub fn release_object_binding(
        &self,
        object: ObjectRef,
    ) -> Result<ReleasedWindowsComObject, String> {
        release_object_binding_shared(&self.state, object)
    }

    /// The class/type name VBA `TypeName` reports for a bound COM object (BUG 5).
    ///
    /// The ProgID's trailing segment is the documented VBA answer for the common
    /// activatable cases (`Scripting.Dictionary` → `"Dictionary"`,
    /// `Excel.Application` → `"Application"`) and is reliable, so it is the
    /// baseline. We then consult the live object's own short type name via
    /// `IDispatch::GetTypeInfo(0)` / `GetDocumentation` and PREFER it only when it
    /// matches that trailing segment — this keeps the answer authoritative when
    /// the typeinfo agrees, while avoiding the `IFoo`-dual-interface-name divergence
    /// (a typeinfo named `"IDictionary"` would otherwise regress `"Dictionary"`).
    /// `None` when there is no binding to name.
    pub fn object_type_name(&self, object: ObjectRef) -> Result<Option<String>, String> {
        let (prog_id_name, dispatch) = {
            let state = self.lock_state("object_type_name")?;
            match state.bindings.get(&ComObjectToken::new(object.raw())) {
                Some(binding) => (binding.prog_id_name.clone(), binding.native_dispatch),
                None => return Ok(None),
            }
        };
        let trailing = prog_id_name
            .rsplit('.')
            .next()
            .filter(|segment| !segment.is_empty())
            .map(str::to_string);
        if dispatch != 0 {
            // SAFETY: the bindings map owns one retained `IDispatch` reference for
            // this handle (W1-com-009) and bindings are released only from the VM
            // thread that is currently inside this call, so the pointer stays live
            // for the GetTypeInfo lookup.
            let typeinfo_name =
                unsafe { crate::live_object_typeinfo_name(dispatch as *mut RawIDispatch) };
            if let (Some(typeinfo_name), Some(trailing)) = (&typeinfo_name, &trailing)
                && typeinfo_name.eq_ignore_ascii_case(trailing)
            {
                return Ok(Some(typeinfo_name.clone()));
            }
        }
        Ok(trailing)
    }

    /// # Safety
    /// The caller must ensure the current thread is COM-initialized before any native
    /// connection-point transport teardown performed by this release path.
    pub unsafe fn release_object(
        &self,
        object: ObjectRef,
    ) -> Result<ReleasedWindowsComObject, String> {
        let released = release_object_binding_shared(&self.state, object)?;
        for transport in released.transports.iter().copied() {
            // SAFETY: release_object_binding_shared just removed these transports from the
            // subscriptions map, and unsubscribe always removes a transport from the map
            // before releasing it (W1-com-004), so each transport here is the sole owner of
            // one final Unadvise/Release; the COM apartment is supplied by this method's
            // `# Safety` contract.
            unsafe { release_subscription_transport(transport) }?;
        }
        Ok(released)
    }

    /// # Safety
    /// The caller must ensure the current thread is COM-initialized and the object/event pair
    /// refers to a live native COM binding owned by this bridge.
    pub unsafe fn subscribe_event(
        &self,
        object: ObjectRef,
        event: ComMemberToken,
    ) -> Result<
        (
            ComSubscriptionToken,
            crate::WindowsComSubscriptionTransport,
            usize,
        ),
        String,
    > {
        // SAFETY: forwarded caller contract — this method's `# Safety` requires a
        // COM-initialized thread and a live object/event binding owned by this bridge,
        // which is what the shared subscribe path requires.
        unsafe { subscribe_event_shared(&self.state, object, event) }
    }

    /// # Safety
    /// The caller must ensure the current thread is COM-initialized and the subscription token
    /// refers to a live native COM transport owned by this bridge.
    pub unsafe fn unsubscribe_event(
        &self,
        subscription: ComSubscriptionToken,
    ) -> Result<(), String> {
        // SAFETY: forwarded caller contract — this method's `# Safety` requires a
        // COM-initialized thread and a subscription token whose native transport is still
        // owned by this bridge, matching the shared unsubscribe path's requirements.
        unsafe { unsubscribe_event_shared(&self.state, subscription) }
    }

    pub fn poll_event_callback(&self) -> Result<Option<ComCallbackPayload>, String> {
        let mut state = self.lock_state("poll_event_callback")?;
        Ok(take_polled_callback_payload(&mut state))
    }

    pub fn event_callback_subscription(
        &self,
        callback: ComCallbackToken,
    ) -> Result<ComSubscriptionToken, String> {
        let state = self.lock_state("event_callback_subscription")?;
        callback_subscription_token(&state, callback)
    }

    pub fn event_callback_arity(&self, callback: ComCallbackToken) -> Result<usize, String> {
        let state = self.lock_state("event_callback_arity")?;
        callback_arity(&state, callback)
    }

    /// Retained value-model callback argument accessor.
    pub fn event_callback_variant(
        &self,
        callback: ComCallbackToken,
        index: usize,
    ) -> Result<Variant, String> {
        let state = self.lock_state("event_callback_variant")?;
        callback_arg(&state, callback, index).map(|value| value.variant().clone())
    }

    pub fn release_event_callback(&self, callback: ComCallbackToken) -> Result<(), String> {
        let mut state = self.lock_state("release_event_callback")?;
        release_callback(&mut state, callback)
    }

    /// # Safety
    /// `dispatch` must be null or carry one retained `IDispatch` reference owned by the caller.
    pub unsafe fn bind_native_dispatch_result(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id_hint: &str,
    ) -> Result<ObjectRef, String> {
        // SAFETY: forwarded caller contract — this method's `# Safety` requires `dispatch`
        // to be null or carry one retained `IDispatch` reference owned by the caller, the
        // exact precondition of the shared binding path (which takes ownership).
        unsafe { bind_native_dispatch_result_shared(&self.state, dispatch, prog_id_hint) }
    }

    pub fn mark_next_callback_pumped(&self) -> Result<Option<ComCallbackToken>, String> {
        let mut state = self.lock_state("mark_next_callback_pumped")?;
        Ok(state.mark_next_callback_pumped())
    }

    pub fn dispatch_invoke_variant(
        &self,
        request: &ComInvokeRequest,
        prefer_vtable: bool,
    ) -> Result<Option<Variant>, WindowsComBridgeDispatchError> {
        let positional_values = legacy_runtime_arg_values(request.args.as_slice());
        validate_named_arg_order(request.args.as_slice())
            .map_err(WindowsComBridgeDispatchError::Message)?;
        // The legacy i32-only `try_vtable_invoke` hook (formerly the in-process
        // `raw_oxvba_test_dispatch_vtable_invoke` behavioral oracle) is now
        // SUBSUMED by the real, spec-driven vtable path that fires at the
        // member-spec decision point inside `execute_bound_variant_with_shared_state`
        // (S3 — see `try_vtable_member_spec_invoke_with_shared_state`). That path
        // sees the full ComMemberSpec (slot / parameter_types / return_type /
        // callconv) and the original Variant args, so it can gate + marshal a true
        // vtable this-call with an IDispatch fallback for any ineligible shape.
        // This closure is retained only to satisfy `execute_bound_variant`'s
        // signature; it always declines, so the legacy i32 candidate never
        // shortcuts the spec-level path.
        let mut try_vtable_invoke =
            |_dispatch: *mut RawIDispatch,
             _binding: &ComBinding,
             _member: i32,
             _positional_values: &[i32]| Ok(None);
        let mut known_member_spec = |binding: &ComBinding, token: ComMemberToken| {
            self.known_member_spec_for_prog_id_name(&binding.prog_id_name, token)
        };
        let transport = crate::ComTransportCounters {
            vtable: &self.vtable_call_count,
            idispatch: &self.idispatch_call_count,
        };
        // SAFETY: any dispatch pointer the callee resolves comes from the bindings map,
        // which owns one retained `IDispatch` reference per native binding (W1-com-009)
        // established at bind time; bindings are only released from the VM thread that is
        // currently inside this call (cross-thread state access is limited to event sinks,
        // which only queue callback payloads), so the pointer stays live across the invoke
        // — including the spec-level vtable this-call, which the gate restricts to a
        // dual interface whose retained reference keeps it alive for the duration.
        let early = unsafe {
            execute_bound_variant_with_shared_state(
                &self.state,
                request,
                prefer_vtable,
                transport,
                &mut try_vtable_invoke,
                &mut known_member_spec,
            )
        }
        .map_err(WindowsComBridgeDispatchError::Message)?;
        if let Some(value) = early {
            return Ok(Some(value));
        }

        let binding = {
            let state = self
                .lock_state("dispatch_invoke")
                .map_err(WindowsComBridgeDispatchError::Message)?;
            state
                .bindings
                .get(&ComObjectToken::new(request.object.raw()))
                .cloned()
        };
        let Some(binding) = binding else {
            return Ok(None);
        };
        if positional_values.is_none() {
            return Err(WindowsComBridgeDispatchError::Message(
                "COM-E-VALUE-TRANSPORT-UNSUPPORTED: projection dispatch requires legacy runtime-token arguments".to_string(),
            ));
        }
        // A binding with a live native dispatch is fully handled by
        // execute_bound_variant_with_shared_state above (it returns Ok(None)
        // only for a missing binding or native_dispatch == 0), so the legacy
        // native fall-through that used to live here was unreachable — and it
        // released the bindings map's retained dispatch reference without a
        // matching AddRef, a loaded gun for any future refactor (W1-com-009).
        let _ = binding;
        Ok(None)
    }

    pub fn dispatch_invoke_call_result(
        &self,
        request: &ComInvokeRequest,
        prefer_vtable: bool,
    ) -> Result<Option<RuntimeCallResult>, WindowsComBridgeDispatchError> {
        validate_named_arg_order(request.args.as_slice())
            .map_err(WindowsComBridgeDispatchError::Message)?;
        let mut try_vtable_invoke =
            |_dispatch: *mut RawIDispatch,
             _binding: &ComBinding,
             _member: i32,
             _positional_values: &[i32]| Ok(None);
        let mut known_member_spec = |binding: &ComBinding, token: ComMemberToken| {
            self.known_member_spec_for_prog_id_name(&binding.prog_id_name, token)
        };
        let transport = crate::ComTransportCounters {
            vtable: &self.vtable_call_count,
            idispatch: &self.idispatch_call_count,
        };
        // SAFETY: the shared-state helper resolves any native dispatch pointer
        // from the bridge bindings map, which owns the retained reference for
        // the duration of this VM-thread call.
        let early = unsafe {
            execute_bound_runtime_call_result_with_shared_state(
                &self.state,
                request,
                prefer_vtable,
                transport,
                &mut try_vtable_invoke,
                &mut known_member_spec,
            )
        }
        .map_err(WindowsComBridgeDispatchError::Message)?;
        if early.is_some() {
            return Ok(early);
        }

        let binding = {
            let state = self
                .lock_state("dispatch_invoke_call_result")
                .map_err(WindowsComBridgeDispatchError::Message)?;
            state
                .bindings
                .get(&ComObjectToken::new(request.object.raw()))
                .cloned()
        };
        let Some(binding) = binding else {
            return Ok(None);
        };
        if binding.native_dispatch == 0 {
            return Ok(None);
        }
        Ok(None)
    }

    /// Attempt a vtable this-call for a late-bound-by-name member whose dispid is
    /// already resolved, recovering the FUNCDESC signature from the LIVE object's
    /// own `ITypeInfo`. This is an OPPORTUNISTIC acceleration layer: it returns
    /// `Ok(Some(value))` on a vtable success (and counts it), and `Ok(None)` in
    /// every other case — an ineligible member, a marshaling proxy, no live type
    /// info, OR a vtable call that failed (the recovered shape was not v1-covered)
    /// — so the caller always continues cleanly to the proven IDispatch invoke.
    /// It therefore never surfaces a hard error of its own (ZERO REGRESSION).
    #[cfg(target_arch = "x86_64")]
    fn try_live_vtable_invoke(
        &self,
        dispatch: *mut RawIDispatch,
        dispid: i32,
        request: &DynamicCallRequest,
        args: &[ComInvokeArg],
    ) -> Result<Option<Variant>, WindowsComBridgeDispatchError> {
        use crate::TypeLibMemberInvokeKind as K;
        // This live-recovered path delegates the actual eligibility gate (and the
        // out-of-process marshaling-proxy exclusion) to
        // `try_vtable_member_spec_invoke_with_shared_state`. We no longer vtable-call
        // the raw IDispatch pointer here: that helper QueryInterfaces the object for
        // the member's typelib-declared DUAL interface IID and slot-calls the QI'd
        // pointer, but ONLY after declining a marshaling proxy. S1 verified live
        // that an out-of-process proxy's dual-IID vtable slots are combase NDR
        // forwarders a typelib `oVft` slot cannot index (a slot call AVs the host),
        // so the proxy exclusion is kept and an out-of-process object falls back to
        // IDispatch; a direct in-process interface (DAO) is vtable-callable.
        //
        // The COM invoke kind this call intends, used to pick the right FUNCDESC
        // when a propget/propput pair shares a memid. PropertyPutRef (Set p = obj)
        // is deferred to IDispatch in v1.
        let preferred_kind = match request.call_kind_hint {
            Some(DynamicCallKind::PropertyLet) => K::PropertyPut,
            Some(DynamicCallKind::PropertySet) => return Ok(None),
            _ => K::Method,
        };
        // OUT-OF-PROCESS FAST-DECLINE (PERFORMANCE — the decisive guard). Recovering a
        // member's FUNCDESC from the LIVE object's ITypeInfo below means walking that
        // typeinfo, and for an out-of-process object the ITypeInfo is a MARSHALLED
        // proxy whose every read is a cross-apartment RPC — locating one member in
        // Excel's 471-member `_Application` took ~5 MINUTES per call. The proxy is then
        // declined for the slot-call anyway (S1: a marshaling proxy's dual-IID vtable
        // slots are combase NDR forwarders a typelib `oVft` cannot index), so this
        // whole live-recovery is pure waste for a proxy. Decline FIRST, on a cheap
        // `QueryInterface(IID_IProxyManager)` probe, so an out-of-process object goes
        // straight to the fast, correct IDispatch path. A direct in-process interface
        // (DAO) fails the proxy probe and proceeds to the in-proc vtable as before.
        // SAFETY: `dispatch` is the live, bindings-map-retained IDispatch for this call.
        if unsafe { crate::windows_invoke::dispatch_is_marshaling_proxy(dispatch.cast()) } {
            return Ok(None);
        }
        // SAFETY: `dispatch` is the live, bindings-map-retained IDispatch for this
        // call (the caller guarded `native_dispatch != 0` and the retained
        // reference keeps it alive for this lookup).
        let metadata =
            unsafe { crate::live_member_metadata_from_dispatch(dispatch, dispid, preferred_kind) };
        let Some(metadata) = metadata else {
            return Ok(None);
        };
        let spec = crate::map_member_metadata_to_spec(&metadata);
        // SAFETY: `dispatch` is the live, bindings-map-retained dual interface;
        // the spec-level attempt gates the slot/callconv/signature before any
        // vtable call and reuses the IDispatch path's resolve/bind closures.
        let outcome = unsafe {
            crate::try_vtable_member_spec_invoke_with_shared_state(
                dispatch.cast(),
                dispid,
                &spec,
                args,
                true,
                &self.state,
            )
        };
        match outcome {
            Ok(Some(value)) => {
                self.vtable_call_count.fetch_add(1, Ordering::Relaxed);
                Ok(Some(value))
            }
            // BEST-EFFORT semantics for this LIVE-RECOVERED path: the spec was
            // recovered heuristically from the object's own ITypeInfo at runtime
            // (not from authoritative bind-time metadata), and some real servers'
            // raw dual getters diverge from their IDispatch::Invoke behavior (DAO
            // `Field.Value` returns "Invalid operation" via the slot yet 7 via
            // Invoke). So a failure here is treated as "this member's vtable shape
            // is not v1-covered" and falls back to the proven IDispatch path,
            // preserving ZERO REGRESSION. (The authoritative fixture/bound path in
            // `try_vtable_member_spec_invoke_with_shared_state` still PROPAGATES a
            // genuine hr<0 to its caller; only this opportunistic acceleration
            // layer absorbs it.)
            Ok(None) | Err(_) => Ok(None),
        }
    }

    /// Non-x64 stub: the vtable marshaller is x64-only, so this always declines.
    #[cfg(not(target_arch = "x86_64"))]
    fn try_live_vtable_invoke(
        &self,
        _dispatch: *mut RawIDispatch,
        _dispid: i32,
        _request: &DynamicCallRequest,
        _args: &[ComInvokeArg],
    ) -> Result<Option<Variant>, WindowsComBridgeDispatchError> {
        Ok(None)
    }

    pub fn dispatch_invoke_dynamic_variant(
        &self,
        request: &DynamicCallRequest,
        prefer_vtable: bool,
    ) -> Result<Option<Variant>, WindowsComBridgeDispatchError> {
        // `prefer_vtable` is threaded through every re-dispatch below: the
        // Token/DefaultMember/Name→token selectors route back through
        // `dispatch_invoke_variant` (which honors the vtable fast path once the
        // member token resolves to a spec carrying a slot), and the late-bound
        // GetIDsOfNames tail genuinely has no static spec, so it stays IDispatch.
        match &request.member {
            DynamicMemberSelector::Token(value)
            | DynamicMemberSelector::TokenNamed { token: value, .. } => {
                return self.dispatch_invoke_variant(
                    &ComInvokeRequest {
                        object: request.object.clone(),
                        member: ComMemberToken::new(*value),
                        args: request.args.clone().into_iter().map(Into::into).collect(),
                        invoke_kind_hint: request.call_kind_hint.map(Into::into),
                    },
                    prefer_vtable,
                );
            }
            DynamicMemberSelector::DefaultMember => {
                return self.dispatch_invoke_variant(
                    &ComInvokeRequest {
                        object: request.object.clone(),
                        member: ComMemberToken::new(0),
                        args: request.args.clone().into_iter().map(Into::into).collect(),
                        invoke_kind_hint: request.call_kind_hint.map(Into::into),
                    },
                    prefer_vtable,
                );
            }
            DynamicMemberSelector::Name(_) => {}
        }

        let args = request
            .args
            .clone()
            .into_iter()
            .map(ComInvokeArg::from)
            .collect::<Vec<_>>();
        validate_named_arg_order(args.as_slice())
            .map_err(WindowsComBridgeDispatchError::Message)?;
        let binding = {
            let state = self
                .lock_state("dispatch_invoke_dynamic")
                .map_err(WindowsComBridgeDispatchError::Message)?;
            state
                .bindings
                .get(&ComObjectToken::new(request.object.raw()))
                .cloned()
        };
        let Some(binding) = binding else {
            return Ok(None);
        };
        let member_name = match &request.member {
            DynamicMemberSelector::Name(name) => name.as_str(),
            DynamicMemberSelector::Token(_)
            | DynamicMemberSelector::TokenNamed { .. }
            | DynamicMemberSelector::DefaultMember => unreachable!(),
        };
        if let Some((member_token, _spec)) = self
            .known_member_spec_for_prog_id_name_by_name(&binding.prog_id_name, member_name)
            .map_err(WindowsComBridgeDispatchError::Message)?
        {
            return self.dispatch_invoke_variant(
                &ComInvokeRequest {
                    object: request.object.clone(),
                    member: member_token,
                    args,
                    invoke_kind_hint: request.call_kind_hint.map(Into::into),
                },
                prefer_vtable,
            );
        }
        // A projection-only binding (`native_dispatch == 0`) has no live IDispatch
        // to name-resolve against; the typelib path above already missed, so there
        // is nothing left to dispatch on. Return a clean error rather than passing a
        // null dispatch into GetIDsOfNames (which does not null-check).
        if binding.native_dispatch == 0 {
            return Err(WindowsComBridgeDispatchError::Message(format!(
                "no live IDispatch for late-bound member '{member_name}' (projection-only binding)"
            )));
        }
        let dispatch = binding.native_dispatch as *mut RawIDispatch;
        let invoke_result = {
            // SAFETY: `binding.native_dispatch` is non-zero (guarded just above) on this
            // late-bound name path (the bindings map owns one retained `IDispatch`
            // reference per native binding, keeping the pointer live for this lookup).
            let dispid = unsafe { crate::get_dispid_by_name(dispatch, member_name) }
                .map_err(WindowsComBridgeDispatchError::Message)?;
            // Early-bound vtable fast path for late-bound-by-name members: ask the
            // LIVE object for its own ITypeInfo FUNCDESC (slot/params/retval/
            // callconv) for this dispid, build a ComMemberSpec, and (for a direct
            // in-process interface, not a marshaling proxy) attempt the vtable
            // this-call. This is what makes a registered dual interface's members
            // (e.g. ACE DAO's in-proc `Recordset.Close`) vtable-eligible even
            // though their `::<invoke-result>` bindings carry no
            // prog-id-resolvable typelib metadata. Any ineligible shape — or a
            // proxy, or a recovered shape that fails — returns Ok(None) and falls
            // through to the proven IDispatch invoke below.
            if prefer_vtable
                && let Some(value) =
                    self.try_live_vtable_invoke(dispatch, dispid, request, args.as_slice())?
            {
                return Ok(Some(value));
            }
            let named_arg_dispids = if args.iter().any(|arg| arg.name.is_some()) {
                // SAFETY: `dispatch` is the same live pointer GetIDsOfNames just succeeded
                // on; the bindings map's retained reference keeps it alive for this lookup.
                unsafe {
                    self.resolve_named_argument_dispids(dispatch, member_name, args.as_slice())
                }
                .map_err(WindowsComBridgeDispatchError::Message)?
            } else {
                Vec::new()
            };
            match request.call_kind_hint {
                Some(DynamicCallKind::PropertyLet) | Some(DynamicCallKind::PropertySet) => {
                    let value_arg_count = args.len().saturating_sub(1);
                    let mut put_named_arg_dispids =
                        if args[..value_arg_count].iter().any(|arg| arg.name.is_some()) {
                            // SAFETY: `dispatch` is the same live pointer GetIDsOfNames
                            // succeeded on above; the bindings map's retained reference
                            // keeps it alive for this lookup.
                            unsafe {
                                self.resolve_named_argument_dispids(
                                    dispatch,
                                    member_name,
                                    &args[..value_arg_count],
                                )
                            }
                            .map_err(WindowsComBridgeDispatchError::Message)?
                        } else {
                            Vec::new()
                        };
                    put_named_arg_dispids.push(crate::COM_DISPID_PROPERTYPUT);
                    let (flags, label) = match request.call_kind_hint {
                        Some(DynamicCallKind::PropertySet) => (
                            windows_sys::Win32::System::Com::DISPATCH_PROPERTYPUTREF,
                            "property-putref",
                        ),
                        _ => (
                            windows_sys::Win32::System::Com::DISPATCH_PROPERTYPUT,
                            "property-put",
                        ),
                    };
                    // SAFETY: `dispatch` is live across this invoke because the bindings
                    // map owns one retained `IDispatch` reference for the binding and
                    // bindings are only released from the VM thread currently inside this
                    // call; `dispid` and the named-arg DISPIDs were resolved from that same
                    // dispatch above.
                    let put_result = unsafe {
                        invoke_dispatch_variant_with_shared_state(
                            dispatch.cast(),
                            dispid,
                            flags,
                            args.as_slice(),
                            put_named_arg_dispids.as_slice(),
                            label,
                            &binding.prog_id_name,
                            &self.state,
                        )
                    };
                    if put_result.is_ok() {
                        self.idispatch_call_count.fetch_add(1, Ordering::Relaxed);
                    }
                    return put_result
                        .map(Some)
                        .map_err(WindowsComBridgeDispatchError::InvokeFailure);
                }
                _ => {}
            }
            // Late-bound name dispatch cannot know whether `member_name` is a method or a
            // property accessor, so start with the combined get-or-call flag used by OLE
            // Automation clients. Some servers, including Excel for parameterized properties
            // such as Range("A1"), reject the combined flag but accept a property-get invoke.
            // If the retry fails, keep the original combined failure as the diagnostic.
            // SAFETY: `dispatch` is live across this invoke because the bindings map owns
            // one retained `IDispatch` reference for the binding and bindings are only
            // released from the VM thread currently inside this call; `dispid` and the
            // named-arg DISPIDs were resolved from that same dispatch above.
            let combined = unsafe {
                invoke_dispatch_variant_with_shared_state(
                    dispatch.cast(),
                    dispid,
                    windows_sys::Win32::System::Com::DISPATCH_METHOD
                        | windows_sys::Win32::System::Com::DISPATCH_PROPERTYGET,
                    args.as_slice(),
                    named_arg_dispids.as_slice(),
                    "get-or-call",
                    &binding.prog_id_name,
                    &self.state,
                )
            };
            match combined {
                Ok(value) => Ok(value),
                // SAFETY: same invariant as the combined invoke directly above — the
                // bindings map's retained `IDispatch` reference keeps `dispatch` live for
                // this property-get retry on the same dispid/args.
                Err(combined_failure) => unsafe {
                    invoke_dispatch_variant_with_shared_state(
                        dispatch.cast(),
                        dispid,
                        windows_sys::Win32::System::Com::DISPATCH_PROPERTYGET,
                        args.as_slice(),
                        named_arg_dispids.as_slice(),
                        "property-get",
                        &binding.prog_id_name,
                        &self.state,
                    )
                    .map_err(|_| combined_failure)
                },
            }
        };
        if invoke_result.is_ok() {
            self.idispatch_call_count.fetch_add(1, Ordering::Relaxed);
        }
        invoke_result
            .map(Some)
            .map_err(WindowsComBridgeDispatchError::InvokeFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic_object::DynamicCallArg;

    /// Insert a native binding (carrying one retained reference on `native_ptr`)
    /// at a fixed handle, with a single member spec keyed by `member_token`, a
    /// pre-cached dispid (so the dispatch decision tree never calls
    /// `GetIDsOfNames` against the fixture), and the chosen prog id.
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn insert_native_member_binding(
        bridge: &WindowsComBridge,
        handle: i32,
        prog_id: &str,
        native_ptr: *mut core::ffi::c_void,
        member_token: crate::ComMemberToken,
        dispid: i32,
        spec: crate::ComMemberSpec,
    ) -> ObjectRef {
        let mut binding = ComBinding::new(prog_id.to_string(), native_ptr as usize);
        binding
            .member_specs
            .insert((member_token, spec.invoke_kind), spec);
        binding.member_dispids.insert(member_token, dispid);
        crate::insert_bound_object_binding_at_handle_shared(
            bridge.shared_state(),
            ObjectRef::from_compat_identity(handle),
            binding,
        )
        .expect("insert native member binding")
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn count_member_spec(
        name: &str,
        vtable_slot: Option<u16>,
        callconv_is_stdcall: bool,
    ) -> crate::ComMemberSpec {
        crate::ComMemberSpec {
            name: name.to_string(),
            requires_argument: false,
            invoke_kind: crate::TypeLibMemberInvokeKind::PropertyGet,
            parameter_names: Vec::new(),
            is_default_member: false,
            vtable_slot,
            parameter_types: Vec::new(),
            parameter_wire_types: Vec::new(),
            parameter_iids: Vec::new(),
            parameter_optional_defaults: Vec::new(),
            return_type: Some(crate::TypeLibParamType::Long),
            return_wire_type: Some(crate::TypeLibWireType::Automation(
                crate::TypeLibParamType::Long,
            )),
            callconv_is_stdcall,
            // A vtable-eligible spec (slot present) carries the dual fixture IID so
            // the dispatch path can QueryInterface the fixture object for it before
            // the slot call; a no-slot spec has no interface identity. The eligible
            // spec is a real custom INTERFACE dual with an AV-safety bound that
            // admits the fixture's custom slots (7..=14); the in-process fixture's
            // QI(dual IID) returns `this`, so the bound only needs to exceed those.
            interface_iid: vtable_slot.map(|_| crate::DUAL_FIXTURE_INTERFACE_IID),
            is_dual: vtable_slot.is_some(),
            source_typekind: vtable_slot.map(|_| crate::SourceTypeKind::Interface),
            vtable_slot_bound: vtable_slot.map(|_| 64),
        }
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn putref_object_member_spec(name: &str, vtable_slot: u16) -> crate::ComMemberSpec {
        crate::ComMemberSpec {
            name: name.to_string(),
            requires_argument: true,
            invoke_kind: crate::TypeLibMemberInvokeKind::PropertyPutRef,
            parameter_names: vec!["value".to_string()],
            is_default_member: false,
            vtable_slot: Some(vtable_slot),
            parameter_types: vec![crate::TypeLibParamType::Object],
            parameter_wire_types: vec![crate::TypeLibWireType::InterfacePointer {
                name: "IDispatch".to_string(),
            }],
            parameter_iids: vec![Some(crate::ComInterfaceIid {
                data1: 0x0002_0400,
                data2: 0,
                data3: 0,
                data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
            })],
            parameter_optional_defaults: vec![crate::OptionalParamDefault::Required],
            return_type: None,
            return_wire_type: None,
            callconv_is_stdcall: true,
            interface_iid: Some(crate::DUAL_FIXTURE_INTERFACE_IID),
            is_dual: true,
            source_typekind: Some(crate::SourceTypeKind::Interface),
            vtable_slot_bound: Some(64),
        }
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn putref_long_member_spec(name: &str, vtable_slot: Option<u16>) -> crate::ComMemberSpec {
        crate::ComMemberSpec {
            name: name.to_string(),
            requires_argument: true,
            invoke_kind: crate::TypeLibMemberInvokeKind::PropertyPutRef,
            parameter_names: vec!["value".to_string()],
            is_default_member: false,
            vtable_slot,
            parameter_types: vec![crate::TypeLibParamType::Long],
            parameter_wire_types: Vec::new(),
            parameter_iids: Vec::new(),
            parameter_optional_defaults: vec![crate::OptionalParamDefault::Required],
            return_type: None,
            return_wire_type: None,
            callconv_is_stdcall: true,
            interface_iid: vtable_slot.map(|_| crate::DUAL_FIXTURE_INTERFACE_IID),
            is_dual: vtable_slot.is_some(),
            source_typekind: vtable_slot.map(|_| crate::SourceTypeKind::Interface),
            vtable_slot_bound: vtable_slot.map(|_| 64),
        }
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn byref_long_method_spec(name: &str, vtable_slot: u16) -> crate::ComMemberSpec {
        crate::ComMemberSpec {
            name: name.to_string(),
            requires_argument: true,
            invoke_kind: crate::TypeLibMemberInvokeKind::Method,
            parameter_names: vec!["value".to_string()],
            is_default_member: false,
            vtable_slot: Some(vtable_slot),
            parameter_types: vec![crate::TypeLibParamType::ByRefLong],
            parameter_wire_types: vec![crate::TypeLibWireType::Automation(
                crate::TypeLibParamType::ByRefLong,
            )],
            parameter_iids: Vec::new(),
            parameter_optional_defaults: vec![crate::OptionalParamDefault::Required],
            return_type: None,
            return_wire_type: None,
            callconv_is_stdcall: true,
            interface_iid: Some(crate::DUAL_FIXTURE_INTERFACE_IID),
            is_dual: true,
            source_typekind: Some(crate::SourceTypeKind::Interface),
            vtable_slot_bound: Some(64),
        }
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn byref_record_method_spec(name: &str, vtable_slot: u16) -> crate::ComMemberSpec {
        crate::ComMemberSpec {
            name: name.to_string(),
            requires_argument: true,
            invoke_kind: crate::TypeLibMemberInvokeKind::Method,
            parameter_names: vec!["value".to_string()],
            is_default_member: false,
            vtable_slot: Some(vtable_slot),
            parameter_types: vec![crate::TypeLibParamType::ByRefRecord],
            parameter_wire_types: vec![crate::TypeLibWireType::ByRefRecord {
                name: "TestLib.Point".to_string(),
                record_info: None,
            }],
            parameter_iids: Vec::new(),
            parameter_optional_defaults: vec![crate::OptionalParamDefault::Required],
            return_type: None,
            return_wire_type: None,
            callconv_is_stdcall: true,
            interface_iid: Some(crate::DUAL_FIXTURE_INTERFACE_IID),
            is_dual: true,
            source_typekind: Some(crate::SourceTypeKind::Interface),
            vtable_slot_bound: Some(64),
        }
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct TestRecord {
        value: i32,
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    unsafe fn clone_test_record(
        record_info: *mut core::ffi::c_void,
        record_data: *const core::ffi::c_void,
    ) -> Result<(*mut core::ffi::c_void, *mut core::ffi::c_void), String> {
        if record_info.is_null() || record_data.is_null() {
            return Err("test record clone received a null record pointer".to_string());
        }
        // SAFETY: this clone callback is only registered (via `test_record_com_value`)
        // for records whose data is a boxed `TestRecord`, and `record_data` was checked
        // non-null above, so this reads a live `TestRecord`.
        let value = unsafe { *record_data.cast::<TestRecord>() };
        Ok((
            record_info,
            Box::into_raw(Box::new(value)).cast::<core::ffi::c_void>(),
        ))
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    unsafe fn destroy_test_record(
        _record_info: *mut core::ffi::c_void,
        record_data: *mut core::ffi::c_void,
    ) {
        if !record_data.is_null() {
            // SAFETY: paired with `clone_test_record`/`test_record_com_value`,
            // `record_data` is a non-null `Box<TestRecord>` raw pointer reclaimed
            // exactly once here.
            unsafe {
                drop(Box::from_raw(record_data.cast::<TestRecord>()));
            }
        }
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn test_record_com_value(value: i32) -> crate::ComValue {
        static RECORD_INFO_SENTINEL: u8 = 0;
        let data = Box::into_raw(Box::new(TestRecord { value })).cast::<core::ffi::c_void>();
        let info = (&RECORD_INFO_SENTINEL as *const u8)
            .cast_mut()
            .cast::<core::ffi::c_void>();
        // SAFETY: `data` is a freshly leaked `Box<TestRecord>`, `info` points at a
        // non-null static sentinel, and the clone/destroy callbacks match that
        // `TestRecord` payload — satisfying `ComRecord::from_raw_parts`.
        let record = unsafe {
            oxvba_runtime::ComRecord::from_raw_parts(
                info,
                data,
                clone_test_record,
                destroy_test_record,
            )
        }
        .expect("test record pointers are non-null");
        crate::ComValue::Record(record)
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn record_variant_value(value: &Variant) -> i32 {
        let record = value
            .as_com_record()
            .expect("writeback should contain a COM record");
        let ptr = record.record_data_ptr();
        assert!(!ptr.is_null(), "record data pointer should be non-null");
        // SAFETY: this helper only reads records created by `test_record_com_value`
        // and mutated by the fixture's `DualRecordFixture` slot.
        unsafe { (*ptr.cast::<TestRecord>()).value }
    }

    /// S3: under a `PreferVtable` policy, a member that passes the vtable gate (a
    /// real custom dual slot, CC_STDCALL, fully-typed v1 signature) dispatches
    /// through the COM vtable — proven by the real S2 dual-vtable fixture
    /// (`get_Count` at slot 7 returns 7) and the `vtable_call_count` increment.
    /// A member that fails the gate (no slot) falls back to `IDispatch::Invoke`,
    /// bumping `idispatch_call_count` instead.
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[test]
    fn prefer_vtable_routes_eligible_member_through_vtable_and_falls_back_otherwise() {
        // ── Supported shape → vtable transport ──
        let bridge = WindowsComBridge::new(false);
        // The real custom dual vtable fixture: slot 7 = get_Count(this, i32*) -> 7.
        let dual = crate::create_oxvba_dual_vtable_object();
        let member = ComMemberToken::new(7);
        let object = insert_native_member_binding(
            &bridge,
            7001,
            "OxVba.DualFixture",
            dual,
            member,
            7, // arbitrary cached dispid; the vtable path ignores it (uses the slot)
            count_member_spec("Count", Some(crate::DUAL_SLOT_GET_COUNT), true),
        );
        let request = ComInvokeRequest {
            object: object.clone(),
            member,
            args: Vec::new(),
            invoke_kind_hint: None,
        };
        let before_vtable = bridge.vtable_call_count();
        let before_idispatch = bridge.idispatch_call_count();
        let value = bridge
            .dispatch_invoke_variant(&request, true)
            .expect("vtable dispatch should not error")
            .expect("a value should be produced");
        assert_eq!(
            value.as_i32(),
            Some(7),
            "get_Count must round-trip 7 through the real vtable slot"
        );
        assert_eq!(
            bridge.vtable_call_count(),
            before_vtable + 1,
            "a supported member must increment the vtable transport counter"
        );
        assert_eq!(
            bridge.idispatch_call_count(),
            before_idispatch,
            "the vtable path must not also count an IDispatch invoke"
        );
        let _ = bridge.release_object_binding(object);

        // ── Unsupported shape (no vtable slot) → IDispatch fallback ──
        // The real `OxVba.TestDispatch` IDispatch object has a working Invoke but
        // its members carry no custom vtable slot, so the gate declines and the
        // call falls back to IDispatch::Invoke (Count -> 7), bumping idispatch.
        let fallback_bridge = WindowsComBridge::new(false);
        let dispatch = crate::create_oxvba_test_dispatch();
        let count = ComMemberToken::new(crate::TEST_DISPID_COUNT);
        let fallback_object = insert_native_member_binding(
            &fallback_bridge,
            7002,
            crate::OXVBA_TEST_DISPATCH_PROGID,
            dispatch.cast::<core::ffi::c_void>(),
            count,
            crate::TEST_DISPID_COUNT,
            count_member_spec("Count", None, true),
        );
        let fallback_request = ComInvokeRequest {
            object: fallback_object.clone(),
            member: count,
            args: Vec::new(),
            invoke_kind_hint: None,
        };
        let before_vtable = fallback_bridge.vtable_call_count();
        let before_idispatch = fallback_bridge.idispatch_call_count();
        let fallback_value = fallback_bridge
            .dispatch_invoke_variant(&fallback_request, true)
            .expect("fallback dispatch should not error")
            .expect("a value should be produced");
        assert_eq!(
            fallback_value.as_i32(),
            Some(7),
            "the IDispatch fallback must still return Count = 7"
        );
        assert_eq!(
            fallback_bridge.vtable_call_count(),
            before_vtable,
            "a no-slot member must NOT take the vtable path"
        );
        assert_eq!(
            fallback_bridge.idispatch_call_count(),
            before_idispatch + 1,
            "an ineligible member must fall back to IDispatch and count it"
        );
        let _ = fallback_bridge.release_object_binding(fallback_object);
    }

    /// PropertyPutRef is no longer globally deferred. Object/interface putref
    /// uses explicit interface-pointer wire metadata, while scalar putref uses
    /// the same typed automation table as property-put.
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[test]
    fn prefer_vtable_routes_object_and_scalar_putref() {
        let bridge = WindowsComBridge::new(false);
        let dual = crate::create_oxvba_dual_vtable_object();
        let putref_slot = crate::DUAL_SLOT_PUTREF_OBJECT_VALUE;
        let member = ComMemberToken::new(putref_slot as i32);
        let object = insert_native_member_binding(
            &bridge,
            7010,
            "OxVba.DualFixture",
            dual,
            member,
            putref_slot as i32,
            putref_object_member_spec("ObjectValue", putref_slot),
        );
        let request = ComInvokeRequest {
            object: object.clone(),
            member,
            args: vec![ComInvokeArg::positional_value(crate::ComValue::Object(
                object.clone(),
            ))],
            invoke_kind_hint: Some(crate::ComInvokeKind::PropertyPutRef),
        };
        let before_vtable = bridge.vtable_call_count();
        let before_idispatch = bridge.idispatch_call_count();
        let value = bridge
            .dispatch_invoke_variant(&request, true)
            .expect("object putref vtable dispatch should not error")
            .expect("putref should produce an Empty value");
        assert_eq!(
            value.vtype(),
            oxvba_runtime::VarType::Empty,
            "HRESULT-only object putref returns Empty through vtable"
        );
        assert_eq!(
            bridge.vtable_call_count(),
            before_vtable + 1,
            "covered object putref must increment the vtable transport counter"
        );
        assert_eq!(
            bridge.idispatch_call_count(),
            before_idispatch,
            "covered object putref must not also dispatch through IDispatch"
        );
        let _ = bridge.release_object_binding(object);

        let scalar_bridge = WindowsComBridge::new(false);
        let scalar_dual = crate::create_oxvba_dual_vtable_object();
        let scalar_putref = ComMemberToken::new(crate::TEST_DISPID_SET_VALUE_REF);
        let scalar_object = insert_native_member_binding(
            &scalar_bridge,
            7011,
            "OxVba.DualFixture",
            scalar_dual,
            scalar_putref,
            crate::TEST_DISPID_SET_VALUE_REF,
            putref_long_member_spec("SetValueRef", Some(crate::DUAL_SLOT_PUTREF_LONG_VALUE)),
        );
        let scalar_request = ComInvokeRequest {
            object: scalar_object.clone(),
            member: scalar_putref,
            args: vec![ComInvokeArg::positional_value(crate::ComValue::I32(7))],
            invoke_kind_hint: Some(crate::ComInvokeKind::PropertyPutRef),
        };
        let before_vtable = scalar_bridge.vtable_call_count();
        let before_idispatch = scalar_bridge.idispatch_call_count();
        let scalar_value = scalar_bridge
            .dispatch_invoke_variant(&scalar_request, true)
            .expect("scalar putref vtable dispatch should not error")
            .expect("scalar putref should produce an Empty value");
        assert_eq!(
            scalar_value.vtype(),
            oxvba_runtime::VarType::Empty,
            "HRESULT-only scalar putref returns Empty through vtable"
        );
        assert_eq!(
            scalar_bridge.vtable_call_count(),
            before_vtable + 1,
            "covered scalar putref must increment the vtable transport counter"
        );
        assert_eq!(
            scalar_bridge.idispatch_call_count(),
            before_idispatch,
            "covered scalar putref must not also dispatch through IDispatch"
        );
        let _ = scalar_bridge.release_object_binding(scalar_object);
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[test]
    fn dispatch_call_result_returns_vtable_byref_writebacks() {
        let bridge = WindowsComBridge::new(false);
        let dual = crate::create_oxvba_dual_vtable_object();
        let member =
            ComMemberToken::new(crate::windows_test_dispatch::DUAL_SLOT_MUTATE_BYREF_LONG as i32);
        let object = insert_native_member_binding(
            &bridge,
            7012,
            "OxVba.DualFixture",
            dual,
            member,
            crate::windows_test_dispatch::DUAL_SLOT_MUTATE_BYREF_LONG as i32,
            byref_long_method_spec(
                "MutateByRefLong",
                crate::windows_test_dispatch::DUAL_SLOT_MUTATE_BYREF_LONG,
            ),
        );
        let slot =
            oxvba_runtime::RuntimeByRefSlot::new(0, Some(oxvba_runtime::RuntimeValueType::Long));
        let request = ComInvokeRequest {
            object: object.clone(),
            member,
            args: vec![ComInvokeArg::positional_by_ref(
                crate::ComValue::I32(7),
                slot,
            )],
            invoke_kind_hint: None,
        };
        let before_vtable = bridge.vtable_call_count();
        let before_idispatch = bridge.idispatch_call_count();
        let result = bridge
            .dispatch_invoke_call_result(&request, true)
            .expect("ByRef vtable call-result dispatch should not error")
            .expect("a call result should be produced");
        assert_eq!(
            result.value.as_ref().map(Variant::vtype),
            Some(oxvba_runtime::VarType::Empty),
            "HRESULT-only ByRef vtable methods produce an Empty result value"
        );
        assert_eq!(result.writebacks.len(), 1);
        assert_eq!(result.writebacks[0].slot, slot);
        assert_eq!(result.writebacks[0].value.as_i32(), Some(1_007));
        assert_eq!(
            bridge.vtable_call_count(),
            before_vtable + 1,
            "ByRef call-result dispatch must use the vtable transport"
        );
        assert_eq!(
            bridge.idispatch_call_count(),
            before_idispatch,
            "ByRef vtable dispatch must not also invoke IDispatch"
        );
        let _ = bridge.release_object_binding(object);
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[test]
    fn dispatch_call_result_returns_vtable_byref_record_writeback() {
        let bridge = WindowsComBridge::new(false);
        let dual = crate::create_oxvba_dual_vtable_object();
        let member =
            ComMemberToken::new(crate::windows_test_dispatch::DUAL_SLOT_MUTATE_BYREF_RECORD as i32);
        let object = insert_native_member_binding(
            &bridge,
            7013,
            "OxVba.DualFixture",
            dual,
            member,
            crate::windows_test_dispatch::DUAL_SLOT_MUTATE_BYREF_RECORD as i32,
            byref_record_method_spec(
                "MutateByRefRecord",
                crate::windows_test_dispatch::DUAL_SLOT_MUTATE_BYREF_RECORD,
            ),
        );
        let slot =
            oxvba_runtime::RuntimeByRefSlot::new(0, Some(oxvba_runtime::RuntimeValueType::Record));
        let request = ComInvokeRequest {
            object: object.clone(),
            member,
            args: vec![ComInvokeArg::positional_by_ref(
                test_record_com_value(crate::windows_test_dispatch::DUAL_RECORD_VALUE),
                slot,
            )],
            invoke_kind_hint: None,
        };
        let before_vtable = bridge.vtable_call_count();
        let before_idispatch = bridge.idispatch_call_count();
        let result = bridge
            .dispatch_invoke_call_result(&request, true)
            .expect("ByRef record vtable dispatch should not error")
            .expect("a call result should be produced");
        assert_eq!(
            result.value.as_ref().map(Variant::vtype),
            Some(oxvba_runtime::VarType::Empty),
            "HRESULT-only ByRef record vtable methods produce an Empty result value"
        );
        assert_eq!(result.writebacks.len(), 1);
        assert_eq!(result.writebacks[0].slot, slot);
        assert_eq!(
            record_variant_value(&result.writebacks[0].value),
            crate::windows_test_dispatch::DUAL_RECORD_MUTATED_VALUE,
            "ByRef record writeback must carry the fixture-mutated record data"
        );
        assert_eq!(
            bridge.vtable_call_count(),
            before_vtable + 1,
            "ByRef record call-result dispatch must use the vtable transport"
        );
        assert_eq!(
            bridge.idispatch_call_count(),
            before_idispatch,
            "ByRef record vtable dispatch must not also invoke IDispatch"
        );
        let _ = bridge.release_object_binding(object);
    }

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[test]
    fn registered_testeventserver_typed_record_safearray_uses_vtable_oracle() {
        let Ok(dispatch) = crate::activate_dispatch_by_prog_id("OxVba.TestEventServer") else {
            return;
        };
        // SAFETY: `dispatch` is the live IDispatch reference returned by
        // `activate_dispatch_by_prog_id` above; resolving a member name on it is valid.
        let dispid = match unsafe { crate::get_dispid_by_name(dispatch, "SumTypedRecordArray") } {
            Ok(dispid) => dispid,
            Err(_) => {
                // SAFETY: `dispatch` is the live reference returned by activation.
                unsafe { crate::release_dispatch(dispatch) };
                return;
            }
        };
        // SAFETY: `dispatch` is the live activated IDispatch and `dispid` was just
        // resolved on it, so recovering that member's live typelib metadata is valid.
        let Some(metadata) = (unsafe {
            crate::live_member_metadata_from_dispatch(
                dispatch,
                dispid,
                crate::TypeLibMemberInvokeKind::Method,
            )
        }) else {
            // SAFETY: `dispatch` is the live reference returned by activation.
            unsafe { crate::release_dispatch(dispatch) };
            return;
        };
        assert!(
            metadata
                .parameter_wire_types
                .iter()
                .any(|wire_type| matches!(
                    wire_type,
                    crate::TypeLibWireType::SafeArray {
                        element_vt: 36,
                        record_info: Some(_),
                    }
                )),
            "live TestEventServer metadata should expose descriptor-backed SAFEARRAY(VT_RECORD)"
        );
        let spec = crate::map_member_metadata_to_spec(&metadata);
        let bridge = WindowsComBridge::new(false);
        let empty = oxvba_runtime::safe_array::SafeArray::from_variants(Vec::new());
        let args = vec![crate::ComInvokeArg::positional_value(
            crate::ComValue::ArrayIntent(empty),
        )];
        // SAFETY: `dispatch` is the live activated object, `dispid`/`spec` describe its
        // `SumTypedRecordArray` member, and `bridge.shared_state()` is this bridge's live
        // client state — the contract for a vtable member-spec invoke on this dispatch.
        let value = unsafe {
            crate::try_vtable_member_spec_invoke_with_shared_state(
                dispatch.cast(),
                dispid,
                &spec,
                &args,
                true,
                bridge.shared_state(),
            )
        }
        .expect("registered TestEventServer vtable record-array call should not error")
        .expect("registered TestEventServer typed record-array member should be vtable-admitted");
        assert_eq!(
            value.as_i32(),
            Some(0),
            "empty typed record array should reach SumTypedRecordArray through vtable"
        );
        // SAFETY: balances the activation reference.
        unsafe { crate::release_dispatch(dispatch) };
    }

    /// S5a HOST-AV SAFETY: a member that IS gate-eligible (a custom slot,
    /// CC_STDCALL, full v1 signature, an interface IID) but whose IID the live
    /// object does NOT expose must `QueryInterface`-fail and fall back to
    /// `IDispatch::Invoke` — it must NEVER vtable-call an unverified pointer. The
    /// `OxVba.TestDispatch` object answers IUnknown/IDispatch (+ its event IIDs)
    /// only, so a spec carrying a bogus IID drives the QI-fail fallback: the call
    /// still returns the correct value (its IDispatch::Invoke for `Count` is 7) with
    /// the IDispatch transport counted and the vtable transport untouched, proving
    /// no slot was ever called on an interface the object does not expose.
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    #[test]
    fn prefer_vtable_falls_back_when_interface_iid_is_not_exposed() {
        let bridge = WindowsComBridge::new(false);
        let dispatch = crate::create_oxvba_test_dispatch();
        let count = ComMemberToken::new(crate::TEST_DISPID_COUNT);
        // Gate-eligible spec EXCEPT its interface IID is one this object's
        // QueryInterface rejects — so the QI fails before any slot call. The slot is
        // present (gate-eligible) but never reached: the IDispatch path serves Count.
        let bogus_iid = crate::ComInterfaceIid {
            data1: 0xDEAD_BEEF,
            data2: 0x0000,
            data3: 0x0000,
            data4: [0; 8],
        };
        let mut spec = count_member_spec("Count", Some(crate::DUAL_SLOT_GET_COUNT), true);
        spec.interface_iid = Some(bogus_iid);
        let object = insert_native_member_binding(
            &bridge,
            7003,
            crate::OXVBA_TEST_DISPATCH_PROGID,
            dispatch.cast::<core::ffi::c_void>(),
            count,
            crate::TEST_DISPID_COUNT,
            spec,
        );
        let request = ComInvokeRequest {
            object: object.clone(),
            member: count,
            args: Vec::new(),
            invoke_kind_hint: None,
        };
        let before_vtable = bridge.vtable_call_count();
        let before_idispatch = bridge.idispatch_call_count();
        let value = bridge
            .dispatch_invoke_variant(&request, true)
            .expect("QI-fail fallback dispatch should not error")
            .expect("a value should be produced");
        assert_eq!(
            value.as_i32(),
            Some(7),
            "the QI-fail IDispatch fallback must still return Count = 7"
        );
        assert_eq!(
            bridge.vtable_call_count(),
            before_vtable,
            "a member whose IID is not QI-able must NOT take the vtable path"
        );
        assert_eq!(
            bridge.idispatch_call_count(),
            before_idispatch + 1,
            "the unexposed-IID member must fall back to IDispatch and count it"
        );
        let _ = bridge.release_object_binding(object);
    }

    /// A projection-only binding (`native_dispatch == 0`) has no live IDispatch. A
    /// late-bound member name that misses the typelib path must return a clean error
    /// rather than passing a null dispatch into GetIDsOfNames. Uses the fixture
    /// `OxVba.TestEventServer` typelib (enabled by the test build) and a member name
    /// it does not export, so the dynamic-name fallback is reached.
    #[test]
    fn dynamic_dispatch_on_projection_only_binding_errors_cleanly() {
        let bridge = WindowsComBridge::new(false);
        let object = bridge
            .bind_projection_object(
                ObjectRef::from_compat_identity(4242),
                "OxVba.TestEventServer",
            )
            .expect("bind projection object for fixture prog id");
        let request = DynamicCallRequest {
            object,
            member: DynamicMemberSelector::Name("NoSuchMember".to_string()),
            args: Vec::<DynamicCallArg>::new(),
            call_kind_hint: Some(DynamicCallKind::Method),
        };
        let result = bridge.dispatch_invoke_dynamic_variant(&request, false);
        match result {
            Err(WindowsComBridgeDispatchError::Message(message)) => {
                assert!(
                    message.contains("no live IDispatch") && message.contains("NoSuchMember"),
                    "expected a clean projection-only error, got: {message}"
                );
            }
            other => panic!("expected a clean Message error, got: {other:?}"),
        }
    }
}
