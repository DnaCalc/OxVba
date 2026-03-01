use std::sync::Arc;

use crate::{
    adapters::standard::StandardHostServices,
    model::{HalDescriptor, HalProfileId, HostPolicy},
    traits::{
        ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, HostServices,
        ProcessEnvHal, TimeLocaleHal, UiInteractionHal,
    },
};

#[derive(Debug, Clone)]
pub struct WasmHostServices {
    inner: StandardHostServices,
}

impl WasmHostServices {
    pub fn new(policy: HostPolicy) -> Self {
        Self {
            inner: StandardHostServices::new(HalProfileId::Wasm, policy),
        }
    }

    pub fn boxed(policy: HostPolicy) -> Arc<dyn HostServices> {
        Arc::new(Self::new(policy))
    }
}

impl HostServices for WasmHostServices {
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
