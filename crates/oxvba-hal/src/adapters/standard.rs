use crate::{
    error::{HalError, HalResult},
    model::{
        CapabilityDescriptor, CapabilityId, CapabilityMaturity, ComInvocationStrategy,
        HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy, UiVirtualizationMode,
        WasmRuntimeClass, host_backed_mode_active,
    },
    traits::{
        ComHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal, FileSystemHal,
        ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibEventDispatchPath,
        TypeLibEventMetadata, TypeLibMemberInvokeKind, TypeLibMemberMetadata, TypeLibMetadataBlob,
        TypeLibResolveRequest, TypeLibResolvedIdentity, TypeLibraryHal, UiInteractionHal,
    },
};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::VARIANT_BOOL;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::{
    CLSCTX_SERVER, CLSIDFromProgID, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF,
    DISPPARAMS, EXCEPINFO,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Variant::{
    VARIANT, VT_BOOL, VT_EMPTY, VT_I4, VT_UI4, VariantClear,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MB_OK, MSG, MessageBoxW, PM_REMOVE, PeekMessageW, TranslateMessage,
};

#[cfg(any(debug_assertions, feature = "hal_contract_checks"))]
macro_rules! hal_contract_assert {
    ($cond:expr, $($arg:tt)+) => {
        assert!(
            $cond,
            "HAL contract assertion failed: {}",
            format_args!($($arg)+)
        );
    };
}

#[cfg(not(any(debug_assertions, feature = "hal_contract_checks")))]
macro_rules! hal_contract_assert {
    ($cond:expr, $($arg:tt)+) => {
        let _ = (stringify!($cond), stringify!($($arg)+));
    };
}

const DISPATCH_INVOKE_MISSING_ARG_TOKEN: i32 = i32::MIN + 2_048;
#[cfg(target_os = "windows")]
const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";
#[cfg(target_os = "windows")]
const EXCEL_APPLICATION_PROGID: &str = "Excel.Application";

#[derive(Debug, Clone)]
pub(crate) struct StandardHostServices {
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    descriptor: HalDescriptor,
    policy: HostPolicy,
    fs_state: Arc<Mutex<FileSystemState>>,
    com_state: Arc<Mutex<ComState>>,
    typelib_state: Arc<Mutex<TypeLibraryCacheState>>,
    dynlink_bindings: Arc<Mutex<BTreeMap<u32, i32>>>,
}

impl StandardHostServices {
    pub(crate) fn new(profile: HalProfileId, policy: HostPolicy) -> Self {
        let runtime_class = policy
            .runtime_class
            .unwrap_or_else(|| HalRuntimeClass::default_for(profile, policy.wasm_runtime_class));
        Self::new_with_runtime_class(profile, runtime_class, policy)
    }

    pub(crate) fn new_with_runtime_class(
        profile: HalProfileId,
        runtime_class: HalRuntimeClass,
        policy: HostPolicy,
    ) -> Self {
        Self {
            profile,
            runtime_class,
            descriptor: descriptor_for_profile(profile, runtime_class, &policy),
            policy,
            fs_state: Arc::new(Mutex::new(FileSystemState::default())),
            com_state: Arc::new(Mutex::new(ComState::default())),
            typelib_state: Arc::new(Mutex::new(TypeLibraryCacheState::default())),
            dynlink_bindings: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub(crate) fn profile(&self) -> HalProfileId {
        self.profile
    }

    pub(crate) fn runtime_class(&self) -> HalRuntimeClass {
        self.runtime_class
    }

    pub(crate) fn descriptor(&self) -> HalDescriptor {
        self.descriptor.clone()
    }

    pub(crate) fn policy(&self) -> &HostPolicy {
        &self.policy
    }

    fn supports(&self, capability: CapabilityId) -> bool {
        self.descriptor.supports(capability)
    }

    fn unsupported(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::capability_unavailable(self.profile, capability, op)
    }

    fn denied(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::policy_denied(self.profile, capability, op)
    }

    fn fs_lock(
        &self,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<std::sync::MutexGuard<'_, FileSystemState>> {
        self.fs_state.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                op,
                "filesystem state lock poisoned",
            )
        })
    }

    fn com_lock(
        &self,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<std::sync::MutexGuard<'_, ComState>> {
        self.com_state.lock().map_err(|_| {
            HalError::adapter_fault(self.profile, capability, op, "com state lock poisoned")
        })
    }

    fn typelib_lock(
        &self,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<std::sync::MutexGuard<'_, TypeLibraryCacheState>> {
        self.typelib_state.lock().map_err(|_| {
            HalError::adapter_fault(self.profile, capability, op, "typelib state lock poisoned")
        })
    }

    fn fs_entry_mut<'a>(
        &'a self,
        state: &'a mut FileSystemState,
        handle: i32,
        op: &'static str,
    ) -> HalResult<&'a mut FileHandleState> {
        state.handles.get_mut(&handle).ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                CapabilityId::FileSystemIo,
                op,
                format!("invalid file handle: {handle}"),
            )
        })
    }

    // Contract-check scaffold: lightweight invariants now, intended to harden over time.
    // Future steps are tracked in docs/spec/HAL_CONTRACT_ASSERTION_HARDENING.md.
    fn assert_fs_invariants(&self, state: &FileSystemState, op: &'static str) {
        #[cfg(any(debug_assertions, feature = "hal_contract_checks"))]
        {
            hal_contract_assert!(
                state.handles.len() <= 511,
                "op={} tracks too many handles: {}",
                op,
                state.handles.len()
            );
            for (handle, entry) in &state.handles {
                hal_contract_assert!(
                    (1..=511).contains(handle),
                    "op={} observed out-of-range handle {}",
                    op,
                    handle
                );
                hal_contract_assert!(
                    entry.position >= 0,
                    "op={} observed negative position {} for handle {}",
                    op,
                    entry.position,
                    handle
                );
                hal_contract_assert!(
                    entry.len >= 0,
                    "op={} observed negative len {} for handle {}",
                    op,
                    entry.len,
                    handle
                );
                if !self.policy.allow_filesystem_mutation {
                    hal_contract_assert!(
                        entry.mode == 0,
                        "op={} observed mutable handle {} while mutation policy is disabled",
                        op,
                        handle
                    );
                }
            }
        }

        #[cfg(not(any(debug_assertions, feature = "hal_contract_checks")))]
        {
            let _ = (state, op);
        }
    }

    // Contract-check scaffold for COM handle-state invariants.
    fn assert_com_invariants(&self, state: &ComState, op: &'static str) {
        #[cfg(any(debug_assertions, feature = "hal_contract_checks"))]
        {
            for (handle, binding) in &state.bindings {
                hal_contract_assert!(
                    *handle >= 20_001,
                    "op={} observed out-of-range COM handle {}",
                    op,
                    handle
                );
                for (member, dispid) in &binding.member_dispids {
                    hal_contract_assert!(
                        *dispid != 0,
                        "op={} observed zero DISPID cache entry for handle {} member {}",
                        op,
                        handle,
                        member
                    );
                }
            }
            #[cfg(not(target_os = "windows"))]
            for (handle, binding) in &state.bindings {
                hal_contract_assert!(
                    binding.native_dispatch == 0,
                    "op={} non-windows binding {} unexpectedly has native dispatch pointer",
                    op,
                    handle
                );
            }
            for (subscription, entry) in &state.subscriptions {
                hal_contract_assert!(
                    *subscription >= 40_001,
                    "op={} observed out-of-range COM event subscription {}",
                    op,
                    subscription
                );
                hal_contract_assert!(
                    state.bindings.contains_key(&entry.object),
                    "op={} observed COM event subscription {} for unknown object {}",
                    op,
                    subscription,
                    entry.object
                );
                #[cfg(target_os = "windows")]
                if let ComEventSubscriptionTransport::NativeConnectionPoint(native) =
                    entry.transport
                {
                    hal_contract_assert!(
                        native.connection_point != 0,
                        "op={} observed native COM subscription {} with null connection point",
                        op,
                        subscription
                    );
                    hal_contract_assert!(
                        native.cookie != 0,
                        "op={} observed native COM subscription {} with zero cookie",
                        op,
                        subscription
                    );
                }
            }
            for callback in &state.pending_callbacks {
                hal_contract_assert!(
                    state.callbacks.contains_key(callback),
                    "op={} observed pending callback token {} without payload",
                    op,
                    callback
                );
            }
            for (callback, payload) in &state.callbacks {
                hal_contract_assert!(
                    *callback >= 60_001,
                    "op={} observed out-of-range callback token {}",
                    op,
                    callback
                );
                hal_contract_assert!(
                    state.subscriptions.contains_key(&payload.subscription),
                    "op={} observed callback for unknown subscription {}",
                    op,
                    payload.subscription
                );
                hal_contract_assert!(
                    payload.args.len() <= 32,
                    "op={} observed oversized callback args payload (len={})",
                    op,
                    payload.args.len()
                );
            }
        }

        #[cfg(not(any(debug_assertions, feature = "hal_contract_checks")))]
        {
            let _ = (state, op);
        }
    }

    fn native_mode_enabled(&self) -> bool {
        host_backed_mode_active(self.profile, &self.policy)
    }

    fn native_fs_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn native_process_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn native_time_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn native_diagnostics_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn native_com_enabled(&self) -> bool {
        self.native_mode_enabled() && self.profile == HalProfileId::Windows
    }

    fn windows_typelib_supported(&self) -> bool {
        self.profile == HalProfileId::Windows && self.supports(CapabilityId::ComActivationDispatch)
    }

    fn resolve_known_typelib_identity(
        &self,
        request: &TypeLibResolveRequest,
    ) -> Option<TypeLibResolvedIdentity> {
        let normalized_importlib = request.importlib_hint.as_deref().map(normalize_ci_token);
        let normalized_libid = request.libid_hint.as_deref().map(normalize_guid_like);

        if normalized_importlib
            .as_deref()
            .is_some_and(|value| value == "stdole2.tlb")
            || normalized_libid
                .as_deref()
                .is_some_and(|value| value == "00020430-0000-0000-c000-000000000046")
        {
            return Some(TypeLibResolvedIdentity {
                reference_name: request.reference_name.clone(),
                importlib: "stdole2.tlb".to_string(),
                libid: Some("00020430-0000-0000-C000-000000000046".to_string()),
                major_version: 2,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:stdole2:2.0:0".to_string(),
            });
        }

        if normalized_importlib
            .as_deref()
            .is_some_and(|value| value == "oxvba_testdispatch.tlb")
            || normalized_libid
                .as_deref()
                .is_some_and(|value| value == "11111111-2222-3333-4444-555555555555")
        {
            return Some(TypeLibResolvedIdentity {
                reference_name: request.reference_name.clone(),
                importlib: "oxvba_testdispatch.tlb".to_string(),
                libid: Some("11111111-2222-3333-4444-555555555555".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:oxvba-testdispatch:1.0:0".to_string(),
            });
        }

        if normalized_importlib
            .as_deref()
            .is_some_and(|value| value == "excel.exe")
            || normalized_libid
                .as_deref()
                .is_some_and(|value| value == "00020813-0000-0000-c000-000000000046")
        {
            return Some(TypeLibResolvedIdentity {
                reference_name: request.reference_name.clone(),
                importlib: "excel.exe".to_string(),
                libid: Some("00020813-0000-0000-C000-000000000046".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:excel.application:1.0:0".to_string(),
            });
        }

        None
    }

    fn build_typelib_metadata(&self, identity: &TypeLibResolvedIdentity) -> TypeLibMetadataBlob {
        let (member_name_to_token, members, events) = if identity
            .importlib
            .eq_ignore_ascii_case("oxvba_testdispatch.tlb")
            || identity.libid.as_deref().is_some_and(|libid| {
                libid.eq_ignore_ascii_case("11111111-2222-3333-4444-555555555555")
            }) {
            let members = vec![
                TypeLibMemberMetadata {
                    name: "Count".to_string(),
                    token: TEST_DISPID_COUNT,
                    requires_argument: false,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                },
                TypeLibMemberMetadata {
                    name: "Exists".to_string(),
                    token: TEST_DISPID_EXISTS,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                },
                TypeLibMemberMetadata {
                    name: "FireChanged".to_string(),
                    token: TEST_DISPID_FIRE_CHANGED,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                },
                TypeLibMemberMetadata {
                    name: "FireChangedPair".to_string(),
                    token: TEST_DISPID_FIRE_CHANGED_PAIR,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                },
                TypeLibMemberMetadata {
                    name: "FireChangedSourceInterface".to_string(),
                    token: TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                },
                TypeLibMemberMetadata {
                    name: "Ping".to_string(),
                    token: TEST_DISPID_PING,
                    requires_argument: false,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                },
                TypeLibMemberMetadata {
                    name: "Lookup".to_string(),
                    token: TEST_DISPID_LOOKUP,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                },
                TypeLibMemberMetadata {
                    name: "SetValue".to_string(),
                    token: TEST_DISPID_SET_VALUE,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
                },
                TypeLibMemberMetadata {
                    name: "SetValueRef".to_string(),
                    token: TEST_DISPID_SET_VALUE_REF,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
                },
                TypeLibMemberMetadata {
                    name: "Value".to_string(),
                    token: TEST_DISPID_VALUE,
                    requires_argument: false,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                },
            ];
            let events = vec![
                TypeLibEventMetadata {
                    name: "Changed".to_string(),
                    token: TEST_EVENT_CHANGED,
                    callback_arity: 1,
                    dispatch_path: TypeLibEventDispatchPath::Dispatch,
                    connection_point_iid: Some(IID_OXVBA_TEST_DISPATCH_EVENTS_STR.to_string()),
                    dispatch_member_id: Some(TEST_EVENT_CHANGED),
                },
                TypeLibEventMetadata {
                    name: "ChangedSourceInterface".to_string(),
                    token: TEST_EVENT_CHANGED_SOURCE_INTERFACE,
                    callback_arity: 1,
                    dispatch_path: TypeLibEventDispatchPath::SourceInterface,
                    connection_point_iid: Some(
                        IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR.to_string(),
                    ),
                    dispatch_member_id: None,
                },
                TypeLibEventMetadata {
                    name: "ChangedPair".to_string(),
                    token: TEST_EVENT_CHANGED_PAIR,
                    callback_arity: 2,
                    dispatch_path: TypeLibEventDispatchPath::Dispatch,
                    connection_point_iid: Some(IID_OXVBA_TEST_DISPATCH_EVENTS_STR.to_string()),
                    dispatch_member_id: Some(TEST_EVENT_CHANGED_PAIR),
                },
            ];
            let member_name_to_token = members
                .iter()
                .map(|entry| (entry.name.clone(), entry.token))
                .collect();
            (member_name_to_token, members, events)
        } else if identity.importlib.eq_ignore_ascii_case("excel.exe")
            || identity.libid.as_deref().is_some_and(|libid| {
                libid.eq_ignore_ascii_case("00020813-0000-0000-C000-000000000046")
            })
        {
            let members = vec![TypeLibMemberMetadata {
                name: "Quit".to_string(),
                token: TEST_DISPID_EXCEL_QUIT,
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }];
            let events = vec![TypeLibEventMetadata {
                name: "Quit".to_string(),
                token: TEST_EVENT_EXCEL_APP_QUIT,
                callback_arity: 0,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: Some(IID_EXCEL_APPLICATION_EVENTS_STR.to_string()),
                dispatch_member_id: None,
            }];
            let member_name_to_token = members
                .iter()
                .map(|entry| (entry.name.clone(), entry.token))
                .collect();
            (member_name_to_token, members, events)
        } else if identity.importlib.eq_ignore_ascii_case("scrrun.dll")
            || identity.libid.as_deref().is_some_and(|libid| {
                libid.eq_ignore_ascii_case("420B2830-E718-11CF-893D-00A0C9054228")
            })
        {
            let members = vec![
                TypeLibMemberMetadata {
                    name: "Count".to_string(),
                    token: TEST_DISPID_COUNT,
                    requires_argument: false,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                },
                TypeLibMemberMetadata {
                    name: "Exists".to_string(),
                    token: TEST_DISPID_EXISTS,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                },
            ];
            let events = vec![TypeLibEventMetadata {
                name: "Exists".to_string(),
                token: TEST_EVENT_CHANGED,
                callback_arity: 1,
                dispatch_path: TypeLibEventDispatchPath::Dispatch,
                connection_point_iid: None,
                dispatch_member_id: Some(TEST_EVENT_CHANGED),
            }];
            let member_name_to_token = members
                .iter()
                .map(|entry| (entry.name.clone(), entry.token))
                .collect();
            (member_name_to_token, members, events)
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };
        TypeLibMetadataBlob {
            identity: identity.clone(),
            member_name_to_token,
            members,
            events,
        }
    }

    #[cfg(target_os = "windows")]
    fn known_typelib_identity_for_prog_id_name(
        &self,
        prog_id_name: &str,
    ) -> Option<TypeLibResolvedIdentity> {
        if prog_id_name.eq_ignore_ascii_case("Scripting.Dictionary") {
            return Some(TypeLibResolvedIdentity {
                reference_name: "Scripting.Dictionary".to_string(),
                importlib: "scrrun.dll".to_string(),
                libid: Some("420B2830-E718-11CF-893D-00A0C9054228".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:scripting.dictionary:1.0:0".to_string(),
            });
        }
        if prog_id_name.eq_ignore_ascii_case(EXCEL_APPLICATION_PROGID) {
            return Some(TypeLibResolvedIdentity {
                reference_name: EXCEL_APPLICATION_PROGID.to_string(),
                importlib: "excel.exe".to_string(),
                libid: Some("00020813-0000-0000-C000-000000000046".to_string()),
                major_version: 1,
                minor_version: 0,
                lcid: Some(0),
                cache_key: "typelib:excel.application:1.0:0".to_string(),
            });
        }
        if !prog_id_name.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID) {
            return None;
        }
        Some(TypeLibResolvedIdentity {
            reference_name: "OxVba.TestDispatch".to_string(),
            importlib: "oxvba_testdispatch.tlb".to_string(),
            libid: Some("11111111-2222-3333-4444-555555555555".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "typelib:oxvba-testdispatch:1.0:0".to_string(),
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn known_typelib_identity_for_prog_id_name(
        &self,
        _prog_id_name: &str,
    ) -> Option<TypeLibResolvedIdentity> {
        None
    }

    #[cfg(target_os = "windows")]
    fn load_typelib_metadata_for_prog_id_name(
        &self,
        prog_id_name: &str,
    ) -> HalResult<Option<TypeLibMetadataBlob>> {
        let Some(identity) = self.known_typelib_identity_for_prog_id_name(prog_id_name) else {
            return Ok(None);
        };
        let capability = CapabilityId::ComActivationDispatch;
        let mut state = self.typelib_lock(capability, "create_object")?;
        if let Some(cached) = state.metadata.get(&identity.cache_key) {
            return Ok(Some(cached.clone()));
        }
        let blob = self.build_typelib_metadata(&identity);
        state.metadata.insert(identity.cache_key, blob.clone());
        Ok(Some(blob))
    }

    #[cfg(not(target_os = "windows"))]
    fn load_typelib_metadata_for_prog_id_name(
        &self,
        _prog_id_name: &str,
    ) -> HalResult<Option<TypeLibMetadataBlob>> {
        Ok(None)
    }

    #[cfg(target_os = "windows")]
    fn registered_event_override_for_prog_id_name(
        &self,
        prog_id_name: &str,
        op: &'static str,
    ) -> HalResult<Option<RegisteredEventOverrideConfig>> {
        let configured_prog_id = match std::env::var("OXVBA_REGISTERED_COM_PROGID") {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        if !configured_prog_id.eq_ignore_ascii_case(prog_id_name) {
            return Ok(None);
        }
        let capability = CapabilityId::ComActivationDispatch;
        let Some(event_token) =
            self.parse_registered_event_env_i32("OXVBA_REGISTERED_EVENT_TOKEN", capability, op)?
        else {
            return Ok(None);
        };
        let callback_arity_raw = self
            .parse_registered_event_env_i32("OXVBA_REGISTERED_EVENT_EXPECTED_ARGC", capability, op)?
            .unwrap_or(1);
        if callback_arity_raw < 0 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                op,
                "registered event override expected arg count must be non-negative",
            ));
        }
        let path = match std::env::var("OXVBA_REGISTERED_EVENT_PATH")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
        {
            Some(value) if value.is_empty() || value == "dispatch" => ComEventPath::Dispatch,
            Some(value) if value == "source-interface" || value == "sourceinterface" => {
                ComEventPath::SourceInterface
            }
            Some(value) => {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!(
                        "registered event override has unsupported path `{value}` (expected `dispatch` or `source-interface`)"
                    ),
                ));
            }
            None => ComEventPath::Dispatch,
        };
        let connection_point_iid = std::env::var("OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let dispatch_member_id = self.parse_registered_event_env_i32(
            "OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER",
            capability,
            op,
        )?;
        let trigger_member = self.parse_registered_event_env_i32(
            "OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER",
            capability,
            op,
        )?;
        let trigger_requires_argument = self
            .parse_registered_event_env_bool(
                "OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG",
                capability,
                op,
            )?
            .unwrap_or(true);
        let trigger_invoke_kind = self
            .parse_registered_event_env_invoke_kind(
                "OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND",
                capability,
                op,
            )?
            .unwrap_or(TypeLibMemberInvokeKind::Method);
        Ok(Some(RegisteredEventOverrideConfig {
            event_token,
            callback_arity: callback_arity_raw as usize,
            path,
            connection_point_iid,
            dispatch_member_id,
            trigger_member,
            trigger_requires_argument,
            trigger_invoke_kind,
        }))
    }

    #[cfg(target_os = "windows")]
    fn parse_registered_event_env_i32(
        &self,
        key: &str,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<Option<i32>> {
        let Some(raw) = std::env::var(key).ok() else {
            return Ok(None);
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        trimmed.parse::<i32>().map(Some).map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                op,
                format!("registered event override `{key}` must parse as i32"),
            )
        })
    }

    #[cfg(target_os = "windows")]
    fn parse_registered_event_env_bool(
        &self,
        key: &str,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<Option<bool>> {
        let Some(raw) = std::env::var(key).ok() else {
            return Ok(None);
        };
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let value = match trimmed.as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("registered event override `{key}` must parse as boolean"),
                ));
            }
        };
        Ok(Some(value))
    }

    #[cfg(target_os = "windows")]
    fn parse_registered_event_env_invoke_kind(
        &self,
        key: &str,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<Option<TypeLibMemberInvokeKind>> {
        let Some(raw) = std::env::var(key).ok() else {
            return Ok(None);
        };
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let kind = match trimmed.as_str() {
            "method" => TypeLibMemberInvokeKind::Method,
            "propertyget" | "property-get" => TypeLibMemberInvokeKind::PropertyGet,
            "propertyput" | "property-put" => TypeLibMemberInvokeKind::PropertyPut,
            "propertyputref" | "property-putref" => TypeLibMemberInvokeKind::PropertyPutRef,
            _ => {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!(
                        "registered event override `{key}` has unsupported invoke kind `{trimmed}`"
                    ),
                ));
            }
        };
        Ok(Some(kind))
    }

    #[cfg(target_os = "windows")]
    fn apply_registered_event_override_to_binding(
        &self,
        binding: &mut ComBinding,
        override_cfg: &RegisteredEventOverrideConfig,
    ) {
        binding.event_specs.insert(
            override_cfg.event_token,
            ComEventSpec {
                callback_arity: override_cfg.callback_arity,
                path: override_cfg.path,
                connection_point_iid: override_cfg.connection_point_iid.clone(),
                dispatch_member_id: override_cfg.dispatch_member_id,
            },
        );
        if let Some(trigger_member) = override_cfg.trigger_member {
            binding.direct_dispatch_specs.insert(
                trigger_member,
                ComDirectDispatchSpec {
                    invoke_kind: override_cfg.trigger_invoke_kind,
                    requires_argument: override_cfg.trigger_requires_argument,
                },
            );
            binding.event_trigger_specs.insert(
                trigger_member,
                ComEventTriggerSpec {
                    event_token: override_cfg.event_token,
                    callback_arity: override_cfg.callback_arity,
                    second_arg_is_incremented: false,
                },
            );
        }
    }

    fn host_fs_base_dir(&self) -> PathBuf {
        let mut out = std::env::temp_dir();
        out.push("oxvba_hal");
        out.push(match self.profile {
            HalProfileId::Windows => "windows",
            HalProfileId::Linux => "linux",
            HalProfileId::MacOs => "macos",
            HalProfileId::Wasm => "wasm",
            HalProfileId::Null => "null",
        });
        out
    }

    fn host_path_from_token(&self, token: i32) -> PathBuf {
        let mut path = self.host_fs_base_dir();
        path.push(format!("token_{}.dat", token.saturating_abs()));
        path
    }

    #[cfg(target_os = "windows")]
    fn spawn_probe_shell_process(&self, command: i32) -> std::io::Result<std::process::Child> {
        Command::new("cmd")
            .args(["/C", &format!("echo OXVBA_HAL_{command} > NUL")])
            .spawn()
    }

    #[cfg(not(target_os = "windows"))]
    fn spawn_probe_shell_process(&self, _command: i32) -> std::io::Result<std::process::Child> {
        Command::new("sh").arg("-c").arg("true").spawn()
    }

    #[cfg(target_os = "windows")]
    fn native_windows_msg_box(&self, prompt: i32, style: i32) -> HalResult<i32> {
        let text = format!("OxVba MsgBox token={prompt}");
        let title = "OxVba";
        let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let style_flags = if style == 0 { MB_OK } else { style as u32 };
        let result = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text_w.as_ptr(),
                title_w.as_ptr(),
                style_flags,
            )
        };
        if result <= 0 {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::UiInteraction,
                "msg_box",
                "native MessageBoxW returned failure",
            ));
        }
        Ok(result)
    }

    #[cfg(not(target_os = "windows"))]
    fn native_windows_msg_box(&self, _prompt: i32, _style: i32) -> HalResult<i32> {
        Ok(1)
    }

    #[cfg(target_os = "windows")]
    fn pump_windows_messages_once(&self) {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        unsafe {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn pump_windows_messages_once(&self) {}

    #[cfg(target_os = "windows")]
    fn ensure_thread_com_apartment(&self, operation: &'static str) -> HalResult<()> {
        THREAD_COM_APARTMENT_READY.with(|ready| {
            if ready.get() {
                return Ok(());
            }
            let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
            if hr < 0 {
                return Err(HalError::adapter_fault(
                    self.profile,
                    CapabilityId::ComActivationDispatch,
                    operation,
                    format!("CoInitializeEx failed with HRESULT {:#010X}", hr as u32),
                ));
            }
            ready.set(true);
            Ok(())
        })
    }

    #[cfg(not(target_os = "windows"))]
    fn ensure_thread_com_apartment(&self, _operation: &'static str) -> HalResult<()> {
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn resolve_native_com_progid(&self, prog_id: i32) -> Option<String> {
        if let Some(value) = self.policy.com_prog_id_overrides.get(&prog_id) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        let env_key = format!("OXVBA_COM_PROGID_{prog_id}");
        if let Ok(value) = std::env::var(env_key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        match prog_id {
            // Controlled in-process COM automation object for OxVba integration tests.
            4 => Some(OXVBA_TEST_DISPATCH_PROGID.to_string()),
            _ => None,
        }
    }

    #[cfg(target_os = "windows")]
    fn has_explicit_native_com_override(&self, prog_id: i32) -> bool {
        if self
            .policy
            .com_prog_id_overrides
            .get(&prog_id)
            .is_some_and(|value| !value.trim().is_empty())
        {
            return true;
        }
        let env_key = format!("OXVBA_COM_PROGID_{prog_id}");
        std::env::var(env_key)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    fn has_explicit_native_com_override(&self, _prog_id: i32) -> bool {
        false
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_native_com_progid(&self, _prog_id: i32) -> Option<String> {
        None
    }

    #[cfg(target_os = "windows")]
    fn native_com_activate_dispatch(&self, prog_id: &str) -> HalResult<*mut RawIDispatch> {
        if prog_id.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID)
            && !self.force_registered_test_dispatch()
        {
            return Ok(create_oxvba_test_dispatch());
        }
        let wide: Vec<u16> = prog_id.encode_utf16().chain(std::iter::once(0)).collect();
        let mut clsid = windows_sys::core::GUID {
            data1: 0,
            data2: 0,
            data3: 0,
            data4: [0; 8],
        };
        let hr = unsafe { CLSIDFromProgID(wide.as_ptr(), &mut clsid) };
        if hr < 0 {
            return Err(self.com_createobject_adapter_fault(format!(
                "CLSIDFromProgID failed for `{prog_id}` with HRESULT {:#010X}",
                hr as u32
            )));
        }

        let mut dispatch_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let hr = unsafe {
            CoCreateInstance(
                &clsid,
                std::ptr::null_mut(),
                CLSCTX_SERVER,
                &IID_IDISPATCH,
                &mut dispatch_ptr,
            )
        };
        if hr < 0 {
            return Err(self.com_createobject_adapter_fault(format!(
                "CoCreateInstance failed for `{prog_id}` with HRESULT {:#010X}",
                hr as u32
            )));
        }
        if dispatch_ptr.is_null() {
            return Err(self.com_createobject_adapter_fault(
                "CoCreateInstance returned a null IDispatch pointer".to_string(),
            ));
        }
        Ok(dispatch_ptr.cast::<RawIDispatch>())
    }

    #[cfg(target_os = "windows")]
    fn force_registered_test_dispatch(&self) -> bool {
        std::env::var("OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH")
            .ok()
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    fn com_createobject_adapter_fault(&self, message: String) -> HalError {
        let hresult = parse_hresult_hex(&message);
        let label = map_com_hresult_label(hresult, None);
        let mut suffix = String::new();
        if let Some(value) = hresult {
            suffix.push_str(&format!("hresult=0x{value:08X};"));
        }
        let prefix = if suffix.is_empty() {
            format!("com-createobject-{label}")
        } else {
            format!("com-createobject-{label};{suffix}")
        };
        HalError::adapter_fault(
            self.profile,
            CapabilityId::ComActivationDispatch,
            "create_object",
            format!("{prefix} {message}"),
        )
    }

    #[cfg(target_os = "windows")]
    fn try_native_com_binding(&self, prog_id: &str) -> HalResult<RawDispatchPtr> {
        self.ensure_thread_com_apartment("create_object")?;
        self.native_com_activate_dispatch(prog_id)
            .map(|dispatch| dispatch as RawDispatchPtr)
    }

    #[cfg(not(target_os = "windows"))]
    fn native_com_activate_dispatch(&self, _prog_id: &str) -> HalResult<*mut core::ffi::c_void> {
        Err(HalError::adapter_fault(
            self.profile,
            CapabilityId::ComActivationDispatch,
            "create_object",
            "native COM activation unavailable on this platform",
        ))
    }

    #[cfg(not(target_os = "windows"))]
    fn try_native_com_binding(&self, prog_id: &str) -> HalResult<RawDispatchPtr> {
        self.native_com_activate_dispatch(prog_id)
            .map(|dispatch| dispatch as RawDispatchPtr)
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_core(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id: &str,
        member: i32,
        arg: i32,
    ) -> HalResult<i32> {
        if let Some(spec) = com_member_spec_for_token(prog_id, member) {
            let dispid = unsafe { raw_get_dispid_by_name(dispatch, &spec.name) }
                .map_err(|message| self.com_dispatch_adapter_fault(message))?;
            return self.native_com_dispatch_invoke_with_member_spec(dispatch, dispid, &spec, arg);
        }
        unsafe { raw_dispatch_property_get_noargs(dispatch, member) }
            .map_err(|message| self.com_dispatch_adapter_fault(message))
    }

    #[cfg(target_os = "windows")]
    fn resolve_member_dispid_cached(
        &self,
        object: i32,
        dispatch: *mut RawIDispatch,
        binding: &ComBinding,
        member: i32,
        cached: Option<i32>,
    ) -> HalResult<Option<(i32, ComMemberSpec)>> {
        let Some(spec) = com_member_spec_for_binding(binding, member)
            .or_else(|| com_member_spec_for_token(&binding.prog_id_name, member))
        else {
            return Ok(None);
        };
        if let Some(dispid) = cached {
            return Ok(Some((dispid, spec)));
        }
        let dispid = unsafe { raw_get_dispid_by_name(dispatch, &spec.name) }
            .map_err(|message| self.com_dispatch_adapter_fault(message))?;
        let mut state = self.com_lock(CapabilityId::ComActivationDispatch, "dispatch_invoke")?;
        if let Some(binding) = state.bindings.get_mut(&object) {
            binding.member_dispids.insert(member, dispid);
        }
        self.assert_com_invariants(&state, "dispatch_invoke_cache_update");
        Ok(Some((dispid, spec)))
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_with_member_spec(
        &self,
        dispatch: *mut RawIDispatch,
        dispid: i32,
        spec: &ComMemberSpec,
        arg: i32,
    ) -> HalResult<i32> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        if spec.requires_argument {
            if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
                return Err(HalError::adapter_fault(
                    self.profile,
                    CapabilityId::ComActivationDispatch,
                    "dispatch_invoke",
                    "member requires argument but DispatchInvoke omitted the third argument",
                ));
            }
        } else {
            match spec.invoke_kind {
                TypeLibMemberInvokeKind::PropertyGet => {
                    return unsafe { raw_dispatch_property_get_noargs(dispatch, dispid) }
                        .map_err(|message| self.com_dispatch_adapter_fault(message));
                }
                TypeLibMemberInvokeKind::Method => {
                    return unsafe { raw_dispatch_invoke_method_noargs(dispatch, dispid) }
                        .map_err(|message| self.com_dispatch_adapter_fault(message));
                }
                TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef => {
                    return Err(HalError::adapter_fault(
                        self.profile,
                        CapabilityId::ComActivationDispatch,
                        "dispatch_invoke",
                        "member requires argument for property put/putref dispatch",
                    ));
                }
            }
        }
        match spec.invoke_kind {
            TypeLibMemberInvokeKind::PropertyGet => unsafe {
                raw_dispatch_property_get_i4(dispatch, dispid, arg)
            },
            TypeLibMemberInvokeKind::Method => unsafe {
                raw_dispatch_invoke_method_i4(dispatch, dispid, arg)
            },
            TypeLibMemberInvokeKind::PropertyPut => unsafe {
                raw_dispatch_property_put_i4(dispatch, dispid, arg)
            },
            TypeLibMemberInvokeKind::PropertyPutRef => unsafe {
                raw_dispatch_property_putref_i4(dispatch, dispid, arg)
            },
        }
        .map_err(|message| self.com_dispatch_adapter_fault(message))
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_with_direct_dispid(
        &self,
        dispatch: *mut RawIDispatch,
        dispid: i32,
        invoke_kind: TypeLibMemberInvokeKind,
        requires_argument: bool,
        arg: i32,
    ) -> HalResult<i32> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        if requires_argument {
            if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
                return Err(HalError::adapter_fault(
                    self.profile,
                    CapabilityId::ComActivationDispatch,
                    "dispatch_invoke",
                    "member requires argument but DispatchInvoke omitted the third argument",
                ));
            }
        } else {
            return match invoke_kind {
                TypeLibMemberInvokeKind::PropertyGet => unsafe {
                    raw_dispatch_property_get_noargs(dispatch, dispid)
                },
                TypeLibMemberInvokeKind::Method => unsafe {
                    raw_dispatch_invoke_method_noargs(dispatch, dispid)
                },
                TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef => {
                    Err("member requires argument for property put/putref dispatch".to_string())
                }
            }
            .map_err(|message| self.com_dispatch_adapter_fault(message));
        }
        match invoke_kind {
            TypeLibMemberInvokeKind::PropertyGet => unsafe {
                raw_dispatch_property_get_i4(dispatch, dispid, arg)
            },
            TypeLibMemberInvokeKind::Method => unsafe {
                raw_dispatch_invoke_method_i4(dispatch, dispid, arg)
            },
            TypeLibMemberInvokeKind::PropertyPut => unsafe {
                raw_dispatch_property_put_i4(dispatch, dispid, arg)
            },
            TypeLibMemberInvokeKind::PropertyPutRef => unsafe {
                raw_dispatch_property_putref_i4(dispatch, dispid, arg)
            },
        }
        .map_err(|message| self.com_dispatch_adapter_fault(message))
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_with_bound_dispatch(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id: &str,
        member: i32,
        arg: i32,
    ) -> HalResult<i32> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        self.native_com_dispatch_invoke_core(dispatch, prog_id, member, arg)
    }

    #[cfg(target_os = "windows")]
    fn try_native_com_vtable_invoke(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id: &str,
        member: i32,
        arg: i32,
    ) -> HalResult<Option<i32>> {
        if self.policy.com_invocation_strategy != ComInvocationStrategy::PreferVtable {
            return Ok(None);
        }
        if !prog_id.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID) {
            return Ok(None);
        }
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        let result = unsafe { raw_oxvba_test_dispatch_vtable_invoke(dispatch, member, arg) }
            .map_err(|message| self.com_dispatch_adapter_fault(message))?;
        Ok(result)
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke(&self, prog_id: &str, member: i32, arg: i32) -> HalResult<i32> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        let dispatch = self.native_com_activate_dispatch(prog_id)?;
        let result = self.native_com_dispatch_invoke_core(dispatch, prog_id, member, arg);
        unsafe {
            raw_release_dispatch(dispatch);
        }
        result
    }

    #[cfg(target_os = "windows")]
    fn com_dispatch_adapter_fault(&self, message: String) -> HalError {
        let hresult = parse_hresult_hex(&message);
        let arg_err = parse_arg_err(&message);
        let label = map_com_hresult_label(hresult, arg_err);
        let mut suffix = String::new();
        if let Some(value) = hresult {
            suffix.push_str(&format!("hresult=0x{value:08X};"));
        }
        if let Some(value) = arg_err {
            suffix.push_str(&format!("arg_err={value};"));
        }
        let prefix = if suffix.is_empty() {
            format!("com-dispatch-{label}")
        } else {
            format!("com-dispatch-{label};{suffix}")
        };
        HalError::adapter_fault(
            self.profile,
            CapabilityId::ComActivationDispatch,
            "dispatch_invoke",
            format!("{prefix} {message}"),
        )
    }

    #[cfg(target_os = "windows")]
    fn queue_com_event_callbacks(
        &self,
        object: i32,
        binding: &ComBinding,
        member: i32,
        arg: i32,
    ) -> HalResult<()> {
        let Some((event, args)) = com_event_callback_args_from_member_token(binding, member, arg)
        else {
            return Ok(());
        };
        let Some(expected_arity) = com_event_signature_arity_for_binding(binding, event) else {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                format!(
                    "COM-E-EVENT-CONNECTIONPOINT-MISSING: object `{}` does not expose event token {}",
                    binding.prog_id_name, event
                ),
            ));
        };
        if com_event_is_source_interface_only(binding, event) {
            if com_event_trace_enabled() {
                eprintln!(
                    "[oxvba-hal][com-event] projection-trigger skipped object={} member={} event={} reason=source-interface-native-lane",
                    object, member, event
                );
            }
            return Ok(());
        }
        if args.len() != expected_arity {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                format!(
                    "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: event token {} expected {} argument(s), queued {}",
                    event,
                    expected_arity,
                    args.len()
                ),
            ));
        }
        let mut state = self.com_lock(CapabilityId::ComActivationDispatch, "dispatch_invoke")?;
        self.assert_com_invariants(&state, "dispatch_invoke-event-pre");
        let queued = state.queue_callbacks_for_source_event(object, event, args.as_slice());
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] projection-trigger object={} member={} event={} args={:?} queued_subscriptions={}",
                object, member, event, args, queued
            );
        }
        self.assert_com_invariants(&state, "dispatch_invoke-event-post");
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn resolve_event_subscription_transport(
        &self,
        binding: &ComBinding,
        subscription: i32,
        object: i32,
        event: i32,
        expected_arity: usize,
    ) -> HalResult<ComEventSubscriptionTransport> {
        if binding.native_dispatch == 0 {
            return Ok(ComEventSubscriptionTransport::Projection);
        }
        let Some(spec) = binding.event_specs.get(&event) else {
            return Ok(ComEventSubscriptionTransport::Projection);
        };
        let Some(connection_point_iid) = spec.connection_point_iid.as_deref() else {
            if matches!(spec.path, ComEventPath::SourceInterface) {
                return Err(HalError::adapter_fault(
                    self.profile,
                    CapabilityId::ComActivationDispatch,
                    "subscribe_event",
                    "COM-E-EVENT-PATH-UNSUPPORTED: source-interface COM event callbacks (COM-EVT-B) require connection-point metadata in current lane",
                ));
            }
            return Ok(ComEventSubscriptionTransport::Projection);
        };
        let sink_mode = match spec.path {
            ComEventPath::Dispatch => ComConnectionPointSinkMode::Dispatch {
                event_dispatch_member: spec
                    .dispatch_member_id
                    .unwrap_or(COM_EVENT_DISPATCH_MEMBER_WILDCARD),
            },
            ComEventPath::SourceInterface => {
                if !source_interface_connection_point_supported(connection_point_iid) {
                    return Err(HalError::adapter_fault(
                        self.profile,
                        CapabilityId::ComActivationDispatch,
                        "subscribe_event",
                        format!(
                            "COM-E-EVENT-PATH-UNSUPPORTED: source-interface COM event callbacks (COM-EVT-B) are unsupported for connection-point IID `{connection_point_iid}` in current lane"
                        ),
                    ));
                }
                ComConnectionPointSinkMode::SourceInterface
            }
        };
        self.ensure_thread_com_apartment("subscribe_event")?;
        let dispatch = binding.native_dispatch as *mut RawIDispatch;
        let request = ComConnectionPointAdviseRequest {
            com_state: Arc::clone(&self.com_state),
            subscription,
            object,
            event_token: event,
            expected_arity,
            sink_mode,
        };
        let advised = unsafe {
            raw_try_advise_connection_point_event(dispatch, request, connection_point_iid)
        }
        .map_err(|message| {
            HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "subscribe_event",
                format!("COM-E-EVENT-ADVISE-FAILED: {message}"),
            )
        })?;
        let transport = match advised {
            Some(native) => ComEventSubscriptionTransport::NativeConnectionPoint(native),
            None => ComEventSubscriptionTransport::Projection,
        };
        if com_event_trace_enabled() {
            let event_dispatch_member = spec
                .dispatch_member_id
                .unwrap_or(COM_EVENT_DISPATCH_MEMBER_WILDCARD);
            eprintln!(
                "[oxvba-hal][com-event] resolve-transport object={} event={} iid={} dispatch_member={} resolved={}",
                object,
                event,
                connection_point_iid,
                event_dispatch_member,
                transport.kind_label()
            );
        }
        Ok(transport)
    }

    #[cfg(target_os = "windows")]
    fn release_event_subscription_transport(
        &self,
        transport: ComEventSubscriptionTransport,
    ) -> HalResult<()> {
        if let ComEventSubscriptionTransport::NativeConnectionPoint(native) = transport {
            self.ensure_thread_com_apartment("unsubscribe_event")?;
            unsafe { raw_unadvise_connection_point(native) }.map_err(|message| {
                HalError::adapter_fault(
                    self.profile,
                    CapabilityId::ComActivationDispatch,
                    "unsubscribe_event",
                    format!("COM-E-EVENT-ADVISE-FAILED: {message}"),
                )
            })?;
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn queue_com_event_callbacks(
        &self,
        _object: i32,
        _binding: &ComBinding,
        _member: i32,
        _arg: i32,
    ) -> HalResult<()> {
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn native_com_dispatch_invoke(
        &self,
        _prog_id: &str,
        _member: i32,
        _arg: i32,
    ) -> HalResult<i32> {
        Err(HalError::adapter_fault(
            self.profile,
            CapabilityId::ComActivationDispatch,
            "dispatch_invoke",
            "native COM invoke unavailable on this platform",
        ))
    }

    #[cfg(not(target_os = "windows"))]
    fn resolve_event_subscription_transport(
        &self,
        _binding: &ComBinding,
        _subscription: i32,
        _object: i32,
        _event: i32,
        _expected_arity: usize,
    ) -> HalResult<ComEventSubscriptionTransport> {
        Ok(ComEventSubscriptionTransport::Projection)
    }

    #[cfg(not(target_os = "windows"))]
    fn release_event_subscription_transport(
        &self,
        _transport: ComEventSubscriptionTransport,
    ) -> HalResult<()> {
        Ok(())
    }
}

pub(crate) fn descriptor_for_profile(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    policy: &HostPolicy,
) -> HalDescriptor {
    HalDescriptor {
        profile,
        runtime_class: runtime_class.as_str(),
        contract_version: "hal-v1",
        adapter_version: "0.1.0",
        capabilities: capability_matrix(profile, runtime_class, policy.wasm_runtime_class),
    }
}

impl UiInteractionHal for StandardHostServices {
    fn msg_box(&self, prompt: i32, style: i32) -> HalResult<i32> {
        let capability = CapabilityId::UiInteraction;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "msg_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(capability, "msg_box"));
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Windows
            && self.runtime_class() == HalRuntimeClass::WindowsGui
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            return self.native_windows_msg_box(prompt, style);
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Linux
            && self.runtime_class() == HalRuntimeClass::LinuxStdio
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            eprintln!("[oxvba-hal] linux-stdio msg_box token={prompt} style={style}");
            return Ok(style.max(1));
        }
        let result = match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "msg_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(style.max(1)),
            UiVirtualizationMode::Disabled => Ok(prompt.max(1)),
        };
        if let Ok(value) = result {
            hal_contract_assert!(
                value >= 1,
                "op=msg_box must return positive token, got {}",
                value
            );
        }
        result
    }

    fn input_box(&self, prompt: i32, default_value: i32) -> HalResult<i32> {
        let capability = CapabilityId::UiInteraction;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(capability, "input_box"));
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Linux
            && self.runtime_class() == HalRuntimeClass::LinuxStdio
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            eprintln!("[oxvba-hal] linux-stdio input_box token={prompt} default={default_value}");
            return Ok(default_value);
        }
        let result = match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "input_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::Disabled => Ok(prompt),
        };
        if let Ok(value) = result {
            match self.policy.ui_virtualization {
                UiVirtualizationMode::ScriptedResponses => {
                    hal_contract_assert!(
                        value == default_value,
                        "op=input_box scripted response mismatch: expected {}, got {}",
                        default_value,
                        value
                    );
                }
                UiVirtualizationMode::Disabled => {
                    hal_contract_assert!(
                        value == prompt,
                        "op=input_box disabled response mismatch: expected {}, got {}",
                        prompt,
                        value
                    );
                }
                UiVirtualizationMode::FailOnPrompt => {}
            }
        }
        result
    }
}

