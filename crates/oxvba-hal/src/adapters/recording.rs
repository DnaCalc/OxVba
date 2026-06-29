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
use oxvba_runtime::{BindingHandle, DynLinkSymbol, ObjectRef, VarType, Variant};

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

    fn encode_variant(value: &Variant) -> Option<serde_json::Value> {
        match value.vtype() {
            VarType::Long => Some(serde_json::json!({
                "kind": "i32",
                "value": value.as_i32()?
            })),
            VarType::Integer => Some(serde_json::json!({
                "kind": "i32",
                "value": i32::from(value.as_i16()?)
            })),
            VarType::Byte => Some(serde_json::json!({
                "kind": "i32",
                "value": i32::from(value.as_u8()?)
            })),
            VarType::Single => Some(serde_json::json!({
                "kind": "f64",
                "value": value.as_f32()? as f64,
                "subtype": "single"
            })),
            VarType::Double => Some(serde_json::json!({
                "kind": "f64",
                "value": value.as_f64()?,
                "subtype": "double"
            })),
            VarType::Date => Some(serde_json::json!({
                "kind": "f64",
                "value": value.as_date_f64()?,
                "subtype": "date"
            })),
            VarType::String => Some(serde_json::json!(value.as_bstr()?.as_str())),
            _ => None,
        }
    }

    fn record_variant(
        &self,
        capability: &str,
        operation: &str,
        family: &str,
        result: &HalResult<Variant>,
    ) {
        if let Ok(value) = result
            && let Some(payload) = Self::encode_variant(value)
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
    fn print_line_variant(&self, data: Variant) -> HalResult<Variant> {
        self.inner.console().print_line_variant(data)
    }

    fn input_fields_variant(&self, count: Variant) -> HalResult<Variant> {
        self.inner.console().input_fields_variant(count)
    }

    fn line_input_variant(&self) -> HalResult<Variant> {
        self.inner.console().line_input_variant()
    }
}

impl UiInteractionHal for RecordingHostServices {
    fn msg_box_variant(&self, prompt: Variant, style: Variant) -> HalResult<Variant> {
        let result = self.inner.ui().msg_box_variant(prompt, style);
        self.record_variant("ui", "msg_box", "ui.msgbox", &result);
        result
    }

    fn input_box_variant(&self, prompt: Variant, default_value: Variant) -> HalResult<Variant> {
        let result = self.inner.ui().input_box_variant(prompt, default_value);
        self.record_variant("ui", "input_box", "ui.inputbox", &result);
        result
    }
}

impl EventPumpHal for RecordingHostServices {
    fn do_events_variant(&self) -> HalResult<Variant> {
        let result = self.inner.events().do_events_variant();
        self.record_variant("events", "do_events", "events.pump", &result);
        result
    }
}

impl FileSystemHal for RecordingHostServices {
    fn open_variant(&self, path: Variant, mode: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().open_variant(path, mode);
        self.record_variant("fs", "open", "fs.open", &result);
        result
    }

    fn close_variant(&self, handle: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().close_variant(handle);
        self.record_variant("fs", "close", "fs.close", &result);
        result
    }

    fn kill_variant(&self, path: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().kill_variant(path);
        self.record_variant("fs", "kill", "fs.kill", &result);
        result
    }

    fn seek_variant(&self, handle: Variant, position: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().seek_variant(handle, position);
        self.record_variant("fs", "seek", "fs.seek", &result);
        result
    }

    fn eof_variant(&self, handle: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().eof_variant(handle);
        self.record_variant("fs", "eof", "fs.eof", &result);
        result
    }

    fn lof_variant(&self, handle: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().lof_variant(handle);
        self.record_variant("fs", "lof", "fs.lof", &result);
        result
    }

    fn free_file_variant(&self, range_selector: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().free_file_variant(range_selector);
        self.record_variant("fs", "free_file", "fs.freefile", &result);
        result
    }

    fn read_bytes_variant(&self, handle: Variant, count: Variant) -> HalResult<Variant> {
        // Complex byte data - delegate without recording.
        self.inner.fs().read_bytes_variant(handle, count)
    }

    fn write_bytes_variant(&self, handle: Variant, data: Variant) -> HalResult<Variant> {
        // Complex byte data - delegate without recording.
        self.inner.fs().write_bytes_variant(handle, data)
    }

    fn print_line_variant(&self, handle: Variant, data: Variant) -> HalResult<Variant> {
        // Complex text data - delegate without recording.
        self.inner.fs().print_line_variant(handle, data)
    }

    fn input_fields_variant(&self, handle: Variant, count: Variant) -> HalResult<Variant> {
        // Complex field data - delegate without recording.
        self.inner.fs().input_fields_variant(handle, count)
    }

    fn line_input_variant(&self, handle: Variant) -> HalResult<Variant> {
        // Complex text data - delegate without recording.
        self.inner.fs().line_input_variant(handle)
    }

    fn loc_variant(&self, handle: Variant) -> HalResult<Variant> {
        let result = self.inner.fs().loc_variant(handle);
        self.record_variant("fs", "loc", "fs.loc", &result);
        result
    }
}

impl ProcessEnvHal for RecordingHostServices {
    fn shell_variant(&self, command: Variant, window_style: Variant) -> HalResult<Variant> {
        let result = self.inner.process().shell_variant(command, window_style);
        self.record_variant("process", "shell", "process.shell", &result);
        result
    }

    fn environ_variant(&self, key: Variant) -> HalResult<Variant> {
        let result = self.inner.process().environ_variant(key);
        self.record_variant("process", "environ", "process.environ", &result);
        result
    }

    fn dir_variant(&self, path: Variant, attrs: Variant) -> HalResult<Variant> {
        let result = self.inner.process().dir_variant(path, attrs);
        self.record_variant("process", "dir", "process.dir", &result);
        result
    }
}

