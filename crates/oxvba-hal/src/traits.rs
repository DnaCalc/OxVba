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

pub trait ConsoleHal: Send + Sync {
    /// Console text output with line semantics for stdio-style hosts.
    fn print_line(&self, data: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Delimited field parsing from stdin-like input (BASIC `Input`).
    fn input_fields(&self, count: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Line-oriented read from stdin-like input (BASIC `Line Input`).
    fn line_input(&self) -> HalResult<RuntimeValue>;
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
    fn kill(&self, path: RuntimeValue) -> HalResult<RuntimeValue>;
    fn seek(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue>;
    fn eof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn lof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    fn free_file(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Binary read: reads `count` bytes from the current position (VBA `Get #`).
    fn read_bytes(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Formatted write output with delimiter semantics (current VBA `Write #` lane).
    fn write_bytes(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Formatted text output with delimiter semantics (VBA `Print #`).
    fn print_line(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Delimited field parsing from stream (VBA `Input #`).
    fn input_fields(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Line-oriented read until newline or EOF (VBA `Line Input #`).
    fn line_input(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
    /// Returns current byte position in the file (VBA `Loc()`).
    fn loc(&self, handle: RuntimeValue) -> HalResult<RuntimeValue>;
}

pub trait ProcessEnvHal: Send + Sync {
    fn shell(&self, command: RuntimeValue, window_style: RuntimeValue) -> HalResult<RuntimeValue>;
    fn environ(&self, key: RuntimeValue) -> HalResult<RuntimeValue>;
    fn dir(&self, path: RuntimeValue, attrs: RuntimeValue) -> HalResult<RuntimeValue>;
}

pub trait ComHal: Send + Sync {
    fn create_object(&self, prog_id: RuntimeValue) -> HalResult<RuntimeValue>;
    fn create_object_variant(&self, prog_id: Variant) -> HalResult<Variant> {
        let prog_id = prog_id.to_runtime_value().map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "create_object_variant",
                detail,
            )
        })?;
        let value = self.create_object(prog_id)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "create_object_variant",
                detail,
            )
        })
    }
    fn release_object(&self, object: ObjectRef) -> HalResult<RuntimeValue>;
    fn release_object_variant(&self, object: ObjectRef) -> HalResult<Variant> {
        let value = self.release_object(object)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "release_object_variant",
                detail,
            )
        })
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
    fn dispatch_invoke_variant(&self, request: &ComInvokeRequest) -> HalResult<Variant> {
        let value = self.dispatch_invoke_runtime_value_v2(request)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke_variant",
                detail,
            )
        })
    }
    fn dispatch_invoke_dynamic_variant(&self, request: &DynamicCallRequest) -> HalResult<Variant> {
        let value = self.dispatch_invoke_dynamic_runtime_value_v2(request)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke_dynamic_variant",
                detail,
            )
        })
    }
    fn subscribe_event(
        &self,
        object: ObjectRef,
        event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken>;
    fn unsubscribe_event(&self, subscription: ComSubscriptionToken) -> HalResult<RuntimeValue>;
    fn unsubscribe_event_variant(&self, subscription: ComSubscriptionToken) -> HalResult<Variant> {
        let value = self.unsubscribe_event(subscription)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "unsubscribe_event_variant",
                detail,
            )
        })
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
        callback: ComCallbackToken,
        index: usize,
    ) -> HalResult<Variant> {
        let value = self.event_callback_arg(callback, index)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "event_callback_variant",
                detail,
            )
        })
    }
    fn release_event_callback(&self, callback: ComCallbackToken) -> HalResult<RuntimeValue>;
    fn release_event_callback_variant(&self, callback: ComCallbackToken) -> HalResult<Variant> {
        let value = self.release_event_callback(callback)?;
        Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::ComActivationDispatch,
                "release_event_callback_variant",
                detail,
            )
        })
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
    fn time_serial_now(&self) -> HalResult<RuntimeValue>;
    fn timer_ticks(&self) -> HalResult<RuntimeValue>;
}

pub trait DynamicLinkHal: Send + Sync {
    /// Resolves descriptor metadata into an invocation binding token.
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<BindingHandle> {
        Ok(descriptor.symbol.raw().into())
    }
    fn bind_descriptor_value(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
    ) -> HalResult<RuntimeValue> {
        self.bind_descriptor(descriptor)
            .map(RuntimeValue::BindingHandle)
    }

    /// Optional argument normalization/writeback preparation hook.
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
        binding: BindingHandle,
        args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        let args = variants_to_runtime_values(args, "invoke_bound_variants")?;
        let (ret, writebacks) = self.invoke_bound_multi(binding, &args)?;
        runtime_result_to_variants(ret, writebacks, "invoke_bound_variants")
    }

    /// Descriptor-driven invoke path used by VM/host integrations.
    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue>;

    /// Descriptor-driven multi-arg invoke path.
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
        descriptor: &DynLinkDescriptorView<'_>,
        args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        let args = variants_to_runtime_values(args, "invoke_descriptor_variants")?;
        let (ret, writebacks) = self.invoke_descriptor_multi(descriptor, &args)?;
        runtime_result_to_variants(ret, writebacks, "invoke_descriptor_variants")
    }

    /// Legacy symbol-token invoke path retained for backward compatibility.
    fn invoke_symbol(&self, symbol: DynLinkSymbol, arg: RuntimeValue) -> HalResult<RuntimeValue>;

    /// Variant-native symbol-token invoke path retained for no-descriptor
    /// VM/JIT external call sites.
    fn invoke_symbol_variant(&self, symbol: DynLinkSymbol, arg: &Variant) -> HalResult<Variant> {
        let arg = arg.to_runtime_value().map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DynamicLinking,
                "invoke_symbol_variant",
                detail,
            )
        })?;
        let ret = self.invoke_symbol(symbol, arg)?;
        Variant::try_from_runtime_value(&ret).map_err(|detail| {
            HalError::adapter_fault(
                HalProfileId::Null,
                CapabilityId::DynamicLinking,
                "invoke_symbol_variant",
                detail,
            )
        })
    }
}

fn variants_to_runtime_values(
    args: &[Variant],
    operation: &'static str,
) -> HalResult<Vec<RuntimeValue>> {
    args.iter()
        .map(|value| {
            value.to_runtime_value().map_err(|detail| {
                HalError::adapter_fault(
                    HalProfileId::Null,
                    CapabilityId::DynamicLinking,
                    operation,
                    detail,
                )
            })
        })
        .collect()
}

fn runtime_result_to_variants(
    ret: RuntimeValue,
    writebacks: Vec<RuntimeValue>,
    operation: &'static str,
) -> HalResult<(Variant, Vec<Variant>)> {
    let ret = Variant::try_from_runtime_value(&ret).map_err(|detail| {
        HalError::adapter_fault(
            HalProfileId::Null,
            CapabilityId::DynamicLinking,
            operation,
            detail,
        )
    })?;
    let writebacks = writebacks
        .iter()
        .map(|value| {
            Variant::try_from_runtime_value(value).map_err(|detail| {
                HalError::adapter_fault(
                    HalProfileId::Null,
                    CapabilityId::DynamicLinking,
                    operation,
                    detail,
                )
            })
        })
        .collect::<HalResult<Vec<_>>>()?;
    Ok((ret, writebacks))
}

pub trait DiagnosticsHal: Send + Sync {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue>;
    fn debug_print(&self, text: RuntimeValue) -> HalResult<RuntimeValue>;
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
