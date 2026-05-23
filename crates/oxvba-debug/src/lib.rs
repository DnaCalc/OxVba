//! Canonical OxVba debugger surface.
//!
//! This crate is introduced by the oxvba-debug handle workset. B01 provides
//! only the public module skeleton and transport-safe placeholder types; later
//! beads move the stateful core out of `oxvba-host` and wire the worker,
//! events, source maps, COM apartment handling, and async surface.

mod async_handle;
pub mod com_apartment;
pub mod command;
pub mod config;
pub mod core;
pub mod errors;
pub mod events;
pub mod handle;
pub mod records;
pub mod source_map;
pub mod views;
pub mod worker;

use std::sync::Arc;

use oxvba_compiler::ProjectManifest;
use oxvba_host::Engine;

pub use config::{
    DebugAttachConfig, DebugComApartment, DebugCoreConfig, DebugEventChannelMode,
    DebugOutputCaptureMode, DebugStartMode,
};
pub use core::{DebugCoreRunResult, DebugSessionCore};
pub use errors::{DebugAttachError, DebugError};
pub use events::{DebugEvent, DebugEventReceiver};
pub use handle::{DebugSessionAttach, DebugSessionHandle};
pub use records::{
    DebugBreakpointRecord, DebugEvaluationRequest, DebugSessionCommandStatus, DebugWatchRecord,
};
pub use views::{
    DebugBreakpointView, DebugExitView, DebugFrameView, DebugModuleView, DebugPauseView,
    DebugRunResultView, DebugSourceLocationView, DebugStopReasonView, DebugValueView,
    DebugWatchView,
};

/// Prepare the raw, stateful debugger core.
///
/// B03 replaces this B01 placeholder with the moved host debugger core.
pub fn prepare_debug_session_core(
    _engine: Arc<Engine>,
    _manifest: ProjectManifest,
    _config: DebugCoreConfig,
) -> Result<DebugSessionCore, DebugAttachError> {
    Err(DebugAttachError::Unsupported(
        "DebugSessionCore is introduced by B03",
    ))
}

/// Attach to a debug session through the consumer-facing handle.
///
/// B05 replaces this B01 placeholder with the worker-backed handle attach path.
pub fn attach_debug_session(
    _engine: Arc<Engine>,
    _manifest: ProjectManifest,
    _config: DebugAttachConfig,
) -> Result<DebugSessionAttach, DebugAttachError> {
    Err(DebugAttachError::Unsupported(
        "DebugSessionHandle is introduced by B05",
    ))
}
