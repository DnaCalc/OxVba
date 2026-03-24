use crate::{
    error::{HalError, HalResult},
    model::{CapabilityId, ComInvocationStrategy},
    traits::{
        ComHal, TypeLibCacheScope, TypeLibMetadataBlob, TypeLibResolveRequest,
        TypeLibResolvedIdentity,
    },
};
use oxvba_com::{
    ComCallbackPayload, ComCallbackToken, ComInvokeRequest, ComMemberToken,
    ComObjectDescriptor, ComObjectTransportKind, ComSubscriptionToken, DynamicCallRequest,
    legacy_runtime_arg_values as com_legacy_runtime_arg_values,
};
#[cfg(target_os = "windows")]
use oxvba_com::WindowsComBridgeDispatchError;
use oxvba_runtime::{ObjectHandle, RuntimeValue, bstr::BStr};

use super::StandardHostServices;

impl ComHal for StandardHostServices {
    fn create_object(&self, prog_id: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "create_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "create_object"));
        }
        if let RuntimeValue::String(BStr(name)) = &prog_id {
            let prog_id_name = name.trim();
            if prog_id_name.is_empty() {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "create_object",
                    "string ProgID activation requires a non-empty ProgID",
                ));
            }
            if let Some(projection) = &self.portable_objects {
                if let Some(_dispatch) = projection.create_object(prog_id_name) {
                    // Portable registry matched: return a synthetic object handle.
                    // The handle base mirrors the existing fallback convention.
                    let hash = prog_id_name
                        .bytes()
                        .fold(0i32, |acc, b| acc.wrapping_add(b as i32));
                    return Ok(RuntimeValue::ObjectHandle(
                        10_000i32.saturating_add(hash).into(),
                    ));
                }
            }
            if self.native_com_enabled() {
                return self.activate_runtime_object_value_for_prog_id_name(prog_id_name);
            }
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "create_object",
                "string ProgID activation requires native COM-enabled Windows host mode or a portable object registration",
            ));
        }
        let prog_id =
            self.runtime_value_to_legacy_i32(&prog_id, capability, "create_object", "prog_id")?;
        if self.native_com_enabled()
            && let Some(prog_id_name) = self.resolve_native_com_progid(prog_id)
        {
            match self.activate_runtime_object_value_for_prog_id_name(&prog_id_name) {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if self.has_explicit_native_com_override(prog_id) {
                        return Err(err);
                    }
                }
            }
        }
        Ok(RuntimeValue::ObjectHandle(
            5_000i32.saturating_add(prog_id).into(),
        ))
    }

    fn release_object(&self, object: ObjectHandle) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "release_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "release_object"));
        }
        let object = object.raw();
        if !self.native_com_enabled() {
            return Ok(RuntimeValue::I32(if object == 0 { 0 } else { 1 }));
        }
        self.ensure_thread_com_apartment("release_object")?;
        #[cfg(target_os = "windows")]
        {
            let released = unsafe { self.com_bridge.release_object(ObjectHandle::new(object)) }
                .map_err(|message| {
                    HalError::adapter_fault(self.profile, capability, "release_object", message)
                })?;
            if super::com_event_trace_enabled() {
                eprintln!(
                    "[oxvba-hal][com-event] release-object object={} removed_callbacks={}",
                    object,
                    released.stale_callbacks.len()
                );
            }
            return Ok(RuntimeValue::I32(1));
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn describe_object(&self, object: ObjectHandle) -> HalResult<Option<ComObjectDescriptor>> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "describe_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "describe_object"));
        }
        #[cfg(target_os = "windows")]
        if self.native_com_enabled() {
            return self.com_bridge.describe_object(object).map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "describe_object", message)
            });
        }
        let object_handle = object;
        let object = object.raw();
        let descriptor = if object == 0 {
            None
        } else {
            Some(ComObjectDescriptor {
                object: object_handle,
                prog_id_name: format!("selector:{object}"),
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

    fn dispatch_invoke_runtime_value_v2(
        &self,
        request: &ComInvokeRequest,
    ) -> HalResult<RuntimeValue> {
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
            match self.com_bridge.dispatch_invoke_runtime_value(
                request,
                self.policy.com_invocation_strategy == ComInvocationStrategy::PreferVtable,
            ) {
                Ok(Some(value)) => return Ok(value),
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
                // Preserve a bounded object-result lane on the deterministic projection path
                // so host-returned CreateObject fallback handles can still roundtrip the
                // controlled self-object members instead of collapsing them into scalars.
                23 | 24 => {
                    return Ok(RuntimeValue::ObjectHandle(
                        object.saturating_add(member).into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(RuntimeValue::I32(
            positional_values
                .iter()
                .fold(object.saturating_add(member), |acc, arg| {
                    acc.saturating_add(*arg)
                }),
        ))
    }

    fn dispatch_invoke_dynamic_runtime_value_v2(
        &self,
        request: &DynamicCallRequest,
    ) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dispatch_invoke"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "dispatch_invoke"));
        }
        #[cfg(target_os = "windows")]
        if self.native_com_enabled() {
            match self.com_bridge.dispatch_invoke_dynamic_runtime_value(
                request,
                self.policy.com_invocation_strategy == ComInvocationStrategy::PreferVtable,
            ) {
                Ok(Some(value)) => return Ok(value),
                Ok(None) => {}
                Err(WindowsComBridgeDispatchError::Message(message)) => {
                    return Err(self.com_dispatch_adapter_fault(message));
                }
                Err(WindowsComBridgeDispatchError::InvokeFailure(failure)) => {
                    return Err(self.com_dispatch_invoke_fault(failure));
                }
            }
        }
        let lowered = request.try_into_com_invoke_request().map_err(|detail| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "dispatch_invoke",
                format!("dynamic call request cannot lower to COM invoke: {detail}"),
            )
        })?;
        self.dispatch_invoke_runtime_value_v2(&lowered)
    }

    fn subscribe_event(
        &self,
        object: ObjectHandle,
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
            let (subscription, transport, expected_arity) =
                unsafe { self.com_bridge.subscribe_event(object, event) }.map_err(|message| {
                    HalError::adapter_fault(self.profile, capability, "subscribe_event", message)
                })?;
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
            return Ok(subscription);
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn unsubscribe_event(&self, subscription: ComSubscriptionToken) -> HalResult<RuntimeValue> {
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
            unsafe { self.com_bridge.unsubscribe_event(subscription) }.map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "unsubscribe_event", message)
            })?;
            return Ok(RuntimeValue::from_legacy_i32(1));
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
            return self.com_bridge.poll_event_callback().map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "poll_event_callback", message)
            });
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
            return self
                .com_bridge
                .event_callback_subscription(callback)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "event_callback_subscription",
                        message,
                    )
                });
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
            return self
                .com_bridge
                .event_callback_arity(callback)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "event_callback_arity",
                        message,
                    )
                });
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn event_callback_arg(
        &self,
        callback: ComCallbackToken,
        index: usize,
    ) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "event_callback_arg"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "event_callback_arg"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arg",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback lookup requires host-backed Windows native mode",
            ));
        }
        #[cfg(target_os = "windows")]
        {
            return self
                .com_bridge
                .event_callback_arg(callback, index)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "event_callback_arg",
                        message,
                    )
                });
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("native COM is not available on this platform")
    }

    fn release_event_callback(&self, callback: ComCallbackToken) -> HalResult<RuntimeValue> {
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
            return Ok(RuntimeValue::from_legacy_i32(1));
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
            return self
                .com_bridge
                .resolve_typelib_reference(request)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "resolve_typelib_reference",
                        message,
                    )
                });
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("typelib resolution is not available on this platform")
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
            return self
                .com_bridge
                .load_typelib_metadata(identity)
                .map_err(|message| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "load_typelib_metadata",
                        message,
                    )
                });
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("typelib resolution is not available on this platform")
    }

    fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> HalResult<RuntimeValue> {
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
            return Ok(RuntimeValue::I32(
                i32::try_from(removed).unwrap_or(i32::MAX),
            ));
        }
        #[cfg(not(target_os = "windows"))]
        unreachable!("typelib resolution is not available on this platform")
    }
}
