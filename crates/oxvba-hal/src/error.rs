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
        }
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
        }
    }
}
