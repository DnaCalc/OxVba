use std::{
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_channel::{Sender, bounded};
use oxvba_host::{
    DirectHostBreakpointId, DirectHostDebugSessionId, DirectHostStackFrameId, DirectHostWatchId,
};

use crate::{
    com_apartment::DebugWorkerApartmentReport,
    command::{CommandReply, DebugCommand},
    errors::DebugError,
    events::{DebugEventHub, DebugEventReceiver},
    views::{
        DebugBreakpointView, DebugFrameView, DebugPauseView, DebugRunResultView, DebugValueView,
        DebugWatchView,
    },
};

#[derive(Debug)]
struct HandleInner {
    session_id: DirectHostDebugSessionId,
    commands: Sender<DebugCommand>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    failure: Arc<Mutex<Option<DebugError>>>,
    events: DebugEventHub,
}

/// Consumer-facing debug handle.
///
/// The handle stores only `Send + Sync` primitives and marshals all debugger
/// state access to the worker thread that owns `DebugSessionCore`.
#[derive(Debug, Clone)]
pub struct DebugSessionHandle {
    inner: Arc<HandleInner>,
}

impl DebugSessionHandle {
    pub(crate) fn new(
        session_id: DirectHostDebugSessionId,
        commands: Sender<DebugCommand>,
        worker: thread::JoinHandle<()>,
        failure: Arc<Mutex<Option<DebugError>>>,
        events: DebugEventHub,
    ) -> Self {
        Self {
            inner: Arc::new(HandleInner {
                session_id,
                commands,
                worker: Mutex::new(Some(worker)),
                failure,
                events,
            }),
        }
    }

    pub fn start(&self) -> Result<DebugRunResultView, DebugError> {
        self.request(DebugCommand::Start)
    }

    pub fn step_into(&self) -> Result<DebugRunResultView, DebugError> {
        self.request(DebugCommand::StepInto)
    }

    pub fn step_over(&self) -> Result<DebugRunResultView, DebugError> {
        self.request(DebugCommand::StepOver)
    }

    pub fn step_out(&self) -> Result<DebugRunResultView, DebugError> {
        self.request(DebugCommand::StepOut)
    }

    pub fn continue_execution(&self) -> Result<DebugRunResultView, DebugError> {
        self.request(DebugCommand::Continue)
    }

    pub fn set_source_breakpoint(
        &self,
        module: &str,
        file_line: u32,
        enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError> {
        let module = module.to_string();
        self.request(|reply| DebugCommand::SetSourceBreakpoint {
            module,
            file_line,
            enabled,
            reply,
        })
    }

    pub fn set_breakpoint_enabled(
        &self,
        id: &DirectHostBreakpointId,
        enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError> {
        let id = id.clone();
        self.request(|reply| DebugCommand::SetBreakpointEnabled { id, enabled, reply })
    }

    pub fn clear_source_breakpoint(&self, id: &DirectHostBreakpointId) -> Result<(), DebugError> {
        let id = id.clone();
        self.request(|reply| DebugCommand::ClearSourceBreakpoint { id, reply })
    }

    pub fn breakpoints(&self) -> Result<Vec<DebugBreakpointView>, DebugError> {
        self.request(DebugCommand::Breakpoints)
    }

    pub fn add_watch(&self, expression: &str) -> Result<DebugWatchView, DebugError> {
        let expression = expression.to_string();
        self.request(|reply| DebugCommand::AddWatch { expression, reply })
    }

    pub fn update_watch(
        &self,
        id: &DirectHostWatchId,
        expression: &str,
    ) -> Result<DebugWatchView, DebugError> {
        let id = id.clone();
        let expression = expression.to_string();
        self.request(|reply| DebugCommand::UpdateWatch {
            id,
            expression,
            reply,
        })
    }

    pub fn remove_watch(&self, id: &DirectHostWatchId) -> Result<(), DebugError> {
        let id = id.clone();
        self.request(|reply| DebugCommand::RemoveWatch { id, reply })
    }

    pub fn evaluate_watches(&self) -> Result<Vec<DebugWatchView>, DebugError> {
        self.request(DebugCommand::EvaluateWatches)
    }

    pub fn current_pause(&self) -> Result<Option<DebugPauseView>, DebugError> {
        self.request(DebugCommand::CurrentPause)
    }

    pub fn stack_frames(&self) -> Result<Vec<DebugFrameView>, DebugError> {
        self.request(DebugCommand::StackFrames)
    }

    pub fn frame_locals(
        &self,
        frame: &DirectHostStackFrameId,
    ) -> Result<Vec<DebugValueView>, DebugError> {
        let frame = frame.clone();
        self.request(|reply| DebugCommand::FrameLocals { frame, reply })
    }

    pub fn evaluate(
        &self,
        frame: Option<&DirectHostStackFrameId>,
        expression: &str,
    ) -> Result<DebugValueView, DebugError> {
        let frame = frame.cloned();
        let expression = expression.to_string();
        self.request(|reply| DebugCommand::Evaluate {
            frame,
            expression,
            reply,
        })
    }

    pub fn subscribe(&self) -> DebugEventReceiver {
        self.inner.events.subscribe()
    }

    pub fn report_worker_apartment(&self) -> Result<DebugWorkerApartmentReport, DebugError> {
        self.request(DebugCommand::ReportWorkerApartment)
    }

    pub fn session_id(&self) -> &DirectHostDebugSessionId {
        &self.inner.session_id
    }

    pub fn detach(self) -> Result<(), DebugError> {
        let outstanding = Arc::strong_count(&self.inner).saturating_sub(1);
        if outstanding > 0 {
            return Err(DebugError::OutstandingHandles { count: outstanding });
        }
        self.shutdown_and_join()
    }

    #[doc(hidden)]
    pub fn panic_worker_for_test(&self) -> Result<(), DebugError> {
        self.inner
            .commands
            .send(DebugCommand::PanicWorker)
            .map_err(|_| {
                self.recorded_failure()
                    .unwrap_or(DebugError::SessionAlreadyDetached)
            })
    }

    fn shutdown_and_join(&self) -> Result<(), DebugError> {
        match self.request(DebugCommand::Shutdown) {
            Ok(()) | Err(DebugError::SessionAlreadyDetached) => {}
            Err(err) => return Err(err),
        }
        self.join_worker()
    }

    fn join_worker(&self) -> Result<(), DebugError> {
        let join = self
            .inner
            .worker
            .lock()
            .map_err(|_| DebugError::Internal("debug worker join lock poisoned".to_string()))?
            .take();
        if let Some(join) = join {
            join.join().map_err(|panic| DebugError::WorkerFailed {
                stage: "join",
                message: panic_message(panic),
            })?;
        }
        if let Some(err) = self.recorded_failure() {
            return Err(err);
        }
        Ok(())
    }

    fn request<T>(
        &self,
        build: impl FnOnce(CommandReply<T>) -> DebugCommand,
    ) -> Result<T, DebugError> {
        if let Some(err) = self.recorded_failure() {
            return Err(err);
        }
        let (reply_tx, reply_rx) = bounded(1);
        self.inner
            .commands
            .send(build(reply_tx))
            .map_err(|_| self.error_after_worker_stopped(DebugError::SessionAlreadyDetached))?;
        reply_rx.recv().map_err(|_| {
            self.error_after_worker_stopped(DebugError::WorkerFailed {
                stage: "reply",
                message: "debug worker stopped before replying".to_string(),
            })
        })?
    }

    fn error_after_worker_stopped(&self, fallback: DebugError) -> DebugError {
        let _ = self.join_worker();
        self.recorded_failure().unwrap_or(fallback)
    }

    fn recorded_failure(&self) -> Option<DebugError> {
        self.inner
            .failure
            .lock()
            .ok()
            .and_then(|failure| failure.clone())
    }
}

impl Drop for HandleInner {
    fn drop(&mut self) {
        if let Ok(mut worker) = self.worker.lock() {
            if worker.is_some() {
                let (reply_tx, reply_rx) = bounded(1);
                let _ = self.commands.send(DebugCommand::Shutdown(reply_tx));
                let _ = reply_rx.recv();
                if let Some(join) = worker.take() {
                    let _ = join.join();
                }
            }
        }
    }
}

fn panic_message(panic: Box<dyn std::any::Any + Send + 'static>) -> String {
    if let Some(message) = panic.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

#[derive(Debug)]
pub struct DebugSessionAttach {
    pub handle: DebugSessionHandle,
    pub events: DebugEventReceiver,
}
