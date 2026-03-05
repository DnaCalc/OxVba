use std::sync::Arc;

use crate::{
    adapters::standard::StandardHostServices,
    model::{HalDescriptor, HalProfileId, HalRuntimeClass, HostPolicy},
    traits::{
        ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, HostServices,
        ProcessEnvHal, TimeLocaleHal, TypeLibraryHal, UiInteractionHal,
    },
};

#[derive(Debug, Clone)]
pub struct WindowsHostServices {
    inner: StandardHostServices,
}

impl WindowsHostServices {
    pub fn new(policy: HostPolicy) -> Self {
        Self {
            inner: StandardHostServices::new(HalProfileId::Windows, policy),
        }
    }

    pub fn new_with_runtime_class(policy: HostPolicy, runtime_class: HalRuntimeClass) -> Self {
        Self {
            inner: StandardHostServices::new_with_runtime_class(
                HalProfileId::Windows,
                runtime_class,
                policy,
            ),
        }
    }

    pub fn boxed(policy: HostPolicy) -> Arc<dyn HostServices> {
        Arc::new(Self::new(policy))
    }

    pub fn boxed_with_runtime_class(
        policy: HostPolicy,
        runtime_class: HalRuntimeClass,
    ) -> Arc<dyn HostServices> {
        Arc::new(Self::new_with_runtime_class(policy, runtime_class))
    }
}

impl HostServices for WindowsHostServices {
    fn profile(&self) -> HalProfileId {
        self.inner.profile()
    }

    fn descriptor(&self) -> HalDescriptor {
        self.inner.descriptor()
    }

    fn policy(&self) -> &HostPolicy {
        self.inner.policy()
    }

    fn ui(&self) -> &dyn UiInteractionHal {
        &self.inner
    }
    fn events(&self) -> &dyn EventPumpHal {
        &self.inner
    }
    fn fs(&self) -> &dyn FileSystemHal {
        &self.inner
    }
    fn process(&self) -> &dyn ProcessEnvHal {
        &self.inner
    }
    fn com(&self) -> &dyn ComHal {
        &self.inner
    }
    fn typelibs(&self) -> &dyn TypeLibraryHal {
        &self.inner
    }
    fn time_locale(&self) -> &dyn TimeLocaleHal {
        &self.inner
    }
    fn dynlink(&self) -> &dyn DynamicLinkHal {
        &self.inner
    }
    fn diag(&self) -> &dyn DiagnosticsHal {
        &self.inner
    }
}
