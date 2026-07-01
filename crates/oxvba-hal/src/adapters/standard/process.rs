use super::filesystem::{expand_host_wildcard_paths, path_contains_wildcards};
use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::ProcessEnvHal,
};
use oxvba_runtime::{
    VarType, Variant,
    bstr::BStr,
    safe_array::{SafeArray, SafeArrayBound, VT_BSTR_VALUE},
    variant_to_vba_string,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use super::StandardHostServices;

#[derive(Debug, Clone, Default)]
pub(super) struct DirSearchState {
    pub(super) remaining: Vec<String>,
}

#[derive(Debug, Clone)]
struct SettingEntry {
    key_name: String,
    value: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct SettingsState {
    values: BTreeMap<(String, String, String), SettingEntry>,
}

impl ProcessEnvHal for StandardHostServices {
    fn command_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "command"));
        }
        if self.native_process_enabled() && !self.policy.deterministic_mode {
            let args = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
            return Ok(Variant::from_string(args));
        }
        Ok(Variant::from_string(BStr::empty()))
    }

    fn get_setting_variant(
        &self,
        appname: Variant,
        section: Variant,
        key: Variant,
        default: Variant,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        let op = "get_setting";
        if !self.supports(capability) {
            return Err(self.unsupported(capability, op));
        }
        let appname = self.setting_text(&appname, capability, op, "appname")?;
        let section = self.setting_text(&section, capability, op, "section")?;
        let key = self.setting_text(&key, capability, op, "key")?;
        let default = self.setting_text(&default, capability, op, "default")?;
        let state = self.settings_lock(capability, op)?;
        let value = state
            .values
            .get(&settings_key(&appname, &section, &key))
            .map(|entry| entry.value.clone())
            .unwrap_or(default);
        Ok(Variant::from_string(value))
    }

    fn get_all_settings_variant(&self, appname: Variant, section: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        let op = "get_all_settings";
        if !self.supports(capability) {
            return Err(self.unsupported(capability, op));
        }
        let appname = self.setting_text(&appname, capability, op, "appname")?;
        let section = self.setting_text(&section, capability, op, "section")?;
        let state = self.settings_lock(capability, op)?;
        let prefix = (
            normalize_setting_name(&appname),
            normalize_setting_name(&section),
        );
        let entries: Vec<SettingEntry> = state
            .values
            .iter()
            .filter(|((app, sec, _), _)| app == &prefix.0 && sec == &prefix.1)
            .map(|(_, entry)| entry.clone())
            .collect();
        if entries.is_empty() {
            return Ok(Variant::empty());
        }

        let mut values = Vec::with_capacity(entries.len() * 2);
        for entry in entries {
            values.push(Variant::from_string(entry.key_name));
            values.push(Variant::from_string(entry.value));
        }
        let bounds = vec![
            SafeArrayBound {
                lower: 0,
                count: u32::try_from(values.len() / 2).map_err(|_| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        op,
                        "settings array row count exceeds SAFEARRAY bounds",
                    )
                })?,
            },
            SafeArrayBound { lower: 0, count: 2 },
        ];
        let array = SafeArray::from_typed_variants_nd(bounds, VT_BSTR_VALUE, values)
            .map_err(|message| HalError::adapter_fault(self.profile, capability, op, message))?;
        Ok(Variant::from_safearray(array))
    }

    fn save_setting_variant(
        &self,
        appname: Variant,
        section: Variant,
        key: Variant,
        setting: Variant,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        let op = "save_setting";
        if !self.supports(capability) {
            return Err(self.unsupported(capability, op));
        }
        let appname = self.setting_text(&appname, capability, op, "appname")?;
        let section = self.setting_text(&section, capability, op, "section")?;
        let key_name = self.setting_text(&key, capability, op, "key")?;
        let value = self.setting_text(&setting, capability, op, "setting")?;
        let mut state = self.settings_lock(capability, op)?;
        state.values.insert(
            settings_key(&appname, &section, &key_name),
            SettingEntry { key_name, value },
        );
        Ok(Variant::empty())
    }

    fn delete_setting_variant(
        &self,
        appname: Variant,
        section: Variant,
        key: Variant,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        let op = "delete_setting";
        if !self.supports(capability) {
            return Err(self.unsupported(capability, op));
        }
        let appname = self.setting_text(&appname, capability, op, "appname")?;
        let section = self.setting_text(&section, capability, op, "section")?;
        let mut state = self.settings_lock(capability, op)?;
        if key.vtype() == VarType::Empty {
            let prefix = (
                normalize_setting_name(&appname),
                normalize_setting_name(&section),
            );
            let before = state.values.len();
            state
                .values
                .retain(|(app, sec, _), _| !(app == &prefix.0 && sec == &prefix.1));
            if state.values.len() == before {
                return Err(settings_missing_fault(
                    self.profile,
                    capability,
                    op,
                    "setting section does not exist",
                ));
            }
            return Ok(Variant::empty());
        }

        let key = self.setting_text(&key, capability, op, "key")?;
        if state
            .values
            .remove(&settings_key(&appname, &section, &key))
            .is_none()
        {
            return Err(settings_missing_fault(
                self.profile,
                capability,
                op,
                "setting key does not exist",
            ));
        }
        Ok(Variant::empty())
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
                .variant_to_i32(&command, capability, "shell", "command")
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
                .variant_to_i32(&command, capability, "shell", "command")
                .unwrap_or(0),
        };
        Ok(Variant::from_i32(if command == 0 { 0 } else { 1 }))
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
                .variant_to_i32(&key, capability, "environ", "key")
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
                .variant_to_i32(&key, capability, "environ", "key")
                .unwrap_or(0),
        };
        Ok(Variant::from_i32(key))
    }

    fn dir_variant(&self, path: Variant, _attrs: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dir"));
        }
        if self.native_process_enabled() {
            let is_continuation = match path.vtype() {
                VarType::Empty | VarType::Null => true,
                VarType::String => path
                    .as_bstr()
                    .map(|text| text.as_str().is_empty())
                    .unwrap_or(false),
                _ => {
                    self.variant_to_i32(&path, capability, "dir", "path")
                        .unwrap_or(1)
                        == 0
                }
            };
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
                self.variant_to_i32(&path, capability, "dir", "path")
                    .unwrap_or(0)
                    != 0,
            ),
        };
        Ok(Variant::from_i32(out))
    }
}

impl StandardHostServices {
    fn setting_text(
        &self,
        value: &Variant,
        capability: CapabilityId,
        op: &'static str,
        field: &'static str,
    ) -> HalResult<String> {
        variant_to_vba_string(value)
            .map(|text| text.as_str().to_string())
            .map_err(|message| {
                HalError::adapter_fault(self.profile, capability, op, format!("{field}: {message}"))
            })
    }
}

fn normalize_setting_name(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn settings_key(appname: &str, section: &str, key: &str) -> (String, String, String) {
    (
        normalize_setting_name(appname),
        normalize_setting_name(section),
        normalize_setting_name(key),
    )
}

fn settings_missing_fault(
    profile: crate::model::HalProfileId,
    capability: CapabilityId,
    op: &'static str,
    message: &'static str,
) -> HalError {
    HalError::adapter_fault(profile, capability, op, message).with_host_error_code(5)
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
