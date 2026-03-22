pub mod builder;
pub mod linux;
pub mod macos;
pub mod null;
pub mod recording;
pub mod replay;
pub(crate) mod standard;
pub mod wasm;
pub mod windows;

use std::sync::Arc;

use crate::{
    model::{HalProfileId, HalRuntimeClass, HostPolicy},
    traits::HostServices,
};

use builder::HostBuilder;

/// Create host services for the given profile and policy.
///
/// Delegates through [`HostBuilder`] internally.
#[deprecated(note = "use HostBuilder instead")]
pub fn for_profile(profile: HalProfileId, policy: HostPolicy) -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(profile)
        .policy(policy)
        .build()
}

/// Create host services for the given profile, runtime class, and policy.
///
/// Delegates through [`HostBuilder`] internally.
#[deprecated(note = "use HostBuilder instead")]
pub fn for_profile_with_runtime_class(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    policy: HostPolicy,
) -> Arc<dyn HostServices> {
    HostBuilder::new()
        .profile(profile)
        .runtime_class(runtime_class)
        .policy(policy)
        .build()
}
