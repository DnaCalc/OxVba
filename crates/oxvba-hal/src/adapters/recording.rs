//! Recording host services: wraps a real host and journals all HAL interactions.

use std::sync::{Arc, Mutex};

use crate::{
    error::HalResult,
    journal::{HalJournal, HalJournalEntry},
    model::{HalDescriptor, HalProfileId, HostPolicy},
    traits::{
        ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal,
        HostServices, ProcessEnvHal, ProjectCatalogHal, ProjectMutationHal, ProjectReferenceHal,
        TimeLocaleHal, TypeLibCacheScope, TypeLibMetadataBlob, TypeLibResolveRequest,
        TypeLibResolvedIdentity, UiInteractionHal,
    },
};
use oxvba_com::{
    ComCallbackPayload, ComCallbackToken, ComInvokeRequest, ComMemberToken, ComObjectDescriptor,
    ComSubscriptionToken, DynamicCallRequest,
};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, F64Subtype, ObjectRef, RuntimeValue};

pub struct RecordingHostServices {
    inner: Arc<dyn HostServices>,
    journal: Mutex<HalJournal>,
    sequence: Mutex<u64>,
}

impl RecordingHostServices {
    pub fn new(inner: Arc<dyn HostServices>) -> Self {
        let profile_name = format!("{:?}", inner.profile()).to_lowercase();
        Self {
            inner,
            journal: Mutex::new(HalJournal::new(&profile_name)),
            sequence: Mutex::new(0),
        }
    }

    /// Returns a clone of the current journal.
    pub fn take_journal(&self) -> HalJournal {
        self.journal
            .lock()
            .expect("journal lock not poisoned")
            .clone()
    }

    fn next_sequence(&self) -> u64 {
        let mut seq = self.sequence.lock().expect("sequence lock not poisoned");
        *seq += 1;
        *seq
    }

    fn encode_runtime_value(rv: &RuntimeValue) -> Option<serde_json::Value> {
        match rv {
            RuntimeValue::I32(value) => Some(serde_json::json!({
                "kind": "i32",
                "value": value
            })),
            RuntimeValue::F64(value) => Some(serde_json::json!({
                "kind": "f64",
                "value": value.as_f64(),
                "subtype": match value.subtype() {
                    F64Subtype::Single => "single",
                    F64Subtype::Double => "double",
                    F64Subtype::Date => "date",
                }
            })),
            _ => None,
        }
    }

    fn record_i32(
        &self,
        capability: &str,
        operation: &str,
        family: &str,
        result: &HalResult<RuntimeValue>,
    ) {
        if let Ok(rv) = result {
            let i = match rv {
                RuntimeValue::I32(v) => *v,
                _ => return,
            };
            let seq = self.next_sequence();
            let entry =
                HalJournalEntry::new(seq, capability, operation, family, serde_json::json!(i));
            self.journal
                .lock()
                .expect("journal lock not poisoned")
                .entries
                .push(entry);
        }
    }

    fn record_string(
        &self,
        capability: &str,
        operation: &str,
        family: &str,
        result: &HalResult<RuntimeValue>,
    ) {
        if let Ok(RuntimeValue::String(s)) = result {
            let seq = self.next_sequence();
            let entry =
                HalJournalEntry::new(seq, capability, operation, family, serde_json::json!(s.0));
            self.journal
                .lock()
                .expect("journal lock not poisoned")
                .entries
                .push(entry);
        }
    }

    fn record_runtime_value(
        &self,
        capability: &str,
        operation: &str,
        family: &str,
        result: &HalResult<RuntimeValue>,
    ) {
        if let Ok(rv) = result
            && let Some(payload) = Self::encode_runtime_value(rv)
        {
            let seq = self.next_sequence();
            let entry = HalJournalEntry::new(seq, capability, operation, family, payload);
            self.journal
                .lock()
                .expect("journal lock not poisoned")
                .entries
                .push(entry);
        }
    }
}

impl HostServices for RecordingHostServices {
    fn profile(&self) -> HalProfileId {
        self.inner.profile()
    }

    fn descriptor(&self) -> HalDescriptor {
        self.inner.descriptor()
    }

    fn policy(&self) -> &HostPolicy {
        self.inner.policy()
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

    fn project_catalog(&self) -> Option<&dyn ProjectCatalogHal> {
        self.inner.project_catalog()
    }

    fn project_references(&self) -> Option<&dyn ProjectReferenceHal> {
        self.inner.project_references()
    }

    fn project_mutation(&self) -> Option<&dyn ProjectMutationHal> {
        self.inner.project_mutation()
    }
}

impl ConsoleHal for RecordingHostServices {
    fn print_line(&self, data: RuntimeValue) -> HalResult<RuntimeValue> {
        self.inner.console().print_line(data)
    }

