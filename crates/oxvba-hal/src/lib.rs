//! oxvba-hal: host abstraction contracts, profile adapters, and conformance harness.

pub mod adapters;
pub mod conformance;
pub mod error;
pub mod model;
pub mod traits;

pub use error::{HalError, HalErrorKind, HalResult};
pub use model::{
    CapabilityDescriptor, CapabilityId, CapabilityMaturity, ComInvocationStrategy, HalDescriptor,
    HalProfileId, HalRuntimeClass, HostPolicy, HostPolicyPreset, UiVirtualizationMode,
    UnsupportedFeatureMode, WasmRuntimeClass, host_backed_mode_active,
    host_backed_profile_matches_host,
};
pub use traits::{
    ComHal, DiagnosticsHal, DynLinkDescriptorView, DynamicLinkHal, EventPumpHal, FileSystemHal,
    HostServices, ProcessEnvHal, TimeLocaleHal, TypeLibCacheScope, TypeLibEventDispatchPath,
    TypeLibEventMetadata, TypeLibMemberMetadata, TypeLibMetadataBlob, TypeLibResolveRequest,
    TypeLibResolvedIdentity, UiInteractionHal,
};
