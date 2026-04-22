use crate::{error::HalError, model::HalProfileId, traits::ComHal};
use oxvba_com::{
    DynamicCallRequest, DynamicEventPayload, DynamicObjectBridge, DynamicObjectToken, DynamicValue,
};
use oxvba_runtime::ObjectHandle;

pub struct HalComDynamicBridge<'a> {
    _profile: HalProfileId,
    com: &'a dyn ComHal,
}

impl<'a> HalComDynamicBridge<'a> {
    pub fn new(profile: HalProfileId, com: &'a dyn ComHal) -> Self {
        Self {
            _profile: profile,
            com,
        }
    }
}

impl DynamicObjectBridge for HalComDynamicBridge<'_> {
    type Error = HalError;

    fn invoke_dynamic(&self, request: &DynamicCallRequest) -> Result<DynamicValue, Self::Error> {
        self.com
            .dispatch_invoke_dynamic_runtime_value_v2(request)
            .map(|value| oxvba_com::ComValue::from_runtime_value(&value))
    }

    fn poll_dynamic_event(&self) -> Result<Option<DynamicEventPayload>, Self::Error> {
        self.com
            .poll_event_callback()
            .map(|payload| payload.map(Into::into))
    }

    fn release_dynamic_object(
        &self,
        object: DynamicObjectToken,
    ) -> Result<DynamicValue, Self::Error> {
        self.com
            .release_object(ObjectHandle::from(object).into())
            .map(|value| oxvba_com::ComValue::from_runtime_value(&value))
    }
}
