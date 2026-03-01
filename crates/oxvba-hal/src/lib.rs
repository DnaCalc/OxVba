//! oxvba-hal: host abstraction contracts, profile adapters, and conformance harness.

pub mod adapters;
pub mod conformance;
pub mod error;
pub mod model;
pub mod traits;

pub use error::{HalError, HalErrorKind, HalResult};
pub use model::{
    CapabilityDescriptor, CapabilityId, CapabilityMaturity, HalDescriptor, HalProfileId,
    HostPolicy, UiVirtualizationMode, UnsupportedFeatureMode,
};
pub use traits::{
    ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, HostServices,
    ProcessEnvHal, TimeLocaleHal, UiInteractionHal,
};
