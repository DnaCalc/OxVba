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
use oxvba_runtime::{BindingHandle, DynLinkSymbol, ObjectRef, Variant};

fn variant_i32(value: &Variant) -> i32 {
    value
        .as_i32()
        .or_else(|| value.as_i16().map(i32::from))
        .or_else(|| value.as_u8().map(i32::from))
        .or_else(|| value.as_bool().map(i32::from))
        .or_else(|| value.as_error_code())
        .or_else(|| value.as_object_ref().map(|value| value.raw()))
        .or_else(|| {
            value
                .as_safearray()
                .map(|value| value.len().min(i32::MAX as usize) as i32)
        })
        .unwrap_or(0)
}

// Null adapter keeps retained Variant methods as the value-model entry points
// for slot-facing VM/JIT code.
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
    fn print_line_variant(&self, _data: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "print_line"))
    }

    fn input_fields_variant(&self, _count: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "input_fields"))
    }

    fn line_input_variant(&self) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "line_input"))
    }
}

impl UiInteractionHal for NullHostServices {
    fn msg_box_variant(&self, _prompt: Variant, _style: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::UiInteraction, "msg_box"))
    }

    fn input_box_variant(&self, _prompt: Variant, _default_value: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::UiInteraction, "input_box"))
    }
}

impl EventPumpHal for NullHostServices {
    fn do_events_variant(&self) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::EventPump, "do_events"))
    }
}

impl FileSystemHal for NullHostServices {
    fn open_variant(&self, _path: Variant, _mode: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "open"))
    }

    fn close_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "close"))
    }

    fn kill_variant(&self, _path: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "kill"))
    }

    fn seek_variant(&self, _handle: Variant, _position: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "seek"))
    }

    fn eof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "eof"))
    }

    fn lof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "lof"))
    }

    fn free_file_variant(&self, _range_selector: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "free_file"))
    }

    fn read_bytes_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "read_bytes"))
    }

    fn write_bytes_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "write_bytes"))
    }

    fn print_line_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "print_line"))
    }

    fn input_fields_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "input_fields"))
    }

    fn line_input_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "line_input"))
    }

    fn loc_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "loc"))
    }
}

impl ProcessEnvHal for NullHostServices {
    fn shell_variant(&self, _command: Variant, _window_style: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "shell"))
    }

    fn environ_variant(&self, _key: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "environ"))
    }

    fn dir_variant(&self, _path: Variant, _attrs: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "dir"))
    }
}

impl ComHal for NullHostServices {
    fn create_object_variant(&self, _prog_id: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "create_object"))
    }

    fn release_object_variant(&self, _object: ObjectRef) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "release_object"))
    }

    fn describe_object(&self, _object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "describe_object"))
    }

    fn subscribe_event(
        &self,
        _object: ObjectRef,
        _event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "subscribe_event"))
    }

    fn unsubscribe_event_variant(&self, _subscription: ComSubscriptionToken) -> HalResult<Variant> {
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

    fn event_callback_variant(
        &self,
        _callback: ComCallbackToken,
        _index: usize,
    ) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arg"))
    }

    fn release_event_callback_variant(&self, _callback: ComCallbackToken) -> HalResult<Variant> {
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
    ) -> HalResult<Variant> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "invalidate_typelib_cache",
        ))
    }
}

impl TimeLocaleHal for NullHostServices {
    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        Ok(Variant::from_date_f64(46_082.0))
    }

    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        Ok(Variant::from_date_f64(45_296.0 / 86_400.0))
    }

    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        Ok(Variant::from_f32(45_296.0))
    }
}

impl DynamicLinkHal for NullHostServices {
    fn invoke_bound_variants(
        &self,
        _binding: BindingHandle,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_descriptor_variants(
        &self,
        _descriptor: &crate::traits::DynLinkDescriptorView<'_>,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_symbol_variant(&self, _symbol: DynLinkSymbol, _arg: &Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
}

impl DiagnosticsHal for NullHostServices {
    fn emit_variant(&self, code: Variant, payload: Variant) -> HalResult<Variant> {
        let code = variant_i32(&code);
        let payload = variant_i32(&payload);
        Ok(Variant::from_i32(code.saturating_add(payload)))
    }

    fn debug_print_variant(&self, _text: Variant) -> HalResult<Variant> {
        Ok(Variant::from_i32(0))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::HalErrorKind,
        traits::{
            ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, FileSystemHal, ProcessEnvHal,
            TimeLocaleHal, UiInteractionHal,
        },
    };
    use oxvba_runtime::{ObjectRef, Variant};

    use super::NullHostServices;

    #[test]
    fn null_backend_exposes_deterministic_floor() {
        let host = NullHostServices::new(crate::HostPolicy::default());
        assert_eq!(
            host.date_serial_now_variant().expect("date"),
            Variant::from_date_f64(46_082.0)
        );
        assert_eq!(
            host.time_serial_now_variant().expect("time"),
            Variant::from_date_f64(45_296.0 / 86_400.0)
        );
        assert_eq!(
            host.timer_ticks_variant().expect("timer"),
            Variant::from_f32(45_296.0)
        );
        assert_eq!(
            host.emit_variant(Variant::from_i32(10), Variant::from_i32(3))
                .expect("emit"),
            Variant::from_i32(13)
        );
    }

    #[test]
    fn null_backend_rejects_host_sensitive_domains() {
        let host = NullHostServices::new(crate::HostPolicy::interactive_dev());
        assert_eq!(
            host.msg_box_variant(Variant::from_i32(1), Variant::from_i32(1))
                .expect_err("msg_box")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.shell_variant(Variant::from_i32(1), Variant::from_i32(0))
                .expect_err("shell")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.subscribe_event(ObjectRef::from_compat_identity(1), 1.into())
                .expect_err("subscribe_event")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.unsubscribe_event_variant(1.into())
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
            host.event_callback_variant(1.into(), 0)
                .expect_err("event_callback_arg")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.release_event_callback_variant(1.into())
                .expect_err("release_event_callback")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
    }

    #[test]
    fn null_variant_companions_are_direct() {
        let host = NullHostServices::new(crate::HostPolicy::default());

        assert_eq!(
            host.date_serial_now_variant().expect("date"),
            Variant::from_date_f64(46_082.0)
        );
        assert_eq!(
            host.emit_variant(Variant::null(), Variant::from_i32(3))
                .expect("emit"),
            Variant::from_i32(3)
        );
        assert_eq!(
            host.debug_print_variant(Variant::null())
                .expect("debug print"),
            Variant::from_i32(0)
        );
        assert_eq!(
            host.invoke_symbol_variant(1.into(), &Variant::null())
                .expect_err("dynamic-link unsupported")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            ConsoleHal::print_line_variant(&host, Variant::null())
                .expect_err("console unsupported")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.open_variant(Variant::null(), Variant::null())
                .expect_err("filesystem unsupported")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.shell_variant(Variant::null(), Variant::null())
                .expect_err("process unsupported")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
impl NullHostServices {}
