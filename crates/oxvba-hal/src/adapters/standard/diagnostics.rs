use crate::{error::HalResult, model::CapabilityId, traits::DiagnosticsHal};
use oxvba_runtime::RuntimeValue;

use super::StandardHostServices;

impl DiagnosticsHal for StandardHostServices {
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "emit"));
        }
        let code = self.runtime_value_to_legacy_i32(&code, capability, "emit", "code")?;
        let payload = self.runtime_value_to_legacy_i32(&payload, capability, "emit", "payload")?;
        if self.native_diagnostics_enabled() {
            eprintln!(
                "[oxvba-hal] profile={:?} code={} payload={}",
                self.profile, code, payload
            );
        }
        Ok(RuntimeValue::I32(code.saturating_add(payload)))
    }
}
