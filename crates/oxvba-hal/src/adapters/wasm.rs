use std::sync::Arc;

use crate::{
    adapters::standard::descriptor_for_profile,
    error::{HalError, HalResult},
    model::CapabilityId,
    model::UiVirtualizationMode,
    model::{HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy},
    traits::{
        ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, HostServices,
        ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
    },
};
use oxvba_com::ComObjectDescriptor;
use oxvba_runtime::RuntimeValue;

#[derive(Debug, Clone)]
pub struct WasmHostServices {
    descriptor: HalDescriptor,
    policy: HostPolicy,
}

impl WasmHostServices {
    pub fn new(policy: HostPolicy) -> Self {
        let runtime_class = policy.runtime_class.unwrap_or(HalRuntimeClass::default_for(
            HalProfileId::Wasm,
            policy.wasm_runtime_class,
        ));
        Self {
            descriptor: descriptor_for_profile(HalProfileId::Wasm, runtime_class, &policy),
            policy,
        }
    }

    pub fn boxed(policy: HostPolicy) -> Arc<dyn HostServices> {
        Arc::new(Self::new(policy))
    }

    fn unsupported(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::capability_unavailable(HalProfileId::Wasm, capability, op)
    }

    fn denied(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::policy_denied(HalProfileId::Wasm, capability, op)
    }

    fn supports(&self, capability: CapabilityId) -> bool {
        self.descriptor.supports(capability)
    }
}

impl HostServices for WasmHostServices {
    fn profile(&self) -> HalProfileId {
        HalProfileId::Wasm
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

impl UiInteractionHal for WasmHostServices {
    fn msg_box(&self, _prompt: i32, style: i32) -> HalResult<i32> {
        if !self.supports(CapabilityId::UiInteraction) {
            return Err(self.unsupported(CapabilityId::UiInteraction, "msg_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(CapabilityId::UiInteraction, "msg_box"));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::ScriptedResponses => Ok(style.max(1)),
            UiVirtualizationMode::FailOnPrompt | UiVirtualizationMode::Disabled => {
                Err(self.denied(CapabilityId::UiInteraction, "msg_box"))
            }
        }
    }

    fn msg_box_value(&self, _prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue> {
        let style = style.to_legacy_i32().map_err(|detail| {
            crate::HalError::adapter_fault(
                HalProfileId::Wasm,
                CapabilityId::UiInteraction,
                "msg_box",
                format!("style cannot enter legacy wasm UI lane: {detail}"),
            )
        })?;
        self.msg_box(0, style).map(RuntimeValue::from_legacy_i32)
    }

    fn input_box(&self, _prompt: i32, default_value: i32) -> HalResult<i32> {
        if !self.supports(CapabilityId::UiInteraction) {
            return Err(self.unsupported(CapabilityId::UiInteraction, "input_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(CapabilityId::UiInteraction, "input_box"));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::FailOnPrompt | UiVirtualizationMode::Disabled => {
                Err(self.denied(CapabilityId::UiInteraction, "input_box"))
            }
        }
    }

    fn input_box_value(
        &self,
        _prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        if !self.supports(CapabilityId::UiInteraction) {
            return Err(self.unsupported(CapabilityId::UiInteraction, "input_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(CapabilityId::UiInteraction, "input_box"));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::FailOnPrompt | UiVirtualizationMode::Disabled => {
                Err(self.denied(CapabilityId::UiInteraction, "input_box"))
            }
        }
    }
}

impl EventPumpHal for WasmHostServices {
    fn do_events(&self) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::I32(0))
    }
}

impl FileSystemHal for WasmHostServices {
    fn open(&self, _path: i32, _mode: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "open"))
    }

    fn open_value(&self, _path: RuntimeValue, _mode: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "open"))
    }

    fn close(&self, _handle: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "close"))
    }

    fn close_value(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "close"))
    }

    fn seek(&self, _handle: i32, _position: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "seek"))
    }

    fn seek_value(
        &self,
        _handle: RuntimeValue,
        _position: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "seek"))
    }

    fn eof(&self, _handle: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "eof"))
    }

    fn eof_value(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "eof"))
    }

    fn lof(&self, _handle: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "lof"))
    }

    fn lof_value(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "lof"))
    }

    fn free_file(&self, _range_selector: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "free_file"))
    }

    fn free_file_value(&self, _range_selector: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "free_file"))
    }
}

impl ProcessEnvHal for WasmHostServices {
    fn shell(&self, _command: i32, _window_style: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "shell"))
    }

    fn shell_value(
        &self,
        _command: RuntimeValue,
        _window_style: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "shell"))
    }

    fn environ(&self, _key: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "environ"))
    }

    fn environ_value(&self, _key: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "environ"))
    }

    fn dir(&self, _path: i32, _attrs: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "dir"))
    }

    fn dir_value(&self, _path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "dir"))
    }
}

