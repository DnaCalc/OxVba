use std::sync::Arc;

use crate::{
    adapters::standard::descriptor_for_profile,
    error::{HalError, HalResult},
    model::CapabilityId,
    model::{HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy},
    traits::{
        ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, HostServices,
        ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, TypeLibraryHal, UiInteractionHal,
    },
};

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
    fn typelibs(&self) -> &dyn TypeLibraryHal {
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

impl UiInteractionHal for NullHostServices {
    fn msg_box(&self, _prompt: i32, _style: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::UiInteraction, "msg_box"))
    }

    fn input_box(&self, _prompt: i32, _default_value: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::UiInteraction, "input_box"))
    }
}

impl EventPumpHal for NullHostServices {
    fn do_events(&self) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::EventPump, "do_events"))
    }
}

impl FileSystemHal for NullHostServices {
    fn open(&self, _path: i32, _mode: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "open"))
    }

    fn close(&self, _handle: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "close"))
    }

    fn seek(&self, _handle: i32, _position: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "seek"))
    }

    fn eof(&self, _handle: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "eof"))
    }

    fn lof(&self, _handle: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "lof"))
    }

    fn free_file(&self, _range_selector: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "free_file"))
    }
}

impl ProcessEnvHal for NullHostServices {
    fn shell(&self, _command: i32, _window_style: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "shell"))
    }

    fn environ(&self, _key: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "environ"))
    }

    fn dir(&self, _path: i32, _attrs: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "dir"))
    }
}

impl ComHal for NullHostServices {
    fn create_object(&self, _prog_id: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "create_object"))
    }

    fn dispatch_invoke(&self, _object: i32, _member: i32, _arg: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "dispatch_invoke"))
    }

    fn subscribe_event(&self, _object: i32, _event: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "subscribe_event"))
    }

    fn unsubscribe_event(&self, _subscription: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "unsubscribe_event"))
    }
}

impl TypeLibraryHal for NullHostServices {
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
    ) -> HalResult<i32> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "invalidate_typelib_cache",
        ))
    }
}

impl TimeLocaleHal for NullHostServices {
    fn date_serial_now(&self) -> HalResult<i32> {
        Ok(20_260_301)
    }

    fn time_serial_now(&self) -> HalResult<i32> {
        Ok(123_456)
    }

    fn timer_ticks(&self) -> HalResult<i32> {
        Ok(42)
    }
}

impl DynamicLinkHal for NullHostServices {
    fn invoke_symbol(&self, _symbol: i32, _arg: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
}

impl DiagnosticsHal for NullHostServices {
    fn emit(&self, code: i32, payload: i32) -> HalResult<i32> {
        Ok(code.saturating_add(payload))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::HalErrorKind,
        traits::{ComHal, DiagnosticsHal, ProcessEnvHal, TimeLocaleHal, UiInteractionHal},
    };

    use super::NullHostServices;

    #[test]
    fn null_backend_exposes_deterministic_floor() {
        let host = NullHostServices::new(crate::HostPolicy::default());
        assert_eq!(host.date_serial_now().expect("date"), 20_260_301);
        assert_eq!(host.time_serial_now().expect("time"), 123_456);
        assert_eq!(host.timer_ticks().expect("timer"), 42);
        assert_eq!(host.emit(10, 3).expect("emit"), 13);
    }

    #[test]
    fn null_backend_rejects_host_sensitive_domains() {
        let host = NullHostServices::new(crate::HostPolicy::interactive_dev());
        assert_eq!(
            host.msg_box(1, 1).expect_err("msg_box").kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.shell(1, 0).expect_err("shell").kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.subscribe_event(1, 1)
                .expect_err("subscribe_event")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.unsubscribe_event(1)
                .expect_err("unsubscribe_event")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
    }
}
