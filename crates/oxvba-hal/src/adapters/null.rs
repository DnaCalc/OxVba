use std::sync::Arc;

use crate::{
    adapters::standard::descriptor_for_profile,
    error::{HalError, HalResult},
    model::CapabilityId,
    model::{HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy},
    traits::{
        ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal,
        HostServices, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
    },
};
use oxvba_com::{ComCallbackToken, ComMemberToken, ComObjectDescriptor, ComSubscriptionToken};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, F64Value, ObjectHandle, RuntimeValue};

#[derive(Debug, Clone)]
pub struct NullHostServices {
    descriptor: HalDescriptor,
    policy: HostPolicy,
}

impl NullHostServices {
    pub fn new(policy: HostPolicy) -> Self {
        let runtime_class = policy.runtime_class.unwrap_or(HalRuntimeClass::NullFloor);
        Self {
            descriptor: descriptor_for_profile(HalProfileId::Null, runtime_class, &policy),
            policy,
        }
    }

    pub fn boxed(policy: HostPolicy) -> Arc<dyn HostServices> {
        Arc::new(Self::new(policy))
    }

    fn unsupported(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::capability_unavailable(HalProfileId::Null, capability, op)
    }
}

impl HostServices for NullHostServices {
    fn profile(&self) -> HalProfileId {
        HalProfileId::Null
    }

    fn descriptor(&self) -> HalDescriptor {
        self.descriptor.clone()
    }

    fn policy(&self) -> &HostPolicy {
        &self.policy
    }

    fn console(&self) -> &dyn ConsoleHal {
        self
    }
    fn ui(&self) -> &dyn UiInteractionHal {
        self
    }
    fn events(&self) -> &dyn EventPumpHal {
        self
    }
    fn fs(&self) -> &dyn FileSystemHal {
        self
    }
    fn process(&self) -> &dyn ProcessEnvHal {
        self
    }
    fn com(&self) -> &dyn ComHal {
        self
    }
    fn time_locale(&self) -> &dyn TimeLocaleHal {
        self
    }
    fn dynlink(&self) -> &dyn DynamicLinkHal {
        self
    }
    fn diag(&self) -> &dyn DiagnosticsHal {
        self
    }
}

impl ConsoleHal for NullHostServices {
    fn print_line(&self, _data: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "print_line"))
    }

    fn input_fields(&self, _count: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "input_fields"))
    }

    fn line_input(&self) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "line_input"))
    }
}

impl UiInteractionHal for NullHostServices {
    fn msg_box(&self, _prompt: RuntimeValue, _style: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::UiInteraction, "msg_box"))
    }

    fn input_box(
        &self,
        _prompt: RuntimeValue,
        _default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::UiInteraction, "input_box"))
    }
}

impl EventPumpHal for NullHostServices {
    fn do_events(&self) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::EventPump, "do_events"))
    }
}

impl FileSystemHal for NullHostServices {
    fn open(&self, _path: RuntimeValue, _mode: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "open"))
    }

    fn close(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "close"))
    }

    fn kill(&self, _path: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "kill"))
    }

    fn seek(&self, _handle: RuntimeValue, _position: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "seek"))
    }

    fn eof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "eof"))
    }

    fn lof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "lof"))
    }

    fn free_file(&self, _range_selector: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "free_file"))
    }

    fn read_bytes(&self, _handle: RuntimeValue, _count: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "read_bytes"))
    }

    fn write_bytes(&self, _handle: RuntimeValue, _data: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "write_bytes"))
    }

    fn print_line(&self, _handle: RuntimeValue, _data: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "print_line"))
    }

    fn input_fields(&self, _handle: RuntimeValue, _count: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "input_fields"))
    }

    fn line_input(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "line_input"))
    }

    fn loc(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "loc"))
    }
}

impl ProcessEnvHal for NullHostServices {
    fn shell(
        &self,
        _command: RuntimeValue,
        _window_style: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "shell"))
    }

    fn environ(&self, _key: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "environ"))
    }

    fn dir(&self, _path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "dir"))
    }
}

