use crate::{error::HalResult, model::CapabilityId, traits::DiagnosticsHal};
use oxvba_runtime::Variant;

use super::StandardHostServices;

impl DiagnosticsHal for StandardHostServices {
    fn emit_variant(&self, code: Variant, payload: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "emit"));
        }
        let code = self.variant_to_i32(&code, capability, "emit", "code")?;
        let payload = self.variant_to_i32(&payload, capability, "emit", "payload")?;
        if self.native_diagnostics_enabled() {
            eprintln!(
                "[oxvba-hal] profile={:?} code={} payload={}",
                self.profile, code, payload
            );
        }
        Ok(Variant::from_i32(code.saturating_add(payload)))
    }

    fn debug_print_variant(&self, text: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "debug_print"));
        }
        let text = self.variant_to_display_text(&text);
        if let Some(callbacks) = self.callbacks.as_ref() {
            callbacks.on_debug_print(&text);
            return Ok(Variant::from_i32(0));
        }
        eprintln!("{text}");
        Ok(Variant::from_i32(0))
    }
}
