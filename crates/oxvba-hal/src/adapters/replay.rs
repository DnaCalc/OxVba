//! Replay host services: deterministic replay from a recorded HAL journal.

use std::sync::Mutex;

use crate::{
    error::{HalError, HalResult},
    journal::{HalJournal, HalJournalEntry},
    model::{CapabilityId, HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy},
    traits::{
        ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal,
        HostServices, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
    },
};
use oxvba_com::{ComCallbackToken, ComMemberToken, ComObjectDescriptor, ComSubscriptionToken};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, F64Value, ObjectRef, RuntimeValue, Variant};

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

    fn replay_runtime_value(&self, op: &'static str) -> HalResult<RuntimeValue> {
        let entry = self.next_entry(op)?;
        self.decode_runtime_value(op, entry)
    }

    fn replay_variant(&self, op: &'static str) -> HalResult<Variant> {
        let value = self.replay_runtime_value(op)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DiagnosticsTelemetry,
                op,
                detail,
            )
        })
    }

    fn replay_i32_variant(&self, op: &'static str) -> HalResult<Variant> {
        let entry = self.next_entry(op)?;
        let value = entry.result.as_i64().unwrap_or(0) as i32;
        Ok(Variant::from_i32(value))
    }

    fn decode_runtime_value(
        &self,
        op: &'static str,
        entry: HalJournalEntry,
    ) -> HalResult<RuntimeValue> {
        let Some(kind) = entry.result.get("kind").and_then(|value| value.as_str()) else {
            return Err(HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DiagnosticsTelemetry,
                op,
                "replay runtime value missing `kind`".to_string(),
            ));
        };
        match kind {
            "i32" => Ok(RuntimeValue::I32(
                entry
                    .result
                    .get("value")
                    .and_then(|value| value.as_i64())
                    .unwrap_or_default() as i32,
            )),
            "f64" => {
                let value = entry
                    .result
                    .get("value")
                    .and_then(|value| value.as_f64())
                    .unwrap_or_default();
                let subtype = entry
                    .result
                    .get("subtype")
                    .and_then(|value| value.as_str())
                    .unwrap_or("double");
                let out = match subtype {
                    "single" => RuntimeValue::F64(F64Value::from_single_f64(value)),
                    "date" => RuntimeValue::F64(F64Value::from_date_f64(value)),
                    _ => RuntimeValue::F64(F64Value::from_f64(value)),
                };
                Ok(out)
            }
            other => Err(HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DiagnosticsTelemetry,
                op,
                format!("replay runtime value kind `{other}` is not supported"),
            )),
        }
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

impl ConsoleHal for ReplayHostServices {
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

impl UiInteractionHal for ReplayHostServices {
    fn msg_box(&self, _prompt: RuntimeValue, _style: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("msg_box")
    }

    fn msg_box_variant(&self, _prompt: Variant, _style: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("msg_box")
    }

    fn input_box(&self, _prompt: RuntimeValue, _default: RuntimeValue) -> HalResult<RuntimeValue> {
        let entry = self.next_entry("input_box")?;
        let text = entry.result.as_str().unwrap_or("").to_string();
        Ok(RuntimeValue::String(oxvba_runtime::bstr::BStr::from(text)))
    }

    fn input_box_variant(&self, _prompt: Variant, _default: Variant) -> HalResult<Variant> {
        let entry = self.next_entry("input_box")?;
        let text = entry.result.as_str().unwrap_or("").to_string();
        Ok(Variant::from_string(text))
    }
}

impl EventPumpHal for ReplayHostServices {
    fn do_events(&self) -> HalResult<RuntimeValue> {
        self.replay_i32("do_events")
    }

    fn do_events_variant(&self) -> HalResult<Variant> {
        self.replay_i32_variant("do_events")
    }
}

impl FileSystemHal for ReplayHostServices {
    fn open(&self, _path: RuntimeValue, _mode: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("open")
    }
    fn open_variant(&self, _path: Variant, _mode: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("open")
    }
    fn close(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("close")
    }
    fn close_variant(&self, _handle: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("close")
    }
    fn kill(&self, _path: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("kill")
    }
    fn kill_variant(&self, _path: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("kill")
    }
    fn seek(&self, _handle: RuntimeValue, _pos: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("seek")
    }
    fn eof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("eof")
    }
    fn eof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("eof")
    }
    fn lof(&self, _handle: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("lof")
    }
    fn lof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("lof")
    }
    fn free_file(&self, _range: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("free_file")
    }
    fn free_file_variant(&self, _range: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("free_file")
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
        self.replay_i32("loc")
    }
    fn loc_variant(&self, _handle: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("loc")
    }
}