impl ComHal for NullHostServices {
    fn create_object(&self, _prog_id: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "create_object"))
    }

    fn release_object(&self, _object: ObjectHandle) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "release_object"))
    }

    fn describe_object(&self, _object: ObjectHandle) -> HalResult<Option<ComObjectDescriptor>> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "describe_object"))
    }

    fn dispatch_invoke_runtime_value_v2(
        &self,
        _request: &oxvba_com::ComInvokeRequest,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "dispatch_invoke"))
    }

    fn dispatch_invoke_dynamic_runtime_value_v2(
        &self,
        _request: &oxvba_com::DynamicCallRequest,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "dispatch_invoke"))
    }

    fn subscribe_event(
        &self,
        _object: ObjectHandle,
        _event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "subscribe_event"))
    }

    fn unsubscribe_event(&self, _subscription: ComSubscriptionToken) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "unsubscribe_event"))
    }

    fn poll_event_callback(&self) -> HalResult<Option<oxvba_com::ComCallbackPayload>> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "poll_event_callback"))
    }

    fn event_callback_subscription(
        &self,
        _callback: ComCallbackToken,
    ) -> HalResult<ComSubscriptionToken> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "event_callback_subscription",
        ))
    }

    fn event_callback_arity(&self, _callback: ComCallbackToken) -> HalResult<usize> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arity"))
    }

    fn event_callback_arg(
        &self,
        _callback: ComCallbackToken,
        _index: usize,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arg"))
    }

    fn release_event_callback(&self, _callback: ComCallbackToken) -> HalResult<RuntimeValue> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "release_event_callback",
        ))
    }
    fn resolve_typelib_reference(
        &self,
        _request: &TypeLibResolveRequest,
    ) -> HalResult<TypeLibResolvedIdentity> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "resolve_typelib_reference",
        ))
    }

    fn load_typelib_metadata(
        &self,
        _identity: &TypeLibResolvedIdentity,
    ) -> HalResult<TypeLibMetadataBlob> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "load_typelib_metadata"))
    }

    fn invalidate_typelib_cache(
        &self,
        _scope: TypeLibCacheScope,
        _reference_name: Option<&str>,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "invalidate_typelib_cache",
        ))
    }
}

impl TimeLocaleHal for NullHostServices {
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::F64(F64Value::from_date_f64(46_082.0)))
    }

    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::F64(F64Value::from_date_f64(
            45_296.0 / 86_400.0,
        )))
    }

    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::F64(F64Value::from_single_f64(45_296.0)))
    }
}

impl DynamicLinkHal for NullHostServices {
    fn invoke_bound(&self, _binding: BindingHandle, _arg: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_descriptor(
        &self,
        _descriptor: &crate::traits::DynLinkDescriptorView<'_>,
        _arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_symbol(&self, _symbol: DynLinkSymbol, _arg: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
}

impl DiagnosticsHal for NullHostServices {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue> {
        let code = code.project_compat_slot_i32().unwrap_or(0);
        let payload = payload.project_compat_slot_i32().unwrap_or(0);
        Ok(RuntimeValue::I32(code.saturating_add(payload)))
    }

    fn debug_print(&self, _text: RuntimeValue) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::I32(0))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::HalErrorKind,
        traits::{ComHal, DiagnosticsHal, ProcessEnvHal, TimeLocaleHal, UiInteractionHal},
    };
    use oxvba_runtime::{F64Value, ObjectHandle, RuntimeValue};

    use super::NullHostServices;

    #[test]
    fn null_backend_exposes_deterministic_floor() {
        let host = NullHostServices::new(crate::HostPolicy::default());
        assert_eq!(
            host.date_serial_now().expect("date"),
            RuntimeValue::F64(F64Value::from_date_f64(46_082.0))
        );
        assert_eq!(
            host.time_serial_now().expect("time"),
            RuntimeValue::F64(F64Value::from_date_f64(45_296.0 / 86_400.0))
        );
        assert_eq!(
            host.timer_ticks().expect("timer"),
            RuntimeValue::F64(F64Value::from_single_f64(45_296.0))
        );
        assert_eq!(
            host.emit(RuntimeValue::I32(10), RuntimeValue::I32(3))
                .expect("emit"),
            RuntimeValue::I32(13)
        );
    }

    #[test]
    fn null_backend_rejects_host_sensitive_domains() {
        let host = NullHostServices::new(crate::HostPolicy::interactive_dev());
        assert_eq!(
            host.msg_box(RuntimeValue::I32(1), RuntimeValue::I32(1))
                .expect_err("msg_box")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.shell(RuntimeValue::I32(1), RuntimeValue::I32(0))
                .expect_err("shell")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.subscribe_event(ObjectHandle::new(1), 1.into())
                .expect_err("subscribe_event")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.unsubscribe_event(1.into())
                .expect_err("unsubscribe_event")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.event_callback_subscription(1.into())
                .expect_err("event_callback_subscription")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.event_callback_arity(1.into())
                .expect_err("event_callback_arity")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.event_callback_arg(1.into(), 0)
                .expect_err("event_callback_arg")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.release_event_callback(1.into())
                .expect_err("release_event_callback")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
impl NullHostServices {}
