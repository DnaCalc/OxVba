use crate::{compat, error::HalResult, model::CapabilityId, traits::DiagnosticsHal};
use oxvba_runtime::{RuntimeValue, Variant};

use super::StandardHostServices;

impl DiagnosticsHal for StandardHostServices {
    // Legacy diagnostics telemetry path. Retained VM/JIT callers should use
    // `emit_variant`, which avoids `RuntimeValue` projection.
    fn emit(&self, code: RuntimeValue, payload: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        let code =
            compat::runtime_value_to_variant(self.profile, capability, "emit", "code", code)?;
        let payload =
            compat::runtime_value_to_variant(self.profile, capability, "emit", "payload", payload)?;
        let result = self.emit_variant(code, payload)?;
        compat::variant_to_runtime_value(self.profile, capability, "emit", result)
    }

    // Legacy debug-print path. Retained VM/JIT callers should use
    // `debug_print_variant`, which avoids `RuntimeValue` projection.
    fn debug_print(&self, text: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        let text = compat::runtime_value_to_variant(
            self.profile,
            capability,
            "debug_print",
            "text",
            text,
        )?;
        let result = self.debug_print_variant(text)?;
        compat::variant_to_runtime_value(self.profile, capability, "debug_print", result)
    }

    fn emit_variant(&self, code: Variant, payload: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "emit"));
        }
        let code = self.variant_project_compat_slot_i32(&code, capability, "emit", "code")?;
        let payload =
            self.variant_project_compat_slot_i32(&payload, capability, "emit", "payload")?;
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
