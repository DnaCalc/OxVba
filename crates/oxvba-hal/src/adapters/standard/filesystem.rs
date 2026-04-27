use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::FileSystemHal,
};
use oxvba_runtime::{RuntimeValue, VarType, Variant, bstr::BStr};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::path::{Component, Path, PathBuf};
#[cfg(target_os = "windows")]
use std::{thread, time::Duration};

use super::StandardHostServices;

#[derive(Debug, Default)]
pub(super) struct FileSystemState {
    pub(super) handles: BTreeMap<i32, FileHandleState>,
}

impl FileSystemState {
    pub(super) fn first_free_in(&self, start: i32, end: i32) -> Option<i32> {
        let in_use: BTreeSet<i32> = self.handles.keys().copied().collect();
        (start..=end).find(|candidate| !in_use.contains(candidate))
    }

    pub(super) fn is_handle_in_use(&self, handle: i32) -> bool {
        self.handles.contains_key(&handle)
    }
}

#[derive(Debug, Clone)]
pub(super) struct FileHandleState {
    pub(super) mode: i32,
    pub(super) position: i32,
    pub(super) len: i32,
    pub(super) host_path: Option<PathBuf>,
    pub(super) data: Vec<u8>,
}

pub(super) fn pseudo_file_len_from_path_token(path: i32) -> i32 {
    let magnitude = path.saturating_abs();
    1 + (magnitude % 4096)
}

pub(super) fn clamp_u64_to_i32(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

fn format_write_field_variant(data: &Variant) -> String {
    match data.vtype() {
        VarType::String => {
            let text = data.as_bstr().unwrap_or_else(BStr::empty);
            let escaped = text.as_str().replace('"', "\"\"");
            format!("\"{escaped}\"")
        }
        VarType::Boolean => {
            if data.as_bool().unwrap_or(false) {
                "#TRUE#".to_string()
            } else {
                "#FALSE#".to_string()
            }
        }
        VarType::Empty | VarType::Null => "#NULL#".to_string(),
        VarType::Integer => data.as_i16().unwrap_or(0).to_string(),
        VarType::Long => data.as_i32().unwrap_or(0).to_string(),
        VarType::LongLong => data.as_i64().unwrap_or(0).to_string(),
        VarType::Byte => data.as_u8().unwrap_or(0).to_string(),
        VarType::Single => data.as_f32().unwrap_or(0.0).to_string(),
        VarType::Double => data.as_f64().unwrap_or(0.0).to_string(),
        VarType::Date => data.as_date_f64().unwrap_or(0.0).to_string(),
        other => format!("{other:?}"),
    }
}

fn parse_input_field(data: &[u8], mut cursor: usize) -> (String, usize) {
    let len = data.len();
    while cursor < len && matches!(data[cursor], b' ' | b'\t') {
        cursor += 1;
    }
    if cursor >= len {
        return (String::new(), cursor);
    }

    let mut out = String::new();
    if data[cursor] == b'"' {
        cursor += 1;
        while cursor < len {
            match data[cursor] {
                b'"' if cursor + 1 < len && data[cursor + 1] == b'"' => {
                    out.push('"');
                    cursor += 2;
                }
                b'"' => {
                    cursor += 1;
                    break;
                }
                byte => {
                    out.push(byte as char);
                    cursor += 1;
                }
            }
        }
    } else {
        let start = cursor;
        while cursor < len && !matches!(data[cursor], b',' | b'\r' | b'\n') {
            cursor += 1;
        }
        out = String::from_utf8_lossy(&data[start..cursor])
            .trim()
            .to_string();
    }

    (out, cursor)
}

fn variant_from_input_field(field: &str) -> Variant {
    if field.eq_ignore_ascii_case("#TRUE#") {
        return Variant::from_bool(true);
    }
    if field.eq_ignore_ascii_case("#FALSE#") {
        return Variant::from_bool(false);
    }
    if field.eq_ignore_ascii_case("#NULL#") {
        return Variant::empty();
    }
    if let Ok(value) = field.parse::<i32>() {
        return Variant::from_i32(value);
    }
    if let Ok(value) = field.parse::<f64>() {
        return Variant::from_f64(value);
    }
    Variant::from_string(field)
}

fn advance_input_separator(data: &[u8], mut cursor: usize) -> usize {
    let len = data.len();
    while cursor < len && matches!(data[cursor], b' ' | b'\t') {
        cursor += 1;
    }
    if cursor < len && data[cursor] == b',' {
        cursor += 1;
    } else if cursor < len && data[cursor] == b'\r' {
        cursor += 1;
        if cursor < len && data[cursor] == b'\n' {
            cursor += 1;
        }
    } else if cursor < len && data[cursor] == b'\n' {
        cursor += 1;
    }
    cursor
}

#[cfg(target_os = "windows")]
fn remove_file_with_retry(host_path: &PathBuf) -> std::io::Result<()> {
    const MAX_ATTEMPTS: usize = 8;
    const RETRY_DELAY_MS: u64 = 25;

    let mut last_err = None;
    for attempt in 0..MAX_ATTEMPTS {
        match fs::remove_file(host_path) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                last_err = Some(err);
                if attempt + 1 < MAX_ATTEMPTS {
                    thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                    continue;
                }
            }
            Err(err) => return Err(err),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("remove_file retry loop exhausted")))
}