impl EventPumpHal for StandardHostServices {
    fn do_events(&self) -> HalResult<i32> {
        let capability = CapabilityId::EventPump;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "do_events"));
        }
        if self.native_mode_enabled() {
            if self.profile == HalProfileId::Windows
                && self.runtime_class() == HalRuntimeClass::WindowsGui
            {
                self.pump_windows_messages_once();
            }
            thread::yield_now();
        }
        if self.native_com_enabled() {
            let mut state = self.com_state.lock().map_err(|_| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "do_events",
                    "com state lock poisoned during event callback pump",
                )
            })?;
            self.assert_com_invariants(&state, "do_events-pre");
            if let Some(callback) = state.pending_callbacks.first().copied() {
                let _ = state.pending_callbacks.remove(0);
                if com_event_trace_enabled() {
                    eprintln!(
                        "[oxvba-hal][com-event] do-events callback={} remaining_pending={}",
                        callback,
                        state.pending_callbacks.len()
                    );
                }
                self.assert_com_invariants(&state, "do_events-post");
                return Ok(callback);
            }
            self.assert_com_invariants(&state, "do_events-post");
        }
        Ok(0)
    }
}

impl FileSystemHal for StandardHostServices {
    fn open(&self, path: i32, mode: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "open"));
        }
        if mode != 0 && !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "open"));
        }

        let mut state = self.fs_lock(capability, "open")?;
        self.assert_fs_invariants(&state, "open-pre");
        let Some(handle) = state.first_free_in(1, 511) else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "open",
                "no free file handles available in supported range",
            ));
        };
        let host_path = if self.native_fs_enabled() {
            let host_path = self.host_path_from_token(path);
            if let Some(parent) = host_path.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "open",
                        format!("failed to create host fs directory: {err}"),
                    )
                })?;
            }
            Some(host_path)
        } else {
            None
        };
        let initial_len = if let Some(host_path) = host_path.as_ref() {
            if mode != 0 {
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(host_path)
                    .map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "open",
                            format!("failed to open host path {}: {err}", host_path.display()),
                        )
                    })?;
                clamp_u64_to_i32(file.metadata().map(|meta| meta.len()).unwrap_or(0))
            } else {
                fs::metadata(host_path)
                    .map(|meta| clamp_u64_to_i32(meta.len()))
                    .unwrap_or_else(|_| pseudo_file_len_from_path_token(path))
            }
        } else if mode == 0 {
            pseudo_file_len_from_path_token(path)
        } else {
            0
        };
        state.handles.insert(
            handle,
            FileHandleState {
                mode,
                position: 0,
                len: initial_len,
                host_path,
            },
        );
        self.assert_fs_invariants(&state, "open-post");
        hal_contract_assert!(
            (1..=511).contains(&handle),
            "op=open returned out-of-range handle {}",
            handle
        );
        Ok(handle)
    }

    fn close(&self, handle: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "close"));
        }
        let mut state = self.fs_lock(capability, "close")?;
        self.assert_fs_invariants(&state, "close-pre");
        if state.handles.remove(&handle).is_some() {
            self.assert_fs_invariants(&state, "close-post");
            Ok(1)
        } else {
            Err(HalError::adapter_fault(
                self.profile,
                capability,
                "close",
                format!("invalid file handle: {handle}"),
            ))
        }
    }

    fn seek(&self, handle: i32, position: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "seek"));
        }
        if position < 0 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "seek",
                format!("negative seek position: {position}"),
            ));
        }

        let mut state = self.fs_lock(capability, "seek")?;
        self.assert_fs_invariants(&state, "seek-pre");
        let final_position = {
            let entry = self.fs_entry_mut(&mut state, handle, "seek")?;
            let prior_len = entry.len;
            let host_path = entry.host_path.clone();
            entry.position = position;
            if position > entry.len && entry.mode != 0 && self.policy.allow_filesystem_mutation {
                if let Some(host_path) = host_path.as_ref() {
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(host_path)
                        .map_err(|err| {
                            HalError::adapter_fault(
                                self.profile,
                                capability,
                                "seek",
                                format!(
                                    "failed to open host path {} for seek: {err}",
                                    host_path.display()
                                ),
                            )
                        })?;
                    file.set_len(position as u64).map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "seek",
                            format!(
                                "failed to extend host path {} to {}: {err}",
                                host_path.display(),
                                position
                            ),
                        )
                    })?;
                }
                entry.len = position;
            }
            hal_contract_assert!(
                entry.position == position,
                "op=seek did not preserve requested position {}; got {}",
                position,
                entry.position
            );
            let expected_len =
                if position > prior_len && entry.mode != 0 && self.policy.allow_filesystem_mutation
                {
                    position
                } else {
                    prior_len
                };
            hal_contract_assert!(
                entry.len == expected_len,
                "op=seek expected len {} but found {}",
                expected_len,
                entry.len
            );
            entry.position
        };
        self.assert_fs_invariants(&state, "seek-post");
        Ok(final_position)
    }

    fn eof(&self, handle: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "eof"));
        }
        let mut state = self.fs_lock(capability, "eof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "eof")?;
        Ok(if entry.position >= entry.len { 1 } else { 0 })
    }

    fn lof(&self, handle: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "lof"));
        }
        let mut state = self.fs_lock(capability, "lof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "lof")?;
        Ok(entry.len)
    }

    fn free_file(&self, range_selector: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "free_file"));
        }
        let (start, end) = if range_selector == 1 {
            (256, 511)
        } else {
            (1, 255)
        };
        let state = self.fs_lock(capability, "free_file")?;
        self.assert_fs_invariants(&state, "free_file");
        let candidate = state.first_free_in(start, end).ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "free_file",
                format!("no free file number in range {start}..={end}"),
            )
        })?;
        hal_contract_assert!(
            (start..=end).contains(&candidate),
            "op=free_file returned {} outside range {}..={}",
            candidate,
            start,
            end
        );
        Ok(candidate)
    }
}

