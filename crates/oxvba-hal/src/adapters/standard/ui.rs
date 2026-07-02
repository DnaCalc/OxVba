use crate::{
    error::{HalError, HalResult},
    model::{CapabilityId, HalProfileId, HalRuntimeClass, UiVirtualizationMode},
    traits::UiInteractionHal,
};
use oxvba_runtime::{VarType, Variant};

use super::StandardHostServices;

impl UiInteractionHal for StandardHostServices {
    fn msg_box_variant(&self, prompt: Variant, style: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::UiInteraction;
        let style = self.variant_to_i32(&style, capability, "msg_box", "style")?;
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
                self.variant_to_i32(&prompt, capability, "msg_box", "prompt")
                    .unwrap_or(1)
                    .max(1),
            )),
        }
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

    fn send_keys_variant(&self, keys: Variant, _wait: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::UiInteraction;
        let op = "send_keys";
        if !self.supports(capability) {
            return Err(self.unsupported(capability, op));
        }
        let key_text = self.variant_to_display_text(&keys);
        if key_text.is_empty() {
            return Ok(Variant::empty());
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(capability, op));
        }
        Err(HalError::unsupported_profile(self.profile, capability, op).with_host_error_code(5))
    }

    fn app_activate_variant(&self, title: Variant, _wait: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::UiInteraction;
        let op = "app_activate";
        if !self.supports(capability) {
            return Err(self.unsupported(capability, op));
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Windows
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            if !self.policy.allow_interaction {
                return Err(self.denied(capability, op));
            }
            if self.native_windows_app_activate(&title)? {
                return Ok(Variant::empty());
            }
            return Err(app_activate_not_found(self.profile));
        }
        if matches!(title.vtype(), VarType::Empty | VarType::Null) {
            return Err(app_activate_not_found(self.profile));
        }
        Err(app_activate_not_found(self.profile))
    }
}

fn app_activate_not_found(profile: HalProfileId) -> HalError {
    HalError::adapter_fault(
        profile,
        CapabilityId::UiInteraction,
        "app_activate",
        "Invalid procedure call or argument",
    )
    .with_host_error_code(5)
}
