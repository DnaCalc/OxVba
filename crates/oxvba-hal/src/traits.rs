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
use oxvba_runtime::{BindingHandle, DynLinkSymbol, ObjectRef, Variant};
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
/// HAL traits below expose a value-model boundary: methods accept and return
/// direct [`Variant`] carriers at VM/JIT call sites. Implementations should
/// override retained companions directly; trait defaults fault rather than
/// silently projecting through compatibility methods.
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
    fn print_line_variant(&self, _data: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ConsoleIo, "print_line_variant")
    }
    /// Delimited field parsing from stdin-like input (BASIC `Input`).
    fn input_fields_variant(&self, _count: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ConsoleIo, "input_fields_variant")
    }
    /// Line-oriented read from stdin-like input (BASIC `Line Input`).
    fn line_input_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ConsoleIo, "line_input_variant")
    }
}

pub trait UiInteractionHal: Send + Sync {
    /// Deterministically implements `MsgBox` interaction or a policy/capability error.
    fn msg_box_variant(&self, _prompt: Variant, _style: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::UiInteraction, "msg_box_variant")
    }
    /// Deterministically implements `InputBox` interaction or a policy/capability error.
    fn input_box_variant(&self, _prompt: Variant, _default_value: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::UiInteraction, "input_box_variant")
    }
}

pub trait EventPumpHal: Send + Sync {
    /// Deterministically pumps host events, or reports unsupported behavior.
    fn do_events_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::EventPump, "do_events_variant")
    }
}

pub trait FileSystemHal: Send + Sync {
    fn open_variant(&self, _path: Variant, _mode: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "open_variant")
    }
    /// `Open … Len = N` — like [`Self::open_variant`] but also records the fixed
    /// record length for Random-mode positioning. The default ignores the length
    /// (sequential/Binary modes do not use it).
    fn open_with_record_len(
        &self,
        path: Variant,
        mode: Variant,
        _record_len: Variant,
    ) -> HalResult<Variant> {
        self.open_variant(path, mode)
    }
    fn close_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "close_variant")
    }
    fn kill_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "kill_variant")
    }
    fn mkdir_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "mkdir_variant")
    }
    fn rmdir_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "rmdir_variant")
    }
    fn cur_dir_variant(&self, _drive: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "cur_dir_variant")
    }
    fn ch_dir_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "ch_dir_variant")
    }
    fn file_len_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "file_len_variant")
    }
    fn file_copy_variant(&self, _source: Variant, _dest: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "file_copy_variant")
    }
    /// File attribute bits for a path (VBA `GetAttr(path)` — `vbReadOnly`/`vbHidden`/…).
    fn get_attr_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "get_attr_variant")
    }
    /// Set a file's attribute bits (VBA `SetAttr path, attributes`).
    fn set_attr_variant(&self, _path: Variant, _attributes: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "set_attr_variant")
    }
    /// Change the current drive (VBA `ChDrive drive`).
    fn ch_drive_variant(&self, _drive: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "ch_drive_variant")
    }
    /// A file's last-modified time as a VBA `Date` serial (VBA `FileDateTime(path)`).
    fn file_date_time_variant(&self, _path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "file_date_time_variant")
    }
    fn seek_variant(&self, _handle: Variant, _position: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "seek_variant")
    }
    fn eof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "eof_variant")
    }
    fn lof_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "lof_variant")
    }
    fn free_file_variant(&self, _range_selector: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "free_file_variant")
    }
    /// Binary read: reads `count` bytes from the current position (VBA `Get #`).
    fn read_bytes_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "read_bytes_variant")
    }
    /// Formatted write output with delimiter semantics (current VBA `Write #` lane).
    fn write_bytes_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "write_bytes_variant")
    }
    /// Formatted text output with delimiter semantics (VBA `Print #`).
    fn print_line_variant(&self, _handle: Variant, _data: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "print_line_variant")
    }
    /// Delimited field parsing from stream (VBA `Input #`).
    fn input_fields_variant(&self, _handle: Variant, _count: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "input_fields_variant")
    }
    /// Line-oriented read until newline or EOF (VBA `Line Input #`).
    fn line_input_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "line_input_variant")
    }
    /// Returns current byte position in the file (VBA `Loc()`).
    fn loc_variant(&self, _handle: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "loc_variant")
    }
    /// Random/binary write of one record (VBA `Put #n, [rec], value`). An empty
    /// `position` writes at the current position; `fixed` (nonzero) marks a
    /// fixed-length-string source (written raw, with no length prefix).
    fn put_record_variant(
        &self,
        _handle: Variant,
        _position: Variant,
        _value: Variant,
        _fixed: Variant,
    ) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "put_record_variant")
    }
    /// Random/binary read of one record (VBA `Get #n, [rec], var`). An empty
    /// `position` reads at the current position; `vtype` is the target's VBA type
    /// code (a `VarType` discriminant) fixing the record size; `str_len` carries
    /// the string-length intent (> 0 = a fixed-length string of that many bytes;
    /// <= 0 = a variable string whose current length is `-str_len`, used for a
    /// Binary-mode read).
    fn get_record_variant(
        &self,
        _handle: Variant,
        _position: Variant,
        _vtype: Variant,
        _str_len: Variant,
    ) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "get_record_variant")
    }
    /// Set the output line width for a handle (VBA `Width #n, width`).
    fn width_variant(&self, _handle: Variant, _width: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "width_variant")
    }
    /// Rename/move a file (VBA `Name old As new`).
    fn name_variant(&self, _old_path: Variant, _new_path: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "name_variant")
    }
    /// Lock a byte range of an open file (VBA `Lock #n [, range]`).
    fn lock_variant(&self, _handle: Variant, _start: Variant, _end: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "lock_variant")
    }
    /// Unlock a byte range of an open file (VBA `Unlock #n [, range]`).
    fn unlock_variant(
        &self,
        _handle: Variant,
        _start: Variant,
        _end: Variant,
    ) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::FileSystemIo, "unlock_variant")
    }
}

