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
    fn msg_box(&self, prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Deterministically implements `InputBox` interaction or a policy/capability error.
    fn input_box(
        &self,
        prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue>;
}

pub trait EventPumpHal: Send + Sync {
    /// Deterministically pumps host events, or reports unsupported behavior.
    fn do_events(&self) -> HalResult<RuntimeValue>;
}

pub trait FileSystemHal: Send + Sync {
    fn open(&self, path: RuntimeValue, mode: RuntimeValue) -> HalResult<RuntimeValue>;
    fn close(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn seek(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue>;
    fn eof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn lof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn free_file(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue>;
}

pub trait ProcessEnvHal: Send + Sync {
    fn shell(&self, command: RuntimeValue, window_style: RuntimeValue) -> HalResult<RuntimeValue>;
    fn environ(&self, key: RuntimeValue) -> HalResult<RuntimeValue>;
    fn dir(&self, path: RuntimeValue, attrs: RuntimeValue) -> HalResult<RuntimeValue>;
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
    fn subscribe_event(&self, object: RuntimeValue, event: RuntimeValue)
    -> HalResult<RuntimeValue>;
    fn unsubscribe_event(&self, subscription: RuntimeValue) -> HalResult<RuntimeValue>;
    fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>>;
    fn event_callback_subscription(&self, callback: RuntimeValue) -> HalResult<RuntimeValue>;
    fn event_callback_arity(&self, callback: RuntimeValue) -> HalResult<RuntimeValue>;
    fn event_callback_arg(
        &self,
        callback: RuntimeValue,
        index: RuntimeValue,
    ) -> HalResult<RuntimeValue>;
    fn release_event_callback(&self, callback: RuntimeValue) -> HalResult<RuntimeValue>;
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
    fn date_serial_now(&self) -> HalResult<RuntimeValue>;
    fn time_serial_now(&self) -> HalResult<RuntimeValue>;
    fn timer_ticks(&self) -> HalResult<RuntimeValue>;
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
    fn invoke_bound(&self, binding: ValueToken, arg: ValueToken) -> HalResult<ValueToken>;
    fn invoke_bound_value(&self, binding: ValueToken, arg: ValueToken) -> HalResult<RuntimeValue> {
        self.invoke_bound(binding, arg)
            .map(RuntimeValue::from_legacy_i32)
    }

    /// Descriptor-driven invoke path used by VM/host integrations.
    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue>;

    /// Legacy symbol-token invoke path retained for backward compatibility.
    fn invoke_symbol(&self, symbol: ValueToken, arg: RuntimeValue) -> HalResult<RuntimeValue>;
}

pub trait DiagnosticsHal: Send + Sync {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue>;
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
