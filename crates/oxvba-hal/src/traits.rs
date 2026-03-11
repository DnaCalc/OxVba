//! HAL trait contracts.
//!
//! Source anchors (Foundation canonical mirror):
//! - `CreateObject`: CONF-discovered-ms-vbal-250520-f945507e-0325
//! - `InputBox`: CONF-discovered-ms-vbal-250520-f945507e-0329
//! - `MsgBox`: CONF-discovered-ms-vbal-250520-f945507e-0337
//! - `Shell`: CONF-discovered-ms-vbal-250520-f945507e-0346
//! - `Dir`: CONF-discovered-ms-vbal-250520-f945507e-0282
//! - `FreeFile`: CONF-discovered-ms-vbal-250520-f945507e-0286

use crate::{
    error::HalResult,
    model::{HalDescriptor, HalProfileId, HostPolicy},
};
use oxvba_com::{ComCallbackPayload, ComInvokeRequest, ComObjectDescriptor};
pub use oxvba_com::{
    TypeLibCacheScope, TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind,
    TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity,
};
use oxvba_runtime::RuntimeValue;

/// VM/runtime value token crossing the current HAL boundary.
/// This is intentionally `i32` for the current register-window runtime representation.
pub type ValueToken = i32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynLinkDescriptorView<'a> {
    pub descriptor_id: u32,
    pub declared_name: &'a str,
    pub library: &'a str,
    pub alias: &'a str,
    pub ordinal_alias: bool,
    pub symbol: ValueToken,
    pub marshal_lane: &'a str,
    pub calling_convention: &'a str,
    pub selection_policy: &'a str,
}

impl DynLinkDescriptorView<'_> {
    pub fn contract_violation(&self) -> Option<&'static str> {
        if self.declared_name.trim().is_empty() {
            return Some("declared_name is empty");
        }
        if self.library.trim().is_empty() {
            return Some("library is empty");
        }
        if self.alias.trim().is_empty() {
            return Some("alias is empty");
        }
        if self.marshal_lane != "m0-deterministic" {
            return Some("marshal_lane is not m0-deterministic");
        }
        if self.calling_convention != "platform-default" {
            return Some("calling_convention is not platform-default");
        }
        let expected_selection_policy = if self.ordinal_alias {
            "ordinal-literal-canonical"
        } else {
            "case-insensitive-canonical"
        };
        if self.selection_policy != expected_selection_policy {
            return Some("selection_policy does not match ordinal_alias contract");
        }
        None
    }
}

pub trait HostServices: Send + Sync {
    fn profile(&self) -> HalProfileId;
    fn descriptor(&self) -> HalDescriptor;
    fn policy(&self) -> &HostPolicy;

    fn ui(&self) -> &dyn UiInteractionHal;
    fn events(&self) -> &dyn EventPumpHal;
    fn fs(&self) -> &dyn FileSystemHal;
    fn process(&self) -> &dyn ProcessEnvHal;
    fn com(&self) -> &dyn ComHal;
    fn time_locale(&self) -> &dyn TimeLocaleHal;
    fn dynlink(&self) -> &dyn DynamicLinkHal;
    fn diag(&self) -> &dyn DiagnosticsHal;
}