    fn input_fields(&self, count: RuntimeValue) -> HalResult<RuntimeValue> {
        self.inner.console().input_fields(count)
    }

    fn line_input(&self) -> HalResult<RuntimeValue> {
        self.inner.console().line_input()
    }
}

impl UiInteractionHal for RecordingHostServices {
    fn msg_box(&self, prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.ui().msg_box(prompt, style);
        self.record_i32("ui", "msg_box", "ui.msgbox", &result);
        result
    }

    fn input_box(
        &self,
        prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        let result = self.inner.ui().input_box(prompt, default_value);
        self.record_string("ui", "input_box", "ui.inputbox", &result);
        result
    }
}

impl EventPumpHal for RecordingHostServices {
    fn do_events(&self) -> HalResult<RuntimeValue> {
        let result = self.inner.events().do_events();
        self.record_i32("events", "do_events", "events.pump", &result);
        result
    }
}

impl FileSystemHal for RecordingHostServices {
    fn open(&self, path: RuntimeValue, mode: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().open(path, mode);
        self.record_i32("fs", "open", "fs.open", &result);
        result
    }

    fn close(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().close(handle);
        self.record_i32("fs", "close", "fs.close", &result);
        result
    }

    fn kill(&self, path: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().kill(path);
        self.record_i32("fs", "kill", "fs.kill", &result);
        result
    }

    fn seek(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().seek(handle, position);
        self.record_i32("fs", "seek", "fs.seek", &result);
        result
    }

    fn eof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().eof(handle);
        self.record_i32("fs", "eof", "fs.eof", &result);
        result
    }

