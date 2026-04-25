use std::sync::Arc;

use crate::{
    adapters::standard::descriptor_for_profile,
    error::{HalError, HalResult},
    model::CapabilityId,
    model::UiVirtualizationMode,
    model::{HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy},
    traits::{
        ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal,
        HostServices, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
    },
};
use oxvba_com::{ComCallbackToken, ComMemberToken, ComObjectDescriptor, ComSubscriptionToken};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, F64Value, ObjectRef, RuntimeValue, Variant};

// WASM adapter keeps legacy `RuntimeValue` methods for sandbox compatibility
// callers. Slot-facing VM/JIT code should use the direct `_variant` companions.
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

impl ConsoleHal for WasmHostServices {
    fn print_line(&self, _data: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "print_line"))
    }

    fn print_line_variant(&self, _data: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "print_line"))
    }

    fn input_fields(&self, _count: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "input_fields"))
    }

    fn input_fields_variant(&self, _count: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "input_fields"))
    }

    fn line_input(&self) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "line_input"))
    }

    fn line_input_variant(&self) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ConsoleIo, "line_input"))
    }
}

impl UiInteractionHal for WasmHostServices {
    fn msg_box(&self, _prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue> {
        if !self.supports(CapabilityId::UiInteraction) {
            return Err(self.unsupported(CapabilityId::UiInteraction, "msg_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(CapabilityId::UiInteraction, "msg_box"));
        }
        let style = style.project_compat_slot_i32().map_err(|detail| {
            crate::HalError::adapter_fault(
                HalProfileId::Wasm,
                CapabilityId::UiInteraction,
                "msg_box",
                format!("style cannot enter legacy wasm UI lane: {detail}"),
            )
        })?;
        match self.policy.ui_virtualization {
            UiVirtualizationMode::ScriptedResponses | UiVirtualizationMode::HostCallback => {
                Ok(RuntimeValue::I32(style.max(1)))
            }
            UiVirtualizationMode::FailOnPrompt | UiVirtualizationMode::Disabled => {
                Err(self.denied(CapabilityId::UiInteraction, "msg_box"))
            }
        }
    }

    fn msg_box_variant(&self, _prompt: Variant, style: Variant) -> HalResult<Variant> {
        if !self.supports(CapabilityId::UiInteraction) {
            return Err(self.unsupported(CapabilityId::UiInteraction, "msg_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(CapabilityId::UiInteraction, "msg_box"));
        }
        let style = style.project_compat_slot_i32().map_err(|detail| {
            crate::HalError::adapter_fault(
                HalProfileId::Wasm,
                CapabilityId::UiInteraction,
                "msg_box_variant",
                format!("style cannot enter legacy wasm UI lane: {detail}"),
            )
        })?;
        match self.policy.ui_virtualization {
            UiVirtualizationMode::ScriptedResponses | UiVirtualizationMode::HostCallback => {
                Ok(Variant::from_i32(style.max(1)))
            }
            UiVirtualizationMode::FailOnPrompt | UiVirtualizationMode::Disabled => {
                Err(self.denied(CapabilityId::UiInteraction, "msg_box"))
            }
        }
    }

    fn input_box(
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
            UiVirtualizationMode::ScriptedResponses | UiVirtualizationMode::HostCallback => {
                Ok(default_value)
            }
            UiVirtualizationMode::FailOnPrompt | UiVirtualizationMode::Disabled => {
                Err(self.denied(CapabilityId::UiInteraction, "input_box"))
            }
        }
    }

    fn input_box_variant(&self, _prompt: Variant, default_value: Variant) -> HalResult<Variant> {
        if !self.supports(CapabilityId::UiInteraction) {
            return Err(self.unsupported(CapabilityId::UiInteraction, "input_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(CapabilityId::UiInteraction, "input_box"));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::ScriptedResponses | UiVirtualizationMode::HostCallback => {
                Ok(default_value)
            }
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

    fn do_events_variant(&self) -> HalResult<Variant> {
        Ok(Variant::from_i32(0))
    }
}

impl FileSystemHal for WasmHostServices {
    fn open(&self, _path: RuntimeValue, _mode: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "open"))
    }

    fn open_variant(&self, _path: Variant, _mode: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "open"))
    }

    fn close(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "close"))
    }

    fn close_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "close"))
    }

    fn kill(&self, _path: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "kill"))
    }

    fn kill_variant(&self, _path: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "kill"))
    }

    fn seek(&self, _handle: RuntimeValue, _position: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "seek"))
    }

    fn eof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "eof"))
    }

    fn eof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "eof"))
    }

    fn lof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "lof"))
    }

    fn lof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "lof"))
    }

    fn free_file(&self, _range_selector: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "free_file"))
    }

    fn free_file_variant(&self, _range_selector: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "free_file"))
    }

    fn read_bytes(&self, _handle: RuntimeValue, _count: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "read_bytes"))
    }

    fn read_bytes_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "read_bytes"))
    }

    fn write_bytes(&self, _handle: RuntimeValue, _data: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "write_bytes"))
    }

    fn write_bytes_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "write_bytes"))
    }

    fn print_line(&self, _handle: RuntimeValue, _data: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "print_line"))
    }

    fn print_line_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "print_line"))
    }

    fn input_fields(&self, _handle: RuntimeValue, _count: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "input_fields"))
    }

    fn input_fields_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "input_fields"))
    }

    fn line_input(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "line_input"))
    }

    fn line_input_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "line_input"))
    }

    fn loc(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "loc"))
    }

    fn loc_variant(&self, _handle: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::FileSystemIo, "loc"))
    }
}