impl ProcessEnvHal for StandardHostServices {
    fn shell(&self, command: i32, _window_style: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "shell"));
        }
        if !self.policy.allow_process_spawn {
            return Err(self.denied(capability, "shell"));
        }
        if self.native_process_enabled() && command != 0 {
            let mut child = self.spawn_probe_shell_process(command).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "shell",
                    format!("failed to spawn probe shell process: {err}"),
                )
            })?;
            let child_id = i32::try_from(child.id()).unwrap_or(i32::MAX).max(1);
            let _ = child.wait();
            return Ok(child_id);
        }
        Ok(if command == 0 { 0 } else { 1 })
    }

    fn environ(&self, key: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "environ"));
        }
        if self.native_process_enabled() {
            let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> =
                std::env::vars_os().collect();
            if vars.is_empty() {
                return Ok(0);
            }
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            let idx = (key.unsigned_abs() as usize) % vars.len();
            let value_len = vars[idx].1.to_string_lossy().len();
            return Ok(value_len.min(i32::MAX as usize) as i32);
        }
        Ok(key)
    }

    fn dir(&self, path: i32, _attrs: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dir"));
        }
        if self.native_process_enabled() {
            let target = if path == 0 {
                std::env::current_dir().map_err(|err| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "dir",
                        format!("failed to get current directory: {err}"),
                    )
                })?
            } else {
                self.host_path_from_token(path)
            };
            return Ok(match fs::read_dir(target) {
                Ok(mut entries) => {
                    if entries.next().is_some() {
                        1
                    } else {
                        0
                    }
                }
                Err(_) => 0,
            });
        }
        Ok(if path == 0 { 0 } else { 1 })
    }
}

impl ComHal for StandardHostServices {
    fn create_object(&self, prog_id: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "create_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "create_object"));
        }
        if self.native_com_enabled()
            && let Some(prog_id_name) = self.resolve_native_com_progid(prog_id)
        {
            match self.try_native_com_binding(&prog_id_name) {
                Ok(native_dispatch) => {
                    let metadata = self.load_typelib_metadata_for_prog_id_name(&prog_id_name)?;
                    #[cfg(target_os = "windows")]
                    let registered_event_override = self
                        .registered_event_override_for_prog_id_name(
                            &prog_id_name,
                            "create_object",
                        )?;
                    let mut state = self.com_lock(capability, "create_object")?;
                    let handle = state.allocate_handle();
                    let mut binding =
                        ComBinding::native(prog_id_name, native_dispatch, metadata.as_ref());
                    #[cfg(target_os = "windows")]
                    if let Some(override_cfg) = registered_event_override.as_ref() {
                        self.apply_registered_event_override_to_binding(&mut binding, override_cfg);
                    }
                    state.bindings.insert(handle, binding);
                    self.assert_com_invariants(&state, "create_object");
                    return Ok(handle);
                }
                Err(err) => {
                    if self.has_explicit_native_com_override(prog_id) {
                        return Err(err);
                    }
                }
            }
        }
        Ok(5_000i32.saturating_add(prog_id))
    }

    fn dispatch_invoke(&self, object: i32, member: i32, arg: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dispatch_invoke"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "dispatch_invoke"));
        }
        if self.native_com_enabled() {
            let (binding, cached_dispid) = {
                let state = self.com_lock(capability, "dispatch_invoke")?;
                self.assert_com_invariants(&state, "dispatch_invoke");
                let binding = state.bindings.get(&object).cloned();
                let cached_dispid = binding
                    .as_ref()
                    .and_then(|entry| entry.member_dispids.get(&member).copied());
                (binding, cached_dispid)
            };
            if let Some(binding) = binding {
                #[cfg(target_os = "windows")]
                if binding.native_dispatch != 0 {
                    let dispatch = binding.native_dispatch as *mut RawIDispatch;
                    let value = if let Some(value) = self.try_native_com_vtable_invoke(
                        dispatch,
                        &binding.prog_id_name,
                        member,
                        arg,
                    )? {
                        value
                    } else if let Some((dispid, spec)) = self.resolve_member_dispid_cached(
                        object,
                        dispatch,
                        &binding,
                        member,
                        cached_dispid,
                    )? {
                        self.native_com_dispatch_invoke_with_member_spec(
                            dispatch, dispid, &spec, arg,
                        )?
                    } else if let Some(spec) = binding.direct_dispatch_specs.get(&member).copied() {
                        self.native_com_dispatch_invoke_with_direct_dispid(
                            dispatch,
                            member,
                            spec.invoke_kind,
                            spec.requires_argument,
                            arg,
                        )?
                    } else {
                        self.native_com_dispatch_invoke_with_bound_dispatch(
                            dispatch,
                            &binding.prog_id_name,
                            member,
                            arg,
                        )?
                    };
                    self.queue_com_event_callbacks(object, &binding, member, arg)?;
                    return Ok(value);
                }
                let value = self.native_com_dispatch_invoke(&binding.prog_id_name, member, arg)?;
                self.queue_com_event_callbacks(object, &binding, member, arg)?;
                return Ok(value);
            }
        }
        let arg = if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
            0
        } else {
            arg
        };
        Ok(object.saturating_add(member).saturating_add(arg))
    }

    fn subscribe_event(&self, object: i32, event: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "subscribe_event"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "subscribe_event"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "subscribe_event",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event subscription requires host-backed Windows native mode",
            ));
        }
        let (binding, expected_arity, subscription) = {
            let mut state = self.com_lock(capability, "subscribe_event")?;
            self.assert_com_invariants(&state, "subscribe_event-pre");
            let Some(binding) = state.bindings.get(&object).cloned() else {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "subscribe_event",
                    format!(
                        "COM-E-EVENT-CONNECTIONPOINT-MISSING: unknown COM object token {object}"
                    ),
                ));
            };
            let Some(expected_arity) = com_event_signature_arity_for_binding(&binding, event)
            else {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "subscribe_event",
                    format!(
                        "COM-E-EVENT-CONNECTIONPOINT-MISSING: object `{}` does not expose event token {}",
                        binding.prog_id_name, event
                    ),
                ));
            };
            let subscription = state.allocate_subscription();
            (binding, expected_arity, subscription)
        };
        let transport = self.resolve_event_subscription_transport(
            &binding,
            subscription,
            object,
            event,
            expected_arity,
        )?;
        let mut state = self.com_lock(capability, "subscribe_event")?;
        self.assert_com_invariants(&state, "subscribe_event-pre-insert");
        if !state.bindings.contains_key(&object) {
            if let Err(err) = self.release_event_subscription_transport(transport) {
                eprintln!(
                    "[oxvba-hal] failed to release abandoned COM event transport for object {object}: {}",
                    err.message
                );
            }
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "subscribe_event",
                format!("COM-E-EVENT-CONNECTIONPOINT-MISSING: unknown COM object token {object}"),
            ));
        }
        state.subscriptions.insert(
            subscription,
            ComEventSubscription {
                object,
                event,
                transport,
            },
        );
        #[cfg(target_os = "windows")]
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] subscribe object={} event={} subscription={} transport={} arity={}",
                object,
                event,
                subscription,
                transport.kind_label(),
                expected_arity
            );
        }
        self.assert_com_invariants(&state, "subscribe_event-post");
        Ok(subscription)
    }

    fn unsubscribe_event(&self, subscription: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "unsubscribe_event"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "unsubscribe_event"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "unsubscribe_event",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event subscription requires host-backed Windows native mode",
            ));
        }
        let transport = {
            let state = self.com_lock(capability, "unsubscribe_event")?;
            self.assert_com_invariants(&state, "unsubscribe_event-pre");
            let Some(entry) = state.subscriptions.get(&subscription) else {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "unsubscribe_event",
                    format!(
                        "COM-E-EVENT-ADVISE-FAILED: unknown COM event subscription token {subscription}"
                    ),
                ));
            };
            entry.transport
        };
        self.release_event_subscription_transport(transport)?;
        let mut state = self.com_lock(capability, "unsubscribe_event")?;
        self.assert_com_invariants(&state, "unsubscribe_event-pre-remove");
        let Some(entry) = state.subscriptions.remove(&subscription) else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "unsubscribe_event",
                format!(
                    "COM-E-EVENT-ADVISE-FAILED: unknown COM event subscription token {subscription}"
                ),
            ));
        };
        let stale_callbacks: BTreeSet<i32> = state
            .callbacks
            .iter()
            .filter_map(|(callback, payload)| {
                if payload.subscription == subscription && payload.object == entry.object {
                    Some(*callback)
                } else {
                    None
                }
            })
            .collect();
        for callback in &stale_callbacks {
            state.callbacks.remove(callback);
        }
        state
            .pending_callbacks
            .retain(|callback| !stale_callbacks.contains(callback));
        self.assert_com_invariants(&state, "unsubscribe_event-post-remove");
        Ok(1)
    }

    fn event_callback_subscription(&self, callback: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "event_callback_subscription"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "event_callback_subscription"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_subscription",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback lookup requires host-backed Windows native mode",
            ));
        }
        let state = self.com_lock(capability, "event_callback_subscription")?;
        self.assert_com_invariants(&state, "event_callback_subscription");
        let Some(payload) = state.callbacks.get(&callback) else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_subscription",
                format!("COM-E-EVENT-CALLBACK-MISSING: unknown callback token {callback}"),
            ));
        };
        Ok(payload.subscription)
    }

    fn event_callback_arity(&self, callback: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "event_callback_arity"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "event_callback_arity"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arity",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback lookup requires host-backed Windows native mode",
            ));
        }
        let state = self.com_lock(capability, "event_callback_arity")?;
        self.assert_com_invariants(&state, "event_callback_arity");
        let Some(payload) = state.callbacks.get(&callback) else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arity",
                format!("COM-E-EVENT-CALLBACK-MISSING: unknown callback token {callback}"),
            ));
        };
        i32::try_from(payload.args.len()).map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arity",
                format!(
                    "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: callback arity {} exceeds deterministic token range",
                    payload.args.len()
                ),
            )
        })
    }

    fn event_callback_arg(&self, callback: i32, index: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "event_callback_arg"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "event_callback_arg"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arg",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback lookup requires host-backed Windows native mode",
            ));
        }
        if index < 0 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arg",
                format!(
                    "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: callback argument index {} is unsupported in current lane",
                    index
                ),
            ));
        }
        let state = self.com_lock(capability, "event_callback_arg")?;
        self.assert_com_invariants(&state, "event_callback_arg");
        let Some(payload) = state.callbacks.get(&callback) else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arg",
                format!("COM-E-EVENT-CALLBACK-MISSING: unknown callback token {callback}"),
            ));
        };
        let idx = index as usize;
        let Some(value) = payload.args.get(idx).copied() else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arg",
                format!(
                    "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: callback argument index {} exceeds callback arity {}",
                    index,
                    payload.args.len()
                ),
            ));
        };
        Ok(value)
    }

    fn release_event_callback(&self, callback: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "release_event_callback"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "release_event_callback"));
        }
        if !self.native_com_enabled() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "release_event_callback",
                "COM-E-EVENT-PATH-UNSUPPORTED: native COM event callback release requires host-backed Windows native mode",
            ));
        }
        let mut state = self.com_lock(capability, "release_event_callback")?;
        self.assert_com_invariants(&state, "release_event_callback-pre");
        if state.callbacks.remove(&callback).is_none() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "release_event_callback",
                format!("COM-E-EVENT-CALLBACK-MISSING: unknown callback token {callback}"),
            ));
        }
        state.pending_callbacks.retain(|token| *token != callback);
        self.assert_com_invariants(&state, "release_event_callback-post");
        Ok(1)
    }
}

impl TypeLibraryHal for StandardHostServices {
    fn resolve_typelib_reference(
        &self,
        request: &TypeLibResolveRequest,
    ) -> HalResult<TypeLibResolvedIdentity> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.windows_typelib_supported() {
            return Err(self.unsupported(capability, "resolve_typelib_reference"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "resolve_typelib_reference"));
        }
        let Some(identity) = self.resolve_known_typelib_identity(request) else {
            let request_key = request
                .importlib_hint
                .as_deref()
                .or(request.libid_hint.as_deref())
                .unwrap_or("<missing-identity>");
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "resolve_typelib_reference",
                format!("no deterministic typelib identity mapping for `{request_key}`"),
            ));
        };
        Ok(identity)
    }

    fn load_typelib_metadata(
        &self,
        identity: &TypeLibResolvedIdentity,
    ) -> HalResult<TypeLibMetadataBlob> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.windows_typelib_supported() {
            return Err(self.unsupported(capability, "load_typelib_metadata"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "load_typelib_metadata"));
        }
        let mut state = self.typelib_lock(capability, "load_typelib_metadata")?;
        if let Some(cached) = state.metadata.get(&identity.cache_key) {
            return Ok(cached.clone());
        }
        let blob = self.build_typelib_metadata(identity);
        state
            .metadata
            .insert(identity.cache_key.clone(), blob.clone());
        Ok(blob)
    }

    fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.windows_typelib_supported() {
            return Err(self.unsupported(capability, "invalidate_typelib_cache"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "invalidate_typelib_cache"));
        }
        let mut state = self.typelib_lock(capability, "invalidate_typelib_cache")?;
        let removed = match scope {
            TypeLibCacheScope::Global => {
                let count = state.metadata.len();
                state.metadata.clear();
                count
            }
            TypeLibCacheScope::Reference => {
                let Some(reference_name) = reference_name else {
                    return Err(HalError::adapter_fault(
                        self.profile,
                        capability,
                        "invalidate_typelib_cache",
                        "reference scope requires reference_name",
                    ));
                };
                let key = normalize_ci_token(reference_name);
                let before = state.metadata.len();
                state
                    .metadata
                    .retain(|_, blob| normalize_ci_token(&blob.identity.reference_name) != key);
                before.saturating_sub(state.metadata.len())
            }
        };
        Ok(i32::try_from(removed).unwrap_or(i32::MAX))
    }
}

impl TimeLocaleHal for StandardHostServices {
    fn date_serial_now(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "date_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok(clamp_u64_to_i32(now.as_secs() / 86_400));
        }
        Ok(20_260_301)
    }

    fn time_serial_now(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "time_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok((now.as_secs() % 86_400) as i32);
        }
        Ok(123_456)
    }

    fn timer_ticks(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "timer_ticks"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let modulo = i32::MAX as u128;
            return Ok((now.as_millis() % modulo) as i32);
        }
        Ok(42)
    }
}

