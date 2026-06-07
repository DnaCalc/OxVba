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

use crate::{events::DebugEventHub, worker::spawn_debug_worker};

pub use com_apartment::{DebugWorkerApartmentKind, DebugWorkerApartmentReport};
pub use config::{
    DebugAttachConfig, DebugComApartment, DebugCoreConfig, DebugEventChannelMode,
    DebugOutputCaptureMode, DebugStartMode,
};
pub use core::{
    DebugBreakpointBindingStatus, DebugBreakpointRecord, DebugBreakpointUnresolvedReason,
    DebugCoreRunResult, DebugEvaluationRequest, DebugFrameValueKind, DebugFrameVariant,
    DebugFrameVariantValue, DebugPauseState, DebugSessionCommandStatus, DebugSessionCore,
    DebugSessionError, DebugVariantEvaluationResult, DebugVariantPauseState, DebugWatchEvaluation,
    DebugWatchEvaluationStatus, DebugWatchRecord, HostDebugVariantRunResult,
};
pub use errors::{DebugAttachError, DebugError};
pub use events::{
    DebugBreakpointChangeKind, DebugEvent, DebugEventDelivery, DebugEventLag, DebugEventReceiver,
    DebugEventRecvError, DebugOutputChannel,
};
pub use handle::{DebugSessionAttach, DebugSessionHandle};
pub use source_map::DebugSourceMap;
pub use views::{
    DebugBreakpointBindingStatusView, DebugBreakpointView, DebugExitView, DebugFrameView,
    DebugModuleView, DebugPauseView, DebugRunResultView, DebugSourceLocationView,
    DebugStopReasonView, DebugValueKindView, DebugValueView, DebugWatchStatusView, DebugWatchView,
    breakpoint_view_from_core, frame_view_from_core, pause_view_from_core,
    run_result_view_from_core, value_view_from_core, watch_view_from_core,
};

/// Prepare the raw, stateful debugger core.
///
/// B03 replaces this B01 placeholder with the moved host debugger core.
pub fn prepare_debug_session_core(
    _engine: Arc<Engine>,
    _manifest: ProjectManifest,
    _config: DebugCoreConfig,
) -> Result<DebugSessionCore, DebugAttachError> {
    let runtime = _engine
        .compile_and_prepare_session(&_manifest)
        .map_err(|diagnostic| DebugAttachError::Prepare {
            message: diagnostic.to_string(),
        })?;
    Ok(DebugSessionCore::new(_engine, _manifest, runtime))
}

/// Attach to a debug session through the consumer-facing handle.
pub fn attach_debug_session(
    engine: Arc<Engine>,
    manifest: ProjectManifest,
    config: DebugAttachConfig,
) -> Result<DebugSessionAttach, DebugAttachError> {
    let events = DebugEventHub::new(config.event_channel);
    let initial_events = events.subscribe();
    let worker = spawn_debug_worker(engine, manifest, config, events.clone())?;
    Ok(DebugSessionAttach {
        handle: DebugSessionHandle::new(
            worker.session_id,
            worker.commands,
            worker.join,
            worker.failure,
            events,
        ),
        events: initial_events,
    })
}
