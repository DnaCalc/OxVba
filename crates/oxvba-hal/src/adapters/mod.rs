pub mod linux;
pub mod macos;
pub mod null;
mod standard;
pub mod wasm;
pub mod windows;

use std::sync::Arc;

use crate::{
    model::{HalProfileId, HalRuntimeClass, HostPolicy},
    traits::HostServices,
};

pub fn for_profile(profile: HalProfileId, policy: HostPolicy) -> Arc<dyn HostServices> {
    let runtime_class = policy
        .runtime_class
        .unwrap_or_else(|| HalRuntimeClass::default_for(profile, policy.wasm_runtime_class));
    for_profile_with_runtime_class(profile, runtime_class, policy)
}

pub fn for_profile_with_runtime_class(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    policy: HostPolicy,
) -> Arc<dyn HostServices> {
    match profile {
        HalProfileId::Windows => {
            windows::WindowsHostServices::boxed_with_runtime_class(policy, runtime_class)
        }
        HalProfileId::Linux => {
            linux::LinuxHostServices::boxed_with_runtime_class(policy, runtime_class)
        }
        HalProfileId::MacOs => {
            macos::MacOsHostServices::boxed_with_runtime_class(policy, runtime_class)
        }
        HalProfileId::Wasm => wasm::WasmHostServices::boxed(policy),
        HalProfileId::Null => null::NullHostServices::boxed(policy),
    }
}
