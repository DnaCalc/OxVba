use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::FileSystemHal,
};
use oxvba_runtime::{VarType, Variant, bstr::BStr};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::path::{Component, Path, PathBuf};
#[cfg(target_os = "windows")]
use std::{thread, time::Duration};

use super::{StandardHostServices, time::system_time_to_local_vba_serial};

// VBA file-attribute bits. These are deliberately identical to the Win32
// `FILE_ATTRIBUTE_*` values, which is what `GetAttr`/`SetAttr` expose.
const VB_READONLY: i32 = 0x0001;
const VB_HIDDEN: i32 = 0x0002;
const VB_SYSTEM: i32 = 0x0004;
const VB_DIRECTORY: i32 = 0x0010;
const VB_ARCHIVE: i32 = 0x0020;
/// The bits `SetAttr` is permitted to change (directory/volume are intrinsic).
const VB_SETTABLE_ATTRS: i32 = VB_READONLY | VB_HIDDEN | VB_SYSTEM | VB_ARCHIVE;

/// The documented VBA attribute bits for a path (VBA `GetAttr`).
fn file_attribute_bits(meta: &fs::Metadata) -> i32 {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        (meta.file_attributes() as i32)
            & (VB_READONLY | VB_HIDDEN | VB_SYSTEM | VB_DIRECTORY | VB_ARCHIVE)
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Only read-only and directory are observable through portable std.
        let mut bits = 0;
        if meta.permissions().readonly() {
            bits |= VB_READONLY;
        }
        if meta.is_dir() {
            bits |= VB_DIRECTORY;
        }
        bits
    }
}

/// Apply the settable attribute bits to `path`, preserving the bits `SetAttr`
/// does not manage (e.g. `vbDirectory`). `settable` is pre-validated to contain
/// only [`VB_SETTABLE_ATTRS`].
fn apply_file_attributes(path: &Path, settable: i32) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_NORMAL, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
        };

        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 buffer that outlives the call;
        // `GetFileAttributesW` only reads it and returns the attribute DWORD (or
        // INVALID_FILE_ATTRIBUTES on failure, which we surface as an OS error).
        let current = unsafe { GetFileAttributesW(wide.as_ptr()) };
        if current == INVALID_FILE_ATTRIBUTES {
            return Err(std::io::Error::last_os_error());
        }
        let preserved = current & !(VB_SETTABLE_ATTRS as u32);
        let mut new_attrs = preserved | (settable as u32);
        if new_attrs == 0 {
            new_attrs = FILE_ATTRIBUTE_NORMAL;
        }
        // SAFETY: same NUL-terminated UTF-16 buffer; `SetFileAttributesW` reads the
        // path and applies `new_attrs`, returning 0 (FALSE) on failure.
        let ok = unsafe { SetFileAttributesW(wide.as_ptr(), new_attrs) };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Portable std can only toggle the read-only bit; hidden/system/archive
        // have no cross-platform equivalent.
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_readonly(settable & VB_READONLY != 0);
        fs::set_permissions(path, perms)
    }
}

