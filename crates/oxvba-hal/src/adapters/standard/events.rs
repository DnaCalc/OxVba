use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::EventPumpHal,
};
use oxvba_runtime::RuntimeValue;
use std::thread;

use super::StandardHostServices;

impl EventPumpHal for StandardHostServices {
    fn do_events(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::EventPump;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "do_events"));
        }
        if self.native_mode_enabled() {
            if self.profile == crate::model::HalProfileId::Windows {
                self.pump_windows_messages_once();
            }
            thread::yield_now();
        }
        #[cfg(target_os = "windows")]
        if self.native_com_enabled() {
            let callback = self
                .com_bridge
                .mark_next_callback_pumped()
                .map_err(|message| {
                    HalError::adapter_fault(self.profile, capability, "do_events", message)
                })?;
            if let Some(callback) = callback {
                return Ok(RuntimeValue::I32(callback.into()));
            }
        }
        Ok(RuntimeValue::I32(0))
    }
}
