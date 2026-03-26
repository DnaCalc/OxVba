use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::FileSystemHal,
};
use oxvba_runtime::{RuntimeValue, bstr::BStr};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

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

fn format_write_field(data: &RuntimeValue) -> String {
    match data {
        RuntimeValue::String(BStr(text)) => {
            let escaped = text.replace('"', "\"\"");
            format!("\"{escaped}\"")
        }
        RuntimeValue::Bool(value) => {
            if *value {
                "#TRUE#".to_string()
            } else {
                "#FALSE#".to_string()
            }
        }
        RuntimeValue::Empty => "#NULL#".to_string(),
        RuntimeValue::I32(value) => value.to_string(),
        RuntimeValue::F64(value) => value.to_string(),
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

impl FileSystemHal for StandardHostServices {
    fn open(&self, path: RuntimeValue, mode: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "open"));
        }
        let mode_raw = self.runtime_value_to_legacy_i32(&mode, capability, "open", "mode")?;
        // Upper 16 bits may carry a requested file number from the VBA Open statement.
        let requested_handle = mode_raw >> 16;
        let mode = mode_raw & 0xFFFF;
        if mode != 0 && !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "open"));
        }
        if let RuntimeValue::String(BStr(path_text)) = &path {
            let mut state = self.fs_lock(capability, "open")?;
            self.assert_fs_invariants(&state, "open-pre");
            let handle = if requested_handle > 0 && requested_handle <= 511 {
                // VBA Open ... As #N — use the requested handle if available.
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
                // Auto-allocate (legacy path / FreeFile-based callers).
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
                let host_path = PathBuf::from(path_text);
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
            return Ok(RuntimeValue::I32(handle));
        }
        let path = self.runtime_value_to_legacy_i32(&path, capability, "open", "path")?;
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
        Ok(RuntimeValue::I32(handle))
    }

    fn close(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "close"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "close", "handle")?;
        let mut state = self.fs_lock(capability, "close")?;
        self.assert_fs_invariants(&state, "close-pre");
        if handle == 0 {
            // VBA `Close` with no arguments: close all open files.
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
            Ok(RuntimeValue::I32(count))
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
            Ok(RuntimeValue::I32(1))
        } else {
            Err(HalError::adapter_fault(
                self.profile,
                capability,
                "close",
                format!("invalid file handle: {handle}"),
            ))
        }
    }

    fn seek(&self, handle: RuntimeValue, position: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "seek"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "seek", "handle")?;
        let position =
            self.runtime_value_to_legacy_i32(&position, capability, "seek", "position")?;
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
        Ok(RuntimeValue::I32(final_position))
    }

    fn eof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "eof"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "eof", "handle")?;
        let mut state = self.fs_lock(capability, "eof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "eof")?;
        Ok(RuntimeValue::I32(if entry.position >= entry.len {
            1
        } else {
            0
        }))
    }

    fn lof(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "lof"));
        }
        let handle = self.runtime_value_to_legacy_i32(&handle, capability, "lof", "handle")?;
        let mut state = self.fs_lock(capability, "lof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "lof")?;
        Ok(RuntimeValue::I32(entry.len))
    }

    fn free_file(&self, range_selector: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "free_file"));
        }
        let range_selector = self.runtime_value_to_legacy_i32(
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
        Ok(RuntimeValue::I32(candidate))
    }

    fn read_bytes(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "read_bytes"));
        }
        let handle_id =
            self.runtime_value_to_legacy_i32(&handle, capability, "read_bytes", "handle")?;
        let count = self.runtime_value_to_legacy_i32(&count, capability, "read_bytes", "count")?;
        let mut state = self.fs_lock(capability, "read_bytes")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "read_bytes")?;
        let pos = entry.position as usize;
        let count = count.max(0) as usize;
        let available = entry.data.len().saturating_sub(pos);
        let actual = count.min(available);
        let bytes = entry.data[pos..pos + actual].to_vec();
        entry.position += actual as i32;
        Ok(RuntimeValue::String(BStr(
            String::from_utf8_lossy(&bytes).into_owned(),
        )))
    }

    fn write_bytes(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "write_bytes"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "write_bytes"));
        }
        let handle_id =
            self.runtime_value_to_legacy_i32(&handle, capability, "write_bytes", "handle")?;
        let bytes = format!("{}\r\n", format_write_field(&data)).into_bytes();
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
        Ok(RuntimeValue::I32(bytes.len() as i32))
    }

    fn print_line(&self, handle: RuntimeValue, data: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "print_line"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "print_line"));
        }
        let handle_id =
            self.runtime_value_to_legacy_i32(&handle, capability, "print_line", "handle")?;
        let text = match &data {
            RuntimeValue::String(BStr(s)) => format!("{s}\r\n"),
            other => {
                let val =
                    self.runtime_value_to_legacy_i32(other, capability, "print_line", "data")?;
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
        Ok(RuntimeValue::I32(0))
    }

    fn input_fields(&self, handle: RuntimeValue, count: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_fields"));
        }
        let handle_id =
            self.runtime_value_to_legacy_i32(&handle, capability, "input_fields", "handle")?;
        let count =
            self.runtime_value_to_legacy_i32(&count, capability, "input_fields", "count")?;
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
        let result = fields.join(",");
        Ok(RuntimeValue::String(BStr(result)))
    }

    fn line_input(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "line_input"));
        }
        let handle_id =
            self.runtime_value_to_legacy_i32(&handle, capability, "line_input", "handle")?;
        let mut state = self.fs_lock(capability, "line_input")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "line_input")?;
        let pos = entry.position as usize;
        if pos >= entry.data.len() {
            return Ok(RuntimeValue::String(BStr(String::new())));
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
        Ok(RuntimeValue::String(BStr(line)))
    }

    fn loc(&self, handle: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "loc"));
        }
        let handle_id = self.runtime_value_to_legacy_i32(&handle, capability, "loc", "handle")?;
        let mut state = self.fs_lock(capability, "loc")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "loc")?;
        Ok(RuntimeValue::I32(entry.position))
    }
}
