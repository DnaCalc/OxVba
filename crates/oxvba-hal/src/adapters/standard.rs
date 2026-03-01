use crate::{
    error::{HalError, HalResult},
    model::{
        CapabilityDescriptor, CapabilityId, CapabilityMaturity, HalDescriptor, HalProfileId,
        HostPolicy, UiVirtualizationMode,
    },
    traits::{
        ComHal, DiagnosticsHal, DynamicLinkHal, EventPumpHal, FileSystemHal, ProcessEnvHal,
        TimeLocaleHal, UiInteractionHal,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone)]
pub(crate) struct StandardHostServices {
    profile: HalProfileId,
    descriptor: HalDescriptor,
    policy: HostPolicy,
    fs_state: Arc<Mutex<FileSystemState>>,
}

impl StandardHostServices {
    pub(crate) fn new(profile: HalProfileId, policy: HostPolicy) -> Self {
        Self {
            profile,
            descriptor: HalDescriptor {
                profile,
                contract_version: "hal-v1",
                adapter_version: "0.1.0",
                capabilities: capability_matrix(profile),
            },
            policy,
            fs_state: Arc::new(Mutex::new(FileSystemState::default())),
        }
    }

    pub(crate) fn profile(&self) -> HalProfileId {
        self.profile
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
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "msg_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(style.max(1)),
            UiVirtualizationMode::Disabled => Ok(prompt.max(1)),
        }
    }

    fn input_box(&self, prompt: i32, default_value: i32) -> HalResult<i32> {
        let capability = CapabilityId::UiInteraction;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_box"));
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(capability, "input_box"));
        }
        match self.policy.ui_virtualization {
            UiVirtualizationMode::FailOnPrompt => Err(self.denied(capability, "input_box")),
            UiVirtualizationMode::ScriptedResponses => Ok(default_value),
            UiVirtualizationMode::Disabled => Ok(prompt),
        }
    }
}

impl EventPumpHal for StandardHostServices {
    fn do_events(&self) -> HalResult<i32> {
        let capability = CapabilityId::EventPump;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "do_events"));
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
        let Some(handle) = state.first_free_in(1, 511) else {
            return Err(HalError::adapter_fault(
                self.profile,
                capability,
                "open",
                "no free file handles available in supported range",
            ));
        };
        let initial_len = if mode == 0 {
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
            },
        );
        Ok(handle)
    }

    fn close(&self, handle: i32) -> HalResult<i32> {
        let capability = CapabilityId::FileSystemIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "close"));
        }
        let mut state = self.fs_lock(capability, "close")?;
        if state.handles.remove(&handle).is_some() {
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
        let entry = self.fs_entry_mut(&mut state, handle, "seek")?;
        entry.position = position;
        if position > entry.len && entry.mode != 0 && self.policy.allow_filesystem_mutation {
            entry.len = position;
        }
        Ok(entry.position)
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
        state.first_free_in(start, end).ok_or_else(|| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "free_file",
                format!("no free file number in range {start}..={end}"),
            )
        })
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
        Ok(if command == 0 { 0 } else { 1 })
    }

    fn environ(&self, key: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "environ"));
        }
        Ok(key)
    }

    fn dir(&self, path: i32, _attrs: i32) -> HalResult<i32> {
        let capability = CapabilityId::ProcessEnv;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dir"));
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
        Ok(5_000 + prog_id)
    }

    fn dispatch_invoke(&self, object: i32, member: i32, arg: i32) -> HalResult<i32> {
        let capability = CapabilityId::ComActivationDispatch;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "dispatch_invoke"));
        }
        if !self.policy.allow_com_activation {
            return Err(self.denied(capability, "dispatch_invoke"));
        }
        Ok(object + member + arg)
    }
}

impl TimeLocaleHal for StandardHostServices {
    fn date_serial_now(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "date_serial_now"));
        }
        Ok(20_260_301)
    }

    fn time_serial_now(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "time_serial_now"));
        }
        Ok(1_234_56)
    }

    fn timer_ticks(&self) -> HalResult<i32> {
        let capability = CapabilityId::TimeLocale;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "timer_ticks"));
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
        Ok(symbol + arg)
    }
}

impl DiagnosticsHal for StandardHostServices {
    fn emit(&self, code: i32, payload: i32) -> HalResult<i32> {
        let capability = CapabilityId::DiagnosticsTelemetry;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "emit"));
        }
        Ok(code.saturating_add(payload))
    }
}

fn capability_matrix(profile: HalProfileId) -> Vec<CapabilityDescriptor> {
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
        HalProfileId::Wasm => {
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
}

fn pseudo_file_len_from_path_token(path: i32) -> i32 {
    let magnitude = path.saturating_abs();
    1 + (magnitude % 4096)
}

#[cfg(test)]
mod tests {
    use crate::{
        model::{HalProfileId, HostPolicy},
        traits::FileSystemHal,
    };

    use super::StandardHostServices;

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
}