    fn lof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().lof(handle);
        self.record_i32("fs", "lof", "fs.lof", &result);
        result
    }

    fn free_file(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().free_file(range_selector);
        self.record_i32("fs", "free_file", "fs.freefile", &result);
        result
    }

    fn read_bytes(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue> {
        // Complex byte data — delegate without recording.
        self.inner.fs().read_bytes(handle, count)
    }

    fn write_bytes(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue> {
        // Complex byte data — delegate without recording.
        self.inner.fs().write_bytes(handle, data)
    }

    fn print_line(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue> {
        // Complex text data — delegate without recording.
        self.inner.fs().print_line(handle, data)
    }

    fn input_fields(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue> {
        // Complex field data — delegate without recording.
        self.inner.fs().input_fields(handle, count)
    }

    fn line_input(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        // Complex text data — delegate without recording.
        self.inner.fs().line_input(handle)
    }

    fn loc(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.fs().loc(handle);
        self.record_i32("fs", "loc", "fs.loc", &result);
        result
    }
}

impl ProcessEnvHal for RecordingHostServices {
    fn shell(&self, command: RuntimeValue, window_style: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.process().shell(command, window_style);
        self.record_i32("process", "shell", "process.shell", &result);
        result
    }

    fn environ(&self, key: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.process().environ(key);
        self.record_i32("process", "environ", "process.environ", &result);
        result
    }

    fn dir(&self, path: RuntimeValue, attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.process().dir(path, attrs);
        self.record_i32("process", "dir", "process.dir", &result);
        result
    }
}

impl ComHal for RecordingHostServices {
    fn create_object(&self, prog_id: RuntimeValue) -> HalResult<RuntimeValue> {
        // Delegate to inner; COM objects are opaque handles, not trivially recorded.
        self.inner.com().create_object(prog_id)
    }

    fn release_object(&self, object: ObjectRef) -> HalResult<RuntimeValue> {
        self.inner.com().release_object(object)
    }

    fn describe_object(&self, object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>> {
        self.inner.com().describe_object(object)
    }

    fn dispatch_invoke_runtime_value_v2(
        &self,
        request: &ComInvokeRequest,
    ) -> HalResult<RuntimeValue> {
        self.inner.com().dispatch_invoke_runtime_value_v2(request)
    }

    fn dispatch_invoke_dynamic_runtime_value_v2(
        &self,
        request: &DynamicCallRequest,
    ) -> HalResult<RuntimeValue> {
        self.inner
            .com()
            .dispatch_invoke_dynamic_runtime_value_v2(request)
    }

    fn subscribe_event(
        &self,
        object: ObjectRef,
        event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken> {
        self.inner.com().subscribe_event(object, event)
    }

    fn unsubscribe_event(&self, subscription: ComSubscriptionToken) -> HalResult<RuntimeValue> {
        self.inner.com().unsubscribe_event(subscription)
    }

    fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>> {
        self.inner.com().poll_event_callback()
    }

    fn event_callback_subscription(
        &self,
        callback: ComCallbackToken,
    ) -> HalResult<ComSubscriptionToken> {
        self.inner.com().event_callback_subscription(callback)
    }

    fn event_callback_arity(&self, callback: ComCallbackToken) -> HalResult<usize> {
        self.inner.com().event_callback_arity(callback)
    }

    fn event_callback_arg(
        &self,
        callback: ComCallbackToken,
        index: usize,
    ) -> HalResult<RuntimeValue> {
        self.inner.com().event_callback_arg(callback, index)
    }

    fn release_event_callback(&self, callback: ComCallbackToken) -> HalResult<RuntimeValue> {
        self.inner.com().release_event_callback(callback)
    }

    fn resolve_typelib_reference(
        &self,
        request: &TypeLibResolveRequest,
    ) -> HalResult<TypeLibResolvedIdentity> {
        self.inner.com().resolve_typelib_reference(request)
    }

    fn load_typelib_metadata(
        &self,
        identity: &TypeLibResolvedIdentity,
    ) -> HalResult<TypeLibMetadataBlob> {
        self.inner.com().load_typelib_metadata(identity)
    }

    fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> HalResult<RuntimeValue> {
        self.inner
            .com()
            .invalidate_typelib_cache(scope, reference_name)
    }
}

impl TimeLocaleHal for RecordingHostServices {
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        let result = self.inner.time_locale().date_serial_now();
        self.record_runtime_value("time", "date_serial_now", "time.date", &result);
        result
    }

    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        let result = self.inner.time_locale().time_serial_now();
        self.record_runtime_value("time", "time_serial_now", "time.time", &result);
        result
    }

    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        let result = self.inner.time_locale().timer_ticks();
        self.record_runtime_value("time", "timer_ticks", "time.timer", &result);
        result
    }
}

impl DynamicLinkHal for RecordingHostServices {
    fn invoke_bound(&self, binding: BindingHandle, arg: RuntimeValue) -> HalResult<RuntimeValue> {
        self.inner.dynlink().invoke_bound(binding, arg)
    }

    fn invoke_descriptor(
        &self,
        descriptor: &crate::traits::DynLinkDescriptorView<'_>,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        self.inner.dynlink().invoke_descriptor(descriptor, arg)
    }

    fn invoke_symbol(&self, symbol: DynLinkSymbol, arg: RuntimeValue) -> HalResult<RuntimeValue> {
        self.inner.dynlink().invoke_symbol(symbol, arg)
    }
}

impl DiagnosticsHal for RecordingHostServices {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue> {
        let result = self.inner.diag().emit(code, payload);
        self.record_i32("diagnostics", "emit", "diagnostics.emit", &result);
        result
    }

    fn debug_print(&self, text: RuntimeValue) -> HalResult<RuntimeValue> {
        self.inner.diag().debug_print(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::null::NullHostServices;

    #[test]
    fn recording_captures_time_calls() {
        let null = NullHostServices::boxed(HostPolicy::default());
        let recorder = RecordingHostServices::new(null);

        // Make a time call through the recorder.
        let _ = recorder.time_locale().timer_ticks();
        let _ = recorder.time_locale().date_serial_now();

        let journal = recorder.take_journal();
        assert_eq!(journal.entries.len(), 2);
        assert_eq!(journal.entries[0].operation, "timer_ticks");
        assert_eq!(journal.entries[0].capability, "time");
        assert_eq!(journal.entries[0].sequence, 1);
        assert_eq!(journal.entries[1].operation, "date_serial_now");
        assert_eq!(journal.entries[1].sequence, 2);
    }

    #[test]
    fn recording_journal_roundtrips_to_json() {
        let null = NullHostServices::boxed(HostPolicy::default());
        let recorder = RecordingHostServices::new(null);

        let _ = recorder.time_locale().timer_ticks();

        let journal = recorder.take_journal();
        let json = journal.to_json().expect("serialize");
        let restored = HalJournal::from_json(&json).expect("deserialize");

        assert_eq!(restored.entries.len(), 1);
        assert_eq!(restored.entries[0].operation, "timer_ticks");
        assert_eq!(restored.entries[0].capability, "time");
        assert_eq!(
            restored.entries[0].result,
            serde_json::json!({
                "kind": "f64",
                "value": 45296.0,
                "subtype": "single"
            })
        );
    }

    #[test]
    fn empty_recorder_has_no_entries() {
        let null = NullHostServices::boxed(HostPolicy::default());
        let recorder = RecordingHostServices::new(null);
        let journal = recorder.take_journal();
        assert!(journal.entries.is_empty());
        assert_eq!(journal.schema, "oxvba.hal-journal.v1");
    }
}
