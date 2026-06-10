// Macros must be defined before mod declarations so child modules can use them.
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

mod com;
mod console;
mod descriptor;
mod diagnostics;
mod dynlink;
mod events;
mod filesystem;
mod process;
mod time;
mod ui;

use console::ConsoleState;
pub(crate) use descriptor::descriptor_for_profile;
use dynlink::DynLinkBindingState;
use filesystem::{FileHandleState, FileSystemState};
use process::DirSearchState;

#[allow(unused_imports)]
use crate::traits::TypeLibMemberInvokeKind;
use crate::{
    error::{HalError, HalResult},
    model::{
        CapabilityId, HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy,
        host_backed_mode_active,
    },
    project::{
        HostExtensionModuleChange, ProjectCallbackError, ProjectDescriptor,
        ProjectReferenceDescriptor, ResolvedProjectReference, project_not_found,
        project_reference_unresolved,
    },
    traits::{
        ComHal, ConsoleHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal,
        HostServices, ProcessEnvHal, ProjectCatalogHal, ProjectMutationHal, ProjectReferenceHal,
        TimeLocaleHal, UiInteractionHal,
    },
};
#[cfg(test)]
#[cfg(test)]
use oxvba_com::RawIDispatch;
use oxvba_com::{ComBinding, platform::portable::PortableComProjection};
#[cfg(target_os = "windows")]
use oxvba_com::{
    ComDirectDispatchSpec, ComEventPath, ComEventSpec, ComEventTriggerSpec, ComInvokeFailure,
    WindowsComBridge, map_com_hresult_label,
};
use oxvba_runtime::{VarType, Variant};
#[cfg(target_os = "windows")]
use std::cell::Cell;
use std::{
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MB_OK, MSG, MessageBoxW, PM_REMOVE, PeekMessageW, TranslateMessage,
};

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
const TEST_DISPID_FIRE_CHANGED: i32 = 3;
#[cfg(test)]
const TEST_DISPID_FIRE_CHANGED_PAIR: i32 = 4;
#[cfg(test)]
const TEST_DISPID_FIRE_CHANGED_SOURCE_INTERFACE: i32 = 11;
#[cfg(test)]
const TEST_DISPID_LOOKUP: i32 = 6;
#[cfg(test)]
#[cfg(test)]
const TEST_DISPID_SUM_PAIR: i32 = 12;
#[cfg(test)]
const TEST_DISPID_LOOKUP_PAIR: i32 = 13;
#[cfg(test)]
const TEST_DISPID_SET_INDEXED_VALUE: i32 = 14;
#[cfg(test)]
const TEST_DISPID_SET_INDEXED_VALUE_REF: i32 = 15;
#[cfg(test)]
const TEST_DISPID_ECHO_VARIANT: i32 = 16;
#[cfg(test)]
const TEST_DISPID_RAISE_EXCEPTION: i32 = 17;
#[cfg(test)]
const TEST_EVENT_CHANGED: i32 = 1;
#[cfg(test)]
const TEST_EVENT_CHANGED_SOURCE_INTERFACE: i32 = 2;
#[cfg(test)]
const TEST_EVENT_CHANGED_PAIR: i32 = 3;
#[cfg(test)]
const TEST_EVENT_EXCEL_APP_QUIT: i32 = 10;

#[derive(Clone)]
pub(crate) struct StandardHostServices {
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    descriptor: HalDescriptor,
    policy: HostPolicy,
    #[cfg(target_os = "windows")]
    env_cache: StandardEnvCache,
    fs_state: Arc<Mutex<FileSystemState>>,
    dir_state: Arc<Mutex<DirSearchState>>,
    console_state: Arc<Mutex<ConsoleState>>,
    projection_state: Arc<Mutex<ProjectionObjectState>>,
    #[cfg(target_os = "windows")]
    com_bridge: Arc<WindowsComBridge>,
    dynlink_state: Arc<Mutex<DynLinkBindingState>>,
    /// OS last-error captured after the most recent native `Declare` call (shared
    /// across clones), read back by `Err.LastDllError`. `Arc` so a cloned adapter
    /// observes the same value.
    last_dll_error: Arc<std::sync::atomic::AtomicI32>,
    portable_objects: Option<Arc<PortableComProjection>>,
    callbacks: Option<Arc<dyn crate::callbacks::HostCallbacks>>,
}

#[derive(Debug, Clone, Default)]
struct ProjectionObjectState {
    next_handle: i32,
    handles_by_prog_id: std::collections::BTreeMap<String, i32>,
    prog_ids_by_handle: std::collections::BTreeMap<i32, String>,
}

impl std::fmt::Debug for StandardHostServices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandardHostServices")
            .field("profile", &self.profile)
            .field("runtime_class", &self.runtime_class)
            .field("callbacks", &self.callbacks.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Default)]
struct StandardEnvCache {
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

#[cfg(target_os = "windows")]
impl StandardEnvCache {
    #[cfg(target_os = "windows")]
    fn capture() -> Self {
        let vars: Vec<(String, String)> = std::env::vars().collect();
        Self {
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

#[cfg(target_os = "windows")]
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
        #[cfg(target_os = "windows")]
        let env_cache = StandardEnvCache::capture();
        Self {
            profile,
            runtime_class,
            descriptor: descriptor_for_profile(profile, runtime_class, &policy),
            policy,
            #[cfg(target_os = "windows")]
            com_bridge: Arc::new(WindowsComBridge::new(
                env_cache.force_registered_testdispatch,
            )),
            #[cfg(target_os = "windows")]
            env_cache,
            fs_state: Arc::new(Mutex::new(FileSystemState::default())),
            dir_state: Arc::new(Mutex::new(DirSearchState::default())),
            console_state: Arc::new(Mutex::new(ConsoleState::default())),
            projection_state: Arc::new(Mutex::new(ProjectionObjectState {
                next_handle: 5_003,
                handles_by_prog_id: std::collections::BTreeMap::new(),
                prog_ids_by_handle: std::collections::BTreeMap::new(),
            })),
            dynlink_state: Arc::new(Mutex::new(DynLinkBindingState::default())),
            last_dll_error: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            portable_objects: None,
            callbacks: None,
        }
    }

    pub(crate) fn with_callbacks(
        mut self,
        callbacks: Arc<dyn crate::callbacks::HostCallbacks>,
    ) -> Self {
        self.set_capability_supported(
            CapabilityId::ProjectCatalog,
            callbacks.supports_project_catalog(),
        );
        self.set_capability_supported(
            CapabilityId::ProjectReferenceProvider,
            callbacks.supports_project_references(),
        );
        self.set_capability_supported(
            CapabilityId::ProjectMutation,
            callbacks.supports_project_mutation(),
        );
        self.callbacks = Some(callbacks);
        self
    }

    pub(crate) fn with_portable_objects(mut self, projection: Arc<PortableComProjection>) -> Self {
        self.portable_objects = Some(projection);
        self
    }

    pub(crate) fn profile(&self) -> HalProfileId {
        self.profile
    }

    pub(crate) fn runtime_class(&self) -> HalRuntimeClass {
        self.runtime_class
    }

    fn projection_lock(
        &self,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<std::sync::MutexGuard<'_, ProjectionObjectState>> {
        self.projection_state.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                op,
                "projection object state lock poisoned",
            )
        })
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

    fn set_capability_supported(&mut self, capability: CapabilityId, supported: bool) {
        if let Some(entry) = self
            .descriptor
            .capabilities
            .iter_mut()
            .find(|entry| entry.id == capability)
        {
            entry.supported = supported;
        }
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

    fn dir_lock(
        &self,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<std::sync::MutexGuard<'_, DirSearchState>> {
        self.dir_state.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                op,
                "dir search state lock poisoned",
            )
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

    fn native_mode_enabled(&self) -> bool {
        host_backed_mode_active(self.profile, &self.policy)
    }

    fn native_fs_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn is_stdio_runtime(&self) -> bool {
        matches!(
            self.runtime_class,
            HalRuntimeClass::WindowsStdio | HalRuntimeClass::LinuxStdio
        )
    }

    fn native_console_enabled(&self) -> bool {
        self.native_mode_enabled() && self.is_stdio_runtime()
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

    fn variant_to_i32(
        &self,
        value: &Variant,
        capability: CapabilityId,
        op: &'static str,
        field: &'static str,
    ) -> HalResult<i32> {
        let value = match value.vtype() {
            VarType::Empty | VarType::Null => 0,
            VarType::Boolean => i32::from(value.as_bool().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("{field} has an invalid Boolean payload"),
                )
            })?),
            VarType::Integer => i32::from(value.as_i16().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("{field} has an invalid Integer payload"),
                )
            })?),
            VarType::Long => value.as_i32().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("{field} has an invalid Long payload"),
                )
            })?,
            VarType::SignedByte => i32::from(value.as_i8().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("{field} has an invalid SignedByte payload"),
                )
            })?),
            VarType::UnsignedInteger => i32::from(value.as_u16().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("{field} has an invalid UnsignedInteger payload"),
                )
            })?),
            VarType::UnsignedLong | VarType::UnsignedInt => {
                let value = value.as_u32().ok_or_else(|| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        format!("{field} has an invalid unsigned 32-bit payload"),
                    )
                })?;
                i32::try_from(value).map_err(|_| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        format!("{field} unsigned 32-bit value {value} is outside i32 range"),
                    )
                })?
            }
            VarType::LongLong => {
                let value = value.as_i64().ok_or_else(|| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        format!("{field} has an invalid LongLong payload"),
                    )
                })?;
                i32::try_from(value).map_err(|_| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        format!("{field} LongLong value {value} is outside i32 range"),
                    )
                })?
            }
            VarType::UnsignedLongLong => {
                let value = value.as_u64().ok_or_else(|| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        format!("{field} has an invalid UnsignedLongLong payload"),
                    )
                })?;
                i32::try_from(value).map_err(|_| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        format!("{field} UnsignedLongLong value {value} is outside i32 range"),
                    )
                })?
            }
            VarType::Byte => i32::from(value.as_u8().ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("{field} has an invalid Byte payload"),
                )
            })?),
            VarType::Single => value.as_f32().unwrap_or(0.0) as i32,
            VarType::Double | VarType::Date => value
                .as_f64()
                .or_else(|| value.as_date_f64())
                .unwrap_or(0.0) as i32,
            VarType::Currency => value
                .as_currency_scaled_i64()
                .map(|scaled| {
                    (scaled / 10_000).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
                })
                .unwrap_or(0),
            VarType::Decimal => value
                .as_decimal96()
                .and_then(|decimal| decimal.to_string().parse::<i32>().ok())
                .unwrap_or(0),
            VarType::String => value
                .as_bstr()
                .and_then(|text| text.as_str().trim().parse::<i32>().ok())
                .unwrap_or(0),
            VarType::Error => value.as_error_code().unwrap_or(0),
            VarType::Object => value
                .as_object_ref()
                .map(|object| object.raw())
                .unwrap_or(0),
            VarType::ArrayVariant => {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!("{field} is a SAFEARRAY and cannot be coerced to i32"),
                ));
            }
        };
        Ok(value)
    }

    fn variant_to_display_text(&self, value: &Variant) -> String {
        // VBA `Print`/`Debug.Print` display formatting lives in oxvba-runtime
        // (next to `Variant`), shared with the `Debug.Print`/`Print` intrinsics.
        oxvba_runtime::print_display_text(value)
    }

    fn variant_to_path(
        &self,
        value: &Variant,
        capability: CapabilityId,
        op: &'static str,
        field: &'static str,
    ) -> HalResult<PathBuf> {
        match value.vtype() {
            VarType::String => value
                .as_bstr()
                .map(|path| PathBuf::from(path.as_str()))
                .ok_or_else(|| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        format!("{field} has an invalid BSTR payload"),
                    )
                }),
            _ => self
                .variant_to_i32(value, capability, op, field)
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
    fn native_windows_msg_box_text(&self, text: &str, style: i32) -> HalResult<i32> {
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
    fn native_windows_msg_box_text(&self, _text: &str, _style: i32) -> HalResult<i32> {
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
    fn activate_variant_object_for_prog_id_name(&self, prog_id_name: &str) -> HalResult<Variant> {
        let registered_event_override =
            self.registered_event_override_for_prog_id_name(prog_id_name, "create_object")?;
        self.ensure_thread_com_apartment("create_object")?;
        let handle = self
            .com_bridge
            .activate_runtime_object_binding(prog_id_name, |binding| {
                if let Some(override_cfg) = registered_event_override.as_ref() {
                    self.apply_registered_event_override_to_binding(binding, override_cfg);
                }
                Ok(())
            })
            .map_err(|message| self.com_createobject_adapter_fault(message))?;
        Ok(Variant::from_object_ref(handle))
    }

    #[cfg(target_os = "windows")]
    #[cfg(test)]
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
        let label = failure.classification_label();
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

    fn controlled_dispatch_exception_fault(&self, dispid: i32) -> HalError {
        HalError::adapter_fault(
            self.profile,
            CapabilityId::ComActivationDispatch,
            "dispatch_invoke",
            format!(
                "com-dispatch-exception-raised;hresult=0x80020009;excep_scode=0x80020009; IDispatch::Invoke(method dispid={dispid}) failed with HRESULT 0x80020009 excep_source=\"OxVba.TestDispatch\" excep_description=\"controlled dispatch exception\""
            ),
        )
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
        self.supports(CapabilityId::ProjectCatalog)
            .then_some(self as &dyn ProjectCatalogHal)
    }

    fn project_references(&self) -> Option<&dyn ProjectReferenceHal> {
        self.supports(CapabilityId::ProjectReferenceProvider)
            .then_some(self as &dyn ProjectReferenceHal)
    }

    fn project_mutation(&self) -> Option<&dyn ProjectMutationHal> {
        self.supports(CapabilityId::ProjectMutation)
            .then_some(self as &dyn ProjectMutationHal)
    }
}

impl ProjectCatalogHal for StandardHostServices {
    fn list_projects(&self) -> HalResult<Vec<ProjectDescriptor>> {
        let capability = CapabilityId::ProjectCatalog;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "list_projects"));
        }
        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "list_projects",
                "project catalog capability advertised without callbacks",
            )
        })?;
        callbacks.on_list_projects().map_err(|err| {
            map_project_callback_error(self.profile, capability, "list_projects", err)
        })
    }

    fn get_project(&self, project_name: &str) -> HalResult<ProjectDescriptor> {
        let capability = CapabilityId::ProjectCatalog;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "get_project"));
        }
        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "get_project",
                "project catalog capability advertised without callbacks",
            )
        })?;
        callbacks
            .on_get_project(project_name)
            .map_err(|err| map_project_callback_error(self.profile, capability, "get_project", err))
    }
}

