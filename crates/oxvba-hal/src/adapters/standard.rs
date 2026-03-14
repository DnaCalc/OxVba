use crate::{
    error::{HalError, HalResult},
    model::{
        CapabilityDescriptor, CapabilityId, CapabilityMaturity, ComInvocationStrategy,
        HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy, UiVirtualizationMode,
        WasmRuntimeClass, host_backed_mode_active,
    },
    traits::{
        ComHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal, FileSystemHal,
        HostServices, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibMemberInvokeKind,
        TypeLibMetadataBlob, TypeLibResolveRequest, TypeLibResolvedIdentity, UiInteractionHal,
    },
};
#[cfg(test)]
pub use oxvba_com::DISPATCH_INVOKE_MISSING_ARG_TOKEN;
#[cfg(target_os = "windows")]
use oxvba_com::invoke_dispatch_runtime_value as com_invoke_dispatch_runtime_value;
#[cfg(target_os = "windows")]
use oxvba_com::take_excepinfo;
#[cfg(target_os = "windows")]
use oxvba_com::windows_variant::{
    set_variant_from_com_value as com_set_variant_from_com_value,
    variant_to_com_value as com_variant_to_com_value,
};
use oxvba_com::{
    COM_DISP_E_PARAMNOTFOUND, COM_DISPID_PROPERTYPUT, ComBinding, ComCallbackPayload,
    ComCallbackToken, ComDirectDispatchSpec, ComEventPath, ComEventSpec,
    ComEventSubscription as SharedComEventSubscription, ComEventTriggerSpec, ComInvokeArg,
    ComInvokeFailure, ComInvokeRequest, ComMemberSpec, ComMemberToken, ComObjectDescriptor,
    ComObjectToken, ComObjectTransportKind, ComSubscriptionToken, ComValue, IID_NULL, RawIDispatch,
    RawIUnknown, TypeLibMetadataCacheState, WindowsComClientState, WindowsComSubscriptionTransport,
    activate_runtime_dispatch as com_activate_runtime_dispatch,
    add_ref_dispatch as raw_add_ref_dispatch, advise_event_subscription,
    bind_native_dispatch_result as com_bind_native_dispatch_result, binding_from_typelib_metadata,
    build_typelib_metadata, cache_member_dispid as com_cache_member_dispid,
    callback_arg as com_callback_arg, callback_arity as com_callback_arity,
    callback_subscription_token as com_callback_subscription_token,
    canonicalize_member_known_args as com_canonicalize_member_known_args,
    event_callback_args_from_member_token, event_is_source_interface_only,
    event_signature_arity_for_binding, get_dispid_by_name as raw_get_dispid_by_name,
    insert_bound_object_binding as com_insert_bound_object_binding,
    known_typelib_identity_for_prog_id_name,
    legacy_runtime_arg_values as com_legacy_runtime_arg_values, map_com_hresult_label,
    member_spec_from_typelib_metadata, plan_bound_runtime_invoke as com_plan_bound_runtime_invoke,
    plan_unbound_runtime_invoke as com_plan_unbound_runtime_invoke,
    query_dispatch_from_unknown as raw_query_dispatch_from_unknown,
    raw_oxvba_test_dispatch_vtable_invoke, release_callback as com_release_callback,
    release_dispatch as raw_release_dispatch, release_object_binding as com_release_object_binding,
    release_subscription_transport,
    remove_subscription_callbacks as com_remove_subscription_callbacks,
    resolve_bound_native_dispatch as com_resolve_bound_native_dispatch,
    resolve_known_typelib_identity,
    resolve_named_argument_dispids as com_resolve_named_argument_dispids,
    resolve_subscription_transport as com_resolve_subscription_transport,
    take_polled_callback_payload as com_take_polled_callback_payload,
    validate_named_arg_order as com_validate_named_arg_order,
};
use oxvba_runtime::{BindingHandle, DynLinkSymbol, ObjectHandle, RuntimeValue, bstr::BStr};
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
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, DISPATCH_METHOD, DISPATCH_PROPERTYGET,
    DISPATCH_PROPERTYPUT, DISPATCH_PROPERTYPUTREF, DISPPARAMS, EXCEPINFO,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Variant::{VARIANT, VT_ERROR, VariantClear};
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

#[cfg(target_os = "windows")]
const OXVBA_TEST_DISPATCH_PROGID: &str = "OxVba.TestDispatch";
#[cfg(target_os = "windows")]
#[cfg(test)]
const IID_OXVBA_TEST_DISPATCH_EVENTS_STR: &str = "11111112-2222-3333-4444-555555555556";
#[cfg(target_os = "windows")]
#[cfg(test)]
const IID_OXVBA_TEST_DISPATCH_SOURCE_EVENTS_STR: &str = "11111113-2222-3333-4444-555555555557";
#[cfg(target_os = "windows")]
#[cfg(test)]
const IID_EXCEL_APPLICATION_EVENTS_STR: &str = "00024413-0000-0000-C000-000000000046";
#[cfg(test)]
const TEST_DISPID_COUNT: i32 = 1;
#[cfg(test)]
const TEST_DISPID_EXISTS: i32 = 2;
#[cfg(test)]
const TEST_DISPID_FIRE_CHANGED: i32 = 3;
#[cfg(test)]
const TEST_DISPID_FIRE_CHANGED_PAIR: i32 = 4;
#[cfg(test)]
const TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE: i32 = 11;
#[cfg(test)]
const TEST_DISPID_PING: i32 = 5;
#[cfg(test)]
const TEST_DISPID_LOOKUP: i32 = 6;
#[cfg(test)]
const TEST_DISPID_SET_VALUE: i32 = 7;
#[cfg(test)]
const TEST_DISPID_SET_VALUE_REF: i32 = 8;
#[cfg(test)]
const TEST_DISPID_VALUE: i32 = 9;
#[cfg(test)]
const TEST_DISPID_EXCEL_QUIT: i32 = 10;
#[cfg(test)]
const TEST_DISPID_SUM_PAIR: i32 = 12;
#[cfg(test)]
const TEST_DISPID_LOOKUP_PAIR: i32 = 13;
#[cfg(test)]
const TEST_DISPID_SET_INDEXED_VALUE: i32 = 14;
#[cfg(test)]
const TEST_DISPID_SET_INDEXED_VALUE_REF: i32 = 15;
const TEST_DISPID_ECHO_VARIANT: i32 = 16;
#[cfg(test)]
const TEST_DISPID_RAISE_EXCEPTION: i32 = 17;
#[cfg(test)]
const TEST_DISPID_RETURN_SMALLINT: i32 = 18;
#[cfg(test)]
const TEST_DISPID_RETURN_UNSIGNED_WORD: i32 = 19;
#[cfg(test)]
const TEST_EVENT_CHANGED: i32 = 1;
#[cfg(test)]
const TEST_EVENT_CHANGED_SOURCE_INTERFACE: i32 = 2;
#[cfg(test)]
const TEST_EVENT_CHANGED_PAIR: i32 = 3;
#[cfg(test)]
const TEST_EVENT_EXCEL_APP_QUIT: i32 = 10;
const COM_EVENT_DISPATCH_MEMBER_WILDCARD: i32 = i32::MIN + 3_333;

#[derive(Debug, Clone)]
pub(crate) struct StandardHostServices {
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    descriptor: HalDescriptor,
    policy: HostPolicy,
    env_cache: StandardEnvCache,
    fs_state: Arc<Mutex<FileSystemState>>,
    com_state: Arc<Mutex<ComState>>,
    typelib_state: Arc<Mutex<TypeLibraryCacheState>>,
    dynlink_state: Arc<Mutex<DynLinkBindingState>>,
}

#[derive(Debug, Clone, Default)]
struct StandardEnvCache {
    native_com_prog_id_overrides: BTreeMap<i32, String>,
    registered_com_prog_id: Option<String>,
    registered_event_token: Option<String>,
    registered_event_expected_argc: Option<String>,
    registered_event_path: Option<String>,
    registered_event_connection_point_iid: Option<String>,
    registered_event_dispatch_member: Option<String>,
    registered_event_trigger_member: Option<String>,
    registered_event_trigger_requires_arg: Option<String>,
    registered_event_trigger_invoke_kind: Option<String>,
    force_registered_testdispatch: bool,
}

impl StandardEnvCache {
    fn capture() -> Self {
        let vars: Vec<(String, String)> = std::env::vars().collect();
        let mut native_com_prog_id_overrides = BTreeMap::new();
        for (key, value) in &vars {
            let Some(suffix) = key.strip_prefix("OXVBA_COM_PROGID_") else {
                continue;
            };
            let Ok(token) = suffix.parse::<i32>() else {
                continue;
            };
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                native_com_prog_id_overrides.insert(token, trimmed.to_string());
            }
        }
        Self {
            native_com_prog_id_overrides,
            registered_com_prog_id: cached_env_value(&vars, "OXVBA_REGISTERED_COM_PROGID"),
            registered_event_token: cached_env_value(&vars, "OXVBA_REGISTERED_EVENT_TOKEN"),
            registered_event_expected_argc: cached_env_value(
                &vars,
                "OXVBA_REGISTERED_EVENT_EXPECTED_ARGC",
            ),
            registered_event_path: cached_env_value(&vars, "OXVBA_REGISTERED_EVENT_PATH"),
            registered_event_connection_point_iid: cached_env_value(
                &vars,
                "OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID",
            ),
            registered_event_dispatch_member: cached_env_value(
                &vars,
                "OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER",
            ),
            registered_event_trigger_member: cached_env_value(
                &vars,
                "OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER",
            ),
            registered_event_trigger_requires_arg: cached_env_value(
                &vars,
                "OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG",
            ),
            registered_event_trigger_invoke_kind: cached_env_value(
                &vars,
                "OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND",
            ),
            force_registered_testdispatch: cached_env_value(
                &vars,
                "OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH",
            )
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false),
        }
    }
}

