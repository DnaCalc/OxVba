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
}

impl HostPolicy {
    pub fn deterministic_runtime() -> Self {
        Self {
            allow_interaction: false,
            allow_process_spawn: true,
            allow_filesystem_mutation: false,
            allow_dynamic_link: false,
            allow_com_activation: true,
            deterministic_mode: true,
            ui_virtualization: UiVirtualizationMode::ScriptedResponses,
            unsupported_feature_mode: UnsupportedFeatureMode::Runtime,
        }
    }

    pub fn deterministic_compile_time() -> Self {
        Self {
            unsupported_feature_mode: UnsupportedFeatureMode::CompileTime,
            ..Self::deterministic_runtime()
        }
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