/// Change the current drive (VBA `ChDrive`). On Windows this selects the drive's
/// remembered working directory; elsewhere there is no drive concept, so it is a
/// no-op.
fn change_drive(letter: char) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        // `set_current_dir("C:")` with no trailing separator selects drive C's
        // current directory, which is exactly ChDrive's effect.
        std::env::set_current_dir(format!("{letter}:"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = letter;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(super) struct FileSystemState {
    pub(super) handles: BTreeMap<i32, FileHandleState>,
}

/// Fixed on-disk byte size for a Random/Binary record of `vtype` (None = variable
/// length, i.e. `String`, or unsupported).
fn record_size(vtype: VarType) -> Option<usize> {
    Some(match vtype {
        VarType::Byte => 1,
        VarType::Integer | VarType::Boolean => 2,
        VarType::Long | VarType::Single => 4,
        VarType::Double | VarType::Currency | VarType::Date | VarType::LongLong => 8,
        _ => return None,
    })
}

fn arr<const N: usize>(b: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    let n = b.len().min(N);
    out[..n].copy_from_slice(&b[..n]);
    out
}

/// VBA `Open` mode codes (`Input`=0, `Output`=1 are the sequential `_` cases).
const MODE_APPEND: i32 = 2;
const MODE_BINARY: i32 = 3;
const MODE_RANDOM: i32 = 4;

/// Serialize a value to its VBA on-disk byte representation (little-endian for
/// numerics). A `String` is encoded as ANSI (system code page): a fixed-length
/// string (`fixed`) is raw with no prefix in any mode; a variable string carries
/// a 2-byte length prefix in Random mode and is raw in Binary mode.
fn serialize_record(value: &Variant, mode: i32, fixed: bool) -> Vec<u8> {
    match value.vtype() {
        VarType::Byte => vec![value.as_u8().unwrap_or(0)],
        VarType::Integer => value.as_i16().unwrap_or(0).to_le_bytes().to_vec(),
        VarType::Boolean => (if value.as_bool().unwrap_or(false) {
            -1i16
        } else {
            0
        })
        .to_le_bytes()
        .to_vec(),
        VarType::Long => value.as_i32().unwrap_or(0).to_le_bytes().to_vec(),
        VarType::Single => value.as_f32().unwrap_or(0.0).to_le_bytes().to_vec(),
        VarType::Double => value.as_f64().unwrap_or(0.0).to_le_bytes().to_vec(),
        VarType::Currency => value
            .as_currency_scaled_i64()
            .unwrap_or(0)
            .to_le_bytes()
            .to_vec(),
        VarType::Date => value.as_date_f64().unwrap_or(0.0).to_le_bytes().to_vec(),
        VarType::LongLong => value.as_i64().unwrap_or(0).to_le_bytes().to_vec(),
        VarType::String => {
            let s = value
                .as_bstr()
                .map(|b| b.as_str().to_string())
                .unwrap_or_default();
            let ansi = oxvba_com::ansi::ansi_encode(&s);
            if !fixed && mode == MODE_RANDOM {
                // Variable-length string in a Random record: 2-byte length prefix.
                let len = ansi.len().min(u16::MAX as usize);
                let mut out = (len as u16).to_le_bytes().to_vec();
                out.extend_from_slice(&ansi[..len]);
                out
            } else {
                // Fixed-length string (already padded to N), or Binary mode: raw ANSI.
                ansi
            }
        }
        _ => Vec::new(),
    }
}

/// Decode a fixed-size record of `vtype` from `bytes` (the caller has already
/// sliced to the record's bytes; for strings `bytes` is the ANSI payload).
fn deserialize_record(vtype: VarType, bytes: &[u8]) -> Variant {
    match vtype {
        VarType::Byte => Variant::from_u8(bytes.first().copied().unwrap_or(0)),
        VarType::Integer => Variant::from_i16(i16::from_le_bytes(arr(bytes))),
        VarType::Boolean => Variant::from_bool(i16::from_le_bytes(arr(bytes)) != 0),
        VarType::Long => Variant::from_i32(i32::from_le_bytes(arr(bytes))),
        VarType::Single => Variant::from_f32(f32::from_le_bytes(arr(bytes))),
        VarType::Double => Variant::from_f64(f64::from_le_bytes(arr(bytes))),
        VarType::Currency => Variant::from_currency_scaled_i64(i64::from_le_bytes(arr(bytes))),
        VarType::Date => Variant::from_date_f64(f64::from_le_bytes(arr(bytes))),
        VarType::LongLong => Variant::from_i64(i64::from_le_bytes(arr(bytes))),
        VarType::String => Variant::from_string(oxvba_com::ansi::ansi_decode(bytes)),
        _ => Variant::empty(),
    }
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
    /// Output line width set by `Width #n` (informational; affects `Print`).
    pub(super) width: i32,
    /// Locked 1-based byte/record ranges (inclusive) for `Lock`/`Unlock`. A `Lock`
    /// overlapping an already-locked range raises VBA error 70.
    pub(super) locks: Vec<(i32, i32)>,
    /// Fixed record length from `Open … For Random … Len = N` (0 = unset; Random
    /// records then use the written value's natural size as the stride).
    pub(super) record_len: i32,
}

/// The 1-based position VBA `Seek(filenumber)` (FUNCTION form) reports: the next
/// byte to read/write (Binary/sequential) or the next record (Random). The
/// internal `position` is a 0-based byte cursor, so the byte form is `cursor + 1`;
/// Random divides by the fixed record length to report a 1-based record number.
fn seek_report(entry: &FileHandleState) -> i32 {
    if entry.mode == MODE_RANDOM && entry.record_len > 0 {
        entry.position / entry.record_len + 1
    } else {
        entry.position.saturating_add(1)
    }
}

/// The value VBA `Loc(filenumber)` reports: the number of the last record
/// read/written (Random); the 1-based offset of the last byte (Binary), which is
/// exactly the 0-based next-byte cursor; or `current byte position \ 128` for the
/// sequential modes. The sequential value is documented by Microsoft as "neither
/// used nor required"; the `\ 128` form matches a live `Output` probe (200→1,
/// 400→3) and the documented formula.
fn loc_report(entry: &FileHandleState) -> i32 {
    match entry.mode {
        MODE_RANDOM if entry.record_len > 0 => entry.position / entry.record_len,
        MODE_BINARY | MODE_RANDOM => entry.position,
        _ => entry.position / 128,
    }
}

/// The byte offset a sequential write (`Print #`/`Write #`) starts at. In
/// `Append` mode every write lands at the current end-of-file regardless of the
/// reported seek cursor (VBA keeps the two decoupled); the other modes write at
/// the 0-based cursor.
fn seq_write_offset(entry: &FileHandleState) -> usize {
    if entry.mode == MODE_APPEND {
        entry.len.max(0) as usize
    } else {
        entry.position.max(0) as usize
    }
}

pub(super) fn pseudo_file_len_from_path_token(path: i32) -> i32 {
    let magnitude = path.saturating_abs();
    1 + (magnitude % 4096)
}

pub(super) fn clamp_u64_to_i32(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
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

impl StandardHostServices {
    /// The 1-based inclusive lock range for `Lock`/`Unlock`: no args = whole file,
    /// one arg = a single record, `start To end` = the explicit range.
    fn lock_range(
        &self,
        start: &Variant,
        end: &Variant,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<(i32, i32)> {
        let start = if matches!(start.vtype(), VarType::Empty | VarType::Null) {
            None
        } else {
            Some(self.variant_to_i32(start, capability, op, "start")?)
        };
        let end = if matches!(end.vtype(), VarType::Empty | VarType::Null) {
            None
        } else {
            Some(self.variant_to_i32(end, capability, op, "end")?)
        };
        Ok(match (start, end) {
            (None, _) => (1, i32::MAX),
            (Some(s), None) => (s, s),
            (Some(s), Some(e)) => (s, e),
        })
    }
}

impl FileSystemHal for StandardHostServices {
    fn open_variant(&self, path: Variant, mode: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "open"));
        }
        let mode_raw = self.variant_to_i32(&mode, capability, "open", "mode")?;
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
                        // VBA `Append` reports `Seek = 1` / `Loc = 0` on a fresh
                        // open even though the file is non-empty; the write cursor
                        // is decoupled and writes always land at EOF (handled in
                        // the sequential write paths). So the reported cursor is 0.
                        (data, len, 0)
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
                    width: 0,
                    locks: Vec::new(),
                    record_len: 0,
                },
            );
            self.assert_fs_invariants(&state, "open-post");
            return Ok(Variant::from_i32(handle));
        }
        let path = self.variant_to_i32(&path, capability, "open", "path")?;
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
                width: 0,
                locks: Vec::new(),
                record_len: 0,
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

    fn open_with_record_len(
        &self,
        path: Variant,
        mode: Variant,
        record_len: Variant,
    ) -> HalResult<Variant> {
        let handle = self.open_variant(path, mode)?;
        // Record a positive `Len = N` for Random-mode fixed-record positioning.
        if let Ok(len) = self.variant_to_i32(&record_len, CapabilityId::FileSystemIo, "open", "len")
            && len > 0
            && let Ok(handle_id) =
                self.variant_to_i32(&handle, CapabilityId::FileSystemIo, "open", "handle")
        {
            let mut state = self.fs_lock(CapabilityId::FileSystemIo, "open")?;
            if let Ok(entry) = self.fs_entry_mut(&mut state, handle_id, "open") {
                entry.record_len = len;
            }
        }
        Ok(handle)
    }

    fn close_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "close"));
        }
        let handle = self.variant_to_i32(&handle, capability, "close", "handle")?;
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

    fn mkdir_variant(&self, path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "mkdir"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "mkdir"));
        }
        let Some(path_text) = path.as_bstr() else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "mkdir",
                "path must be a string",
            ));
        };
        if self.native_fs_enabled() {
            // VBA `MkDir` creates a single directory (error 75 if it already
            // exists / the parent is missing); mirror with a non-recursive create.
            fs::create_dir(Path::new(&path_text.as_str())).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "mkdir",
                    format!("failed to create directory {path_text}: {err}"),
                )
            })?;
        }
        Ok(Variant::from_i32(0))
    }

    fn rmdir_variant(&self, path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "rmdir"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "rmdir"));
        }
        let Some(path_text) = path.as_bstr() else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "rmdir",
                "path must be a string",
            ));
        };
        if self.native_fs_enabled() {
            // VBA `RmDir` removes an empty directory; `remove_dir` (non-recursive)
            // matches (it errors if the directory is non-empty or missing).
            fs::remove_dir(Path::new(&path_text.as_str())).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "rmdir",
                    format!("failed to remove directory {path_text}: {err}"),
                )
            })?;
        }
        Ok(Variant::from_i32(0))
    }

    fn seek_variant(&self, handle: Variant, position: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "seek"));
        }
        let handle = self.variant_to_i32(&handle, capability, "seek", "handle")?;
        // The `Seek(filenumber)` FUNCTION form omits the position — it READS the
        // current 1-based position (next byte, or next record in Random) without
        // moving the cursor.
        if matches!(position.vtype(), VarType::Empty | VarType::Null) {
            let mut state = self.fs_lock(capability, "seek")?;
            let entry = self.fs_entry_mut(&mut state, handle, "seek")?;
            return Ok(Variant::from_i32(seek_report(entry)));
        }
        // The `Seek #n, pos` STATEMENT repositions. `pos` is 1-based: a byte offset
        // for Binary/sequential, a record number for Random (mapped to its byte
        // slot via the fixed record length). VBA requires `pos >= 1`, and a bare
        // Seek past EOF does NOT extend the file — only a subsequent write does.
        let position = self.variant_to_i32(&position, capability, "seek", "position")?;
        if position < 1 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "seek",
                format!("bad record number for seek: {position}"),
            ));
        }
        let mut state = self.fs_lock(capability, "seek")?;
        self.assert_fs_invariants(&state, "seek-pre");
        {
            let entry = self.fs_entry_mut(&mut state, handle, "seek")?;
            entry.position = if entry.mode == MODE_RANDOM && entry.record_len > 0 {
                (position - 1).saturating_mul(entry.record_len)
            } else {
                position - 1
            };
        }
        self.assert_fs_invariants(&state, "seek-post");
        // The Seek STATEMENT yields no value; the discarded result is `Empty`.
        Ok(Variant::empty())
    }

    fn eof_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "eof"));
        }
        let handle = self.variant_to_i32(&handle, capability, "eof", "handle")?;
        let mut state = self.fs_lock(capability, "eof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "eof")?;
        Ok(Variant::from_i32(if entry.position >= entry.len {
            1
        } else {
            0
        }))
    }

    fn lof_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "lof"));
        }
        let handle = self.variant_to_i32(&handle, capability, "lof", "handle")?;
        let mut state = self.fs_lock(capability, "lof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "lof")?;
        Ok(Variant::from_i32(entry.len))
    }

    fn free_file_variant(&self, range_selector: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "free_file"));
        }
        let range_selector =
            self.variant_to_i32(&range_selector, capability, "free_file", "range_selector")?;
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

    fn read_bytes_variant(&self, handle: Variant, count: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "read_bytes"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "read_bytes", "handle")?;
        let count = self.variant_to_i32(&count, capability, "read_bytes", "count")?;
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

    fn write_bytes_variant(&self, handle: Variant, data: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "write_bytes"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "write_bytes"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "write_bytes", "handle")?;
        let bytes = format!("{}\r\n", oxvba_runtime::write_display_text(&data)).into_bytes();
        let mut state = self.fs_lock(capability, "write_bytes")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "write_bytes")?;
        let pos = seq_write_offset(entry);
        let end = pos + bytes.len();
        if end > entry.data.len() {
            entry.data.resize(end, 0);
        }
        entry.data[pos..end].copy_from_slice(&bytes);
        entry.position = end as i32;
        entry.len = entry.data.len() as i32;
        Ok(Variant::from_i32(bytes.len() as i32))
    }

    fn print_line_variant(&self, handle: Variant, data: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "print_line"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "print_line"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "print_line", "handle")?;
        // The caller (oxvba-lib `file_print`/`file_write`) assembles the complete VBA
        // record — per-field formatting, separators, print zones, and the `\r\n`
        // terminator (when not suppressed by a trailing `;`/`,`) — so this sink writes
        // the text verbatim at the current position.
        let text = data
            .as_bstr()
            .map(|t| t.as_str().to_string())
            .unwrap_or_default();
        let bytes = text.as_bytes();
        let mut state = self.fs_lock(capability, "print_line")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "print_line")?;
        let pos = seq_write_offset(entry);
        let end = pos + bytes.len();
        if end > entry.data.len() {
            entry.data.resize(end, 0);
        }
        entry.data[pos..end].copy_from_slice(bytes);
        entry.position = end as i32;
        entry.len = entry.data.len() as i32;
        Ok(Variant::from_i32(0))
    }

    fn input_fields_variant(&self, handle: Variant, count: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_fields"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "input_fields", "handle")?;
        let count = self.variant_to_i32(&count, capability, "input_fields", "count")?;
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

    fn line_input_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "line_input"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "line_input", "handle")?;
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

    fn loc_variant(&self, handle: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "loc"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "loc", "handle")?;
        let mut state = self.fs_lock(capability, "loc")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "loc")?;
        Ok(Variant::from_i32(loc_report(entry)))
    }

    fn put_record_variant(
        &self,
        handle: Variant,
        position: Variant,
        value: Variant,
        fixed: Variant,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "put"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "put", "handle")?;
        let explicit = if matches!(position.vtype(), VarType::Empty | VarType::Null) {
            None
        } else {
            Some(self.variant_to_i32(&position, capability, "put", "position")?)
        };
        // A fixed-length-string source writes raw (no length prefix) in any mode.
        let fixed = self
            .variant_to_i32(&fixed, capability, "put", "fixed")
            .unwrap_or(0)
            != 0;
        let mut state = self.fs_lock(capability, "put")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "put")?;
        let mode = entry.mode;
        let bytes = serialize_record(&value, mode, fixed);
        // Random mode positions by fixed-width record slot (`(rec-1)*stride`),
        // padding the slot; Binary mode positions by byte offset.
        let (offset, slot) = if mode == MODE_RANDOM {
            let stride = if entry.record_len > 0 {
                entry.record_len as usize
            } else {
                bytes.len()
            };
            if entry.record_len > 0 && bytes.len() > stride {
                return Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "put",
                    "Bad record length: value exceeds the file's Len",
                ));
            }
            let off = match explicit {
                Some(rec) if rec >= 1 => ((rec - 1) as usize).saturating_mul(stride),
                _ => entry.position.max(0) as usize,
            };
            (off, stride)
        } else {
            let off = match explicit {
                Some(rec) if rec >= 1 => (rec - 1) as usize,
                _ => entry.position.max(0) as usize,
            };
            (off, bytes.len())
        };
        let write_end = offset + bytes.len();
        let slot_end = offset + slot;
        if slot_end > entry.data.len() {
            entry.data.resize(slot_end, 0);
        }
        entry.data[offset..write_end].copy_from_slice(&bytes);
        entry.position = slot_end as i32;
        entry.len = entry.len.max(slot_end as i32);
        Ok(Variant::empty())
    }

    fn get_record_variant(
        &self,
        handle: Variant,
        position: Variant,
        vtype: Variant,
        str_len: Variant,
    ) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "get"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "get", "handle")?;
        let explicit = if matches!(position.vtype(), VarType::Empty | VarType::Null) {
            None
        } else {
            Some(self.variant_to_i32(&position, capability, "get", "position")?)
        };
        let code = self.variant_to_i32(&vtype, capability, "get", "type")? as u16;
        let vt = VarType::from_u16(code).unwrap_or(VarType::Empty);
        let fixed_numeric = record_size(vt);
        // `str_len` carries the string length intent: > 0 = a fixed-length string of
        // that many bytes (no prefix, any mode); <= 0 = a variable string, where
        // `-str_len` is the target's current Len (used for a Binary read; Random
        // uses the on-disk 2-byte length prefix).
        let str_len = self
            .variant_to_i32(&str_len, capability, "get", "str_len")
            .unwrap_or(0);
        let mut state = self.fs_lock(capability, "get")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "get")?;
        let mode = entry.mode;
        // Random mode strides by the fixed record length (or the value's natural
        // size: numeric size, or a fixed string's byte length); Binary by byte offset.
        let stride = if entry.record_len > 0 {
            entry.record_len as usize
        } else if let Some(size) = fixed_numeric {
            size
        } else if str_len > 0 {
            str_len as usize
        } else {
            0
        };
        let offset = if mode == MODE_RANDOM {
            match explicit {
                Some(rec) if rec >= 1 => ((rec - 1) as usize).saturating_mul(stride),
                _ => entry.position.max(0) as usize,
            }
        } else {
            match explicit {
                Some(rec) if rec >= 1 => (rec - 1) as usize,
                _ => entry.position.max(0) as usize,
            }
        };
        let data_len = entry.data.len();
        let at = |start: usize, n: usize| -> (usize, usize) {
            (start.min(data_len), (start + n).min(data_len))
        };
        let (value, advance) = match (fixed_numeric, vt) {
            (Some(size), _) => {
                let (s, e) = at(offset, size);
                let v = deserialize_record(vt, &entry.data[s..e]);
                (
                    v,
                    if mode == MODE_RANDOM && stride > 0 {
                        stride
                    } else {
                        size
                    },
                )
            }
            (None, VarType::String) if str_len > 0 => {
                // Fixed-length string: exactly `str_len` ANSI bytes, no prefix.
                let n = str_len as usize;
                let (s, e) = at(offset, n);
                let v = deserialize_record(VarType::String, &entry.data[s..e]);
                (
                    v,
                    if mode == MODE_RANDOM && stride > 0 {
                        stride
                    } else {
                        n
                    },
                )
            }
            (None, VarType::String) if mode == MODE_RANDOM => {
                // Variable-length string in a Random record: 2-byte length prefix.
                let len = if offset + 2 <= data_len {
                    u16::from_le_bytes([entry.data[offset], entry.data[offset + 1]]) as usize
                } else {
                    0
                };
                let (s, e) = at(offset + 2, len);
                let v = deserialize_record(VarType::String, &entry.data[s..e]);
                (v, if stride > 0 { stride } else { 2 + len })
            }
            (None, VarType::String) if mode == MODE_BINARY => {
                // Variable-length string in Binary: read the target's current Len
                // (`-str_len`) ANSI bytes, no prefix.
                let n = (-str_len).max(0) as usize;
                let (s, e) = at(offset, n);
                (deserialize_record(VarType::String, &entry.data[s..e]), n)
            }
            (None, _) => (Variant::empty(), 0),
        };
        entry.position = (offset + advance) as i32;
        Ok(value)
    }

    fn width_variant(&self, handle: Variant, width: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "width"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "width", "handle")?;
        let width = self.variant_to_i32(&width, capability, "width", "width")?;
        let mut state = self.fs_lock(capability, "width")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "width")?;
        entry.width = width.max(0);
        Ok(Variant::empty())
    }

    fn cur_dir_variant(&self, _drive: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "curdir"));
        }
        // A read of process state; the deterministic lane reports the empty
        // string rather than leaking the real working directory.
        if self.native_fs_enabled() {
            let dir = std::env::current_dir().map_err(|err| {
                HalError::adapter_fault(self.profile, capability, "curdir", err.to_string())
            })?;
            Ok(Variant::from_string(BStr::from(
                dir.to_string_lossy().as_ref(),
            )))
        } else {
            Ok(Variant::from_string(BStr::empty()))
        }
    }

    fn ch_dir_variant(&self, path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "chdir"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "chdir"));
        }
        if self.native_fs_enabled() {
            let dir = self.variant_to_path(&path, capability, "chdir", "path")?;
            std::env::set_current_dir(&dir).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "chdir",
                    format!("failed to change directory to {}: {err}", dir.display()),
                )
            })?;
        }
        Ok(Variant::from_i32(0))
    }

    fn file_len_variant(&self, path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "filelen"));
        }
        if self.native_fs_enabled() {
            let p = self.variant_to_path(&path, capability, "filelen", "path")?;
            let len = fs::metadata(&p).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "filelen",
                    format!("failed to stat {}: {err}", p.display()),
                )
            })?;
            // VBA `FileLen` returns a `Long`.
            Ok(Variant::from_i32(len.len().min(i32::MAX as u64) as i32))
        } else {
            Ok(Variant::from_i32(0))
        }
    }

    fn file_copy_variant(&self, source: Variant, dest: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "filecopy"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "filecopy"));
        }
        if self.native_fs_enabled() {
            let src = self.variant_to_path(&source, capability, "filecopy", "source")?;
            let dst = self.variant_to_path(&dest, capability, "filecopy", "destination")?;
            fs::copy(&src, &dst).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "filecopy",
                    format!(
                        "failed to copy {} to {}: {err}",
                        src.display(),
                        dst.display()
                    ),
                )
            })?;
        }
        Ok(Variant::from_i32(0))
    }

    fn get_attr_variant(&self, path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "getattr"));
        }
        if self.native_fs_enabled() {
            let p = self.variant_to_path(&path, capability, "getattr", "path")?;
            let meta = fs::metadata(&p).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "getattr",
                    format!("failed to stat {}: {err}", p.display()),
                )
            })?;
            // VBA `GetAttr` returns an `Integer` of the documented attribute bits.
            Ok(Variant::from_i32(file_attribute_bits(&meta)))
        } else {
            Ok(Variant::from_i32(0))
        }
    }

    fn set_attr_variant(&self, path: Variant, attributes: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "setattr"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "setattr"));
        }
        let requested = self.variant_to_i32(&attributes, capability, "setattr", "attributes")?;
        // `SetAttr` manages only these bits; `vbDirectory`/`vbVolume` are intrinsic
        // to the entry and cannot be assigned, so a request to set them is an error
        // (real VBA raises 5, "Invalid procedure call") rather than a silent no-op.
        if requested & !VB_SETTABLE_ATTRS != 0 {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "setattr",
                format!(
                    "attribute value {requested} sets bits SetAttr cannot change \
                     (only vbReadOnly|vbHidden|vbSystem|vbArchive)"
                ),
            ));
        }
        if self.native_fs_enabled() {
            let p = self.variant_to_path(&path, capability, "setattr", "path")?;
            apply_file_attributes(&p, requested).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "setattr",
                    format!("failed to set attributes on {}: {err}", p.display()),
                )
            })?;
        }
        Ok(Variant::from_i32(0))
    }

    fn ch_drive_variant(&self, drive: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "chdrive"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "chdrive"));
        }
        let Some(text) = drive.as_bstr() else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "chdrive",
                "drive must be a string",
            ));
        };
        // VBA `ChDrive` looks only at the first character; "" is a no-op.
        if let Some(letter) = text.as_str().chars().next()
            && self.native_fs_enabled()
        {
            change_drive(letter).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "chdrive",
                    format!("failed to change drive to {letter:?}: {err}"),
                )
            })?;
        }
        Ok(Variant::from_i32(0))
    }

    fn file_date_time_variant(&self, path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "filedatetime"));
        }
        if self.native_fs_enabled() {
            let p = self.variant_to_path(&path, capability, "filedatetime", "path")?;
            let meta = fs::metadata(&p).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "filedatetime",
                    format!("failed to stat {}: {err}", p.display()),
                )
            })?;
            let modified = meta.modified().map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "filedatetime",
                    format!("modification time unavailable for {}: {err}", p.display()),
                )
            })?;
            // VBA `FileDateTime` reports the local wall-clock timestamp shown by
            // the host filesystem, matching the native `Now`/`Date`/`Time` facet.
            Ok(Variant::from_date_f64(system_time_to_local_vba_serial(
                modified,
            )))
        } else {
            // Deterministic lane: no real files; a stable zero serial (1899-12-30).
            Ok(Variant::from_date_f64(0.0))
        }
    }

    fn name_variant(&self, old_path: Variant, new_path: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "name"));
        }
        if !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "name"));
        }
        // Real rename when host filesystem access is enabled; otherwise the
        // deterministic lane has no host file to move (a documented no-op).
        if self.native_fs_enabled() {
            let old = self.variant_to_path(&old_path, capability, "name", "old")?;
            let new = self.variant_to_path(&new_path, capability, "name", "new")?;
            if let Some(parent) = new.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::rename(&old, &new).map_err(|err| {
                HalError::adapter_fault(
                    self.profile,
                    capability,
                    "name",
                    format!(
                        "failed to rename {} -> {}: {err}",
                        old.display(),
                        new.display()
                    ),
                )
            })?;
        }
        Ok(Variant::empty())
    }

    fn lock_variant(&self, handle: Variant, start: Variant, end: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "lock"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "lock", "handle")?;
        let range = self.lock_range(&start, &end, capability, "lock")?;
        let mut state = self.fs_lock(capability, "lock")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "lock")?;
        // Re-locking an already-locked range is VBA error 70 (permission denied).
        if entry
            .locks
            .iter()
            .any(|&(s, e)| range.0 <= e && s <= range.1)
        {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "lock",
                "Permission denied: range already locked",
            ));
        }
        entry.locks.push(range);
        Ok(Variant::empty())
    }

    fn unlock_variant(&self, handle: Variant, start: Variant, end: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "unlock"));
        }
        let handle_id = self.variant_to_i32(&handle, capability, "unlock", "handle")?;
        let range = self.lock_range(&start, &end, capability, "unlock")?;
        let mut state = self.fs_lock(capability, "unlock")?;
        let entry = self.fs_entry_mut(&mut state, handle_id, "unlock")?;
        entry.locks.retain(|&r| r != range);
        Ok(Variant::empty())
    }
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