fn cached_env_value(vars: &[(String, String)], key: &str) -> Option<String> {
    vars.iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
            env_cache: StandardEnvCache::capture(),
            fs_state: Arc::new(Mutex::new(FileSystemState::default())),
            com_state: Arc::new(Mutex::new(ComState::default())),
            typelib_state: Arc::new(Mutex::new(TypeLibraryCacheState::default())),
            dynlink_state: Arc::new(Mutex::new(DynLinkBindingState::default())),
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
                    handle.raw() >= 20_001,
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
                    subscription.raw() >= 40_001,
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
                    callback.raw() >= 60_001,
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
        resolve_known_typelib_identity(request)
    }

    fn build_typelib_metadata(&self, identity: &TypeLibResolvedIdentity) -> TypeLibMetadataBlob {
        build_typelib_metadata(identity)
    }

    #[cfg(target_os = "windows")]
    fn known_typelib_identity_for_prog_id_name(
        &self,
        prog_id_name: &str,
    ) -> Option<TypeLibResolvedIdentity> {
        known_typelib_identity_for_prog_id_name(prog_id_name)
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
        Ok(Some(state.load_or_build(&identity, |identity| {
            self.build_typelib_metadata(identity)
        })))
    }

    #[cfg(not(target_os = "windows"))]
    fn load_typelib_metadata_for_prog_id_name(
        &self,
        _prog_id_name: &str,
    ) -> HalResult<Option<TypeLibMetadataBlob>> {
        Ok(None)
    }

    #[cfg(target_os = "windows")]
    fn known_member_spec_for_prog_id_name(
        &self,
        prog_id_name: &str,
        member: ComMemberToken,
    ) -> HalResult<Option<ComMemberSpec>> {
        Ok(self
            .load_typelib_metadata_for_prog_id_name(prog_id_name)?
            .as_ref()
            .and_then(|blob| member_spec_from_typelib_metadata(blob, member)))
    }

    #[cfg(not(target_os = "windows"))]
    fn known_member_spec_for_prog_id_name(
        &self,
        _prog_id_name: &str,
        _member: ComMemberToken,
    ) -> HalResult<Option<ComMemberSpec>> {
        Ok(None)
    }

    #[cfg(target_os = "windows")]
    fn registered_event_override_for_prog_id_name(
        &self,
        prog_id_name: &str,
        op: &'static str,
    ) -> HalResult<Option<RegisteredEventOverrideConfig>> {
        let Some(configured_prog_id) = self.env_cache.registered_com_prog_id.as_deref() else {
            return Ok(None);
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
        let path = match self
            .env_cache
            .registered_event_path
            .as_deref()
            .map(|value| value.to_ascii_lowercase())
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
        let connection_point_iid = self.env_cache.registered_event_connection_point_iid.clone();
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
        let raw = match key {
            "OXVBA_REGISTERED_EVENT_TOKEN" => self.env_cache.registered_event_token.as_deref(),
            "OXVBA_REGISTERED_EVENT_EXPECTED_ARGC" => {
                self.env_cache.registered_event_expected_argc.as_deref()
            }
            "OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER" => {
                self.env_cache.registered_event_dispatch_member.as_deref()
            }
            "OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER" => {
                self.env_cache.registered_event_trigger_member.as_deref()
            }
            _ => None,
        };
        let Some(raw) = raw else {
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
        let raw = match key {
            "OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG" => self
                .env_cache
                .registered_event_trigger_requires_arg
                .as_deref(),
            _ => None,
        };
        let Some(raw) = raw else {
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
        let raw = match key {
            "OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND" => self
                .env_cache
                .registered_event_trigger_invoke_kind
                .as_deref(),
            _ => None,
        };
        let Some(raw) = raw else {
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
            override_cfg.event_token.into(),
            ComEventSpec {
                callback_arity: override_cfg.callback_arity,
                path: override_cfg.path,
                connection_point_iid: override_cfg.connection_point_iid.clone(),
                dispatch_member_id: override_cfg.dispatch_member_id,
            },
        );
        if let Some(trigger_member) = override_cfg.trigger_member {
            binding.direct_dispatch_specs.insert(
                trigger_member.into(),
                ComDirectDispatchSpec {
                    invoke_kind: override_cfg.trigger_invoke_kind,
                    requires_argument: override_cfg.trigger_requires_argument,
                },
            );
            binding.event_trigger_specs.insert(
                trigger_member.into(),
                ComEventTriggerSpec {
                    event_token: override_cfg.event_token.into(),
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

    fn runtime_value_to_legacy_i32(
        &self,
        value: &RuntimeValue,
        capability: CapabilityId,
        op: &'static str,
        field: &'static str,
    ) -> HalResult<i32> {
        value.to_legacy_i32().map_err(|detail| {
            HalError::adapter_fault(
                self.profile,
                capability,
                op,
                format!("{field} cannot enter the legacy runtime token lane: {detail}"),
            )
        })
    }

    fn runtime_value_to_display_text(&self, value: &RuntimeValue) -> String {
        match value {
            RuntimeValue::String(BStr(text)) => text.clone(),
            RuntimeValue::Bool(value) => {
                if *value {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            }
            RuntimeValue::Empty => String::new(),
            RuntimeValue::Null => "Null".to_string(),
            RuntimeValue::ErrorCode(code) => format!("Error {code}"),
            RuntimeValue::I32(value) => value.to_string(),
            RuntimeValue::ArrayIntent(array) => format!("<array:{}>", array.len),
            RuntimeValue::ObjectHandle(handle) => format!("<object:{handle}>"),
            RuntimeValue::BindingHandle(handle) => format!("<binding:{handle}>"),
        }
    }

    fn runtime_value_to_path(
        &self,
        value: &RuntimeValue,
        capability: CapabilityId,
        op: &'static str,
        field: &'static str,
    ) -> HalResult<PathBuf> {
        match value {
            RuntimeValue::String(BStr(path)) => Ok(PathBuf::from(path)),
            other => self
                .runtime_value_to_legacy_i32(other, capability, op, field)
                .map(|token| self.host_path_from_token(token)),
        }
    }

    #[cfg(target_os = "windows")]
    fn spawn_probe_shell_process(&self, command: i32) -> std::io::Result<std::process::Child> {
        Command::new("cmd")
            .args(["/C", &format!("echo OXVBA_HAL_{command} > NUL")])
            .spawn()
    }

    #[cfg(target_os = "windows")]
    fn spawn_probe_shell_process_text(
        &self,
        command: &str,
    ) -> std::io::Result<std::process::Child> {
        Command::new("cmd").args(["/C", command]).spawn()
    }

    #[cfg(not(target_os = "windows"))]
    fn spawn_probe_shell_process(&self, _command: i32) -> std::io::Result<std::process::Child> {
        Command::new("sh").arg("-c").arg("true").spawn()
    }

    #[cfg(not(target_os = "windows"))]
    fn spawn_probe_shell_process_text(
        &self,
        command: &str,
    ) -> std::io::Result<std::process::Child> {
        Command::new("sh").arg("-c").arg(command).spawn()
    }

    #[cfg(target_os = "windows")]
    fn native_windows_msg_box_value(&self, prompt: &RuntimeValue, style: i32) -> HalResult<i32> {
        let text = self.runtime_value_to_display_text(prompt);
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
    fn native_windows_msg_box_value(&self, _prompt: &RuntimeValue, _style: i32) -> HalResult<i32> {
        Ok(1)
    }

    #[cfg(target_os = "windows")]
    fn pump_windows_messages_once(&self) {
        // SAFETY: MSG is a plain old data Win32 struct that is immediately initialized by
        // PeekMessageW before any field reads.
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: The message loop uses the current thread queue only and the MSG pointer stays
        // valid for the duration of the Win32 calls.
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
            // SAFETY: We initialize COM for the current thread only once and request the STA
            // apartment model required by the current Windows COM adapter.
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
        if let Some(value) = self.env_cache.native_com_prog_id_overrides.get(&prog_id) {
            return Some(value.clone());
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
        self.env_cache
            .native_com_prog_id_overrides
            .contains_key(&prog_id)
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
    fn resolve_named_argument_dispids(
        &self,
        dispatch: *mut RawIDispatch,
        member_name: &str,
        args: &[ComInvokeArg],
    ) -> HalResult<Vec<i32>> {
        unsafe { com_resolve_named_argument_dispids(dispatch, member_name, args) }
            .map_err(|message| self.com_dispatch_adapter_fault(message))
    }

    #[cfg(target_os = "windows")]
    fn native_com_activate_dispatch(&self, prog_id: &str) -> HalResult<*mut RawIDispatch> {
        com_activate_runtime_dispatch(prog_id, self.force_registered_test_dispatch())
            .map_err(|message| self.com_createobject_adapter_fault(message))
    }

    #[cfg(target_os = "windows")]
    fn force_registered_test_dispatch(&self) -> bool {
        self.env_cache.force_registered_testdispatch
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
    fn try_binding_from_typelib_metadata(&self, prog_id: &str) -> HalResult<RawDispatchPtr> {
        self.ensure_thread_com_apartment("create_object")?;
        self.native_com_activate_dispatch(prog_id)
            .map(|dispatch| dispatch as RawDispatchPtr)
    }

    #[cfg(target_os = "windows")]
    fn resolve_native_dispatch_for_object_arg(
        &self,
        object: ObjectHandle,
        op: &'static str,
    ) -> HalResult<*mut RawIDispatch> {
        let capability = CapabilityId::ComActivationDispatch;
        let state = self.com_lock(capability, op)?;
        com_resolve_bound_native_dispatch(&state, object)
            .map_err(|message| HalError::adapter_fault(self.profile, capability, op, message))
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
    fn try_binding_from_typelib_metadata(&self, prog_id: &str) -> HalResult<RawDispatchPtr> {
        self.native_com_activate_dispatch(prog_id)
            .map(|dispatch| dispatch as RawDispatchPtr)
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_core(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id: &str,
        member: i32,
        args: &[ComInvokeArg],
    ) -> HalResult<i32> {
        if let Some(spec) = self.known_member_spec_for_prog_id_name(prog_id, member.into())? {
            // SAFETY: `dispatch` is a live IDispatch pointer owned by this adapter and `spec.name`
            // is converted to a temporary wide buffer inside the helper.
            let dispid = unsafe { raw_get_dispid_by_name(dispatch, &spec.name) }
                .map_err(|message| self.com_dispatch_adapter_fault(message))?;
            return self.native_com_dispatch_invoke_with_member_spec(dispatch, dispid, &spec, args);
        }
        if args.iter().any(|arg| arg.name.is_some()) {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                "named arguments require a resolved COM member name and remain unsupported for default-member/direct-DISPID dispatch",
            ));
        }
        let mut resolve_object = |handle: ObjectHandle| {
            self.resolve_native_dispatch_for_object_arg(handle, "dispatch_invoke")
                .map_err(|err| format!("{} [{}] {}", err.stable_code, err.operation, err.message))
        };
        // SAFETY: `dispatch` is a live IDispatch pointer and `member` is treated as a direct
        // DISPID for the controlled late-bound fallback path.
        unsafe {
            raw_dispatch_property_get_i4_args(dispatch, member, args, &[], &mut resolve_object)
        }
        .map_err(|failure| self.com_dispatch_invoke_fault(failure))
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
        let member = ComMemberToken::new(member);
        let spec = if let Some(spec) = com_member_spec_for_binding(binding, member) {
            spec
        } else if let Some(spec) =
            self.known_member_spec_for_prog_id_name(&binding.prog_id_name, member)?
        {
            spec
        } else {
            return Ok(None);
        };
        if let Some(dispid) = cached {
            return Ok(Some((dispid, spec)));
        }
        // SAFETY: `dispatch` is a live IDispatch pointer owned by this adapter and `spec.name`
        let dispid = unsafe { raw_get_dispid_by_name(dispatch, &spec.name) }
            .map_err(|message| self.com_dispatch_adapter_fault(message))?;
        let mut state = self.com_lock(CapabilityId::ComActivationDispatch, "dispatch_invoke")?;
        com_cache_member_dispid(&mut state, ObjectHandle::new(object), member, dispid);
        self.assert_com_invariants(&state, "dispatch_invoke_cache_update");
        Ok(Some((dispid, spec)))
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_with_member_spec(
        &self,
        dispatch: *mut RawIDispatch,
        dispid: i32,
        spec: &ComMemberSpec,
        args: &[ComInvokeArg],
    ) -> HalResult<i32> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        let canonical_args;
        let args = match spec.invoke_kind {
            TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef => {
                canonical_args =
                    com_canonicalize_member_known_args(spec, args).map_err(|message| {
                        HalError::adapter_fault(
                            self.profile,
                            CapabilityId::ComActivationDispatch,
                            "dispatch_invoke",
                            message,
                        )
                    })?;
                canonical_args.as_slice()
            }
            _ => args,
        };
        if spec.requires_argument {
            if args.iter().all(|arg| arg.value.is_none()) {
                return Err(HalError::adapter_fault(
                    self.profile,
                    CapabilityId::ComActivationDispatch,
                    "dispatch_invoke",
                    "member requires argument but DispatchInvoke omitted the third argument",
                ));
            }
        } else {
            let mut resolve_object = |handle: ObjectHandle| {
                self.resolve_native_dispatch_for_object_arg(handle, "dispatch_invoke")
                    .map_err(|err| {
                        format!("{} [{}] {}", err.stable_code, err.operation, err.message)
                    })
            };
            match spec.invoke_kind {
                TypeLibMemberInvokeKind::PropertyGet => {
                    // SAFETY: `dispatch` is a live IDispatch pointer and `dispid` was resolved for
                    // this member on the same interface.
                    return unsafe {
                        raw_dispatch_property_get_i4_args(
                            dispatch,
                            dispid,
                            &[],
                            &[],
                            &mut resolve_object,
                        )
                    }
                    .map_err(|failure| self.com_dispatch_invoke_fault(failure));
                }
                TypeLibMemberInvokeKind::Method => {
                    // SAFETY: `dispatch` is a live IDispatch pointer and `dispid` targets a method
                    // on the same interface without arguments.
                    return unsafe {
                        raw_dispatch_invoke_method_i4_args(
                            dispatch,
                            dispid,
                            &[],
                            &[],
                            &mut resolve_object,
                        )
                    }
                    .map_err(|failure| self.com_dispatch_invoke_fault(failure));
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
        let mut resolve_object = |handle: ObjectHandle| {
            self.resolve_native_dispatch_for_object_arg(handle, "dispatch_invoke")
                .map_err(|err| format!("{} [{}] {}", err.stable_code, err.operation, err.message))
        };
        match spec.invoke_kind {
            // SAFETY: `dispatch` is a live IDispatch pointer and the helper marshals all invoke
            // arguments into stack-owned VARIANT storage for the Invoke call.
            TypeLibMemberInvokeKind::PropertyGet => unsafe {
                let named_arg_dispids =
                    self.resolve_named_argument_dispids(dispatch, &spec.name, args)?;
                raw_dispatch_property_get_i4_args(
                    dispatch,
                    dispid,
                    args,
                    &named_arg_dispids,
                    &mut resolve_object,
                )
            },
            // SAFETY: Same as above; the helper owns all temporary Automation structures.
            TypeLibMemberInvokeKind::Method => unsafe {
                let named_arg_dispids =
                    self.resolve_named_argument_dispids(dispatch, &spec.name, args)?;
                raw_dispatch_invoke_method_i4_args(
                    dispatch,
                    dispid,
                    args,
                    &named_arg_dispids,
                    &mut resolve_object,
                )
            },
            // SAFETY: Same as above; property-put marshalling uses a stack-local VARIANT.
            TypeLibMemberInvokeKind::PropertyPut => unsafe {
                let named_arg_dispids = self.resolve_named_argument_dispids(
                    dispatch,
                    &spec.name,
                    &args[..args.len() - 1],
                )?;
                raw_dispatch_property_put_i4_args(
                    dispatch,
                    dispid,
                    args,
                    &named_arg_dispids,
                    &mut resolve_object,
                )
            },
            // SAFETY: Same as above; property-putref uses the same validated pointer and argument.
            TypeLibMemberInvokeKind::PropertyPutRef => unsafe {
                let named_arg_dispids = self.resolve_named_argument_dispids(
                    dispatch,
                    &spec.name,
                    &args[..args.len() - 1],
                )?;
                raw_dispatch_property_putref_i4_args(
                    dispatch,
                    dispid,
                    args,
                    &named_arg_dispids,
                    &mut resolve_object,
                )
            },
        }
        .map_err(|failure| self.com_dispatch_invoke_fault(failure))
    }

    #[cfg(target_os = "windows")]
    fn try_native_com_vtable_invoke(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id: &str,
        member: i32,
        args: &[i32],
    ) -> HalResult<Option<i32>> {
        if self.policy.com_invocation_strategy != ComInvocationStrategy::PreferVtable {
            return Ok(None);
        }
        if !prog_id.eq_ignore_ascii_case(OXVBA_TEST_DISPATCH_PROGID) {
            return Ok(None);
        }
        if member == TEST_DISPID_ECHO_VARIANT {
            return Ok(None);
        }
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        // SAFETY: `dispatch` points at the controlled OxVba.TestDispatch implementation, so the
        // helper may downcast to the known vtable layout for the prefer-vtable lane.
        let result = unsafe { raw_oxvba_test_dispatch_vtable_invoke(dispatch, member, args) }
            .map_err(|message| self.com_dispatch_adapter_fault(message))?;
        Ok(result)
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke(
        &self,
        prog_id: &str,
        member: i32,
        args: &[ComInvokeArg],
    ) -> HalResult<i32> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        let dispatch = self.native_com_activate_dispatch(prog_id)?;
        let result = self.native_com_dispatch_invoke_core(dispatch, prog_id, member, args);
        // SAFETY: `dispatch` was returned by native_com_activate_dispatch and has not been
        // released yet; this balances the adapter-owned AddRef from activation.
        unsafe {
            raw_release_dispatch(dispatch);
        }
        result
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_core_runtime_value(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id: &str,
        member: i32,
        args: &[ComInvokeArg],
    ) -> HalResult<RuntimeValue> {
        let plan = com_plan_unbound_runtime_invoke(
            member.into(),
            args,
            self.known_member_spec_for_prog_id_name(prog_id, member.into())?,
        )
        .map_err(|message| {
            HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                message,
            )
        })?;
        match plan {
            oxvba_com::UnboundRuntimeInvokePlan::MemberSpec(spec) => {
                let dispid = unsafe { raw_get_dispid_by_name(dispatch, &spec.name) }
                    .map_err(|message| self.com_dispatch_adapter_fault(message))?;
                self.native_com_dispatch_invoke_with_member_spec_runtime_value(
                    dispatch, dispid, &spec, args, prog_id,
                )
            }
            oxvba_com::UnboundRuntimeInvokePlan::DirectPropertyGet { dispid } => unsafe {
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid.raw(),
                    DISPATCH_PROPERTYGET,
                    args,
                    &[],
                    ("property-get", prog_id),
                )
            }
            .map_err(|failure| self.com_dispatch_invoke_fault(failure)),
        }
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_with_member_spec_runtime_value(
        &self,
        dispatch: *mut RawIDispatch,
        dispid: i32,
        spec: &ComMemberSpec,
        args: &[ComInvokeArg],
        prog_id_hint: &str,
    ) -> HalResult<RuntimeValue> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        let canonical_args;
        let args = match spec.invoke_kind {
            TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef => {
                canonical_args =
                    com_canonicalize_member_known_args(spec, args).map_err(|message| {
                        HalError::adapter_fault(
                            self.profile,
                            CapabilityId::ComActivationDispatch,
                            "dispatch_invoke",
                            message,
                        )
                    })?;
                canonical_args.as_slice()
            }
            _ => args,
        };
        if spec.requires_argument && args.iter().all(|arg| arg.value.is_none()) {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                "member requires argument but DispatchInvoke omitted the third argument",
            ));
        }
        if !spec.requires_argument {
            let mut resolve_object = |handle: ObjectHandle| {
                self.resolve_native_dispatch_for_object_arg(handle, "dispatch_invoke")
                    .map_err(|err| {
                        format!("{} [{}] {}", err.stable_code, err.operation, err.message)
                    })
            };
            match spec.invoke_kind {
                TypeLibMemberInvokeKind::PropertyGet => {
                    // SAFETY: `dispatch` is a live IDispatch pointer and `dispid` was resolved for
                    // this member on the same interface.
                    return unsafe {
                        raw_dispatch_property_get_i4_args(
                            dispatch,
                            dispid,
                            &[],
                            &[],
                            &mut resolve_object,
                        )
                    }
                    .map(RuntimeValue::I32)
                    .map_err(|failure| self.com_dispatch_invoke_fault(failure));
                }
                TypeLibMemberInvokeKind::Method => {
                    // SAFETY: `dispatch` is a live IDispatch pointer and `dispid` targets a method
                    // on the same interface without arguments.
                    return unsafe {
                        raw_dispatch_invoke_method_i4_args(
                            dispatch,
                            dispid,
                            &[],
                            &[],
                            &mut resolve_object,
                        )
                    }
                    .map(RuntimeValue::I32)
                    .map_err(|failure| self.com_dispatch_invoke_fault(failure));
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
                let named_arg_dispids =
                    self.resolve_named_argument_dispids(dispatch, &spec.name, args)?;
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_PROPERTYGET,
                    args,
                    &named_arg_dispids,
                    ("property-get", prog_id_hint),
                )
            },
            TypeLibMemberInvokeKind::Method => unsafe {
                let named_arg_dispids =
                    self.resolve_named_argument_dispids(dispatch, &spec.name, args)?;
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_METHOD,
                    args,
                    &named_arg_dispids,
                    ("method", prog_id_hint),
                )
            },
            TypeLibMemberInvokeKind::PropertyPut => unsafe {
                let named_arg_dispids = self.resolve_named_argument_dispids(
                    dispatch,
                    &spec.name,
                    &args[..args.len().saturating_sub(1)],
                )?;
                let mut all_named = named_arg_dispids;
                all_named.push(COM_DISPID_PROPERTYPUT);
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_PROPERTYPUT,
                    args,
                    &all_named,
                    ("property-put", prog_id_hint),
                )
            },
            TypeLibMemberInvokeKind::PropertyPutRef => unsafe {
                let named_arg_dispids = self.resolve_named_argument_dispids(
                    dispatch,
                    &spec.name,
                    &args[..args.len().saturating_sub(1)],
                )?;
                let mut all_named = named_arg_dispids;
                all_named.push(COM_DISPID_PROPERTYPUT);
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_PROPERTYPUTREF,
                    args,
                    &all_named,
                    ("property-putref", prog_id_hint),
                )
            },
        }
        .map_err(|failure| self.com_dispatch_invoke_fault(failure))
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_with_direct_dispid_runtime_value(
        &self,
        dispatch: *mut RawIDispatch,
        dispid: i32,
        invoke_kind: TypeLibMemberInvokeKind,
        requires_argument: bool,
        args: &[ComInvokeArg],
        prog_id_hint: &str,
    ) -> HalResult<RuntimeValue> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        if requires_argument && args.iter().all(|arg| arg.value.is_none()) {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                "member requires argument but DispatchInvoke omitted the third argument",
            ));
        }
        if !requires_argument {
            let mut resolve_object = |handle: ObjectHandle| {
                self.resolve_native_dispatch_for_object_arg(handle, "dispatch_invoke")
                    .map_err(|err| {
                        format!("{} [{}] {}", err.stable_code, err.operation, err.message)
                    })
            };
            return match invoke_kind {
                TypeLibMemberInvokeKind::PropertyGet => unsafe {
                    raw_dispatch_property_get_i4_args(
                        dispatch,
                        dispid,
                        &[],
                        &[],
                        &mut resolve_object,
                    )
                },
                TypeLibMemberInvokeKind::Method => unsafe {
                    raw_dispatch_invoke_method_i4_args(
                        dispatch,
                        dispid,
                        &[],
                        &[],
                        &mut resolve_object,
                    )
                },
                TypeLibMemberInvokeKind::PropertyPut | TypeLibMemberInvokeKind::PropertyPutRef => {
                    return Err(HalError::adapter_fault(
                        self.profile,
                        CapabilityId::ComActivationDispatch,
                        "dispatch_invoke",
                        "member requires argument for property put/putref dispatch",
                    ));
                }
            }
            .map(RuntimeValue::I32)
            .map_err(|failure| self.com_dispatch_invoke_fault(failure));
        }
        if args.iter().any(|arg| arg.name.is_some()) {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                "named arguments require a resolved COM member name and are unsupported for direct-DISPID dispatch",
            ));
        }
        match invoke_kind {
            TypeLibMemberInvokeKind::PropertyGet => unsafe {
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_PROPERTYGET,
                    args,
                    &[],
                    ("property-get", prog_id_hint),
                )
            },
            TypeLibMemberInvokeKind::Method => unsafe {
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_METHOD,
                    args,
                    &[],
                    ("method", prog_id_hint),
                )
            },
            TypeLibMemberInvokeKind::PropertyPut => unsafe {
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_PROPERTYPUT,
                    args,
                    &[COM_DISPID_PROPERTYPUT],
                    ("property-put", prog_id_hint),
                )
            },
            TypeLibMemberInvokeKind::PropertyPutRef => unsafe {
                self.native_dispatch_invoke_runtime_value_args(
                    dispatch,
                    dispid,
                    DISPATCH_PROPERTYPUTREF,
                    args,
                    &[COM_DISPID_PROPERTYPUT],
                    ("property-putref", prog_id_hint),
                )
            },
        }
        .map_err(|failure| self.com_dispatch_invoke_fault(failure))
    }

    #[cfg(target_os = "windows")]
    fn native_com_dispatch_invoke_with_bound_dispatch_runtime_value(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id: &str,
        member: i32,
        args: &[ComInvokeArg],
    ) -> HalResult<RuntimeValue> {
        self.ensure_thread_com_apartment("dispatch_invoke")?;
        self.native_com_dispatch_invoke_core_runtime_value(dispatch, prog_id, member, args)
    }

    #[cfg(target_os = "windows")]
    fn bind_native_dispatch_result(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id_hint: &str,
        op: &'static str,
    ) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        let mut state = self.com_lock(capability, op)?;
        let handle = unsafe { com_bind_native_dispatch_result(&mut state, dispatch, prog_id_hint) };
        self.assert_com_invariants(&state, op);
        Ok(RuntimeValue::ObjectHandle(handle))
    }

    #[cfg(target_os = "windows")]
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn native_dispatch_invoke_runtime_value_args(
        &self,
        dispatch: *mut RawIDispatch,
        dispid: i32,
        flags: u16,
        args: &[ComInvokeArg],
        named_arg_dispids: &[i32],
        context: (&'static str, &str),
    ) -> Result<RuntimeValue, ComInvokeFailure> {
        let (label, prog_id_hint) = context;
        com_invoke_dispatch_runtime_value(
            dispatch.cast(),
            dispid,
            flags,
            args,
            named_arg_dispids,
            label,
            prog_id_hint,
            &mut |handle| {
                self.resolve_native_dispatch_for_object_arg(handle, "dispatch_invoke")
                    .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                    .map_err(|err| {
                        format!("{} [{}] {}", err.stable_code, err.operation, err.message)
                    })
            },
            &mut |unknown| {
                raw_query_dispatch_from_unknown(unknown.cast::<RawIUnknown>())
                    .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
            },
            &mut |dispatch| {
                raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
            },
            &mut |dispatch, prog_id_hint, op| {
                self.bind_native_dispatch_result(dispatch.cast::<RawIDispatch>(), prog_id_hint, op)
                    .map_err(|err| {
                        format!("{} [{}] {}", err.stable_code, err.operation, err.message)
                    })
            },
        )
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
    fn com_dispatch_invoke_fault(&self, failure: ComInvokeFailure) -> HalError {
        let label = map_com_hresult_label(failure.hr.map(|hr| hr as u32), failure.arg_err);
        let mut suffix = String::new();
        if let Some(hr) = failure.hr {
            suffix.push_str(&format!("hresult=0x{:08X};", hr as u32));
        }
        if let Some(value) = failure.arg_err {
            suffix.push_str(&format!("arg_err={value};"));
        }
        if let Some(excep) = &failure.excep
            && let Some(scode) = excep.scode
        {
            suffix.push_str(&format!("excep_scode=0x{:08X};", scode as u32));
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
            format!("{prefix} {}", failure.render()),
        )
    }

    #[cfg(target_os = "windows")]
    fn queue_com_event_callbacks(
        &self,
        object: i32,
        binding: &ComBinding,
        member: i32,
        args: Option<&[i32]>,
    ) -> HalResult<()> {
        let member = ComMemberToken::new(member);
        let Some(trigger_spec) = binding.event_trigger_specs.get(&member).copied() else {
            return Ok(());
        };
        let Some(args) = args else {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                format!(
                    "COM-E-VALUE-TRANSPORT-UNSUPPORTED: projected event trigger `{}` requires legacy callback argument transport",
                    trigger_spec.event_token
                ),
            ));
        };
        let Some((event, args)) = event_callback_args_from_member_token(binding, member, args)
        else {
            return Ok(());
        };
        let Some(expected_arity) = event_signature_arity_for_binding(binding, event) else {
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
        if event_is_source_interface_only(binding, event) {
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
        let queued = state.queue_callbacks_for_source_event(
            object.into(),
            event,
            args.as_slice(),
            |transport| transport.is_projection(),
        );
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
        let event = ComMemberToken::new(event);
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
        self.ensure_thread_com_apartment("subscribe_event")?;
        let dispatch = binding.native_dispatch as *mut RawIDispatch;
        // SAFETY: `dispatch` is a live COM object pointer, `spec` and `connection_point_iid`
        // came from deterministic metadata for this binding, and the cloned shared state is owned
        // by the adapter for the lifetime of the subscription transport.
        let advised = unsafe {
            advise_event_subscription(
                dispatch,
                Arc::clone(&self.com_state),
                subscription.into(),
                spec,
                expected_arity,
                connection_point_iid,
            )
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
            // SAFETY: `native` came from a successful Advise call in this adapter and is released
            // at most once here or during teardown paths that remove the same subscription.
            unsafe {
                release_subscription_transport(
                    ComEventSubscriptionTransport::NativeConnectionPoint(native),
                )
            }
            .map_err(|message| {
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
        _args: Option<&[i32]>,
    ) -> HalResult<()> {
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn native_com_dispatch_invoke(
        &self,
        _prog_id: &str,
        _member: i32,
        _args: &[i32],
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

impl HostServices for StandardHostServices {
    fn profile(&self) -> HalProfileId {
        self.profile()
    }

    fn descriptor(&self) -> HalDescriptor {
        self.descriptor()
    }

    fn policy(&self) -> &HostPolicy {
        self.policy()
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
    fn msg_box(&self, prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::UiInteraction;
        let style = self.runtime_value_to_legacy_i32(&style, capability, "msg_box", "style")?;
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
            return self
                .native_windows_msg_box_value(&prompt, style)
                .map(RuntimeValue::I32);
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Linux
            && self.runtime_class() == HalRuntimeClass::LinuxStdio
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            eprintln!(
                "[oxvba-hal] linux-stdio msg_box prompt={:?} style={style}",
                prompt
            );
            return Ok(RuntimeValue::I32(style.max(1)));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "msg_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(RuntimeValue::I32(style.max(1))),
            UiVirtualizationMode::Disabled => Ok(RuntimeValue::I32(
                prompt.to_legacy_i32().unwrap_or(1).max(1),
            )),
        }
    }

    fn input_box(
        &self,
        prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
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
            eprintln!(
                "[oxvba-hal] linux-stdio input_box prompt={:?} default={:?}",
                prompt, default_value
            );
            return Ok(default_value);
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "input_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::Disabled => Ok(prompt),
        }
    }
}

impl EventPumpHal for StandardHostServices {
    fn do_events(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::EventPump;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "do_events"));
        }
        if self.native_mode_enabled() {
            if self.profile == HalProfileId::Windows {
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
            if let Some(callback) = state.mark_next_callback_pumped() {
                if com_event_trace_enabled() {
                    eprintln!(
                        "[oxvba-hal][com-event] do-events callback={} remaining_pending={} last_pumped={:?}",
                        callback,
                        state.pending_callbacks.len(),
                        state.last_pumped_callback
                    );
                }
                self.assert_com_invariants(&state, "do_events-post");
                return Ok(RuntimeValue::I32(callback.into()));
            }
            self.assert_com_invariants(&state, "do_events-post");
        }
        Ok(RuntimeValue::I32(0))
    }
}

impl FileSystemHal for StandardHostServices {
    fn open(&self, path: RuntimeValue, mode: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "open"));
        }
        let mode = self.runtime_value_to_legacy_i32(&mode, capability, "open", "mode")?;
        if mode != 0 && !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "open"));
        }
        if let RuntimeValue::String(BStr(path_text)) = &path {
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
                let host_path = PathBuf::from(path_text);
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
                        .unwrap_or(1)
                }
            } else if mode == 0 {
                i32::from(!path_text.is_empty())
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
            return Ok(RuntimeValue::I32(handle));
        }
        let path = self.runtime_value_to_legacy_i32(&path, capability, "open", "path")?;
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
        Ok(RuntimeValue::I32(handle))
    }

    fn close(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "close"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "close", "handle")?;
        let mut state = self.fs_lock(capability, "close")?;
        self.assert_fs_invariants(&state, "close-pre");
        if state.handles.remove(&handle).is_some() {
            self.assert_fs_invariants(&state, "close-post");
            Ok(RuntimeValue::I32(1))
        } else {
            Err(HalError::adapter_fault(
                self.profile,
                capability,
                "close",
                format!("invalid file handle: {handle}"),
            ))
        }
    }

    fn seek(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "seek"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "seek", "handle")?;
        let position =
            self.runtime_value_to_legacy_i32(&position, capability, "seek", "position")?;
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
        Ok(RuntimeValue::I32(final_position))
    }

    fn eof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "eof"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "eof", "handle")?;
        let mut state = self.fs_lock(capability, "eof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "eof")?;
        Ok(RuntimeValue::I32(if entry.position >= entry.len {
            1
        } else {
            0
        }))
    }

    fn lof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "lof"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "lof", "handle")?;
        let mut state = self.fs_lock(capability, "lof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "lof")?;
        Ok(RuntimeValue::I32(entry.len))
    }

    fn free_file(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "free_file"));
        }
        let range_selector = self.runtime_value_to_legacy_i32(
            &range_selector,
            capability,
            "free_file",
            "range_selector",
        )?;
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
        Ok(RuntimeValue::I32(candidate))
    }
}

