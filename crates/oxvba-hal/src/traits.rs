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
    error::{HalError, HalResult},
    model::{CapabilityId, HalDescriptor, HalProfileId, HostPolicy},
    project::{
        HostExtensionModuleChange, ProjectDescriptor, ProjectReferenceDescriptor,
        ResolvedProjectReference,
    },
};
use oxvba_com::{
    ComCallbackPayload, ComCallbackToken, ComInvokeRequest, ComMemberToken, ComObjectDescriptor,
    ComSubscriptionToken, DynamicCallRequest,
};
pub use oxvba_com::{
    TypeLibCacheScope, TypeLibEventDispatchPath, TypeLibEventMetadata, TypeLibMemberInvokeKind,
    TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibParamType, TypeLibResolveRequest,
    TypeLibResolvedIdentity,
};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, ObjectRef, RuntimeValue, Variant};
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynLinkDescriptorView<'a> {
    pub descriptor_id: u32,
    pub declared_name: &'a str,
    pub library: &'a str,
    pub alias: &'a str,
    pub ordinal_alias: bool,
    pub symbol: DynLinkSymbol,
    pub marshal_lane: &'a str,
    pub calling_convention: &'a str,
    pub selection_policy: &'a str,
    pub param_count: usize,
    pub param_types: &'a [String],
    pub param_by_ref: &'a [bool],
    pub return_type: Option<Cow<'a, str>>,
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
        if self.marshal_lane != "m0-deterministic" && self.marshal_lane != "m1-native-ffi" {
            return Some("marshal_lane is not m0-deterministic or m1-native-ffi");
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
        if self.param_count != self.param_types.len() {
            return Some("param_count does not match param_types length");
        }
        if self.param_count != self.param_by_ref.len() {
            return Some("param_count does not match param_by_ref length");
        }
        None
    }
}

/// Root HAL service registry.
///
/// HAL traits below expose a value-model boundary: methods that accept or
/// return [`RuntimeValue`] are retained compatibility projection contracts for
/// older adapters, while `_variant` companions are the retained value-model
/// entry points used by VM/JIT callers. Implementations must override retained
/// companions directly; trait defaults fault rather than silently projecting
/// through compatibility `RuntimeValue` methods.
pub trait HostServices: Send + Sync {
    fn profile(&self) -> HalProfileId;
    fn descriptor(&self) -> HalDescriptor;
    fn policy(&self) -> &HostPolicy;

    fn console(&self) -> &dyn ConsoleHal;
    fn ui(&self) -> &dyn UiInteractionHal;
    fn events(&self) -> &dyn EventPumpHal;
    fn fs(&self) -> &dyn FileSystemHal;
    fn process(&self) -> &dyn ProcessEnvHal;
    fn com(&self) -> &dyn ComHal;
    fn time_locale(&self) -> &dyn TimeLocaleHal;
    fn dynlink(&self) -> &dyn DynamicLinkHal;
    fn diag(&self) -> &dyn DiagnosticsHal;
    fn project_catalog(&self) -> Option<&dyn ProjectCatalogHal> {
        None
    }
    fn project_references(&self) -> Option<&dyn ProjectReferenceHal> {
        None
    }
    fn project_mutation(&self) -> Option<&dyn ProjectMutationHal> {
        None
    }
}

fn variant_companion_not_overridden<T>(
    capability: CapabilityId,
    operation: &'static str,
) -> HalResult<T> {
    Err(HalError::adapter_fault(
        HalProfileId::Null,
        capability,
        operation,
        "retained Variant companion is not implemented by this HAL adapter",
    ))
}

pub trait ConsoleHal: Send + Sync {
    /// Console text output with line semantics for stdio-style hosts.
    fn print_line(&self, data: RuntimeValue) -> HalResult<RuntimeValue>;
    fn print_line_variant(&self, _data: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ConsoleIo, "print_line_variant")
    }
    /// Delimited field parsing from stdin-like input (BASIC `Input`).
    fn input_fields(&self, count: RuntimeValue) -> HalResult<RuntimeValue>;
    fn input_fields_variant(&self, _count: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ConsoleIo, "input_fields_variant")
    }
    /// Line-oriented read from stdin-like input (BASIC `Line Input`).
    fn line_input(&self) -> HalResult<RuntimeValue>;
    fn line_input_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ConsoleIo, "line_input_variant")
    }
}

pub trait UiInteractionHal: Send + Sync {
    /// Deterministically implements `MsgBox` interaction or a policy/capability error.
    fn msg_box(&self, prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue>;
    fn msg_box_variant(&self, _prompt: Variant, _style: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::UiInteraction, "msg_box_variant")
    }
    /// Deterministically implements `InputBox` interaction or a policy/capability error.
    fn input_box(
        &self,
        prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue>;
    fn input_box_variant(&self, _prompt: Variant, _default_value: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::UiInteraction, "input_box_variant")
    }
}

pub trait EventPumpHal: Send + Sync {
    /// Deterministically pumps host events, or reports unsupported behavior.
    fn do_events(&self) -> HalResult<RuntimeValue>;
    fn do_events_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::EventPump, "do_events_variant")
    }
}

