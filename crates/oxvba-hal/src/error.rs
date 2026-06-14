//! Deterministic HAL error taxonomy.

use crate::model::{CapabilityId, HalProfileId};
use thiserror::Error;

pub type HalResult<T> = Result<T, HalError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalErrorKind {
    CapabilityUnavailable,
    PolicyDenied,
    AdapterFault,
    UnsupportedProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "{kind:?} [{stable_code}] profile={profile:?} capability={capability:?} op={operation}: {message}"
)]
pub struct HalError {
    pub kind: HalErrorKind,
    pub stable_code: &'static str,
    pub profile: HalProfileId,
    pub capability: CapabilityId,
    pub operation: &'static str,
    pub message: String,
    /// The host-domain numeric error code this fault should surface to the
    /// embedder, when one is known.
    ///
    /// For COM dispatch faults this is the real VBA `Err.Number` derived from
    /// the underlying HRESULT / `EXCEPINFO` (e.g. 457 for a duplicate-key
    /// `Scripting.Dictionary.Add`). It is `None` for faults that have no
    /// host-specific number, in which case the VM falls back to VBA error 5
    /// ("invalid procedure call or argument").
    pub host_error_code: Option<i32>,
}

impl HalError {
    pub fn capability_unavailable(
        profile: HalProfileId,
        capability: CapabilityId,
        operation: &'static str,
    ) -> Self {
        Self {
            kind: HalErrorKind::CapabilityUnavailable,
            stable_code: "HAL-E-CAP-UNAVAILABLE",
            profile,
            capability,
            operation,
            message: "capability is not supported by active HAL profile".to_string(),
            host_error_code: None,
        }
    }

    pub fn policy_denied(
        profile: HalProfileId,
        capability: CapabilityId,
        operation: &'static str,
    ) -> Self {
        Self {
            kind: HalErrorKind::PolicyDenied,
            stable_code: "HAL-E-POLICY-DENIED",
            profile,
            capability,
            operation,
            message: "operation blocked by host policy".to_string(),
            host_error_code: None,
        }
    }

    pub fn adapter_fault(
        profile: HalProfileId,
        capability: CapabilityId,
        operation: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind: HalErrorKind::AdapterFault,
            stable_code: "HAL-E-ADAPTER-FAULT",
            profile,
            capability,
            operation,
            message: message.into(),
            host_error_code: None,
        }
    }

    /// Attach a host-domain numeric error code (e.g. a VBA `Err.Number`
    /// recovered from a COM dispatch failure) to this fault.
    #[must_use]
    pub fn with_host_error_code(mut self, code: i32) -> Self {
        self.host_error_code = Some(code);
        self
    }

    pub fn unsupported_profile(
        profile: HalProfileId,
        capability: CapabilityId,
        operation: &'static str,
    ) -> Self {
        Self {
            kind: HalErrorKind::UnsupportedProfile,
            stable_code: "HAL-E-UNSUPPORTED-PROFILE",
            profile,
            capability,
            operation,
            message: "operation is not implemented for the active profile".to_string(),
            host_error_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{CapabilityId, HalProfileId};

    use super::{HalError, HalErrorKind};

    #[test]
    fn capability_unavailable_factory_is_stable() {
        let err = HalError::capability_unavailable(
            HalProfileId::Null,
            CapabilityId::UiInteraction,
            "msg_box",
        );
        assert_eq!(err.kind, HalErrorKind::CapabilityUnavailable);
        assert_eq!(err.stable_code, "HAL-E-CAP-UNAVAILABLE");
        assert_eq!(err.profile, HalProfileId::Null);
        assert_eq!(err.capability, CapabilityId::UiInteraction);
        assert_eq!(err.operation, "msg_box");
        assert_eq!(
            err.message,
            "capability is not supported by active HAL profile"
        );
    }

    #[test]
    fn policy_denied_factory_is_stable() {
        let err = HalError::policy_denied(HalProfileId::Windows, CapabilityId::ProcessEnv, "shell");
        assert_eq!(err.kind, HalErrorKind::PolicyDenied);
        assert_eq!(err.stable_code, "HAL-E-POLICY-DENIED");
        assert_eq!(err.profile, HalProfileId::Windows);
        assert_eq!(err.capability, CapabilityId::ProcessEnv);
        assert_eq!(err.operation, "shell");
        assert_eq!(err.message, "operation blocked by host policy");
    }

    #[test]
    fn adapter_fault_factory_preserves_message() {
        let err = HalError::adapter_fault(
            HalProfileId::Linux,
            CapabilityId::DynamicLinking,
            "invoke_symbol",
            "descriptor mismatch",
        );
        assert_eq!(err.kind, HalErrorKind::AdapterFault);
        assert_eq!(err.stable_code, "HAL-E-ADAPTER-FAULT");
        assert_eq!(err.profile, HalProfileId::Linux);
        assert_eq!(err.capability, CapabilityId::DynamicLinking);
        assert_eq!(err.operation, "invoke_symbol");
        assert_eq!(err.message, "descriptor mismatch");
    }

    #[test]
    fn unsupported_profile_factory_is_stable() {
        let err = HalError::unsupported_profile(
            HalProfileId::Wasm,
            CapabilityId::ComActivationDispatch,
            "create_object",
        );
        assert_eq!(err.kind, HalErrorKind::UnsupportedProfile);
        assert_eq!(err.stable_code, "HAL-E-UNSUPPORTED-PROFILE");
        assert_eq!(err.profile, HalProfileId::Wasm);
        assert_eq!(err.capability, CapabilityId::ComActivationDispatch);
        assert_eq!(err.operation, "create_object");
        assert_eq!(
            err.message,
            "operation is not implemented for the active profile"
        );
    }
}
