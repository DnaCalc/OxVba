//! Non-Windows stub implementation of DynamicObjectBridge.
//!
//! Provides deterministic error returns for all COM operations,
//! allowing `cargo check --target x86_64-unknown-linux-gnu` to compile.

use crate::dynamic_object::{
    DynamicCallRequest, DynamicEventPayload, DynamicObjectBridge,
    DynamicObjectToken, DynamicValue,
};

/// A stub DynamicObjectBridge that returns errors for all operations.
/// Used on non-Windows platforms where COM is unavailable.
pub struct StubDynamicObjectBridge;

impl DynamicObjectBridge for StubDynamicObjectBridge {
    type Error = String;

    fn invoke_dynamic(
        &self,
        request: &DynamicCallRequest,
    ) -> Result<DynamicValue, Self::Error> {
        Err(format!(
            "COM not available on this platform: cannot invoke on object {:?}",
            request.object
        ))
    }

    fn poll_dynamic_event(&self) -> Result<Option<DynamicEventPayload>, Self::Error> {
        Ok(None)
    }

    fn release_dynamic_object(
        &self,
        _object: DynamicObjectToken,
    ) -> Result<DynamicValue, Self::Error> {
        Err("COM not available on this platform: cannot release object".to_string())
    }
}
