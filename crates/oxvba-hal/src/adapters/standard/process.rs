use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::ProcessEnvHal,
};
use oxvba_runtime::{RuntimeValue, bstr::BStr};
use std::fs;

use super::StandardHostServices;

impl ProcessEnvHal for StandardHostServices {
    fn shell(&self, command: RuntimeValue, _window_style: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "shell"));
        }
        if !self.policy.allow_process_spawn {
            return Err(self.denied(capability, "shell"));
        }
        if self.native_process_enabled()
            && let RuntimeValue::String(BStr(text)) = &command
            && !text.trim().is_empty()
        {
            let mut child = self.spawn_probe_shell_process_text(text).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "shell",
                    format!("failed to spawn probe shell process: {err}"),
                )
            })?;
            let child_id = i32::try_from(child.id()).unwrap_or(i32::MAX).max(1);
            let _ = child.wait();
            return Ok(RuntimeValue::I32(child_id));
        }
        if self.native_process_enabled() {
            let command = self
                .runtime_value_to_legacy_i32(&command, capability, "shell", "command")
                .unwrap_or(0);
            if command != 0 {
                let mut child = self.spawn_probe_shell_process(command).map_err(|err| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "shell",
                        format!("failed to spawn probe shell process: {err}"),
                    )
                })?;
                let child_id = i32::try_from(child.id()).unwrap_or(i32::MAX).max(1);
                let _ = child.wait();
                return Ok(RuntimeValue::I32(child_id));
            }
        }
        let command = match &command {
            RuntimeValue::String(BStr(text)) => i32::from(!text.trim().is_empty()),
            other => self
                .runtime_value_to_legacy_i32(other, capability, "shell", "command")
                .unwrap_or(0),
        };
        Ok(RuntimeValue::from_legacy_i32(if command == 0 {
            0
        } else {
            1
        }))
    }

    fn environ(&self, key: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "environ"));
        }
        if self.native_process_enabled()
            && let RuntimeValue::String(BStr(name)) = &key
        {
            let value_len = std::env::var_os(name)
                .map(|value| value.to_string_lossy().len())
                .unwrap_or(0);
            return Ok(RuntimeValue::I32(value_len.min(i32::MAX as usize) as i32));
        }
        if self.native_process_enabled() {
            let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> =
                std::env::vars_os().collect();
            if vars.is_empty() {
                return Ok(RuntimeValue::I32(0));
            }
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            let key = self
                .runtime_value_to_legacy_i32(&key, capability, "environ", "key")
                .unwrap_or(0);
            let idx = (key.unsigned_abs() as usize) % vars.len();
            let value_len = vars[idx].1.to_string_lossy().len();
            return Ok(RuntimeValue::I32(value_len.min(i32::MAX as usize) as i32));
        }
        let key = match &key {
            RuntimeValue::String(BStr(text)) => text.len().min(i32::MAX as usize) as i32,
            other => self
                .runtime_value_to_legacy_i32(other, capability, "environ", "key")
                .unwrap_or(0),
        };
        Ok(RuntimeValue::from_legacy_i32(key))
    }

    fn dir(&self, path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dir"));
        }
        if self.native_process_enabled() {
            let target = match &path {
                RuntimeValue::Empty | RuntimeValue::Null | RuntimeValue::I32(0) => {
                    std::env::current_dir().map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "dir",
                            format!("failed to get current directory: {err}"),
                        )
                    })?
                }
                _ => self.runtime_value_to_path(&path, capability, "dir", "path")?,
            };
            let out = match fs::read_dir(target) {
                Ok(mut entries) => i32::from(entries.next().is_some()),
                Err(_) => 0,
            };
            return Ok(RuntimeValue::I32(out));
        }
        let out = match &path {
            RuntimeValue::Empty | RuntimeValue::Null | RuntimeValue::I32(0) => 0,
            RuntimeValue::String(BStr(text)) => i32::from(!text.is_empty()),
            other => i32::from(
                self.runtime_value_to_legacy_i32(other, capability, "dir", "path")
                    .unwrap_or(0)
                    != 0,
            ),
        };
        Ok(RuntimeValue::I32(out))
    }
}