impl ComHal for RecordingHostServices {
    fn create_object_variant(&self, prog_id: Variant) -> HalResult<Variant> {
        // Delegate to inner; COM objects are opaque handles, not trivially recorded.
        self.inner.com().create_object_variant(prog_id)
    }

    fn get_object_variant(&self, pathname: Variant, class: Variant) -> HalResult<Variant> {
        self.inner.com().get_object_variant(pathname, class)
    }

    fn release_object_variant(&self, object: ObjectRef) -> HalResult<Variant> {
        self.inner.com().release_object_variant(object)
    }

    fn describe_object(&self, object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>> {
        self.inner.com().describe_object(object)
    }

    fn enumerate_object(&self, object: ObjectRef) -> HalResult<Vec<Variant>> {
        self.inner.com().enumerate_object(object)
    }

    fn object_type_name(&self, object: ObjectRef) -> HalResult<Option<String>> {
        self.inner.com().object_type_name(object)
    }

    fn dispatch_invoke_variant(&self, request: &ComInvokeRequest) -> HalResult<Variant> {
        self.inner.com().dispatch_invoke_variant(request)
    }

    fn dispatch_invoke_dynamic_variant(&self, request: &DynamicCallRequest) -> HalResult<Variant> {
        self.inner.com().dispatch_invoke_dynamic_variant(request)
    }

    fn subscribe_event(
        &self,
        object: ObjectRef,
        event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken> {
        self.inner.com().subscribe_event(object, event)
    }

    fn unsubscribe_event_variant(&self, subscription: ComSubscriptionToken) -> HalResult<Variant> {
        self.inner.com().unsubscribe_event_variant(subscription)
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

    fn event_callback_variant(
        &self,
        callback: ComCallbackToken,
        index: usize,
    ) -> HalResult<Variant> {
        self.inner.com().event_callback_variant(callback, index)
    }

    fn release_event_callback_variant(&self, callback: ComCallbackToken) -> HalResult<Variant> {
        self.inner.com().release_event_callback_variant(callback)
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
    ) -> HalResult<Variant> {
        self.inner
            .com()
            .invalidate_typelib_cache(scope, reference_name)
    }

    fn com_dispatch_transport_counts(&self) -> (u64, u64) {
        self.inner.com().com_dispatch_transport_counts()
    }
}

impl TimeLocaleHal for RecordingHostServices {
    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        let result = self.inner.time_locale().date_serial_now_variant();
        self.record_variant("time", "date_serial_now", "time.date", &result);
        result
    }

    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        let result = self.inner.time_locale().time_serial_now_variant();
        self.record_variant("time", "time_serial_now", "time.time", &result);
        result
    }

    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        let result = self.inner.time_locale().timer_ticks_variant();
        self.record_variant("time", "timer_ticks", "time.timer", &result);
        result
    }
}

impl DynamicLinkHal for RecordingHostServices {
    fn invoke_bound_variants(
        &self,
        binding: BindingHandle,
        args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        self.inner.dynlink().invoke_bound_variants(binding, args)
    }

    fn invoke_descriptor_variants(
        &self,
        descriptor: &crate::traits::DynLinkDescriptorView<'_>,
        args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        self.inner
            .dynlink()
            .invoke_descriptor_variants(descriptor, args)
    }

    fn invoke_symbol_variant(&self, symbol: DynLinkSymbol, arg: &Variant) -> HalResult<Variant> {
        self.inner.dynlink().invoke_symbol_variant(symbol, arg)
    }
}

impl DiagnosticsHal for RecordingHostServices {
    fn emit_variant(&self, code: Variant, payload: Variant) -> HalResult<Variant> {
        let result = self.inner.diag().emit_variant(code, payload);
        self.record_variant("diagnostics", "emit", "diagnostics.emit", &result);
        result
    }

    fn debug_print_variant(&self, text: Variant) -> HalResult<Variant> {
        self.inner.diag().debug_print_variant(text)
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

        // Make time calls through the recorder.
        let _ = recorder.time_locale().timer_ticks_variant();
        let _ = recorder.time_locale().date_serial_now_variant();

        let journal = recorder.take_journal();
        assert_eq!(journal.entries.len(), 2);
        assert_eq!(journal.entries[0].operation, "timer_ticks");
        assert_eq!(journal.entries[0].capability, "time");
        assert_eq!(journal.entries[0].sequence, 1);
        assert_eq!(journal.entries[1].operation, "date_serial_now");
        assert_eq!(journal.entries[1].sequence, 2);
    }

    #[test]
    fn recording_captures_variant_calls_without_trait_projection() {
        let null = NullHostServices::boxed(HostPolicy::default());
        let recorder = RecordingHostServices::new(null);

        let _ = recorder.time_locale().timer_ticks_variant();
        let _ = recorder
            .diag()
            .emit_variant(Variant::from_i32(10), Variant::from_i32(3));
        let _ = recorder.time_locale().date_serial_now_variant();

        let journal = recorder.take_journal();
        assert_eq!(journal.entries.len(), 3);
        assert_eq!(journal.entries[0].operation, "timer_ticks");
        assert_eq!(journal.entries[0].result["subtype"], "single");
        assert_eq!(journal.entries[1].operation, "emit");
        assert_eq!(journal.entries[1].result["value"], 13);
        assert_eq!(journal.entries[2].operation, "date_serial_now");
        assert_eq!(journal.entries[2].result["subtype"], "date");
    }

    #[test]
    fn recording_journal_roundtrips_to_json() {
        let null = NullHostServices::boxed(HostPolicy::default());
        let recorder = RecordingHostServices::new(null);

        let _ = recorder.time_locale().timer_ticks_variant();

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
