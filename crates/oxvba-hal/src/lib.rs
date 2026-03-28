//! oxvba-hal: host abstraction contracts, profile adapters, and conformance harness.

pub mod adapters;
pub mod callbacks;
pub mod conformance;
pub mod dynamic_bridge;
pub mod error;
pub mod journal;
pub mod model;
pub mod project;
pub mod traits;

pub use dynamic_bridge::HalComDynamicBridge;
pub use error::{HalError, HalErrorKind, HalResult};
pub use model::{
    CapabilityDescriptor, CapabilityId, CapabilityMaturity, ComInvocationStrategy, HalDescriptor,
    HalProfileId, HalRuntimeClass, HostPolicy, HostPolicyPreset, UiVirtualizationMode,
    UnsupportedFeatureMode, WasmRuntimeClass, host_backed_mode_active,
    host_backed_profile_matches_host,
};
pub use project::{
    HostExtensionModuleChange, InMemoryProjectCatalog, ProjectCallbackError, ProjectCallbackResult,
    ProjectDescriptor, ProjectDescriptorKind, ProjectReferenceDescriptor, ProjectReferenceKind,
    ResolvedProjectReference,
};
pub use traits::{
    ComHal, ConsoleHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal,
    FileSystemHal, HostServices, ProcessEnvHal, ProjectCatalogHal, ProjectMutationHal,
    ProjectReferenceHal, TimeLocaleHal, TypeLibCacheScope, TypeLibEventDispatchPath,
    TypeLibEventMetadata, TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibResolveRequest,
    TypeLibResolvedIdentity, UiInteractionHal,
};