impl ComHal for WasmHostServices {
    fn create_object(&self, _prog_id: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "create_object"))
    }

    fn create_object_value(&self, _prog_id: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "create_object"))
    }

    fn release_object(&self, _object: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "release_object"))
    }

    fn describe_object(&self, _object: i32) -> HalResult<Option<ComObjectDescriptor>> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "describe_object"))
    }

    fn dispatch_invoke_v2(&self, _request: &oxvba_com::ComInvokeRequest) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "dispatch_invoke"))
    }

    fn subscribe_event(&self, _object: i32, _event: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "subscribe_event"))
    }

    fn subscribe_event_value(
        &self,
        _object: RuntimeValue,
        _event: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "subscribe_event"))
    }

    fn unsubscribe_event(&self, _subscription: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "unsubscribe_event"))
    }

    fn unsubscribe_event_value(&self, _subscription: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "unsubscribe_event"))
    }

    fn poll_event_callback(&self) -> HalResult<Option<oxvba_com::ComCallbackPayload>> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "poll_event_callback"))
    }

    fn event_callback_subscription(&self, _callback: i32) -> HalResult<i32> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "event_callback_subscription",
        ))
    }

    fn event_callback_subscription_value(
        &self,
        _callback: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "event_callback_subscription",
        ))
    }

    fn event_callback_arity(&self, _callback: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arity"))
    }

    fn event_callback_arity_value(&self, _callback: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arity"))
    }

    fn event_callback_arg(&self, _callback: i32, _index: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arg"))
    }

    fn event_callback_arg_value(
        &self,
        _callback: RuntimeValue,
        _index: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arg"))
    }

    fn release_event_callback(&self, _callback: i32) -> HalResult<i32> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "release_event_callback",
        ))
    }

    fn release_event_callback_value(&self, _callback: RuntimeValue) -> HalResult<RuntimeValue> {
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
    ) -> HalResult<i32> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "invalidate_typelib_cache",
        ))
    }
}

impl TimeLocaleHal for WasmHostServices {
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::I32(20_260_301))
    }

    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::I32(123_456))
    }

    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::I32(42))
    }
}

impl DynamicLinkHal for WasmHostServices {
    fn invoke_symbol(&self, _symbol: i32, _arg: i32) -> HalResult<i32> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_descriptor_value(
        &self,
        _descriptor: &crate::traits::DynLinkDescriptorView<'_>,
        _arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_descriptor"))
    }

    fn invoke_symbol_value(&self, _symbol: i32, _arg: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
}

impl DiagnosticsHal for WasmHostServices {
    fn emit(&self, code: i32, payload: i32) -> HalResult<i32> {
        Ok(code.saturating_add(payload))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::HalErrorKind,
        model::{CapabilityId, UiVirtualizationMode, WasmRuntimeClass},
        traits::{ComHal, HostServices, ProcessEnvHal, UiInteractionHal},
    };

    use super::WasmHostServices;

    #[test]
    fn wasm_backend_requires_scripted_or_fail_virtualization() {
        let host = WasmHostServices::new(crate::HostPolicy {
            allow_interaction: true,
            ui_virtualization: UiVirtualizationMode::Disabled,
            ..crate::HostPolicy::interactive_dev()
        });
        assert_eq!(
            host.msg_box(3, 1).expect_err("msg_box").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.input_box(3, 1).expect_err("input_box").kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn wasm_backend_rejects_process_env_domain() {
        let host = WasmHostServices::new(crate::HostPolicy::interactive_dev());
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
        assert_eq!(
            host.event_callback_subscription(1)
                .expect_err("event_callback_subscription")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.event_callback_arity(1)
                .expect_err("event_callback_arity")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.event_callback_arg(1, 0)
                .expect_err("event_callback_arg")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
        assert_eq!(
            host.release_event_callback(1)
                .expect_err("release_event_callback")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
    }

    #[test]
    fn wasm_browser_sandbox_descriptor_disables_ui_capability() {
        let host = WasmHostServices::new(
            crate::HostPolicy::deterministic_runtime()
                .with_wasm_runtime_class(WasmRuntimeClass::BrowserSandbox),
        );
        assert_eq!(host.descriptor().runtime_class, "browser-sandbox");
        assert!(
            !host.descriptor().supports(CapabilityId::UiInteraction),
            "browser sandbox class should disable ui capability"
        );
        assert_eq!(
            host.msg_box(1, 1).expect_err("msg_box").kind,
            HalErrorKind::CapabilityUnavailable
        );
    }
}