impl DynamicLinkHal for StandardHostServices {
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<i32> {
        let capability = CapabilityId::DynamicLinking;
        const LANE_M0: &str = "m0-deterministic";
        const CONV_PLATFORM_DEFAULT: &str = "platform-default";
        const POLICY_CASE_INSENSITIVE: &str = "case-insensitive-canonical";
        const POLICY_ORDINAL_LITERAL: &str = "ordinal-literal-canonical";

        if !self.supports(capability) {
            return Err(self.unsupported(capability, "invoke_symbol"));
        }
        if !self.policy.allow_dynamic_link {
            return Err(self.denied(capability, "invoke_symbol"));
        }

        if descriptor.marshal_lane != LANE_M0 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "unsupported marshaling lane `{}` for descriptor {}",
                    descriptor.marshal_lane, descriptor.descriptor_id
                ),
            ));
        }
        if descriptor.calling_convention != CONV_PLATFORM_DEFAULT {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "unsupported calling convention `{}` for descriptor {}",
                    descriptor.calling_convention, descriptor.descriptor_id
                ),
            ));
        }
        let expected_selection_policy = if descriptor.ordinal_alias {
            POLICY_ORDINAL_LITERAL
        } else {
            POLICY_CASE_INSENSITIVE
        };
        let legacy_symbol_mode = descriptor.selection_policy == "legacy-symbol";
        if !legacy_symbol_mode && descriptor.selection_policy != expected_selection_policy {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "unsupported selection policy `{}` for descriptor {} (expected `{}`)",
                    descriptor.selection_policy,
                    descriptor.descriptor_id,
                    expected_selection_policy
                ),
            ));
        }
        if descriptor.declared_name.trim().is_empty() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "descriptor {} has empty declared_name",
                    descriptor.descriptor_id
                ),
            ));
        }
        if descriptor.library.trim().is_empty() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!("descriptor {} has empty library", descriptor.descriptor_id),
            ));
        }
        if descriptor.alias.trim().is_empty() {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!("descriptor {} has empty alias", descriptor.descriptor_id),
            ));
        }
        if descriptor.ordinal_alias {
            let ordinal_digits = descriptor.alias.strip_prefix('#').ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "ordinal alias descriptor {} must start with `#`",
                        descriptor.descriptor_id
                    ),
                )
            })?;
            if ordinal_digits.is_empty() || !ordinal_digits.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "ordinal alias descriptor {} must contain decimal digits after `#`",
                        descriptor.descriptor_id
                    ),
                ));
            }
        }
        if legacy_symbol_mode
            && !(descriptor.declared_name == "<legacy>"
                && descriptor.library == "<legacy>"
                && descriptor.alias == "<legacy>")
        {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                format!(
                    "legacy selection policy is only valid for legacy descriptors (id={})",
                    descriptor.descriptor_id
                ),
            ));
        }

        let binding = descriptor.symbol;
        let mut table = self.dynlink_bindings.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                "dynlink binding table lock poisoned",
            )
        })?;
        if let Some(existing) = table.get(&descriptor.descriptor_id).copied() {
            if existing != binding {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "descriptor {} binding mismatch: existing={}, new={}",
                        descriptor.descriptor_id, existing, binding
                    ),
                ));
            }
            return Ok(existing);
        }
        table.insert(descriptor.descriptor_id, binding);
        Ok(binding)
    }

    fn prepare_invoke(&self, _binding: i32, arg: i32) -> HalResult<i32> {
        Ok(arg)
    }

    fn invoke_bound(&self, binding: i32, arg: i32) -> HalResult<i32> {
        let capability = CapabilityId::DynamicLinking;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "invoke_symbol"));
        }
        if !self.policy.allow_dynamic_link {
            return Err(self.denied(capability, "invoke_symbol"));
        }
        if self.native_mode_enabled()
            && matches!(self.profile, HalProfileId::Windows | HalProfileId::Linux)
        {
            return match binding {
                s if s == external_symbol_token("host", "ping", "hostping") => {
                    Ok(arg.saturating_add(1))
                }
                s if s == external_symbol_token("host", "double", "hostdouble") => {
                    Ok(arg.saturating_mul(2))
                }
                _ => Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!("symbol token {binding} not resolved in host-backed lane"),
                )),
            };
        }
        Ok(binding.saturating_add(arg))
    }

    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: i32,
    ) -> HalResult<i32> {
        let binding = self.bind_descriptor(descriptor)?;
        let prepared = self.prepare_invoke(binding, arg)?;
        self.invoke_bound(binding, prepared)
    }

    fn invoke_symbol(&self, symbol: i32, arg: i32) -> HalResult<i32> {
        let descriptor = DynLinkDescriptorView {
            descriptor_id: symbol as u32,
            declared_name: "<legacy>",
            library: "<legacy>",
            alias: "<legacy>",
            ordinal_alias: false,
            symbol,
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "legacy-symbol",
        };
        self.invoke_descriptor(&descriptor, arg)
    }
}

impl DiagnosticsHal for StandardHostServices {
    fn emit(&self, code: i32, payload: i32) -> HalResult<i32> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "emit"));
        }
        if self.native_diagnostics_enabled() {
            eprintln!(
                "[oxvba-hal] profile={:?} code={} payload={}",
                self.profile, code, payload
            );
        }
        Ok(code.saturating_add(payload))
    }
}