pub trait FileSystemHal: Send + Sync {
    fn open(&self, path: RuntimeValue, mode: RuntimeValue) -> HalResult<RuntimeValue>;
    fn open_variant(&self, _path: Variant, _mode: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "open_variant")
    }
    fn close(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn close_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "close_variant")
    }
    fn kill(&self, path: RuntimeValue) -> HalResult<RuntimeValue>;
    fn kill_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "kill_variant")
    }
    fn seek(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue>;
    fn seek_variant(&self, _handle: Variant, _position: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "seek_variant")
    }
    fn eof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn eof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "eof_variant")
    }
    fn lof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn lof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "lof_variant")
    }
    fn free_file(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue>;
    fn free_file_variant(&self, _range_selector: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "free_file_variant")
    }
    /// Binary read: reads `count` bytes from the current position (VBA `Get #`).
    fn read_bytes(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue>;
    fn read_bytes_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "read_bytes_variant")
    }
    /// Formatted write output with delimiter semantics (current VBA `Write #` lane).
    fn write_bytes(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue>;
    fn write_bytes_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "write_bytes_variant")
    }
    /// Formatted text output with delimiter semantics (VBA `Print #`).
    fn print_line(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue>;
    fn print_line_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "print_line_variant")
    }
    /// Delimited field parsing from stream (VBA `Input #`).
    fn input_fields(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue>;
    fn input_fields_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "input_fields_variant")
    }
    /// Line-oriented read until newline or EOF (VBA `Line Input #`).
    fn line_input(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn line_input_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "line_input_variant")
    }
    /// Returns current byte position in the file (VBA `Loc()`).
    fn loc(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn loc_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "loc_variant")
    }
}

pub trait ProcessEnvHal: Send + Sync {
    fn shell(&self, command: RuntimeValue, window_style: RuntimeValue) -> HalResult<RuntimeValue>;
    fn shell_variant(&self, _command: Variant, _window_style: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ProcessEnv, "shell_variant")
    }
    fn environ(&self, key: RuntimeValue) -> HalResult<RuntimeValue>;
    fn environ_variant(&self, _key: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ProcessEnv, "environ_variant")
    }
    fn dir(&self, path: RuntimeValue, attrs: RuntimeValue) -> HalResult<RuntimeValue>;
    fn dir_variant(&self, _path: Variant, _attrs: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ProcessEnv, "dir_variant")
    }
}

pub trait ComHal: Send + Sync {
    fn create_object(&self, prog_id: RuntimeValue) -> HalResult<RuntimeValue>;
    fn create_object_variant(&self, _prog_id: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "create_object_variant",
        )
    }
    fn release_object(&self, object: ObjectRef) -> HalResult<RuntimeValue>;
    fn release_object_variant(&self, _object: ObjectRef) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "release_object_variant",
        )
    }
    fn describe_object(&self, object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>>;
    /// Compatibility COM invoke seam. Implementations may override the
    /// Variant-native methods below to avoid this semantic projection.
    fn dispatch_invoke_runtime_value_v2(
        &self,
        request: &ComInvokeRequest,
    ) -> HalResult<RuntimeValue>;
    fn dispatch_invoke_dynamic_runtime_value_v2(
        &self,
        request: &DynamicCallRequest,
    ) -> HalResult<RuntimeValue>;
    fn dispatch_invoke_variant(&self, _request: &ComInvokeRequest) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "dispatch_invoke_variant",
        )
    }
    fn dispatch_invoke_dynamic_variant(&self, _request: &DynamicCallRequest) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "dispatch_invoke_dynamic_variant",
        )
    }
    fn subscribe_event(
        &self,
        object: ObjectRef,
        event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken>;
    fn unsubscribe_event(&self, subscription: ComSubscriptionToken) -> HalResult<RuntimeValue>;
    fn unsubscribe_event_variant(&self, _subscription: ComSubscriptionToken) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "unsubscribe_event_variant",
        )
    }
    fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>>;
    fn event_callback_subscription(
        &self,
        callback: ComCallbackToken,
    ) -> HalResult<ComSubscriptionToken>;
    fn event_callback_arity(&self, callback: ComCallbackToken) -> HalResult<usize>;
    fn event_callback_arg(
        &self,
        callback: ComCallbackToken,
        index: usize,
    ) -> HalResult<RuntimeValue>;
    fn event_callback_variant(
        &self,
        _callback: ComCallbackToken,
        _index: usize,
    ) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "event_callback_variant",
        )
    }
    fn release_event_callback(&self, callback: ComCallbackToken) -> HalResult<RuntimeValue>;
    fn release_event_callback_variant(&self, _callback: ComCallbackToken) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "release_event_callback_variant",
        )
    }
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
    ) -> HalResult<RuntimeValue>;
}

pub trait TimeLocaleHal: Send + Sync {
    fn date_serial_now(&self) -> HalResult<RuntimeValue>;
    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::TimeLocale, "date_serial_now_variant")
    }
    fn time_serial_now(&self) -> HalResult<RuntimeValue>;
    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::TimeLocale, "time_serial_now_variant")
    }
    fn timer_ticks(&self) -> HalResult<RuntimeValue>;
    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::TimeLocale, "timer_ticks_variant")
    }
}

