pub mod linux;
pub mod macos;
pub mod null;
mod standard;
pub mod wasm;
pub mod windows;

use std::sync::Arc;

use crate::{
    model::{HalProfileId, HostPolicy},
    traits::HostServices,
};

pub fn for_profile(profile: HalProfileId, policy: HostPolicy) -> Arc<dyn HostServices> {
    match profile {
        HalProfileId::Windows => windows::WindowsHostServices::boxed(policy),
        HalProfileId::Linux => linux::LinuxHostServices::boxed(policy),
        HalProfileId::MacOs => macos::MacOsHostServices::boxed(policy),
        HalProfileId::Wasm => wasm::WasmHostServices::boxed(policy),
        HalProfileId::Null => null::NullHostServices::boxed(policy),
    }
}