impl ProcessEnvHal for WasmHostServices {
    fn shell(
        &self,
        _command: RuntimeValue,
        _window_style: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "shell"))
    }

    fn shell_variant(&self, _command: Variant, _window_style: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "shell"))
    }

    fn environ(&self, _key: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "environ"))
    }

    fn environ_variant(&self, _key: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "environ"))
    }

    fn dir(&self, _path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "dir"))
    }

    fn dir_variant(&self, _path: Variant, _attrs: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ProcessEnv, "dir"))
    }
}

impl ComHal for WasmHostServices {
    fn create_object(&self, _prog_id: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "create_object"))
    }

    fn create_object_variant(&self, _prog_id: Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "create_object"))
    }

    fn release_object(&self, _object: ObjectRef) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "release_object"))
    }

    fn describe_object(&self, _object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>> {
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
        _object: ObjectRef,
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

impl WasmHostServices {
    /// Returns true when real system time should be used (WASI runtime, non-deterministic mode).
    fn use_real_time(&self) -> bool {
        use crate::model::WasmRuntimeClass;
        !self.policy.deterministic_mode && self.policy.wasm_runtime_class == WasmRuntimeClass::Wasi
    }
}

impl TimeLocaleHal for WasmHostServices {
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        if self.use_real_time()
            && let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let days_since_epoch = (elapsed.as_secs() / 86400) as i32;
            // VBA serial date: 1 Jan 1900 = 2. Unix epoch 1 Jan 1970 = day 25569.
            let serial = f64::from(days_since_epoch + 25569);
            return Ok(RuntimeValue::F64(F64Value::from_date_f64(serial)));
        }
        Ok(RuntimeValue::F64(F64Value::from_date_f64(46_082.0))) // deterministic fallback
    }

    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        if self.use_real_time()
            && let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let days_since_epoch = (elapsed.as_secs() / 86400) as i32;
            let serial = f64::from(days_since_epoch + 25569);
            return Ok(Variant::from_date_f64(serial));
        }
        Ok(Variant::from_date_f64(46_082.0))
    }

    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        if self.use_real_time()
            && let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let secs_today = (elapsed.as_secs() % 86400) as f64
                + f64::from(elapsed.subsec_nanos()) / 1_000_000_000.0;
            return Ok(RuntimeValue::F64(F64Value::from_date_f64(
                secs_today / 86_400.0,
            )));
        }
        Ok(RuntimeValue::F64(F64Value::from_date_f64(
            45_296.0 / 86_400.0,
        ))) // deterministic fallback
    }

    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        if self.use_real_time()
            && let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let secs_today = (elapsed.as_secs() % 86400) as f64
                + f64::from(elapsed.subsec_nanos()) / 1_000_000_000.0;
            return Ok(Variant::from_date_f64(secs_today / 86_400.0));
        }
        Ok(Variant::from_date_f64(45_296.0 / 86_400.0))
    }

    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        if self.use_real_time()
            && let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let secs_today = (elapsed.as_secs() % 86400) as f64
                + f64::from(elapsed.subsec_nanos()) / 1_000_000_000.0;
            return Ok(RuntimeValue::F64(F64Value::from_single_f64(secs_today)));
        }
        Ok(RuntimeValue::F64(F64Value::from_single_f64(45_296.0))) // deterministic fallback
    }

    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        if self.use_real_time()
            && let Ok(elapsed) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            let secs_today = (elapsed.as_secs() % 86400) as f64
                + f64::from(elapsed.subsec_nanos()) / 1_000_000_000.0;
            return Ok(Variant::from_f32(secs_today as f32));
        }
        Ok(Variant::from_f32(45_296.0))
    }
}