impl ProcessEnvHal for StandardHostServices {
    fn shell(&self, command: RuntimeValue, _window_style: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "shell"));
        }
        if !self.policy.allow_process_spawn {
            return Err(self.denied(capability, "shell"));
        }
        if self.native_process_enabled()
            && let RuntimeValue::String(BStr(text)) = &command
            && !text.trim().is_empty()
        {
            let mut child = self.spawn_probe_shell_process_text(text).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "shell",
                    format!("failed to spawn probe shell process: {err}"),
                )
            })?;
            let child_id = i32::try_from(child.id()).unwrap_or(i32::MAX).max(1);
            let _ = child.wait();
            return Ok(RuntimeValue::I32(child_id));
        }
        if self.native_process_enabled() {
            let command = self
                .runtime_value_to_legacy_i32(&command, capability, "shell", "command")
                .unwrap_or(0);
            if command != 0 {
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
                return Ok(RuntimeValue::I32(child_id));
            }
        }
        let command = match &command {
            RuntimeValue::String(BStr(text)) => i32::from(!text.trim().is_empty()),
            other => self
                .runtime_value_to_legacy_i32(other, capability, "shell", "command")
                .unwrap_or(0),
        };
        Ok(RuntimeValue::from_legacy_i32(if command == 0 {
            0
        } else {
            1
        }))
    }

    fn environ(&self, key: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "environ"));
        }
        if self.native_process_enabled()
            && let RuntimeValue::String(BStr(name)) = &key
        {
            let value_len = std::env::var_os(name)
                .map(|value| value.to_string_lossy().len())
                .unwrap_or(0);
            return Ok(RuntimeValue::I32(value_len.min(i32::MAX as usize) as i32));
        }
        if self.native_process_enabled() {
            let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> =
                std::env::vars_os().collect();
            if vars.is_empty() {
                return Ok(RuntimeValue::I32(0));
            }
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            let key = self
                .runtime_value_to_legacy_i32(&key, capability, "environ", "key")
                .unwrap_or(0);
            let idx = (key.unsigned_abs() as usize) % vars.len();
            let value_len = vars[idx].1.to_string_lossy().len();
            return Ok(RuntimeValue::I32(value_len.min(i32::MAX as usize) as i32));
        }
        let key = match &key {
            RuntimeValue::String(BStr(text)) => text.len().min(i32::MAX as usize) as i32,
            other => self
                .runtime_value_to_legacy_i32(other, capability, "environ", "key")
                .unwrap_or(0),
        };
        Ok(RuntimeValue::from_legacy_i32(key))
    }

    fn dir(&self, path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dir"));
        }
        if self.native_process_enabled() {
            let target = match &path {
                RuntimeValue::Empty | RuntimeValue::Null | RuntimeValue::I32(0) => {
                    std::env::current_dir().map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "dir",
                            format!("failed to get current directory: {err}"),
                        )
                    })?
                }
                _ => self.runtime_value_to_path(&path, capability, "dir", "path")?,
            };
            let out = match fs::read_dir(target) {
                Ok(mut entries) => i32::from(entries.next().is_some()),
                Err(_) => 0,
            };
            return Ok(RuntimeValue::I32(out));
        }
        let out = match &path {
            RuntimeValue::Empty | RuntimeValue::Null | RuntimeValue::I32(0) => 0,
            RuntimeValue::String(BStr(text)) => i32::from(!text.is_empty()),
            other => i32::from(
                self.runtime_value_to_legacy_i32(other, capability, "dir", "path")
                    .unwrap_or(0)
                    != 0,
            ),
        };
        Ok(RuntimeValue::I32(out))
    }
}

