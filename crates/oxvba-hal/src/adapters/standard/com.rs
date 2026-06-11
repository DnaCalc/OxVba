#![cfg_attr(not(target_os = "windows"), allow(dead_code, unused_variables))]

#[cfg(target_os = "windows")]
use crate::model::ComInvocationStrategy;
use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::{
        ComHal, TypeLibCacheScope, TypeLibMetadataBlob, TypeLibResolveRequest,
        TypeLibResolvedIdentity,
    },
};
#[cfg(target_os = "windows")]
use oxvba_com::RawIDispatch;
#[cfg(target_os = "windows")]
use oxvba_com::WindowsComBridgeDispatchError;
use oxvba_com::{
    ComCallbackPayload, ComCallbackToken, ComInvokeRequest, ComMemberToken, ComObjectDescriptor,
    ComObjectTransportKind, ComSubscriptionToken, DynamicCallRequest, DynamicMemberSelector,
    known_typelib_identity_for_prog_id_name,
    legacy_runtime_arg_values as com_legacy_runtime_arg_values,
};
use oxvba_runtime::{ObjectRef, Variant, variant_to_vba_string};

use super::StandardHostServices;

fn is_dispatch_fixture_prog_id_name(prog_id_name: &str) -> bool {
    known_typelib_identity_for_prog_id_name(prog_id_name).is_some_and(|identity| {
        identity
            .importlib
            .to_ascii_lowercase()
            .starts_with("oxvba_testdispatch")
    })
}

fn allocate_projection_object_ref(
    host: &StandardHostServices,
    prog_id_name: &str,
) -> HalResult<ObjectRef> {
    let capability = CapabilityId::ComActivationDispatch;
    let mut state = host.projection_lock(capability, "create_object")?;
    if let Some(handle) = state.handles_by_prog_id.get(prog_id_name).copied() {
        return Ok(ObjectRef::from_compat_identity(handle));
    }
    state.next_handle = state.next_handle.saturating_add(1).max(1);
    let object = ObjectRef::from_compat_identity(state.next_handle);
    state
        .handles_by_prog_id
        .insert(prog_id_name.to_string(), object.raw());
    state
        .prog_ids_by_handle
        .insert(object.raw(), prog_id_name.to_string());
    Ok(object)
}

fn release_projection_object_ref(
    host: &StandardHostServices,
    object: &ObjectRef,
) -> HalResult<bool> {
    let capability = CapabilityId::ComActivationDispatch;
    let mut state = host.projection_lock(capability, "release_object")?;
    if let Some(prog_id_name) = state.prog_ids_by_handle.remove(&object.raw()) {
        state.handles_by_prog_id.remove(&prog_id_name);
        return Ok(true);
    }
    Ok(false)
}

fn projection_prog_id_name(
    host: &StandardHostServices,
    object: &ObjectRef,
) -> HalResult<Option<String>> {
    let capability = CapabilityId::ComActivationDispatch;
    let state = host.projection_lock(capability, "describe_object")?;
    Ok(state.prog_ids_by_handle.get(&object.raw()).cloned())
}

#[cfg(target_os = "windows")]
fn try_bind_projection_object_metadata(
    host: &StandardHostServices,
    object: ObjectRef,
    prog_id_name: &str,
) -> HalResult<()> {
    host.com_bridge
        .bind_projection_object(object, prog_id_name)
        .map(|_| ())
        .map_err(|message| {
            HalError::adapter_fault(
                host.profile(),
                CapabilityId::ComActivationDispatch,
                "create_object",
                message,
            )
        })
}