impl DynamicLinkHal for WasmHostServices {
    fn invoke_bound(&self, _binding: BindingHandle, _arg: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_bound_variants(
        &self,
        _binding: BindingHandle,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_descriptor(
        &self,
        _descriptor: &crate::traits::DynLinkDescriptorView<'_>,
        _arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_descriptor_variants(
        &self,
        _descriptor: &crate::traits::DynLinkDescriptorView<'_>,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_symbol(&self, _symbol: DynLinkSymbol, _arg: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }

    fn invoke_symbol_variant(&self, _symbol: DynLinkSymbol, _arg: &Variant) -> HalResult<Variant> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
}

impl DiagnosticsHal for WasmHostServices {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue> {
        let code = code.project_compat_slot_i32().unwrap_or(0);
        let payload = payload.project_compat_slot_i32().unwrap_or(0);
        Ok(RuntimeValue::I32(code.saturating_add(payload)))
    }

    fn debug_print(&self, _text: RuntimeValue) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::I32(0))
    }

    fn emit_variant(&self, code: Variant, payload: Variant) -> HalResult<Variant> {
        let code = code.project_compat_slot_i32().unwrap_or(0);
        let payload = payload.project_compat_slot_i32().unwrap_or(0);
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
        model::{CapabilityId, UiVirtualizationMode, WasmRuntimeClass},
        traits::{
            ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal,
            HostServices, ProcessEnvHal, TimeLocaleHal, UiInteractionHal,
        },
    };
    use oxvba_runtime::{ObjectRef, RuntimeValue, Variant};

    use super::WasmHostServices;

    #[test]
    fn wasm_backend_requires_scripted_or_fail_virtualization() {
        let host = WasmHostServices::new(crate::HostPolicy {
            allow_interaction: true,
            ui_virtualization: UiVirtualizationMode::Disabled,
            ..crate::HostPolicy::interactive_dev()
        });
        assert_eq!(
            host.msg_box(RuntimeValue::I32(3), RuntimeValue::I32(1))
                .expect_err("msg_box")
                .kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.input_box(RuntimeValue::I32(3), RuntimeValue::I32(1))
                .expect_err("input_box")
                .kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn wasm_backend_rejects_process_env_domain() {
        let host = WasmHostServices::new(crate::HostPolicy::interactive_dev());
        assert_eq!(
            host.shell(RuntimeValue::I32(1), RuntimeValue::I32(0))
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
            host.msg_box(RuntimeValue::I32(1), RuntimeValue::I32(1))
                .expect_err("msg_box")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
    }

    #[test]
    fn wasm_variant_companions_are_direct() {
        let host = WasmHostServices::new(crate::HostPolicy::deterministic_runtime());

        assert_eq!(
            host.date_serial_now_variant().expect("date"),
            Variant::from_date_f64(46_082.0)
        );
        assert_eq!(
            host.emit_variant(Variant::null(), Variant::from_i32(3))
                .expect("emit"),
            Variant::from_i32(2)
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
            host.do_events_variant().expect("do events"),
            Variant::from_i32(0)
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
impl WasmHostServices {}