impl ComHal for StandardHostServices {
    fn create_object(&self, prog_id: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "create_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "create_object"));
        }
        if let RuntimeValue::String(BStr(name)) = &prog_id {
            let prog_id_name = name.trim();
            if prog_id_name.is_empty() {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "create_object",
                    "string ProgID activation requires a non-empty ProgID",
                ));
            }
            if self.native_com_enabled() {
                match self.try_binding_from_typelib_metadata(prog_id_name) {
                    Ok(native_dispatch) => {
                        let metadata = self.load_typelib_metadata_for_prog_id_name(prog_id_name)?;
                        #[cfg(target_os = "windows")]
                        let registered_event_override = self
                            .registered_event_override_for_prog_id_name(
                                prog_id_name,
                                "create_object",
                            )?;
                        let mut state = self.com_lock(capability, "create_object")?;
                        let mut binding = binding_from_typelib_metadata(
                            prog_id_name.to_string(),
                            native_dispatch,
                            metadata.as_ref(),
                        );
                        #[cfg(target_os = "windows")]
                        if let Some(override_cfg) = registered_event_override.as_ref() {
                            self.apply_registered_event_override_to_binding(
                                &mut binding,
                                override_cfg,
                            );
                        }
                        let handle = com_insert_bound_object_binding(&mut state, binding);
                        self.assert_com_invariants(&state, "create_object");
                        return Ok(RuntimeValue::ObjectHandle(handle));
                    }
                    Err(err) => return Err(err),
                }
            }
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "create_object",
                "string ProgID activation requires native COM-enabled Windows host mode",
            ));
        }
        let prog_id =
            self.runtime_value_to_legacy_i32(&prog_id, capability, "create_object", "prog_id")?;
        if self.native_com_enabled()
            && let Some(prog_id_name) = self.resolve_native_com_progid(prog_id)
        {
            match self.try_binding_from_typelib_metadata(&prog_id_name) {
                Ok(native_dispatch) => {
                    let metadata = self.load_typelib_metadata_for_prog_id_name(&prog_id_name)?;
                    #[cfg(target_os = "windows")]
                    let registered_event_override = self
                        .registered_event_override_for_prog_id_name(
                            &prog_id_name,
                            "create_object",
                        )?;
                    let mut state = self.com_lock(capability, "create_object")?;
                    let mut binding = binding_from_typelib_metadata(
                        prog_id_name,
                        native_dispatch,
                        metadata.as_ref(),
                    );
                    #[cfg(target_os = "windows")]
                    if let Some(override_cfg) = registered_event_override.as_ref() {
                        self.apply_registered_event_override_to_binding(&mut binding, override_cfg);
                    }
                    let handle = com_insert_bound_object_binding(&mut state, binding);
                    self.assert_com_invariants(&state, "create_object");
                    return Ok(RuntimeValue::ObjectHandle(handle));
                }
                Err(err) => {
                    if self.has_explicit_native_com_override(prog_id) {
                        return Err(err);
                    }
                }
            }
        }
        Ok(RuntimeValue::ObjectHandle(
            5_000i32.saturating_add(prog_id).into(),
        ))
    }

    fn release_object(&self, object: ObjectHandle) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "release_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "release_object"));
        }
        let object = object.raw();
        if !self.native_com_enabled() {
            return Ok(RuntimeValue::I32(if object == 0 { 0 } else { 1 }));
        }
        let released = {
            let mut state = self.com_lock(capability, "release_object")?;
            self.assert_com_invariants(&state, "release_object-pre");
            let released = com_release_object_binding(&mut state, ObjectHandle::new(object))
                .map_err(|message| {
                    HalError::adapter_fault(self.profile, capability, "release_object", message)
                })?;
            self.assert_com_invariants(&state, "release_object-post");
            released
        };
        for transport in released.transports {
            self.release_event_subscription_transport(transport)?;
        }
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] release-object object={} removed_callbacks={}",
                object,
                released.stale_callbacks.len()
            );
        }
        Ok(RuntimeValue::I32(1))
    }

    fn describe_object(&self, object: ObjectHandle) -> HalResult<Option<ComObjectDescriptor>> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "describe_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "describe_object"));
        }
        let object_handle = object;
        let object = object.raw();
        let descriptor = if self.native_com_enabled() {
            let state = self.com_lock(capability, "describe_object")?;
            self.assert_com_invariants(&state, "describe_object");
            state
                .bindings
                .get(&ComObjectToken::new(object))
                .map(|binding| {
                    binding.descriptor(
                        object_handle,
                        known_typelib_identity_for_prog_id_name(&binding.prog_id_name)
                            .map(|identity| identity.cache_key),
                    )
                })
        } else if object == 0 {
            None
        } else {
            Some(ComObjectDescriptor {
                object: object_handle,
                prog_id_name: format!("selector:{object}"),
                transport: ComObjectTransportKind::Projection,
                supports_events: false,
                known_member_tokens: Vec::new(),
                known_event_tokens: Vec::new(),
                default_member_token: None,
                default_member_name: None,
                typelib_cache_key: None,
            })
        };
        Ok(descriptor)
    }

    fn dispatch_invoke_runtime_value_v2(
        &self,
        request: &ComInvokeRequest,
    ) -> HalResult<RuntimeValue> {
        let object = request.object.raw();
        let member = request.member.raw();
        let args = request.args.as_slice();
        let positional_values = com_legacy_runtime_arg_values(args);
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dispatch_invoke"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "dispatch_invoke"));
        }
        if self.native_com_enabled() {
            #[cfg(target_os = "windows")]
            let _ = com_validate_named_arg_order(args).map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "dispatch_invoke", message)
            })?;
            let (binding, cached_dispid) = {
                let state = self.com_lock(capability, "dispatch_invoke")?;
                self.assert_com_invariants(&state, "dispatch_invoke");
                let binding = state.bindings.get(&ComObjectToken::new(object)).cloned();
                let cached_dispid = binding
                    .as_ref()
                    .and_then(|entry| entry.member_dispids.get(&request.member).copied());
                (binding, cached_dispid)
            };
            if let Some(binding) = binding {
                #[cfg(target_os = "windows")]
                if binding.native_dispatch != 0 {
                    let dispatch = binding.native_dispatch as *mut RawIDispatch;
                    let plan = com_plan_bound_runtime_invoke(&binding, request, cached_dispid)
                        .map_err(|message| {
                            HalError::adapter_fault(
                                self.profile,
                                capability,
                                "dispatch_invoke",
                                message,
                            )
                        })?;
                    let effective_member = plan.effective_member;
                    let effective_cached_dispid = plan.effective_cached_dispid;
                    let named_default_member_spec = plan.named_default_member_spec;
                    let direct_dispatch_spec = plan.direct_dispatch_spec;
                    let legacy_vtable_candidate_args = plan.legacy_vtable_candidate_args;
                    let value = if let Some(positional_values) =
                        legacy_vtable_candidate_args.as_ref()
                        && let Some(value) = self.try_native_com_vtable_invoke(
                            dispatch,
                            &binding.prog_id_name,
                            effective_member.raw(),
                            positional_values,
                        )? {
                        RuntimeValue::I32(value)
                    } else if let Some((token, spec)) = named_default_member_spec {
                        let (dispid, spec) = self
                            .resolve_member_dispid_cached(
                                object,
                                dispatch,
                                &binding,
                                token.raw(),
                                effective_cached_dispid,
                            )?
                            .map(|(dispid, _)| (dispid, spec))
                            .ok_or_else(|| {
                                HalError::adapter_fault(
                                    self.profile,
                                    capability,
                                    "dispatch_invoke",
                                    "default member identity unavailable for named late-bound dispatch",
                                )
                            })?;
                        self.native_com_dispatch_invoke_with_member_spec_runtime_value(
                            dispatch,
                            dispid,
                            &spec,
                            args,
                            &binding.prog_id_name,
                        )?
                    } else if let Some((dispid, spec)) = self.resolve_member_dispid_cached(
                        object,
                        dispatch,
                        &binding,
                        effective_member.raw(),
                        effective_cached_dispid,
                    )? {
                        self.native_com_dispatch_invoke_with_member_spec_runtime_value(
                            dispatch,
                            dispid,
                            &spec,
                            args,
                            &binding.prog_id_name,
                        )?
                    } else if let Some(spec) = direct_dispatch_spec {
                        self.native_com_dispatch_invoke_with_direct_dispid_runtime_value(
                            dispatch,
                            effective_member.raw(),
                            spec.invoke_kind,
                            spec.requires_argument,
                            args,
                            &binding.prog_id_name,
                        )?
                    } else {
                        self.native_com_dispatch_invoke_with_bound_dispatch_runtime_value(
                            dispatch,
                            &binding.prog_id_name,
                            effective_member.raw(),
                            args,
                        )?
                    };
                    self.queue_com_event_callbacks(
                        object,
                        &binding,
                        member,
                        positional_values.as_deref(),
                    )?;
                    return Ok(value);
                }
                let positional_values = positional_values.as_ref().ok_or_else(|| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "dispatch_invoke",
                        "COM-E-VALUE-TRANSPORT-UNSUPPORTED: projection dispatch requires legacy runtime-token arguments",
                    )
                })?;
                let value = self.native_com_dispatch_invoke(&binding.prog_id_name, member, args)?;
                self.queue_com_event_callbacks(object, &binding, member, Some(positional_values))?;
                return Ok(RuntimeValue::I32(value));
            }
        }
        let positional_values = positional_values.ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "dispatch_invoke",
                "COM-E-VALUE-TRANSPORT-UNSUPPORTED: fallback dispatch lane requires legacy runtime-token arguments",
            )
        })?;
        Ok(RuntimeValue::I32(
            positional_values
                .iter()
                .fold(object.saturating_add(member), |acc, arg| {
                    acc.saturating_add(*arg)
                }),
        ))
    }

    fn subscribe_event(
        &self,
        object: RuntimeValue,
        event: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
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
        let object =
            self.runtime_value_to_legacy_i32(&object, capability, "subscribe_event", "object")?;
        let event =
            self.runtime_value_to_legacy_i32(&event, capability, "subscribe_event", "event")?;
        let (binding, expected_arity, subscription) = {
            let mut state = self.com_lock(capability, "subscribe_event")?;
            self.assert_com_invariants(&state, "subscribe_event-pre");
            let Some(binding) = state.bindings.get(&ComObjectToken::new(object)).cloned() else {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "subscribe_event",
                    format!(
                        "COM-E-EVENT-CONNECTIONPOINT-MISSING: unknown COM object token {object}"
                    ),
                ));
            };
            let Some(expected_arity) = event_signature_arity_for_binding(&binding, event.into())
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
            subscription.raw(),
            object,
            event,
            expected_arity,
        )?;
        let mut state = self.com_lock(capability, "subscribe_event")?;
        self.assert_com_invariants(&state, "subscribe_event-pre-insert");
        if !state.bindings.contains_key(&ComObjectToken::new(object)) {
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
                object: object.into(),
                event: event.into(),
                transport,
            },
        );
        #[cfg(target_os = "windows")]
        if com_event_trace_enabled() {
            eprintln!(
                "[oxvba-hal][com-event] subscribe object={} event={} subscription={} transport={} arity={}",
                object,
                event,
                subscription.raw(),
                transport.kind_label(),
                expected_arity
            );
        }
        self.assert_com_invariants(&state, "subscribe_event-post");
        Ok(RuntimeValue::from_legacy_i32(subscription.into()))
    }

    fn unsubscribe_event(&self, subscription: RuntimeValue) -> HalResult<RuntimeValue> {
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
        let subscription = self.runtime_value_to_legacy_i32(
            &subscription,
            capability,
            "unsubscribe_event",
            "subscription",
        )?;
        let transport = {
            let state = self.com_lock(capability, "unsubscribe_event")?;
            self.assert_com_invariants(&state, "unsubscribe_event-pre");
            com_resolve_subscription_transport(&state, ComSubscriptionToken::new(subscription))
                .map_err(|message| {
                    HalError::adapter_fault(self.profile, capability, "unsubscribe_event", message)
                })?
        };
        self.release_event_subscription_transport(transport)?;
        let mut state = self.com_lock(capability, "unsubscribe_event")?;
        self.assert_com_invariants(&state, "unsubscribe_event-pre-remove");
        let _stale_callbacks =
            com_remove_subscription_callbacks(&mut state, ComSubscriptionToken::new(subscription))
                .map_err(|message| {
                    HalError::adapter_fault(self.profile, capability, "unsubscribe_event", message)
                })?;
        self.assert_com_invariants(&state, "unsubscribe_event-post-remove");
        Ok(RuntimeValue::from_legacy_i32(1))
    }

    fn poll_event_callback(&self) -> HalResult<Option<ComCallbackPayload>> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "poll_event_callback"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "poll_event_callback"));
        }
        if !self.native_com_enabled() {
            return Ok(None);
        }
        let mut state = self.com_lock(capability, "poll_event_callback")?;
        self.assert_com_invariants(&state, "poll_event_callback-pre");
        let payload = com_take_polled_callback_payload(&mut state);
        self.assert_com_invariants(&state, "poll_event_callback-post");
        Ok(payload)
    }

    fn event_callback_subscription(&self, callback: RuntimeValue) -> HalResult<RuntimeValue> {
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
        let callback = self.runtime_value_to_legacy_i32(
            &callback,
            capability,
            "event_callback_subscription",
            "callback",
        )?;
        let state = self.com_lock(capability, "event_callback_subscription")?;
        self.assert_com_invariants(&state, "event_callback_subscription");
        let subscription = com_callback_subscription_token(&state, ComCallbackToken::new(callback))
            .map_err(|message| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "event_callback_subscription",
                    message,
                )
            })?;
        Ok(RuntimeValue::from_legacy_i32(subscription.into()))
    }

    fn event_callback_arity(&self, callback: RuntimeValue) -> HalResult<RuntimeValue> {
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
        let callback = self.runtime_value_to_legacy_i32(
            &callback,
            capability,
            "event_callback_arity",
            "callback",
        )?;
        let state = self.com_lock(capability, "event_callback_arity")?;
        self.assert_com_invariants(&state, "event_callback_arity");
        let callback_arity =
            com_callback_arity(&state, ComCallbackToken::new(callback)).map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "event_callback_arity", message)
            })?;
        let arity = i32::try_from(callback_arity).map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "event_callback_arity",
                format!(
                    "COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH: callback arity {} exceeds deterministic token range",
                    callback_arity
                ),
            )
        })?;
        Ok(RuntimeValue::from_legacy_i32(arity))
    }

    fn event_callback_arg(
        &self,
        callback: RuntimeValue,
        index: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
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
        let callback = self.runtime_value_to_legacy_i32(
            &callback,
            capability,
            "event_callback_arg",
            "callback",
        )?;
        let index =
            self.runtime_value_to_legacy_i32(&index, capability, "event_callback_arg", "index")?;
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
        let value = com_callback_arg(&state, ComCallbackToken::new(callback), index as usize)
            .map_err(|message| {
                HalError::adapter_fault(self.profile, capability, "event_callback_arg", message)
            })?;
        Ok(value.to_runtime_value())
    }

    fn release_event_callback(&self, callback: RuntimeValue) -> HalResult<RuntimeValue> {
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
        let callback = self.runtime_value_to_legacy_i32(
            &callback,
            capability,
            "release_event_callback",
            "callback",
        )?;
        let mut state = self.com_lock(capability, "release_event_callback")?;
        self.assert_com_invariants(&state, "release_event_callback-pre");
        com_release_callback(&mut state, ComCallbackToken::new(callback)).map_err(|message| {
            HalError::adapter_fault(self.profile, capability, "release_event_callback", message)
        })?;
        self.assert_com_invariants(&state, "release_event_callback-post");
        Ok(RuntimeValue::from_legacy_i32(1))
    }

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
        Ok(state.load_or_build(identity, |identity| self.build_typelib_metadata(identity)))
    }

    fn invalidate_typelib_cache(
        &self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.windows_typelib_supported() {
            return Err(self.unsupported(capability, "invalidate_typelib_cache"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "invalidate_typelib_cache"));
        }
        let mut state = self.typelib_lock(capability, "invalidate_typelib_cache")?;
        let removed = state.invalidate(scope, reference_name).map_err(|detail| {
            HalError::adapter_fault(self.profile, capability, "invalidate_typelib_cache", detail)
        })?;
        Ok(RuntimeValue::I32(
            i32::try_from(removed).unwrap_or(i32::MAX),
        ))
    }
}

impl TimeLocaleHal for StandardHostServices {
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "date_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok(RuntimeValue::I32(clamp_u64_to_i32(now.as_secs() / 86_400)));
        }
        Ok(RuntimeValue::I32(20_260_301))
    }

    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "time_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok(RuntimeValue::I32((now.as_secs() % 86_400) as i32));
        }
        Ok(RuntimeValue::I32(123_456))
    }

    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "timer_ticks"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let modulo = i32::MAX as u128;
            return Ok(RuntimeValue::I32((now.as_millis() % modulo) as i32));
        }
        Ok(RuntimeValue::I32(42))
    }
}

