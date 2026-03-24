use crate::{error::HalResult, model::CapabilityId, traits::TimeLocaleHal};
use oxvba_runtime::RuntimeValue;
use std::time::{SystemTime, UNIX_EPOCH};

use super::StandardHostServices;
use super::filesystem::clamp_u64_to_i32;

impl TimeLocaleHal for StandardHostServices {
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "date_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok(RuntimeValue::I32(clamp_u64_to_i32(now.as_secs() / 86_400)));
        }
        Ok(RuntimeValue::I32(20_260_301))
    }

    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "time_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok(RuntimeValue::I32((now.as_secs() % 86_400) as i32));
        }
        Ok(RuntimeValue::I32(123_456))
    }

    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "timer_ticks"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let modulo = i32::MAX as u128;
            return Ok(RuntimeValue::I32((now.as_millis() % modulo) as i32));
        }
        Ok(RuntimeValue::I32(42))
    }
}