pub trait ProcessEnvHal: Send + Sync {
    fn shell_variant(&self, _command: Variant, _window_style: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ProcessEnv, "shell_variant")
    }
    fn environ_variant(&self, _key: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ProcessEnv, "environ_variant")
    }
    fn dir_variant(&self, _path: Variant, _attrs: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ProcessEnv, "dir_variant")
    }
}

pub trait ComHal: Send + Sync {
    fn create_object_variant(&self, _prog_id: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "create_object_variant",
        )
    }
    /// VBA `GetObject([pathname], [class])`. Three modes by the shape of the arguments:
    /// an omitted `pathname` (an `Empty`/`Null` variant) binds to the currently-running
    /// instance of `class` (`GetActiveObject`); a zero-length `pathname` creates a new
    /// instance of `class` (like `CreateObject`); a non-empty `pathname` binds to the
    /// object that file names (`CoGetObject`). A failure (e.g. no running instance) should
    /// surface to VBA as error 429, but currently flattens to 5 — see the
    /// `hal-errors-flattened-to-5` gap; the HRESULT is preserved in the fault message.
    fn get_object_variant(&self, _pathname: Variant, _class: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::ComActivationDispatch, "get_object_variant")
    }
    /// Bind an existing native `IDispatch` pointer into the host COM state and
    /// associate it with a ProgID for subsequent host-root activation.
    ///
    /// # Safety
    ///
    /// `dispatch` must be null or a valid `IDispatch` pointer carrying one
    /// retained reference owned by the caller. Implementations that accept the
    /// pointer take ownership of that reference.
    unsafe fn bind_native_dispatch_object_variant(
        &self,
        _prog_id: &str,
        _dispatch: *mut core::ffi::c_void,
    ) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "bind_native_dispatch_object_variant",
        )
    }
    fn release_object_variant(&self, _object: ObjectRef) -> HalResult<Variant> {
        variant_companion_not_overridden(
            CapabilityId::ComActivationDispatch,
            "release_object_variant",
        )
    }
    fn describe_object(&self, object: ObjectRef) -> HalResult<Option<ComObjectDescriptor>>;
    /// Snapshot a foreign COM collection's elements via `IEnumVARIANT` (VBA
    /// `For Each`). Collecting every element up front is intentional: the
    /// returned `Vec` is the loop's iteration source, so a mid-loop `Exit For`
    /// works against the snapshot (no live enumerator is held). Object-valued
    /// elements are bound as `ObjectRef`s like any other COM result. Adapters
    /// without a real COM transport (null/wasm/replay) return a capability error
    /// by default, which the VM surfaces through normal VBA error handling.
    fn enumerate_object(&self, _object: ObjectRef) -> HalResult<Vec<Variant>> {
        variant_companion_not_overridden(CapabilityId::ComActivationDispatch, "enumerate_object")
    }
    /// The class/type name VBA's `TypeName` reports for a COM object — e.g.
    /// `"Dictionary"` for a `Scripting.Dictionary`. `None` when the adapter
    /// cannot determine it (no COM transport, or the object exposes no usable
    /// type identity), in which case the caller keeps the generic `"Object"`.
    fn object_type_name(&self, _object: ObjectRef) -> HalResult<Option<String>> {
        Ok(None)
    }
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
    /// Dynamic dispatch that also returns per-argument write-back values for
    /// `[out]`/`[in,out]` COM parameters — `Some(value)` for an argument the
    /// callee modified, aligned to `request.args`. The default returns no
    /// write-backs (value-only); a host with COM out-param support overrides this.
    /// The caller (vm2) applies a write-back only to a `ByRef` call-site slot.
    fn dispatch_invoke_dynamic_variant_with_writebacks(
        &self,
        request: &DynamicCallRequest,
    ) -> HalResult<(Variant, Vec<Option<Variant>>)> {
        Ok((self.dispatch_invoke_dynamic_variant(request)?, Vec::new()))
    }
    fn subscribe_event(
        &self,
        object: ObjectRef,
        event: ComMemberToken,
    ) -> HalResult<ComSubscriptionToken>;
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
    ) -> HalResult<Variant>;

    /// `(vtable_call_count, idispatch_call_count)`: how many early-bound member
    /// calls this adapter has dispatched through the COM vtable fast path versus
    /// `IDispatch::Invoke` so far. A host test reads this to prove which transport
    /// carried a given member under a `PreferVtable` policy. Adapters with no real
    /// COM transport (deterministic / null / wasm) report `(0, 0)`.
    fn com_dispatch_transport_counts(&self) -> (u64, u64) {
        (0, 0)
    }
}

