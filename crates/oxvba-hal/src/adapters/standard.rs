use crate::{
    error::{HalError, HalResult},
    model::{
        CapabilityDescriptor, CapabilityId, CapabilityMaturity, HalDescriptor, HalProfileId,
        HalRuntimeClass, HostPolicy, UiVirtualizationMode, WasmRuntimeClass,
        host_backed_mode_active,
    },
    traits::{
        ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, ProcessEnvHal,
        TimeLocaleHal, UiInteractionHal,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MB_OK, MSG, MessageBoxW, PM_REMOVE, PeekMessageW, TranslateMessage,
};

#[cfg(any(debug_assertions, feature = "hal_contract_checks"))]
macro_rules! hal_contract_assert {
    ($cond:expr, $($arg:tt)+) => {
        assert!(
            $cond,
            "HAL contract assertion failed: {}",
            format_args!($($arg)+)
        );
    };
}

#[cfg(not(any(debug_assertions, feature = "hal_contract_checks")))]
macro_rules! hal_contract_assert {
    ($cond:expr, $($arg:tt)+) => {
        let _ = (stringify!($cond), stringify!($($arg)+));
    };
}

#[derive(Debug, Clone)]
pub(crate) struct StandardHostServices {
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    descriptor: HalDescriptor,
    policy: HostPolicy,
    fs_state: Arc<Mutex<FileSystemState>>,
}

impl StandardHostServices {
    pub(crate) fn new(profile: HalProfileId, policy: HostPolicy) -> Self {
        let runtime_class = policy
            .runtime_class
            .unwrap_or_else(|| HalRuntimeClass::default_for(profile, policy.wasm_runtime_class));
        Self::new_with_runtime_class(profile, runtime_class, policy)
    }

    pub(crate) fn new_with_runtime_class(
        profile: HalProfileId,
        runtime_class: HalRuntimeClass,
        policy: HostPolicy,
    ) -> Self {
        Self {
            profile,
            runtime_class,
            descriptor: descriptor_for_profile(profile, runtime_class, &policy),
            policy,
            fs_state: Arc::new(Mutex::new(FileSystemState::default())),
        }
    }

    pub(crate) fn profile(&self) -> HalProfileId {
        self.profile
    }

    pub(crate) fn runtime_class(&self) -> HalRuntimeClass {
        self.runtime_class
    }

    pub(crate) fn descriptor(&self) -> HalDescriptor {
        self.descriptor.clone()
    }

    pub(crate) fn policy(&self) -> &HostPolicy {
        &self.policy
    }

    fn supports(&self, capability: CapabilityId) -> bool {
        self.descriptor.supports(capability)
    }

    fn unsupported(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::capability_unavailable(self.profile, capability, op)
    }

    fn denied(&self, capability: CapabilityId, op: &'static str) -> HalError {
        HalError::policy_denied(self.profile, capability, op)
    }

    fn fs_lock(
        &self,
        capability: CapabilityId,
        op: &'static str,
    ) -> HalResult<std::sync::MutexGuard<'_, FileSystemState>> {
        self.fs_state.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                capability,
                op,
                "filesystem state lock poisoned",
            )
        })
    }

    fn fs_entry_mut<'a>(
        &'a self,
        state: &'a mut FileSystemState,
        handle: i32,
        op: &'static str,
    ) -> HalResult<&'a mut FileHandleState> {
        state.handles.get_mut(&handle).ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                CapabilityId::FileSystemIo,
                op,
                format!("invalid file handle: {handle}"),
            )
        })
    }

    // Contract-check scaffold: lightweight invariants now, intended to harden over time.
    // Future steps are tracked in docs/spec/HAL_CONTRACT_ASSERTION_HARDENING.md.
    fn assert_fs_invariants(&self, state: &FileSystemState, op: &'static str) {
        #[cfg(any(debug_assertions, feature = "hal_contract_checks"))]
        {
            hal_contract_assert!(
                state.handles.len() <= 511,
                "op={} tracks too many handles: {}",
                op,
                state.handles.len()
            );
            for (handle, entry) in &state.handles {
                hal_contract_assert!(
                    (1..=511).contains(handle),
                    "op={} observed out-of-range handle {}",
                    op,
                    handle
                );
                hal_contract_assert!(
                    entry.position >= 0,
                    "op={} observed negative position {} for handle {}",
                    op,
                    entry.position,
                    handle
                );
                hal_contract_assert!(
                    entry.len >= 0,
                    "op={} observed negative len {} for handle {}",
                    op,
                    entry.len,
                    handle
                );
                if !self.policy.allow_filesystem_mutation {
                    hal_contract_assert!(
                        entry.mode == 0,
                        "op={} observed mutable handle {} while mutation policy is disabled",
                        op,
                        handle
                    );
                }
            }
        }

        #[cfg(not(any(debug_assertions, feature = "hal_contract_checks")))]
        {
            let _ = (state, op);
        }
    }

    fn native_mode_enabled(&self) -> bool {
        host_backed_mode_active(self.profile, &self.policy)
    }

    fn native_fs_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn native_process_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn native_time_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn native_diagnostics_enabled(&self) -> bool {
        self.native_mode_enabled()
    }

    fn host_fs_base_dir(&self) -> PathBuf {
        let mut out = std::env::temp_dir();
        out.push("oxvba_hal");
        out.push(match self.profile {
            HalProfileId::Windows => "windows",
            HalProfileId::Linux => "linux",
            HalProfileId::MacOs => "macos",
            HalProfileId::Wasm => "wasm",
            HalProfileId::Null => "null",
        });
        out
    }

    fn host_path_from_token(&self, token: i32) -> PathBuf {
        let mut path = self.host_fs_base_dir();
        path.push(format!("token_{}.dat", token.saturating_abs()));
        path
    }

    #[cfg(target_os = "windows")]
    fn spawn_probe_shell_process(&self, command: i32) -> std::io::Result<std::process::Child> {
        Command::new("cmd")
            .args(["/C", &format!("echo OXVBA_HAL_{command} > NUL")])
            .spawn()
    }

    #[cfg(not(target_os = "windows"))]
    fn spawn_probe_shell_process(&self, _command: i32) -> std::io::Result<std::process::Child> {
        Command::new("sh").arg("-c").arg("true").spawn()
    }

    #[cfg(target_os = "windows")]
    fn native_windows_msg_box(&self, prompt: i32, style: i32) -> HalResult<i32> {
        let text = format!("OxVba MsgBox token={prompt}");
        let title = "OxVba";
        let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let style_flags = if style == 0 { MB_OK } else { style as u32 };
        let result = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text_w.as_ptr(),
                title_w.as_ptr(),
                style_flags,
            )
        };
        if result <= 0 {
            return Err(HalError::adapter_fault(
                self.profile,
                CapabilityId::UiInteraction,
                "msg_box",
                "native MessageBoxW returned failure",
            ));
        }
        Ok(result)
    }

    #[cfg(not(target_os = "windows"))]
    fn native_windows_msg_box(&self, _prompt: i32, _style: i32) -> HalResult<i32> {
        Ok(1)
    }

    #[cfg(target_os = "windows")]
    fn pump_windows_messages_once(&self) {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        unsafe {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn pump_windows_messages_once(&self) {}
}

pub(crate) fn descriptor_for_profile(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    policy: &HostPolicy,
) -> HalDescriptor {
    HalDescriptor {
        profile,
        runtime_class: runtime_class.as_str(),
        contract_version: "hal-v1",
        adapter_version: "0.1.0",
        capabilities: capability_matrix(profile, runtime_class, policy.wasm_runtime_class),
    }
}

impl UiInteractionHal for StandardHostServices {
    fn msg_box(&self, prompt: i32, style: i32) -> HalResult<i32> {
        let capability = CapabilityId::UiInteraction;
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
            return self.native_windows_msg_box(prompt, style);
        }
        if self.native_mode_enabled()
            && self.profile == HalProfileId::Linux
            && self.runtime_class() == HalRuntimeClass::LinuxStdio
            && self.policy.ui_virtualization == UiVirtualizationMode::Disabled
        {
            eprintln!("[oxvba-hal] linux-stdio msg_box token={prompt} style={style}");
            return Ok(style.max(1));
        }
        let result = match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "msg_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(style.max(1)),
            UiVirtualizationMode::Disabled => Ok(prompt.max(1)),
        };
        if let Ok(value) = result {
            hal_contract_assert!(
                value >= 1,
                "op=msg_box must return positive token, got {}",
                value
            );
        }
        result
    }

    fn input_box(&self, prompt: i32, default_value: i32) -> HalResult<i32> {
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
            eprintln!("[oxvba-hal] linux-stdio input_box token={prompt} default={default_value}");
            return Ok(default_value);
        }
        let result = match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "input_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::Disabled => Ok(prompt),
        };
        if let Ok(value) = result {
            match self.policy.ui_virtualization {
                UiVirtualizationMode::ScriptedResponses => {
                    hal_contract_assert!(
                        value == default_value,
                        "op=input_box scripted response mismatch: expected {}, got {}",
                        default_value,
                        value
                    );
                }
                UiVirtualizationMode::Disabled => {
                    hal_contract_assert!(
                        value == prompt,
                        "op=input_box disabled response mismatch: expected {}, got {}",
                        prompt,
                        value
                    );
                }
                UiVirtualizationMode::FailOnPrompt => {}
            }
        }
        result
    }
}