impl ProcessEnvHal for ReplayHostServices {
    fn shell(&self, _cmd: RuntimeValue, _style: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("shell")
    }
    fn shell_variant(&self, _cmd: Variant, _style: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("shell")
    }
    fn environ(&self, _key: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("environ")
    }
    fn environ_variant(&self, _key: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("environ")
    }
    fn dir(&self, _path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("dir")
    }
    fn dir_variant(&self, _path: Variant, _attrs: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("dir")
    }
}

impl ComHal for ReplayHostServices {
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
        self.replay_runtime_value("date_serial_now")
    }
    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        self.replay_variant("date_serial_now")
    }
    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        self.replay_runtime_value("time_serial_now")
    }
    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        self.replay_variant("time_serial_now")
    }
    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        self.replay_runtime_value("timer_ticks")
    }
    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        self.replay_variant("timer_ticks")
    }
}

impl DynamicLinkHal for ReplayHostServices {
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
        _desc: &crate::traits::DynLinkDescriptorView<'_>,
        _arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Err(self.unsupported(CapabilityId::DynamicLinking, "invoke_symbol"))
    }
    fn invoke_descriptor_variants(
        &self,
        _desc: &crate::traits::DynLinkDescriptorView<'_>,
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

impl DiagnosticsHal for ReplayHostServices {
    fn emit(&self, _code: RuntimeValue, _payload: RuntimeValue) -> HalResult<RuntimeValue> {
        self.replay_i32("emit")
    }

    fn emit_variant(&self, _code: Variant, _payload: Variant) -> HalResult<Variant> {
        self.replay_i32_variant("emit")
    }

    fn debug_print(&self, _text: RuntimeValue) -> HalResult<RuntimeValue> {
        Ok(RuntimeValue::I32(0))
    }

    fn debug_print_variant(&self, _text: Variant) -> HalResult<Variant> {
        Ok(Variant::from_i32(0))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        error::HalErrorKind,
        journal::{HalJournal, HalJournalEntry},
        traits::{
            DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, ProcessEnvHal,
            TimeLocaleHal, UiInteractionHal,
        },
    };
    use oxvba_runtime::Variant;

    use super::ReplayHostServices;

    #[test]
    fn replay_variant_companions_consume_journal_without_trait_projection() {
        let mut journal = HalJournal::new("replay");
        journal.entries.push(HalJournalEntry::new(
            1,
            "time",
            "date_serial_now",
            "time.date",
            serde_json::json!({"kind": "f64", "subtype": "date", "value": 46082.0}),
        ));
        journal.entries.push(HalJournalEntry::new(
            2,
            "diagnostics",
            "emit",
            "diagnostics.emit",
            serde_json::json!(7),
        ));
        journal.entries.push(HalJournalEntry::new(
            3,
            "ui",
            "msg_box",
            "ui.msgbox",
            serde_json::json!(1),
        ));
        journal.entries.push(HalJournalEntry::new(
            4,
            "ui",
            "input_box",
            "ui.inputbox",
            serde_json::json!("answer"),
        ));
        journal.entries.push(HalJournalEntry::new(
            5,
            "event",
            "do_events",
            "event.pump",
            serde_json::json!(0),
        ));
        journal.entries.push(HalJournalEntry::new(
            6,
            "filesystem",
            "open",
            "filesystem.open",
            serde_json::json!(12),
        ));
        journal.entries.push(HalJournalEntry::new(
            7,
            "process",
            "shell",
            "process.shell",
            serde_json::json!(42),
        ));

        let host = ReplayHostServices::new(journal, crate::HostPolicy::default());

        assert_eq!(
            host.date_serial_now_variant().expect("date"),
            Variant::from_date_f64(46_082.0)
        );
        assert_eq!(
            host.emit_variant(Variant::null(), Variant::from_i32(3))
                .expect("emit"),
            Variant::from_i32(7)
        );
        assert_eq!(
            host.debug_print_variant(Variant::null())
                .expect("debug print"),
            Variant::from_i32(0)
        );
        assert_eq!(
            host.msg_box_variant(Variant::null(), Variant::null())
                .expect("msg box"),
            Variant::from_i32(1)
        );
        assert_eq!(
            host.input_box_variant(Variant::null(), Variant::null())
                .expect("input box")
                .as_bstr()
                .expect("input box string")
                .as_str(),
            "answer"
        );
        assert_eq!(
            host.do_events_variant().expect("do events"),
            Variant::from_i32(0)
        );
        assert_eq!(
            host.open_variant(Variant::null(), Variant::null())
                .expect("open"),
            Variant::from_i32(12)
        );
        assert_eq!(
            host.shell_variant(Variant::null(), Variant::null())
                .expect("shell"),
            Variant::from_i32(42)
        );
        assert_eq!(
            host.invoke_symbol_variant(1.into(), &Variant::null())
                .expect_err("dynamic-link unsupported")
                .kind,
            HalErrorKind::CapabilityUnavailable
        );
    }
}
