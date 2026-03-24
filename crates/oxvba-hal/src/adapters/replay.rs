//! Replay host services: deterministic replay from a recorded HAL journal.

use std::sync::Mutex;

use crate::{
    error::{HalError, HalResult},
    journal::{HalJournal, HalJournalEntry},
    model::{CapabilityId, HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy},
    traits::{
        ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, HostServices,
        ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
    },
};
use oxvba_com::{ComCallbackToken, ComMemberToken, ComObjectDescriptor, ComSubscriptionToken};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, ObjectHandle, RuntimeValue};

use super::standard::descriptor_for_profile;

pub struct ReplayHostServices {
    journal: HalJournal,
    cursor: Mutex<usize>,
    descriptor: HalDescriptor,
    policy: HostPolicy,
}

impl ReplayHostServices {
    pub fn new(journal: HalJournal, policy: HostPolicy) -> Self {
        let descriptor =
            descriptor_for_profile(HalProfileId::Null, HalRuntimeClass::NullFloor, &policy);
        Self {
            journal,
            cursor: Mutex::new(0),
            descriptor,
            policy,
        }
    }

    pub fn from_json(json: &str, policy: HostPolicy) -> Result<Self, String> {
        let journal =
            HalJournal::from_json(json).map_err(|e| format!("journal parse error: {e}"))?;
        Ok(Self::new(journal, policy))
    }

    fn next_entry(&self, expected_op: &'static str) -> HalResult<HalJournalEntry> {
        let mut cursor = self.cursor.lock().map_err(|_| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DiagnosticsTelemetry,
                expected_op,
                "replay cursor lock poisoned",
            )
        })?;
        if *cursor >= self.journal.entries.len() {
            return Err(HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DiagnosticsTelemetry,
                expected_op,
                format!(
                    "replay journal exhausted at cursor {} (expected {expected_op})",
                    *cursor
                ),
            ));
        }
        let entry = self.journal.entries[*cursor].clone();
        if entry.operation != expected_op {
            return Err(HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DiagnosticsTelemetry,
                expected_op,
                format!(
                    "replay mismatch at cursor {}: expected `{expected_op}` but journal has `{}`",
                    *cursor, entry.operation
                ),
            ));
        }
        *cursor += 1;
        Ok(entry)
    }

    fn replay_i32(&self, op: &'static str) -> HalResult<RuntimeValue> {
        let entry = self.next_entry(op)?;
        let value = entry.result.as_i64().unwrap_or(0) as i32;
        Ok(RuntimeValue::I32(value))
    }

    fn unsupported(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::capability_unavailable(HalProfileId::Null, capability, op)
    }
}

impl HostServices for ReplayHostServices {
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

impl UiInteractionHal for ReplayHostServices {
    fn msg_box(&self, _prompt: RuntimeValue, _style: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("msg_box")
    }
    fn input_box(&self, _prompt: RuntimeValue, _default: RuntimeValue) -> HalResult<RuntimeValue> {
        let entry = self.next_entry("input_box")?;
        let text = entry.result.as_str().unwrap_or("").to_string();
        Ok(RuntimeValue::String(oxvba_runtime::bstr::BStr(text)))
    }
}

impl EventPumpHal for ReplayHostServices {
    fn do_events(&self) -> HalResult<RuntimeValue> {
        self.replay_i32("do_events")
    }
}

impl FileSystemHal for ReplayHostServices {
    fn open(&self, _path: RuntimeValue, _mode: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("open")
    }
    fn close(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("close")
    }
    fn seek(&self, _handle: RuntimeValue, _pos: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("seek")
    }
    fn eof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("eof")
    }
    fn lof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("lof")
    }
    fn free_file(&self, _range: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("free_file")
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
        self.replay_i32("loc")
    }
}

impl ProcessEnvHal for ReplayHostServices {
    fn shell(&self, _cmd: RuntimeValue, _style: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("shell")
    }
    fn environ(&self, _key: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("environ")
    }
    fn dir(&self, _path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("dir")
    }
}

impl ComHal for ReplayHostServices {
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
    fn unsubscribe_event(&self, _sub: ComSubscriptionToken) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "unsubscribe_event"))
    }
    fn poll_event_callback(&self) -> HalResult<Option<oxvba_com::ComCallbackPayload>> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "poll_event_callback"))
    }
    fn event_callback_subscription(
        &self,
        _cb: ComCallbackToken,
    ) -> HalResult<ComSubscriptionToken> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "event_callback_subscription",
        ))
    }
    fn event_callback_arity(&self, _cb: ComCallbackToken) -> HalResult<usize> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arity"))
    }
    fn event_callback_arg(&self, _cb: ComCallbackToken, _idx: usize) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "event_callback_arg"))
    }
    fn release_event_callback(&self, _cb: ComCallbackToken) -> HalResult<RuntimeValue> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "release_event_callback",
        ))
    }
    fn resolve_typelib_reference(
        &self,
        _req: &TypeLibResolveRequest,
    ) -> HalResult<TypeLibResolvedIdentity> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "resolve_typelib_reference",
        ))
    }
    fn load_typelib_metadata(
        &self,
        _id: &TypeLibResolvedIdentity,
    ) -> HalResult<TypeLibMetadataBlob> {
        Err(self.unsupported(CapabilityId::ComActivationDispatch, "load_typelib_metadata"))
    }
    fn invalidate_typelib_cache(
        &self,
        _scope: TypeLibCacheScope,
        _ref_name: Option<&str>,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(
            CapabilityId::ComActivationDispatch,
            "invalidate_typelib_cache",
        ))
    }
}

impl TimeLocaleHal for ReplayHostServices {
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        self.replay_i32("date_serial_now")
    }
    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        self.replay_i32("time_serial_now")
    }
    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        self.replay_i32("timer_ticks")
    }
}

impl DynamicLinkHal for ReplayHostServices {
    fn invoke_bound(&self, _binding: BindingHandle, _arg: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
    fn invoke_descriptor(
        &self,
        _desc: &crate::traits::DynLinkDescriptorView<'_>,
        _arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
    fn invoke_symbol(&self, _symbol: DynLinkSymbol, _arg: RuntimeValue) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
}

impl DiagnosticsHal for ReplayHostServices {
    fn emit(&self, _code: RuntimeValue, _payload: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("emit")
    }
}