impl EventPumpHal for StandardHostServices {
    fn do_events(&self) -> HalResult<i32> {
        let capability = CapabilityId::EventPump;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "do_events"));
        }
        if self.native_mode_enabled() {
            if self.profile == HalProfileId::Windows
                && self.runtime_class() == HalRuntimeClass::WindowsGui
            {
                self.pump_windows_messages_once();
            }
            thread::yield_now();
        }
        Ok(0)
    }
}

impl FileSystemHal for StandardHostServices {
    fn open(&self, path: i32, mode: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "open"));
        }
        if mode != 0 && !self.policy.allow_filesystem_mutation {
            return Err(self.denied(capability, "open"));
        }

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
        let initial_len = if let Some(host_path) = host_path.as_ref() {
            if mode != 0 {
                let file = OpenOptions::new()
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
                clamp_u64_to_i32(file.metadata().map(|meta| meta.len()).unwrap_or(0))
            } else {
                fs::metadata(host_path)
                    .map(|meta| clamp_u64_to_i32(meta.len()))
                    .unwrap_or_else(|_| pseudo_file_len_from_path_token(path))
            }
        } else if mode == 0 {
            pseudo_file_len_from_path_token(path)
        } else {
            0
        };
        state.handles.insert(
            handle,
            FileHandleState {
                mode,
                position: 0,
                len: initial_len,
                host_path,
            },
        );
        self.assert_fs_invariants(&state, "open-post");
        hal_contract_assert!(
            (1..=511).contains(&handle),
            "op=open returned out-of-range handle {}",
            handle
        );
        Ok(handle)
    }

    fn close(&self, handle: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "close"));
        }
        let mut state = self.fs_lock(capability, "close")?;
        self.assert_fs_invariants(&state, "close-pre");
        if state.handles.remove(&handle).is_some() {
            self.assert_fs_invariants(&state, "close-post");
            Ok(1)
        } else {
            Err(HalError::adapter_fault(
                self.profile,
                capability,
                "close",
                format!("invalid file handle: {handle}"),
            ))
        }
    }

    fn seek(&self, handle: i32, position: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "seek"));
        }
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
                if let Some(host_path) = host_path.as_ref() {
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(host_path)
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
        Ok(final_position)
    }

    fn eof(&self, handle: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "eof"));
        }
        let mut state = self.fs_lock(capability, "eof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "eof")?;
        Ok(if entry.position >= entry.len { 1 } else { 0 })
    }

    fn lof(&self, handle: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "lof"));
        }
        let mut state = self.fs_lock(capability, "lof")?;
        let entry = self.fs_entry_mut(&mut state, handle, "lof")?;
        Ok(entry.len)
    }

    fn free_file(&self, range_selector: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "free_file"));
        }
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
        Ok(candidate)
    }
}