impl FileSystemHal for StandardHostServices {
    // Legacy filesystem path. Retained VM/JIT callers should use
    // `open_variant`, which keeps path/mode/result values as Variant carriers.
    fn open(&self, path: RuntimeValue, mode: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let path = runtime_value_to_filesystem_variant(self.profile, capability, "open", path)?;
        let mode = runtime_value_to_filesystem_variant(self.profile, capability, "open", mode)?;
        let result = self.open_variant(path, mode)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "open", result)
    }

    fn open_variant(&self, path: Variant, mode: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "open"));
        }
        let mode_raw = self.variant_project_compat_slot_i32(&mode, capability, "open", "mode")?;
        let requested_handle = mode_raw >> 16;
        let mode = mode_raw & 0xFFFF;
        if mode != 0 && !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "open"));
        }
        if let Some(path_text) = path.as_bstr() {
            let mut state = self.fs_lock(capability, "open")?;
            self.assert_fs_invariants(&state, "open-pre");
            let handle = if requested_handle > 0 && requested_handle <= 511 {
                if state.is_handle_in_use(requested_handle) {
                    return Err(HalError::adapter_fault(
                        self.profile,
                        capability,
                        "open",
                        format!("file handle #{requested_handle} is already in use"),
                    ));
                }
                requested_handle
            } else {
                state.first_free_in(1, 511).ok_or_else(|| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "open",
                        "no free file handles available in supported range",
                    )
                })?
            };
            let host_path = if self.native_fs_enabled() {
                let host_path = PathBuf::from(path_text.as_str());
                if let Some(parent) = host_path.parent() {
                    fs::create_dir_all(parent).map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "open",
                            format!("failed to create host fs directory: {err}"),
                        )
                    })?;
                }
                Some(host_path)
            } else {
                None
            };
            let (initial_data, initial_len, initial_position) = if let Some(host_path) =
                host_path.as_ref()
            {
                match mode {
                    0 => {
                        let data = fs::read(host_path).map_err(|err| {
                            HalError::adapter_fault(
                                self.profile,
                                capability,
                                "open",
                                format!("failed to read host path {}: {err}", host_path.display()),
                            )
                        })?;
                        let len = clamp_u64_to_i32(data.len() as u64);
                        (data, len, 0)
                    }
                    1 => {
                        OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .open(host_path.as_path())
                            .map_err(|err| {
                                HalError::adapter_fault(
                                    self.profile,
                                    capability,
                                    "open",
                                    format!(
                                        "failed to create output host path {}: {err}",
                                        host_path.display()
                                    ),
                                )
                            })?;
                        (Vec::new(), 0, 0)
                    }
                    2 => {
                        OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create(true)
                            .truncate(false)
                            .open(host_path.as_path())
                            .map_err(|err| {
                                HalError::adapter_fault(
                                    self.profile,
                                    capability,
                                    "open",
                                    format!(
                                        "failed to open append host path {}: {err}",
                                        host_path.display()
                                    ),
                                )
                            })?;
                        let data = fs::read(host_path).unwrap_or_default();
                        let len = clamp_u64_to_i32(data.len() as u64);
                        (data, len, len)
                    }
                    _ => {
                        OpenOptions::new()
                            .read(true)
                            .write(true)
                            .create(true)
                            .truncate(false)
                            .open(host_path.as_path())
                            .map_err(|err| {
                                HalError::adapter_fault(
                                    self.profile,
                                    capability,
                                    "open",
                                    format!(
                                        "failed to open host path {}: {err}",
                                        host_path.display()
                                    ),
                                )
                            })?;
                        let data = fs::read(host_path).unwrap_or_default();
                        let len = clamp_u64_to_i32(data.len() as u64);
                        (data, len, 0)
                    }
                }
            } else if mode == 0 {
                (Vec::new(), i32::from(!path_text.is_empty()), 0)
            } else {
                (Vec::new(), 0, 0)
            };
            state.handles.insert(
                handle,
                FileHandleState {
                    mode,
                    position: initial_position,
                    len: initial_len,
                    host_path,
                    data: initial_data,
                },
            );
            self.assert_fs_invariants(&state, "open-post");
            return Ok(Variant::from_i32(handle));
        }
        let path = self.variant_project_compat_slot_i32(&path, capability, "open", "path")?;
        let mut state = self.fs_lock(capability, "open")?;
        self.assert_fs_invariants(&state, "open-pre");
        let Some(handle) = state.first_free_in(1, 511) else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "open",
                "no free file handles available in supported range",
            ));
        };
        let host_path = if self.native_fs_enabled() {
            let host_path = self.host_path_from_token(path);
            if let Some(parent) = host_path.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "open",
                        format!("failed to create host fs directory: {err}"),
                    )
                })?;
            }
            Some(host_path)
        } else {
            None
        };
        let (initial_data, initial_len, initial_position) = if let Some(host_path) =
            host_path.as_ref()
        {
            match mode {
                0 => {
                    let data = fs::read(host_path).map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "open",
                            format!("failed to read host path {}: {err}", host_path.display()),
                        )
                    })?;
                    let len = clamp_u64_to_i32(data.len() as u64);
                    (data, len, 0)
                }
                1 => {
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .open(host_path)
                        .map_err(|err| {
                            HalError::adapter_fault(
                                self.profile,
                                capability,
                                "open",
                                format!(
                                    "failed to create output host path {}: {err}",
                                    host_path.display()
                                ),
                            )
                        })?;
                    (Vec::new(), 0, 0)
                }
                2 => {
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(host_path)
                        .map_err(|err| {
                            HalError::adapter_fault(
                                self.profile,
                                capability,
                                "open",
                                format!(
                                    "failed to open append host path {}: {err}",
                                    host_path.display()
                                ),
                            )
                        })?;
                    let data = fs::read(host_path).unwrap_or_default();
                    let len = clamp_u64_to_i32(data.len() as u64);
                    (data, len, len)
                }
                _ => {
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(host_path)
                        .map_err(|err| {
                            HalError::adapter_fault(
                                self.profile,
                                capability,
                                "open",
                                format!("failed to open host path {}: {err}", host_path.display()),
                            )
                        })?;
                    let data = fs::read(host_path).unwrap_or_default();
                    let len = clamp_u64_to_i32(data.len() as u64);
                    (data, len, 0)
                }
            }
        } else if mode == 0 {
            (Vec::new(), pseudo_file_len_from_path_token(path), 0)
        } else {
            (Vec::new(), 0, 0)
        };
        state.handles.insert(
            handle,
            FileHandleState {
                mode,
                position: initial_position,
                len: initial_len,
                host_path,
                data: initial_data,
            },
        );
        self.assert_fs_invariants(&state, "open-post");
        hal_contract_assert!(
            (1..=511).contains(&handle),
            "op=open returned out-of-range handle {}",
            handle
        );
        Ok(Variant::from_i32(handle))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `close_variant`.
    fn close(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle =
            runtime_value_to_filesystem_variant(self.profile, capability, "close", handle)?;
        let result = self.close_variant(handle)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "close", result)
    }

    fn close_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "close"));
        }
        let handle =
            self.variant_project_compat_slot_i32(&handle, capability, "close", "handle")?;
        let mut state = self.fs_lock(capability, "close")?;
        self.assert_fs_invariants(&state, "close-pre");
        if handle == 0 {
            let drained: Vec<FileHandleState> = state.handles.values().cloned().collect();
            let count = drained.len() as i32;
            state.handles.clear();
            for entry in drained {
                if entry.mode != 0
                    && let Some(host_path) = entry.host_path.as_ref()
                {
                    fs::write(host_path, &entry.data).map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "close",
                            format!("failed to flush host path {}: {err}", host_path.display()),
                        )
                    })?;
                }
            }
            self.assert_fs_invariants(&state, "close-all-post");
            Ok(Variant::from_i32(count))
        } else if let Some(entry) = state.handles.remove(&handle) {
            if entry.mode != 0
                && let Some(host_path) = entry.host_path.as_ref()
            {
                fs::write(host_path, &entry.data).map_err(|err| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "close",
                        format!("failed to flush host path {}: {err}", host_path.display()),
                    )
                })?;
            }
            self.assert_fs_invariants(&state, "close-post");
            Ok(Variant::from_i32(1))
        } else {
            Err(HalError::adapter_fault(
                self.profile,
                capability,
                "close",
                format!("invalid file handle: {handle}"),
            ))
        }
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `kill_variant`.
    fn kill(&self, path: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let path = runtime_value_to_filesystem_variant(self.profile, capability, "kill", path)?;
        let result = self.kill_variant(path)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "kill", result)
    }

    fn kill_variant(&self, path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "kill"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "kill"));
        }
        let Some(path_text) = path.as_bstr() else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "kill",
                "path must be a string",
            ));
        };
        if path_text.as_str().contains('*') || path_text.as_str().contains('?') {
            if self.native_fs_enabled() {
                let wildcard_path = path_text.as_str();
                let matched_paths = expand_host_wildcard_paths(Path::new(&wildcard_path))
                    .map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "kill",
                            format!("failed to expand wildcard Kill path {path_text}: {err}"),
                        )
                    })?
                    .into_iter()
                    .filter(|path| path.is_file())
                    .collect::<Vec<_>>();
                if matched_paths.is_empty() {
                    return Err(HalError::adapter_fault(
                        self.profile,
                        capability,
                        "kill",
                        format!("wildcard Kill path matched no files: {path_text}"),
                    ));
                }
                for matched_path in matched_paths {
                    #[cfg(target_os = "windows")]
                    let remove_result = remove_file_with_retry(&matched_path);
                    #[cfg(not(target_os = "windows"))]
                    let remove_result = fs::remove_file(&matched_path);

                    remove_result.map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "kill",
                            format!(
                                "failed to remove host path {} expanded from {}: {err}",
                                matched_path.display(),
                                path_text
                            ),
                        )
                    })?;
                }
            }
            return Ok(Variant::from_i32(0));
        }
        if self.native_fs_enabled() {
            let host_path = PathBuf::from(path_text.as_str());
            #[cfg(target_os = "windows")]
            let remove_result = remove_file_with_retry(&host_path);
            #[cfg(not(target_os = "windows"))]
            let remove_result = fs::remove_file(&host_path);

            remove_result.map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "kill",
                    format!("failed to remove host path {}: {err}", host_path.display()),
                )
            })?;
        }
        Ok(Variant::from_i32(0))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `seek_variant`.
    fn seek(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle = runtime_value_to_filesystem_variant(self.profile, capability, "seek", handle)?;
        let position =
            runtime_value_to_filesystem_variant(self.profile, capability, "seek", position)?;
        let result = self.seek_variant(handle, position)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "seek", result)
    }

    fn seek_variant(&self, handle: Variant, position: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "seek"));
        }
        let handle = self.variant_project_compat_slot_i32(&handle, capability, "seek", "handle")?;
        let position =
            self.variant_project_compat_slot_i32(&position, capability, "seek", "position")?;
        if position < 0 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "seek",
                format!("negative seek position: {position}"),
            ));
        }

        let mut state = self.fs_lock(capability, "seek")?;
        self.assert_fs_invariants(&state, "seek-pre");
        let final_position = {
            let entry = self.fs_entry_mut(&mut state, handle, "seek")?;
            let prior_len = entry.len;
            let host_path = entry.host_path.clone();
            entry.position = position;
            if position > entry.len && entry.mode != 0 && self.policy.allow_filesystem_mutation {
                if position as usize > entry.data.len() {
                    entry.data.resize(position as usize, 0);
                }
                if let Some(host_path) = host_path.as_ref() {
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(host_path.as_path())
                        .map_err(|err| {
                            HalError::adapter_fault(
                                self.profile,
                                capability,
                                "seek",
                                format!(
                                    "failed to open host path {} for seek: {err}",
                                    host_path.display()
                                ),
                            )
                        })?;
                    file.set_len(position as u64).map_err(|err| {
                        HalError::adapter_fault(
                            self.profile,
                            capability,
                            "seek",
                            format!(
                                "failed to extend host path {} to {}: {err}",
                                host_path.display(),
                                position
                            ),
                        )
                    })?;
                }
                entry.len = position;
            }
            hal_contract_assert!(
                entry.position == position,
                "op=seek did not preserve requested position {}; got {}",
                position,
                entry.position
            );
            let expected_len =
                if position > prior_len && entry.mode != 0 && self.policy.allow_filesystem_mutation
                {
                    position
                } else {
                    prior_len
                };
            hal_contract_assert!(
                entry.len == expected_len,
                "op=seek expected len {} but found {}",
                expected_len,
                entry.len
            );
            entry.position
        };
        self.assert_fs_invariants(&state, "seek-post");
        Ok(Variant::from_i32(final_position))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `eof_variant`.
    fn eof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle = runtime_value_to_filesystem_variant(self.profile, capability, "eof", handle)?;
        let result = self.eof_variant(handle)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "eof", result)
    }

    fn eof_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "eof"));
        }
        let handle = self.variant_project_compat_slot_i32(&handle, capability, "eof", "handle")?;
        let mut state = self.fs_lock(capability, "eof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "eof")?;
        Ok(Variant::from_i32(if entry.position >= entry.len {
            1
        } else {
            0
        }))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `lof_variant`.
    fn lof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle = runtime_value_to_filesystem_variant(self.profile, capability, "lof", handle)?;
        let result = self.lof_variant(handle)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "lof", result)
    }

    fn lof_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "lof"));
        }
        let handle = self.variant_project_compat_slot_i32(&handle, capability, "lof", "handle")?;
        let mut state = self.fs_lock(capability, "lof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "lof")?;
        Ok(Variant::from_i32(entry.len))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `free_file_variant`.
    fn free_file(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let range_selector = runtime_value_to_filesystem_variant(
            self.profile,
            capability,
            "free_file",
            range_selector,
        )?;
        let result = self.free_file_variant(range_selector)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "free_file", result)
    }

    fn free_file_variant(&self, range_selector: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "free_file"));
        }
        let range_selector = self.variant_project_compat_slot_i32(
            &range_selector,
            capability,
            "free_file",
            "range_selector",
        )?;
        let (start, end) = if range_selector == 1 {
            (256, 511)
        } else {
            (1, 255)
        };
        let state = self.fs_lock(capability, "free_file")?;
        self.assert_fs_invariants(&state, "free_file");
        let candidate = state.first_free_in(start, end).ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "free_file",
                format!("no free file number in range {start}..={end}"),
            )
        })?;
        hal_contract_assert!(
            (start..=end).contains(&candidate),
            "op=free_file returned {} outside range {}..={}",
            candidate,
            start,
            end
        );
        Ok(Variant::from_i32(candidate))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `read_bytes_variant`.
    fn read_bytes(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle =
            runtime_value_to_filesystem_variant(self.profile, capability, "read_bytes", handle)?;
        let count =
            runtime_value_to_filesystem_variant(self.profile, capability, "read_bytes", count)?;
        let result = self.read_bytes_variant(handle, count)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "read_bytes", result)
    }

    fn read_bytes_variant(&self, handle: Variant, count: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "read_bytes"));
        }
        let handle_id =
            self.variant_project_compat_slot_i32(&handle, capability, "read_bytes", "handle")?;
        let count =
            self.variant_project_compat_slot_i32(&count, capability, "read_bytes", "count")?;
        let mut state = self.fs_lock(capability, "read_bytes")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "read_bytes")?;
        let pos = entry.position as usize;
        let count = count.max(0) as usize;
        let available = entry.data.len().saturating_sub(pos);
        let actual = count.min(available);
        let bytes = entry.data[pos..pos + actual].to_vec();
        entry.position += actual as i32;
        Ok(Variant::from_string(
            String::from_utf8_lossy(&bytes).into_owned(),
        ))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `write_bytes_variant`.
    fn write_bytes(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle =
            runtime_value_to_filesystem_variant(self.profile, capability, "write_bytes", handle)?;
        let data =
            runtime_value_to_filesystem_variant(self.profile, capability, "write_bytes", data)?;
        let result = self.write_bytes_variant(handle, data)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "write_bytes", result)
    }

    fn write_bytes_variant(&self, handle: Variant, data: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "write_bytes"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "write_bytes"));
        }
        let handle_id =
            self.variant_project_compat_slot_i32(&handle, capability, "write_bytes", "handle")?;
        let bytes = format!("{}\r\n", format_write_field_variant(&data)).into_bytes();
        let mut state = self.fs_lock(capability, "write_bytes")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "write_bytes")?;
        let pos = entry.position as usize;
        let end = pos + bytes.len();
        if end > entry.data.len() {
            entry.data.resize(end, 0);
        }
        entry.data[pos..end].copy_from_slice(&bytes);
        entry.position = end as i32;
        entry.len = entry.data.len() as i32;
        Ok(Variant::from_i32(bytes.len() as i32))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `print_line_variant`.
    fn print_line(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle =
            runtime_value_to_filesystem_variant(self.profile, capability, "print_line", handle)?;
        let data =
            runtime_value_to_filesystem_variant(self.profile, capability, "print_line", data)?;
        let result = self.print_line_variant(handle, data)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "print_line", result)
    }

    fn print_line_variant(&self, handle: Variant, data: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "print_line"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "print_line"));
        }
        let handle_id =
            self.variant_project_compat_slot_i32(&handle, capability, "print_line", "handle")?;
        let text = match data.as_bstr() {
            Some(text) => format!("{text}\r\n"),
            None => {
                let val =
                    self.variant_project_compat_slot_i32(&data, capability, "print_line", "data")?;
                format!("{val}\r\n")
            }
        };
        let bytes = text.as_bytes();
        let mut state = self.fs_lock(capability, "print_line")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "print_line")?;
        let pos = entry.position as usize;
        let end = pos + bytes.len();
        if end > entry.data.len() {
            entry.data.resize(end, 0);
        }
        entry.data[pos..end].copy_from_slice(bytes);
        entry.position = end as i32;
        entry.len = entry.data.len() as i32;
        Ok(Variant::from_i32(0))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `input_fields_variant`.
    fn input_fields(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle =
            runtime_value_to_filesystem_variant(self.profile, capability, "input_fields", handle)?;
        let count =
            runtime_value_to_filesystem_variant(self.profile, capability, "input_fields", count)?;
        let result = self.input_fields_variant(handle, count)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "input_fields", result)
    }

    fn input_fields_variant(&self, handle: Variant, count: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_fields"));
        }
        let handle_id =
            self.variant_project_compat_slot_i32(&handle, capability, "input_fields", "handle")?;
        let count =
            self.variant_project_compat_slot_i32(&count, capability, "input_fields", "count")?;
        let count = count.max(1) as usize;
        let mut state = self.fs_lock(capability, "input_fields")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "input_fields")?;
        let pos = entry.position as usize;
        let mut fields = Vec::new();
        let mut cursor = pos;
        while fields.len() < count && cursor < entry.data.len() {
            let (field, next_cursor) = parse_input_field(&entry.data, cursor);
            fields.push(field);
            cursor = advance_input_separator(&entry.data, next_cursor);
        }
        entry.position = cursor as i32;
        if count == 1 {
            if let Some(field) = fields.first() {
                return Ok(variant_from_input_field(field));
            }
            return Ok(Variant::from_string(BStr::empty()));
        }
        let result = fields.join(",");
        Ok(Variant::from_string(result))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `line_input_variant`.
    fn line_input(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle =
            runtime_value_to_filesystem_variant(self.profile, capability, "line_input", handle)?;
        let result = self.line_input_variant(handle)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "line_input", result)
    }

    fn line_input_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "line_input"));
        }
        let handle_id =
            self.variant_project_compat_slot_i32(&handle, capability, "line_input", "handle")?;
        let mut state = self.fs_lock(capability, "line_input")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "line_input")?;
        let pos = entry.position as usize;
        if pos >= entry.data.len() {
            return Ok(Variant::from_string(BStr::empty()));
        }
        let remaining = &entry.data[pos..];
        let line_end = remaining
            .iter()
            .position(|&b| b == b'\n')
            .map(|i| {
                if i > 0 && remaining[i - 1] == b'\r' {
                    (i - 1, i + 1)
                } else {
                    (i, i + 1)
                }
            })
            .unwrap_or((remaining.len(), remaining.len()));
        let line = String::from_utf8_lossy(&remaining[..line_end.0]).into_owned();
        entry.position += line_end.1 as i32;
        Ok(Variant::from_string(line))
    }

    // Legacy filesystem path. Retained VM/JIT callers should use
    // `loc_variant`.
    fn loc(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        let handle = runtime_value_to_filesystem_variant(self.profile, capability, "loc", handle)?;
        let result = self.loc_variant(handle)?;
        filesystem_variant_to_runtime_value(self.profile, capability, "loc", result)
    }

    fn loc_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "loc"));
        }
        let handle_id =
            self.variant_project_compat_slot_i32(&handle, capability, "loc", "handle")?;
        let mut state = self.fs_lock(capability, "loc")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "loc")?;
        Ok(Variant::from_i32(entry.position))
    }
}

