//! HAL conformance probes and report model.

use std::collections::HashSet;

use crate::{
    HalResult,
    error::HalErrorKind,
    model::{ALL_CAPABILITIES, CapabilityId, HalDescriptor},
    traits::HostServices,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStatus {
    Passed,
    CapabilityUnavailable,
    PolicyDenied,
    AdapterFault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    pub capability: CapabilityId,
    pub operation: &'static str,
    pub status: ProbeStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub descriptor: HalDescriptor,
    pub passed: bool,
    pub failures: Vec<String>,
    pub probes: Vec<ProbeOutcome>,
}

impl ConformanceReport {
    pub fn summary_line(&self) -> String {
        format!(
            "HAL conformance: profile={:?} passed={} failures={} probes={}",
            self.descriptor.profile,
            self.passed,
            self.failures.len(),
            self.probes.len()
        )
    }
}

pub fn run_conformance(host: &dyn HostServices) -> ConformanceReport {
    let descriptor = host.descriptor();
    let mut failures = Vec::new();
    let mut probes = Vec::new();

    validate_descriptor_shape(&descriptor, &mut failures);

    let mut probe = |capability: CapabilityId, operation: &'static str, result: HalResult<()>| {
        let supports = descriptor.supports(capability);
        let policy_gated = is_policy_gated(capability);
        let outcome = match result {
            Ok(()) => {
                if supports {
                    ProbeOutcome {
                        capability,
                        operation,
                        status: ProbeStatus::Passed,
                        detail: None,
                    }
                } else {
                    failures.push(format!(
                        "{operation} unexpectedly succeeded while capability {:?} is unsupported",
                        capability
                    ));
                    ProbeOutcome {
                        capability,
                        operation,
                        status: ProbeStatus::Passed,
                        detail: Some("unexpected success for unsupported capability".to_string()),
                    }
                }
            }
            Err(err) => {
                let status = match err.kind {
                    HalErrorKind::CapabilityUnavailable => ProbeStatus::CapabilityUnavailable,
                    HalErrorKind::PolicyDenied => ProbeStatus::PolicyDenied,
                    HalErrorKind::AdapterFault | HalErrorKind::UnsupportedProfile => {
                        ProbeStatus::AdapterFault
                    }
                };
                if supports {
                    if status == ProbeStatus::PolicyDenied && policy_gated {
                        // expected in policy-denied mode
                    } else {
                        failures.push(format!(
                            "{operation} failed for supported capability {:?}: {} ({})",
                            capability, err.stable_code, err.message
                        ));
                    }
                } else if status != ProbeStatus::CapabilityUnavailable {
                    failures.push(format!(
                        "{operation} failed with {:?} for unsupported capability {:?}; expected CapabilityUnavailable",
                        status, capability
                    ));
                }
                ProbeOutcome {
                    capability,
                    operation,
                    status,
                    detail: Some(format!("{}: {}", err.stable_code, err.message)),
                }
            }
        };
        probes.push(outcome);
    };

    probe(
        CapabilityId::UiInteraction,
        "ui.msg_box",
        host.ui().msg_box(7, 1).map(|_| ()),
    );
    probe(
        CapabilityId::EventPump,
        "events.do_events",
        host.events().do_events().map(|_| ()),
    );
    probe(
        CapabilityId::FileSystemIo,
        "fs.free_file",
        host.fs().free_file(0).map(|_| ()),
    );
    probe(
        CapabilityId::ProcessEnv,
        "process.shell",
        host.process().shell(1, 0).map(|_| ()),
    );
    probe(
        CapabilityId::ComActivationDispatch,
        "com.create_object",
        host.com().create_object(4).map(|_| ()),
    );
    probe(
        CapabilityId::TimeLocale,
        "time.timer_ticks",
        host.time_locale().timer_ticks().map(|_| ()),
    );
    probe(
        CapabilityId::DynamicLinking,
        "dynlink.invoke_symbol",
        host.dynlink().invoke_symbol(1, 2).map(|_| ()),
    );
    probe(
        CapabilityId::DiagnosticsTelemetry,
        "diag.emit",
        host.diag().emit(1, 2).map(|_| ()),
    );

    ConformanceReport {
        descriptor,
        passed: failures.is_empty(),
        failures,
        probes,
    }
}

fn validate_descriptor_shape(descriptor: &HalDescriptor, failures: &mut Vec<String>) {
    if descriptor.contract_version.is_empty() {
        failures.push("descriptor.contract_version must not be empty".to_string());
    }
    if descriptor.adapter_version.is_empty() {
        failures.push("descriptor.adapter_version must not be empty".to_string());
    }

    let seen: HashSet<_> = descriptor
        .capabilities
        .iter()
        .map(|entry| entry.id)
        .collect();
    for required in ALL_CAPABILITIES {
        if !seen.contains(&required) {
            failures.push(format!(
                "descriptor missing required capability entry {:?}",
                required
            ));
        }
    }
    if seen.len() != descriptor.capabilities.len() {
        failures.push("descriptor contains duplicate capability entries".to_string());
    }
}

const fn is_policy_gated(capability: CapabilityId) -> bool {
    matches!(
        capability,
        CapabilityId::UiInteraction
            | CapabilityId::ProcessEnv
            | CapabilityId::ComActivationDispatch
            | CapabilityId::DynamicLinking
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        adapters::for_profile,
        model::{CapabilityId, HalProfileId, HostPolicy},
    };

    use super::run_conformance;

    #[test]
    fn conformance_l0_passes_for_all_profiles_in_runtime_mode() {
        for profile in [
            HalProfileId::Windows,
            HalProfileId::Linux,
            HalProfileId::MacOs,
            HalProfileId::Wasm,
            HalProfileId::Null,
        ] {
            let host = for_profile(profile, HostPolicy::deterministic_runtime());
            let report = run_conformance(host.as_ref());
            assert!(
                report.passed,
                "profile {:?} failed HAL conformance: {:?}",
                profile, report.failures
            );
        }
    }

    #[test]
    fn windows_declares_com_supported_only_on_windows() {
        let windows = for_profile(HalProfileId::Windows, HostPolicy::deterministic_runtime());
        assert!(
            windows
                .descriptor()
                .supports(CapabilityId::ComActivationDispatch),
            "windows profile must declare COM capability"
        );

        for profile in [
            HalProfileId::Linux,
            HalProfileId::MacOs,
            HalProfileId::Wasm,
            HalProfileId::Null,
        ] {
            let host = for_profile(profile, HostPolicy::deterministic_runtime());
            assert!(
                !host
                    .descriptor()
                    .supports(CapabilityId::ComActivationDispatch),
                "profile {:?} must not declare COM capability",
                profile
            );
        }
    }
}
