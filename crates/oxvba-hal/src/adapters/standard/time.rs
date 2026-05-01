use crate::{error::HalResult, model::CapabilityId, traits::TimeLocaleHal};
use oxvba_runtime::{Variant, compat::RuntimeValue};
use std::time::{SystemTime, UNIX_EPOCH};

use super::StandardHostServices;

const DETERMINISTIC_DATE_SERIAL: f64 = 46_082.0;
const DETERMINISTIC_SECONDS: f64 = 45_296.0;

fn deterministic_date_variant() -> Variant {
    Variant::from_date_f64(DETERMINISTIC_DATE_SERIAL)
}

fn deterministic_time_variant() -> Variant {
    Variant::from_date_f64(DETERMINISTIC_SECONDS / 86_400.0)
}

fn deterministic_timer_variant() -> Variant {
    Variant::from_f32(DETERMINISTIC_SECONDS as f32)
}

fn system_time_components() -> std::time::Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}

impl TimeLocaleHal for StandardHostServices {
    // Legacy time path. Retained VM/JIT callers should use
    // `date_serial_now_variant`, which returns a Variant Date carrier directly.
    fn date_serial_now(&self) -> HalResult<RuntimeValue> {
        self.date_serial_now_variant()?
            .to_runtime_value()
            .map_err(|detail| self.time_adapter_fault("date_serial_now", detail))
    }

    // Legacy time path. Retained VM/JIT callers should use
    // `time_serial_now_variant`, which returns a Variant Date carrier directly.
    fn time_serial_now(&self) -> HalResult<RuntimeValue> {
        self.time_serial_now_variant()?
            .to_runtime_value()
            .map_err(|detail| self.time_adapter_fault("time_serial_now", detail))
    }

    // Legacy time path. Retained VM/JIT callers should use
    // `timer_ticks_variant`, which returns a Variant Single carrier directly.
    fn timer_ticks(&self) -> HalResult<RuntimeValue> {
        self.timer_ticks_variant()?
            .to_runtime_value()
            .map_err(|detail| self.time_adapter_fault("timer_ticks", detail))
    }

    fn date_serial_now_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "date_serial_now"));
        }
        if self.native_time_enabled() {
            let now = system_time_components();
            let serial = (now.as_secs() / 86_400) as f64 + 25_569.0;
            return Ok(Variant::from_date_f64(serial.floor()));
        }
        Ok(deterministic_date_variant())
    }

    fn time_serial_now_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "time_serial_now"));
        }
        if self.native_time_enabled() {
            let now = system_time_components();
            let secs_today =
                (now.as_secs() % 86_400) as f64 + f64::from(now.subsec_nanos()) / 1_000_000_000.0;
            return Ok(Variant::from_date_f64(secs_today / 86_400.0));
        }
        Ok(deterministic_time_variant())
    }

    fn timer_ticks_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "timer_ticks"));
        }
        if self.native_time_enabled() {
            let now = system_time_components();
            let secs_today =
                (now.as_secs() % 86_400) as f64 + f64::from(now.subsec_nanos()) / 1_000_000_000.0;
            return Ok(Variant::from_f32(secs_today as f32));
        }
        Ok(deterministic_timer_variant())
    }
}

impl StandardHostServices {
    fn time_adapter_fault(
        &self,
        operation: &'static str,
        detail: String,
    ) -> crate::error::HalError {
        crate::error::HalError::adapter_fault(
            self.profile,
            CapabilityId::TimeLocale,
            operation,
            detail,
        )
    }
}