fn runtime_value_to_filesystem_variant(
    profile: crate::model::HalProfileId,
    capability: CapabilityId,
    operation: &'static str,
    value: RuntimeValue,
) -> HalResult<Variant> {
    match value {
        RuntimeValue::BindingHandle(handle) => Ok(Variant::from_i32(handle.raw())),
        value => Variant::try_from_runtime_value(&value).map_err(|detail| {
            HalError::adapter_fault(
                profile,
                capability,
                operation,
                format!("failed to project RuntimeValue argument into Variant: {detail}"),
            )
        }),
    }
}

fn filesystem_variant_to_runtime_value(
    profile: crate::model::HalProfileId,
    capability: CapabilityId,
    operation: &'static str,
    value: Variant,
) -> HalResult<RuntimeValue> {
    value.to_runtime_value().map_err(|detail| {
        HalError::adapter_fault(
            profile,
            capability,
            operation,
            format!("failed to project retained Variant result into RuntimeValue: {detail}"),
        )
    })
}

pub(super) fn path_contains_wildcards(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(part) => {
            let text = part.to_string_lossy();
            text.contains('*') || text.contains('?')
        }
        _ => false,
    })
}

pub(super) fn expand_host_wildcard_paths(path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let components = path.components().collect::<Vec<_>>();
    let mut roots = vec![PathBuf::new()];
    let mut index = 0;

    while index < components.len() {
        match components[index] {
            Component::Prefix(prefix) => {
                for root in &mut roots {
                    root.push(prefix.as_os_str());
                }
                index += 1;
            }
            Component::RootDir => {
                for root in &mut roots {
                    root.push(Component::RootDir.as_os_str());
                }
                index += 1;
            }
            Component::CurDir => {
                index += 1;
            }
            Component::ParentDir => {
                for root in &mut roots {
                    root.push("..");
                }
                index += 1;
            }
            Component::Normal(_) => break,
        }
    }

    let mut matches = Vec::new();
    expand_host_wildcard_components(&roots, &components[index..], &mut matches)?;
    matches.sort_by_key(|path| wildcard_casefold(&path.to_string_lossy()));
    matches.dedup();
    Ok(matches)
}