impl ComHal for StandardHostServices {
    fn create_object_variant(&self, prog_id: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "create_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "create_object"));
        }
        let prog_id_value = variant_to_vba_string(&prog_id).map_err(|detail| {
            HalError::adapter_fault(self.profile, capability, "create_object", detail)
        })?;
        let prog_id_text = prog_id_value.as_str();
        let prog_id_name = prog_id_text.trim();
        if prog_id_name.is_empty() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "create_object",
                "CreateObject requires a non-empty ProgID string",
            ));
        }
        if let Some(projection) = &self.portable_objects
            && let Some(_dispatch) = projection.create_object(prog_id_name)
        {
            // Portable registry matched: return a synthetic object handle.
            // The handle base mirrors the existing fallback convention.
            let object = allocate_projection_object_ref(self, prog_id_name)?;
            #[cfg(target_os = "windows")]
            try_bind_projection_object_metadata(self, object.clone(), prog_id_name)?;
            return Ok(Variant::from_object_ref(object));
        }
        #[cfg(target_os = "windows")]
        if self.native_com_enabled() {
            if let Some(object) = self
                .com_bridge
                .host_dispatch_object_for_prog_id(prog_id_name)
                .map_err(|message| self.com_createobject_adapter_fault(message))?
            {
                return Ok(Variant::from_object_ref(object));
            }
            match self.activate_variant_object_for_prog_id_name(prog_id_name) {
                Ok(value) => {
                    return Ok(value);
                }
                Err(_err) if is_dispatch_fixture_prog_id_name(prog_id_name) => {
                    let object = allocate_projection_object_ref(self, prog_id_name)?;
                    try_bind_projection_object_metadata(self, object.clone(), prog_id_name)?;
                    return Ok(Variant::from_object_ref(object));
                }
                Err(err) => return Err(err),
            }
        }
        let object = allocate_projection_object_ref(self, prog_id_name)?;
        #[cfg(target_os = "windows")]
        try_bind_projection_object_metadata(self, object.clone(), prog_id_name)?;
        Ok(Variant::from_object_ref(object))
    }

    unsafe fn bind_native_dispatch_object_variant(
        &self,
        prog_id: &str,
        dispatch: *mut core::ffi::c_void,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "bind_native_dispatch_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "bind_native_dispatch_object"));
        }
        #[cfg(target_os = "windows")]
        {
            if !self.native_com_enabled() {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "bind_native_dispatch_object",
                    "native COM is disabled for this host profile",
                ));
            }
            self.ensure_thread_com_apartment("bind_native_dispatch_object")?;
            // SAFETY: this unsafe fn's trait contract (ComHal::
            // bind_native_dispatch_object_variant) obliges our caller to pass null or
            // an `IDispatch` pointer carrying one retained reference it owns; the
            // bridge takes ownership of that reference on success or failure, and
            // `ensure_thread_com_apartment` above initialized COM on this thread.
            let object = unsafe {
                self.com_bridge
                    .bind_host_dispatch_object(prog_id, dispatch.cast::<RawIDispatch>())
            }
            .map_err(|message| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "bind_native_dispatch_object",
                    message,
                )
            })?;
            Ok(Variant::from_object_ref(object))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (prog_id, dispatch);
            Err(self.unsupported(capability, "bind_native_dispatch_object"))
        }
    }

    fn release_object_variant(&self, object: ObjectRef) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "release_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "release_object"));
        }
        let removed_projection = release_projection_object_ref(self, &object)?;
        let object_raw = object.raw();
        if !self.native_com_enabled() {
            return Ok(Variant::from_i32(
                if removed_projection || object_raw != 0 {
                    1
                } else {
                    0
                },
            ));
        }
        self.ensure_thread_com_apartment("release_object")?;
        #[cfg(target_os = "windows")]
        {
            // SAFETY: `ensure_thread_com_apartment` succeeded above, satisfying
            // release_object's contract that this thread is COM-initialized for the
            // native connection-point transport teardown the release path performs;
            // the bindings map owns the one retained dispatch reference being dropped.
            let released =
                unsafe { self.com_bridge.release_object(object.clone()) }.map_err(|message| {
                    HalError::adapter_fault(self.profile, capability, "release_object", message)
                })?;
            if super::com_event_trace_enabled() {
                eprintln!(
                    "[oxvba-hal][com-event] release-object object={} removed_callbacks={}",
                    object_raw,
                    released.stale_callbacks.len()
                );
            }
            Ok(Variant::from_i32(1))
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn describe_object(&self, object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "describe_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "describe_object"));
        }
        #[cfg(target_os = "windows")]
        {
            let descriptor =
                self.com_bridge
                    .describe_object(object.clone())
                    .map_err(|message| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "describe_object",
                            message,
                        )
                    })?;
            if descriptor.is_some() || self.native_com_enabled() {
                return Ok(descriptor);
            }
        }
        let descriptor = if object.raw() == 0 {
            None
        } else if let Some(prog_id_name) = projection_prog_id_name(self, &object)? {
            Some(ComObjectDescriptor {
                object: object.clone(),
                prog_id_name,
                transport: ComObjectTransportKind::Projection,
                supports_events: false,
                known_member_tokens: Vec::new(),
                known_event_tokens: Vec::new(),
                default_member_token: None,
                default_member_name: None,
                typelib_cache_key: None,
            })
        } else {
            Some(ComObjectDescriptor {
                object: object.clone(),
                prog_id_name: projection_prog_id_name(self, &object)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| format!("projection:{}", object.raw())),
                transport: ComObjectTransportKind::Projection,
                supports_events: false,
                known_member_tokens: Vec::new(),
                known_event_tokens: Vec::new(),
                default_member_token: None,
                default_member_name: None,
                typelib_cache_key: None,
            })
        };
        Ok(descriptor)
    }

    fn dispatch_invoke_variant(&self, request: &ComInvokeRequest) -> HalResult<Variant> {
        let object = request.object.raw();
        let member = request.member.raw();
        let args = request.args.as_slice();
        let positional_values = com_legacy_runtime_arg_values(args);
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dispatch_invoke"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "dispatch_invoke"));
        }
        #[cfg(target_os = "windows")]
        if self.native_com_enabled() {
            match self.com_bridge.dispatch_invoke_variant(
                request,
                self.policy.com_invocation_strategy == ComInvocationStrategy::PreferVtable,
            ) {
                Ok(Some(value)) => {
                    return Ok(value);
                }
                Ok(None) => {}
                Err(WindowsComBridgeDispatchError::Message(message)) => {
                    return Err(self.com_dispatch_adapter_fault(message));
                }
                Err(WindowsComBridgeDispatchError::InvokeFailure(failure)) => {
                    return Err(self.com_dispatch_invoke_fault(failure));
                }
            }
        }
        let positional_values = positional_values.ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "dispatch_invoke",
                "COM-E-VALUE-TRANSPORT-UNSUPPORTED: fallback dispatch lane requires legacy runtime-token arguments",
            )
        })?;
        if args.is_empty() {
            match member {
                // Preserve the controlled imported RaiseException lane on the deterministic
                // projection path so host-returned CreateObject fallback handles can surface
                // the same bounded adapter fault instead of silently collapsing into success.
                17 => {
                    return Err(self.controlled_dispatch_exception_fault(member));
                }
                // Preserve the controlled self-object lane on the deterministic projection
                // path by returning the already bound object identity rather than inventing
                // another raw handle that carries no metadata.
                23 | 24 => {
                    return Ok(Variant::from_object_ref(ObjectRef::from_compat_identity(
                        object,
                    )));
                }
                _ => {}
            }
        }
        Ok(Variant::from_i32(
            positional_values
                .iter()
                .fold(object.saturating_add(member), |acc, arg| {
                    acc.saturating_add(*arg)
                }),
        ))
    }

    fn dispatch_invoke_dynamic_variant(&self, request: &DynamicCallRequest) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dispatch_invoke"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "dispatch_invoke"));
        }
        #[cfg(target_os = "windows")]
        if self.native_com_enabled() {
            match self.com_bridge.dispatch_invoke_dynamic_variant(
                request,
                self.policy.com_invocation_strategy == ComInvocationStrategy::PreferVtable,
            ) {
                Ok(Some(value)) => {
                    return Ok(value);
                }
                Ok(None) => {}
                Err(WindowsComBridgeDispatchError::Message(message)) => {
                    return Err(self.com_dispatch_adapter_fault(message));
                }
                Err(WindowsComBridgeDispatchError::InvokeFailure(failure)) => {
                    return Err(self.com_dispatch_invoke_fault(failure));
                }
            }
        }
        let lowered = match &request.member {
            DynamicMemberSelector::Name(name) => {
                #[cfg(target_os = "windows")]
                {
                    if let Some(descriptor) = self
                        .com_bridge
                        .describe_object(request.object.clone())
                        .map_err(|message| self.com_dispatch_adapter_fault(message))?
                        && let Some((member_token, _)) = self
                            .com_bridge
                            .known_member_spec_for_prog_id_name_by_name(
                                &descriptor.prog_id_name,
                                name,
                            )
                            .map_err(|message| self.com_dispatch_adapter_fault(message))?
                    {
                        ComInvokeRequest {
                            object: request.object.clone(),
                            member: member_token,
                            args: request.args.clone().into_iter().map(Into::into).collect(),
                            invoke_kind_hint: request.call_kind_hint.map(Into::into),
                        }
                    } else {
                        return Err(HalError::adapter_fault(
                            self.profile,
                            capability,
                            "dispatch_invoke",
                            format!(
                                "COM-E-DYNAMIC-NAME-UNRESOLVED: dynamic member name `{name}` did not resolve through authoritative object metadata"
                            ),
                        ));
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    return Err(HalError::adapter_fault(
                        self.profile,
                        capability,
                        "dispatch_invoke",
                        format!(
                            "COM-E-DYNAMIC-NAME-UNRESOLVED: dynamic member name `{name}` requires authoritative metadata resolution before COM lowering"
                        ),
                    ));
                }
            }
            DynamicMemberSelector::Token(_) | DynamicMemberSelector::DefaultMember => {
                request.try_into_com_invoke_request().map_err(|detail| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "dispatch_invoke",
                        format!("dynamic call request cannot lower to COM invoke: {detail}"),
                    )
                })?
            }
        };
        self.dispatch_invoke_variant(&lowered)
    }

    fn subscribe_event(
        &self,
        object: ObjectRef,
        event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "subscribe_event"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "subscribe_event"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "subscribe_event",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event subscription requires host-backed Windows native mode",
            ));
        }
        self.ensure_thread_com_apartment("subscribe_event")?;
        #[cfg(target_os = "windows")]
        {
            // SAFETY: `ensure_thread_com_apartment` succeeded above, so this thread is
            // COM-initialized as subscribe_event requires; the bridge resolves `object`
            // against its own bindings map (erroring on stale tokens), so any native
            // connection-point Advise runs on a dispatch reference that map still
            // retains for the duration of the call.
            let (subscription, transport, expected_arity) =
                unsafe { self.com_bridge.subscribe_event(object.clone(), event) }.map_err(
                    |message| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "subscribe_event",
                            message,
                        )
                    },
                )?;
            if super::com_event_trace_enabled() {
                eprintln!(
                    "[oxvba-hal][com-event] subscribe object={} event={} subscription={} transport={} arity={}",
                    object.raw(),
                    event.raw(),
                    subscription.raw(),
                    transport.kind_label(),
                    expected_arity
                );
            }
            Ok(subscription)
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn unsubscribe_event_variant(&self, subscription: ComSubscriptionToken) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "unsubscribe_event"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "unsubscribe_event"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "unsubscribe_event",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event subscription requires host-backed Windows native mode",
            ));
        }
        self.ensure_thread_com_apartment("unsubscribe_event")?;
        #[cfg(target_os = "windows")]
        {
            // SAFETY: `ensure_thread_com_apartment` succeeded above, so this thread is
            // COM-initialized as unsubscribe_event requires; the bridge resolves
            // `subscription` in its own subscription table (erroring on stale tokens),
            // so the Unadvise teardown only touches a transport the bridge still owns.
            unsafe { self.com_bridge.unsubscribe_event(subscription) }.map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "unsubscribe_event", message)
            })?;
            Ok(Variant::from_i32(1))
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "poll_event_callback"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "poll_event_callback"));
        }
        if !self.native_com_enabled() {
            return Ok(None);
        }
        #[cfg(target_os = "windows")]
        {
            self.com_bridge.poll_event_callback().map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "poll_event_callback", message)
            })
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn event_callback_subscription(
        &self,
        callback: ComCallbackToken,
    ) -> HalResult<ComSubscriptionToken> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "event_callback_subscription"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "event_callback_subscription"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_subscription",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback lookup requires host-backed Windows native mode",
            ));
        }
        #[cfg(target_os = "windows")]
        {
            self.com_bridge
                .event_callback_subscription(callback)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "event_callback_subscription",
                        message,
                    )
                })
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn event_callback_arity(&self, callback: ComCallbackToken) -> HalResult<usize> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "event_callback_arity"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "event_callback_arity"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arity",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback lookup requires host-backed Windows native mode",
            ));
        }
        #[cfg(target_os = "windows")]
        {
            self.com_bridge
                .event_callback_arity(callback)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "event_callback_arity",
                        message,
                    )
                })
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn event_callback_variant(
        &self,
        callback: ComCallbackToken,
        index: usize,
    ) -> HalResult<oxvba_runtime::Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "event_callback_variant"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "event_callback_variant"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_variant",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback lookup requires host-backed Windows native mode",
            ));
        }
        #[cfg(target_os = "windows")]
        {
            self.com_bridge
                .event_callback_variant(callback, index)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "event_callback_variant",
                        message,
                    )
                })
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn release_event_callback_variant(&self, callback: ComCallbackToken) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "release_event_callback"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "release_event_callback"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "release_event_callback",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback release requires host-backed Windows native mode",
            ));
        }
        #[cfg(target_os = "windows")]
        {
            self.com_bridge
                .release_event_callback(callback)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "release_event_callback",
                        message,
                    )
                })?;
            Ok(Variant::from_i32(1))
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn resolve_typelib_reference(
        &self,
        request: &TypeLibResolveRequest,
    ) -> HalResult<TypeLibResolvedIdentity> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.windows_typelib_supported() {
            return Err(self.unsupported(capability, "resolve_typelib_reference"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "resolve_typelib_reference"));
        }
        #[cfg(target_os = "windows")]
        {
            self.com_bridge
                .resolve_typelib_reference(request)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "resolve_typelib_reference",
                        message,
                    )
                })
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = request;
            Err(self.unsupported(capability, "resolve_typelib_reference"))
        }
    }

    fn load_typelib_metadata(
        &self,
        identity: &TypeLibResolvedIdentity,
    ) -> HalResult<TypeLibMetadataBlob> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.windows_typelib_supported() {
            return Err(self.unsupported(capability, "load_typelib_metadata"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "load_typelib_metadata"));
        }
        #[cfg(target_os = "windows")]
        {
            self.com_bridge
                .load_typelib_metadata(identity)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "load_typelib_metadata",
                        message,
                    )
                })
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = identity;
            Err(self.unsupported(capability, "load_typelib_metadata"))
        }
    }

    fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.windows_typelib_supported() {
            return Err(self.unsupported(capability, "invalidate_typelib_cache"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "invalidate_typelib_cache"));
        }
        #[cfg(target_os = "windows")]
        {
            let removed = self
                .com_bridge
                .invalidate_typelib_cache(scope, reference_name)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "invalidate_typelib_cache",
                        message,
                    )
                })?;
            Ok(Variant::from_i32(
                i32::try_from(removed).unwrap_or(i32::MAX),
            ))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (scope, reference_name);
            Err(self.unsupported(capability, "invalidate_typelib_cache"))
        }
    }

    fn com_dispatch_transport_counts(&self) -> (u64, u64) {
        #[cfg(target_os = "windows")]
        {
            (
                self.com_bridge.vtable_call_count(),
                self.com_bridge.idispatch_call_count(),
            )
        }
        #[cfg(not(target_os = "windows"))]
        {
            (0, 0)
        }
    }
}
