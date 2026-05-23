#[cfg(feature = "tokio")]
use crate::{errors::DebugError, handle::DebugSessionHandle, views::DebugRunResultView};

#[cfg(feature = "tokio")]
impl DebugSessionHandle {
    pub async fn step_into_async(&self) -> Result<DebugRunResultView, DebugError> {
        self.step_into()
    }
}