fn expand_host_wildcard_components(
    bases: &[PathBuf],
    components: &[Component<'_>],
    matches: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    if components.is_empty() {
        matches.extend(bases.iter().cloned());
        return Ok(());
    }

    let component = components[0];
    match component {
        Component::CurDir => expand_host_wildcard_components(bases, &components[1..], matches),
        Component::ParentDir => {
            let next = bases
                .iter()
                .map(|base| {
                    let mut path = base.clone();
                    path.push("..");
                    path
                })
                .collect::<Vec<_>>();
            expand_host_wildcard_components(&next, &components[1..], matches)
        }
        Component::Normal(part) => {
            let text = part.to_string_lossy();
            let has_wildcards = text.contains('*') || text.contains('?');
            if !has_wildcards {
                let next = bases
                    .iter()
                    .map(|base| {
                        let mut path = base.clone();
                        path.push(part);
                        path
                    })
                    .collect::<Vec<_>>();
                return expand_host_wildcard_components(&next, &components[1..], matches);
            }

            let mut next = Vec::new();
            for base in bases {
                let dir = if base.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    base.clone()
                };
                for entry in fs::read_dir(&dir)? {
                    let entry = entry?;
                    let entry_path = entry.path();
                    let entry_name = entry.file_name();
                    if !wildcard_match(&text, &entry_name.to_string_lossy()) {
                        continue;
                    }
                    if components.len() > 1 && !entry.file_type()?.is_dir() {
                        continue;
                    }
                    next.push(entry_path);
                }
            }
            expand_host_wildcard_components(&next, &components[1..], matches)
        }
        Component::Prefix(_) | Component::RootDir => {
            unreachable!("prefix/root handled before recursion")
        }
    }
}

fn wildcard_match(pattern: &str, candidate: &str) -> bool {
    let pattern: Vec<char> = wildcard_casefold(pattern).chars().collect();
    let candidate: Vec<char> = wildcard_casefold(candidate).chars().collect();
    let mut dp = vec![vec![false; candidate.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for i in 0..pattern.len() {
        if pattern[i] == '*' {
            dp[i + 1][0] = dp[i][0];
        }
        for j in 0..candidate.len() {
            dp[i + 1][j + 1] = match pattern[i] {
                '*' => dp[i][j + 1] || dp[i + 1][j] || dp[i][j],
                '?' => dp[i][j],
                ch => dp[i][j] && ch == candidate[j],
            };
        }
    }

    dp[pattern.len()][candidate.len()]
}

fn wildcard_casefold(text: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        text.to_ascii_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        text.to_string()
    }
}