impl DynamicLinkHal for StandardHostServices {
    fn bind_descriptor(&self, descriptor: &DynLinkDescriptorView<'_>) -> HalResult<BindingHandle> {
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

        let mut state = self.dynlink_state.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "invoke_symbol",
                "dynlink binding table lock poisoned",
            )
        })?;
        if let Some(existing) = state.descriptors.get(&descriptor.descriptor_id).copied() {
            let Some(existing_symbol) = state.bindings.get(&existing).copied() else {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "descriptor {} binding {} is missing from dynlink registry",
                        descriptor.descriptor_id, existing
                    ),
                ));
            };
            if existing_symbol != descriptor.symbol {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "descriptor {} binding mismatch: existing={} resolved_symbol={} new_symbol={}",
                        descriptor.descriptor_id, existing, existing_symbol, descriptor.symbol
                    ),
                ));
            }
            return Ok(existing);
        }
        let binding = state.allocate_binding();
        state.descriptors.insert(descriptor.descriptor_id, binding);
        state.bindings.insert(binding, descriptor.symbol);
        Ok(binding)
    }

    fn prepare_invoke(
        &self,
        _binding: BindingHandle,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        Ok(arg)
    }

    fn invoke_bound(&self, binding: BindingHandle, arg: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::DynamicLinking;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "invoke_symbol"));
        }
        if !self.policy.allow_dynamic_link {
            return Err(self.denied(capability, "invoke_symbol"));
        }
        let symbol = {
            let state = self.dynlink_state.lock().map_err(|_| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    "dynlink binding table lock poisoned",
                )
            })?;
            state.bindings.get(&binding).copied().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "binding handle {} is not resolved in dynlink registry",
                        binding
                    ),
                )
            })?
        };
        let arg = self.runtime_value_to_legacy_i32(&arg, capability, "invoke_symbol", "arg")?;
        if self.native_mode_enabled()
            && matches!(self.profile, HalProfileId::Windows | HalProfileId::Linux)
        {
            return match symbol.raw() {
                s if s == external_symbol_token("host", "ping", "hostping") => {
                    Ok(RuntimeValue::I32(arg.saturating_add(1)))
                }
                s if s == external_symbol_token("host", "double", "hostdouble") => {
                    Ok(RuntimeValue::I32(arg.saturating_mul(2)))
                }
                _ => Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!(
                        "binding handle {} resolved to unsupported symbol token {} in host-backed lane",
                        binding, symbol
                    ),
                )),
            };
        }
        Ok(RuntimeValue::I32(symbol.raw().saturating_add(arg)))
    }

    fn invoke_descriptor(
        &self,
        descriptor: &DynLinkDescriptorView<'_>,
        arg: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        let binding = self.bind_descriptor(descriptor)?;
        let prepared = self.prepare_invoke(binding, arg)?;
        self.invoke_bound(binding, prepared)
    }

    fn invoke_symbol(&self, symbol: DynLinkSymbol, arg: RuntimeValue) -> HalResult<RuntimeValue> {
        let arg = self.runtime_value_to_legacy_i32(
            &arg,
            CapabilityId::DynamicLinking,
            "invoke_symbol",
            "arg",
        )?;
        let descriptor = DynLinkDescriptorView {
            descriptor_id: symbol.raw() as u32,
            declared_name: "<legacy>",
            library: "<legacy>",
            alias: "<legacy>",
            ordinal_alias: false,
            symbol,
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "legacy-symbol",
        };
        self.invoke_descriptor(&descriptor, RuntimeValue::I32(arg))
    }
}

impl DiagnosticsHal for StandardHostServices {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "emit"));
        }
        let code = self.runtime_value_to_legacy_i32(&code, capability, "emit", "code")?;
        let payload = self.runtime_value_to_legacy_i32(&payload, capability, "emit", "payload")?;
        if self.native_diagnostics_enabled() {
            eprintln!(
                "[oxvba-hal] profile={:?} code={} payload={}",
                self.profile, code, payload
            );
        }
        Ok(RuntimeValue::I32(code.saturating_add(payload)))
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

type ComState = WindowsComClientState;
type ComEventSubscription = SharedComEventSubscription<WindowsComSubscriptionTransport>;
type ComEventSubscriptionTransport = WindowsComSubscriptionTransport;
type TypeLibraryCacheState = TypeLibMetadataCacheState;

#[derive(Debug, Default)]
struct DynLinkBindingState {
    next_binding: i32,
    descriptors: BTreeMap<u32, BindingHandle>,
    bindings: BTreeMap<BindingHandle, DynLinkSymbol>,
}

impl DynLinkBindingState {
    fn allocate_binding(&mut self) -> BindingHandle {
        self.next_binding = self.next_binding.saturating_add(1).max(1);
        BindingHandle::new(80_000i32.saturating_add(self.next_binding))
    }
}

