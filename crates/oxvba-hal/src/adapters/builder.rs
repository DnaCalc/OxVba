//! Host builder: constructs host services from profile, policy, and optional components.

use std::sync::Arc;

use oxvba_com::PortableComProjection;

use crate::{
    callbacks::HostCallbacks,
    model::{HalProfileId, HalRuntimeClass, HostPolicy},
    traits::HostServices,
};

use super::{null, recording::RecordingHostServices, standard, wasm};

/// Builder for constructing host service instances with optional components.
pub struct HostBuilder {
    profile: HalProfileId,
    runtime_class: Option<HalRuntimeClass>,
    policy: HostPolicy,
    callbacks: Option<Arc<dyn HostCallbacks>>,
    portable_objects: Option<Arc<PortableComProjection>>,
    recording: bool,
}

impl HostBuilder {
    pub fn new() -> Self {
        Self {
            profile: HalProfileId::Null,
            runtime_class: None,
            policy: HostPolicy::default(),
            callbacks: None,
            portable_objects: None,
            recording: false,
        }
    }

    pub fn profile(mut self, profile: HalProfileId) -> Self {
        self.profile = profile;
        self
    }

    pub fn runtime_class(mut self, runtime_class: HalRuntimeClass) -> Self {
        self.runtime_class = Some(runtime_class);
        self
    }

    pub fn policy(mut self, policy: HostPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn callbacks(mut self, callbacks: Arc<dyn HostCallbacks>) -> Self {
        self.callbacks = Some(callbacks);
        self
    }

    /// Set a portable COM projection for host-registered objects.
    pub fn portable_objects(mut self, projection: Arc<PortableComProjection>) -> Self {
        self.portable_objects = Some(projection);
        self
    }

    /// Enable recording of HAL interactions into a journal.
    pub fn recording(mut self, enable: bool) -> Self {
        self.recording = enable;
        self
    }

    pub fn build(self) -> Arc<dyn HostServices> {
        let runtime_class = self.runtime_class.unwrap_or_else(|| {
            HalRuntimeClass::default_for(self.profile, self.policy.wasm_runtime_class)
        });

        let host: Arc<dyn HostServices> = match self.profile {
            HalProfileId::Windows | HalProfileId::Linux | HalProfileId::MacOs => {
                let mut host = standard::StandardHostServices::new_with_runtime_class(
                    self.profile,
                    runtime_class,
                    self.policy,
                );
                if let Some(cb) = self.callbacks {
                    host = host.with_callbacks(cb);
                }
                if let Some(projection) = self.portable_objects {
                    host = host.with_portable_objects(projection);
                }
                Arc::new(host)
            }
            HalProfileId::Wasm => wasm::WasmHostServices::boxed(self.policy),
            HalProfileId::Null => null::NullHostServices::boxed(self.policy),
        };

        if self.recording {
            Arc::new(RecordingHostServices::new(host))
        } else {
            host
        }
    }
}

impl Default for HostBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callbacks::DefaultHostCallbacks;
    use crate::model::HostPolicy;
    use crate::traits::HostServices;

    #[test]
    fn builder_creates_windows_host() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .policy(HostPolicy::deterministic_runtime())
            .build();
        assert_eq!(host.profile(), HalProfileId::Windows);
    }

    #[test]
    fn builder_creates_null_host() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Null)
            .policy(HostPolicy::strict_ci())
            .build();
        assert_eq!(host.profile(), HalProfileId::Null);
    }

    #[test]
    fn builder_accepts_callbacks() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .policy(HostPolicy::deterministic_runtime())
            .callbacks(Arc::new(DefaultHostCallbacks))
            .build();
        assert_eq!(host.profile(), HalProfileId::Windows);
    }

    #[test]
    fn builder_with_explicit_runtime_class() {
        let host = HostBuilder::new()
            .profile(HalProfileId::Windows)
            .runtime_class(HalRuntimeClass::WindowsHeadless)
            .policy(HostPolicy::deterministic_runtime())
            .build();
        assert_eq!(host.descriptor().runtime_class, "windows-headless");
    }
}
