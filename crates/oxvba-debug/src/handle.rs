use std::sync::Arc;

use crossbeam_channel::unbounded;
use oxvba_host::{
    DirectHostBreakpointId, DirectHostDebugSessionId, DirectHostStackFrameId, DirectHostWatchId,
};

use crate::{
    errors::DebugError,
    events::DebugEventReceiver,
    views::{
        DebugBreakpointView, DebugPauseView, DebugRunResultView, DebugValueView, DebugWatchView,
    },
};

#[derive(Debug)]
struct HandleInner {
    session_id: DirectHostDebugSessionId,
}

/// Consumer-facing debug handle.
///
/// The skeleton stores only `Send + Sync` fields; B05 replaces the placeholder
/// methods with worker-backed command marshalling.
#[derive(Debug, Clone)]
pub struct DebugSessionHandle {
    inner: Arc<HandleInner>,
}

impl DebugSessionHandle {
    pub fn start(&self) -> Result<DebugRunResultView, DebugError> {
        Err(DebugError::UnsupportedCommand("start"))
    }

    pub fn step_into(&self) -> Result<DebugRunResultView, DebugError> {
        Err(DebugError::UnsupportedCommand("step_into"))
    }

    pub fn step_over(&self) -> Result<DebugRunResultView, DebugError> {
        Err(DebugError::UnsupportedCommand("step_over"))
    }

    pub fn step_out(&self) -> Result<DebugRunResultView, DebugError> {
        Err(DebugError::UnsupportedCommand("step_out"))
    }

    pub fn continue_execution(&self) -> Result<DebugRunResultView, DebugError> {
        Err(DebugError::UnsupportedCommand("continue_execution"))
    }

    pub fn set_source_breakpoint(
        &self,
        _module: &str,
        _file_line: u32,
        _enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError> {
        Err(DebugError::UnsupportedCommand("set_source_breakpoint"))
    }

    pub fn set_breakpoint_enabled(
        &self,
        id: &DirectHostBreakpointId,
        _enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError> {
        Err(DebugError::UnknownBreakpoint(id.clone()))
    }

    pub fn clear_source_breakpoint(&self, id: &DirectHostBreakpointId) -> Result<(), DebugError> {
        Err(DebugError::UnknownBreakpoint(id.clone()))
    }

    pub fn breakpoints(&self) -> Result<Vec<DebugBreakpointView>, DebugError> {
        Ok(Vec::new())
    }

    pub fn add_watch(&self, _expression: &str) -> Result<DebugWatchView, DebugError> {
        Err(DebugError::UnsupportedCommand("add_watch"))
    }

    pub fn update_watch(
        &self,
        id: &DirectHostWatchId,
        _expression: &str,
    ) -> Result<DebugWatchView, DebugError> {
        Err(DebugError::UnknownWatch(id.clone()))
    }

    pub fn remove_watch(&self, id: &DirectHostWatchId) -> Result<(), DebugError> {
        Err(DebugError::UnknownWatch(id.clone()))
    }

    pub fn evaluate_watches(&self) -> Result<Vec<DebugWatchView>, DebugError> {
        Ok(Vec::new())
    }

    pub fn current_pause(&self) -> Result<Option<DebugPauseView>, DebugError> {
        Ok(None)
    }

    pub fn stack_frames(&self) -> Result<Vec<crate::views::DebugFrameView>, DebugError> {
        Ok(Vec::new())
    }

    pub fn frame_locals(
        &self,
        frame: &DirectHostStackFrameId,
    ) -> Result<Vec<DebugValueView>, DebugError> {
        Err(DebugError::UnknownFrame(frame.clone()))
    }

    pub fn evaluate(
        &self,
        frame: Option<&DirectHostStackFrameId>,
        _expression: &str,
    ) -> Result<DebugValueView, DebugError> {
        match frame {
            Some(frame) => Err(DebugError::UnknownFrame(frame.clone())),
            None => Err(DebugError::NotPaused),
        }
    }

    pub fn subscribe(&self) -> DebugEventReceiver {
        let (_tx, rx) = unbounded();
        DebugEventReceiver::new(rx)
    }

    pub fn session_id(&self) -> &DirectHostDebugSessionId {
        &self.inner.session_id
    }

    pub fn detach(self) -> Result<(), DebugError> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct DebugSessionAttach {
    pub handle: DebugSessionHandle,
    pub events: DebugEventReceiver,
}