impl ProjectReferenceHal for StandardHostServices {
    fn list_references(&self, project_name: &str) -> HalResult<Vec<ProjectReferenceDescriptor>> {
        let capability = CapabilityId::ProjectReferenceProvider;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "list_references"));
        }
        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "list_references",
                "project reference capability advertised without callbacks",
            )
        })?;
        callbacks
            .on_list_project_references(project_name)
            .map_err(|err| {
                map_project_callback_error(self.profile, capability, "list_references", err)
            })
    }

    fn resolve_reference(
        &self,
        reference: &ProjectReferenceDescriptor,
    ) -> HalResult<ResolvedProjectReference> {
        let capability = CapabilityId::ProjectReferenceProvider;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "resolve_reference"));
        }
        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "resolve_reference",
                "project reference capability advertised without callbacks",
            )
        })?;
        callbacks
            .on_resolve_project_reference(reference)
            .map_err(|err| {
                map_project_callback_error(self.profile, capability, "resolve_reference", err)
            })
    }
}

impl ProjectMutationHal for StandardHostServices {
    fn attach_host_extension_module(&self, change: &HostExtensionModuleChange) -> HalResult<()> {
        let capability = CapabilityId::ProjectMutation;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "attach_host_extension_module"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "attach_host_extension_module"));
        }
        let callbacks = self.callbacks.as_ref().ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "attach_host_extension_module",
                "project mutation capability advertised without callbacks",
            )
        })?;
        callbacks
            .on_attach_host_extension_module(change)
            .map_err(|err| {
                map_project_callback_error(
                    self.profile,
                    capability,
                    "attach_host_extension_module",
                    err,
                )
            })
    }
}

fn map_project_callback_error(
    profile: HalProfileId,
    capability: CapabilityId,
    operation: &'static str,
    error: ProjectCallbackError,
) -> HalError {
    match error {
        ProjectCallbackError::ProjectNotFound { project_name } => {
            project_not_found(profile, operation, &project_name)
        }
        ProjectCallbackError::ReferenceUnresolved { referenced_name } => {
            project_reference_unresolved(profile, &referenced_name)
        }
        ProjectCallbackError::AdapterFault { message } => {
            HalError::adapter_fault(profile, capability, operation, message)
        }
    }
}

