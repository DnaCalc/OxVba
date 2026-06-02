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
    callback_subscription_token, execute_bound_variant_with_shared_state,
    host_dispatch_object_for_prog_id_shared, insert_bound_object_binding_at_handle_shared,
    invoke_bound_dispatch_legacy_i32_result, invoke_dispatch_variant_with_shared_state,
    legacy_runtime_arg_values, member_spec_from_typelib_metadata,
    member_token_and_spec_from_typelib_metadata_name, query_unknown_from_dispatch,
    queue_projection_event_callbacks_shared, release_callback, release_object_binding_shared,
    release_subscription_transport, resolve_bound_native_dispatch_shared,
    resolve_known_typelib_identity, resolve_named_argument_dispids,
    resolve_typelib_identity_for_prog_id_name, subscribe_event_shared,
    take_polled_callback_payload, unsubscribe_event_shared, validate_named_arg_order,
};
use oxvba_runtime::{ObjectRef, Variant};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Clone)]
pub struct WindowsComBridge {
    state: Arc<Mutex<WindowsComClientState>>,
    typelib_state: Arc<Mutex<TypeLibMetadataCacheState>>,
    force_registered_test_dispatch: bool,
}

#[derive(Debug, Clone)]
pub enum WindowsComBridgeDispatchError {
    Message(String),
    InvokeFailure(ComInvokeFailure),
}

impl WindowsComBridge {
    pub fn new(force_registered_test_dispatch: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(WindowsComClientState::default())),
            typelib_state: Arc::new(Mutex::new(TypeLibMetadataCacheState::default())),
            force_registered_test_dispatch,
        }
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
        match unsafe { query_unknown_from_dispatch(dispatch) } {
            Ok(unknown) => {
                binding.native_unknown = unknown as usize;
            }
            Err(message) => {
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

    /// # Safety
    /// The caller must ensure the current thread is COM-initialized before any native
    /// connection-point transport teardown performed by this release path.
    pub unsafe fn release_object(
        &self,
        object: ObjectRef,
    ) -> Result<ReleasedWindowsComObject, String> {
        let released = release_object_binding_shared(&self.state, object)?;
        for transport in released.transports.iter().copied() {
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
        unsafe { subscribe_event_shared(&self.state, object, event) }
    }

    /// # Safety
    /// The caller must ensure the current thread is COM-initialized and the subscription token
    /// refers to a live native COM transport owned by this bridge.
    pub unsafe fn unsubscribe_event(
        &self,
        subscription: ComSubscriptionToken,
    ) -> Result<(), String> {
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
        let mut try_vtable_invoke =
            |dispatch: *mut RawIDispatch,
             binding: &ComBinding,
             member: i32,
             positional_values: &[i32]| {
                if !prefer_vtable {
                    return Ok(None);
                }
                #[cfg(any(test, feature = "fixture-typelibs"))]
                {
                    if !binding
                        .prog_id_name
                        .eq_ignore_ascii_case(crate::OXVBA_TEST_DISPATCH_PROGID)
                    {
                        return Ok(None);
                    }
                    if member == crate::TEST_DISPID_ECHO_VARIANT {
                        return Ok(None);
                    }
                    unsafe {
                        crate::raw_oxvba_test_dispatch_vtable_invoke(
                            dispatch,
                            member,
                            positional_values,
                        )
                    }
                }
                #[cfg(not(any(test, feature = "fixture-typelibs")))]
                {
                    let _ = (dispatch, binding, member, positional_values);
                    Ok(None)
                }
            };
        let mut known_member_spec = |binding: &ComBinding, token: ComMemberToken| {
            self.known_member_spec_for_prog_id_name(&binding.prog_id_name, token)
        };
        let early = unsafe {
            execute_bound_variant_with_shared_state(
                &self.state,
                request,
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
        if binding.native_dispatch == 0 {
            return Ok(None);
        }
        let dispatch = binding.native_dispatch as *mut RawIDispatch;
        let invoke_result = unsafe {
            invoke_bound_dispatch_legacy_i32_result(
                dispatch,
                request.member,
                request.args.as_slice(),
                &mut |member_token| {
                    self.known_member_spec_for_prog_id_name(&binding.prog_id_name, member_token)
                },
                &mut |member_name, invoke_args| {
                    self.resolve_named_argument_dispids(dispatch, member_name, invoke_args)
                },
                &mut |handle| {
                    self.resolve_native_dispatch_for_object(handle)
                        .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
            )
        };
        unsafe {
            crate::release_dispatch(dispatch);
        }
        let value = invoke_result.map_err(WindowsComBridgeDispatchError::InvokeFailure)?;
        queue_projection_event_callbacks_shared(
            &self.state,
            request.object.clone(),
            &binding,
            request.member,
            request.args.as_slice(),
        )
        .map_err(WindowsComBridgeDispatchError::Message)?;
        Ok(Some(Variant::from_i32(value)))
    }

    pub fn dispatch_invoke_dynamic_variant(
        &self,
        request: &DynamicCallRequest,
        prefer_vtable: bool,
    ) -> Result<Option<Variant>, WindowsComBridgeDispatchError> {
        let _ = prefer_vtable;
        match &request.member {
            DynamicMemberSelector::Token(value) => {
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
            DynamicMemberSelector::Token(_) | DynamicMemberSelector::DefaultMember => {
                unreachable!()
            }
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
        let dispatch = binding.native_dispatch as *mut RawIDispatch;
        let invoke_result = {
            let dispid = unsafe { crate::get_dispid_by_name(dispatch, member_name) }
                .map_err(WindowsComBridgeDispatchError::Message)?;
            let named_arg_dispids = if args.iter().any(|arg| arg.name.is_some()) {
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
                    return unsafe {
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
                    }
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
                    .or_else(|_| Err(combined_failure))
                },
            }
        };
        invoke_result
            .map(Some)
            .map_err(WindowsComBridgeDispatchError::InvokeFailure)
    }
}
