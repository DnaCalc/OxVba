use crate::{
    error::HalResult,
    model::{CapabilityId, HalProfileId, HalRuntimeClass, UiVirtualizationMode},
    traits::UiInteractionHal,
};
use oxvba_runtime::RuntimeValue;

use super::StandardHostServices;

impl UiInteractionHal for StandardHostServices {
    fn msg_box(&self, prompt: RuntimeValue, style: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::UiInteraction;
        let style =
            self.runtime_value_project_compat_slot_i32(&style, capability, "msg_box", "style")?;
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
            return self
                .native_windows_msg_box_value(&prompt, style)
                .map(RuntimeValue::I32);
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Linux
            && self.runtime_class() == HalRuntimeClass::LinuxStdio
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            eprintln!(
                "[oxvba-hal] linux-stdio msg_box prompt={:?} style={style}",
                prompt
            );
            return Ok(RuntimeValue::I32(style.max(1)));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "msg_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(RuntimeValue::I32(style.max(1))),
            UiVirtualizationMode::HostCallback => {
                if let Some(cb) = &self.callbacks {
                    let prompt_text = self.runtime_value_to_display_text(&prompt);
                    Ok(RuntimeValue::I32(cb.on_msg_box(&prompt_text, style)))
                } else {
                    Ok(RuntimeValue::I32(style.max(1)))
                }
            }
            UiVirtualizationMode::Disabled => Ok(RuntimeValue::I32(
                prompt.project_compat_slot_i32().unwrap_or(1).max(1),
            )),
        }
    }

    fn input_box(
        &self,
        prompt: RuntimeValue,
        default_value: RuntimeValue,
    ) -> HalResult<RuntimeValue> {
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
                "[oxvba-hal] linux-stdio input_box prompt={:?} default={:?}",
                prompt, default_value
            );
            return Ok(default_value);
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "input_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::HostCallback => {
                if let Some(cb) = &self.callbacks {
                    let prompt_text = self.runtime_value_to_display_text(&prompt);
                    let default_text = self.runtime_value_to_display_text(&default_value);
                    let result = cb.on_input_box(&prompt_text, &default_text);
                    Ok(RuntimeValue::String(oxvba_runtime::bstr::BStr(result)))
                } else {
                    Ok(default_value)
                }
            }
            UiVirtualizationMode::Disabled => Ok(prompt),
        }
    }
}