impl ProcessEnvHal for StandardHostServices {
    fn shell(&self, command: i32, _window_style: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "shell"));
        }
        if !self.policy.allow_process_spawn {
            return Err(self.denied(capability, "shell"));
        }
        if self.native_process_enabled() && command != 0 {
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
            return Ok(child_id);
        }
        Ok(if command == 0 { 0 } else { 1 })
    }

    fn environ(&self, key: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "environ"));
        }
        if self.native_process_enabled() {
            let mut vars: Vec<(std::ffi::OsString, std::ffi::OsString)> =
                std::env::vars_os().collect();
            if vars.is_empty() {
                return Ok(0);
            }
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            let idx = (key.unsigned_abs() as usize) % vars.len();
            let value_len = vars[idx].1.to_string_lossy().len();
            return Ok(value_len.min(i32::MAX as usize) as i32);
        }
        Ok(key)
    }

    fn dir(&self, path: i32, _attrs: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dir"));
        }
        if self.native_process_enabled() {
            let target = if path == 0 {
                std::env::current_dir().map_err(|err| {
                    HalError::adapter_fault(
                        self.profile,
                        capability,
                        "dir",
                        format!("failed to get current directory: {err}"),
                    )
                })?
            } else {
                self.host_path_from_token(path)
            };
            return Ok(match fs::read_dir(target) {
                Ok(mut entries) => {
                    if entries.next().is_some() {
                        1
                    } else {
                        0
                    }
                }
                Err(_) => 0,
            });
        }
        Ok(if path == 0 { 0 } else { 1 })
    }
}

