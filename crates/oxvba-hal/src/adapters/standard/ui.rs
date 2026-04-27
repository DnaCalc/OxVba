use crate::{
    compat,
    error::HalResult,
    model::{CapabilityId, HalProfileId, HalRuntimeClass, UiVirtualizationMode},
    traits::UiInteractionHal,
};
use oxvba_runtime::{RuntimeValue, Variant};

use super::StandardHostServices;

impl UiInteractionHal for StandardHostServices {
    // Legacy UI path. Retained VM/JIT callers should use `msg_box_variant`,
    // which preserves the prompt/style as Variant carriers.
    fn msg_box(&self, prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::UiInteraction;
        let prompt = compat::runtime_value_to_variant(
            self.profile,
            capability,
            "msg_box",
            "prompt",
            prompt,
        )?;
        let style =
            compat::runtime_value_to_variant(self.profile, capability, "msg_box", "style", style)?;
        let result = self.msg_box_variant(prompt, style)?;
        compat::variant_to_runtime_value(self.profile, capability, "msg_box", result)
    }

    fn msg_box_variant(&self, prompt: Variant, style: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::UiInteraction;
        let style = self.variant_project_compat_slot_i32(&style, capability, "msg_box", "style")?;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "msg_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(capability, "msg_box"));
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Windows
            && self.runtime_class() == HalRuntimeClass::WindowsGui
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            let prompt_text = self.variant_to_display_text(&prompt);
            return self
                .native_windows_msg_box_text(&prompt_text, style)
                .map(Variant::from_i32);
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Linux
            && self.runtime_class() == HalRuntimeClass::LinuxStdio
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            eprintln!(
                "[oxvba-hal] linux-stdio msg_box prompt={} style={style}",
                self.variant_to_display_text(&prompt)
            );
            return Ok(Variant::from_i32(style.max(1)));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "msg_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(Variant::from_i32(style.max(1))),
            UiVirtualizationMode::HostCallback => {
                if let Some(cb) = &self.callbacks {
                    let prompt_text = self.variant_to_display_text(&prompt);
                    Ok(Variant::from_i32(cb.on_msg_box(&prompt_text, style)))
                } else {
                    Ok(Variant::from_i32(style.max(1)))
                }
            }
            UiVirtualizationMode::Disabled => Ok(Variant::from_i32(
                prompt.project_compat_slot_i32().unwrap_or(1).max(1),
            )),
        }
    }

    fn input_box(
        &self,
        prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
        // Legacy UI path. Retained VM/JIT callers should use
        // `input_box_variant`, which preserves prompt/default values as
        // Variant carriers.
        let capability = CapabilityId::UiInteraction;
        let prompt = compat::runtime_value_to_variant(
            self.profile,
            capability,
            "input_box",
            "prompt",
            prompt,
        )?;
        let default_value = compat::runtime_value_to_variant(
            self.profile,
            capability,
            "input_box",
            "default_value",
            default_value,
        )?;
        let result = self.input_box_variant(prompt, default_value)?;
        compat::variant_to_runtime_value(self.profile, capability, "input_box", result)
    }

    fn input_box_variant(&self, prompt: Variant, default_value: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::UiInteraction;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(capability, "input_box"));
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Linux
            && self.runtime_class() == HalRuntimeClass::LinuxStdio
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            eprintln!(
                "[oxvba-hal] linux-stdio input_box prompt={} default={}",
                self.variant_to_display_text(&prompt),
                self.variant_to_display_text(&default_value)
            );
            return Ok(default_value);
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "input_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::HostCallback => {
                if let Some(cb) = &self.callbacks {
                    let prompt_text = self.variant_to_display_text(&prompt);
                    let default_text = self.variant_to_display_text(&default_value);
                    let result = cb.on_input_box(&prompt_text, &default_text);
                    Ok(Variant::from_string(result))
                } else {
                    Ok(default_value)
                }
            }
            UiVirtualizationMode::Disabled => Ok(prompt),
        }
    }
}
