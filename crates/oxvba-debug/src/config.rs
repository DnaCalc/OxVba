/// Configuration for the raw debug core.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugCoreConfig {
    /// Reserved for future core-only options.
    pub preserve_runtime_line_basis: bool,
}

/// Worker COM-apartment selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugComApartment {
    Sta,
    Mta,
    None,
}

/// Event-channel backpressure policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugEventChannelMode {
    Bounded(usize),
    Unbounded,
}

/// Debug output capture policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOutputCaptureMode {
    Disabled,
    DebugPrintAndStdio,
}

/// Initial execution policy for a newly attached session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugStartMode {
    Manual,
    StopOnEntry,
}

/// Consumer-facing handle attach configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugAttachConfig {
    pub com_apartment: DebugComApartment,
    pub event_channel: DebugEventChannelMode,
    pub start_mode: DebugStartMode,
    pub output_capture: DebugOutputCaptureMode,
}

impl Default for DebugAttachConfig {
    fn default() -> Self {
        Self {
            com_apartment: DebugComApartment::Sta,
            event_channel: DebugEventChannelMode::Bounded(256),
            start_mode: DebugStartMode::Manual,
            output_capture: DebugOutputCaptureMode::DebugPrintAndStdio,
        }
    }
}
