//! Contract model for HAL capabilities, profile descriptors, and host policy.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HalProfileId {
    Windows,
    Linux,
    MacOs,
    Wasm,
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapabilityId {
    UiInteraction,
    EventPump,
    FileSystemIo,
    ProcessEnv,
    ComActivationDispatch,
    TimeLocale,
    DynamicLinking,
    DiagnosticsTelemetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityMaturity {
    Stub,
    Experimental,
    Provisional,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    pub id: CapabilityId,
    pub supported: bool,
    pub maturity: CapabilityMaturity,
    pub spec_anchor: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HalDescriptor {
    pub profile: HalProfileId,
    pub runtime_class: &'static str,
    pub contract_version: &'static str,
    pub adapter_version: &'static str,
    pub capabilities: Vec<CapabilityDescriptor>,
}

impl HalDescriptor {
    pub fn capability(&self, id: CapabilityId) -> Option<&CapabilityDescriptor> {
        self.capabilities.iter().find(|entry| entry.id == id)
    }

    pub fn supports(&self, id: CapabilityId) -> bool {
        self.capability(id).is_some_and(|entry| entry.supported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiVirtualizationMode {
    Disabled,
    ScriptedResponses,
    FailOnPrompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedFeatureMode {
    CompileTime,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmRuntimeClass {
    Wasi,
    BrowserSandbox,
}

impl WasmRuntimeClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wasi => "wasi",
            Self::BrowserSandbox => "browser-sandbox",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPolicyPreset {
    StrictCi,
    DeterministicRuntime,
    DeterministicCompileTime,
    InteractiveDev,
}

impl HostPolicyPreset {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StrictCi => "strict-ci",
            Self::DeterministicRuntime => "deterministic-runtime",
            Self::DeterministicCompileTime => "deterministic-compile-time",
            Self::InteractiveDev => "interactive-dev",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPolicy {
    pub allow_interaction: bool,
    pub allow_process_spawn: bool,
    pub allow_filesystem_mutation: bool,
    pub allow_dynamic_link: bool,
    pub allow_com_activation: bool,
    pub deterministic_mode: bool,
    pub ui_virtualization: UiVirtualizationMode,
    pub unsupported_feature_mode: UnsupportedFeatureMode,
    pub wasm_runtime_class: WasmRuntimeClass,
}

impl HostPolicy {
    pub fn for_preset(preset: HostPolicyPreset) -> Self {
        match preset {
            HostPolicyPreset::StrictCi => Self {
                allow_interaction: false,
                allow_process_spawn: false,
                allow_filesystem_mutation: false,
                allow_dynamic_link: false,
                allow_com_activation: false,
                deterministic_mode: true,
                ui_virtualization: UiVirtualizationMode::FailOnPrompt,
                unsupported_feature_mode: UnsupportedFeatureMode::CompileTime,
                wasm_runtime_class: WasmRuntimeClass::Wasi,
            },
            HostPolicyPreset::DeterministicRuntime => Self {
                allow_interaction: false,
                allow_process_spawn: true,
                allow_filesystem_mutation: false,
                allow_dynamic_link: false,
                allow_com_activation: true,
                deterministic_mode: true,
                ui_virtualization: UiVirtualizationMode::ScriptedResponses,
                unsupported_feature_mode: UnsupportedFeatureMode::Runtime,
                wasm_runtime_class: WasmRuntimeClass::Wasi,
            },
            HostPolicyPreset::DeterministicCompileTime => Self {
                unsupported_feature_mode: UnsupportedFeatureMode::CompileTime,
                ..Self::for_preset(HostPolicyPreset::DeterministicRuntime)
            },
            HostPolicyPreset::InteractiveDev => Self {
                allow_interaction: true,
                allow_process_spawn: true,
                allow_filesystem_mutation: true,
                allow_dynamic_link: true,
                allow_com_activation: true,
                deterministic_mode: false,
                ui_virtualization: UiVirtualizationMode::Disabled,
                unsupported_feature_mode: UnsupportedFeatureMode::Runtime,
                wasm_runtime_class: WasmRuntimeClass::Wasi,
            },
        }
    }

    pub fn with_wasm_runtime_class(mut self, runtime_class: WasmRuntimeClass) -> Self {
        self.wasm_runtime_class = runtime_class;
        self
    }

    pub fn strict_ci() -> Self {
        Self::for_preset(HostPolicyPreset::StrictCi)
    }

    pub fn deterministic_runtime() -> Self {
        Self::for_preset(HostPolicyPreset::DeterministicRuntime)
    }

    pub fn deterministic_compile_time() -> Self {
        Self::for_preset(HostPolicyPreset::DeterministicCompileTime)
    }

    pub fn interactive_dev() -> Self {
        Self::for_preset(HostPolicyPreset::InteractiveDev)
    }
}

impl Default for HostPolicy {
    fn default() -> Self {
        Self::deterministic_runtime()
    }
}

pub const ALL_CAPABILITIES: [CapabilityId; 8] = [
    CapabilityId::UiInteraction,
    CapabilityId::EventPump,
    CapabilityId::FileSystemIo,
    CapabilityId::ProcessEnv,
    CapabilityId::ComActivationDispatch,
    CapabilityId::TimeLocale,
    CapabilityId::DynamicLinking,
    CapabilityId::DiagnosticsTelemetry,
];

pub fn host_backed_profile_matches_host(profile: HalProfileId) -> bool {
    match profile {
        HalProfileId::Windows => cfg!(target_os = "windows"),
        HalProfileId::Linux => cfg!(target_os = "linux"),
        _ => false,
    }
}

pub fn host_backed_mode_active(profile: HalProfileId, policy: &HostPolicy) -> bool {
    !policy.deterministic_mode && host_backed_profile_matches_host(profile)
}

#[cfg(test)]
mod tests {
    use super::{
        HalProfileId, HostPolicy, HostPolicyPreset, UiVirtualizationMode, UnsupportedFeatureMode,
        WasmRuntimeClass, host_backed_profile_matches_host,
    };

    #[test]
    fn preset_deterministic_runtime_matches_existing_factory() {
        assert_eq!(
            HostPolicy::deterministic_runtime(),
            HostPolicy::for_preset(HostPolicyPreset::DeterministicRuntime)
        );
    }

    #[test]
    fn preset_deterministic_compile_time_sets_compile_mode() {
        let policy = HostPolicy::deterministic_compile_time();
        assert_eq!(
            policy.unsupported_feature_mode,
            UnsupportedFeatureMode::CompileTime
        );
        assert_eq!(policy.wasm_runtime_class, WasmRuntimeClass::Wasi);
        assert!(policy.deterministic_mode);
    }

    #[test]
    fn strict_ci_is_fully_restrictive_and_compile_time_gated() {
        let policy = HostPolicy::strict_ci();
        assert!(!policy.allow_interaction);
        assert!(!policy.allow_process_spawn);
        assert!(!policy.allow_filesystem_mutation);
        assert!(!policy.allow_dynamic_link);
        assert!(!policy.allow_com_activation);
        assert!(policy.deterministic_mode);
        assert_eq!(policy.ui_virtualization, UiVirtualizationMode::FailOnPrompt);
        assert_eq!(
            policy.unsupported_feature_mode,
            UnsupportedFeatureMode::CompileTime
        );
        assert_eq!(policy.wasm_runtime_class, WasmRuntimeClass::Wasi);
    }

    #[test]
    fn interactive_dev_relaxes_policy_for_local_exploration() {
        let policy = HostPolicy::interactive_dev();
        assert!(policy.allow_interaction);
        assert!(policy.allow_process_spawn);
        assert!(policy.allow_filesystem_mutation);
        assert!(policy.allow_dynamic_link);
        assert!(policy.allow_com_activation);
        assert!(!policy.deterministic_mode);
        assert_eq!(policy.ui_virtualization, UiVirtualizationMode::Disabled);
        assert_eq!(
            policy.unsupported_feature_mode,
            UnsupportedFeatureMode::Runtime
        );
        assert_eq!(policy.wasm_runtime_class, WasmRuntimeClass::Wasi);
    }

    #[test]
    fn wasm_runtime_class_override_is_available() {
        let policy = HostPolicy::deterministic_runtime()
            .with_wasm_runtime_class(WasmRuntimeClass::BrowserSandbox);
        assert_eq!(policy.wasm_runtime_class, WasmRuntimeClass::BrowserSandbox);
    }

    #[test]
    fn host_backed_profile_match_function_is_stable() {
        let windows = host_backed_profile_matches_host(HalProfileId::Windows);
        let linux = host_backed_profile_matches_host(HalProfileId::Linux);
        let macos = host_backed_profile_matches_host(HalProfileId::MacOs);
        let wasm = host_backed_profile_matches_host(HalProfileId::Wasm);
        let null = host_backed_profile_matches_host(HalProfileId::Null);
        assert!(!macos);
        assert!(!wasm);
        assert!(!null);
        if cfg!(target_os = "windows") {
            assert!(windows);
            assert!(!linux);
        } else if cfg!(target_os = "linux") {
            assert!(!windows);
            assert!(linux);
        } else {
            assert!(!windows);
            assert!(!linux);
        }
    }
}
