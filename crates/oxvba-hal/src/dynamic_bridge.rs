use crate::{
    error::HalError,
    model::{CapabilityId, HalProfileId},
    traits::ComHal,
};
use oxvba_com::{
    DynamicCallRequest, DynamicEventPayload, DynamicObjectBridge, DynamicObjectToken, DynamicValue,
};

pub struct HalComDynamicBridge<'a> {
    profile: HalProfileId,
    com: &'a dyn ComHal,
}

impl<'a> HalComDynamicBridge<'a> {
    pub fn new(profile: HalProfileId, com: &'a dyn ComHal) -> Self {
        Self { profile, com }
    }
}

impl DynamicObjectBridge for HalComDynamicBridge<'_> {
    type Error = HalError;

    fn invoke_dynamic(&self, request: &DynamicCallRequest) -> Result<DynamicValue, Self::Error> {
        let request = request.try_into_com_invoke_request().map_err(|detail| {
            HalError::adapter_fault(
                self.profile,
                CapabilityId::ComActivationDispatch,
                "dispatch_invoke",
                format!("dynamic call request cannot lower to COM invoke: {detail}"),
            )
        })?;
        self.com
            .dispatch_invoke_runtime_value_v2(&request)
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
            .release_object(object.into())
            .map(|value| oxvba_com::ComValue::from_runtime_value(&value))
    }
}
