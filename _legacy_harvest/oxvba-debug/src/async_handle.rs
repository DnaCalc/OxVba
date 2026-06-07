#[cfg(feature = "tokio")]
use oxvba_host::{DirectHostBreakpointId, DirectHostStackFrameId, DirectHostWatchId};

#[cfg(feature = "tokio")]
use crate::{
    com_apartment::DebugWorkerApartmentReport,
    errors::DebugError,
    handle::DebugSessionHandle,
    views::{
        DebugBreakpointView, DebugFrameView, DebugPauseView, DebugRunResultView, DebugValueView,
        DebugWatchView,
    },
};

#[cfg(feature = "tokio")]
async fn run_blocking<F, T>(operation: F) -> Result<T, DebugError>
where
    F: FnOnce() -> Result<T, DebugError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|join_error| DebugError::WorkerFailed {
            stage: "async join",
            message: join_error.to_string(),
        })?
}

#[cfg(feature = "tokio")]
impl DebugSessionHandle {
    pub async fn start_async(&self) -> Result<DebugRunResultView, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.start()).await
    }

    pub async fn step_into_async(&self) -> Result<DebugRunResultView, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.step_into()).await
    }

    pub async fn step_over_async(&self) -> Result<DebugRunResultView, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.step_over()).await
    }

    pub async fn step_out_async(&self) -> Result<DebugRunResultView, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.step_out()).await
    }

    pub async fn continue_execution_async(&self) -> Result<DebugRunResultView, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.continue_execution()).await
    }

    pub async fn set_source_breakpoint_async(
        &self,
        module: &str,
        file_line: u32,
        enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError> {
        let handle = self.clone();
        let module = module.to_owned();
        run_blocking(move || handle.set_source_breakpoint(&module, file_line, enabled)).await
    }

    pub async fn set_breakpoint_enabled_async(
        &self,
        id: &DirectHostBreakpointId,
        enabled: bool,
    ) -> Result<DebugBreakpointView, DebugError> {
        let handle = self.clone();
        let id = id.clone();
        run_blocking(move || handle.set_breakpoint_enabled(&id, enabled)).await
    }

    pub async fn clear_source_breakpoint_async(
        &self,
        id: &DirectHostBreakpointId,
    ) -> Result<(), DebugError> {
        let handle = self.clone();
        let id = id.clone();
        run_blocking(move || handle.clear_source_breakpoint(&id)).await
    }

    pub async fn breakpoints_async(&self) -> Result<Vec<DebugBreakpointView>, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.breakpoints()).await
    }

    pub async fn add_watch_async(&self, expression: &str) -> Result<DebugWatchView, DebugError> {
        let handle = self.clone();
        let expression = expression.to_owned();
        run_blocking(move || handle.add_watch(&expression)).await
    }

    pub async fn update_watch_async(
        &self,
        id: &DirectHostWatchId,
        expression: &str,
    ) -> Result<DebugWatchView, DebugError> {
        let handle = self.clone();
        let id = id.clone();
        let expression = expression.to_owned();
        run_blocking(move || handle.update_watch(&id, &expression)).await
    }

    pub async fn remove_watch_async(&self, id: &DirectHostWatchId) -> Result<(), DebugError> {
        let handle = self.clone();
        let id = id.clone();
        run_blocking(move || handle.remove_watch(&id)).await
    }

    pub async fn evaluate_watches_async(&self) -> Result<Vec<DebugWatchView>, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.evaluate_watches()).await
    }

    pub async fn current_pause_async(&self) -> Result<Option<DebugPauseView>, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.current_pause()).await
    }

    pub async fn stack_frames_async(&self) -> Result<Vec<DebugFrameView>, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.stack_frames()).await
    }

    pub async fn frame_locals_async(
        &self,
        frame: &DirectHostStackFrameId,
    ) -> Result<Vec<DebugValueView>, DebugError> {
        let handle = self.clone();
        let frame = frame.clone();
        run_blocking(move || handle.frame_locals(&frame)).await
    }

    pub async fn evaluate_async(
        &self,
        frame: Option<&DirectHostStackFrameId>,
        expression: &str,
    ) -> Result<DebugValueView, DebugError> {
        let handle = self.clone();
        let frame = frame.cloned();
        let expression = expression.to_owned();
        run_blocking(move || handle.evaluate(frame.as_ref(), &expression)).await
    }

    pub async fn report_worker_apartment_async(
        &self,
    ) -> Result<DebugWorkerApartmentReport, DebugError> {
        let handle = self.clone();
        run_blocking(move || handle.report_worker_apartment()).await
    }

    pub async fn detach_async(self) -> Result<(), DebugError> {
        run_blocking(move || self.detach()).await
    }
}