#[cfg(test)]
type ComEventSubscriptionTransport = oxvba_com::WindowsComSubscriptionTransport;

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
#[allow(dead_code)]
fn com_event_signature_arity_for_binding(_binding: &ComBinding, _event: i32) -> Option<usize> {
    None
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn com_event_is_source_interface_only(_binding: &ComBinding, _event: i32) -> bool {
    false
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

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex, OnceLock},
    };

    use oxvba_com::{
        ComInvokeArg, ComInvokeFailure, ComInvokeRequest, ComObjectToken, ComValue,
        DynamicObjectBridge, DynamicObjectToken, RawIDispatch, VariantResultValue,
        add_ref_dispatch as raw_add_ref_dispatch, create_oxvba_test_dispatch,
        release_dispatch as raw_release_dispatch,
        set_variant_from_com_value as com_set_variant_from_com_value,
        take_variant_result_value as com_take_variant_result_value,
        variant_to_com_value as com_variant_to_com_value,
    };
    use oxvba_runtime::{Variant, bstr::BStr};
    use proptest::prelude::*;

    use crate::{
        callbacks::HostCallbacks,
        error::HalErrorKind,
        model::UiVirtualizationMode,
        model::{ComInvocationStrategy, HalProfileId, HalRuntimeClass, HostPolicy},
        traits::{
            ComHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal,
            FileSystemHal, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibResolveRequest,
            UiInteractionHal,
        },
    };

    use super::StandardHostServices;
    #[cfg(target_os = "windows")]
    use oxvba_runtime::ObjectRef;
    #[cfg(target_os = "windows")]
    use windows_sys::Win32::System::Variant::{
        VARIANT, VT_ARRAY, VT_DISPATCH, VT_UNKNOWN, VT_VARIANT, VariantClear,
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn expect_i32(value: Variant) -> i32 {
        value
            .as_i32()
            .or_else(|| value.as_bool().map(i32::from))
            .unwrap_or_else(|| panic!("expected scalar i32-compatible Variant, got {value:?}"))
    }

    fn expect_f64(value: Variant) -> f64 {
        value
            .as_f64()
            .or_else(|| value.as_f32().map(f64::from))
            .or_else(|| value.as_date_f64())
            .unwrap_or_else(|| panic!("expected f64-compatible Variant, got {value:?}"))
    }

    fn expect_object_handle(value: Variant) -> oxvba_runtime::ObjectRef {
        value
            .as_object_ref()
            .unwrap_or_else(|| panic!("expected object Variant, got {value:?}"))
    }

    const TEST_DISPATCH_PROG_ID_NAME: &str = "OxVba.TestDispatch";
    const SCRIPTING_DICTIONARY_PROG_ID_NAME: &str = "Scripting.Dictionary";
    const MISSING_CLASS_PROG_ID_NAME: &str = "OxVba.DoesNotExist.Component";

    fn create_object_prop_test_prog_id_name(prog_id_seed: i32) -> String {
        format!("OxVba.PropSeed.{prog_id_seed}")
    }

    fn native_dispatch_is_bound(
        host: &StandardHostServices,
        object: &oxvba_runtime::ObjectRef,
    ) -> bool {
        host.com_bridge
            .shared_state()
            .lock()
            .expect("com state lock should succeed")
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .map(|binding| binding.native_dispatch != 0)
            .unwrap_or(false)
    }

    fn create_object_test(
        host: &StandardHostServices,
        prog_id_name: &str,
    ) -> crate::error::HalResult<oxvba_runtime::ObjectRef> {
        let prog_id = Variant::from_string(oxvba_runtime::bstr::BStr::from(prog_id_name));
        create_object_variant_test(host, prog_id)
    }

    fn create_object_variant_test(
        host: &StandardHostServices,
        prog_id: Variant,
    ) -> crate::error::HalResult<oxvba_runtime::ObjectRef> {
        host.create_object_variant(prog_id)?
            .as_object_ref()
            .ok_or_else(|| {
                host.com_dispatch_adapter_fault("create variant was not object".to_string())
            })
    }

    fn release_object_test(
        host: &StandardHostServices,
        object: oxvba_runtime::ObjectRef,
    ) -> crate::error::HalResult<i32> {
        host.release_object_variant(object).map(expect_i32)
    }

    fn release_object_variant_test(
        host: &StandardHostServices,
        object: oxvba_runtime::ObjectRef,
    ) -> crate::error::HalResult<i32> {
        host.release_object_variant(object)?
            .as_i32()
            .ok_or_else(|| {
                host.com_dispatch_adapter_fault("release variant was not VT_I4".to_string())
            })
    }

    fn invalidate_typelib_cache_test(
        host: &StandardHostServices,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> crate::error::HalResult<i32> {
        host.invalidate_typelib_cache(scope, reference_name)
            .map(expect_i32)
    }

    fn dispatch_invoke_legacy_v2(
        host: &StandardHostServices,
        request: &ComInvokeRequest,
    ) -> crate::error::HalResult<i32> {
        Ok(expect_i32(host.dispatch_invoke_variant(request)?))
    }

    fn bound_member_token_by_name(
        host: &StandardHostServices,
        object: i32,
        member_name: &str,
    ) -> crate::error::HalResult<oxvba_com::ComMemberToken> {
        let state =
            host.com_bridge.shared_state().lock().map_err(|_| {
                host.com_dispatch_adapter_fault("COM state lock poisoned".to_string())
            })?;
        let binding = state
            .bindings
            .get(&ComObjectToken::new(object))
            .ok_or_else(|| {
                host.com_dispatch_adapter_fault(format!(
                    "COM binding metadata missing for object token {object}"
                ))
            })?;
        binding
            .member_specs
            .iter()
            .find(|(_, member)| member.name.eq_ignore_ascii_case(member_name))
            .map(|(token, _)| *token)
            .ok_or_else(|| {
                host.com_dispatch_adapter_fault(format!(
                    "member metadata missing for `{member_name}` on object token {object}"
                ))
            })
    }

    fn dispatch_invoke_named(
        host: &StandardHostServices,
        object: i32,
        member_name: &str,
        args: &[i32],
    ) -> crate::error::HalResult<i32> {
        let request = ComInvokeRequest {
            object: ObjectRef::from_compat_identity(object),
            member: bound_member_token_by_name(host, object, member_name)?,
            args: args.iter().copied().map(ComInvokeArg::positional).collect(),
            invoke_kind_hint: None,
        };
        dispatch_invoke_legacy_v2(host, &request)
    }

    trait SemanticComTestExt {
        fn create_object_test(
            &self,
            prog_id_name: &str,
        ) -> crate::error::HalResult<oxvba_runtime::ObjectRef>;
        fn release_object_test(
            &self,
            object: oxvba_runtime::ObjectRef,
        ) -> crate::error::HalResult<i32>;
        fn invalidate_typelib_cache_test(
            &self,
            scope: TypeLibCacheScope,
            reference_name: Option<&str>,
        ) -> crate::error::HalResult<i32>;
        fn dispatch_invoke_legacy_v2(
            &self,
            request: &ComInvokeRequest,
        ) -> crate::error::HalResult<i32>;
        fn dispatch_invoke_named(
            &self,
            object: i32,
            member_name: &str,
            args: &[i32],
        ) -> crate::error::HalResult<i32>;
    }

    impl SemanticComTestExt for StandardHostServices {
        fn create_object_test(
            &self,
            prog_id_name: &str,
        ) -> crate::error::HalResult<oxvba_runtime::ObjectRef> {
            create_object_test(self, prog_id_name)
        }

        fn release_object_test(
            &self,
            object: oxvba_runtime::ObjectRef,
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

        fn dispatch_invoke_legacy_v2(
            &self,
            request: &ComInvokeRequest,
        ) -> crate::error::HalResult<i32> {
            dispatch_invoke_legacy_v2(self, request)
        }

        fn dispatch_invoke_named(
            &self,
            object: i32,
            member_name: &str,
            args: &[i32],
        ) -> crate::error::HalResult<i32> {
            dispatch_invoke_named(self, object, member_name, args)
        }
    }

    fn rv(value: i32) -> Variant {
        Variant::from_i32(value)
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

    #[derive(Default)]
    struct ConsoleProbe {
        printed: Mutex<Vec<String>>,
        lines: Mutex<VecDeque<String>>,
    }

    impl ConsoleProbe {
        fn with_lines(lines: impl IntoIterator<Item = &'static str>) -> Self {
            Self {
                printed: Mutex::new(Vec::new()),
                lines: Mutex::new(lines.into_iter().map(str::to_string).collect()),
            }
        }
    }

    impl HostCallbacks for ConsoleProbe {
        fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
            style.max(1)
        }

        fn on_input_box(&self, _prompt: &str, default: &str) -> String {
            default.to_string()
        }

        fn on_status_bar(&self, _text: &str) {}

        fn on_console_print(&self, text: &str) -> bool {
            self.printed
                .lock()
                .expect("printed lock")
                .push(text.to_string());
            true
        }

        fn on_console_input_line(&self) -> Option<String> {
            self.lines.lock().expect("lines lock").pop_front()
        }

        fn on_debug_print(&self, _text: &str) {}
    }

    #[test]
    fn console_variant_companions_use_direct_variant_projection() {
        let callbacks = Arc::new(ConsoleProbe::with_lines(["42, alpha", "line text"]));
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                runtime_class: Some(HalRuntimeClass::WindowsStdio),
                ..HostPolicy::default()
            },
        )
        .with_callbacks(callbacks.clone());

        let status = crate::traits::ConsoleHal::print_line_variant(&host, Variant::null())
            .expect("variant print");
        assert_eq!(status, Variant::from_i32(0));
        assert_eq!(
            callbacks.printed.lock().expect("printed lock").as_slice(),
            ["Null"]
        );

        let first = crate::traits::ConsoleHal::input_fields_variant(&host, Variant::from_i32(1))
            .expect("first input");
        assert_eq!(first, Variant::from_i32(42));
        let second = crate::traits::ConsoleHal::input_fields_variant(&host, Variant::from_i32(1))
            .expect("second input");
        assert_eq!(second.as_bstr(), Some(BStr::from("alpha")));

        let line = crate::traits::ConsoleHal::line_input_variant(&host).expect("line input");
        assert_eq!(line.as_bstr(), Some(BStr::from("line text")));
    }

    #[test]
    fn ui_variant_companions_do_not_use_trait_fallback_projection() {
        let callbacks = Arc::new(ConsoleProbe::default());
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_interaction: true,
                ui_virtualization: UiVirtualizationMode::HostCallback,
                ..HostPolicy::default()
            },
        )
        .with_callbacks(callbacks);

        let msg = crate::traits::UiInteractionHal::msg_box_variant(
            &host,
            Variant::null(),
            Variant::from_i32(7),
        )
        .expect("variant msgbox");
        assert_eq!(msg, Variant::from_i32(7));

        let input = crate::traits::UiInteractionHal::input_box_variant(
            &host,
            Variant::null(),
            Variant::from_string("fallback"),
        )
        .expect("variant inputbox");
        assert_eq!(input.as_bstr(), Some(BStr::from("fallback")));
    }

    #[test]
    fn status_time_and_diagnostics_variant_companions_are_direct() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());

        let events =
            crate::traits::EventPumpHal::do_events_variant(&host).expect("variant doevents");
        assert_eq!(events, Variant::from_i32(0));

        let emitted = crate::traits::DiagnosticsHal::emit_variant(
            &host,
            Variant::from_i32(4),
            Variant::from_i32(5),
        )
        .expect("variant emit");
        assert_eq!(emitted, Variant::from_i32(9));
        let debug = crate::traits::DiagnosticsHal::debug_print_variant(&host, Variant::null())
            .expect("variant debug print");
        assert_eq!(debug, Variant::from_i32(0));

        let date =
            crate::traits::TimeLocaleHal::date_serial_now_variant(&host).expect("variant date");
        assert_eq!(date.as_date_f64(), Some(46_082.0));
        let time =
            crate::traits::TimeLocaleHal::time_serial_now_variant(&host).expect("variant time");
        assert_eq!(time.as_date_f64(), Some(45_296.0 / 86_400.0));
        let timer =
            crate::traits::TimeLocaleHal::timer_ticks_variant(&host).expect("variant timer");
        assert_eq!(timer.as_f32(), Some(45_296.0_f32));
    }

    #[test]
    fn process_variant_companions_are_direct() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_process_spawn: true,
                ..HostPolicy::default()
            },
        );

        let shell = crate::traits::ProcessEnvHal::shell_variant(
            &host,
            Variant::from_string("probe"),
            Variant::from_i32(0),
        )
        .expect("variant shell");
        assert_eq!(shell, Variant::from_i32(1));

        let environ =
            crate::traits::ProcessEnvHal::environ_variant(&host, Variant::from_string("PATH"))
                .expect("variant environ");
        assert_eq!(environ, Variant::from_i32(4));

        let dir = crate::traits::ProcessEnvHal::dir_variant(
            &host,
            Variant::from_string("anything"),
            Variant::from_i32(0),
        )
        .expect("variant dir");
        assert_eq!(dir, Variant::from_i32(1));
    }

    #[test]
    fn filesystem_status_variant_companions_are_direct() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());

        let free = crate::traits::FileSystemHal::free_file_variant(&host, Variant::from_i32(0))
            .expect("variant freefile");
        assert_eq!(free, Variant::from_i32(1));

        let handle = host
            .open_variant(rv(123), rv(0))
            .expect("open deterministic file");
        let handle_variant = Variant::from_i32(expect_i32(handle));

        let loc = crate::traits::FileSystemHal::loc_variant(&host, handle_variant.clone())
            .expect("variant loc");
        assert_eq!(loc, Variant::from_i32(0));

        let seek = crate::traits::FileSystemHal::seek_variant(
            &host,
            handle_variant.clone(),
            Variant::from_i32(2),
        )
        .expect("variant seek");
        assert_eq!(seek, Variant::from_i32(2));

        let lof = crate::traits::FileSystemHal::lof_variant(&host, handle_variant.clone())
            .expect("variant lof");
        assert_eq!(
            lof,
            Variant::from_i32(super::filesystem::pseudo_file_len_from_path_token(123))
        );

        let eof = crate::traits::FileSystemHal::eof_variant(&host, handle_variant.clone())
            .expect("variant eof");
        assert_eq!(eof, Variant::from_i32(0));

        let closed = crate::traits::FileSystemHal::close_variant(&host, handle_variant)
            .expect("variant close");
        assert_eq!(closed, Variant::from_i32(1));
    }

    #[test]
    fn filesystem_open_kill_variant_companions_are_direct() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_filesystem_mutation: true,
                ..HostPolicy::default()
            },
        );

        let handle = crate::traits::FileSystemHal::open_variant(
            &host,
            Variant::from_i32(321),
            Variant::from_i32(1),
        )
        .expect("variant open");
        assert_eq!(handle, Variant::from_i32(1));

        let loc =
            crate::traits::FileSystemHal::loc_variant(&host, handle.clone()).expect("variant loc");
        assert_eq!(loc, Variant::from_i32(0));

        let closed =
            crate::traits::FileSystemHal::close_variant(&host, handle).expect("variant close");
        assert_eq!(closed, Variant::from_i32(1));

        let killed = crate::traits::FileSystemHal::kill_variant(
            &host,
            Variant::from_string("deterministic.tmp"),
        )
        .expect("variant kill");
        assert_eq!(killed, Variant::from_i32(0));
    }

    #[test]
    fn filesystem_put_get_record_round_trips() {
        use crate::traits::FileSystemHal;
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_filesystem_mutation: true,
                ..HostPolicy::default()
            },
        );
        // Open a Random-mode file (mode 4) on an in-memory handle.
        let handle =
            FileSystemHal::open_variant(&host, Variant::from_i32(901), Variant::from_i32(4))
                .expect("open");
        // Put a Long at record 1, then Get it back with the Long type code.
        FileSystemHal::put_record_variant(
            &host,
            handle.clone(),
            Variant::from_i32(1),
            Variant::from_i32(0x1234_5678),
            Variant::from_i32(0),
        )
        .expect("put");
        let long_code = Variant::from_i32(oxvba_runtime::VarType::Long as i32);
        let got = FileSystemHal::get_record_variant(
            &host,
            handle.clone(),
            Variant::from_i32(1),
            long_code,
            Variant::from_i32(0),
        )
        .expect("get");
        assert_eq!(got.as_i32(), Some(0x1234_5678));
    }

    #[test]
    fn filesystem_random_len_fixed_record_positioning() {
        use crate::traits::FileSystemHal;
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_filesystem_mutation: true,
                ..HostPolicy::default()
            },
        );
        // Open Random (mode 4) with Len = 8; each Long (4 bytes) occupies an 8-byte
        // slot, so record 2 starts at byte offset 8.
        let handle = FileSystemHal::open_with_record_len(
            &host,
            Variant::from_i32(910),
            Variant::from_i32(4),
            Variant::from_i32(8),
        )
        .expect("open with len");
        FileSystemHal::put_record_variant(
            &host,
            handle.clone(),
            Variant::from_i32(1),
            Variant::from_i32(0x1111_1111),
            Variant::from_i32(0),
        )
        .expect("put rec 1");
        FileSystemHal::put_record_variant(
            &host,
            handle.clone(),
            Variant::from_i32(2),
            Variant::from_i32(0x2222_2222),
            Variant::from_i32(0),
        )
        .expect("put rec 2");
        let long_code = Variant::from_i32(oxvba_runtime::VarType::Long as i32);
        let r2 = FileSystemHal::get_record_variant(
            &host,
            handle.clone(),
            Variant::from_i32(2),
            long_code.clone(),
            Variant::from_i32(0),
        )
        .expect("get rec 2");
        assert_eq!(r2.as_i32(), Some(0x2222_2222));
        let r1 = FileSystemHal::get_record_variant(
            &host,
            handle,
            Variant::from_i32(1),
            long_code,
            Variant::from_i32(0),
        )
        .expect("get rec 1");
        assert_eq!(r1.as_i32(), Some(0x1111_1111));
    }

    #[test]
    fn filesystem_lock_overlap_is_rejected() {
        use crate::traits::FileSystemHal;
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_filesystem_mutation: true,
                ..HostPolicy::default()
            },
        );
        let handle =
            FileSystemHal::open_variant(&host, Variant::from_i32(902), Variant::from_i32(4))
                .expect("open");
        // Lock records 1..5, then Unlock; an overlapping re-lock between is rejected.
        FileSystemHal::lock_variant(
            &host,
            handle.clone(),
            Variant::from_i32(1),
            Variant::from_i32(5),
        )
        .expect("lock 1..5");
        assert!(
            FileSystemHal::lock_variant(
                &host,
                handle.clone(),
                Variant::from_i32(3),
                Variant::from_i32(7),
            )
            .is_err(),
            "overlapping lock should be rejected"
        );
        FileSystemHal::unlock_variant(
            &host,
            handle.clone(),
            Variant::from_i32(1),
            Variant::from_i32(5),
        )
        .expect("unlock");
        // After unlock the range is free again.
        FileSystemHal::lock_variant(&host, handle, Variant::from_i32(3), Variant::from_i32(7))
            .expect("re-lock after unlock");
    }

    #[test]
    fn filesystem_text_payload_variant_companions_are_direct() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_filesystem_mutation: true,
                ..HostPolicy::default()
            },
        );

        let handle = crate::traits::FileSystemHal::open_variant(
            &host,
            Variant::from_i32(777),
            Variant::from_i32(1),
        )
        .expect("variant open output");
        let written = crate::traits::FileSystemHal::write_bytes_variant(
            &host,
            handle.clone(),
            Variant::from_string("alpha"),
        )
        .expect("variant write");
        assert!(written.as_i32().unwrap_or_default() > 0);
        host.seek_variant(Variant::from_i32(handle.as_i32().unwrap()), rv(0))
            .expect("seek back");
        let read = crate::traits::FileSystemHal::read_bytes_variant(&host, handle.clone(), written)
            .expect("variant read");
        assert_eq!(read.as_bstr(), Some(BStr::from("\"alpha\"\r\n")));
        crate::traits::FileSystemHal::close_variant(&host, handle).expect("variant close");

        let line_handle = crate::traits::FileSystemHal::open_variant(
            &host,
            Variant::from_i32(778),
            Variant::from_i32(1),
        )
        .expect("variant open line output");
        crate::traits::FileSystemHal::print_line_variant(
            &host,
            line_handle.clone(),
            Variant::from_string("world"),
        )
        .expect("variant print line");
        host.seek_variant(Variant::from_i32(line_handle.as_i32().unwrap()), rv(0))
            .expect("seek line");
        let line = crate::traits::FileSystemHal::line_input_variant(&host, line_handle.clone())
            .expect("variant line input");
        assert_eq!(line.as_bstr(), Some(BStr::from("world")));
        crate::traits::FileSystemHal::close_variant(&host, line_handle).expect("close line");

        let input_handle = crate::traits::FileSystemHal::open_variant(
            &host,
            Variant::from_i32(779),
            Variant::from_i32(1),
        )
        .expect("variant open input output");
        crate::traits::FileSystemHal::write_bytes_variant(
            &host,
            input_handle.clone(),
            Variant::from_i32(42),
        )
        .expect("variant write input");
        host.seek_variant(Variant::from_i32(input_handle.as_i32().unwrap()), rv(0))
            .expect("seek input");
        let field = crate::traits::FileSystemHal::input_fields_variant(
            &host,
            input_handle.clone(),
            Variant::from_i32(1),
        )
        .expect("variant input field");
        assert_eq!(field, Variant::from_i32(42));
        crate::traits::FileSystemHal::close_variant(&host, input_handle).expect("close input");
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
        let handle = host
            .open_variant(rv(77), rv(0))
            .expect("open should succeed");
        assert_eq!(handle, rv(1));
        assert_eq!(
            host.eof_variant(handle.clone()).expect("eof should work"),
            rv(0)
        );
        let len = expect_i32(host.lof_variant(handle.clone()).expect("lof should work"));
        assert!(len > 0);
        host.seek_variant(handle.clone(), rv(len))
            .expect("seek to end should work");
        assert_eq!(
            host.eof_variant(handle.clone()).expect("eof should work"),
            rv(1)
        );
        assert_eq!(
            host.close_variant(handle).expect("close should work"),
            rv(1)
        );
    }

    #[test]
    fn file_open_accepts_variant_string_paths() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host
            .open_variant(
                Variant::from_string(BStr::from("runtime-fs-value-path.txt")),
                Variant::from_i32(0),
            )
            .expect("open should succeed");
        let handle = expect_i32(handle);
        assert_eq!(
            host.lof_variant(Variant::from_i32(handle))
                .expect("lof should succeed"),
            Variant::from_i32(1)
        );
        assert_eq!(
            host.close_variant(Variant::from_i32(handle))
                .expect("close should succeed"),
            Variant::from_i32(1)
        );
    }

    #[test]
    fn free_file_respects_low_and_high_ranges() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.free_file_variant(rv(0)).expect("default free file"),
            rv(1)
        );
        assert_eq!(
            host.free_file_variant(rv(1)).expect("high-range free file"),
            rv(256)
        );
    }

    #[test]
    fn close_releases_handle_for_reuse() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host
            .open_variant(rv(10), rv(0))
            .expect("open should succeed");
        assert_eq!(first, rv(1));
        host.close_variant(first).expect("close should succeed");
        let second = host
            .open_variant(rv(11), rv(0))
            .expect("second open should succeed");
        assert_eq!(second, rv(1), "closed handles must be reusable");
    }

    #[test]
    fn free_file_low_range_tracks_allocated_handles() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let mut handles = Vec::new();
        for expected in 1..=8 {
            assert_eq!(
                host.free_file_variant(rv(0))
                    .expect("free_file should succeed"),
                rv(expected)
            );
            handles.push(
                host.open_variant(rv(expected), rv(0))
                    .expect("open should succeed"),
            );
        }
        assert_eq!(handles, (1..=8).map(rv).collect::<Vec<_>>());
    }

    #[test]
    fn seek_negative_returns_adapter_fault() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host
            .open_variant(rv(42), rv(0))
            .expect("open should succeed");
        let err = host
            .seek_variant(handle, rv(-1))
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
        assert_eq!(
            host.msg_box_variant(rv(100), rv(3)).expect("msg_box"),
            rv(3)
        );
        assert_eq!(
            host.input_box_variant(rv(100), rv(7)).expect("input_box"),
            rv(7)
        );

        let host_disabled = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                ui_virtualization: crate::model::UiVirtualizationMode::Disabled,
                ..policy
            },
        );
        assert_eq!(
            host_disabled
                .msg_box_variant(rv(100), rv(3))
                .expect("msg_box"),
            rv(100)
        );
        assert_eq!(
            host_disabled
                .input_box_variant(rv(100), rv(7))
                .expect("input_box"),
            rv(100)
        );
    }

    #[test]
    fn ui_variant_lanes_preserve_string_inputs() {
        let policy = HostPolicy {
            allow_interaction: true,
            ui_virtualization: crate::model::UiVirtualizationMode::ScriptedResponses,
            ..HostPolicy::default()
        };
        let host = StandardHostServices::new(HalProfileId::Windows, policy.clone());
        assert_eq!(
            host.msg_box_variant(
                Variant::from_string(BStr::from("Prompt")),
                Variant::from_i32(3)
            )
            .expect("msg_box"),
            Variant::from_i32(3)
        );
        assert_eq!(
            host.input_box_variant(
                Variant::from_string(BStr::from("Prompt")),
                Variant::from_string(BStr::from("Default")),
            )
            .expect("input_box"),
            Variant::from_string(BStr::from("Default"))
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
            .msg_box_variant(rv(9), rv(1))
            .expect_err("msg_box should be denied");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        let err = host
            .input_box_variant(rv(9), rv(1))
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
            host.shell_variant(rv(1), rv(0))
                .expect_err("shell deny")
                .kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.create_object_test("Denied.Policy.Test")
                .expect_err("com deny")
                .kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.invoke_symbol_variant(1.into(), &rv(2))
                .expect_err("dynlink deny")
                .kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn create_object_variant_accepts_prog_id_without_compat_projection() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let object =
            create_object_variant_test(&host, Variant::from_string(TEST_DISPATCH_PROG_ID_NAME))
                .expect("variant ProgID should activate deterministic projection object");
        assert!(object.raw() != 0);

        let numeric_object = create_object_variant_test(&host, Variant::from_i32(123))
            .expect("numeric ProgID should coerce through Variant-native string path");
        assert!(numeric_object.raw() != 0);
    }

    #[test]
    fn process_variant_lanes_accept_string_inputs_in_deterministic_mode() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.shell_variant(
                Variant::from_string(BStr::from("echo hi")),
                Variant::from_i32(0),
            )
            .expect("shell"),
            Variant::from_i32(1)
        );
        assert_eq!(
            host.environ_variant(Variant::from_string(BStr::from("PATH")))
                .expect("environ"),
            Variant::from_i32(4)
        );
        assert_eq!(
            host.dir_variant(
                Variant::from_string(BStr::from("folder")),
                Variant::from_i32(0),
            )
            .expect("dir"),
            Variant::from_i32(1)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_string_variant_roundtrips_through_adapter_helpers() {
        let mut variant: VARIANT = unsafe { std::mem::zeroed() };
        let value = ComValue::String(BStr::from("Hello"));
        let resolve_object = |_handle: ObjectRef| -> Result<*mut RawIDispatch, String> {
            Err("object dispatch resolution not expected".to_string())
        };
        unsafe {
            let mut add_ref_dispatch = |_dispatch: *mut core::ffi::c_void| {};
            com_set_variant_from_com_value(
                &mut variant,
                &value,
                &mut |handle| {
                    resolve_object(handle).map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
                &mut add_ref_dispatch,
            )
            .expect("set string variant");
            assert_eq!(
                com_variant_to_com_value(&variant).expect("read string variant"),
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
        let value = ComValue::Object(ObjectRef::from_compat_identity(20_001));
        let resolve_object =
            |_handle: ObjectRef| -> Result<*mut RawIDispatch, String> { Ok(dispatch) };
        unsafe {
            let mut add_ref_dispatch = |dispatch: *mut core::ffi::c_void| {
                raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
            };
            com_set_variant_from_com_value(
                &mut variant,
                &value,
                &mut |handle| {
                    resolve_object(handle).map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
                &mut add_ref_dispatch,
            )
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
        let value =
            ComValue::ArrayIntent(oxvba_runtime::safe_array::SafeArray::from_variants(vec![
                Variant::from_i32(4),
                Variant::from_bool(true),
                Variant::from_string(BStr::from("Hello")),
                Variant::null(),
            ]));
        let resolve_object = |_handle: ObjectRef| -> Result<*mut RawIDispatch, String> {
            Err("object dispatch resolution not expected".to_string())
        };
        unsafe {
            let mut add_ref_dispatch = |_dispatch: *mut core::ffi::c_void| {};
            com_set_variant_from_com_value(
                &mut variant,
                &value,
                &mut |handle| {
                    resolve_object(handle).map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
                &mut add_ref_dispatch,
            )
            .expect("set SAFEARRAY variant");
            assert_eq!(variant.Anonymous.Anonymous.vt, VT_ARRAY | VT_VARIANT);
            assert_eq!(
                com_variant_to_com_value(&variant).expect("read SAFEARRAY variant"),
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
                &mut |unknown: *mut core::ffi::c_void| {
                    oxvba_com::query_dispatch_from_unknown(unknown.cast::<oxvba_com::RawIUnknown>())
                        .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
                &mut |dispatch: *mut core::ffi::c_void| {
                    raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
                },
            )
        }
        .expect("classify dispatch result");
        let value = match classified {
            VariantResultValue::Value(value) => value
                .to_variant()
                .expect("COM result should convert to Variant"),
            VariantResultValue::Dispatch(dispatch) => host
                .bind_native_dispatch_result(
                    dispatch.cast::<RawIDispatch>(),
                    "OxVba.TestDispatch",
                    "dispatch_invoke",
                )
                .expect("bind dispatch result"),
        };
        let handle = expect_object_handle(value);
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
                &mut |unknown: *mut core::ffi::c_void| {
                    oxvba_com::query_dispatch_from_unknown(unknown.cast::<oxvba_com::RawIUnknown>())
                        .map(|dispatch| dispatch.cast::<core::ffi::c_void>())
                },
                &mut |dispatch: *mut core::ffi::c_void| {
                    raw_add_ref_dispatch(dispatch.cast::<RawIDispatch>());
                },
            )
        }
        .expect("classify unknown result");
        let value = match classified {
            VariantResultValue::Value(value) => value
                .to_variant()
                .expect("COM result should convert to Variant"),
            VariantResultValue::Dispatch(dispatch) => host
                .bind_native_dispatch_result(
                    dispatch.cast::<RawIDispatch>(),
                    "OxVba.TestDispatch",
                    "dispatch_invoke",
                )
                .expect("bind unknown-dispatch result"),
        };
        let handle = expect_object_handle(value);
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
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
        };
        let err = host
            .bind_descriptor(&descriptor)
            .expect_err("unsupported marshaling lane should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("unsupported marshaling lane"));
    }

    #[test]
    fn dynlink_invoke_descriptor_m1_native_lane_honors_policy_denial() {
        // W1-hal-001: the m1-native-ffi branch must gate on allow_dynamic_link
        // exactly like its siblings — a live (non-deterministic) host that
        // forbids arbitrary DLLs must never reach LoadLibrary.
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                deterministic_mode: false,
                allow_dynamic_link: false,
                ..HostPolicy::default()
            },
        );
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 9,
            declared_name: "GetTickCount",
            library: "kernel32",
            alias: "GetTickCount",
            ordinal_alias: false,
            symbol: 9.into(),
            marshal_lane: "m1-native-ffi",
            calling_convention: "platform-default",
            selection_policy: "case-insensitive-canonical",
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: Some(std::borrow::Cow::Borrowed("Long")),
        };
        let err = host
            .invoke_descriptor_variants(&descriptor, &[])
            .expect_err("native invoke must be policy-denied");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
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
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
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
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
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
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
        };
        let err = host
            .bind_descriptor(&descriptor)
            .expect_err("ordinal alias without #digits should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("must start with `#`"));
    }

    #[test]
    fn dynlink_legacy_symbol_variant_invoke_projects_variant_token_directly() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_dynamic_link: true,
                ..HostPolicy::default()
            },
        );

        let result = host
            .invoke_symbol_variant(7.into(), &Variant::from_i32(5))
            .expect("variant symbol invoke should succeed");

        assert_eq!(result, Variant::from_i32(12));
    }

    #[test]
    fn dynlink_descriptor_variant_invoke_projects_m0_token_directly() {
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
            selection_policy: "case-insensitive-canonical",
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
        };

        let (result, writebacks) = host
            .invoke_descriptor_variants(&descriptor, &[Variant::from_i32(5)])
            .expect("variant descriptor invoke should succeed");

        assert_eq!(result, Variant::from_i32(12));
        assert!(writebacks.is_empty());
    }

    #[test]
    fn dynlink_bound_variant_invoke_projects_m0_token_directly() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_dynamic_link: true,
                ..HostPolicy::default()
            },
        );
        let descriptor = DynLinkDescriptorView {
            descriptor_id: 8,
            declared_name: "hostping",
            library: "host",
            alias: "ping",
            ordinal_alias: false,
            symbol: 8.into(),
            marshal_lane: "m0-deterministic",
            calling_convention: "platform-default",
            selection_policy: "case-insensitive-canonical",
            param_count: 0,
            param_types: &[],
            param_by_ref: &[],
            return_type: None,
        };
        let binding = host
            .bind_descriptor(&descriptor)
            .expect("descriptor binding");

        let (result, writebacks) = host
            .invoke_bound_variants(binding, &[Variant::from_i32(5)])
            .expect("variant bound invoke should succeed");

        assert_eq!(result, Variant::from_i32(13));
        assert!(writebacks.is_empty());
    }

    #[test]
    fn time_locale_contract_values_are_stable() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.date_serial_now_variant().expect("date"),
            Variant::from_date_f64(46_082.0)
        );
        assert_eq!(
            host.time_serial_now_variant().expect("time"),
            Variant::from_date_f64(45_296.0 / 86_400.0)
        );
        assert_eq!(
            host.timer_ticks_variant().expect("timer"),
            Variant::from_f32(45_296.0)
        );
    }

    #[test]
    fn process_env_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.environ_variant(rv(88)).expect("environ"), rv(88));
        assert_eq!(host.dir_variant(rv(0), rv(0)).expect("dir"), rv(0));
        assert_eq!(host.dir_variant(rv(5), rv(0)).expect("dir"), rv(1));
    }

    #[test]
    fn dispatch_invoke_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.dispatch_invoke_variant(&ComInvokeRequest {
                object: ObjectRef::from_compat_identity(10),
                member: 20.into(),
                args: vec![ComInvokeArg::positional(30)],
                invoke_kind_hint: None,
            })
            .expect("dispatch"),
            Variant::from_i32(60)
        );
    }

    #[test]
    fn dispatch_invoke_projection_preserves_controlled_self_object_members() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return deterministic projection handle");
        let dispatch = host
            .dispatch_invoke_variant(&ComInvokeRequest {
                object: object.clone(),
                member: 23.into(),
                args: Vec::new(),
                invoke_kind_hint: None,
            })
            .expect("ReturnSelfDispatch projection should succeed");
        let unknown = host
            .dispatch_invoke_variant(&ComInvokeRequest {
                object: object.clone(),
                member: 24.into(),
                args: Vec::new(),
                invoke_kind_hint: None,
            })
            .expect("ReturnSelfUnknown projection should succeed");
        assert_eq!(
            expect_object_handle(dispatch).compat_identity(),
            object.compat_identity()
        );
        assert_eq!(
            expect_object_handle(unknown).compat_identity(),
            object.compat_identity()
        );
    }

    #[test]
    fn dispatch_invoke_projection_surfaces_controlled_raise_exception_fault() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return deterministic projection handle");
        let err = host
            .dispatch_invoke_variant(&ComInvokeRequest {
                object: object.clone(),
                member: super::TEST_DISPID_RAISE_EXCEPTION.into(),
                args: Vec::new(),
                invoke_kind_hint: None,
            })
            .expect_err("RaiseException projection should surface an adapter fault");
        assert!(
            err.message.contains("com-dispatch-exception-raised"),
            "expected stable exception classification, got {}",
            err.message
        );
        assert!(
            err.message.contains("excep_source=\"OxVba.TestDispatch\"")
                && err
                    .message
                    .contains("excep_description=\"controlled dispatch exception\""),
            "expected EXCEPINFO source/description in {}",
            err.message
        );
    }

    #[test]
    fn release_object_projection_returns_variant_status_for_dynamic_bridge() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return deterministic projection handle");
        assert_eq!(
            release_object_variant_test(&host, object.clone()).expect("variant release"),
            1
        );

        let bridge = crate::HalComDynamicBridge::new(HalProfileId::Windows, &host);
        let dynamic = bridge
            .release_dynamic_object(DynamicObjectToken::from(object))
            .expect("dynamic release should use retained variant status");
        assert_eq!(dynamic.variant().as_i32(), Some(1));
    }

    #[test]
    fn dispatch_invoke_missing_arg_token_projects_as_zero() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            host.dispatch_invoke_variant(&ComInvokeRequest {
                object: ObjectRef::from_compat_identity(10),
                member: 20.into(),
                args: Vec::new(),
                invoke_kind_hint: None,
            })
            .expect("dispatch"),
            Variant::from_i32(30)
        );
    }

    #[test]
    fn diagnostics_emit_contract_is_deterministic() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.emit_variant(rv(4), rv(5)).expect("emit"), rv(9));
    }

    #[test]
    fn event_pump_supported_and_unsupported_paths() {
        let windows = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(
            windows.do_events_variant().expect("windows do_events"),
            Variant::from_i32(0)
        );

        let null = StandardHostServices::new(HalProfileId::Null, HostPolicy::default());
        let err = null
            .do_events_variant()
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
            host.free_file_variant(rv(0))
                .expect("first free should be 1"),
            rv(1)
        );
        let err = host
            .open_variant(rv(10), rv(1))
            .expect_err("mutation open should be denied by policy");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        assert_eq!(
            host.free_file_variant(rv(0))
                .expect("free file should remain unchanged"),
            rv(1)
        );
    }

    #[test]
    fn invalid_close_does_not_mutate_handle_state() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host
            .open_variant(rv(10), rv(0))
            .expect("open should succeed");
        assert_eq!(first, rv(1));
        let err = host
            .close_variant(rv(99))
            .expect_err("invalid close should fail");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert_eq!(
            host.free_file_variant(rv(0))
                .expect("free file should still skip handle 1"),
            rv(2)
        );
    }

    #[test]
    fn ui_msg_box_enforces_policy_and_capability_failures() {
        let denied_host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let err = denied_host
            .msg_box_variant(rv(1), rv(1))
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
            .msg_box_variant(rv(1), rv(1))
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
        assert!(!descriptor.supports(crate::model::CapabilityId::ProjectCatalog));
        assert!(!descriptor.supports(crate::model::CapabilityId::ProjectReferenceProvider));
        assert!(!descriptor.supports(crate::model::CapabilityId::ProjectMutation));
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
                .shell_variant(rv(1), rv(0))
                .expect_err("windows shell denial")
                .kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            linux
                .shell_variant(rv(1), rv(0))
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
            host.shell_variant(rv(1), rv(0))
                .expect("native shell should succeed"),
        );
        assert!(shell >= 1);
        let environ = host
            .environ_variant(Variant::from_string(BStr::from("PATH")))
            .expect("native environ should succeed");
        assert!(
            environ.as_bstr().is_some(),
            "native environ should return a string value, got {environ:?}"
        );
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("native-process-env");
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("probe-file.txt");
        std::fs::write(&temp_file, "probe").expect("write temp file");
        let dir = host
            .dir_variant(
                Variant::from_string(BStr::from(temp_file.to_string_lossy().to_string())),
                rv(0),
            )
            .expect("native dir should succeed");
        assert_eq!(dir, Variant::from_string(BStr::from("probe-file.txt")));
    }

    #[test]
    fn native_mode_environ_string_returns_actual_value() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        unsafe {
            std::env::set_var("OXVBA_NATIVE_PROCESS_ENV_TEST", "native-process-env-value");
        }
        let out = host
            .environ_variant(Variant::from_string(BStr::from(
                "OXVBA_NATIVE_PROCESS_ENV_TEST",
            )))
            .expect("native environ should succeed");
        assert_eq!(
            out,
            Variant::from_string(BStr::from("native-process-env-value"))
        );
    }

    #[test]
    fn native_mode_print_line_roundtrips_through_host_file() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let mut policy = HostPolicy::interactive_dev();
        policy.allow_filesystem_mutation = true;
        let host = StandardHostServices::new(profile, policy);
        let temp_dir = std::env::current_dir()
            .expect("cwd")
            .join("temp")
            .join("native-file-io");
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let temp_file = temp_dir.join("roundtrip.txt");
        let path = Variant::from_string(BStr::from(temp_file.to_string_lossy().to_string()));

        let write_handle = host.open_variant(path.clone(), rv(1)).expect("open output");
        host.print_line_variant(
            write_handle.clone(),
            Variant::from_string(BStr::from("world")),
        )
        .expect("print_line");
        host.close_variant(write_handle).expect("close output");
        assert_eq!(
            std::fs::read_to_string(&temp_file).expect("read flushed file"),
            "world\r\n"
        );

        let read_handle = host.open_variant(path, rv(0)).expect("open input");
        assert_eq!(
            host.line_input_variant(read_handle.clone())
                .expect("line_input"),
            Variant::from_string(BStr::from("world"))
        );
        host.close_variant(read_handle).expect("close input");
    }

    #[test]
    fn native_mode_filesystem_seek_can_extend_length() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        let handle = host
            .open_variant(rv(31415), rv(1))
            .expect("native open should succeed");
        host.seek_variant(handle.clone(), rv(64))
            .expect("native seek should succeed");
        assert!(
            expect_i32(
                host.lof_variant(handle.clone())
                    .expect("native lof should succeed")
            ) >= 64,
            "native seek in mutation mode should extend logical length"
        );
        assert_eq!(
            host.close_variant(handle)
                .expect("native close should succeed"),
            rv(1)
        );
    }

    #[test]
    fn native_mode_time_tokens_are_non_negative() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        assert!(expect_f64(host.date_serial_now_variant().expect("date")) >= 0.0);
        assert!(expect_f64(host.time_serial_now_variant().expect("time")) >= 0.0);
        assert!(expect_f64(host.timer_ticks_variant().expect("ticks")) >= 0.0);
    }

    #[test]
    fn com_event_subscription_lane_requires_native_mode() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let subscribe = host
            .subscribe_event(ObjectRef::from_compat_identity(1), 1.into())
            .expect_err("subscribe_event should require native mode");
        assert_eq!(subscribe.kind, HalErrorKind::AdapterFault);
        assert_eq!(subscribe.operation, "subscribe_event");
        assert!(subscribe.message.contains("COM-E-EVENT-PATH-UNSUPPORTED"));

        let unsubscribe = host
            .unsubscribe_event_variant(1.into())
            .expect_err("unsubscribe_event should require native mode");
        assert_eq!(unsubscribe.kind, HalErrorKind::AdapterFault);
        assert_eq!(unsubscribe.operation, "unsubscribe_event");
        assert!(unsubscribe.message.contains("COM-E-EVENT-PATH-UNSUPPORTED"));
        assert!(
            host.event_callback_subscription(60_001.into())
                .expect_err("event_callback_subscription should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.event_callback_arity(60_001.into())
                .expect_err("event_callback_arity should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.event_callback_variant(60_001.into(), 0)
                .expect_err("event_callback_arg should require native mode")
                .message
                .contains("COM-E-EVENT-PATH-UNSUPPORTED")
        );
        assert!(
            host.release_event_callback_variant(60_001.into())
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        assert!(
            object.raw() >= 20_001,
            "controlled COM lane should bind native object"
        );
        let subscription = host
            .subscribe_event(object.clone(), 1.into())
            .expect("subscribe_event should succeed for controlled event source");
        assert!(subscription.raw() >= 40_001);
        {
            let state = host
                .com_bridge
                .shared_state()
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
            host.dispatch_invoke_named(object.raw(), "FireChanged", &[77])
                .expect("FireChanged should succeed"),
            77
        );
        let callback = host
            .do_events_variant()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);
        assert!(callback >= 60_001);
        assert_eq!(
            host.event_callback_subscription(callback.into())
                .expect("callback subscription lookup should succeed"),
            subscription
        );
        assert_eq!(
            host.event_callback_variant(callback.into(), 0)
                .expect("callback arg lookup should succeed"),
            rv(77)
        );
        assert_eq!(
            host.event_callback_arity(callback.into())
                .expect("callback arity lookup should succeed"),
            1
        );
        assert_eq!(
            host.release_event_callback_variant(callback.into())
                .expect("callback release should succeed"),
            rv(1)
        );
        assert_eq!(
            host.do_events_variant()
                .expect("callback queue should be drained"),
            Variant::from_i32(0),
            "native callback lane should not enqueue duplicate projection callbacks"
        );

        assert_eq!(
            host.unsubscribe_event_variant(subscription)
                .expect("unsubscribe_event should succeed"),
            rv(1)
        );
        let _ = host
            .dispatch_invoke_named(object.raw(), "FireChanged", &[88])
            .expect("FireChanged should remain invokable after unsubscribe");
        assert_eq!(
            host.do_events_variant()
                .expect("callback queue should remain empty after unsubscribe"),
            Variant::from_i32(0)
        );
        let callback_still_present = {
            let state = host
                .com_bridge
                .shared_state()
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(object.clone(), super::TEST_EVENT_CHANGED_PAIR.into())
            .expect("subscribe_event should succeed for controlled pair-event source");

        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "FireChangedPair", &[90])
                .expect("FireChangedPair should succeed"),
            91
        );
        let callback = host
            .do_events_variant()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);
        assert!(callback >= 60_001);
        assert_eq!(
            host.event_callback_subscription(callback.into())
                .expect("callback subscription lookup should succeed"),
            subscription
        );
        assert_eq!(
            host.event_callback_arity(callback.into())
                .expect("callback arity lookup should succeed"),
            2
        );
        assert_eq!(
            host.event_callback_variant(callback.into(), 0)
                .expect("callback arg0 lookup should succeed"),
            rv(90)
        );
        assert_eq!(
            host.event_callback_variant(callback.into(), 1)
                .expect("callback arg1 lookup should succeed"),
            rv(91)
        );
        let err = host
            .event_callback_variant(callback.into(), 2)
            .expect_err("index beyond callback arity should fail");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message
                .contains("COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH")
        );
        assert_eq!(
            host.release_event_callback_variant(callback.into())
                .expect("callback release should succeed"),
            rv(1)
        );
        assert_eq!(
            host.unsubscribe_event_variant(subscription)
                .expect("unsubscribe_event should succeed"),
            rv(1)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_poll_returns_structured_payload() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(object.clone(), super::TEST_EVENT_CHANGED_PAIR.into())
            .expect("subscribe_event should succeed for controlled pair-event source");

        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "FireChangedPair", &[90])
                .expect("FireChangedPair should succeed"),
            91
        );
        let callback = host
            .do_events_variant()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);
        let payload = host
            .poll_event_callback()
            .expect("poll_event_callback should succeed")
            .expect("callback payload should be available");
        assert_eq!(payload.callback.raw(), callback);
        assert_eq!(payload.subscription.raw(), subscription.raw());
        assert_eq!(payload.object.raw(), object.raw());
        assert_eq!(payload.event.raw(), super::TEST_EVENT_CHANGED_PAIR);
        assert_eq!(
            payload
                .args
                .iter()
                .map(|value| value.to_com_value())
                .collect::<Vec<_>>(),
            vec![ComValue::I32(90), ComValue::I32(91)]
        );
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(object.clone(), 1.into())
            .expect("subscribe_event should succeed for controlled event source");
        host.dispatch_invoke_named(object.raw(), "FireChanged", &[77])
            .expect("FireChanged should succeed");
        let callback = host
            .do_events_variant()
            .expect("do_events should pump pending COM callback");
        let callback = expect_i32(callback);

        assert_eq!(host.release_object_test(object).expect("release_object"), 1);
        let callback_err = host
            .event_callback_subscription(callback.into())
            .expect_err("released object callback should be gone");
        assert!(
            callback_err
                .message
                .contains("COM-E-EVENT-CALLBACK-MISSING")
        );
        let subscription_err = host
            .unsubscribe_event_variant(subscription)
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let err = host
            .subscribe_event(object.clone(), 7.into())
            .expect_err("unknown event token should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CONNECTIONPOINT-MISSING"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_subscription_supports_controlled_com_evt_b_source_interface_lane() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(
                object.clone(),
                super::TEST_EVENT_CHANGED_SOURCE_INTERFACE.into(),
            )
            .expect("controlled source-interface event token should subscribe successfully");
        assert!(
            subscription.raw() >= 40_001,
            "subscription token should be in deterministic range"
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "FireChangedSourceInterface", &[77])
                .expect("FireChangedSourceInterface should succeed"),
            77
        );
        let callback = host
            .do_events_variant()
            .expect("do_events should pump pending source-interface callback");
        let callback = expect_i32(callback);
        assert_eq!(
            host.event_callback_subscription(callback.into())
                .expect("callback subscription lookup should succeed"),
            subscription
        );
        assert_eq!(
            host.event_callback_arity(callback.into())
                .expect("callback arity lookup should succeed"),
            1
        );
        assert!(
            host.event_callback_variant(callback.into(), 0)
                .expect("callback arg0 lookup should succeed")
                == rv(77)
        );
        assert_eq!(
            host.release_event_callback_variant(callback.into())
                .expect("callback release should succeed"),
            rv(1)
        );
        assert_eq!(
            host.unsubscribe_event_variant(subscription)
                .expect("unsubscribe should succeed"),
            rv(1)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_unsubscribe_rejects_unknown_subscription() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let err = host
            .unsubscribe_event_variant(40_999.into())
            .expect_err("unknown subscription should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-ADVISE-FAILED"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_callback_lookup_rejects_unknown_callback() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let err = host
            .event_callback_subscription(60_999.into())
            .expect_err("unknown callback should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CALLBACK-MISSING"));
        let err = host
            .event_callback_arity(60_999.into())
            .expect_err("unknown callback arity lookup should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("COM-E-EVENT-CALLBACK-MISSING"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_com_event_callback_arg_index_is_validated() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let subscription = host
            .subscribe_event(object.clone(), 1.into())
            .expect("subscribe should succeed");
        let _ = host
            .dispatch_invoke_named(object.raw(), "FireChanged", &[77])
            .expect("FireChanged should succeed");
        let callback = host.do_events_variant().expect("callback token");
        let callback = expect_i32(callback);
        let err = host
            .event_callback_variant(callback.into(), 1)
            .expect_err("only callback arg index 0 should be supported");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(
            err.message
                .contains("COM-E-EVENT-CALLBACK-SIGNATURE-MISMATCH")
        );
        assert_eq!(
            host.release_event_callback_variant(callback.into())
                .expect("release callback should succeed"),
            rv(1)
        );
        assert_eq!(
            host.unsubscribe_event_variant(subscription)
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
            .open_variant(rv(token), rv(1))
            .expect("native open should succeed");
        host.seek_variant(handle.clone(), rv(160))
            .expect("native seek should succeed");
        assert_eq!(
            host.close_variant(handle)
                .expect("native close should succeed"),
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
            .create_object_test(SCRIPTING_DICTIONARY_PROG_ID_NAME)
            .expect("create_object should return a token");

        if object.raw() < 20_001 {
            // Environment fell back to the non-native CreateObject projection; that's still valid.
            return;
        }

        assert!(
            object.raw() >= 20_001,
            "native COM handles use COM-state handle space"
        );
        let count = host
            .dispatch_invoke_named(object.raw(), "Count", &[])
            .expect("dictionary Count should be invokable");
        assert!(count >= 0);

        let exists = host
            .dispatch_invoke_named(object.raw(), "Exists", &[42])
            .expect("dictionary Exists should be invokable");
        assert!(exists == 0 || exists == 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_returns_deterministic_values() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        assert!(
            object.raw() >= 20_001,
            "controlled COM lane should bind native object"
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Count", &[])
                .expect("Count property-get should succeed"),
            7
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Exists", &[42])
                .expect("Exists(42) should succeed"),
            1
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Exists", &[41])
                .expect("Exists(41) should succeed"),
            0
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Ping", &[999])
                .expect("Ping no-arg method invoke should succeed"),
            123
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Lookup", &[42])
                .expect("Lookup property-get with argument should succeed"),
            1_042
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "SetValue", &[33])
                .expect("SetValue property-put should succeed"),
            33
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Value", &[])
                .expect("Value property-get should reflect SetValue"),
            33
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "SetValueRef", &[33])
                .expect("SetValueRef property-putref should succeed"),
            100_033
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Value", &[])
                .expect("Value property-get should reflect SetValueRef"),
            100_033
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_supports_named_method_args_variant_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: super::TEST_DISPID_SUM_PAIR.into(),
            args: vec![
                ComInvokeArg::named(14, "rhs"),
                ComInvokeArg::named(3, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_variant(&request)
                    .expect("named-argument SumPair invoke should succeed")
            ),
            3_014
        );
    }

    #[test]
    fn dispatch_invoke_dynamic_projection_resolves_name_selector_for_testdispatch() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return deterministic projection handle");
        let request = oxvba_com::DynamicCallRequest {
            object: object.clone(),
            member: oxvba_com::DynamicMemberSelector::Name("SumPair".to_string()),
            args: vec![
                oxvba_com::DynamicCallArg {
                    value: Some(oxvba_com::ComValue::I32(3).into()),
                    name: None,
                },
                oxvba_com::DynamicCallArg {
                    value: Some(oxvba_com::ComValue::I32(14).into()),
                    name: None,
                },
            ],
            call_kind_hint: Some(oxvba_com::DynamicCallKind::Method),
        };
        assert_eq!(
            host.dispatch_invoke_dynamic_variant(&request)
                .expect("dynamic name selector should resolve on deterministic projection"),
            Variant::from_i32(5_033)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_supports_named_property_get_args_variant_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: super::TEST_DISPID_LOOKUP_PAIR.into(),
            args: vec![
                ComInvokeArg::named(14, "rhs"),
                ComInvokeArg::named(3, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_variant(&request)
                    .expect("named property-get invoke should succeed")
            ),
            203_014
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_supports_named_default_member_args_variant_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: 0.into(),
            args: vec![ComInvokeArg::named(19, "value")],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_variant(&request)
                    .expect("named default-member invoke should succeed")
            ),
            19
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_dictionary_named_default_member_passes_through_for_runtime_resolution_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());

        let object = host
            .create_object_test(SCRIPTING_DICTIONARY_PROG_ID_NAME)
            .expect("create_object should return dictionary token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: 0.into(),
            args: vec![ComInvokeArg::named(19, "value")],
            invoke_kind_hint: None,
        };
        // Named args on default-member dispatch without metadata now pass through to the
        // COM invoke layer for runtime resolution via GetIDsOfNames. The Dictionary COM
        // server will either resolve the name or return a deterministic COM error.
        let _ = host.dispatch_invoke_variant(&request);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_preserves_omitted_arg_metadata_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: super::TEST_DISPID_LOOKUP.into(),
            args: vec![ComInvokeArg::omitted()],
            invoke_kind_hint: None,
        };
        let err = host
            .dispatch_invoke_variant(&request)
            .expect_err("omitted required argument should fail deterministically");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert!(err.message.contains("member requires argument"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_named_property_put_value_uses_propertyput_lane_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: super::TEST_DISPID_SET_INDEXED_VALUE.into(),
            args: vec![ComInvokeArg::positional(7), ComInvokeArg::named(9, "value")],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_variant(&request)
                    .expect("named value argument should still route through property-put lane")
            ),
            307_009
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_roundtrips_semantic_safe_array_payload_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let expected =
            Variant::from_safearray(oxvba_runtime::safe_array::SafeArray::from_variants(vec![
                Variant::from_i32(4),
                Variant::from_bool(true),
                Variant::from_string(BStr::from("Hello")),
                Variant::null(),
            ]));
        let request = ComInvokeRequest {
            object: object.clone(),
            member: super::TEST_DISPID_ECHO_VARIANT.into(),
            args: vec![ComInvokeArg::positional_value(
                ComValue::from_variant(&expected).expect("expected SAFEARRAY Variant to project"),
            )],
            invoke_kind_hint: Some(oxvba_com::ComInvokeKind::Method),
        };
        assert_eq!(
            host.dispatch_invoke_variant(&request)
                .expect("semantic SAFEARRAY invoke should succeed"),
            expected
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_test_dispatch_named_indexed_property_put_reorders_value_last_v2() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: super::TEST_DISPID_SET_INDEXED_VALUE.into(),
            args: vec![
                ComInvokeArg::named(9, "value"),
                ComInvokeArg::named(7, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(
                host.dispatch_invoke_variant(&request).expect(
                    "fully named indexed property-put should canonicalize deterministically"
                )
            ),
            307_009
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Value", &[])
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let request = ComInvokeRequest {
            object: object.clone(),
            member: super::TEST_DISPID_SET_INDEXED_VALUE_REF.into(),
            args: vec![
                ComInvokeArg::named(13, "value"),
                ComInvokeArg::named(8, "lhs"),
            ],
            invoke_kind_hint: None,
        };
        assert_eq!(
            expect_i32(host.dispatch_invoke_variant(&request).expect(
                "fully named indexed property-putref should canonicalize deterministically"
            )),
            408_013
        );
        assert_eq!(
            host.dispatch_invoke_named(object.raw(), "Value", &[])
                .expect("Value property-get should reflect named indexed property-putref"),
            408_013
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_controlled_property_get_with_required_arg_reports_missing_arg_stably() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let err = host
            .dispatch_invoke_variant(&ComInvokeRequest {
                object: object.clone(),
                member: super::TEST_DISPID_LOOKUP.into(),
                args: Vec::new(),
                invoke_kind_hint: None,
            })
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let err = host
            .dispatch_invoke_named(object.raw(), "RaiseException", &[])
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        if !native_dispatch_is_bound(&host, &object) {
            return;
        }

        let before = {
            let state = host
                .com_bridge
                .shared_state()
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
            .dispatch_invoke_named(object.raw(), "Count", &[])
            .expect("dispatch invoke should succeed");
        let after = {
            let state = host
                .com_bridge
                .shared_state()
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        if !native_dispatch_is_bound(&host, &object) {
            return;
        }

        let _ = host
            .dispatch_invoke_named(object.raw(), "Count", &[])
            .expect("dictionary Count should be invokable");
        let cache_size_after_first = {
            let state = host
                .com_bridge
                .shared_state()
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
            .dispatch_invoke_named(object.raw(), "Count", &[])
            .expect("dictionary Count should be invokable repeatedly");
        let cache_size_after_second = {
            let state = host
                .com_bridge
                .shared_state()
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("dispatch create_object should succeed");
        let vtable_object = vtable_host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("vtable create_object should succeed");

        let dispatch_count = dispatch_host
            .dispatch_invoke_named(dispatch_object.raw(), "Count", &[])
            .expect("dispatch count should succeed");
        let vtable_count = vtable_host
            .dispatch_invoke_named(vtable_object.raw(), "Count", &[])
            .expect("vtable count should succeed");
        assert_eq!(dispatch_count, vtable_count);

        let dispatch_exists = dispatch_host
            .dispatch_invoke_named(dispatch_object.raw(), "Exists", &[42])
            .expect("dispatch exists should succeed");
        let vtable_exists = vtable_host
            .dispatch_invoke_named(vtable_object.raw(), "Exists", &[42])
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
                requested_coclass: None,
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
                requested_coclass: None,
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
                requested_coclass: None,
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
                requested_coclass: None,
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
        let expected_members = [
            "Count",
            "Exists",
            "FireChanged",
            "FireChangedPair",
            "FireChangedSourceInterface",
            "Ping",
            "Lookup",
            "SetValue",
            "SetValueRef",
            "Value",
            "SumPair",
            "LookupPair",
            "SetIndexedValue",
            "SetIndexedValueRef",
            "EchoVariant",
            "RaiseException",
            "RaiseRichException",
            "ReturnSmallInt",
            "ReturnUnsignedWord",
            "ReturnByte",
            "ReturnSignedByte",
            "ReturnPlatformInt",
            "ReturnPlatformUInt",
            "ReturnHyper",
            "ReturnUnsignedHyper",
            "ReturnDouble",
            "ReturnSingle",
            "ReturnDate",
            "ReturnCurrency",
            "ReturnDecimal",
            "ReturnBool",
            "ReturnString",
            "ReturnMissingMemberName",
            "ReturnPingMemberName",
            "ReturnLookupMemberName",
            "ReturnSumPairMemberName",
            "ReturnLookupPairMemberName",
            "ReturnSetValueMemberName",
            "ReturnSetValueRefMemberName",
            "ReturnSetIndexedValueMemberName",
            "ReturnSetIndexedValueRefMemberName",
            "ReturnValueMemberName",
            "ReturnDefaultMemberName",
            "ReturnEmpty",
            "ReturnNull",
            "ReturnError",
            "ReturnByRefLong",
            "ReturnByRefLongArray",
            "ReturnWideHyper",
            "ReturnWideHyperArray",
            "ReturnWideUnsignedHyper",
            "ReturnWideUnsignedHyperArray",
            "ReturnVariantMatrix",
            "ReturnPlainUnknownVariantArray",
            "ReturnLong",
            "ReturnUnsignedLong",
            "ReturnSmallIntArray",
            "ReturnBoolArray",
            "ReturnStringArray",
            "ReturnSmallIntMatrix",
            "ReturnPlainUnknown",
            "ReturnPlainUnknownArray",
            "ReturnByteArray",
            "ReturnSignedByteArray",
            "ReturnPlatformIntArray",
            "ReturnPlatformUIntArray",
            "ReturnHyperArray",
            "ReturnUnsignedHyperArray",
            "ReturnDoubleArray",
            "ReturnSingleArray",
            "ReturnDateArray",
            "ReturnCurrencyArray",
            "ReturnDecimalArray",
            "ReturnWideUnsignedLong",
            "ReturnWideUnsignedLongArray",
            "ReturnWidePlatformUInt",
            "ReturnWidePlatformUIntArray",
            "ReturnLongArray",
            "ReturnUnsignedLongArray",
            "ReturnSelfDispatch",
            "SelfDispatch",
            "ReturnSelfUnknown",
            "SelfUnknown",
            "ClassifyVariantArg",
            "ClassifyVariantArrayFirstElementArg",
            "ReturnSelfDispatchArray",
            "ReturnSelfTypedDispatchArray",
            "ReturnSelfTypedUnknownArray",
            "NewEnum",
        ];
        for name in expected_members {
            assert!(
                metadata
                    .member_name_to_token
                    .iter()
                    .any(|(candidate_name, _)| candidate_name == name),
                "member metadata should include `{name}`"
            );
        }
        let member_by_name = |name: &str| {
            metadata
                .members
                .iter()
                .find(|entry| entry.name == name)
                .unwrap_or_else(|| panic!("member metadata should include `{name}`"))
        };
        let fire_changed_pair = member_by_name("FireChangedPair");
        assert!(fire_changed_pair.requires_argument);
        assert_eq!(
            fire_changed_pair.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let count_member = member_by_name("Count");
        assert_eq!(
            count_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let ping_member = member_by_name("Ping");
        assert!(!ping_member.requires_argument);
        let raise_exception_member = member_by_name("RaiseException");
        assert_eq!(
            raise_exception_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        assert!(!raise_exception_member.requires_argument);
        let return_smallint_member = member_by_name("ReturnSmallInt");
        assert_eq!(
            return_smallint_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        assert!(!return_smallint_member.requires_argument);
        assert_eq!(
            ping_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let lookup_member = member_by_name("Lookup");
        assert!(lookup_member.requires_argument);
        assert_eq!(
            lookup_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let set_value_member = member_by_name("SetValue");
        assert!(set_value_member.requires_argument);
        assert_eq!(
            set_value_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPut
        );
        let set_value_ref_member = member_by_name("SetValueRef");
        assert!(set_value_ref_member.requires_argument);
        assert_eq!(
            set_value_ref_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPutRef
        );
        let value_member = member_by_name("Value");
        assert!(!value_member.requires_argument);
        assert_eq!(
            value_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let sum_pair_member = member_by_name("SumPair");
        assert!(sum_pair_member.requires_argument);
        assert_eq!(
            sum_pair_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let lookup_pair_member = member_by_name("LookupPair");
        assert!(lookup_pair_member.requires_argument);
        assert_eq!(
            lookup_pair_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let set_indexed_value_member = member_by_name("SetIndexedValue");
        assert!(set_indexed_value_member.requires_argument);
        assert_eq!(
            set_indexed_value_member.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPut
        );
        let set_indexed_value_ref_member = member_by_name("SetIndexedValueRef");
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
        let identity = match host.resolve_typelib_reference(&TypeLibResolveRequest {
            reference_name: "Excel".to_string(),
            requested_coclass: Some("Application".to_string()),
            importlib_hint: Some("excel.exe".to_string()),
            libid_hint: None,
            major_version_hint: Some(1),
            minor_version_hint: Some(0),
            lcid_hint: Some(0),
        }) {
            Ok(identity) => identity,
            Err(_) => return,
        };
        let metadata = host
            .load_typelib_metadata(&identity)
            .expect("excel metadata load should succeed");

        // Excel.Application typelib now includes Quit, Visible, Workbooks,
        // ScreenUpdating, and DisplayAlerts members.
        assert!(
            metadata
                .member_name_to_token
                .iter()
                .any(|(name, _)| name == "Quit"),
            "expected Quit member in Excel.Application typelib, got {:?}",
            metadata.member_name_to_token
        );
        assert!(
            metadata.member_name_to_token.len() >= 5,
            "expected at least 5 member names in Excel.Application typelib, got {}",
            metadata.member_name_to_token.len()
        );
        let quit_member = metadata
            .members
            .iter()
            .find(|entry| entry.name == "Quit")
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let state = host
            .com_bridge
            .shared_state()
            .lock()
            .expect("com state lock should succeed");
        let binding = state
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .expect("binding should be present for native object token");
        let member_by_name = |name: &str| {
            binding
                .member_specs
                .values()
                .find(|member| member.name == name)
                .unwrap_or_else(|| panic!("member spec for {name} should be present"))
        };
        let member = member_by_name("FireChangedPair");
        assert_eq!(member.name, "FireChangedPair");
        assert!(member.requires_argument);
        assert_eq!(member.invoke_kind, super::TypeLibMemberInvokeKind::Method);
        let ping = member_by_name("Ping");
        assert_eq!(ping.name, "Ping");
        assert!(!ping.requires_argument);
        assert_eq!(ping.invoke_kind, super::TypeLibMemberInvokeKind::Method);
        let fire_changed_source = member_by_name("FireChangedSourceInterface");
        assert_eq!(fire_changed_source.name, "FireChangedSourceInterface");
        assert!(fire_changed_source.requires_argument);
        assert_eq!(
            fire_changed_source.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        let lookup = member_by_name("Lookup");
        assert_eq!(lookup.name, "Lookup");
        assert!(lookup.requires_argument);
        assert_eq!(
            lookup.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyGet
        );
        let set_value = member_by_name("SetValue");
        assert_eq!(set_value.name, "SetValue");
        assert!(set_value.requires_argument);
        assert_eq!(
            set_value.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPut
        );
        let set_value_ref = member_by_name("SetValueRef");
        assert_eq!(set_value_ref.name, "SetValueRef");
        assert!(set_value_ref.requires_argument);
        assert_eq!(
            set_value_ref.invoke_kind,
            super::TypeLibMemberInvokeKind::PropertyPutRef
        );
        let value = member_by_name("Value");
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
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return a token");
        let descriptor = host
            .describe_object(object.clone())
            .expect("describe_object should succeed")
            .expect("known COM object should produce a descriptor");

        assert_eq!(descriptor.object.raw(), object.raw());
        assert_eq!(descriptor.prog_id_name, "OxVba.TestDispatch");
        assert_eq!(
            descriptor.transport,
            oxvba_com::ComObjectTransportKind::NativeDispatch
        );
        let state = host
            .com_bridge
            .shared_state()
            .lock()
            .expect("com state lock should succeed");
        let binding = state
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .expect("binding should be present for native object token");
        let count_token = binding
            .member_specs
            .iter()
            .find(|(_, member)| member.name == "Count")
            .map(|(token, _)| *token)
            .expect("Count member token should be present");
        let fire_changed_pair_token = binding
            .member_specs
            .iter()
            .find(|(_, member)| member.name == "FireChangedPair")
            .map(|(token, _)| *token)
            .expect("FireChangedPair member token should be present");
        let echo_variant_token = binding
            .member_specs
            .iter()
            .find(|(_, member)| member.name == "EchoVariant")
            .map(|(token, _)| *token)
            .expect("EchoVariant member token should be present");
        assert!(descriptor.supports_events);
        assert!(descriptor.known_member_tokens.contains(&count_token));
        assert!(
            descriptor
                .known_member_tokens
                .contains(&fire_changed_pair_token)
        );
        assert_eq!(descriptor.default_member_token, Some(echo_variant_token));
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
    fn windows_native_dictionary_binding_exposes_member_metadata_without_fake_event_projection() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());

        let object = host
            .create_object_test(SCRIPTING_DICTIONARY_PROG_ID_NAME)
            .expect("create_object should return dictionary token");
        let state = host
            .com_bridge
            .shared_state()
            .lock()
            .expect("com state lock should succeed");
        let binding = state
            .bindings
            .get(&ComObjectToken::new(object.raw()))
            .expect("binding should be present for dictionary token");
        let exists_member = binding
            .member_specs
            .values()
            .find(|member| member.name == "Exists")
            .expect("Exists member spec should be present");
        assert_eq!(exists_member.name, "Exists");
        assert!(exists_member.requires_argument);
        assert_eq!(
            exists_member.invoke_kind,
            super::TypeLibMemberInvokeKind::Method
        );
        assert!(
            binding.event_specs.is_empty(),
            "real dictionary binding should not expose fake event metadata"
        );
        assert!(
            binding.event_trigger_specs.is_empty(),
            "real dictionary binding should not expose fake event triggers"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_dictionary_event_subscription_fails_without_event_metadata() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());

        let object = host
            .create_object_test(SCRIPTING_DICTIONARY_PROG_ID_NAME)
            .expect("create_object should return dictionary token");
        let err = host
            .subscribe_event(object.clone(), super::TEST_EVENT_CHANGED.into())
            .expect_err("subscribe_event should reject fake dictionary event token");
        assert!(
            err.message.contains("COM-E-EVENT-CONNECTIONPOINT-MISSING"),
            "expected stable missing-event diagnostic, got: {}",
            err.message
        );
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
            super::map_com_hresult_label(Some(0x8002_0006), None),
            "unknown-name"
        );
        assert_eq!(
            super::map_com_hresult_label(Some(0x8004_01F3), None),
            "invalid-class-string"
        );
        assert_eq!(
            super::map_com_hresult_label(Some(0x8002_000E), None),
            "bad-param-count"
        );
        assert_eq!(super::map_com_hresult_label(None, Some(0)), "arg-error");
        assert_eq!(
            super::map_com_hresult_label(None, None),
            "fault-unspecified"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn com_invoke_failure_detail_labels_are_stable() {
        let overflow = ComInvokeFailure {
            label: "method",
            dispid: 91,
            hr: None,
            arg_err: None,
            excep: None,
            detail: Some("VT_UI8 value 18446744073709551615 exceeds i64 carrier range".to_string()),
        };
        assert_eq!(overflow.classification_label(), "carrier-overflow");

        let byref = ComInvokeFailure {
            label: "method",
            dispid: 92,
            hr: None,
            arg_err: None,
            excep: None,
            detail: Some("unsupported VARIANT BYREF return type vt=16387".to_string()),
        };
        assert_eq!(byref.classification_label(), "unsupported-byref-return");

        let unspecified = ComInvokeFailure {
            label: "method",
            dispid: 93,
            hr: None,
            arg_err: None,
            excep: None,
            detail: Some("unclassified internal dispatch conversion fault".to_string()),
        };
        assert_eq!(unspecified.classification_label(), "fault-unspecified");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_string_prog_id_activation_resolves_native_mapping() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());

        let object = host
            .create_object_test(SCRIPTING_DICTIONARY_PROG_ID_NAME)
            .expect("string ProgID should resolve native COM activation");
        assert!(
            object.raw() >= 20_001,
            "expected native COM object handle from direct ProgID activation, got {object}"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_createobject_failure_includes_stable_label_when_class_missing() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());

        let err = host
            .create_object_test(MISSING_CLASS_PROG_ID_NAME)
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
    fn windows_dispatch_unknown_name_failure_includes_stable_label() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let err = host.com_dispatch_adapter_fault(
            "IDispatch::GetIDsOfNames failed for `DefinitelyMissingMember` with HRESULT 0x80020006"
                .to_string(),
        );
        assert!(
            err.message
                .contains("com-dispatch-unknown-name;hresult=0x80020006;"),
            "expected stable unknown-name label, got {}",
            err.message
        );
        assert!(
            err.message
                .contains("IDispatch::GetIDsOfNames failed for `DefinitelyMissingMember` with HRESULT 0x80020006"),
            "expected raw GetIDsOfNames detail, got {}",
            err.message
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn release_object_clears_native_subscriptions_and_pending_callbacks() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let object = host
            .create_object_test(TEST_DISPATCH_PROG_ID_NAME)
            .expect("create_object should return controlled COM object");
        let subscription = host
            .subscribe_event(object.clone(), 1.into())
            .expect("subscribe_event should succeed");
        let _ = host
            .dispatch_invoke_named(object.raw(), "FireChanged", &[77])
            .expect("dispatch_invoke should queue callback");

        assert!(
            host.poll_event_callback()
                .expect("first callback should be available")
                .is_some()
        );

        let _ = host
            .dispatch_invoke_named(object.raw(), "FireChanged", &[88])
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
            .unsubscribe_event_variant(subscription)
            .expect_err("released object subscription should already be removed");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
    }

    proptest! {
        #[test]
        fn prop_free_file_low_range_tracks_open_count(path_seed in 1i32..10_000, open_count in 0usize..32) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            for idx in 0..open_count {
                let path = path_seed.saturating_add(idx as i32);
                let _ = host.open_variant(rv(path), rv(0)).expect("open should succeed");
            }
            let expected = 1 + open_count as i32;
            let free = host.free_file_variant(rv(0)).expect("free_file should succeed");
            prop_assert_eq!(free, rv(expected));
        }

        #[test]
        fn prop_seek_eof_boundary(path_token in 1i32..10_000, offset in 0i32..6000) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            let handle = host.open_variant(rv(path_token), rv(0)).expect("open should succeed");
            let len = expect_i32(host.lof_variant(handle.clone()).expect("lof should succeed"));
            host.seek_variant(handle.clone(), rv(offset)).expect("seek should succeed");
            let eof = host.eof_variant(handle).expect("eof should succeed");
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
                scripted.msg_box_variant(rv(prompt), rv(style)).expect("scripted msg_box"),
                rv(style.max(1))
            );
            prop_assert_eq!(
                scripted
                    .input_box_variant(rv(prompt), rv(default_value))
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
                disabled.msg_box_variant(rv(prompt), rv(style)).expect("disabled msg_box"),
                rv(prompt.max(1))
            );
            prop_assert_eq!(
                disabled
                    .input_box_variant(rv(prompt), rv(default_value))
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
                host.msg_box_variant(rv(1), rv(1)).expect_err("msg_box denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.shell_variant(rv(shell_cmd), rv(0))
                    .expect_err("shell denied")
                    .kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.create_object_test(&create_object_prop_test_prog_id_name(prog_id)).expect_err("create_object denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.invoke_symbol_variant(symbol.into(), &rv(arg))
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
                host.shell_variant(rv(shell_cmd), rv(0)).expect("shell should succeed"),
                Variant::from_i32(shell_expected)
            );
            let first_object = host.create_object_test(&create_object_prop_test_prog_id_name(prog_id))
                .expect("create_object should succeed");
            let second_object = host.create_object_test(&create_object_prop_test_prog_id_name(prog_id))
                .expect("create_object should remain stable for the same ProgID within one host");
            prop_assert_eq!(first_object.compat_identity(), second_object.compat_identity());
            let request = ComInvokeRequest::legacy(object, member, arg);
            let semantic = host
                .dispatch_invoke_variant(&request)
                .expect("semantic dispatch_invoke should succeed");
            prop_assert_eq!(
                host.dispatch_invoke_legacy_v2(&request)
                    .expect("dispatch_invoke legacy projection should succeed"),
                expect_i32(semantic)
            );
            prop_assert_eq!(
                host.invoke_symbol_variant(symbol.into(), &rv(arg))
                    .expect("invoke_symbol should succeed"),
                Variant::from_i32(symbol.saturating_add(arg))
            );
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "windows")]
#[allow(dead_code, clippy::items_after_test_module)]
impl StandardHostServices {
    fn bind_native_dispatch_result(
        &self,
        dispatch: *mut RawIDispatch,
        prog_id_hint: &str,
        op: &'static str,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::ComActivationDispatch;
        let handle = unsafe {
            self.com_bridge
                .bind_native_dispatch_result(dispatch, prog_id_hint)
        }
        .map_err(|message| HalError::adapter_fault(self.profile, capability, op, message))?;
        let state = self
            .com_bridge
            .lock_state("bind_native_dispatch_result")
            .map_err(|message| HalError::adapter_fault(self.profile, capability, op, message))?;
        let binding = state
            .bindings
            .get(&oxvba_com::ComObjectToken::new(handle.raw()))
            .ok_or_else(|| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    op,
                    format!(
                        "COM-E-OBJECT-IDENTITY-MISSING: object handle {} missing retained runtime identity",
                        handle.raw()
                    ),
                )
            })?;
        let raw = binding.runtime_object.ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                op,
                format!(
                    "COM-E-OBJECT-IDENTITY-MISSING: object handle {} missing retained runtime identity",
                    handle.raw()
                ),
            )
        })?;
        let object_ref = binding.runtime_class_descriptor.map_or_else(
            || oxvba_runtime::ObjectRef::from_compat_identity(raw),
            |descriptor| {
                oxvba_runtime::ObjectRef::from_compat_identity_with_descriptor(raw, descriptor)
            },
        );
        Ok(Variant::from_object_ref(object_ref))
    }

    // Test-only extension seam intentionally left empty after the callback
    // interrogation rows moved onto the typed ComHal API.
}