impl ComHal for StandardHostServices {
    fn create_object(&self, prog_id: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "create_object"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "create_object"));
        }
        Ok(5_000i32.saturating_add(prog_id))
    }

    fn dispatch_invoke(&self, object: i32, member: i32, arg: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dispatch_invoke"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "dispatch_invoke"));
        }
        Ok(object.saturating_add(member).saturating_add(arg))
    }
}

impl TimeLocaleHal for StandardHostServices {
    fn date_serial_now(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "date_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok(clamp_u64_to_i32(now.as_secs() / 86_400));
        }
        Ok(20_260_301)
    }

    fn time_serial_now(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "time_serial_now"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            return Ok((now.as_secs() % 86_400) as i32);
        }
        Ok(123_456)
    }

    fn timer_ticks(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "timer_ticks"));
        }
        if self.native_time_enabled() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let modulo = i32::MAX as u128;
            return Ok((now.as_millis() % modulo) as i32);
        }
        Ok(42)
    }
}

impl DynamicLinkHal for StandardHostServices {
    fn invoke_symbol(&self, symbol: i32, arg: i32) -> HalResult<i32> {
        let capability = CapabilityId::DynamicLinking;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "invoke_symbol"));
        }
        if !self.policy.allow_dynamic_link {
            return Err(self.denied(capability, "invoke_symbol"));
        }
        if self.native_mode_enabled()
            && matches!(self.profile, HalProfileId::Windows | HalProfileId::Linux)
        {
            return match symbol {
                s if s == external_symbol_token("host", "ping", "hostping") => {
                    Ok(arg.saturating_add(1))
                }
                s if s == external_symbol_token("host", "double", "hostdouble") => {
                    Ok(arg.saturating_mul(2))
                }
                _ => Err(HalError::adapter_fault(
                    self.profile,
                    capability,
                    "invoke_symbol",
                    format!("symbol token {symbol} not resolved in host-backed lane"),
                )),
            };
        }
        Ok(symbol.saturating_add(arg))
    }
}

impl DiagnosticsHal for StandardHostServices {
    fn emit(&self, code: i32, payload: i32) -> HalResult<i32> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "emit"));
        }
        if self.native_diagnostics_enabled() {
            eprintln!(
                "[oxvba-hal] profile={:?} code={} payload={}",
                self.profile, code, payload
            );
        }
        Ok(code.saturating_add(payload))
    }
}