#[cfg(test)]
mod record_io_tests {
    use super::{MODE_BINARY, MODE_RANDOM, deserialize_record, record_size, serialize_record};
    use oxvba_runtime::{VarType, Variant};

    #[test]
    fn long_serializes_little_endian_and_round_trips() {
        let bytes = serialize_record(&Variant::from_i32(0x1234_5678), MODE_RANDOM, false);
        assert_eq!(bytes, vec![0x78, 0x56, 0x34, 0x12]);
        assert_eq!(
            deserialize_record(VarType::Long, &bytes).as_i32(),
            Some(0x1234_5678)
        );
    }

    #[test]
    fn fixed_record_sizes_match_vba() {
        assert_eq!(record_size(VarType::Byte), Some(1));
        assert_eq!(record_size(VarType::Integer), Some(2));
        assert_eq!(record_size(VarType::Long), Some(4));
        assert_eq!(record_size(VarType::Double), Some(8));
        assert_eq!(record_size(VarType::String), None);
    }

    #[test]
    fn double_and_boolean_round_trip() {
        let d = serialize_record(&Variant::from_f64(3.5), MODE_RANDOM, false);
        assert_eq!(d.len(), 8);
        assert_eq!(deserialize_record(VarType::Double, &d).as_f64(), Some(3.5));
        let b = serialize_record(&Variant::from_bool(true), MODE_RANDOM, false);
        assert_eq!(b, vec![0xFF, 0xFF]);
        assert_eq!(
            deserialize_record(VarType::Boolean, &b).as_bool(),
            Some(true)
        );
    }

    #[test]
    fn string_framing_varies_by_mode_and_fixedness() {
        // Variable string, Random: 2-byte ANSI length prefix + ANSI bytes.
        let r = serialize_record(&Variant::from_string("Hi"), MODE_RANDOM, false);
        assert_eq!(r, vec![0x02, 0x00, b'H', b'i']);
        assert_eq!(
            deserialize_record(VarType::String, &r[2..])
                .as_bstr()
                .unwrap()
                .as_str(),
            "Hi"
        );
        // Variable string, Binary: raw ANSI, no length prefix.
        assert_eq!(
            serialize_record(&Variant::from_string("Hi"), MODE_BINARY, false),
            vec![b'H', b'i']
        );
        // Fixed-length string (already padded to N): raw ANSI, no prefix, even in Random.
        let f = serialize_record(&Variant::from_string("ab  "), MODE_RANDOM, true);
        assert_eq!(f, vec![b'a', b'b', b' ', b' ']);
    }
}