fn capability_matrix(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    wasm_runtime_class: WasmRuntimeClass,
) -> Vec<CapabilityDescriptor> {
    use CapabilityId as C;
    use CapabilityMaturity as M;
    let mut out = Vec::new();
    let mut push = |id: C, supported: bool, maturity: M, spec_anchor: &'static str| {
        out.push(CapabilityDescriptor {
            id,
            supported,
            maturity,
            spec_anchor,
        });
    };

    match profile {
        HalProfileId::Windows => {
            push(
                C::UiInteraction,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Provisional, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(
                C::ComActivationDispatch,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0325",
            );
            push(
                C::TimeLocale,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Provisional, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
        HalProfileId::Linux => {
            push(
                C::UiInteraction,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Experimental, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
        HalProfileId::MacOs => {
            push(
                C::UiInteraction,
                true,
                M::Stub,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Stub, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Stub, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
        HalProfileId::Wasm => match runtime_class {
            HalRuntimeClass::WasmWasiLocal => {
                push(
                    C::UiInteraction,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0337",
                );
                push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                push(
                    C::TimeLocale,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0252",
                );
                push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            }
            HalRuntimeClass::WasmBrowserSandbox => {
                push(
                    C::UiInteraction,
                    false,
                    M::Stable,
                    "MS-VBAL:MsgBox/InputBox",
                );
                push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                push(
                    C::TimeLocale,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0252",
                );
                push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            }
            _ => match wasm_runtime_class {
                WasmRuntimeClass::Wasi => {
                    push(
                        C::UiInteraction,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0337",
                    );
                    push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                    push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                    push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                    push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                    push(
                        C::TimeLocale,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0252",
                    );
                    push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                    push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                }
                WasmRuntimeClass::BrowserSandbox => {
                    push(
                        C::UiInteraction,
                        false,
                        M::Stable,
                        "MS-VBAL:MsgBox/InputBox",
                    );
                    push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                    push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                    push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                    push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                    push(
                        C::TimeLocale,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0252",
                    );
                    push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                    push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                }
            },
        },
        HalProfileId::Null => {
            push(
                C::UiInteraction,
                false,
                M::Stable,
                "MS-VBAL:MsgBox/InputBox",
            );
            push(C::EventPump, false, M::Stable, "MS-VBAL:DoEvents");
            push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
            push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell/Dir/Environ");
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Stable,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
    }
    out
}

#[derive(Debug, Default)]
struct FileSystemState {
    handles: BTreeMap<i32, FileHandleState>,
}

impl FileSystemState {
    fn first_free_in(&self, start: i32, end: i32) -> Option<i32> {
        let in_use: BTreeSet<i32> = self.handles.keys().copied().collect();
        (start..=end).find(|candidate| !in_use.contains(candidate))
    }
}

#[derive(Debug, Clone)]
struct FileHandleState {
    mode: i32,
    position: i32,
    len: i32,
    host_path: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct ComState {
    next_handle: i32,
    next_subscription: i32,
    next_callback: i32,
    bindings: BTreeMap<i32, ComBinding>,
    subscriptions: BTreeMap<i32, ComEventSubscription>,
    callbacks: BTreeMap<i32, ComEventCallback>,
    pending_callbacks: Vec<i32>,
}

impl ComState {
    fn allocate_handle(&mut self) -> i32 {
        self.next_handle = self.next_handle.saturating_add(1).max(1);
        20_000i32.saturating_add(self.next_handle)
    }

    fn allocate_subscription(&mut self) -> i32 {
        self.next_subscription = self.next_subscription.saturating_add(1).max(1);
        40_000i32.saturating_add(self.next_subscription)
    }

    fn allocate_callback(&mut self) -> i32 {
        self.next_callback = self.next_callback.saturating_add(1).max(1);
        60_000i32.saturating_add(self.next_callback)
    }

    fn queue_callback_for_subscription(&mut self, subscription: i32, args: &[i32]) -> bool {
        let Some(entry) = self.subscriptions.get(&subscription).cloned() else {
            return false;
        };
        let callback = self.allocate_callback();
        self.callbacks.insert(
            callback,
            ComEventCallback {
                subscription,
                object: entry.object,
                event: entry.event,
                args: args.to_vec(),
            },
        );
        self.pending_callbacks.push(callback);
        true
    }

    fn queue_callbacks_for_source_event(&mut self, object: i32, event: i32, args: &[i32]) -> usize {
        let targets: Vec<i32> = self
            .subscriptions
            .iter()
            .filter_map(|(subscription, entry)| {
                if entry.object == object && entry.event == event && entry.transport.is_projection()
                {
                    Some(*subscription)
                } else {
                    None
                }
            })
            .collect();
        for subscription in &targets {
            let _ = self.queue_callback_for_subscription(*subscription, args);
        }
        targets.len()
    }
}

#[derive(Debug, Default)]
struct TypeLibraryCacheState {
    metadata: BTreeMap<String, TypeLibMetadataBlob>,
}

type RawDispatchPtr = usize;
#[cfg(target_os = "windows")]
type RawUnknownPtr = usize;

#[cfg(target_os = "windows")]
thread_local! {
    static THREAD_COM_APARTMENT_READY: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, Clone)]
struct ComBinding {
    prog_id_name: String,
    native_dispatch: RawDispatchPtr,
    member_dispids: BTreeMap<i32, i32>,
    member_specs: BTreeMap<i32, ComMemberSpec>,
    direct_dispatch_specs: BTreeMap<i32, ComDirectDispatchSpec>,
    event_specs: BTreeMap<i32, ComEventSpec>,
    event_trigger_specs: BTreeMap<i32, ComEventTriggerSpec>,
}

impl ComBinding {
    fn native(
        prog_id_name: String,
        native_dispatch: RawDispatchPtr,
        metadata: Option<&TypeLibMetadataBlob>,
    ) -> Self {
        Self {
            prog_id_name,
            native_dispatch,
            member_dispids: BTreeMap::new(),
            member_specs: metadata
                .map(com_member_specs_from_typelib_metadata)
                .unwrap_or_default(),
            direct_dispatch_specs: BTreeMap::new(),
            event_specs: metadata
                .map(com_event_specs_from_typelib_metadata)
                .unwrap_or_default(),
            event_trigger_specs: metadata
                .map(com_event_trigger_specs_from_typelib_metadata)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComEventSubscription {
    object: i32,
    event: i32,
    transport: ComEventSubscriptionTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComEventSubscriptionTransport {
    Projection,
    #[cfg(target_os = "windows")]
    NativeConnectionPoint(ComNativeConnectionPointTransport),
}

impl ComEventSubscriptionTransport {
    const fn is_projection(self) -> bool {
        matches!(self, Self::Projection)
    }

    const fn kind_label(self) -> &'static str {
        match self {
            Self::Projection => "projection",
            #[cfg(target_os = "windows")]
            Self::NativeConnectionPoint(_) => "native-connection-point",
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComNativeConnectionPointTransport {
    connection_point: usize,
    cookie: u32,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComConnectionPointSinkMode {
    Dispatch { event_dispatch_member: i32 },
    SourceInterface,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct ComConnectionPointAdviseRequest {
    com_state: Arc<Mutex<ComState>>,
    subscription: i32,
    object: i32,
    event_token: i32,
    expected_arity: usize,
    sink_mode: ComConnectionPointSinkMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComEventCallback {
    subscription: i32,
    object: i32,
    event: i32,
    args: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComEventPath {
    Dispatch,
    SourceInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComEventSpec {
    callback_arity: usize,
    path: ComEventPath,
    connection_point_iid: Option<String>,
    dispatch_member_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComMemberSpec {
    name: String,
    requires_argument: bool,
    invoke_kind: TypeLibMemberInvokeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComDirectDispatchSpec {
    requires_argument: bool,
    invoke_kind: TypeLibMemberInvokeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComEventTriggerSpec {
    event_token: i32,
    callback_arity: usize,
    second_arg_is_incremented: bool,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisteredEventOverrideConfig {
    event_token: i32,
    callback_arity: usize,
    path: ComEventPath,
    connection_point_iid: Option<String>,
    dispatch_member_id: Option<i32>,
    trigger_member: Option<i32>,
    trigger_requires_argument: bool,
    trigger_invoke_kind: TypeLibMemberInvokeKind,
}

#[cfg(target_os = "windows")]
fn com_member_spec_for_token(prog_id: &str, member: i32) -> Option<ComMemberSpec> {
    if prog_id.eq_ignore_ascii_case(EXCEL_APPLICATION_PROGID) {
        return match member {
            TEST_DISPID_EXCEL_QUIT => Some(ComMemberSpec {
                name: "Quit".to_string(),
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }),
            _ => None,
        };
    }
    if prog_id.eq_ignore_ascii_case("Scripting.Dictionary") {
        return match member {
            1 => Some(ComMemberSpec {
                name: "Count".to_string(),
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            }),
            2 => Some(ComMemberSpec {
                name: "Exists".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }),
            _ => None,
        };
    }
    if prog_id.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID) {
        return match member {
            1 => Some(ComMemberSpec {
                name: "Count".to_string(),
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            }),
            2 => Some(ComMemberSpec {
                name: "Exists".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }),
            3 => Some(ComMemberSpec {
                name: "FireChanged".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }),
            4 => Some(ComMemberSpec {
                name: "FireChangedPair".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }),
            11 => Some(ComMemberSpec {
                name: "FireChangedSourceInterface".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }),
            5 => Some(ComMemberSpec {
                name: "Ping".to_string(),
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::Method,
            }),
            6 => Some(ComMemberSpec {
                name: "Lookup".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            }),
            7 => Some(ComMemberSpec {
                name: "SetValue".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPut,
            }),
            8 => Some(ComMemberSpec {
                name: "SetValueRef".to_string(),
                requires_argument: true,
                invoke_kind: TypeLibMemberInvokeKind::PropertyPutRef,
            }),
            9 => Some(ComMemberSpec {
                name: "Value".to_string(),
                requires_argument: false,
                invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
            }),
            _ => None,
        };
    }
    None
}

#[cfg(target_os = "windows")]
fn com_event_signature_arity_for_binding(binding: &ComBinding, event: i32) -> Option<usize> {
    binding
        .event_specs
        .get(&event)
        .map(|spec| spec.callback_arity)
}

#[cfg(target_os = "windows")]
fn com_event_is_source_interface_only(binding: &ComBinding, event: i32) -> bool {
    matches!(
        binding.event_specs.get(&event),
        Some(ComEventSpec {
            path: ComEventPath::SourceInterface,
            ..
        })
    )
}

#[cfg(target_os = "windows")]
fn source_interface_connection_point_supported(connection_point_iid: &str) -> bool {
    connection_point_iid.eq_ignore_ascii_case(IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR)
}

#[cfg(not(target_os = "windows"))]
fn com_event_signature_arity_for_binding(_binding: &ComBinding, _event: i32) -> Option<usize> {
    None
}

#[cfg(not(target_os = "windows"))]
fn com_event_is_source_interface_only(_binding: &ComBinding, _event: i32) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn com_event_callback_args_from_member_token(
    binding: &ComBinding,
    member: i32,
    arg: i32,
) -> Option<(i32, Vec<i32>)> {
    let spec = binding.event_trigger_specs.get(&member)?;
    let args = match spec.callback_arity {
        0 => Vec::new(),
        1 => vec![arg],
        2 if spec.second_arg_is_incremented => vec![arg, arg.saturating_add(1)],
        n => vec![arg; n],
    };
    Some((spec.event_token, args))
}

fn com_event_trigger_specs_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> BTreeMap<i32, ComEventTriggerSpec> {
    let events_by_name: BTreeMap<String, &TypeLibEventMetadata> = blob
        .events
        .iter()
        .map(|event| (normalize_ci_token(&event.name), event))
        .collect();
    let mut out = BTreeMap::new();
    for member in &blob.members {
        let normalized_member_name = normalize_ci_token(&member.name);
        let event_name = normalized_member_name
            .strip_prefix("fire")
            .or_else(|| normalized_member_name.strip_prefix("raise"))
            .unwrap_or(normalized_member_name.as_str());
        let Some(event) = events_by_name.get(event_name) else {
            continue;
        };
        out.insert(
            member.token,
            ComEventTriggerSpec {
                event_token: event.token,
                callback_arity: usize::from(event.callback_arity),
                second_arg_is_incremented: normalized_member_name.ends_with("pair"),
            },
        );
    }
    out
}

fn com_event_specs_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> BTreeMap<i32, ComEventSpec> {
    blob.events
        .iter()
        .map(|event| {
            (
                event.token,
                ComEventSpec {
                    callback_arity: usize::from(event.callback_arity),
                    path: match event.dispatch_path {
                        TypeLibEventDispatchPath::Dispatch => ComEventPath::Dispatch,
                        TypeLibEventDispatchPath::SourceInterface => ComEventPath::SourceInterface,
                    },
                    connection_point_iid: event.connection_point_iid.clone(),
                    dispatch_member_id: event.dispatch_member_id,
                },
            )
        })
        .collect()
}

fn com_member_specs_from_typelib_metadata(
    blob: &TypeLibMetadataBlob,
) -> BTreeMap<i32, ComMemberSpec> {
    blob.members
        .iter()
        .map(|member| {
            (
                member.token,
                ComMemberSpec {
                    name: member.name.clone(),
                    requires_argument: member.requires_argument,
                    invoke_kind: member.invoke_kind,
                },
            )
        })
        .collect()
}

fn com_member_spec_for_binding(binding: &ComBinding, member: i32) -> Option<ComMemberSpec> {
    binding.member_specs.get(&member).cloned()
}

#[cfg(not(target_os = "windows"))]
fn com_event_callback_args_from_member_token(
    _binding: &ComBinding,
    _member: i32,
    _arg: i32,
) -> Option<(i32, Vec<i32>)> {
    None
}

#[cfg(target_os = "windows")]
impl Drop for ComState {
    fn drop(&mut self) {
        for subscription in self.subscriptions.values() {
            if let ComEventSubscriptionTransport::NativeConnectionPoint(native) =
                subscription.transport
            {
                unsafe {
                    let _ = raw_unadvise_connection_point(native);
                }
            }
        }
        self.subscriptions.clear();
        self.callbacks.clear();
        self.pending_callbacks.clear();
        for binding in self.bindings.values_mut() {
            if binding.native_dispatch != 0 {
                unsafe {
                    raw_release_dispatch(binding.native_dispatch as *mut RawIDispatch);
                }
                binding.native_dispatch = 0;
            }
        }
        self.bindings.clear();
    }
}

#[cfg(target_os = "windows")]
fn parse_hresult_hex(message: &str) -> Option<u32> {
    let marker = "HRESULT 0x";
    let offset = message.find(marker)?;
    let start = offset + marker.len();
    let end = start.saturating_add(8).min(message.len());
    let hex = message.get(start..end)?;
    if hex.len() != 8 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

#[cfg(target_os = "windows")]
fn com_event_trace_enabled() -> bool {
    std::env::var("OXVBA_COM_EVENT_TRACE")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn parse_arg_err(message: &str) -> Option<u32> {
    let marker = "arg_err=";
    let offset = message.find(marker)?;
    let start = offset + marker.len();
    let tail = message.get(start..)?;
    let digits: String = tail.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u32>().ok()
}

#[cfg(target_os = "windows")]
fn map_com_hresult_label(hresult: Option<u32>, arg_err: Option<u32>) -> &'static str {
    if arg_err.is_some() {
        return "arg-error";
    }
    match hresult {
        Some(0x8004_0154) => "class-not-registered",
        Some(0x8004_01F3) => "invalid-class-string",
        Some(0x8002_0003) => "member-not-found",
        Some(0x8002_0005) => "type-mismatch",
        Some(0x8002_0009) => "exception-raised",
        Some(0x8007_0057) => "invalid-argument",
        Some(_) => "native-failure",
        None => "fault-unspecified",
    }
}

#[cfg(target_os = "windows")]
const IID_IDISPATCH: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x0002_0400,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[cfg(target_os = "windows")]
const IID_NULL: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};

#[cfg(target_os = "windows")]
const IID_IUNKNOWN: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x0000_0000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[cfg(target_os = "windows")]
const IID_ICONNECTIONPOINTCONTAINER: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0xB196_B284,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};

#[cfg(target_os = "windows")]
const IID_ICONNECTIONPOINT: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0xB196_B286,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};

#[cfg(target_os = "windows")]
const IID_OXVBA_TEST_DISPATCH_EVENTS: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x1111_1112,
    data2: 0x2222,
    data3: 0x3333,
    data4: [0x44, 0x44, 0x55, 0x55, 0x55, 0x55, 0x55, 0x56],
};
#[cfg(target_os = "windows")]
const IID_OXVBA_TEST_DISPATCH_EVENTS_STR: &str = "11111112-2222-3333-4444-555555555556";
#[cfg(target_os = "windows")]
const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS: windows_sys::core::GUID = windows_sys::core::GUID {
    data1: 0x1111_1113,
    data2: 0x2222,
    data3: 0x3333,
    data4: [0x44, 0x44, 0x55, 0x55, 0x55, 0x55, 0x55, 0x57],
};
#[cfg(target_os = "windows")]
const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR: &str = "11111113-2222-3333-4444-555555555557";
#[cfg(target_os = "windows")]
const IID_EXCEL_APPLICATION_EVENTS_STR: &str = "00024413-0000-0000-C000-000000000046";

#[cfg(target_os = "windows")]
const COM_S_OK: i32 = 0;
#[cfg(target_os = "windows")]
const COM_E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
#[cfg(target_os = "windows")]
const COM_E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
#[cfg(target_os = "windows")]
const COM_E_INVALIDARG: i32 = 0x8007_0057u32 as i32;
#[cfg(target_os = "windows")]
const COM_DISP_E_MEMBERNOTFOUND: i32 = 0x8002_0003u32 as i32;
#[cfg(target_os = "windows")]
const COM_DISP_E_UNKNOWNNAME: i32 = 0x8002_0006u32 as i32;
#[cfg(target_os = "windows")]
const COM_DISP_E_BADPARAMCOUNT: i32 = 0x8002_000Eu32 as i32;
#[cfg(target_os = "windows")]
const COM_DISP_E_TYPEMISMATCH: i32 = 0x8002_0005u32 as i32;
#[cfg(target_os = "windows")]
const COM_DISPID_PROPERTYPUT: i32 = -3;
#[cfg(target_os = "windows")]
const COM_CONNECT_E_NOCONNECTION: i32 = 0x8004_0004u32 as i32;
#[cfg(target_os = "windows")]
const COM_CONNECT_E_CANNOTCONNECT: i32 = 0x8004_0002u32 as i32;
const TEST_DISPID_COUNT: i32 = 1;
const TEST_DISPID_EXISTS: i32 = 2;
const TEST_DISPID_FIRE_CHANGED: i32 = 3;
const TEST_DISPID_FIRE_CHANGED_PAIR: i32 = 4;
const TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE: i32 = 11;
const TEST_DISPID_PING: i32 = 5;
const TEST_DISPID_LOOKUP: i32 = 6;
const TEST_DISPID_SET_VALUE: i32 = 7;
const TEST_DISPID_SET_VALUE_REF: i32 = 8;
const TEST_DISPID_VALUE: i32 = 9;
const TEST_DISPID_EXCEL_QUIT: i32 = 10;
const TEST_EVENT_CHANGED: i32 = 1;
const TEST_EVENT_CHANGED_SOURCE_INTERFACE: i32 = 2;
const TEST_EVENT_CHANGED_PAIR: i32 = 3;
const TEST_EVENT_EXCEL_APP_QUIT: i32 = 10;
const COM_EVENT_DISPATCH_MEMBER_WILDCARD: i32 = i32::MIN + 3_333;

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIUnknownVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIUnknown {
    vtbl: *const RawIUnknownVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIDispatchVtbl {
    unknown: RawIUnknownVtbl,
    get_type_info_count:
        unsafe extern "system" fn(this: *mut core::ffi::c_void, pctinfo: *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        itinfo: u32,
        lcid: u32,
        pptinfo: *mut *mut core::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        rgsznames: *mut *mut u16,
        cnames: u32,
        lcid: u32,
        rgdispid: *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        dispidmember: i32,
        riid: *const windows_sys::core::GUID,
        lcid: u32,
        wflags: u16,
        pparams: *mut DISPPARAMS,
        pvarresult: *mut VARIANT,
        pexcepinfo: *mut EXCEPINFO,
        puargerr: *mut u32,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIDispatch {
    vtbl: *const RawIDispatchVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIConnectionPointContainerVtbl {
    unknown: RawIUnknownVtbl,
    enum_connection_points: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pp_enum: *mut *mut core::ffi::c_void,
    ) -> i32,
    find_connection_point: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const windows_sys::core::GUID,
        pp_cp: *mut *mut core::ffi::c_void,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIConnectionPointContainer {
    vtbl: *const RawIConnectionPointContainerVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIConnectionPointVtbl {
    unknown: RawIUnknownVtbl,
    get_connection_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        p_iid: *mut windows_sys::core::GUID,
    ) -> i32,
    get_connection_point_container: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pp_cpc: *mut *mut core::ffi::c_void,
    ) -> i32,
    advise: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        p_unk_sink: *mut core::ffi::c_void,
        pdw_cookie: *mut u32,
    ) -> i32,
    unadvise: unsafe extern "system" fn(this: *mut core::ffi::c_void, dw_cookie: u32) -> i32,
    enum_connections: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        pp_enum: *mut *mut core::ffi::c_void,
    ) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawIConnectionPoint {
    vtbl: *const RawIConnectionPointVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestDispatchInterface {
    vtbl: *const RawIDispatchVtbl,
    owner: *mut OxvbaTestDispatchObject,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestConnectionPointContainerInterface {
    vtbl: *const RawIConnectionPointContainerVtbl,
    owner: *mut OxvbaTestDispatchObject,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaTestConnectionPointInterface {
    vtbl: *const RawIConnectionPointVtbl,
    owner: *mut OxvbaTestDispatchObject,
    kind: OxvbaTestConnectionPointKind,
}

#[cfg(target_os = "windows")]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OxvbaTestConnectionPointKind {
    Dispatch = 1,
    SourceInterface = 2,
}

#[cfg(target_os = "windows")]
struct OxvbaTestDispatchObject {
    dispatch: OxvbaTestDispatchInterface,
    connection_point_container: OxvbaTestConnectionPointContainerInterface,
    dispatch_connection_point: OxvbaTestConnectionPointInterface,
    source_connection_point: OxvbaTestConnectionPointInterface,
    value_state: AtomicI32,
    ref_count: AtomicU32,
    next_cookie: AtomicU32,
    dispatch_sinks: Mutex<BTreeMap<u32, RawDispatchPtr>>,
    source_interface_sinks: Mutex<BTreeMap<u32, RawUnknownPtr>>,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaComEventSink {
    dispatch: RawIDispatch,
    ref_count: AtomicU32,
    com_state: Arc<Mutex<ComState>>,
    subscription: i32,
    object: i32,
    event_token: i32,
    event_dispatch_member: i32,
    expected_arity: usize,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawOxvbaTestDispatchSourceEventsVtbl {
    unknown: RawIUnknownVtbl,
    changed: unsafe extern "system" fn(this: *mut core::ffi::c_void, value: i32) -> i32,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct RawOxvbaTestDispatchSourceEvents {
    vtbl: *const RawOxvbaTestDispatchSourceEventsVtbl,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct OxvbaComEventSourceInterfaceSink {
    source: RawOxvbaTestDispatchSourceEvents,
    ref_count: AtomicU32,
    com_state: Arc<Mutex<ComState>>,
    subscription: i32,
    object: i32,
    event_token: i32,
    expected_arity: usize,
}

#[cfg(target_os = "windows")]
static OXVBA_TEST_DISPATCH_VTBL: RawIDispatchVtbl = RawIDispatchVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_test_query_interface,
        add_ref: oxvba_test_add_ref,
        release: oxvba_test_release,
    },
    get_type_info_count: oxvba_test_get_type_info_count,
    get_type_info: oxvba_test_get_type_info,
    get_ids_of_names: oxvba_test_get_ids_of_names,
    invoke: oxvba_test_invoke,
};

#[cfg(target_os = "windows")]
static OXVBA_TEST_CONNECTIONPOINTCONTAINER_VTBL: RawIConnectionPointContainerVtbl =
    RawIConnectionPointContainerVtbl {
        unknown: RawIUnknownVtbl {
            query_interface: oxvba_test_connection_point_container_query_interface,
            add_ref: oxvba_test_connection_point_container_add_ref,
            release: oxvba_test_connection_point_container_release,
        },
        enum_connection_points: oxvba_test_enum_connection_points,
        find_connection_point: oxvba_test_find_connection_point,
    };

#[cfg(target_os = "windows")]
static OXVBA_TEST_CONNECTIONPOINT_VTBL: RawIConnectionPointVtbl = RawIConnectionPointVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_test_connection_point_query_interface,
        add_ref: oxvba_test_connection_point_add_ref,
        release: oxvba_test_connection_point_release,
    },
    get_connection_interface: oxvba_test_get_connection_interface,
    get_connection_point_container: oxvba_test_get_connection_point_container,
    advise: oxvba_test_advise,
    unadvise: oxvba_test_unadvise,
    enum_connections: oxvba_test_enum_connections,
};

#[cfg(target_os = "windows")]
static OXVBA_COM_EVENT_SINK_VTBL: RawIDispatchVtbl = RawIDispatchVtbl {
    unknown: RawIUnknownVtbl {
        query_interface: oxvba_event_sink_query_interface,
        add_ref: oxvba_event_sink_add_ref,
        release: oxvba_event_sink_release,
    },
    get_type_info_count: oxvba_event_sink_get_type_info_count,
    get_type_info: oxvba_event_sink_get_type_info,
    get_ids_of_names: oxvba_event_sink_get_ids_of_names,
    invoke: oxvba_event_sink_invoke,
};

#[cfg(target_os = "windows")]
static OXVBA_COM_EVENT_SOURCE_INTERFACE_SINK_VTBL: RawOxvbaTestDispatchSourceEventsVtbl =
    RawOxvbaTestDispatchSourceEventsVtbl {
        unknown: RawIUnknownVtbl {
            query_interface: oxvba_event_source_interface_sink_query_interface,
            add_ref: oxvba_event_source_interface_sink_add_ref,
            release: oxvba_event_source_interface_sink_release,
        },
        changed: oxvba_event_source_interface_sink_changed,
    };

#[cfg(target_os = "windows")]
fn create_oxvba_test_dispatch() -> *mut RawIDispatch {
    let mut object = Box::new(OxvbaTestDispatchObject {
        dispatch: OxvbaTestDispatchInterface {
            vtbl: &OXVBA_TEST_DISPATCH_VTBL,
            owner: std::ptr::null_mut(),
        },
        connection_point_container: OxvbaTestConnectionPointContainerInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINTCONTAINER_VTBL,
            owner: std::ptr::null_mut(),
        },
        dispatch_connection_point: OxvbaTestConnectionPointInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINT_VTBL,
            owner: std::ptr::null_mut(),
            kind: OxvbaTestConnectionPointKind::Dispatch,
        },
        source_connection_point: OxvbaTestConnectionPointInterface {
            vtbl: &OXVBA_TEST_CONNECTIONPOINT_VTBL,
            owner: std::ptr::null_mut(),
            kind: OxvbaTestConnectionPointKind::SourceInterface,
        },
        value_state: AtomicI32::new(0),
        ref_count: AtomicU32::new(1),
        next_cookie: AtomicU32::new(0),
        dispatch_sinks: Mutex::new(BTreeMap::new()),
        source_interface_sinks: Mutex::new(BTreeMap::new()),
    });
    let object_ptr: *mut OxvbaTestDispatchObject = &mut *object;
    object.dispatch.owner = object_ptr;
    object.connection_point_container.owner = object_ptr;
    object.dispatch_connection_point.owner = object_ptr;
    object.source_connection_point.owner = object_ptr;
    let dispatch_ptr =
        (&mut object.dispatch as *mut OxvbaTestDispatchInterface).cast::<RawIDispatch>();
    let _ = Box::into_raw(object);
    dispatch_ptr
}

#[cfg(target_os = "windows")]
fn create_oxvba_com_event_sink(
    com_state: Arc<Mutex<ComState>>,
    subscription: i32,
    object: i32,
    event_token: i32,
    event_dispatch_member: i32,
    expected_arity: usize,
) -> *mut core::ffi::c_void {
    let sink = Box::new(OxvbaComEventSink {
        dispatch: RawIDispatch {
            vtbl: &OXVBA_COM_EVENT_SINK_VTBL,
        },
        ref_count: AtomicU32::new(1),
        com_state,
        subscription,
        object,
        event_token,
        event_dispatch_member,
        expected_arity,
    });
    Box::into_raw(sink).cast::<core::ffi::c_void>()
}

#[cfg(target_os = "windows")]
fn create_oxvba_com_event_source_interface_sink(
    com_state: Arc<Mutex<ComState>>,
    subscription: i32,
    object: i32,
    event_token: i32,
    expected_arity: usize,
) -> *mut core::ffi::c_void {
    let sink = Box::new(OxvbaComEventSourceInterfaceSink {
        source: RawOxvbaTestDispatchSourceEvents {
            vtbl: &OXVBA_COM_EVENT_SOURCE_INTERFACE_SINK_VTBL,
        },
        ref_count: AtomicU32::new(1),
        com_state,
        subscription,
        object,
        event_token,
        expected_arity,
    });
    Box::into_raw(sink).cast::<core::ffi::c_void>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn guid_equals(lhs: *const windows_sys::core::GUID, rhs: &windows_sys::core::GUID) -> bool {
    if lhs.is_null() {
        return false;
    }
    let lhs = &*lhs;
    lhs.data1 == rhs.data1
        && lhs.data2 == rhs.data2
        && lhs.data3 == rhs.data3
        && lhs.data4 == rhs.data4
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_utf16_z(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len = len.saturating_add(1);
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    String::from_utf16(slice).ok()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_dispatch(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestDispatchInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_connection_point_container(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestConnectionPointContainerInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_test_dispatch_owner_from_connection_point(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaTestDispatchObject {
    let iface = this.cast::<OxvbaTestConnectionPointInterface>();
    (*iface).owner
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_connection_point_kind(
    this: *mut core::ffi::c_void,
) -> OxvbaTestConnectionPointKind {
    let iface = this.cast::<OxvbaTestConnectionPointInterface>();
    (*iface).kind
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_com_event_sink(this: *mut core::ffi::c_void) -> *mut OxvbaComEventSink {
    this.cast::<OxvbaComEventSink>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn as_oxvba_com_event_source_interface_sink(
    this: *mut core::ffi::c_void,
) -> *mut OxvbaComEventSourceInterfaceSink {
    this.cast::<OxvbaComEventSourceInterfaceSink>()
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_add_ref(owner: *mut OxvbaTestDispatchObject) -> u32 {
    (*owner).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_release(owner: *mut OxvbaTestDispatchObject) -> u32 {
    let prev = (*owner).ref_count.fetch_sub(1, Ordering::AcqRel);
    let next = prev.saturating_sub(1);
    if next == 0 {
        let mut dispatch_sinks = match (*owner).dispatch_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let retained_dispatch: Vec<RawDispatchPtr> = dispatch_sinks.values().copied().collect();
        dispatch_sinks.clear();
        drop(dispatch_sinks);
        for sink in retained_dispatch {
            raw_release_dispatch(sink as *mut RawIDispatch);
        }
        let mut source_interface_sinks = match (*owner).source_interface_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let retained_source: Vec<RawUnknownPtr> =
            source_interface_sinks.values().copied().collect();
        source_interface_sinks.clear();
        drop(source_interface_sinks);
        for sink in retained_source {
            raw_release_unknown(sink as *mut core::ffi::c_void);
        }
        drop(Box::from_raw(owner));
    }
    next
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_owner_query_interface(
    owner: *mut OxvbaTestDispatchObject,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IDISPATCH) {
        *ppv = (&mut (*owner).dispatch as *mut OxvbaTestDispatchInterface).cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINTCONTAINER) {
        *ppv = (&mut (*owner).connection_point_container
            as *mut OxvbaTestConnectionPointContainerInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINT) {
        *ppv = (&mut (*owner).dispatch_connection_point as *mut OxvbaTestConnectionPointInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_fire_event(
    owner: *mut OxvbaTestDispatchObject,
    dispid: i32,
    args: &[i32],
) -> i32 {
    let sinks: Vec<RawDispatchPtr> = {
        let sinks = match (*owner).dispatch_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        sinks.values().copied().collect()
    };
    for sink in sinks {
        let dispatch = sink as *mut RawIDispatch;
        let _ = raw_dispatch_invoke_event(dispatch, dispid, args);
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn oxvba_test_fire_source_interface_event(
    owner: *mut OxvbaTestDispatchObject,
    value: i32,
) -> i32 {
    let sinks: Vec<RawUnknownPtr> = {
        let sinks = match (*owner).source_interface_sinks.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        sinks.values().copied().collect()
    };
    for sink in sinks {
        let source = sink as *mut RawOxvbaTestDispatchSourceEvents;
        if source.is_null() {
            continue;
        }
        let _ = ((*(*source).vtbl).changed)(source.cast(), value);
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_event(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    args: &[i32],
) -> Result<(), String> {
    if dispatch.is_null() {
        return Err("event sink dispatch pointer is null".to_string());
    }
    let mut variants: Vec<VARIANT> = vec![std::mem::zeroed(); args.len()];
    for (idx, value) in args.iter().enumerate() {
        let slot = args.len().saturating_sub(1).saturating_sub(idx);
        variants[slot].Anonymous.Anonymous.vt = VT_I4;
        variants[slot].Anonymous.Anonymous.Anonymous.lVal = *value;
    }
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: if variants.is_empty() {
            std::ptr::null_mut()
        } else {
            variants.as_mut_ptr()
        },
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: u32::try_from(variants.len()).unwrap_or(u32::MAX),
        cNamedArgs: 0,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_METHOD,
        &mut params,
        std::ptr::null_mut(),
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(event dispid={dispid}) failed with HRESULT {:#010X} (arg_err={arg_err})",
            hr as u32
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_token_from_invoke_arg(
    variant: *const VARIANT,
    arg_index: usize,
) -> Result<i32, i32> {
    if variant.is_null() {
        return Err(COM_DISP_E_TYPEMISMATCH);
    }
    match (*variant).Anonymous.Anonymous.vt {
        VT_I4 => Ok((*variant).Anonymous.Anonymous.Anonymous.lVal),
        VT_UI4 => Ok((*variant).Anonymous.Anonymous.Anonymous.ulVal as i32),
        VT_BOOL => Ok(if (*variant).Anonymous.Anonymous.Anonymous.boolVal == 0 {
            0
        } else {
            1
        }),
        VT_EMPTY if arg_index == 0 => Ok(0),
        _ => Err(COM_DISP_E_TYPEMISMATCH),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_property_put_i4_from_params(
    pparams: *mut DISPPARAMS,
    puargerr: *mut u32,
) -> Result<i32, i32> {
    if pparams.is_null() {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let params = &*pparams;
    if params.cArgs != 1
        || params.cNamedArgs != 1
        || params.rgvarg.is_null()
        || params.rgdispidNamedArgs.is_null()
        || *params.rgdispidNamedArgs != COM_DISPID_PROPERTYPUT
    {
        return Err(COM_DISP_E_BADPARAMCOUNT);
    }
    let arg = &*params.rgvarg;
    match raw_variant_token_from_invoke_arg(arg, 0) {
        Ok(value) => Ok(value),
        Err(hr) => {
            if !puargerr.is_null() {
                *puargerr = 0;
            }
            Err(hr)
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_i32(value: i32, result: *mut VARIANT) {
    if result.is_null() {
        return;
    }
    (*result).Anonymous.Anonymous.vt = VT_I4;
    (*result).Anonymous.Anonymous.Anonymous.lVal = value;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_bool(value: bool, result: *mut VARIANT) {
    if result.is_null() {
        return;
    }
    (*result).Anonymous.Anonymous.vt = VT_BOOL;
    (*result).Anonymous.Anonymous.Anonymous.boolVal = if value { -1 } else { 0 };
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_query_interface(owner, riid, ppv)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_query_interface(owner, riid, ppv)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_add_ref(
    this: *mut core::ffi::c_void,
) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_container_release(
    this: *mut core::ffi::c_void,
) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_enum_connection_points(
    _this: *mut core::ffi::c_void,
    _pp_enum: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_find_connection_point(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    pp_cp: *mut *mut core::ffi::c_void,
) -> i32 {
    if pp_cp.is_null() {
        return COM_E_INVALIDARG;
    }
    *pp_cp = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point_container(this);
    if guid_equals(riid, &IID_OXVBA_TEST_DISPATCH_EVENTS) {
        *pp_cp = (&mut (*owner).dispatch_connection_point
            as *mut OxvbaTestConnectionPointInterface)
            .cast();
    } else if guid_equals(riid, &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS) {
        *pp_cp = (&mut (*owner).source_connection_point as *mut OxvbaTestConnectionPointInterface)
            .cast();
    } else {
        return COM_CONNECT_E_NOCONNECTION;
    }
    let _ = oxvba_test_owner_add_ref(owner);
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_ICONNECTIONPOINT) {
        *ppv = this;
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    if guid_equals(riid, &IID_ICONNECTIONPOINTCONTAINER) {
        *ppv = (&mut (*owner).connection_point_container
            as *mut OxvbaTestConnectionPointContainerInterface)
            .cast();
        let _ = oxvba_test_owner_add_ref(owner);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    oxvba_test_owner_add_ref(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_connection_point_release(this: *mut core::ffi::c_void) -> u32 {
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    oxvba_test_owner_release(owner)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_connection_interface(
    this: *mut core::ffi::c_void,
    p_iid: *mut windows_sys::core::GUID,
) -> i32 {
    if p_iid.is_null() {
        return COM_E_INVALIDARG;
    }
    *p_iid = match oxvba_test_connection_point_kind(this) {
        OxvbaTestConnectionPointKind::Dispatch => IID_OXVBA_TEST_DISPATCH_EVENTS,
        OxvbaTestConnectionPointKind::SourceInterface => IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS,
    };
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_connection_point_container(
    this: *mut core::ffi::c_void,
    pp_cpc: *mut *mut core::ffi::c_void,
) -> i32 {
    if pp_cpc.is_null() {
        return COM_E_INVALIDARG;
    }
    *pp_cpc = std::ptr::null_mut();
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    *pp_cpc = (&mut (*owner).connection_point_container
        as *mut OxvbaTestConnectionPointContainerInterface)
        .cast();
    let _ = oxvba_test_owner_add_ref(owner);
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_advise(
    this: *mut core::ffi::c_void,
    p_unk_sink: *mut core::ffi::c_void,
    pdw_cookie: *mut u32,
) -> i32 {
    if p_unk_sink.is_null() || pdw_cookie.is_null() {
        return COM_E_INVALIDARG;
    }
    *pdw_cookie = 0;
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    let kind = oxvba_test_connection_point_kind(this);
    let mut sink_interface: *mut core::ffi::c_void = std::ptr::null_mut();
    let unknown = p_unk_sink.cast::<RawIUnknown>();
    let expected_iid = match kind {
        OxvbaTestConnectionPointKind::Dispatch => &IID_IDISPATCH,
        OxvbaTestConnectionPointKind::SourceInterface => &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS,
    };
    let hr = ((*(*unknown).vtbl).query_interface)(p_unk_sink, expected_iid, &mut sink_interface);
    if hr < 0 || sink_interface.is_null() {
        return COM_CONNECT_E_CANNOTCONNECT;
    }
    let mut cookie = (*owner)
        .next_cookie
        .fetch_add(1, Ordering::AcqRel)
        .saturating_add(1);
    if cookie == 0 {
        cookie = 1;
    }
    match kind {
        OxvbaTestConnectionPointKind::Dispatch => {
            let mut sinks = match (*owner).dispatch_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            while sinks.contains_key(&cookie) {
                cookie = cookie.saturating_add(1).max(1);
            }
            sinks.insert(cookie, sink_interface as RawDispatchPtr);
        }
        OxvbaTestConnectionPointKind::SourceInterface => {
            let mut sinks = match (*owner).source_interface_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            while sinks.contains_key(&cookie) {
                cookie = cookie.saturating_add(1).max(1);
            }
            sinks.insert(cookie, sink_interface as RawUnknownPtr);
        }
    }
    *pdw_cookie = cookie;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_unadvise(this: *mut core::ffi::c_void, dw_cookie: u32) -> i32 {
    if dw_cookie == 0 {
        return COM_CONNECT_E_NOCONNECTION;
    }
    let owner = as_oxvba_test_dispatch_owner_from_connection_point(this);
    let kind = oxvba_test_connection_point_kind(this);
    let sink = match kind {
        OxvbaTestConnectionPointKind::Dispatch => {
            let mut sinks = match (*owner).dispatch_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            sinks.remove(&dw_cookie).map(|sink| sink as RawUnknownPtr)
        }
        OxvbaTestConnectionPointKind::SourceInterface => {
            let mut sinks = match (*owner).source_interface_sinks.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            sinks.remove(&dw_cookie)
        }
    };
    let Some(sink) = sink else {
        return COM_CONNECT_E_NOCONNECTION;
    };
    raw_release_unknown(sink as *mut core::ffi::c_void);
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_enum_connections(
    _this: *mut core::ffi::c_void,
    _pp_enum: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_type_info_count(
    _this: *mut core::ffi::c_void,
    pctinfo: *mut u32,
) -> i32 {
    if pctinfo.is_null() {
        return COM_E_INVALIDARG;
    }
    *pctinfo = 0;
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_test_get_type_info(
    _this: *mut core::ffi::c_void,
    _itinfo: u32,
    _lcid: u32,
    _pptinfo: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_get_ids_of_names(
    _this: *mut core::ffi::c_void,
    _riid: *const windows_sys::core::GUID,
    rgsznames: *mut *mut u16,
    cnames: u32,
    _lcid: u32,
    rgdispid: *mut i32,
) -> i32 {
    if rgsznames.is_null() || rgdispid.is_null() || cnames == 0 {
        return COM_E_INVALIDARG;
    }
    let name = read_utf16_z(*rgsznames)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let dispid = match name.as_str() {
        "count" => TEST_DISPID_COUNT,
        "exists" => TEST_DISPID_EXISTS,
        "firechanged" => TEST_DISPID_FIRE_CHANGED,
        "firechangedpair" => TEST_DISPID_FIRE_CHANGED_PAIR,
        "firechangedsourceinterface" => TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
        "ping" => TEST_DISPID_PING,
        "lookup" => TEST_DISPID_LOOKUP,
        "setvalue" => TEST_DISPID_SET_VALUE,
        "setvalueref" => TEST_DISPID_SET_VALUE_REF,
        "value" => TEST_DISPID_VALUE,
        _ => return COM_DISP_E_UNKNOWNNAME,
    };
    *rgdispid = dispid;
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_test_invoke(
    this: *mut core::ffi::c_void,
    dispidmember: i32,
    _riid: *const windows_sys::core::GUID,
    _lcid: u32,
    wflags: u16,
    pparams: *mut DISPPARAMS,
    pvarresult: *mut VARIANT,
    _pexcepinfo: *mut EXCEPINFO,
    puargerr: *mut u32,
) -> i32 {
    let (cargs, rgvarg) = if pparams.is_null() {
        (0, std::ptr::null_mut())
    } else {
        ((*pparams).cArgs, (*pparams).rgvarg)
    };
    match dispidmember {
        TEST_DISPID_COUNT => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_i32(7, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_EXISTS => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let key = match arg.Anonymous.Anonymous.vt {
                VT_I4 => arg.Anonymous.Anonymous.Anonymous.lVal,
                VT_UI4 => arg.Anonymous.Anonymous.Anonymous.ulVal as i32,
                _ => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return COM_DISP_E_TYPEMISMATCH;
                }
            };
            set_variant_bool(key == 42, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_event(owner, TEST_EVENT_CHANGED, &[value]);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED_PAIR => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_event(
                owner,
                TEST_EVENT_CHANGED_PAIR,
                &[value, value.saturating_add(1)],
            );
            set_variant_i32(value.saturating_add(1), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let _ = oxvba_test_fire_source_interface_event(owner, value);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_PING => {
            if (wflags & DISPATCH_METHOD) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            set_variant_i32(123, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_LOOKUP => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 1 || rgvarg.is_null() {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let arg = &*rgvarg;
            let value = match raw_variant_token_from_invoke_arg(arg, 0) {
                Ok(value) => value,
                Err(hr) => {
                    if !puargerr.is_null() {
                        *puargerr = 0;
                    }
                    return hr;
                }
            };
            set_variant_i32(value.saturating_add(1_000), pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_VALUE => {
            if (wflags & DISPATCH_PROPERTYPUT) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_property_put_i4_from_params(pparams, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            (*owner).value_state.store(value, Ordering::Release);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_SET_VALUE_REF => {
            if (wflags & DISPATCH_PROPERTYPUTREF) == 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let value = match raw_property_put_i4_from_params(pparams, puargerr) {
                Ok(value) => value,
                Err(hr) => return hr,
            };
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let stored = value.saturating_add(100_000);
            (*owner).value_state.store(stored, Ordering::Release);
            set_variant_i32(stored, pvarresult);
            COM_S_OK
        }
        TEST_DISPID_VALUE => {
            if (wflags & DISPATCH_PROPERTYGET) == 0 || cargs != 0 {
                return COM_DISP_E_BADPARAMCOUNT;
            }
            let owner = as_oxvba_test_dispatch_owner_from_dispatch(this);
            let value = (*owner).value_state.load(Ordering::Acquire);
            set_variant_i32(value, pvarresult);
            COM_S_OK
        }
        _ => COM_DISP_E_MEMBERNOTFOUND,
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_sink_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_IDISPATCH) {
        *ppv = this;
        let sink = as_oxvba_com_event_sink(this);
        (*sink).ref_count.fetch_add(1, Ordering::AcqRel);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_sink_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let sink = as_oxvba_com_event_sink(this);
    (*sink).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_sink_release(this: *mut core::ffi::c_void) -> u32 {
    let sink = as_oxvba_com_event_sink(this);
    let prev = (*sink).ref_count.fetch_sub(1, Ordering::AcqRel);
    let next = prev.saturating_sub(1);
    if next == 0 {
        drop(Box::from_raw(sink));
    }
    next
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_sink_get_type_info_count(
    _this: *mut core::ffi::c_void,
    pctinfo: *mut u32,
) -> i32 {
    if pctinfo.is_null() {
        return COM_E_INVALIDARG;
    }
    *pctinfo = 0;
    COM_S_OK
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_event_sink_get_type_info(
    _this: *mut core::ffi::c_void,
    _itinfo: u32,
    _lcid: u32,
    _pptinfo: *mut *mut core::ffi::c_void,
) -> i32 {
    COM_E_NOTIMPL
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn oxvba_event_sink_get_ids_of_names(
    _this: *mut core::ffi::c_void,
    _riid: *const windows_sys::core::GUID,
    _rgsznames: *mut *mut u16,
    _cnames: u32,
    _lcid: u32,
    _rgdispid: *mut i32,
) -> i32 {
    COM_DISP_E_UNKNOWNNAME
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_sink_invoke(
    this: *mut core::ffi::c_void,
    dispidmember: i32,
    _riid: *const windows_sys::core::GUID,
    _lcid: u32,
    _wflags: u16,
    pparams: *mut DISPPARAMS,
    _pvarresult: *mut VARIANT,
    _pexcepinfo: *mut EXCEPINFO,
    puargerr: *mut u32,
) -> i32 {
    let sink = as_oxvba_com_event_sink(this);
    if (*sink).event_dispatch_member != COM_EVENT_DISPATCH_MEMBER_WILDCARD
        && dispidmember != (*sink).event_dispatch_member
    {
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] sink-invoke ignored subscription={} expected_dispid={} actual_dispid={}",
                (*sink).subscription,
                (*sink).event_dispatch_member,
                dispidmember
            );
        }
        return COM_DISP_E_MEMBERNOTFOUND;
    }
    let (cargs, rgvarg) = if pparams.is_null() {
        (0usize, std::ptr::null_mut())
    } else {
        ((*pparams).cArgs as usize, (*pparams).rgvarg)
    };
    if cargs != (*sink).expected_arity || (cargs > 0 && rgvarg.is_null()) {
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] sink-invoke bad-arity subscription={} expected={} actual={} rgvarg_null={}",
                (*sink).subscription,
                (*sink).expected_arity,
                cargs,
                rgvarg.is_null()
            );
        }
        return COM_DISP_E_BADPARAMCOUNT;
    }
    let mut args = Vec::with_capacity(cargs);
    for idx in (0..cargs).rev() {
        let variant = rgvarg.add(idx);
        let arg_index = cargs.saturating_sub(1).saturating_sub(idx);
        let value = match raw_variant_token_from_invoke_arg(variant, arg_index) {
            Ok(value) => value,
            Err(hr) => {
                if com_event_trace_enabled() {
                    eprintln!(
                        "[oxvba-hal][com-event] sink-invoke arg-conversion-failed subscription={} arg_index={} hresult={:#010X}",
                        (*sink).subscription,
                        arg_index,
                        hr as u32
                    );
                }
                if !puargerr.is_null() {
                    *puargerr = u32::try_from(arg_index).unwrap_or(u32::MAX);
                }
                return hr;
            }
        };
        args.push(value);
    }
    let mut state = match (*sink).com_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(subscription) = state.subscriptions.get(&(*sink).subscription)
        && subscription.object == (*sink).object
        && subscription.event == (*sink).event_token
    {
        let queued = state.queue_callback_for_subscription((*sink).subscription, args.as_slice());
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] sink-invoke subscription={} object={} event={} dispid={} args={:?} queued={}",
                (*sink).subscription,
                (*sink).object,
                (*sink).event_token,
                dispidmember,
                args,
                queued
            );
        }
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_source_interface_sink_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const windows_sys::core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return COM_E_INVALIDARG;
    }
    *ppv = std::ptr::null_mut();
    if guid_equals(riid, &IID_IUNKNOWN) || guid_equals(riid, &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS)
    {
        *ppv = this;
        let sink = as_oxvba_com_event_source_interface_sink(this);
        (*sink).ref_count.fetch_add(1, Ordering::AcqRel);
        return COM_S_OK;
    }
    COM_E_NOINTERFACE
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_source_interface_sink_add_ref(
    this: *mut core::ffi::c_void,
) -> u32 {
    let sink = as_oxvba_com_event_source_interface_sink(this);
    (*sink).ref_count.fetch_add(1, Ordering::AcqRel) + 1
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_source_interface_sink_release(
    this: *mut core::ffi::c_void,
) -> u32 {
    let sink = as_oxvba_com_event_source_interface_sink(this);
    let prev = (*sink).ref_count.fetch_sub(1, Ordering::AcqRel);
    let next = prev.saturating_sub(1);
    if next == 0 {
        drop(Box::from_raw(sink));
    }
    next
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn oxvba_event_source_interface_sink_changed(
    this: *mut core::ffi::c_void,
    value: i32,
) -> i32 {
    let sink = as_oxvba_com_event_source_interface_sink(this);
    if (*sink).expected_arity != 1 {
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] source-sink bad-arity subscription={} expected={} actual=1",
                (*sink).subscription,
                (*sink).expected_arity
            );
        }
        return COM_DISP_E_BADPARAMCOUNT;
    }
    let mut state = match (*sink).com_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(subscription) = state.subscriptions.get(&(*sink).subscription)
        && subscription.object == (*sink).object
        && subscription.event == (*sink).event_token
    {
        let args = [value];
        let queued = state.queue_callback_for_subscription((*sink).subscription, &args);
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] source-sink-changed subscription={} object={} event={} value={} queued={}",
                (*sink).subscription,
                (*sink).object,
                (*sink).event_token,
                value,
                queued
            );
        }
    }
    COM_S_OK
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_release_dispatch(dispatch: *mut RawIDispatch) {
    if dispatch.is_null() {
        return;
    }
    let vtbl = (*dispatch).vtbl;
    ((*vtbl).unknown.release)(dispatch.cast());
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_release_connection_point(connection_point: *mut RawIConnectionPoint) {
    if connection_point.is_null() {
        return;
    }
    let vtbl = (*connection_point).vtbl;
    ((*vtbl).unknown.release)(connection_point.cast());
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_release_unknown(unknown: *mut core::ffi::c_void) {
    if unknown.is_null() {
        return;
    }
    let unknown = unknown.cast::<RawIUnknown>();
    let vtbl = (*unknown).vtbl;
    ((*vtbl).release)(unknown.cast());
}

#[cfg(target_os = "windows")]
fn parse_guid_canonical(input: &str) -> Option<windows_sys::core::GUID> {
    let normalized = normalize_guid_like(input);
    let parts: Vec<&str> = normalized.split('-').collect();
    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        return None;
    }
    let data1 = u32::from_str_radix(parts[0], 16).ok()?;
    let data2 = u16::from_str_radix(parts[1], 16).ok()?;
    let data3 = u16::from_str_radix(parts[2], 16).ok()?;
    let mut data4 = [0u8; 8];
    data4[0] = u8::from_str_radix(&parts[3][0..2], 16).ok()?;
    data4[1] = u8::from_str_radix(&parts[3][2..4], 16).ok()?;
    for idx in 0..6 {
        let start = idx * 2;
        let end = start + 2;
        data4[idx + 2] = u8::from_str_radix(&parts[4][start..end], 16).ok()?;
    }
    Some(windows_sys::core::GUID {
        data1,
        data2,
        data3,
        data4,
    })
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_try_advise_connection_point_event(
    dispatch: *mut RawIDispatch,
    request: ComConnectionPointAdviseRequest,
    connection_point_iid: &str,
) -> Result<Option<ComNativeConnectionPointTransport>, String> {
    if dispatch.is_null() {
        return Ok(None);
    }
    let event_interface = parse_guid_canonical(connection_point_iid).ok_or_else(|| {
        format!("invalid connection-point IID `{connection_point_iid}` in event metadata")
    })?;
    let mut cpc_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    let hr = ((*(*dispatch).vtbl).unknown.query_interface)(
        dispatch.cast(),
        &IID_ICONNECTIONPOINTCONTAINER,
        &mut cpc_ptr,
    );
    if hr < 0 || cpc_ptr.is_null() {
        return Err(format!(
            "IUnknown::QueryInterface(IConnectionPointContainer) failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    let cpc = cpc_ptr.cast::<RawIConnectionPointContainer>();
    let mut cp_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    let hr = ((*(*cpc).vtbl).find_connection_point)(cpc.cast(), &event_interface, &mut cp_ptr);
    raw_release_unknown(cpc.cast());
    if hr < 0 || cp_ptr.is_null() {
        return Err(format!(
            "IConnectionPointContainer::FindConnectionPoint failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    let connection_point = cp_ptr.cast::<RawIConnectionPoint>();
    let sink = match request.sink_mode {
        ComConnectionPointSinkMode::Dispatch {
            event_dispatch_member,
        } => create_oxvba_com_event_sink(
            request.com_state,
            request.subscription,
            request.object,
            request.event_token,
            event_dispatch_member,
            request.expected_arity,
        ),
        ComConnectionPointSinkMode::SourceInterface => {
            if !guid_equals(&event_interface, &IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS) {
                raw_release_connection_point(connection_point);
                return Err(format!(
                    "source-interface event sink is unsupported for connection-point IID `{connection_point_iid}` in current lane"
                ));
            }
            create_oxvba_com_event_source_interface_sink(
                request.com_state,
                request.subscription,
                request.object,
                request.event_token,
                request.expected_arity,
            )
        }
    };
    let mut cookie = 0u32;
    let hr =
        ((*(*connection_point).vtbl).advise)(connection_point.cast(), sink.cast(), &mut cookie);
    raw_release_unknown(sink);
    if hr < 0 || cookie == 0 {
        raw_release_connection_point(connection_point);
        return Err(format!(
            "IConnectionPoint::Advise failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(Some(ComNativeConnectionPointTransport {
        connection_point: connection_point as usize,
        cookie,
    }))
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_unadvise_connection_point(
    transport: ComNativeConnectionPointTransport,
) -> Result<(), String> {
    if transport.connection_point == 0 {
        return Ok(());
    }
    let connection_point = transport.connection_point as *mut RawIConnectionPoint;
    let hr = ((*(*connection_point).vtbl).unadvise)(connection_point.cast(), transport.cookie);
    raw_release_connection_point(connection_point);
    if hr < 0 {
        return Err(format!(
            "IConnectionPoint::Unadvise failed with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_get_dispid_by_name(dispatch: *mut RawIDispatch, name: &str) -> Result<i32, String> {
    let mut name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut name_ptr = name_wide.as_mut_ptr();
    let mut dispid = 0i32;
    let hr = ((*(*dispatch).vtbl).get_ids_of_names)(
        dispatch.cast(),
        &IID_NULL,
        &mut name_ptr,
        1,
        0x0400,
        &mut dispid,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::GetIDsOfNames failed for `{name}` with HRESULT {:#010X}",
            hr as u32
        ));
    }
    Ok(dispid)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_to_token(variant: &VARIANT) -> Result<i32, String> {
    let value = match variant.Anonymous.Anonymous.vt {
        VT_EMPTY => 0,
        VT_I4 => variant.Anonymous.Anonymous.Anonymous.lVal,
        VT_UI4 => variant.Anonymous.Anonymous.Anonymous.ulVal as i32,
        VT_BOOL => {
            let value: VARIANT_BOOL = variant.Anonymous.Anonymous.Anonymous.boolVal;
            if value == 0 { 0 } else { 1 }
        }
        vt => {
            return Err(format!("unsupported VARIANT return type vt={vt}"));
        }
    };
    Ok(value)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_oxvba_test_dispatch_vtable_invoke(
    dispatch: *mut RawIDispatch,
    member: i32,
    arg: i32,
) -> Result<Option<i32>, String> {
    if dispatch.is_null() {
        return Err("null dispatch pointer for vtable invoke".to_string());
    }
    if !std::ptr::eq((*dispatch).vtbl, &OXVBA_TEST_DISPATCH_VTBL) {
        return Ok(None);
    }
    match member {
        TEST_DISPID_COUNT => Ok(Some(7)),
        TEST_DISPID_EXISTS => {
            if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(if arg == 42 { 1 } else { 0 }))
        }
        TEST_DISPID_FIRE_CHANGED => {
            if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(arg))
        }
        TEST_DISPID_FIRE_CHANGED_PAIR => {
            if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
                return Err(format!(
                    "IDispatch::Invoke(method) failed with HRESULT {:#010X} (arg_err={})",
                    COM_DISP_E_BADPARAMCOUNT as u32, 0
                ));
            }
            Ok(Some(arg.saturating_add(1)))
        }
        _ => Ok(None),
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_property_get_noargs(
    dispatch: *mut RawIDispatch,
    dispid: i32,
) -> Result<i32, String> {
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_PROPERTYGET,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(property-get) failed with HRESULT {:#010X} (arg_err={})",
            hr as u32, arg_err
        ));
    }

    let token = raw_variant_to_token(&result)?;
    let _ = VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_property_get_i4(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    arg: i32,
) -> Result<i32, String> {
    let mut arg_variant: VARIANT = std::mem::zeroed();
    arg_variant.Anonymous.Anonymous.vt = VT_I4;
    arg_variant.Anonymous.Anonymous.Anonymous.lVal = arg;

    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: &mut arg_variant,
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 1,
        cNamedArgs: 0,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_PROPERTYGET,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(property-get dispid={dispid}) failed with HRESULT {:#010X} (arg_err={})",
            hr as u32, arg_err
        ));
    }

    let token = raw_variant_to_token(&result)?;
    let _ = VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_property_put_i4(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    arg: i32,
) -> Result<i32, String> {
    let mut arg_variant: VARIANT = std::mem::zeroed();
    arg_variant.Anonymous.Anonymous.vt = VT_I4;
    arg_variant.Anonymous.Anonymous.Anonymous.lVal = arg;

    let mut named_arg = COM_DISPID_PROPERTYPUT;
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: &mut arg_variant,
        rgdispidNamedArgs: &mut named_arg,
        cArgs: 1,
        cNamedArgs: 1,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_PROPERTYPUT,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(property-put dispid={dispid}) failed with HRESULT {:#010X} (arg_err={})",
            hr as u32, arg_err
        ));
    }

    let token = raw_variant_to_token(&result)?;
    let _ = VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_property_putref_i4(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    arg: i32,
) -> Result<i32, String> {
    let mut arg_variant: VARIANT = std::mem::zeroed();
    arg_variant.Anonymous.Anonymous.vt = VT_I4;
    arg_variant.Anonymous.Anonymous.Anonymous.lVal = arg;

    let mut named_arg = COM_DISPID_PROPERTYPUT;
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: &mut arg_variant,
        rgdispidNamedArgs: &mut named_arg,
        cArgs: 1,
        cNamedArgs: 1,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_PROPERTYPUTREF,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(property-putref dispid={dispid}) failed with HRESULT {:#010X} (arg_err={})",
            hr as u32, arg_err
        ));
    }

    let token = raw_variant_to_token(&result)?;
    let _ = VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_method_noargs(
    dispatch: *mut RawIDispatch,
    dispid: i32,
) -> Result<i32, String> {
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_METHOD,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(method dispid={dispid}) failed with HRESULT {:#010X} (arg_err={})",
            hr as u32, arg_err
        ));
    }

    let token = raw_variant_to_token(&result)?;
    let _ = VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_method_i4(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    arg: i32,
) -> Result<i32, String> {
    let mut arg_variant: VARIANT = std::mem::zeroed();
    arg_variant.Anonymous.Anonymous.vt = VT_I4;
    arg_variant.Anonymous.Anonymous.Anonymous.lVal = arg;

    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = 0u32;
    let mut params = DISPPARAMS {
        rgvarg: &mut arg_variant,
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 1,
        cNamedArgs: 0,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        DISPATCH_METHOD,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    if hr < 0 {
        return Err(format!(
            "IDispatch::Invoke(method dispid={dispid}) failed with HRESULT {:#010X} (arg_err={})",
            hr as u32, arg_err
        ));
    }

    let token = raw_variant_to_token(&result)?;
    let _ = VariantClear(&mut result);
    Ok(token)
}

fn pseudo_file_len_from_path_token(path: i32) -> i32 {
    let magnitude = path.saturating_abs();
    1 + (magnitude % 4096)
}

fn clamp_u64_to_i32(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

fn normalize_ci_token(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn normalize_guid_like(input: &str) -> String {
    input
        .trim()
        .trim_matches('{')
        .trim_matches('}')
        .to_ascii_lowercase()
}

fn external_symbol_token(library: &str, alias: &str, name: &str) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in library
        .bytes()
        .chain([b'!'])
        .chain(alias.bytes())
        .chain([b'!'])
        .chain(name.bytes())
    {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    (hash & 0x7fff_ffff).max(1) as i32
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::{
        error::HalErrorKind,
        model::{ComInvocationStrategy, HalProfileId, HostPolicy},
        traits::{
            ComHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal,
            FileSystemHal, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibResolveRequest,
            TypeLibraryHal, UiInteractionHal,
        },
    };

    use super::StandardHostServices;

    fn current_native_profile() -> Option<HalProfileId> {
        if cfg!(target_os = "windows") {
            Some(HalProfileId::Windows)
        } else if cfg!(target_os = "linux") {
            Some(HalProfileId::Linux)
        } else {
            None
        }
    }

    #[test]
    fn file_open_seek_eof_lof_close_roundtrip() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host.open(77, 0).expect("open should succeed");
        assert_eq!(handle, 1);
        assert_eq!(host.eof(handle).expect("eof should work"), 0);
        assert!(host.lof(handle).expect("lof should work") > 0);
        host.seek(handle, host.lof(handle).expect("lof should work"))
            .expect("seek to end should work");
        assert_eq!(host.eof(handle).expect("eof should work"), 1);
        assert_eq!(host.close(handle).expect("close should work"), 1);
    }

    #[test]
    fn free_file_respects_low_and_high_ranges() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.free_file(0).expect("default free file"), 1);
        assert_eq!(host.free_file(1).expect("high-range free file"), 256);
    }

    #[test]
    fn close_releases_handle_for_reuse() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host.open(10, 0).expect("open should succeed");
        assert_eq!(first, 1);
        host.close(first).expect("close should succeed");
        let second = host.open(11, 0).expect("second open should succeed");
        assert_eq!(second, 1, "closed handles must be reusable");
    }

    #[test]
    fn free_file_low_range_tracks_allocated_handles() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let mut handles = Vec::new();
        for expected in 1..=8 {
            assert_eq!(
                host.free_file(0).expect("free_file should succeed"),
                expected
            );
            handles.push(host.open(expected, 0).expect("open should succeed"));
        }
        assert_eq!(handles, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn seek_negative_returns_adapter_fault() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host.open(42, 0).expect("open should succeed");
        let err = host
            .seek(handle, -1)
            .expect_err("negative seek should return error");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
    }

    #[test]
    fn ui_virtualization_modes_follow_contract() {
        let policy = HostPolicy {
            allow_interaction: true,
            ui_virtualization: crate::model::UiVirtualizationMode::ScriptedResponses,
            ..HostPolicy::default()
        };
        let host = StandardHostServices::new(HalProfileId::Windows, policy.clone());
        assert_eq!(host.msg_box(100, 3).expect("msg_box"), 3);
        assert_eq!(host.input_box(100, 7).expect("input_box"), 7);

        let host_disabled = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                ui_virtualization: crate::model::UiVirtualizationMode::Disabled,
                ..policy
            },
        );
        assert_eq!(host_disabled.msg_box(100, 3).expect("msg_box"), 100);
        assert_eq!(host_disabled.input_box(100, 7).expect("input_box"), 100);
    }

    #[test]
    fn ui_fail_on_prompt_returns_policy_denied() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_interaction: true,
                ui_virtualization: crate::model::UiVirtualizationMode::FailOnPrompt,
                ..HostPolicy::default()
            },
        );
        let err = host.msg_box(9, 1).expect_err("msg_box should be denied");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        let err = host
            .input_box(9, 1)
            .expect_err("input_box should be denied");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
    }

    #[test]
    fn process_com_dynlink_policy_denials_are_enforced() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_process_spawn: false,
                allow_com_activation: false,
                allow_dynamic_link: false,
                ..HostPolicy::default()
            },
        );

        assert_eq!(
            host.shell(1, 0).expect_err("shell deny").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.create_object(1).expect_err("com deny").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.invoke_symbol(1, 2).expect_err("dynlink deny").kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn dynlink_bind_descriptor_rejects_unsupported_marshal_lane() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_dynamic_link: true,
                ..HostPolicy::default()
            },
        );
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 7,
            declared_name: "hostping",
            library: "host",
            alias: "ping",
            ordinal_alias: false,
            symbol: 7,
            marshal_lane: "m2-pointer-lpstr",
            calling_convention: "platform-default",
            selection_policy: "case-insensitive-canonical",
        };
        let err = host
            .bind_descriptor(&descriptor)
            .expect_err("unsupported marshaling lane should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("unsupported marshaling lane"));
    }

    #[test]
    fn dynlink_bind_descriptor_rejects_unsupported_calling_convention() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_dynamic_link: true,
                ..HostPolicy::default()
            },
        );
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 7,
            declared_name: "hostping",
            library: "host",
            alias: "ping",
            ordinal_alias: false,
            symbol: 7,
            marshal_lane: "m0-deterministic",
            calling_convention: "stdcall",
            selection_policy: "case-insensitive-canonical",
        };
        let err = host
            .bind_descriptor(&descriptor)
            .expect_err("unsupported calling convention should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("unsupported calling convention"));
    }

    #[test]
    fn dynlink_bind_descriptor_rejects_selection_policy_mismatch() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_dynamic_link: true,
                ..HostPolicy::default()
            },
        );
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 7,
            declared_name: "hostping",
            library: "host",
            alias: "ping",
            ordinal_alias: false,
            symbol: 7,
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "ordinal-literal-canonical",
        };
        let err = host
            .bind_descriptor(&descriptor)
            .expect_err("selection policy mismatch should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("unsupported selection policy"));
    }

    #[test]
    fn dynlink_bind_descriptor_validates_ordinal_alias_shape() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_dynamic_link: true,
                ..HostPolicy::default()
            },
        );
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 7,
            declared_name: "hostping",
            library: "host",
            alias: "ping",
            ordinal_alias: true,
            symbol: 7,
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "ordinal-literal-canonical",
        };
        let err = host
            .bind_descriptor(&descriptor)
            .expect_err("ordinal alias without #digits should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("must start with `#`"));
    }

    #[test]
    fn time_locale_contract_values_are_stable() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.date_serial_now().expect("date"), 20_260_301);
        assert_eq!(host.time_serial_now().expect("time"), 123_456);
        assert_eq!(host.timer_ticks().expect("timer"), 42);
    }

    #[test]
    fn process_env_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.environ(88).expect("environ"), 88);
        assert_eq!(host.dir(0, 0).expect("dir"), 0);
        assert_eq!(host.dir(5, 0).expect("dir"), 1);
    }

    #[test]
    fn dispatch_invoke_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.dispatch_invoke(10, 20, 30).expect("dispatch"), 60);
    }

    #[test]
    fn dispatch_invoke_missing_arg_token_projects_as_zero() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.dispatch_invoke(10, 20, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
                .expect("dispatch"),
            30
        );
    }

    #[test]
    fn diagnostics_emit_contract_is_deterministic() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.emit(4, 5).expect("emit"), 9);
    }

    #[test]
    fn event_pump_supported_and_unsupported_paths() {
        let windows = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(windows.do_events().expect("windows do_events"), 0);

        let null = StandardHostServices::new(HalProfileId::Null, HostPolicy::default());
        let err = null
            .do_events()
            .expect_err("null do_events should be unsupported");
        assert_eq!(err.kind, HalErrorKind::CapabilityUnavailable);
    }

    #[test]
    fn file_open_denied_has_no_state_side_effects() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_filesystem_mutation: false,
                ..HostPolicy::default()
            },
        );
        assert_eq!(host.free_file(0).expect("first free should be 1"), 1);
        let err = host
            .open(10, 1)
            .expect_err("mutation open should be denied by policy");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        assert_eq!(
            host.free_file(0)
                .expect("free file should remain unchanged"),
            1
        );
    }

    #[test]
    fn invalid_close_does_not_mutate_handle_state() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host.open(10, 0).expect("open should succeed");
        assert_eq!(first, 1);
        let err = host.close(99).expect_err("invalid close should fail");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert_eq!(
            host.free_file(0)
                .expect("free file should still skip handle 1"),
            2
        );
    }

    #[test]
    fn ui_msg_box_enforces_policy_and_capability_failures() {
        let denied_host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let err = denied_host
            .msg_box(1, 1)
            .expect_err("interaction is denied by default policy");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);

        let null_host = StandardHostServices::new(
            HalProfileId::Null,
            HostPolicy {
                allow_interaction: true,
                ..HostPolicy::default()
            },
        );
        let err = null_host
            .msg_box(1, 1)
            .expect_err("null profile should report unsupported capability");
        assert_eq!(err.kind, HalErrorKind::CapabilityUnavailable);
    }

    #[test]
    fn null_profile_support_set_is_explicit() {
        let host = StandardHostServices::new(HalProfileId::Null, HostPolicy::default());
        let descriptor = host.descriptor();
        assert!(!descriptor.supports(crate::model::CapabilityId::UiInteraction));
        assert!(!descriptor.supports(crate::model::CapabilityId::EventPump));
        assert!(!descriptor.supports(crate::model::CapabilityId::FileSystemIo));
        assert!(!descriptor.supports(crate::model::CapabilityId::ProcessEnv));
        assert!(!descriptor.supports(crate::model::CapabilityId::ComActivationDispatch));
        assert!(descriptor.supports(crate::model::CapabilityId::TimeLocale));
        assert!(!descriptor.supports(crate::model::CapabilityId::DynamicLinking));
        assert!(descriptor.supports(crate::model::CapabilityId::DiagnosticsTelemetry));
    }

    #[test]
    fn maturity_does_not_affect_policy_denial_shape() {
        let policy = HostPolicy {
            allow_process_spawn: false,
            ..HostPolicy::default()
        };
        let windows = StandardHostServices::new(HalProfileId::Windows, policy.clone());
        let linux = StandardHostServices::new(HalProfileId::Linux, policy);
        assert_eq!(
            windows.shell(1, 0).expect_err("windows shell denial").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            linux.shell(1, 0).expect_err("linux shell denial").kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn native_mode_process_and_env_paths_are_callable() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        let shell = host.shell(1, 0).expect("native shell should succeed");
        assert!(shell >= 1);
        let environ = host.environ(3).expect("native environ should succeed");
        assert!(environ >= 0);
        let dir = host.dir(0, 0).expect("native dir should succeed");
        assert!(dir == 0 || dir == 1);
    }

    #[test]
    fn native_mode_filesystem_seek_can_extend_length() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        let handle = host.open(31415, 1).expect("native open should succeed");
        host.seek(handle, 64).expect("native seek should succeed");
        assert!(
            host.lof(handle).expect("native lof should succeed") >= 64,
            "native seek in mutation mode should extend logical length"
        );
        assert_eq!(host.close(handle).expect("native close should succeed"), 1);
    }

    #[test]
    fn native_mode_time_tokens_are_non_negative() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        assert!(host.date_serial_now().expect("date") >= 0);
        assert!(host.time_serial_now().expect("time") >= 0);
        assert!(host.timer_ticks().expect("ticks") >= 0);
    }

    #[test]
    fn com_event_subscription_lane_requires_native_mode() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let subscribe = host
            .subscribe_event(1, 1)
            .expect_err("subscribe_event should require native mode");
        assert_eq!(subscribe.kind, HalErrorKind::AdapterFault);
        assert_eq!(subscribe.operation, "subscribe_event");
        assert!(subscribe.message.contains("COM-E-EVENT-PATH-UNSUPPORTED"));

        let unsubscribe = host
            .unsubscribe_event(1)
            .expect_err("unsubscribe_event should require native mode");
        assert_eq!(unsubscribe.kind, HalErrorKind::AdapterFault);
        assert_eq!(unsubscribe.operation, "unsubscribe_event");
        assert!(unsubscribe.message.contains("COM-E-EVENT-PATH-UNSUPPORTED"));
        assert!(
            host.event_callback_subscription(60_001)
                .expect_err("event_callback_subscription should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.event_callback_arity(60_001)
                .expect_err("event_callback_arity should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.event_callback_arg(60_001, 0)
                .expect_err("event_callback_arg should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.release_event_callback(60_001)
                .expect_err("release_event_callback should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_subscription_lifecycle_is_tracked() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        assert!(
            object >= 20_001,
            "controlled COM lane should bind native object"
        );
        let subscription = host
            .subscribe_event(object, 1)
            .expect("subscribe_event should succeed for controlled event source");
        assert!(subscription >= 40_001);
        {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            let registered = state
                .subscriptions
                .get(&subscription)
                .expect("subscription should be tracked");
            assert!(
                matches!(
                    registered.transport,
                    super::ComEventSubscriptionTransport::NativeConnectionPoint(_)
                ),
                "controlled lane should use native connection-point transport"
            );
        }

        assert_eq!(
            host.dispatch_invoke(object, 3, 77)
                .expect("FireChanged should succeed"),
            77
        );
        let callback = host
            .do_events()
            .expect("do_events should pump pending COM callback");
        assert!(callback >= 60_001);
        assert_eq!(
            host.event_callback_subscription(callback)
                .expect("callback subscription lookup should succeed"),
            subscription
        );
        assert_eq!(
            host.event_callback_arg(callback, 0)
                .expect("callback arg lookup should succeed"),
            77
        );
        assert_eq!(
            host.event_callback_arity(callback)
                .expect("callback arity lookup should succeed"),
            1
        );
        assert_eq!(
            host.release_event_callback(callback)
                .expect("callback release should succeed"),
            1
        );
        assert_eq!(
            host.do_events().expect("callback queue should be drained"),
            0,
            "native callback lane should not enqueue duplicate projection callbacks"
        );

        assert_eq!(
            host.unsubscribe_event(subscription)
                .expect("unsubscribe_event should succeed"),
            1
        );
        let _ = host
            .dispatch_invoke(object, 3, 88)
            .expect("FireChanged should remain invokable after unsubscribe");
        assert_eq!(
            host.do_events()
                .expect("callback queue should remain empty after unsubscribe"),
            0
        );
        let callback_still_present = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state.callbacks.contains_key(&callback)
        };
        assert!(
            !callback_still_present,
            "released callback payload should be removed from callback registry"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_multi_arg_callback_payload_roundtrips() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(object, super::TEST_EVENT_CHANGED_PAIR)
            .expect("subscribe_event should succeed for controlled pair-event source");

        assert_eq!(
            host.dispatch_invoke(object, super::TEST_DISPID_FIRE_CHANGED_PAIR, 90)
                .expect("FireChangedPair should succeed"),
            91
        );
        let callback = host
            .do_events()
            .expect("do_events should pump pending COM callback");
        assert!(callback >= 60_001);
        assert_eq!(
            host.event_callback_subscription(callback)
                .expect("callback subscription lookup should succeed"),
            subscription
        );
        assert_eq!(
            host.event_callback_arity(callback)
                .expect("callback arity lookup should succeed"),
            2
        );
        assert_eq!(
            host.event_callback_arg(callback, 0)
                .expect("callback arg0 lookup should succeed"),
            90
        );
        assert_eq!(
            host.event_callback_arg(callback, 1)
                .expect("callback arg1 lookup should succeed"),
            91
        );
        let err = host
            .event_callback_arg(callback, 2)
            .expect_err("index beyond callback arity should fail");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message
                .contains("COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH")
        );
        assert_eq!(
            host.release_event_callback(callback)
                .expect("callback release should succeed"),
            1
        );
        assert_eq!(
            host.unsubscribe_event(subscription)
                .expect("unsubscribe_event should succeed"),
            1
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_subscription_rejects_unknown_event_token() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        let err = host
            .subscribe_event(object, 7)
            .expect_err("unknown event token should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CONNECTIONPOINT-MISSING"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_subscription_supports_controlled_com_evt_b_source_interface_lane() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(object, super::TEST_EVENT_CHANGED_SOURCE_INTERFACE)
            .expect("controlled source-interface event token should subscribe successfully");
        assert!(
            subscription >= 40_001,
            "subscription token should be in deterministic range"
        );
        assert_eq!(
            host.dispatch_invoke(object, super::TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE, 77)
                .expect("FireChangedSourceInterface should succeed"),
            77
        );
        let callback = host
            .do_events()
            .expect("do_events should pump pending source-interface callback");
        assert_eq!(
            host.event_callback_subscription(callback)
                .expect("callback subscription lookup should succeed"),
            subscription
        );
        assert_eq!(
            host.event_callback_arity(callback)
                .expect("callback arity lookup should succeed"),
            1
        );
        assert!(
            host.event_callback_arg(callback, 0)
                .expect("callback arg0 lookup should succeed")
                == 77
        );
        assert_eq!(
            host.release_event_callback(callback)
                .expect("callback release should succeed"),
            1
        );
        assert_eq!(
            host.unsubscribe_event(subscription)
                .expect("unsubscribe should succeed"),
            1
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_unsubscribe_rejects_unknown_subscription() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let err = host
            .unsubscribe_event(40_999)
            .expect_err("unknown subscription should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-ADVISE-FAILED"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_callback_lookup_rejects_unknown_callback() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let err = host
            .event_callback_subscription(60_999)
            .expect_err("unknown callback should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CALLBACK-MISSING"));
        let err = host
            .event_callback_arity(60_999)
            .expect_err("unknown callback arity lookup should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CALLBACK-MISSING"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_callback_arg_index_is_validated() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(object, 1)
            .expect("subscribe should succeed");
        let _ = host
            .dispatch_invoke(object, 3, 77)
            .expect("FireChanged should succeed");
        let callback = host.do_events().expect("callback token");
        let err = host
            .event_callback_arg(callback, 1)
            .expect_err("only callback arg index 0 should be supported");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message
                .contains("COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH")
        );
        assert_eq!(
            host.release_event_callback(callback)
                .expect("release callback should succeed"),
            1
        );
        assert_eq!(
            host.unsubscribe_event(subscription)
                .expect("unsubscribe should succeed"),
            1
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_mode_persists_mutation_to_host_file() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let token = 424_242;
        let host_path = host.host_path_from_token(token);
        let _ = std::fs::remove_file(&host_path);

        let handle = host.open(token, 1).expect("native open should succeed");
        host.seek(handle, 160).expect("native seek should succeed");
        assert_eq!(host.close(handle).expect("native close should succeed"), 1);

        let metadata = std::fs::metadata(&host_path).expect("host-backed file should exist");
        assert!(
            metadata.len() >= 160,
            "host-backed file should reflect seek growth"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_dictionary_lane_executes_when_available() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");

        if object == 5_004 {
            // Environment lacks native activation prerequisites; deterministic fallback remains valid.
            return;
        }

        assert!(
            object >= 20_001,
            "native COM handles use COM-state handle space"
        );
        let count = host
            .dispatch_invoke(object, 1, 0)
            .expect("dictionary Count should be invokable");
        assert!(count >= 0);

        let exists = host
            .dispatch_invoke(object, 2, 42)
            .expect("dictionary Exists should be invokable");
        assert!(exists == 0 || exists == 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_returns_deterministic_values() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        assert!(
            object >= 20_001,
            "controlled COM lane should bind native object"
        );
        assert_eq!(
            host.dispatch_invoke(object, 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
                .expect("Count property-get should succeed"),
            7
        );
        assert_eq!(
            host.dispatch_invoke(object, 2, 42)
                .expect("Exists(42) should succeed"),
            1
        );
        assert_eq!(
            host.dispatch_invoke(object, 2, 41)
                .expect("Exists(41) should succeed"),
            0
        );
        assert_eq!(
            host.dispatch_invoke(object, super::TEST_DISPID_PING, 999)
                .expect("Ping no-arg method invoke should succeed"),
            123
        );
        assert_eq!(
            host.dispatch_invoke(object, super::TEST_DISPID_LOOKUP, 42)
                .expect("Lookup property-get with argument should succeed"),
            1_042
        );
        assert_eq!(
            host.dispatch_invoke(object, super::TEST_DISPID_SET_VALUE, 33)
                .expect("SetValue property-put should succeed"),
            33
        );
        assert_eq!(
            host.dispatch_invoke(
                object,
                super::TEST_DISPID_VALUE,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN
            )
            .expect("Value property-get should reflect SetValue"),
            33
        );
        assert_eq!(
            host.dispatch_invoke(object, super::TEST_DISPID_SET_VALUE_REF, 33)
                .expect("SetValueRef property-putref should succeed"),
            100_033
        );
        assert_eq!(
            host.dispatch_invoke(
                object,
                super::TEST_DISPID_VALUE,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN
            )
            .expect("Value property-get should reflect SetValueRef"),
            100_033
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_property_get_with_required_arg_reports_missing_arg_stably() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        let err = host
            .dispatch_invoke(
                object,
                super::TEST_DISPID_LOOKUP,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN,
            )
            .expect_err("Lookup should reject missing argument");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message.contains("member requires argument"),
            "expected stable missing-argument surface, got {}",
            err.message
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_binding_keeps_stable_dispatch_identity() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        if object == 5_004 {
            return;
        }

        let before = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            let binding = state
                .bindings
                .get(&object)
                .expect("native object should be tracked");
            assert!(
                binding.native_dispatch != 0,
                "native COM binding should hold a dispatch pointer"
            );
            binding.native_dispatch
        };
        let _ = host
            .dispatch_invoke(object, 1, 0)
            .expect("dispatch invoke should succeed");
        let after = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state
                .bindings
                .get(&object)
                .expect("native object should remain tracked")
                .native_dispatch
        };
        assert_eq!(
            before, after,
            "COM dispatch identity should stay stable for the bound object token"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_member_dispid_cache_populates_for_known_tokens() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        if object == 5_004 {
            return;
        }

        let _ = host
            .dispatch_invoke(object, 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
            .expect("dictionary Count should be invokable");
        let cache_size_after_first = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state
                .bindings
                .get(&object)
                .expect("binding should remain tracked")
                .member_dispids
                .len()
        };
        let _ = host
            .dispatch_invoke(object, 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
            .expect("dictionary Count should be invokable repeatedly");
        let cache_size_after_second = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state
                .bindings
                .get(&object)
                .expect("binding should remain tracked")
                .member_dispids
                .len()
        };
        assert!(
            cache_size_after_first >= 1,
            "member cache should populate after first invocation"
        );
        assert_eq!(
            cache_size_after_first, cache_size_after_second,
            "repeated invocation should reuse cached member DISPID entries"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_prefer_vtable_strategy_matches_dispatch_results() {
        let dispatch_host =
            StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let mut vtable_policy = HostPolicy::interactive_dev();
        vtable_policy.com_invocation_strategy = ComInvocationStrategy::PreferVtable;
        let vtable_host = StandardHostServices::new(HalProfileId::Windows, vtable_policy);

        let dispatch_object = dispatch_host
            .create_object(4)
            .expect("dispatch create_object should succeed");
        let vtable_object = vtable_host
            .create_object(4)
            .expect("vtable create_object should succeed");

        let dispatch_count = dispatch_host
            .dispatch_invoke(dispatch_object, 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
            .expect("dispatch count should succeed");
        let vtable_count = vtable_host
            .dispatch_invoke(vtable_object, 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
            .expect("vtable count should succeed");
        assert_eq!(dispatch_count, vtable_count);

        let dispatch_exists = dispatch_host
            .dispatch_invoke(dispatch_object, 2, 42)
            .expect("dispatch exists should succeed");
        let vtable_exists = vtable_host
            .dispatch_invoke(vtable_object, 2, 42)
            .expect("vtable exists should succeed");
        assert_eq!(dispatch_exists, vtable_exists);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_typelib_resolve_load_and_cache_roundtrip() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let identity = host
            .resolve_typelib_reference(&TypeLibResolveRequest {
                reference_name: "StdOle".to_string(),
                importlib_hint: Some("stdole2.tlb".to_string()),
                libid_hint: None,
                major_version_hint: Some(2),
                minor_version_hint: Some(0),
                lcid_hint: Some(0),
            })
            .expect("typelib resolution should succeed");
        assert_eq!(identity.importlib, "stdole2.tlb");
        let first = host
            .load_typelib_metadata(&identity)
            .expect("first metadata load should succeed");
        let second = host
            .load_typelib_metadata(&identity)
            .expect("second metadata load should succeed");
        assert_eq!(first, second);
        let removed = host
            .invalidate_typelib_cache(TypeLibCacheScope::Global, None)
            .expect("global invalidation should succeed");
        assert!(removed >= 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_typelib_reference_invalidation_scope_is_stable() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let alpha = host
            .resolve_typelib_reference(&TypeLibResolveRequest {
                reference_name: "StdOle".to_string(),
                importlib_hint: Some("stdole2.tlb".to_string()),
                libid_hint: None,
                major_version_hint: Some(2),
                minor_version_hint: Some(0),
                lcid_hint: Some(0),
            })
            .expect("stdole resolve should succeed");
        let beta = host
            .resolve_typelib_reference(&TypeLibResolveRequest {
                reference_name: "OxVba".to_string(),
                importlib_hint: Some("oxvba_testdispatch.tlb".to_string()),
                libid_hint: None,
                major_version_hint: Some(1),
                minor_version_hint: Some(0),
                lcid_hint: Some(0),
            })
            .expect("oxvba resolve should succeed");
        let _ = host
            .load_typelib_metadata(&alpha)
            .expect("alpha metadata load should succeed");
        let _ = host
            .load_typelib_metadata(&beta)
            .expect("beta metadata load should succeed");

        let removed = host
            .invalidate_typelib_cache(TypeLibCacheScope::Reference, Some("StdOle"))
            .expect("reference invalidation should succeed");
        assert_eq!(removed, 1);

        let beta_again = host
            .load_typelib_metadata(&beta)
            .expect("beta metadata should remain cached/available");
        assert_eq!(beta_again.identity.reference_name, "OxVba");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_typelib_testdispatch_metadata_includes_member_and_event_shapes() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let identity = host
            .resolve_typelib_reference(&TypeLibResolveRequest {
                reference_name: "OxVba".to_string(),
                importlib_hint: Some("oxvba_testdispatch.tlb".to_string()),
                libid_hint: None,
                major_version_hint: Some(1),
                minor_version_hint: Some(0),
                lcid_hint: Some(0),
            })
            .expect("oxvba resolve should succeed");
        let metadata = host
            .load_typelib_metadata(&identity)
            .expect("metadata load should succeed");
        assert_eq!(
            metadata.member_name_to_token,
            vec![
                ("Count".to_string(), super::TEST_DISPID_COUNT),
                ("Exists".to_string(), super::TEST_DISPID_EXISTS),
                ("FireChanged".to_string(), super::TEST_DISPID_FIRE_CHANGED),
                (
                    "FireChangedPair".to_string(),
                    super::TEST_DISPID_FIRE_CHANGED_PAIR
                ),
                (
                    "FireChangedSourceInterface".to_string(),
                    super::TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE
                ),
                ("Ping".to_string(), super::TEST_DISPID_PING),
                ("Lookup".to_string(), super::TEST_DISPID_LOOKUP),
                ("SetValue".to_string(), super::TEST_DISPID_SET_VALUE),
                ("SetValueRef".to_string(), super::TEST_DISPID_SET_VALUE_REF),
                ("Value".to_string(), super::TEST_DISPID_VALUE),
            ]
        );
        let fire_changed_pair = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_FIRE_CHANGED_PAIR)
            .expect("FireChangedPair metadata should exist");
        assert!(fire_changed_pair.requires_argument);
        assert_eq!(
            fire_changed_pair.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let count_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_COUNT)
            .expect("Count metadata should exist");
        assert_eq!(
            count_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let ping_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_PING)
            .expect("Ping metadata should exist");
        assert!(!ping_member.requires_argument);
        assert_eq!(
            ping_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let lookup_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_LOOKUP)
            .expect("Lookup metadata should exist");
        assert!(lookup_member.requires_argument);
        assert_eq!(
            lookup_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let set_value_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_SET_VALUE)
            .expect("SetValue metadata should exist");
        assert!(set_value_member.requires_argument);
        assert_eq!(
            set_value_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPut
        );
        let set_value_ref_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_SET_VALUE_REF)
            .expect("SetValueRef metadata should exist");
        assert!(set_value_ref_member.requires_argument);
        assert_eq!(
            set_value_ref_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPutRef
        );
        let value_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_VALUE)
            .expect("Value metadata should exist");
        assert!(!value_member.requires_argument);
        assert_eq!(
            value_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let source_interface_event = metadata
            .events
            .iter()
            .find(|entry| entry.token == super::TEST_EVENT_CHANGED_SOURCE_INTERFACE)
            .expect("source-interface event metadata should exist");
        assert_eq!(source_interface_event.callback_arity, 1);
        assert_eq!(
            source_interface_event.dispatch_path,
            super::TypeLibEventDispatchPath::SourceInterface
        );
        assert_eq!(
            source_interface_event.connection_point_iid.as_deref(),
            Some(super::IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR)
        );
        assert!(source_interface_event.dispatch_member_id.is_none());
        let dispatch_event = metadata
            .events
            .iter()
            .find(|entry| entry.token == super::TEST_EVENT_CHANGED_PAIR)
            .expect("dispatch event metadata should exist");
        assert_eq!(
            dispatch_event.connection_point_iid.as_deref(),
            Some(super::IID_OXVBA_TEST_DISPATCH_EVENTS_STR)
        );
        assert_eq!(
            dispatch_event.dispatch_member_id,
            Some(super::TEST_EVENT_CHANGED_PAIR)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_typelib_excel_metadata_includes_quit_event_connection_point_shape() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let identity = host
            .resolve_typelib_reference(&TypeLibResolveRequest {
                reference_name: "Excel".to_string(),
                importlib_hint: Some("excel.exe".to_string()),
                libid_hint: None,
                major_version_hint: Some(1),
                minor_version_hint: Some(0),
                lcid_hint: Some(0),
            })
            .expect("excel resolve should succeed");
        let metadata = host
            .load_typelib_metadata(&identity)
            .expect("excel metadata load should succeed");

        assert_eq!(
            metadata.member_name_to_token,
            vec![("Quit".to_string(), super::TEST_DISPID_EXCEL_QUIT)]
        );
        let quit_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_EXCEL_QUIT)
            .expect("Quit member metadata should exist");
        assert!(!quit_member.requires_argument);
        assert_eq!(
            quit_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );

        let quit_event = metadata
            .events
            .iter()
            .find(|entry| entry.token == super::TEST_EVENT_EXCEL_APP_QUIT)
            .expect("Quit event metadata should exist");
        assert_eq!(quit_event.callback_arity, 0);
        assert_eq!(
            quit_event.dispatch_path,
            super::TypeLibEventDispatchPath::Dispatch
        );
        assert_eq!(
            quit_event.connection_point_iid.as_deref(),
            Some(super::IID_EXCEL_APPLICATION_EVENTS_STR)
        );
        assert!(quit_event.dispatch_member_id.is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_binding_caches_typelib_member_and_event_specs() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object(4)
            .expect("create_object should return a token");
        let state = host
            .com_state
            .lock()
            .expect("com state lock should succeed");
        let binding = state
            .bindings
            .get(&object)
            .expect("binding should be present for native object token");
        let member = binding
            .member_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_PAIR)
            .expect("member spec for FireChangedPair should be present");
        assert_eq!(member.name, "FireChangedPair");
        assert!(member.requires_argument);
        assert_eq!(member.invoke_kind, super::TypeLibMemberInvokeKind::Method);
        let ping = binding
            .member_specs
            .get(&super::TEST_DISPID_PING)
            .expect("member spec for Ping should be present");
        assert_eq!(ping.name, "Ping");
        assert!(!ping.requires_argument);
        assert_eq!(ping.invoke_kind, super::TypeLibMemberInvokeKind::Method);
        let fire_changed_source = binding
            .member_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE)
            .expect("member spec for FireChangedSourceInterface should be present");
        assert_eq!(fire_changed_source.name, "FireChangedSourceInterface");
        assert!(fire_changed_source.requires_argument);
        assert_eq!(
            fire_changed_source.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let lookup = binding
            .member_specs
            .get(&super::TEST_DISPID_LOOKUP)
            .expect("member spec for Lookup should be present");
        assert_eq!(lookup.name, "Lookup");
        assert!(lookup.requires_argument);
        assert_eq!(
            lookup.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let set_value = binding
            .member_specs
            .get(&super::TEST_DISPID_SET_VALUE)
            .expect("member spec for SetValue should be present");
        assert_eq!(set_value.name, "SetValue");
        assert!(set_value.requires_argument);
        assert_eq!(
            set_value.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPut
        );
        let set_value_ref = binding
            .member_specs
            .get(&super::TEST_DISPID_SET_VALUE_REF)
            .expect("member spec for SetValueRef should be present");
        assert_eq!(set_value_ref.name, "SetValueRef");
        assert!(set_value_ref.requires_argument);
        assert_eq!(
            set_value_ref.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPutRef
        );
        let value = binding
            .member_specs
            .get(&super::TEST_DISPID_VALUE)
            .expect("member spec for Value should be present");
        assert_eq!(value.name, "Value");
        assert!(!value.requires_argument);
        assert_eq!(
            value.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let dispatch_event = binding
            .event_specs
            .get(&super::TEST_EVENT_CHANGED_PAIR)
            .expect("ChangedPair event spec should be present");
        assert_eq!(dispatch_event.callback_arity, 2);
        assert_eq!(dispatch_event.path, super::ComEventPath::Dispatch);
        assert_eq!(
            dispatch_event.connection_point_iid.as_deref(),
            Some(super::IID_OXVBA_TEST_DISPATCH_EVENTS_STR)
        );
        assert_eq!(
            dispatch_event.dispatch_member_id,
            Some(super::TEST_EVENT_CHANGED_PAIR)
        );
        let source_interface_event = binding
            .event_specs
            .get(&super::TEST_EVENT_CHANGED_SOURCE_INTERFACE)
            .expect("source-interface event spec should be present");
        assert_eq!(source_interface_event.callback_arity, 1);
        assert_eq!(
            source_interface_event.path,
            super::ComEventPath::SourceInterface
        );
        assert_eq!(
            source_interface_event.connection_point_iid.as_deref(),
            Some(super::IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR)
        );
        assert!(source_interface_event.dispatch_member_id.is_none());
        let fire_changed_trigger = binding
            .event_trigger_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED)
            .expect("FireChanged trigger spec should be present");
        assert_eq!(fire_changed_trigger.event_token, super::TEST_EVENT_CHANGED);
        assert_eq!(fire_changed_trigger.callback_arity, 1);
        assert!(!fire_changed_trigger.second_arg_is_incremented);
        let fire_changed_pair_trigger = binding
            .event_trigger_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_PAIR)
            .expect("FireChangedPair trigger spec should be present");
        assert_eq!(
            fire_changed_pair_trigger.event_token,
            super::TEST_EVENT_CHANGED_PAIR
        );
        assert_eq!(fire_changed_pair_trigger.callback_arity, 2);
        assert!(fire_changed_pair_trigger.second_arg_is_incremented);
        let fire_changed_source_trigger = binding
            .event_trigger_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE)
            .expect("FireChangedSourceInterface trigger spec should be present");
        assert_eq!(
            fire_changed_source_trigger.event_token,
            super::TEST_EVENT_CHANGED_SOURCE_INTERFACE
        );
        assert_eq!(fire_changed_source_trigger.callback_arity, 1);
        assert!(!fire_changed_source_trigger.second_arg_is_incremented);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_dictionary_binding_includes_event_projection_metadata() {
        let mut policy = HostPolicy::interactive_dev();
        policy
            .com_prog_id_overrides
            .insert(4, "Scripting.Dictionary".to_string());
        let host = StandardHostServices::new(HalProfileId::Windows, policy);
        let object = host
            .create_object(4)
            .expect("create_object should return dictionary token");
        let state = host
            .com_state
            .lock()
            .expect("com state lock should succeed");
        let binding = state
            .bindings
            .get(&object)
            .expect("binding should be present for dictionary token");
        let exists_member = binding
            .member_specs
            .get(&super::TEST_DISPID_EXISTS)
            .expect("Exists member spec should be present");
        assert_eq!(exists_member.name, "Exists");
        assert!(exists_member.requires_argument);
        assert_eq!(
            exists_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let exists_event = binding
            .event_specs
            .get(&super::TEST_EVENT_CHANGED)
            .expect("dictionary projection event spec should be present");
        assert_eq!(exists_event.callback_arity, 1);
        assert_eq!(exists_event.path, super::ComEventPath::Dispatch);
        assert!(exists_event.connection_point_iid.is_none());
        let exists_trigger = binding
            .event_trigger_specs
            .get(&super::TEST_DISPID_EXISTS)
            .expect("Exists member should project callback trigger");
        assert_eq!(exists_trigger.event_token, super::TEST_EVENT_CHANGED);
        assert_eq!(exists_trigger.callback_arity, 1);
        assert!(!exists_trigger.second_arg_is_incremented);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_dictionary_event_projection_routes_subscription_lifecycle() {
        let mut policy = HostPolicy::interactive_dev();
        policy
            .com_prog_id_overrides
            .insert(4, "Scripting.Dictionary".to_string());
        let host = StandardHostServices::new(HalProfileId::Windows, policy);
        let object = host
            .create_object(4)
            .expect("create_object should return dictionary token");
        let subscription = host
            .subscribe_event(object, super::TEST_EVENT_CHANGED)
            .expect("subscribe_event should succeed for dictionary projection event");
        assert!(
            subscription >= 40_001,
            "subscription token should be in deterministic range"
        );
        {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            let registered = state
                .subscriptions
                .get(&subscription)
                .expect("dictionary projection subscription should be tracked");
            assert!(
                matches!(
                    registered.transport,
                    super::ComEventSubscriptionTransport::Projection
                ),
                "dictionary lane should continue to use projection transport"
            );
        }
        assert_eq!(
            host.dispatch_invoke(object, super::TEST_DISPID_EXISTS, 42)
                .expect("Exists invoke should succeed"),
            0
        );
        let callback = host
            .do_events()
            .expect("do_events should return queued dictionary callback");
        assert!(callback >= 60_001, "callback token should be in range");
        assert_eq!(
            host.event_callback_subscription(callback)
                .expect("callback subscription lookup should succeed"),
            subscription
        );
        assert_eq!(
            host.event_callback_arg(callback, 0)
                .expect("callback arg lookup should succeed"),
            42
        );
        host.release_event_callback(callback)
            .expect("callback release should succeed");
        host.unsubscribe_event(subscription)
            .expect("unsubscribe_event should succeed");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_hresult_mapping_labels_are_stable() {
        assert_eq!(
            super::map_com_hresult_label(Some(0x8004_0154), None),
            "class-not-registered"
        );
        assert_eq!(
            super::map_com_hresult_label(Some(0x8002_0003), None),
            "member-not-found"
        );
        assert_eq!(
            super::map_com_hresult_label(Some(0x8004_01F3), None),
            "invalid-class-string"
        );
        assert_eq!(super::map_com_hresult_label(None, Some(0)), "arg-error");
        assert_eq!(
            super::map_com_hresult_label(None, None),
            "fault-unspecified"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_com_policy_override_resolves_native_mapping() {
        let mut policy = HostPolicy::interactive_dev();
        policy
            .com_prog_id_overrides
            .insert(4, "Scripting.Dictionary".to_string());
        let host = StandardHostServices::new(HalProfileId::Windows, policy);
        let object = host
            .create_object(4)
            .expect("policy override should resolve native COM activation");
        assert!(
            object >= 20_001,
            "expected native COM object handle from policy override, got {object}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_createobject_failure_includes_stable_label_when_class_missing() {
        let mut policy = HostPolicy::interactive_dev();
        policy
            .com_prog_id_overrides
            .insert(4, "OxVba.DoesNotExist.Component".to_string());
        let host = StandardHostServices::new(HalProfileId::Windows, policy);
        let err = host
            .create_object(4)
            .expect_err("missing class should fail create_object");
        assert!(
            err.message
                .contains("com-createobject-class-not-registered")
                || err
                    .message
                    .contains("com-createobject-invalid-class-string")
                || err.message.contains("0x80040154"),
            "expected stable class-not-registered label, got {}",
            err.message
        );
    }

    proptest! {
        #[test]
        fn prop_free_file_low_range_tracks_open_count(path_seed in 1i32..10_000, open_count in 0usize..32) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            for idx in 0..open_count {
                let path = path_seed.saturating_add(idx as i32);
                let _ = host.open(path, 0).expect("open should succeed");
            }
            let expected = 1 + open_count as i32;
            let free = host.free_file(0).expect("free_file should succeed");
            prop_assert_eq!(free, expected);
        }

        #[test]
        fn prop_seek_eof_boundary(path_token in 1i32..10_000, offset in 0i32..6000) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            let handle = host.open(path_token, 0).expect("open should succeed");
            let len = host.lof(handle).expect("lof should succeed");
            host.seek(handle, offset).expect("seek should succeed");
            let eof = host.eof(handle).expect("eof should succeed");
            let expected = if offset >= len { 1 } else { 0 };
            prop_assert_eq!(eof, expected);
        }

        #[test]
        fn prop_ui_virtualization_projection_is_stable(prompt in any::<i32>(), style in any::<i32>(), default_value in any::<i32>()) {
            let scripted = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: true,
                    ui_virtualization: crate::model::UiVirtualizationMode::ScriptedResponses,
                    ..HostPolicy::default()
                },
            );
            prop_assert_eq!(
                scripted.msg_box(prompt, style).expect("scripted msg_box"),
                style.max(1)
            );
            prop_assert_eq!(
                scripted.input_box(prompt, default_value).expect("scripted input_box"),
                default_value
            );

            let disabled = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: true,
                    ui_virtualization: crate::model::UiVirtualizationMode::Disabled,
                    ..HostPolicy::default()
                },
            );
            prop_assert_eq!(
                disabled.msg_box(prompt, style).expect("disabled msg_box"),
                prompt.max(1)
            );
            prop_assert_eq!(
                disabled.input_box(prompt, default_value).expect("disabled input_box"),
                prompt
            );
        }

        #[test]
        fn prop_host_sensitive_policy_denials_are_stable(
            shell_cmd in any::<i32>(),
            prog_id in any::<i32>(),
            symbol in any::<i32>(),
            arg in any::<i32>()
        ) {
            let host = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: false,
                    allow_process_spawn: false,
                    allow_com_activation: false,
                    allow_dynamic_link: false,
                    ..HostPolicy::default()
                },
            );

            prop_assert_eq!(
                host.msg_box(1, 1).expect_err("msg_box denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.shell(shell_cmd, 0).expect_err("shell denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.create_object(prog_id).expect_err("create_object denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.invoke_symbol(symbol, arg).expect_err("invoke_symbol denied").kind,
                HalErrorKind::PolicyDenied
            );
        }

        #[test]
        fn prop_process_com_dynlink_projection_is_stable(
            shell_cmd in any::<i32>(),
            prog_id in any::<i32>(),
            object in any::<i32>(),
            member in any::<i32>(),
            arg in any::<i32>(),
            symbol in any::<i32>()
        ) {
            let host = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: true,
                    allow_process_spawn: true,
                    allow_com_activation: true,
                    allow_dynamic_link: true,
                    ..HostPolicy::default()
                },
            );

            let shell_expected = if shell_cmd == 0 { 0 } else { 1 };
            prop_assert_eq!(
                host.shell(shell_cmd, 0).expect("shell should succeed"),
                shell_expected
            );
            prop_assert_eq!(
                host.create_object(prog_id).expect("create_object should succeed"),
                5_000i32.saturating_add(prog_id)
            );
            prop_assert_eq!(
                host.dispatch_invoke(object, member, arg)
                    .expect("dispatch_invoke should succeed"),
                object.saturating_add(member).saturating_add(arg)
            );
            prop_assert_eq!(
                host.invoke_symbol(symbol, arg).expect("invoke_symbol should succeed"),
                symbol.saturating_add(arg)
            );
        }
    }
}