pub trait DynamicLinkHal: Send + Sync {
    /// Resolves descriptor metadata into an invocation binding token.
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<BindingHandle> {
        Ok(descriptor.symbol.raw().into())
    }

    /// Compatibility projection for callers that still model binding tokens as
    /// `RuntimeValue`.
    fn bind_descriptor_value(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
    ) -> HalResult<RuntimeValue> {
        self.bind_descriptor(descriptor)
            .map(RuntimeValue::BindingHandle)
    }

    /// Optional legacy argument normalization/writeback preparation hook.
    ///
    /// Variant-native invoke paths should avoid this unless they are adapting
    /// an older HAL implementation.
    fn prepare_invoke(
        &self,
        _binding: BindingHandle,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Ok(arg)
    }

    /// Invokes a previously bound symbol token (single-arg legacy path).
    fn invoke_bound(&self, binding: BindingHandle, arg: RuntimeValue) -> HalResult<RuntimeValue>;

    /// Invokes a previously bound symbol with multiple arguments.
    /// Returns `(return_value, writeback_values)` where writeback_values contains
    /// modified ByRef argument values to write back to caller slots.
    ///
    /// This remains the legacy semantic-value transport. Prefer
    /// `invoke_bound_variants` for retained VM/JIT slot values.
    fn invoke_bound_multi(
        &self,
        binding: BindingHandle,
        args: &[RuntimeValue],
    ) -> HalResult<(RuntimeValue, Vec<RuntimeValue>)> {
        let arg = args.first().cloned().unwrap_or(RuntimeValue::I32(0));
        self.invoke_bound(binding, arg).map(|rv| (rv, Vec::new()))
    }

    /// Variant-native multi-argument invoke path.
    ///
    /// This is the canonical transport for VM/JIT slots. The `RuntimeValue`
    /// multi-call remains as a compatibility projection for older HAL adapters.
    fn invoke_bound_variants(
        &self,
        _binding: BindingHandle,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        variant_companion_not_overridden(CapabilityId::DynamicLinking, "invoke_bound_variants")
    }

    /// Legacy descriptor-driven invoke path used by compatibility integrations.
    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue>;

    /// Legacy descriptor-driven multi-arg invoke path.
    ///
    /// Prefer `invoke_descriptor_variants` for retained VM/JIT slot values.
    fn invoke_descriptor_multi(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        args: &[RuntimeValue],
    ) -> HalResult<(RuntimeValue, Vec<RuntimeValue>)> {
        let arg = args.first().cloned().unwrap_or(RuntimeValue::I32(0));
        self.invoke_descriptor(descriptor, arg)
            .map(|rv| (rv, Vec::new()))
    }

    /// Descriptor-driven Variant-native multi-arg invoke path.
    fn invoke_descriptor_variants(
        &self,
        _descriptor: &DynLinkDescriptorView<'_>,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        variant_companion_not_overridden(CapabilityId::DynamicLinking, "invoke_descriptor_variants")
    }

    /// Legacy symbol-token invoke path retained for backward compatibility.
    fn invoke_symbol(&self, symbol: DynLinkSymbol, arg: RuntimeValue) -> HalResult<RuntimeValue>;

    /// Variant-native symbol-token invoke path retained for no-descriptor
    /// VM/JIT external call sites.
    fn invoke_symbol_variant(&self, _symbol: DynLinkSymbol, _arg: &Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::DynamicLinking, "invoke_symbol_variant")
    }
}

pub trait DiagnosticsHal: Send + Sync {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue>;
    fn emit_variant(&self, _code: Variant, _payload: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::DiagnosticsTelemetry, "emit_variant")
    }
    fn debug_print(&self, text: RuntimeValue) -> HalResult<RuntimeValue>;
    fn debug_print_variant(&self, _text: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::DiagnosticsTelemetry, "debug_print_variant")
    }
}

pub trait ProjectCatalogHal: Send + Sync {
    fn list_projects(&self) -> HalResult<Vec<ProjectDescriptor>>;
    fn get_project(&self, project_name: &str) -> HalResult<ProjectDescriptor>;
}

pub trait ProjectReferenceHal: Send + Sync {
    fn list_references(&self, project_name: &str) -> HalResult<Vec<ProjectReferenceDescriptor>>;
    fn resolve_reference(
        &self,
        reference: &ProjectReferenceDescriptor,
    ) -> HalResult<ResolvedProjectReference>;
}

pub trait ProjectMutationHal: Send + Sync {
    fn attach_host_extension_module(&self, change: &HostExtensionModuleChange) -> HalResult<()>;
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
            symbol: 100.into(),
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "case-insensitive-canonical",
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
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
            symbol: 200.into(),
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "case-insensitive-canonical",
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
        };
        assert_eq!(
            descriptor.contract_violation(),
            Some("selection_policy does not match ordinal_alias contract")
        );
    }
}