type RawDispatchPtr = usize;
#[cfg(target_os = "windows")]
thread_local! {
    static THREAD_COM_APARTMENT_READY: Cell<bool> = const { Cell::new(false) };
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
#[cfg(not(target_os = "windows"))]
fn com_event_signature_arity_for_binding(_binding: &ComBinding, _event: i32) -> Option<usize> {
    None
}

#[cfg(not(target_os = "windows"))]
fn com_event_is_source_interface_only(_binding: &ComBinding, _event: i32) -> bool {
    false
}
fn com_member_spec_for_binding(
    binding: &ComBinding,
    member: ComMemberToken,
) -> Option<ComMemberSpec> {
    binding.member_specs.get(&member).cloned()
}

#[cfg(target_os = "windows")]
#[cfg(not(target_os = "windows"))]
fn com_event_callback_args_from_member_token(
    _binding: &ComBinding,
    _member: i32,
    _arg: i32,
) -> Option<(i32, Vec<i32>)> {
    None
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
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_variant_to_com_value(variant: &VARIANT) -> Result<ComValue, String> {
    com_variant_to_com_value(variant)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_dispatch_arg<F>(
    variant: *mut VARIANT,
    value: &ComValue,
    resolve_object: &mut F,
) -> Result<(), String>
where
    F: FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
{
    if variant.is_null() {
        return Ok(());
    }
    let mut resolve_dispatch = |handle: ObjectHandle| {
        resolve_object(handle).map(|dispatch| dispatch.cast::<core::ffi::c_void>())
    };
    let mut add_ref_dispatch = |dispatch: *mut core::ffi::c_void| {
        raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
    };
    match value {
        ComValue::ObjectHandle(_) => com_set_variant_from_com_value(
            variant,
            value,
            &mut resolve_dispatch,
            &mut add_ref_dispatch,
        )?,
        _ => {
            let mut unexpected_object_resolution = |_handle: ObjectHandle| {
                Err("object dispatch resolution not expected for non-object COM value".to_string())
            };
            let mut unexpected_add_ref = |_dispatch: *mut core::ffi::c_void| {};
            com_set_variant_from_com_value(
                variant,
                value,
                &mut unexpected_object_resolution,
                &mut unexpected_add_ref,
            )?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn set_variant_missing_arg(variant: &mut VARIANT) {
    variant.Anonymous.Anonymous.vt = VT_ERROR;
    variant.Anonymous.Anonymous.Anonymous.scode = COM_DISP_E_PARAMNOTFOUND;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn clear_variant_args(args: &mut [VARIANT]) {
    for variant in args {
        let _ = VariantClear(variant);
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_i4_args(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    flags: u16,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    label: &'static str,
    resolve_object: &mut impl FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
) -> Result<i32, ComInvokeFailure> {
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        match arg.value {
            Some(ref value) => set_variant_dispatch_arg(&mut variant, value, resolve_object)
                .map_err(|detail| ComInvokeFailure {
                    label,
                    dispid,
                    hr: None,
                    arg_err: None,
                    excep: None,
                    detail: Some(detail),
                })?,
            None => set_variant_missing_arg(&mut variant),
        }
        invoke_args.push(variant);
    }

    let mut named_arg_dispids_reversed: Vec<i32> =
        named_arg_dispids.iter().rev().copied().collect();
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if named_arg_dispids_reversed.is_empty() {
            std::ptr::null_mut()
        } else {
            named_arg_dispids_reversed.as_mut_ptr()
        },
        cArgs: args.len() as u32,
        cNamedArgs: named_arg_dispids.len() as u32,
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        flags,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    clear_variant_args(&mut invoke_args);
    if hr < 0 {
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: (arg_err != u32::MAX).then_some(arg_err),
            excep: take_excepinfo(&mut excep),
            detail: None,
        });
    }

    let token = match raw_variant_to_com_value(&result) {
        Ok(value) => match value.to_runtime_token() {
            Ok(token) => token,
            Err(detail) => {
                let _ = VariantClear(&mut result);
                return Err(ComInvokeFailure {
                    label,
                    dispid,
                    hr: None,
                    arg_err: None,
                    excep: None,
                    detail: Some(detail),
                });
            }
        },
        Err(detail) => {
            let _ = VariantClear(&mut result);
            return Err(ComInvokeFailure {
                label,
                dispid,
                hr: None,
                arg_err: None,
                excep: None,
                detail: Some(detail),
            });
        }
    };
    let _ = VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_i4_args_positional(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    flags: u16,
    args: &[ComValue],
    property_put_named_arg: bool,
    label: &'static str,
    resolve_object: &mut impl FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
) -> Result<i32, ComInvokeFailure> {
    let mut invoke_args: Vec<VARIANT> = Vec::with_capacity(args.len());
    for arg in args.iter().rev() {
        let mut variant: VARIANT = std::mem::zeroed();
        set_variant_dispatch_arg(&mut variant, arg, resolve_object).map_err(|detail| {
            ComInvokeFailure {
                label,
                dispid,
                hr: None,
                arg_err: None,
                excep: None,
                detail: Some(detail),
            }
        })?;
        invoke_args.push(variant);
    }

    let mut named_arg = COM_DISPID_PROPERTYPUT;
    let mut result: VARIANT = std::mem::zeroed();
    let mut excep: EXCEPINFO = std::mem::zeroed();
    let mut arg_err = u32::MAX;
    let mut params = DISPPARAMS {
        rgvarg: if invoke_args.is_empty() {
            std::ptr::null_mut()
        } else {
            invoke_args.as_mut_ptr()
        },
        rgdispidNamedArgs: if property_put_named_arg {
            &mut named_arg
        } else {
            std::ptr::null_mut()
        },
        cArgs: args.len() as u32,
        cNamedArgs: if property_put_named_arg { 1 } else { 0 },
    };
    let hr = ((*(*dispatch).vtbl).invoke)(
        dispatch.cast(),
        dispid,
        &IID_NULL,
        0x0400,
        flags,
        &mut params,
        &mut result,
        &mut excep,
        &mut arg_err,
    );
    clear_variant_args(&mut invoke_args);
    if hr < 0 {
        return Err(ComInvokeFailure {
            label,
            dispid,
            hr: Some(hr),
            arg_err: (arg_err != u32::MAX).then_some(arg_err),
            excep: take_excepinfo(&mut excep),
            detail: None,
        });
    }

    let token = match raw_variant_to_com_value(&result) {
        Ok(value) => match value.to_runtime_token() {
            Ok(token) => token,
            Err(detail) => {
                let _ = VariantClear(&mut result);
                return Err(ComInvokeFailure {
                    label,
                    dispid,
                    hr: None,
                    arg_err: None,
                    excep: None,
                    detail: Some(detail),
                });
            }
        },
        Err(detail) => {
            let _ = VariantClear(&mut result);
            return Err(ComInvokeFailure {
                label,
                dispid,
                hr: None,
                arg_err: None,
                excep: None,
                detail: Some(detail),
            });
        }
    };
    let _ = VariantClear(&mut result);
    Ok(token)
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_property_get_i4_args(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    resolve_object: &mut impl FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
) -> Result<i32, ComInvokeFailure> {
    raw_dispatch_invoke_i4_args(
        dispatch,
        dispid,
        DISPATCH_PROPERTYGET,
        args,
        named_arg_dispids,
        "property-get",
        resolve_object,
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_property_put_i4_args(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    resolve_object: &mut impl FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
) -> Result<i32, ComInvokeFailure> {
    if named_arg_dispids.is_empty()
        && args
            .iter()
            .all(|arg| arg.name.is_none() && arg.value.is_some())
    {
        let positional_args: Result<Vec<ComValue>, String> = args
            .iter()
            .filter_map(|arg| arg.value.clone())
            .map(|value| {
                value.to_legacy_dispatch_token()?;
                Ok(value)
            })
            .collect();
        if let Ok(positional_args) = positional_args {
            return raw_dispatch_invoke_i4_args_positional(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUT,
                &positional_args,
                true,
                "property-put",
                resolve_object,
            );
        }
    }
    let mut all_named_arg_dispids = named_arg_dispids.to_vec();
    all_named_arg_dispids.push(COM_DISPID_PROPERTYPUT);
    raw_dispatch_invoke_i4_args(
        dispatch,
        dispid,
        DISPATCH_PROPERTYPUT,
        args,
        &all_named_arg_dispids,
        "property-put",
        resolve_object,
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_property_putref_i4_args(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    resolve_object: &mut impl FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
) -> Result<i32, ComInvokeFailure> {
    if named_arg_dispids.is_empty()
        && args
            .iter()
            .all(|arg| arg.name.is_none() && arg.value.is_some())
    {
        let positional_args: Result<Vec<ComValue>, String> = args
            .iter()
            .filter_map(|arg| arg.value.clone())
            .map(|value| {
                value.to_legacy_dispatch_token()?;
                Ok(value)
            })
            .collect();
        if let Ok(positional_args) = positional_args {
            return raw_dispatch_invoke_i4_args_positional(
                dispatch,
                dispid,
                DISPATCH_PROPERTYPUTREF,
                &positional_args,
                true,
                "property-putref",
                resolve_object,
            );
        }
    }
    let mut all_named_arg_dispids = named_arg_dispids.to_vec();
    all_named_arg_dispids.push(COM_DISPID_PROPERTYPUT);
    raw_dispatch_invoke_i4_args(
        dispatch,
        dispid,
        DISPATCH_PROPERTYPUTREF,
        args,
        &all_named_arg_dispids,
        "property-putref",
        resolve_object,
    )
}

#[cfg(target_os = "windows")]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn raw_dispatch_invoke_method_i4_args(
    dispatch: *mut RawIDispatch,
    dispid: i32,
    args: &[ComInvokeArg],
    named_arg_dispids: &[i32],
    resolve_object: &mut impl FnMut(ObjectHandle) -> Result<*mut RawIDispatch, String>,
) -> Result<i32, ComInvokeFailure> {
    raw_dispatch_invoke_i4_args(
        dispatch,
        dispid,
        DISPATCH_METHOD,
        args,
        named_arg_dispids,
        "method",
        resolve_object,
    )
}

fn pseudo_file_len_from_path_token(path: i32) -> i32 {
    let magnitude = path.saturating_abs();
    1 + (magnitude % 4096)
}

fn clamp_u64_to_i32(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
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
    use std::sync::{Mutex, OnceLock};

    use oxvba_com::{ComInvokeArg, ComInvokeRequest, ComValue};
    use oxvba_runtime::{RuntimeValue, bstr::BStr};
    use proptest::prelude::*;

    use crate::{
        error::HalErrorKind,
        model::{ComInvocationStrategy, HalProfileId, HostPolicy},
        traits::{
            ComHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal,
            FileSystemHal, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibResolveRequest,
            UiInteractionHal,
        },
    };

    use super::StandardHostServices;
    #[cfg(target_os = "windows")]
    use super::{
        ComObjectToken, RawIDispatch, RawIUnknown, raw_add_ref_dispatch,
        raw_query_dispatch_from_unknown, raw_release_dispatch, raw_variant_to_com_value,
        set_variant_dispatch_arg,
    };
    #[cfg(target_os = "windows")]
    use oxvba_com::take_variant_result_value as com_take_variant_result_value;
    #[cfg(target_os = "windows")]
    use oxvba_com::{VariantResultValue, create_oxvba_test_dispatch};
    #[cfg(target_os = "windows")]
    use oxvba_runtime::ObjectHandle;
    #[cfg(target_os = "windows")]
    use windows_sys::Win32::System::Variant::{
        VARIANT, VT_ARRAY, VT_DISPATCH, VT_UNKNOWN, VT_VARIANT, VariantClear,
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn expect_i32(value: RuntimeValue) -> i32 {
        let RuntimeValue::I32(value) = value else {
            panic!("expected RuntimeValue::I32, got {value:?}");
        };
        value
    }

    fn expect_object_handle(value: RuntimeValue) -> oxvba_runtime::ObjectHandle {
        let RuntimeValue::ObjectHandle(handle) = value else {
            panic!("expected RuntimeValue::ObjectHandle, got {value:?}");
        };
        handle
    }

    fn create_object_test(
        host: &StandardHostServices,
        prog_id: i32,
    ) -> crate::error::HalResult<oxvba_runtime::ObjectHandle> {
        host.create_object(RuntimeValue::I32(prog_id))
            .map(expect_object_handle)
    }

    fn release_object_test(
        host: &StandardHostServices,
        object: oxvba_runtime::ObjectHandle,
    ) -> crate::error::HalResult<i32> {
        host.release_object(object).map(expect_i32)
    }

    fn invalidate_typelib_cache_test(
        host: &StandardHostServices,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> crate::error::HalResult<i32> {
        host.invalidate_typelib_cache(scope, reference_name)
            .map(expect_i32)
    }

    fn dispatch_invoke_legacy(
        host: &StandardHostServices,
        object: i32,
        member: i32,
        arg: i32,
    ) -> crate::error::HalResult<i32> {
        dispatch_invoke_legacy_v2(host, &ComInvokeRequest::legacy(object, member, arg))
    }

    fn dispatch_invoke_legacy_v2(
        host: &StandardHostServices,
        request: &ComInvokeRequest,
    ) -> crate::error::HalResult<i32> {
        host.dispatch_invoke_runtime_value_v2(request)?
            .to_legacy_i32()
            .map_err(|message| host.com_dispatch_adapter_fault(message))
    }

    trait SemanticComTestExt {
        fn create_object_test(
            &self,
            prog_id: i32,
        ) -> crate::error::HalResult<oxvba_runtime::ObjectHandle>;
        fn release_object_test(
            &self,
            object: oxvba_runtime::ObjectHandle,
        ) -> crate::error::HalResult<i32>;
        fn invalidate_typelib_cache_test(
            &self,
            scope: TypeLibCacheScope,
            reference_name: Option<&str>,
        ) -> crate::error::HalResult<i32>;
        fn dispatch_invoke_legacy(
            &self,
            object: i32,
            member: i32,
            arg: i32,
        ) -> crate::error::HalResult<i32>;
        fn dispatch_invoke_legacy_v2(
            &self,
            request: &ComInvokeRequest,
        ) -> crate::error::HalResult<i32>;
    }

    impl SemanticComTestExt for StandardHostServices {
        fn create_object_test(
            &self,
            prog_id: i32,
        ) -> crate::error::HalResult<oxvba_runtime::ObjectHandle> {
            create_object_test(self, prog_id)
        }

        fn release_object_test(
            &self,
            object: oxvba_runtime::ObjectHandle,
        ) -> crate::error::HalResult<i32> {
            release_object_test(self, object)
        }

        fn invalidate_typelib_cache_test(
            &self,
            scope: TypeLibCacheScope,
            reference_name: Option<&str>,
        ) -> crate::error::HalResult<i32> {
            invalidate_typelib_cache_test(self, scope, reference_name)
        }

        fn dispatch_invoke_legacy(
            &self,
            object: i32,
            member: i32,
            arg: i32,
        ) -> crate::error::HalResult<i32> {
            dispatch_invoke_legacy(self, object, member, arg)
        }

        fn dispatch_invoke_legacy_v2(
            &self,
            request: &ComInvokeRequest,
        ) -> crate::error::HalResult<i32> {
            dispatch_invoke_legacy_v2(self, request)
        }
    }

    fn rv(value: i32) -> RuntimeValue {
        RuntimeValue::I32(value)
    }

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
    #[cfg(target_os = "windows")]
    fn com_force_registered_testdispatch_is_cached_at_host_construction() {
        let _guard = env_lock().lock().expect("env lock should be available");
        // SAFETY: Test-only serialized environment mutation guarded by env_lock.
        unsafe {
            std::env::set_var("OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH", "1");
        }
        let cached_true = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        // SAFETY: Test-only serialized environment mutation guarded by env_lock.
        unsafe {
            std::env::set_var("OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH", "0");
        }
        let cached_false = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        // SAFETY: Test-only serialized environment mutation guarded by env_lock.
        unsafe {
            std::env::remove_var("OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH");
        }

        assert!(
            cached_true.force_registered_test_dispatch(),
            "first host should preserve env-cached COM dispatch override"
        );
        assert!(
            !cached_false.force_registered_test_dispatch(),
            "later hosts should see later env values at construction time"
        );
    }

    #[test]
    fn file_open_seek_eof_lof_close_roundtrip() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host.open(rv(77), rv(0)).expect("open should succeed");
        assert_eq!(handle, rv(1));
        assert_eq!(host.eof(handle.clone()).expect("eof should work"), rv(0));
        let len = expect_i32(host.lof(handle.clone()).expect("lof should work"));
        assert!(len > 0);
        host.seek(handle.clone(), rv(len))
            .expect("seek to end should work");
        assert_eq!(host.eof(handle.clone()).expect("eof should work"), rv(1));
        assert_eq!(host.close(handle).expect("close should work"), rv(1));
    }

    #[test]
    fn file_open_accepts_runtime_string_paths() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host
            .open(
                RuntimeValue::String(BStr("runtime-fs-value-path.txt".to_string())),
                RuntimeValue::I32(0),
            )
            .expect("open should succeed");
        let RuntimeValue::I32(handle) = handle else {
            panic!("open should return file handle");
        };
        assert_eq!(
            host.lof(RuntimeValue::I32(handle))
                .expect("lof should succeed"),
            RuntimeValue::I32(1)
        );
        assert_eq!(
            host.close(RuntimeValue::I32(handle))
                .expect("close should succeed"),
            RuntimeValue::I32(1)
        );
    }

    #[test]
    fn free_file_respects_low_and_high_ranges() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.free_file(rv(0)).expect("default free file"), rv(1));
        assert_eq!(
            host.free_file(rv(1)).expect("high-range free file"),
            rv(256)
        );
    }

    #[test]
    fn close_releases_handle_for_reuse() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host.open(rv(10), rv(0)).expect("open should succeed");
        assert_eq!(first, rv(1));
        host.close(first).expect("close should succeed");
        let second = host
            .open(rv(11), rv(0))
            .expect("second open should succeed");
        assert_eq!(second, rv(1), "closed handles must be reusable");
    }

    #[test]
    fn free_file_low_range_tracks_allocated_handles() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let mut handles = Vec::new();
        for expected in 1..=8 {
            assert_eq!(
                host.free_file(rv(0)).expect("free_file should succeed"),
                rv(expected)
            );
            handles.push(host.open(rv(expected), rv(0)).expect("open should succeed"));
        }
        assert_eq!(handles, (1..=8).map(rv).collect::<Vec<_>>());
    }

    #[test]
    fn seek_negative_returns_adapter_fault() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host.open(rv(42), rv(0)).expect("open should succeed");
        let err = host
            .seek(handle, rv(-1))
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
        assert_eq!(host.msg_box(rv(100), rv(3)).expect("msg_box"), rv(3));
        assert_eq!(host.input_box(rv(100), rv(7)).expect("input_box"), rv(7));

        let host_disabled = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                ui_virtualization: crate::model::UiVirtualizationMode::Disabled,
                ..policy
            },
        );
        assert_eq!(
            host_disabled.msg_box(rv(100), rv(3)).expect("msg_box"),
            rv(100)
        );
        assert_eq!(
            host_disabled.input_box(rv(100), rv(7)).expect("input_box"),
            rv(100)
        );
    }

    #[test]
    fn ui_runtime_value_lanes_preserve_string_inputs() {
        let policy = HostPolicy {
            allow_interaction: true,
            ui_virtualization: crate::model::UiVirtualizationMode::ScriptedResponses,
            ..HostPolicy::default()
        };
        let host = StandardHostServices::new(HalProfileId::Windows, policy.clone());
        assert_eq!(
            host.msg_box(
                RuntimeValue::String(BStr("Prompt".to_string())),
                RuntimeValue::I32(3),
            )
            .expect("msg_box"),
            RuntimeValue::I32(3)
        );
        assert_eq!(
            host.input_box(
                RuntimeValue::String(BStr("Prompt".to_string())),
                RuntimeValue::String(BStr("Default".to_string())),
            )
            .expect("input_box"),
            RuntimeValue::String(BStr("Default".to_string()))
        );
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
        let err = host
            .msg_box(rv(9), rv(1))
            .expect_err("msg_box should be denied");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        let err = host
            .input_box(rv(9), rv(1))
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
            host.shell(rv(1), rv(0)).expect_err("shell deny").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.create_object_test(1).expect_err("com deny").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.invoke_symbol(1.into(), rv(2))
                .expect_err("dynlink deny")
                .kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn process_runtime_value_lanes_accept_string_inputs_in_deterministic_mode() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.shell(
                RuntimeValue::String(BStr("echo hi".to_string())),
                RuntimeValue::I32(0),
            )
            .expect("shell"),
            RuntimeValue::I32(1)
        );
        assert_eq!(
            host.environ(RuntimeValue::String(BStr("PATH".to_string())))
                .expect("environ"),
            RuntimeValue::I32(4)
        );
        assert_eq!(
            host.dir(
                RuntimeValue::String(BStr("folder".to_string())),
                RuntimeValue::I32(0),
            )
            .expect("dir"),
            RuntimeValue::I32(1)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_string_variant_roundtrips_through_adapter_helpers() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::String(BStr("Hello".to_string()));
        let mut resolve_object =
            |_handle: ObjectHandle| Err("object dispatch resolution not expected".to_string());
        unsafe {
            set_variant_dispatch_arg(&mut variant, &value, &mut resolve_object)
                .expect("set string variant");
            assert_eq!(
                raw_variant_to_com_value(&variant).expect("read string variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_object_handle_variant_uses_dispatch_pointer_lane() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let dispatch = create_oxvba_test_dispatch();
        let value = ComValue::ObjectHandle(ObjectHandle::new(20_001));
        let mut resolve_object = |_handle: ObjectHandle| Ok(dispatch);
        unsafe {
            set_variant_dispatch_arg(&mut variant, &value, &mut resolve_object)
                .expect("set object-handle variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_DISPATCH);
            assert!(!variant.Anonymous.Anonymous.Anonymous.pdispVal.is_null());
            let _ = VariantClear(&mut variant);
            raw_release_dispatch(dispatch);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_safe_array_variant_roundtrips_through_adapter_helpers() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::ArrayIntent(oxvba_runtime::safe_array::SafeArray::from_values(vec![
            RuntimeValue::I32(4),
            RuntimeValue::Bool(true),
            RuntimeValue::String(BStr("Hello".to_string())),
            RuntimeValue::Null,
        ]));
        let mut resolve_object =
            |_handle: ObjectHandle| Err("object dispatch resolution not expected".to_string());
        unsafe {
            set_variant_dispatch_arg(&mut variant, &value, &mut resolve_object)
                .expect("set SAFEARRAY variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_ARRAY | VT_VARIANT);
            assert_eq!(
                raw_variant_to_com_value(&variant).expect("read SAFEARRAY variant"),
                value
            );
            let _ = VariantClear(&mut variant);
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn dispatch_result_variant_binds_runtime_object_handle() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let dispatch = create_oxvba_test_dispatch();
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        variant.Anonymous.Anonymous.vt = VT_DISPATCH;
        variant.Anonymous.Anonymous.Anonymous.pdispVal = dispatch.cast();
        let classified = unsafe {
            com_take_variant_result_value(
                &mut variant,
                &mut |unknown| {
                    raw_query_dispatch_from_unknown(unknown.cast::<RawIUnknown>())
                        .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
                &mut |dispatch| {
                    raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
                },
            )
        }
        .expect("classify dispatch result");
        let value = match classified {
            VariantResultValue::Value(value) => value.to_runtime_value(),
            VariantResultValue::Dispatch(dispatch) => host
                .bind_native_dispatch_result(
                    dispatch.cast::<RawIDispatch>(),
                    "OxVba.TestDispatch",
                    "dispatch_invoke",
                )
                .expect("bind dispatch result"),
        };
        let RuntimeValue::ObjectHandle(handle) = value else {
            panic!("expected object handle runtime value");
        };
        assert!(handle.raw() >= 20_001);
        assert_eq!(
            host.release_object_test(handle)
                .expect("release_object should succeed"),
            1
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn unknown_variant_result_binds_runtime_object_handle_when_dispatch_is_available() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let dispatch = create_oxvba_test_dispatch();
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        variant.Anonymous.Anonymous.vt = VT_UNKNOWN;
        variant.Anonymous.Anonymous.Anonymous.punkVal = dispatch.cast();
        let classified = unsafe {
            com_take_variant_result_value(
                &mut variant,
                &mut |unknown| {
                    raw_query_dispatch_from_unknown(unknown.cast::<RawIUnknown>())
                        .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
                &mut |dispatch| {
                    raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
                },
            )
        }
        .expect("classify unknown result");
        let value = match classified {
            VariantResultValue::Value(value) => value.to_runtime_value(),
            VariantResultValue::Dispatch(dispatch) => host
                .bind_native_dispatch_result(
                    dispatch.cast::<RawIDispatch>(),
                    "OxVba.TestDispatch",
                    "dispatch_invoke",
                )
                .expect("bind unknown-dispatch result"),
        };
        let RuntimeValue::ObjectHandle(handle) = value else {
            panic!("expected object handle runtime value");
        };
        assert!(handle.raw() >= 20_001);
        assert_eq!(
            host.release_object_test(handle)
                .expect("release_object should succeed"),
            1
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
            symbol: 7.into(),
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
            symbol: 7.into(),
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
            symbol: 7.into(),
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
            symbol: 7.into(),
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
        assert_eq!(
            host.date_serial_now().expect("date"),
            RuntimeValue::I32(20_260_301)
        );
        assert_eq!(
            host.time_serial_now().expect("time"),
            RuntimeValue::I32(123_456)
        );
        assert_eq!(host.timer_ticks().expect("timer"), RuntimeValue::I32(42));
    }

    #[test]
    fn process_env_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.environ(rv(88)).expect("environ"), rv(88));
        assert_eq!(host.dir(rv(0), rv(0)).expect("dir"), rv(0));
        assert_eq!(host.dir(rv(5), rv(0)).expect("dir"), rv(1));
    }

    #[test]
    fn dispatch_invoke_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.dispatch_invoke_legacy(10, 20, 30).expect("dispatch"),
            60
        );
    }

    #[test]
    fn dispatch_invoke_missing_arg_token_projects_as_zero() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.dispatch_invoke_legacy(10, 20, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
                .expect("dispatch"),
            30
        );
    }

    #[test]
    fn diagnostics_emit_contract_is_deterministic() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.emit(rv(4), rv(5)).expect("emit"), rv(9));
    }

    #[test]
    fn event_pump_supported_and_unsupported_paths() {
        let windows = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            windows.do_events().expect("windows do_events"),
            RuntimeValue::I32(0)
        );

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
        assert_eq!(
            host.free_file(rv(0)).expect("first free should be 1"),
            rv(1)
        );
        let err = host
            .open(rv(10), rv(1))
            .expect_err("mutation open should be denied by policy");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        assert_eq!(
            host.free_file(rv(0))
                .expect("free file should remain unchanged"),
            rv(1)
        );
    }

    #[test]
    fn invalid_close_does_not_mutate_handle_state() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host.open(rv(10), rv(0)).expect("open should succeed");
        assert_eq!(first, rv(1));
        let err = host.close(rv(99)).expect_err("invalid close should fail");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert_eq!(
            host.free_file(rv(0))
                .expect("free file should still skip handle 1"),
            rv(2)
        );
    }

    #[test]
    fn ui_msg_box_enforces_policy_and_capability_failures() {
        let denied_host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let err = denied_host
            .msg_box(rv(1), rv(1))
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
            .msg_box(rv(1), rv(1))
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
            windows
                .shell(rv(1), rv(0))
                .expect_err("windows shell denial")
                .kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            linux
                .shell(rv(1), rv(0))
                .expect_err("linux shell denial")
                .kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn native_mode_process_and_env_paths_are_callable() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        let shell = expect_i32(
            host.shell(rv(1), rv(0))
                .expect("native shell should succeed"),
        );
        assert!(shell >= 1);
        let environ = expect_i32(host.environ(rv(3)).expect("native environ should succeed"));
        assert!(environ >= 0);
        let dir = expect_i32(host.dir(rv(0), rv(0)).expect("native dir should succeed"));
        assert!(dir == 0 || dir == 1);
    }

    #[test]
    fn native_mode_filesystem_seek_can_extend_length() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        let handle = host
            .open(rv(31415), rv(1))
            .expect("native open should succeed");
        host.seek(handle.clone(), rv(64))
            .expect("native seek should succeed");
        assert!(
            expect_i32(host.lof(handle.clone()).expect("native lof should succeed")) >= 64,
            "native seek in mutation mode should extend logical length"
        );
        assert_eq!(
            host.close(handle).expect("native close should succeed"),
            rv(1)
        );
    }

    #[test]
    fn native_mode_time_tokens_are_non_negative() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        assert!(expect_i32(host.date_serial_now().expect("date")) >= 0);
        assert!(expect_i32(host.time_serial_now().expect("time")) >= 0);
        assert!(expect_i32(host.timer_ticks().expect("ticks")) >= 0);
    }

    #[test]
    fn com_event_subscription_lane_requires_native_mode() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let subscribe = host
            .subscribe_event(rv(1), rv(1))
            .expect_err("subscribe_event should require native mode");
        assert_eq!(subscribe.kind, HalErrorKind::AdapterFault);
        assert_eq!(subscribe.operation, "subscribe_event");
        assert!(subscribe.message.contains("COM-E-EVENT-PATH-UNSUPPORTED"));

        let unsubscribe = host
            .unsubscribe_event(rv(1))
            .expect_err("unsubscribe_event should require native mode");
        assert_eq!(unsubscribe.kind, HalErrorKind::AdapterFault);
        assert_eq!(unsubscribe.operation, "unsubscribe_event");
        assert!(unsubscribe.message.contains("COM-E-EVENT-PATH-UNSUPPORTED"));
        assert!(
            host.event_callback_subscription(rv(60_001))
                .expect_err("event_callback_subscription should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.event_callback_arity(rv(60_001))
                .expect_err("event_callback_arity should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.event_callback_arg(rv(60_001), rv(0))
                .expect_err("event_callback_arg should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.release_event_callback(rv(60_001))
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
            .create_object_test(4)
            .expect("create_object should return a token");
        assert!(
            object.raw() >= 20_001,
            "controlled COM lane should bind native object"
        );
        let subscription = host
            .subscribe_event(rv(object.into()), rv(1))
            .expect("subscribe_event should succeed for controlled event source");
        let subscription = expect_i32(subscription);
        assert!(subscription >= 40_001);
        {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            let registered = state
                .subscriptions
                .get(&subscription.into())
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
            host.dispatch_invoke_legacy(object.into(), 3, 77)
                .expect("FireChanged should succeed"),
            77
        );
        let callback = host
            .do_events()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);
        assert!(callback >= 60_001);
        assert_eq!(
            host.event_callback_subscription(rv(callback))
                .expect("callback subscription lookup should succeed"),
            rv(subscription)
        );
        assert_eq!(
            host.event_callback_arg(rv(callback), rv(0))
                .expect("callback arg lookup should succeed"),
            rv(77)
        );
        assert_eq!(
            host.event_callback_arity(rv(callback))
                .expect("callback arity lookup should succeed"),
            rv(1)
        );
        assert_eq!(
            host.release_event_callback(rv(callback))
                .expect("callback release should succeed"),
            rv(1)
        );
        assert_eq!(
            host.do_events().expect("callback queue should be drained"),
            RuntimeValue::I32(0),
            "native callback lane should not enqueue duplicate projection callbacks"
        );

        assert_eq!(
            host.unsubscribe_event(rv(subscription))
                .expect("unsubscribe_event should succeed"),
            rv(1)
        );
        let _ = host
            .dispatch_invoke_legacy(object.into(), 3, 88)
            .expect("FireChanged should remain invokable after unsubscribe");
        assert_eq!(
            host.do_events()
                .expect("callback queue should remain empty after unsubscribe"),
            RuntimeValue::I32(0)
        );
        let callback_still_present = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state.callbacks.contains_key(&callback.into())
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
            .create_object_test(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(rv(object.into()), rv(super::TEST_EVENT_CHANGED_PAIR))
            .expect("subscribe_event should succeed for controlled pair-event source");
        let subscription = expect_i32(subscription);

        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), super::TEST_DISPID_FIRE_CHANGED_PAIR, 90)
                .expect("FireChangedPair should succeed"),
            91
        );
        let callback = host
            .do_events()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);
        assert!(callback >= 60_001);
        assert_eq!(
            host.event_callback_subscription(rv(callback))
                .expect("callback subscription lookup should succeed"),
            rv(subscription)
        );
        assert_eq!(
            host.event_callback_arity(rv(callback))
                .expect("callback arity lookup should succeed"),
            rv(2)
        );
        assert_eq!(
            host.event_callback_arg(rv(callback), rv(0))
                .expect("callback arg0 lookup should succeed"),
            rv(90)
        );
        assert_eq!(
            host.event_callback_arg(rv(callback), rv(1))
                .expect("callback arg1 lookup should succeed"),
            rv(91)
        );
        let err = host
            .event_callback_arg(rv(callback), rv(2))
            .expect_err("index beyond callback arity should fail");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message
                .contains("COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH")
        );
        assert_eq!(
            host.release_event_callback(rv(callback))
                .expect("callback release should succeed"),
            rv(1)
        );
        assert_eq!(
            host.unsubscribe_event(rv(subscription))
                .expect("unsubscribe_event should succeed"),
            rv(1)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_poll_returns_structured_payload() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(rv(object.into()), rv(super::TEST_EVENT_CHANGED_PAIR))
            .expect("subscribe_event should succeed for controlled pair-event source");
        let subscription = expect_i32(subscription);

        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), super::TEST_DISPID_FIRE_CHANGED_PAIR, 90)
                .expect("FireChangedPair should succeed"),
            91
        );
        let callback = host
            .do_events()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);
        let payload = host
            .poll_event_callback()
            .expect("poll_event_callback should succeed")
            .expect("callback payload should be available");
        assert_eq!(payload.callback.raw(), callback);
        assert_eq!(payload.subscription.raw(), subscription);
        assert_eq!(payload.object.raw(), object.raw());
        assert_eq!(payload.event.raw(), super::TEST_EVENT_CHANGED_PAIR);
        assert_eq!(payload.args, vec![ComValue::I32(90), ComValue::I32(91)]);
        assert!(
            host.poll_event_callback()
                .expect("poll_event_callback should succeed when queue is empty")
                .is_none()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_release_object_clears_subscriptions_and_callbacks() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(rv(object.into()), rv(1))
            .expect("subscribe_event should succeed for controlled event source");
        let subscription = expect_i32(subscription);
        host.dispatch_invoke_legacy(object.into(), 3, 77)
            .expect("FireChanged should succeed");
        let callback = host
            .do_events()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);

        assert_eq!(host.release_object_test(object).expect("release_object"), 1);
        let callback_err = host
            .event_callback_subscription(rv(callback))
            .expect_err("released object callback should be gone");
        assert!(
            callback_err
                .message
                .contains("COM-E-EVENT-CALLBACK-MISSING")
        );
        let subscription_err = host
            .unsubscribe_event(rv(subscription))
            .expect_err("released object subscription should be gone");
        assert!(
            subscription_err
                .message
                .contains("COM-E-EVENT-ADVISE-FAILED")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_subscription_rejects_unknown_event_token() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let err = host
            .subscribe_event(rv(object.into()), rv(7))
            .expect_err("unknown event token should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CONNECTIONPOINT-MISSING"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_subscription_supports_controlled_com_evt_b_source_interface_lane() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(
                rv(object.into()),
                rv(super::TEST_EVENT_CHANGED_SOURCE_INTERFACE),
            )
            .expect("controlled source-interface event token should subscribe successfully");
        let subscription = expect_i32(subscription);
        assert!(
            subscription >= 40_001,
            "subscription token should be in deterministic range"
        );
        assert_eq!(
            host.dispatch_invoke_legacy(
                object.into(),
                super::TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE,
                77
            )
            .expect("FireChangedSourceInterface should succeed"),
            77
        );
        let callback = host
            .do_events()
            .expect("do_events should pump pending source-interface callback");
        let callback = expect_i32(callback);
        assert_eq!(
            host.event_callback_subscription(rv(callback))
                .expect("callback subscription lookup should succeed"),
            rv(subscription)
        );
        assert_eq!(
            host.event_callback_arity(rv(callback))
                .expect("callback arity lookup should succeed"),
            rv(1)
        );
        assert!(
            host.event_callback_arg(rv(callback), rv(0))
                .expect("callback arg0 lookup should succeed")
                == rv(77)
        );
        assert_eq!(
            host.release_event_callback(rv(callback))
                .expect("callback release should succeed"),
            rv(1)
        );
        assert_eq!(
            host.unsubscribe_event(rv(subscription))
                .expect("unsubscribe should succeed"),
            rv(1)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_unsubscribe_rejects_unknown_subscription() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let err = host
            .unsubscribe_event(rv(40_999))
            .expect_err("unknown subscription should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-ADVISE-FAILED"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_callback_lookup_rejects_unknown_callback() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let err = host
            .event_callback_subscription(rv(60_999))
            .expect_err("unknown callback should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CALLBACK-MISSING"));
        let err = host
            .event_callback_arity(rv(60_999))
            .expect_err("unknown callback arity lookup should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CALLBACK-MISSING"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_callback_arg_index_is_validated() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(rv(object.into()), rv(1))
            .expect("subscribe should succeed");
        let subscription = expect_i32(subscription);
        let _ = host
            .dispatch_invoke_legacy(object.into(), 3, 77)
            .expect("FireChanged should succeed");
        let callback = host.do_events().expect("callback token");
        let callback = expect_i32(callback);
        let err = host
            .event_callback_arg(rv(callback), rv(1))
            .expect_err("only callback arg index 0 should be supported");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message
                .contains("COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH")
        );
        assert_eq!(
            host.release_event_callback(rv(callback))
                .expect("release callback should succeed"),
            rv(1)
        );
        assert_eq!(
            host.unsubscribe_event(rv(subscription))
                .expect("unsubscribe should succeed"),
            rv(1)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_mode_persists_mutation_to_host_file() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let token = 424_242;
        let host_path = host.host_path_from_token(token);
        let _ = std::fs::remove_file(&host_path);

        let handle = host
            .open(rv(token), rv(1))
            .expect("native open should succeed");
        host.seek(handle.clone(), rv(160))
            .expect("native seek should succeed");
        assert_eq!(
            host.close(handle).expect("native close should succeed"),
            rv(1)
        );

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
            .create_object_test(4)
            .expect("create_object should return a token");

        if object.raw() == 5_004 {
            // Environment lacks native activation prerequisites; deterministic fallback remains valid.
            return;
        }

        assert!(
            object.raw() >= 20_001,
            "native COM handles use COM-state handle space"
        );
        let count = host
            .dispatch_invoke_legacy(object.into(), 1, 0)
            .expect("dictionary Count should be invokable");
        assert!(count >= 0);

        let exists = host
            .dispatch_invoke_legacy(object.into(), 2, 42)
            .expect("dictionary Exists should be invokable");
        assert!(exists == 0 || exists == 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_returns_deterministic_values() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        assert!(
            object.raw() >= 20_001,
            "controlled COM lane should bind native object"
        );
        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
                .expect("Count property-get should succeed"),
            7
        );
        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), 2, 42)
                .expect("Exists(42) should succeed"),
            1
        );
        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), 2, 41)
                .expect("Exists(41) should succeed"),
            0
        );
        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), super::TEST_DISPID_PING, 999)
                .expect("Ping no-arg method invoke should succeed"),
            123
        );
        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), super::TEST_DISPID_LOOKUP, 42)
                .expect("Lookup property-get with argument should succeed"),
            1_042
        );
        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), super::TEST_DISPID_SET_VALUE, 33)
                .expect("SetValue property-put should succeed"),
            33
        );
        assert_eq!(
            host.dispatch_invoke_legacy(
                object.into(),
                super::TEST_DISPID_VALUE,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN
            )
            .expect("Value property-get should reflect SetValue"),
            33
        );
        assert_eq!(
            host.dispatch_invoke_legacy(object.into(), super::TEST_DISPID_SET_VALUE_REF, 33)
                .expect("SetValueRef property-putref should succeed"),
            100_033
        );
        assert_eq!(
            host.dispatch_invoke_legacy(
                object.into(),
                super::TEST_DISPID_VALUE,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN
            )
            .expect("Value property-get should reflect SetValueRef"),
            100_033
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_supports_named_method_args_runtime_value_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: super::TEST_DISPID_SUM_PAIR.into(),
            args: vec![
                ComInvokeArg::named(14, "rhs"),
                ComInvokeArg::named(3, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_runtime_value_v2(&request)
                    .expect("named-argument SumPair invoke should succeed")
            ),
            3_014
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_supports_named_property_get_args_runtime_value_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: super::TEST_DISPID_LOOKUP_PAIR.into(),
            args: vec![
                ComInvokeArg::named(14, "rhs"),
                ComInvokeArg::named(3, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_runtime_value_v2(&request)
                    .expect("named property-get invoke should succeed")
            ),
            203_014
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_supports_named_default_member_args_runtime_value_v2()
    {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: 0.into(),
            args: vec![ComInvokeArg::named(19, "value")],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_runtime_value_v2(&request)
                    .expect("named default-member invoke should succeed")
            ),
            19
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_dictionary_named_default_member_requires_identity_v2() {
        let mut policy = HostPolicy::interactive_dev();
        policy
            .com_prog_id_overrides
            .insert(4, "Scripting.Dictionary".to_string());
        let host = StandardHostServices::new(HalProfileId::Windows, policy);
        let object = host
            .create_object_test(4)
            .expect("create_object should return dictionary token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: 0.into(),
            args: vec![ComInvokeArg::named(19, "value")],
            invoke_kind_hint: None,
        };
        let err = host
            .dispatch_invoke_runtime_value_v2(&request)
            .expect_err("named default-member invoke should fail without identity");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message
                .contains("default member identity unavailable for named late-bound dispatch"),
            "expected precise default-member blocker, got {}",
            err.message
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_preserves_omitted_arg_metadata_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: super::TEST_DISPID_LOOKUP.into(),
            args: vec![ComInvokeArg::omitted()],
            invoke_kind_hint: None,
        };
        let err = host
            .dispatch_invoke_legacy_v2(&request)
            .expect_err("omitted required argument should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("member requires argument"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_named_property_put_value_uses_propertyput_lane_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: super::TEST_DISPID_SET_INDEXED_VALUE.into(),
            args: vec![ComInvokeArg::positional(7), ComInvokeArg::named(9, "value")],
            invoke_kind_hint: None,
        };
        assert_eq!(
            host.dispatch_invoke_legacy_v2(&request)
                .expect("named value argument should still route through property-put lane"),
            307_009
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_roundtrips_semantic_safe_array_payload_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let expected =
            RuntimeValue::ArrayIntent(oxvba_runtime::safe_array::SafeArray::from_values(vec![
                RuntimeValue::I32(4),
                RuntimeValue::Bool(true),
                RuntimeValue::String(BStr("Hello".to_string())),
                RuntimeValue::Null,
            ]));
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: super::TEST_DISPID_ECHO_VARIANT.into(),
            args: vec![ComInvokeArg::positional_value(
                ComValue::from_runtime_value(&expected),
            )],
            invoke_kind_hint: Some(oxvba_com::ComInvokeKind::Method),
        };
        assert_eq!(
            host.dispatch_invoke_runtime_value_v2(&request)
                .expect("semantic SAFEARRAY invoke should succeed"),
            expected
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_named_indexed_property_put_reorders_value_last_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: super::TEST_DISPID_SET_INDEXED_VALUE.into(),
            args: vec![
                ComInvokeArg::named(9, "value"),
                ComInvokeArg::named(7, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            host.dispatch_invoke_legacy_v2(&request)
                .expect("fully named indexed property-put should canonicalize deterministically"),
            307_009
        );
        assert_eq!(
            host.dispatch_invoke_legacy(
                object.into(),
                super::TEST_DISPID_VALUE,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN
            )
            .expect("Value property-get should reflect named indexed property-put"),
            307_009
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_named_indexed_property_putref_reorders_value_last_v2()
     {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.raw().into(),
            member: super::TEST_DISPID_SET_INDEXED_VALUE_REF.into(),
            args: vec![
                ComInvokeArg::named(13, "value"),
                ComInvokeArg::named(8, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            host.dispatch_invoke_legacy_v2(&request).expect(
                "fully named indexed property-putref should canonicalize deterministically"
            ),
            408_013
        );
        assert_eq!(
            host.dispatch_invoke_legacy(
                object.into(),
                super::TEST_DISPID_VALUE,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN
            )
            .expect("Value property-get should reflect named indexed property-putref"),
            408_013
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_property_get_with_required_arg_reports_missing_arg_stably() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let err = host
            .dispatch_invoke_legacy(
                object.into(),
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
    fn windows_native_controlled_exception_surfaces_excepinfo_without_fake_arg_error() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let err = host
            .dispatch_invoke_legacy(
                object.into(),
                super::TEST_DISPID_RAISE_EXCEPTION,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN,
            )
            .expect_err("RaiseException should surface an adapter fault");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message.contains("com-dispatch-exception-raised"),
            "expected exception-raised classification, got {}",
            err.message
        );
        assert!(
            err.message
                .contains("excep_description=\"controlled dispatch exception\""),
            "expected EXCEPINFO description in adapter fault, got {}",
            err.message
        );
        assert!(
            !err.message.contains("arg_err="),
            "unexpected arg_err classification leak in {}",
            err.message
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_binding_keeps_stable_dispatch_identity() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        if object.raw() == 5_004 {
            return;
        }

        let before = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            let binding = state
                .bindings
                .get(&ComObjectToken::new(object.raw()))
                .expect("native object should be tracked");
            assert!(
                binding.native_dispatch != 0,
                "native COM binding should hold a dispatch pointer"
            );
            binding.native_dispatch
        };
        let _ = host
            .dispatch_invoke_legacy(object.into(), 1, 0)
            .expect("dispatch invoke should succeed");
        let after = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state
                .bindings
                .get(&ComObjectToken::new(object.raw()))
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
            .create_object_test(4)
            .expect("create_object should return a token");
        if object.raw() == 5_004 {
            return;
        }

        let _ = host
            .dispatch_invoke_legacy(object.into(), 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
            .expect("dictionary Count should be invokable");
        let cache_size_after_first = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state
                .bindings
                .get(&ComObjectToken::new(object.raw()))
                .expect("binding should remain tracked")
                .member_dispids
                .len()
        };
        let _ = host
            .dispatch_invoke_legacy(object.into(), 1, super::DISPATCH_INVOKE_MISSING_ARG_TOKEN)
            .expect("dictionary Count should be invokable repeatedly");
        let cache_size_after_second = {
            let state = host
                .com_state
                .lock()
                .expect("com state lock should succeed");
            state
                .bindings
                .get(&ComObjectToken::new(object.raw()))
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
            .create_object_test(4)
            .expect("dispatch create_object should succeed");
        let vtable_object = vtable_host
            .create_object_test(4)
            .expect("vtable create_object should succeed");

        let dispatch_count = dispatch_host
            .dispatch_invoke_legacy(
                dispatch_object.into(),
                1,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN,
            )
            .expect("dispatch count should succeed");
        let vtable_count = vtable_host
            .dispatch_invoke_legacy(
                vtable_object.into(),
                1,
                super::DISPATCH_INVOKE_MISSING_ARG_TOKEN,
            )
            .expect("vtable count should succeed");
        assert_eq!(dispatch_count, vtable_count);

        let dispatch_exists = dispatch_host
            .dispatch_invoke_legacy(dispatch_object.into(), 2, 42)
            .expect("dispatch exists should succeed");
        let vtable_exists = vtable_host
            .dispatch_invoke_legacy(vtable_object.into(), 2, 42)
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
            .invalidate_typelib_cache_test(TypeLibCacheScope::Global, None)
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
            .invalidate_typelib_cache_test(TypeLibCacheScope::Reference, Some("StdOle"))
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
                ("SumPair".to_string(), super::TEST_DISPID_SUM_PAIR),
                ("LookupPair".to_string(), super::TEST_DISPID_LOOKUP_PAIR),
                (
                    "SetIndexedValue".to_string(),
                    super::TEST_DISPID_SET_INDEXED_VALUE
                ),
                (
                    "SetIndexedValueRef".to_string(),
                    super::TEST_DISPID_SET_INDEXED_VALUE_REF
                ),
                ("EchoVariant".to_string(), 16),
                (
                    "RaiseException".to_string(),
                    super::TEST_DISPID_RAISE_EXCEPTION
                ),
                (
                    "ReturnSmallInt".to_string(),
                    super::TEST_DISPID_RETURN_SMALLINT
                ),
                (
                    "ReturnUnsignedWord".to_string(),
                    super::TEST_DISPID_RETURN_UNSIGNED_WORD
                ),
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
        let raise_exception_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_RAISE_EXCEPTION)
            .expect("RaiseException metadata should exist");
        assert_eq!(
            raise_exception_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        assert!(!raise_exception_member.requires_argument);
        let return_smallint_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_RETURN_SMALLINT)
            .expect("ReturnSmallInt metadata should exist");
        assert_eq!(
            return_smallint_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        assert!(!return_smallint_member.requires_argument);
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
        let sum_pair_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_SUM_PAIR)
            .expect("SumPair metadata should exist");
        assert!(sum_pair_member.requires_argument);
        assert_eq!(
            sum_pair_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let lookup_pair_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_LOOKUP_PAIR)
            .expect("LookupPair metadata should exist");
        assert!(lookup_pair_member.requires_argument);
        assert_eq!(
            lookup_pair_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let set_indexed_value_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_SET_INDEXED_VALUE)
            .expect("SetIndexedValue metadata should exist");
        assert!(set_indexed_value_member.requires_argument);
        assert_eq!(
            set_indexed_value_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPut
        );
        let set_indexed_value_ref_member = metadata
            .members
            .iter()
            .find(|entry| entry.token == super::TEST_DISPID_SET_INDEXED_VALUE_REF)
            .expect("SetIndexedValueRef metadata should exist");
        assert!(set_indexed_value_ref_member.requires_argument);
        assert_eq!(
            set_indexed_value_ref_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPutRef
        );
        let source_interface_event = metadata
            .events
            .iter()
            .find(|entry| entry.token == super::TEST_EVENT_CHANGED_SOURCE_INTERFACE)
            .expect("source-interface event metadata should exist");
        assert_eq!(source_interface_event.callback_arity, 1);
        assert_eq!(
            source_interface_event.dispatch_path,
            oxvba_com::TypeLibEventDispatchPath::SourceInterface
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
            oxvba_com::TypeLibEventDispatchPath::Dispatch
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
            .create_object_test(4)
            .expect("create_object should return a token");
        let state = host
            .com_state
            .lock()
            .expect("com state lock should succeed");
        let binding = state
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .expect("binding should be present for native object token");
        let member = binding
            .member_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_PAIR.into())
            .expect("member spec for FireChangedPair should be present");
        assert_eq!(member.name, "FireChangedPair");
        assert!(member.requires_argument);
        assert_eq!(member.invoke_kind, super::TypeLibMemberInvokeKind::Method);
        let ping = binding
            .member_specs
            .get(&super::TEST_DISPID_PING.into())
            .expect("member spec for Ping should be present");
        assert_eq!(ping.name, "Ping");
        assert!(!ping.requires_argument);
        assert_eq!(ping.invoke_kind, super::TypeLibMemberInvokeKind::Method);
        let fire_changed_source = binding
            .member_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE.into())
            .expect("member spec for FireChangedSourceInterface should be present");
        assert_eq!(fire_changed_source.name, "FireChangedSourceInterface");
        assert!(fire_changed_source.requires_argument);
        assert_eq!(
            fire_changed_source.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let lookup = binding
            .member_specs
            .get(&super::TEST_DISPID_LOOKUP.into())
            .expect("member spec for Lookup should be present");
        assert_eq!(lookup.name, "Lookup");
        assert!(lookup.requires_argument);
        assert_eq!(
            lookup.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let set_value = binding
            .member_specs
            .get(&super::TEST_DISPID_SET_VALUE.into())
            .expect("member spec for SetValue should be present");
        assert_eq!(set_value.name, "SetValue");
        assert!(set_value.requires_argument);
        assert_eq!(
            set_value.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPut
        );
        let set_value_ref = binding
            .member_specs
            .get(&super::TEST_DISPID_SET_VALUE_REF.into())
            .expect("member spec for SetValueRef should be present");
        assert_eq!(set_value_ref.name, "SetValueRef");
        assert!(set_value_ref.requires_argument);
        assert_eq!(
            set_value_ref.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPutRef
        );
        let value = binding
            .member_specs
            .get(&super::TEST_DISPID_VALUE.into())
            .expect("member spec for Value should be present");
        assert_eq!(value.name, "Value");
        assert!(!value.requires_argument);
        assert_eq!(
            value.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let dispatch_event = binding
            .event_specs
            .get(&super::TEST_EVENT_CHANGED_PAIR.into())
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
            .get(&super::TEST_EVENT_CHANGED_SOURCE_INTERFACE.into())
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
            .get(&super::TEST_DISPID_FIRE_CHANGED.into())
            .expect("FireChanged trigger spec should be present");
        assert_eq!(
            fire_changed_trigger.event_token,
            super::TEST_EVENT_CHANGED.into()
        );
        assert_eq!(fire_changed_trigger.callback_arity, 1);
        assert!(!fire_changed_trigger.second_arg_is_incremented);
        let fire_changed_pair_trigger = binding
            .event_trigger_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_PAIR.into())
            .expect("FireChangedPair trigger spec should be present");
        assert_eq!(
            fire_changed_pair_trigger.event_token,
            super::TEST_EVENT_CHANGED_PAIR.into()
        );
        assert_eq!(fire_changed_pair_trigger.callback_arity, 2);
        assert!(fire_changed_pair_trigger.second_arg_is_incremented);
        let fire_changed_source_trigger = binding
            .event_trigger_specs
            .get(&super::TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE.into())
            .expect("FireChangedSourceInterface trigger spec should be present");
        assert_eq!(
            fire_changed_source_trigger.event_token,
            super::TEST_EVENT_CHANGED_SOURCE_INTERFACE.into()
        );
        assert_eq!(fire_changed_source_trigger.callback_arity, 1);
        assert!(!fire_changed_source_trigger.second_arg_is_incremented);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_object_descriptor_reports_identity_and_capabilities() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return a token");
        let descriptor = host
            .describe_object(object)
            .expect("describe_object should succeed")
            .expect("known COM object should produce a descriptor");

        assert_eq!(descriptor.object.raw(), object.raw());
        assert_eq!(descriptor.prog_id_name, "OxVba.TestDispatch");
        assert_eq!(
            descriptor.transport,
            oxvba_com::ComObjectTransportKind::NativeDispatch
        );
        assert!(descriptor.supports_events);
        assert!(
            descriptor
                .known_member_tokens
                .contains(&super::TEST_DISPID_COUNT.into())
        );
        assert!(
            descriptor
                .known_member_tokens
                .contains(&super::TEST_DISPID_FIRE_CHANGED_PAIR.into())
        );
        assert_eq!(
            descriptor.default_member_token,
            Some(super::TEST_DISPID_ECHO_VARIANT.into())
        );
        assert_eq!(
            descriptor.default_member_name.as_deref(),
            Some("EchoVariant")
        );
        assert!(
            descriptor
                .known_event_tokens
                .contains(&super::TEST_EVENT_CHANGED.into())
        );
        assert!(
            descriptor
                .known_event_tokens
                .contains(&super::TEST_EVENT_CHANGED_PAIR.into())
        );
        assert_eq!(
            descriptor.typelib_cache_key.as_deref(),
            Some("typelib:oxvba-testdispatch:1.0:0")
        );
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
            .create_object_test(4)
            .expect("create_object should return dictionary token");
        let state = host
            .com_state
            .lock()
            .expect("com state lock should succeed");
        let binding = state
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .expect("binding should be present for dictionary token");
        let exists_member = binding
            .member_specs
            .get(&super::TEST_DISPID_EXISTS.into())
            .expect("Exists member spec should be present");
        assert_eq!(exists_member.name, "Exists");
        assert!(exists_member.requires_argument);
        assert_eq!(
            exists_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let exists_event = binding
            .event_specs
            .get(&super::TEST_EVENT_CHANGED.into())
            .expect("dictionary projection event spec should be present");
        assert_eq!(exists_event.callback_arity, 1);
        assert_eq!(exists_event.path, super::ComEventPath::Dispatch);
        assert!(exists_event.connection_point_iid.is_none());
        let exists_trigger = binding
            .event_trigger_specs
            .get(&super::TEST_DISPID_EXISTS.into())
            .expect("Exists member should project callback trigger");
        assert_eq!(exists_trigger.event_token, super::TEST_EVENT_CHANGED.into());
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
            .create_object_test(4)
            .expect("create_object should return dictionary token");
        let subscription = host
            .subscribe_event(rv(object.into()), rv(super::TEST_EVENT_CHANGED))
            .expect("subscribe_event should succeed for dictionary projection event");
        let subscription = expect_i32(subscription);
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
                .get(&subscription.into())
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
            host.dispatch_invoke_legacy(object.into(), super::TEST_DISPID_EXISTS, 42)
                .expect("Exists invoke should succeed"),
            0
        );
        let callback = host
            .do_events()
            .expect("do_events should return queued dictionary callback");
        let callback = expect_i32(callback);
        assert!(callback >= 60_001, "callback token should be in range");
        assert_eq!(
            host.event_callback_subscription(rv(callback))
                .expect("callback subscription lookup should succeed"),
            rv(subscription)
        );
        assert_eq!(
            host.event_callback_arg(rv(callback), rv(0))
                .expect("callback arg lookup should succeed"),
            rv(42)
        );
        host.release_event_callback(rv(callback))
            .expect("callback release should succeed");
        host.unsubscribe_event(rv(subscription))
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
            .create_object_test(4)
            .expect("policy override should resolve native COM activation");
        assert!(
            object.raw() >= 20_001,
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
            .create_object_test(4)
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

    #[cfg(target_os = "windows")]
    #[test]
    fn release_object_clears_native_subscriptions_and_pending_callbacks() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(4)
            .expect("create_object should return controlled COM object");
        let subscription = host
            .subscribe_event(rv(object.into()), rv(1))
            .expect("subscribe_event should succeed");
        let subscription = expect_i32(subscription);
        let _ = host
            .dispatch_invoke_legacy(object.into(), 3, 77)
            .expect("dispatch_invoke should queue callback");

        assert!(
            host.poll_event_callback()
                .expect("first callback should be available")
                .is_some()
        );

        let _ = host
            .dispatch_invoke_legacy(object.into(), 3, 88)
            .expect("second callback should queue");
        assert_eq!(
            host.release_object_test(object)
                .expect("release_object should clear tracked COM state"),
            1
        );
        assert!(
            host.poll_event_callback()
                .expect("released object should not leave callback payloads behind")
                .is_none()
        );

        let err = host
            .unsubscribe_event(rv(subscription))
            .expect_err("released object subscription should already be removed");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
    }

    proptest! {
        #[test]
        fn prop_free_file_low_range_tracks_open_count(path_seed in 1i32..10_000, open_count in 0usize..32) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            for idx in 0..open_count {
                let path = path_seed.saturating_add(idx as i32);
                let _ = host.open(rv(path), rv(0)).expect("open should succeed");
            }
            let expected = 1 + open_count as i32;
            let free = host.free_file(rv(0)).expect("free_file should succeed");
            prop_assert_eq!(free, rv(expected));
        }

        #[test]
        fn prop_seek_eof_boundary(path_token in 1i32..10_000, offset in 0i32..6000) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            let handle = host.open(rv(path_token), rv(0)).expect("open should succeed");
            let len = expect_i32(host.lof(handle.clone()).expect("lof should succeed"));
            host.seek(handle.clone(), rv(offset)).expect("seek should succeed");
            let eof = host.eof(handle).expect("eof should succeed");
            let expected = if offset >= len { 1 } else { 0 };
            prop_assert_eq!(eof, rv(expected));
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
                scripted.msg_box(rv(prompt), rv(style)).expect("scripted msg_box"),
                rv(style.max(1))
            );
            prop_assert_eq!(
                scripted
                    .input_box(rv(prompt), rv(default_value))
                    .expect("scripted input_box"),
                rv(default_value)
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
                disabled.msg_box(rv(prompt), rv(style)).expect("disabled msg_box"),
                rv(prompt.max(1))
            );
            prop_assert_eq!(
                disabled
                    .input_box(rv(prompt), rv(default_value))
                    .expect("disabled input_box"),
                rv(prompt)
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
                host.msg_box(rv(1), rv(1)).expect_err("msg_box denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.shell(rv(shell_cmd), rv(0))
                    .expect_err("shell denied")
                    .kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.create_object_test(prog_id).expect_err("create_object denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.invoke_symbol(symbol.into(), rv(arg))
                    .expect_err("invoke_symbol denied")
                    .kind,
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
                host.shell(rv(shell_cmd), rv(0)).expect("shell should succeed"),
                RuntimeValue::from_legacy_i32(shell_expected)
            );
            prop_assert_eq!(
                host.create_object_test(prog_id)
                    .expect("create_object should succeed")
                    .raw(),
                5_000i32.saturating_add(prog_id)
            );
            let request = ComInvokeRequest::legacy(object, member, arg);
            let semantic = host
                .dispatch_invoke_runtime_value_v2(&request)
                .expect("semantic dispatch_invoke should succeed");
            prop_assert_eq!(
                host.dispatch_invoke_legacy_v2(&request)
                    .expect("dispatch_invoke legacy projection should succeed"),
                semantic
                    .to_legacy_i32()
                    .expect("semantic dispatch result should project to legacy slot")
            );
            prop_assert_eq!(
                host.invoke_symbol(symbol.into(), rv(arg))
                    .expect("invoke_symbol should succeed"),
                RuntimeValue::I32(symbol.saturating_add(arg))
            );
        }
    }
}