pub trait TimeLocaleHal: Send + Sync {
    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::TimeLocale, "date_serial_now_variant")
    }
    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::TimeLocale, "time_serial_now_variant")
    }
    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::TimeLocale, "timer_ticks_variant")
    }
}

pub trait DynamicLinkHal: Send + Sync {
    /// Resolves descriptor metadata into an invocation binding token.
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<BindingHandle> {
        Ok(descriptor.symbol.raw().into())
    }

    /// Variant-native multi-argument invoke path.
    fn invoke_bound_variants(
        &self,
        _binding: BindingHandle,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        variant_companion_not_overridden(CapabilityId::DynamicLinking, "invoke_bound_variants")
    }

    /// Descriptor-driven Variant-native multi-arg invoke path.
    fn invoke_descriptor_variants(
        &self,
        _descriptor: &DynLinkDescriptorView<'_>,
        _args: &[Variant],
    ) -> HalResult<(Variant, Vec<Variant>)> {
        variant_companion_not_overridden(CapabilityId::DynamicLinking, "invoke_descriptor_variants")
    }

    /// Variant-native symbol-token invoke path retained for no-descriptor
    /// VM/JIT external call sites.
    fn invoke_symbol_variant(&self, _symbol: DynLinkSymbol, _arg: &Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::DynamicLinking, "invoke_symbol_variant")
    }

    /// The OS last-error code captured immediately after the most recent native
    /// `Declare` call on this adapter — what VBA's `Err.LastDllError` reads. `0` for
    /// adapters that make no real native call (deterministic / null / wasm), or before
    /// any native call has run.
    fn last_dll_error(&self) -> i32 {
        0
    }
}

pub trait DiagnosticsHal: Send + Sync {
    fn emit_variant(&self, _code: Variant, _payload: Variant) -> HalResult<Variant> {
        variant_companion_not_overridden(CapabilityId::DiagnosticsTelemetry, "emit_variant")
    }
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