pub trait UiInteractionHal: Send + Sync {
    /// Deterministically implements `MsgBox` interaction or a policy/capability error.
    fn msg_box(&self, prompt: ValueToken, style: ValueToken) -> HalResult<ValueToken>;
    fn msg_box_value(&self, prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Deterministically implements `InputBox` interaction or a policy/capability error.
    fn input_box(&self, prompt: ValueToken, default_value: ValueToken) -> HalResult<ValueToken>;
    fn input_box_value(
        &self,
        prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue>;
}

pub trait EventPumpHal: Send + Sync {
    /// Deterministically pumps host events, or reports unsupported behavior.
    fn do_events(&self) -> HalResult<ValueToken>;
    fn do_events_value(&self) -> HalResult<RuntimeValue> {
        self.do_events().map(RuntimeValue::from_legacy_i32)
    }
}

pub trait FileSystemHal: Send + Sync {
    fn open(&self, path: ValueToken, mode: ValueToken) -> HalResult<ValueToken>;
    fn open_value(&self, path: RuntimeValue, mode: RuntimeValue) -> HalResult<RuntimeValue>;
    fn close(&self, handle: ValueToken) -> HalResult<ValueToken>;
    fn close_value(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn seek(&self, handle: ValueToken, position: ValueToken) -> HalResult<ValueToken>;
    fn seek_value(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue>;
    fn eof(&self, handle: ValueToken) -> HalResult<ValueToken>;
    fn eof_value(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn lof(&self, handle: ValueToken) -> HalResult<ValueToken>;
    fn lof_value(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn free_file(&self, range_selector: ValueToken) -> HalResult<ValueToken>;
    fn free_file_value(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue>;
}

pub trait ProcessEnvHal: Send + Sync {
    fn shell(&self, command: ValueToken, window_style: ValueToken) -> HalResult<ValueToken>;
    fn shell_value(
        &self,
        command: RuntimeValue,
        window_style: RuntimeValue,
    ) -> HalResult<RuntimeValue>;
    fn environ(&self, key: ValueToken) -> HalResult<ValueToken>;
    fn environ_value(&self, key: RuntimeValue) -> HalResult<RuntimeValue>;
    fn dir(&self, path: ValueToken, attrs: ValueToken) -> HalResult<ValueToken>;
    fn dir_value(&self, path: RuntimeValue, attrs: RuntimeValue) -> HalResult<RuntimeValue>;
}

pub trait ComHal: Send + Sync {
    fn create_object(&self, prog_id: ValueToken) -> HalResult<ValueToken>;
    fn create_object_value(&self, prog_id: RuntimeValue) -> HalResult<RuntimeValue>;
    fn release_object(&self, object: ValueToken) -> HalResult<ValueToken>;
    fn describe_object(&self, object: ValueToken) -> HalResult<Option<ComObjectDescriptor>>;
    fn dispatch_invoke_v2(&self, request: &ComInvokeRequest) -> HalResult<ValueToken>;
    fn dispatch_invoke_runtime_value_v2(
        &self,
        request: &ComInvokeRequest,
    ) -> HalResult<RuntimeValue> {
        self.dispatch_invoke_v2(request)
            .map(RuntimeValue::from_legacy_i32)
    }
    fn dispatch_invoke(
        &self,
        object: ValueToken,
        member: ValueToken,
        arg: ValueToken,
    ) -> HalResult<ValueToken> {
        let request = ComInvokeRequest::legacy(object, member, arg);
        self.dispatch_invoke_v2(&request)
    }
    fn subscribe_event(&self, object: ValueToken, event: ValueToken) -> HalResult<ValueToken>;
    fn subscribe_event_value(
        &self,
        object: RuntimeValue,
        event: RuntimeValue,
    ) -> HalResult<RuntimeValue>;
    fn unsubscribe_event(&self, subscription: ValueToken) -> HalResult<ValueToken>;
    fn unsubscribe_event_value(&self, subscription: RuntimeValue) -> HalResult<RuntimeValue>;
    fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>>;
    fn event_callback_subscription(&self, callback: ValueToken) -> HalResult<ValueToken>;
    fn event_callback_subscription_value(&self, callback: RuntimeValue) -> HalResult<RuntimeValue>;
    fn event_callback_arity(&self, callback: ValueToken) -> HalResult<ValueToken>;
    fn event_callback_arity_value(&self, callback: RuntimeValue) -> HalResult<RuntimeValue>;
    fn event_callback_arg(&self, callback: ValueToken, index: ValueToken) -> HalResult<ValueToken>;
    fn event_callback_arg_value(
        &self,
        callback: RuntimeValue,
        index: RuntimeValue,
    ) -> HalResult<RuntimeValue>;
    fn release_event_callback(&self, callback: ValueToken) -> HalResult<ValueToken>;
    fn release_event_callback_value(&self, callback: RuntimeValue) -> HalResult<RuntimeValue>;
    fn resolve_typelib_reference(
        &self,
        request: &TypeLibResolveRequest,
    ) -> HalResult<TypeLibResolvedIdentity>;
    fn load_typelib_metadata(
        &self,
        identity: &TypeLibResolvedIdentity,
    ) -> HalResult<TypeLibMetadataBlob>;
    fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> HalResult<ValueToken>;
}

pub trait TimeLocaleHal: Send + Sync {
    fn date_serial_now(&self) -> HalResult<ValueToken>;
    fn date_serial_now_value(&self) -> HalResult<RuntimeValue> {
        self.date_serial_now().map(RuntimeValue::from_legacy_i32)
    }
    fn time_serial_now(&self) -> HalResult<ValueToken>;
    fn time_serial_now_value(&self) -> HalResult<RuntimeValue> {
        self.time_serial_now().map(RuntimeValue::from_legacy_i32)
    }
    fn timer_ticks(&self) -> HalResult<ValueToken>;
    fn timer_ticks_value(&self) -> HalResult<RuntimeValue> {
        self.timer_ticks().map(RuntimeValue::from_legacy_i32)
    }
}

pub trait DynamicLinkHal: Send + Sync {
    /// Resolves descriptor metadata into an invocation binding token.
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<ValueToken> {
        Ok(descriptor.symbol)
    }
    fn bind_descriptor_value(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
    ) -> HalResult<RuntimeValue> {
        self.bind_descriptor(descriptor)
            .map(RuntimeValue::from_legacy_i32)
    }

    /// Optional argument normalization/writeback preparation hook.
    fn prepare_invoke(&self, _binding: ValueToken, arg: ValueToken) -> HalResult<ValueToken> {
        Ok(arg)
    }
    fn prepare_invoke_value(
        &self,
        binding: ValueToken,
        arg: ValueToken,
    ) -> HalResult<RuntimeValue> {
        self.prepare_invoke(binding, arg)
            .map(RuntimeValue::from_legacy_i32)
    }

    /// Invokes a previously bound symbol token.
    fn invoke_bound(&self, binding: ValueToken, arg: ValueToken) -> HalResult<ValueToken> {
        self.invoke_symbol(binding, arg)
    }
    fn invoke_bound_value(&self, binding: ValueToken, arg: ValueToken) -> HalResult<RuntimeValue> {
        self.invoke_bound(binding, arg)
            .map(RuntimeValue::from_legacy_i32)
    }

    /// Descriptor-driven invoke path used by VM/host integrations.
    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: ValueToken,
    ) -> HalResult<ValueToken> {
        let binding = self.bind_descriptor(descriptor)?;
        let prepared = self.prepare_invoke(binding, arg)?;
        self.invoke_bound(binding, prepared)
    }
    fn invoke_descriptor_value(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue>;

    /// Legacy symbol-token invoke path retained for backward compatibility.
    fn invoke_symbol(&self, symbol: ValueToken, arg: ValueToken) -> HalResult<ValueToken>;
    fn invoke_symbol_value(&self, symbol: ValueToken, arg: RuntimeValue)
    -> HalResult<RuntimeValue>;
}

pub trait DiagnosticsHal: Send + Sync {
    fn emit(&self, code: ValueToken, payload: ValueToken) -> HalResult<ValueToken>;
    fn emit_value(&self, code: ValueToken, payload: ValueToken) -> HalResult<RuntimeValue> {
        self.emit(code, payload).map(RuntimeValue::from_legacy_i32)
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::DynLinkDescriptorView;

    #[kani::proof]
    fn dynlink_contract_accepts_canonical_non_ordinal_descriptor() {
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 1,
            declared_name: "GetTickCount",
            library: "kernel32.dll",
            alias: "GetTickCount",
            ordinal_alias: false,
            symbol: 100,
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "case-insensitive-canonical",
        };
        assert_eq!(descriptor.contract_violation(), None);
    }

    #[kani::proof]
    fn dynlink_contract_rejects_mismatched_selection_policy() {
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 2,
            declared_name: "OrdinalCall",
            library: "example.dll",
            alias: "7",
            ordinal_alias: true,
            symbol: 200,
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "case-insensitive-canonical",
        };
        assert_eq!(
            descriptor.contract_violation(),
            Some("selection_policy does not match ordinal_alias contract")
        );
    }
}