fn capability_matrix(
    profile: HalProfileId,
    runtime_class: HalRuntimeClass,
    wasm_runtime_class: WasmRuntimeClass,
) -> Vec<CapabilityDescriptor> {
    use CapabilityId as C;
    use CapabilityMaturity as M;
    let mut out = Vec::new();
    let mut push = |id: C, supported: bool, maturity: M, spec_anchor: &'static str| {
        out.push(CapabilityDescriptor {
            id,
            supported,
            maturity,
            spec_anchor,
        });
    };

    match profile {
        HalProfileId::Windows => {
            push(
                C::UiInteraction,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Provisional, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(
                C::ComActivationDispatch,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0325",
            );
            push(
                C::TimeLocale,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Provisional, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
        HalProfileId::Linux => {
            push(
                C::UiInteraction,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Provisional,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Experimental, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
        HalProfileId::MacOs => {
            push(
                C::UiInteraction,
                true,
                M::Stub,
                "CONF-discovered-ms-vbal-250520-f945507e-0337",
            );
            push(C::EventPump, true, M::Stub, "MS-VBAL:DoEvents");
            push(
                C::FileSystemIo,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0286",
            );
            push(
                C::ProcessEnv,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0346",
            );
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Experimental,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, true, M::Stub, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
        HalProfileId::Wasm => match runtime_class {
            HalRuntimeClass::WasmWasiLocal => {
                push(
                    C::UiInteraction,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0337",
                );
                push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                push(
                    C::TimeLocale,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0252",
                );
                push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            }
            HalRuntimeClass::WasmBrowserSandbox => {
                push(
                    C::UiInteraction,
                    false,
                    M::Stable,
                    "MS-VBAL:MsgBox/InputBox",
                );
                push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                push(
                    C::TimeLocale,
                    true,
                    M::Experimental,
                    "CONF-discovered-ms-vbal-250520-f945507e-0252",
                );
                push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
            }
            _ => match wasm_runtime_class {
                WasmRuntimeClass::Wasi => {
                    push(
                        C::UiInteraction,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0337",
                    );
                    push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                    push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                    push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                    push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                    push(
                        C::TimeLocale,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0252",
                    );
                    push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                    push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                }
                WasmRuntimeClass::BrowserSandbox => {
                    push(
                        C::UiInteraction,
                        false,
                        M::Stable,
                        "MS-VBAL:MsgBox/InputBox",
                    );
                    push(C::EventPump, true, M::Experimental, "MS-VBAL:DoEvents");
                    push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
                    push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell");
                    push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
                    push(
                        C::TimeLocale,
                        true,
                        M::Experimental,
                        "CONF-discovered-ms-vbal-250520-f945507e-0252",
                    );
                    push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
                    push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
                }
            },
        },
        HalProfileId::Null => {
            push(
                C::UiInteraction,
                false,
                M::Stable,
                "MS-VBAL:MsgBox/InputBox",
            );
            push(C::EventPump, false, M::Stable, "MS-VBAL:DoEvents");
            push(C::FileSystemIo, false, M::Stable, "MS-VBAL:file-io");
            push(C::ProcessEnv, false, M::Stable, "MS-VBAL:Shell/Dir/Environ");
            push(C::ComActivationDispatch, false, M::Stable, "MS-OAUT");
            push(
                C::TimeLocale,
                true,
                M::Stable,
                "CONF-discovered-ms-vbal-250520-f945507e-0252",
            );
            push(C::DynamicLinking, false, M::Stable, "MS-VBAL:Declare");
            push(C::DiagnosticsTelemetry, true, M::Stable, "OxVba:HAL");
        }
    }
    out
}

#[derive(Debug, Default)]
struct FileSystemState {
    handles: BTreeMap<i32, FileHandleState>,
}

impl FileSystemState {
    fn first_free_in(&self, start: i32, end: i32) -> Option<i32> {
        let in_use: BTreeSet<i32> = self.handles.keys().copied().collect();
        (start..=end).find(|candidate| !in_use.contains(candidate))
    }
}

#[derive(Debug, Clone)]
struct FileHandleState {
    mode: i32,
    position: i32,
    len: i32,
    host_path: Option<PathBuf>,
}

fn pseudo_file_len_from_path_token(path: i32) -> i32 {
    let magnitude = path.saturating_abs();
    1 + (magnitude % 4096)
}

fn clamp_u64_to_i32(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

fn external_symbol_token(library: &str, alias: &str, name: &str) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in library
        .bytes()
        .chain([b'!'])
        .chain(alias.bytes())
        .chain([b'!'])
        .chain(name.bytes())
    {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    (hash & 0x7fff_ffff).max(1) as i32
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use crate::{
        error::HalErrorKind,
        model::{HalProfileId, HostPolicy},
        traits::{
            ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, ProcessEnvHal,
            TimeLocaleHal, UiInteractionHal,
        },
    };

    use super::StandardHostServices;

    fn current_native_profile() -> Option<HalProfileId> {
        if cfg!(target_os = "windows") {
            Some(HalProfileId::Windows)
        } else if cfg!(target_os = "linux") {
            Some(HalProfileId::Linux)
        } else {
            None
        }
    }

    #[test]
    fn file_open_seek_eof_lof_close_roundtrip() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host.open(77, 0).expect("open should succeed");
        assert_eq!(handle, 1);
        assert_eq!(host.eof(handle).expect("eof should work"), 0);
        assert!(host.lof(handle).expect("lof should work") > 0);
        host.seek(handle, host.lof(handle).expect("lof should work"))
            .expect("seek to end should work");
        assert_eq!(host.eof(handle).expect("eof should work"), 1);
        assert_eq!(host.close(handle).expect("close should work"), 1);
    }

    #[test]
    fn free_file_respects_low_and_high_ranges() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.free_file(0).expect("default free file"), 1);
        assert_eq!(host.free_file(1).expect("high-range free file"), 256);
    }

    #[test]
    fn close_releases_handle_for_reuse() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host.open(10, 0).expect("open should succeed");
        assert_eq!(first, 1);
        host.close(first).expect("close should succeed");
        let second = host.open(11, 0).expect("second open should succeed");
        assert_eq!(second, 1, "closed handles must be reusable");
    }

    #[test]
    fn free_file_low_range_tracks_allocated_handles() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let mut handles = Vec::new();
        for expected in 1..=8 {
            assert_eq!(
                host.free_file(0).expect("free_file should succeed"),
                expected
            );
            handles.push(host.open(expected, 0).expect("open should succeed"));
        }
        assert_eq!(handles, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn seek_negative_returns_adapter_fault() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let handle = host.open(42, 0).expect("open should succeed");
        let err = host
            .seek(handle, -1)
            .expect_err("negative seek should return error");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
    }

    #[test]
    fn ui_virtualization_modes_follow_contract() {
        let policy = HostPolicy {
            allow_interaction: true,
            ui_virtualization: crate::model::UiVirtualizationMode::ScriptedResponses,
            ..HostPolicy::default()
        };
        let host = StandardHostServices::new(HalProfileId::Windows, policy.clone());
        assert_eq!(host.msg_box(100, 3).expect("msg_box"), 3);
        assert_eq!(host.input_box(100, 7).expect("input_box"), 7);

        let host_disabled = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                ui_virtualization: crate::model::UiVirtualizationMode::Disabled,
                ..policy
            },
        );
        assert_eq!(host_disabled.msg_box(100, 3).expect("msg_box"), 100);
        assert_eq!(host_disabled.input_box(100, 7).expect("input_box"), 100);
    }

    #[test]
    fn ui_fail_on_prompt_returns_policy_denied() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_interaction: true,
                ui_virtualization: crate::model::UiVirtualizationMode::FailOnPrompt,
                ..HostPolicy::default()
            },
        );
        let err = host.msg_box(9, 1).expect_err("msg_box should be denied");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        let err = host
            .input_box(9, 1)
            .expect_err("input_box should be denied");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
    }

    #[test]
    fn process_com_dynlink_policy_denials_are_enforced() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_process_spawn: false,
                allow_com_activation: false,
                allow_dynamic_link: false,
                ..HostPolicy::default()
            },
        );

        assert_eq!(
            host.shell(1, 0).expect_err("shell deny").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.create_object(1).expect_err("com deny").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            host.invoke_symbol(1, 2).expect_err("dynlink deny").kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn time_locale_contract_values_are_stable() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.date_serial_now().expect("date"), 20_260_301);
        assert_eq!(host.time_serial_now().expect("time"), 123_456);
        assert_eq!(host.timer_ticks().expect("timer"), 42);
    }

    #[test]
    fn process_env_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.environ(88).expect("environ"), 88);
        assert_eq!(host.dir(0, 0).expect("dir"), 0);
        assert_eq!(host.dir(5, 0).expect("dir"), 1);
    }

    #[test]
    fn dispatch_invoke_deterministic_projection_contract() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.dispatch_invoke(10, 20, 30).expect("dispatch"), 60);
    }

    #[test]
    fn diagnostics_emit_contract_is_deterministic() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(host.emit(4, 5).expect("emit"), 9);
    }

    #[test]
    fn event_pump_supported_and_unsupported_paths() {
        let windows = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        assert_eq!(windows.do_events().expect("windows do_events"), 0);

        let null = StandardHostServices::new(HalProfileId::Null, HostPolicy::default());
        let err = null
            .do_events()
            .expect_err("null do_events should be unsupported");
        assert_eq!(err.kind, HalErrorKind::CapabilityUnavailable);
    }

    #[test]
    fn file_open_denied_has_no_state_side_effects() {
        let host = StandardHostServices::new(
            HalProfileId::Windows,
            HostPolicy {
                allow_filesystem_mutation: false,
                ..HostPolicy::default()
            },
        );
        assert_eq!(host.free_file(0).expect("first free should be 1"), 1);
        let err = host
            .open(10, 1)
            .expect_err("mutation open should be denied by policy");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        assert_eq!(
            host.free_file(0)
                .expect("free file should remain unchanged"),
            1
        );
    }

    #[test]
    fn invalid_close_does_not_mutate_handle_state() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let first = host.open(10, 0).expect("open should succeed");
        assert_eq!(first, 1);
        let err = host.close(99).expect_err("invalid close should fail");
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert_eq!(
            host.free_file(0)
                .expect("free file should still skip handle 1"),
            2
        );
    }

    #[test]
    fn ui_msg_box_enforces_policy_and_capability_failures() {
        let denied_host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
        let err = denied_host
            .msg_box(1, 1)
            .expect_err("interaction is denied by default policy");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);

        let null_host = StandardHostServices::new(
            HalProfileId::Null,
            HostPolicy {
                allow_interaction: true,
                ..HostPolicy::default()
            },
        );
        let err = null_host
            .msg_box(1, 1)
            .expect_err("null profile should report unsupported capability");
        assert_eq!(err.kind, HalErrorKind::CapabilityUnavailable);
    }

    #[test]
    fn null_profile_support_set_is_explicit() {
        let host = StandardHostServices::new(HalProfileId::Null, HostPolicy::default());
        let descriptor = host.descriptor();
        assert!(!descriptor.supports(crate::model::CapabilityId::UiInteraction));
        assert!(!descriptor.supports(crate::model::CapabilityId::EventPump));
        assert!(!descriptor.supports(crate::model::CapabilityId::FileSystemIo));
        assert!(!descriptor.supports(crate::model::CapabilityId::ProcessEnv));
        assert!(!descriptor.supports(crate::model::CapabilityId::ComActivationDispatch));
        assert!(descriptor.supports(crate::model::CapabilityId::TimeLocale));
        assert!(!descriptor.supports(crate::model::CapabilityId::DynamicLinking));
        assert!(descriptor.supports(crate::model::CapabilityId::DiagnosticsTelemetry));
    }

    #[test]
    fn maturity_does_not_affect_policy_denial_shape() {
        let policy = HostPolicy {
            allow_process_spawn: false,
            ..HostPolicy::default()
        };
        let windows = StandardHostServices::new(HalProfileId::Windows, policy.clone());
        let linux = StandardHostServices::new(HalProfileId::Linux, policy);
        assert_eq!(
            windows.shell(1, 0).expect_err("windows shell denial").kind,
            HalErrorKind::PolicyDenied
        );
        assert_eq!(
            linux.shell(1, 0).expect_err("linux shell denial").kind,
            HalErrorKind::PolicyDenied
        );
    }

    #[test]
    fn native_mode_process_and_env_paths_are_callable() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        let shell = host.shell(1, 0).expect("native shell should succeed");
        assert!(shell >= 1);
        let environ = host.environ(3).expect("native environ should succeed");
        assert!(environ >= 0);
        let dir = host.dir(0, 0).expect("native dir should succeed");
        assert!(dir == 0 || dir == 1);
    }

    #[test]
    fn native_mode_filesystem_seek_can_extend_length() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        let handle = host.open(31415, 1).expect("native open should succeed");
        host.seek(handle, 64).expect("native seek should succeed");
        assert!(
            host.lof(handle).expect("native lof should succeed") >= 64,
            "native seek in mutation mode should extend logical length"
        );
        assert_eq!(host.close(handle).expect("native close should succeed"), 1);
    }

    #[test]
    fn native_mode_time_tokens_are_non_negative() {
        let Some(profile) = current_native_profile() else {
            return;
        };
        let host = StandardHostServices::new(profile, HostPolicy::interactive_dev());
        assert!(host.date_serial_now().expect("date") >= 0);
        assert!(host.time_serial_now().expect("time") >= 0);
        assert!(host.timer_ticks().expect("ticks") >= 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_native_mode_persists_mutation_to_host_file() {
        let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::interactive_dev());
        let token = 424_242;
        let host_path = host.host_path_from_token(token);
        let _ = std::fs::remove_file(&host_path);

        let handle = host.open(token, 1).expect("native open should succeed");
        host.seek(handle, 160).expect("native seek should succeed");
        assert_eq!(host.close(handle).expect("native close should succeed"), 1);

        let metadata = std::fs::metadata(&host_path).expect("host-backed file should exist");
        assert!(
            metadata.len() >= 160,
            "host-backed file should reflect seek growth"
        );
    }

    proptest! {
        #[test]
        fn prop_free_file_low_range_tracks_open_count(path_seed in 1i32..10_000, open_count in 0usize..32) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            for idx in 0..open_count {
                let path = path_seed.saturating_add(idx as i32);
                let _ = host.open(path, 0).expect("open should succeed");
            }
            let expected = 1 + open_count as i32;
            let free = host.free_file(0).expect("free_file should succeed");
            prop_assert_eq!(free, expected);
        }

        #[test]
        fn prop_seek_eof_boundary(path_token in 1i32..10_000, offset in 0i32..6000) {
            let host = StandardHostServices::new(HalProfileId::Windows, HostPolicy::default());
            let handle = host.open(path_token, 0).expect("open should succeed");
            let len = host.lof(handle).expect("lof should succeed");
            host.seek(handle, offset).expect("seek should succeed");
            let eof = host.eof(handle).expect("eof should succeed");
            let expected = if offset >= len { 1 } else { 0 };
            prop_assert_eq!(eof, expected);
        }

        #[test]
        fn prop_ui_virtualization_projection_is_stable(prompt in any::<i32>(), style in any::<i32>(), default_value in any::<i32>()) {
            let scripted = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: true,
                    ui_virtualization: crate::model::UiVirtualizationMode::ScriptedResponses,
                    ..HostPolicy::default()
                },
            );
            prop_assert_eq!(
                scripted.msg_box(prompt, style).expect("scripted msg_box"),
                style.max(1)
            );
            prop_assert_eq!(
                scripted.input_box(prompt, default_value).expect("scripted input_box"),
                default_value
            );

            let disabled = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: true,
                    ui_virtualization: crate::model::UiVirtualizationMode::Disabled,
                    ..HostPolicy::default()
                },
            );
            prop_assert_eq!(
                disabled.msg_box(prompt, style).expect("disabled msg_box"),
                prompt.max(1)
            );
            prop_assert_eq!(
                disabled.input_box(prompt, default_value).expect("disabled input_box"),
                prompt
            );
        }

        #[test]
        fn prop_host_sensitive_policy_denials_are_stable(
            shell_cmd in any::<i32>(),
            prog_id in any::<i32>(),
            symbol in any::<i32>(),
            arg in any::<i32>()
        ) {
            let host = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: false,
                    allow_process_spawn: false,
                    allow_com_activation: false,
                    allow_dynamic_link: false,
                    ..HostPolicy::default()
                },
            );

            prop_assert_eq!(
                host.msg_box(1, 1).expect_err("msg_box denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.shell(shell_cmd, 0).expect_err("shell denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.create_object(prog_id).expect_err("create_object denied").kind,
                HalErrorKind::PolicyDenied
            );
            prop_assert_eq!(
                host.invoke_symbol(symbol, arg).expect_err("invoke_symbol denied").kind,
                HalErrorKind::PolicyDenied
            );
        }

        #[test]
        fn prop_process_com_dynlink_projection_is_stable(
            shell_cmd in any::<i32>(),
            prog_id in any::<i32>(),
            object in any::<i32>(),
            member in any::<i32>(),
            arg in any::<i32>(),
            symbol in any::<i32>()
        ) {
            let host = StandardHostServices::new(
                HalProfileId::Windows,
                HostPolicy {
                    allow_interaction: true,
                    allow_process_spawn: true,
                    allow_com_activation: true,
                    allow_dynamic_link: true,
                    ..HostPolicy::default()
                },
            );

            let shell_expected = if shell_cmd == 0 { 0 } else { 1 };
            prop_assert_eq!(
                host.shell(shell_cmd, 0).expect("shell should succeed"),
                shell_expected
            );
            prop_assert_eq!(
                host.create_object(prog_id).expect("create_object should succeed"),
                5_000i32.saturating_add(prog_id)
            );
            prop_assert_eq!(
                host.dispatch_invoke(object, member, arg)
                    .expect("dispatch_invoke should succeed"),
                object.saturating_add(member).saturating_add(arg)
            );
            prop_assert_eq!(
                host.invoke_symbol(symbol, arg).expect("invoke_symbol should succeed"),
                symbol.saturating_add(arg)
            );
        }
    }
}
