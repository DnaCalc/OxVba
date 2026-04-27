use super::filesystem::{expand_host_wildcard_paths, path_contains_wildcards};
use crate::{
    compat,
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::ProcessEnvHal,
};
use oxvba_runtime::{RuntimeValue, VarType, Variant, bstr::BStr};
use std::path::{Path, PathBuf};

use super::StandardHostServices;

#[derive(Debug, Clone, Default)]
pub(super) struct DirSearchState {
    pub(super) remaining: Vec<String>,
}

impl ProcessEnvHal for StandardHostServices {
    // Legacy compatibility wrapper. The retained value-model entry point for
    // VM/JIT callers is `shell_variant`; this method preserves the historical
    // `RuntimeValue` contract, including compatibility slot-token projection.
    fn shell(&self, command: RuntimeValue, _window_style: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        let command = compat::runtime_value_to_variant(
            self.profile,
            capability,
            "shell",
            "command",
            command,
        )?;
        let result = self.shell_variant(command, Variant::empty())?;
        compat::variant_to_runtime_value(self.profile, capability, "shell", result)
    }

    // Legacy compatibility wrapper. The retained value-model entry point for
    // VM/JIT callers is `environ_variant`; this method preserves the historical
    // `RuntimeValue` contract, including compatibility slot-token projection.
    fn environ(&self, key: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        let key =
            compat::runtime_value_to_variant(self.profile, capability, "environ", "key", key)?;
        let result = self.environ_variant(key)?;
        compat::variant_to_runtime_value(self.profile, capability, "environ", result)
    }

    // Legacy compatibility wrapper. The retained value-model entry point for
    // VM/JIT callers is `dir_variant`; this method preserves the historical
    // `RuntimeValue` contract, including compatibility slot-token projection.
    fn dir(&self, path: RuntimeValue, _attrs: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ProcessEnv;
        let path = compat::runtime_value_to_variant(self.profile, capability, "dir", "path", path)?;
        let result = self.dir_variant(path, Variant::empty())?;
        compat::variant_to_runtime_value(self.profile, capability, "dir", result)
    }

    fn shell_variant(&self, command: Variant, _window_style: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "shell"));
        }
        if !self.policy.allow_process_spawn {
            return Err(self.denied(capability, "shell"));
        }
        if self.native_process_enabled()
            && let Some(text) = command.as_bstr()
            && !text.as_str().trim().is_empty()
        {
            let command_text = text.as_str();
            let mut child = self
                .spawn_probe_shell_process_text(&command_text)
                .map_err(|err| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "shell",
                        format!("failed to spawn probe shell process: {err}"),
                    )
                })?;
            let child_id = i32::try_from(child.id()).unwrap_or(i32::MAX).max(1);
            let _ = child.wait();
            return Ok(Variant::from_i32(child_id));
        }
        if self.native_process_enabled() {
            let command = self
                .variant_project_compat_slot_i32(&command, capability, "shell", "command")
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
                return Ok(Variant::from_i32(child_id));
            }
        }
        let command = match command.as_bstr() {
            Some(text) => i32::from(!text.as_str().trim().is_empty()),
            None => self
                .variant_project_compat_slot_i32(&command, capability, "shell", "command")
                .unwrap_or(0),
        };
        Ok(Variant::from_compat_slot_i32(if command == 0 {
            0
        } else {
            1
        }))
    }

    fn environ_variant(&self, key: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "environ"));
        }
        if self.native_process_enabled()
            && let Some(name) = key.as_bstr()
        {
            let value = std::env::var_os(name.as_str())
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();
            return Ok(Variant::from_string(value));
        }
        if self.native_process_enabled() {
            let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> =
                std::env::vars_os().collect();
            if vars.is_empty() {
                return Ok(Variant::from_string(BStr::empty()));
            }
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            let key = self
                .variant_project_compat_slot_i32(&key, capability, "environ", "key")
                .unwrap_or(0);
            let idx = (key.unsigned_abs() as usize) % vars.len();
            let entry = format!(
                "{}={}",
                vars[idx].0.to_string_lossy(),
                vars[idx].1.to_string_lossy()
            );
            return Ok(Variant::from_string(entry));
        }
        let key = match key.as_bstr() {
            Some(text) => text.as_str().len().min(i32::MAX as usize) as i32,
            None => self
                .variant_project_compat_slot_i32(&key, capability, "environ", "key")
                .unwrap_or(0),
        };
        Ok(Variant::from_compat_slot_i32(key))
    }

    fn dir_variant(&self, path: Variant, _attrs: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dir"));
        }
        if self.native_process_enabled() {
            let is_continuation = matches!(path.vtype(), VarType::Empty | VarType::Null)
                || path.project_compat_slot_i32().unwrap_or(1) == 0;
            if is_continuation {
                let mut state = self.dir_lock(capability, "dir")?;
                let next = if state.remaining.is_empty() {
                    String::new()
                } else {
                    state.remaining.remove(0)
                };
                return Ok(Variant::from_string(next));
            }

            let target = self.variant_to_path(&path, capability, "dir", "path")?;
            let matches = enumerate_dir_matches(&target).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "dir",
                    format!("failed to enumerate dir path {}: {err}", target.display()),
                )
            })?;
            let mut names = matches
                .into_iter()
                .map(|entry| {
                    entry
                        .file_name()
                        .map(|value| value.to_string_lossy().to_string())
                        .unwrap_or_else(|| entry.display().to_string())
                })
                .collect::<Vec<_>>();
            let first = names.first().cloned().unwrap_or_default();
            let mut state = self.dir_lock(capability, "dir")?;
            state.remaining = if names.len() > 1 {
                names.drain(1..).collect()
            } else {
                Vec::new()
            };
            return Ok(Variant::from_string(first));
        }
        let out = match path.vtype() {
            VarType::Empty | VarType::Null => 0,
            VarType::String => {
                i32::from(path.as_bstr().map(|text| !text.is_empty()).unwrap_or(false))
            }
            _ => i32::from(
                self.variant_project_compat_slot_i32(&path, capability, "dir", "path")
                    .unwrap_or(0)
                    != 0,
            ),
        };
        Ok(Variant::from_i32(out))
    }
}

fn enumerate_dir_matches(target: &Path) -> std::io::Result<Vec<PathBuf>> {
    if path_contains_wildcards(target) {
        return expand_host_wildcard_paths(target);
    }
    if target.exists() {
        return Ok(vec![target.to_path_buf()]);
    }
    Ok(Vec::new())
}
