//! HAL conformance probes and report model.

use std::collections::{BTreeMap, HashSet};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClauseCheckStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseCheck {
    pub clause_id: &'static str,
    pub status: ClauseCheckStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOutcome {
    pub capability: CapabilityId,
    pub operation: &'static str,
    pub clause_ids: Vec<&'static str>,
    pub status: ProbeStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub descriptor: HalDescriptor,
    pub passed: bool,
    pub failures: Vec<String>,
    pub clause_checks: Vec<ClauseCheck>,
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

    pub fn clause_coverage(&self) -> BTreeMap<&'static str, bool> {
        let mut out = BTreeMap::new();
        for check in &self.clause_checks {
            out.insert(check.clause_id, check.status == ClauseCheckStatus::Passed);
        }
        for probe in &self.probes {
            let probe_passed = matches!(
                probe.status,
                ProbeStatus::Passed
                    | ProbeStatus::CapabilityUnavailable
                    | ProbeStatus::PolicyDenied
            );
            for clause_id in &probe.clause_ids {
                out.entry(clause_id)
                    .and_modify(|current| *current = *current && probe_passed)
                    .or_insert(probe_passed);
            }
        }
        out
    }
}

pub fn run_conformance(host: &dyn HostServices) -> ConformanceReport {
    let descriptor = host.descriptor();
    let mut clause_checks = Vec::new();
    let mut failures = Vec::new();
    let mut probes = Vec::new();

    validate_descriptor_shape(&descriptor, &mut clause_checks, &mut failures);

    let mut probe = |capability: CapabilityId,
                     operation: &'static str,
                     clause_ids: &'static [&'static str],
                     result: HalResult<()>| {
        let supports = descriptor.supports(capability);
        let policy_gated = is_policy_gated(capability);
        let outcome = match result {
            Ok(()) => {
                if supports {
                    ProbeOutcome {
                        capability,
                        operation,
                        clause_ids: clause_ids.to_vec(),
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
                        clause_ids: clause_ids.to_vec(),
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
                    clause_ids: clause_ids.to_vec(),
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
        &["HAL-UI-001", "HAL-GEN-001", "HAL-GEN-003", "HAL-GEN-004"],
        host.ui().msg_box(7, 1).map(|_| ()),
    );
    probe(
        CapabilityId::EventPump,
        "events.do_events",
        &["HAL-EVT-001", "HAL-GEN-001", "HAL-GEN-003"],
        host.events().do_events().map(|_| ()),
    );
    probe(
        CapabilityId::FileSystemIo,
        "fs.free_file",
        &["HAL-FS-006", "HAL-GEN-001", "HAL-GEN-003"],
        host.fs().free_file(0).map(|_| ()),
    );
    probe(
        CapabilityId::ProcessEnv,
        "process.shell",
        &["HAL-PROC-001", "HAL-GEN-001", "HAL-GEN-003", "HAL-GEN-004"],
        host.process().shell(1, 0).map(|_| ()),
    );
    probe(
        CapabilityId::ComActivationDispatch,
        "com.create_object",
        &[
            "HAL-COM-001",
            "HAL-COM-003",
            "HAL-GEN-001",
            "HAL-GEN-003",
            "HAL-GEN-004",
        ],
        host.com().create_object(4).map(|_| ()),
    );
    probe(
        CapabilityId::TimeLocale,
        "time.timer_ticks",
        &["HAL-TIME-001", "HAL-GEN-001", "HAL-GEN-003"],
        host.time_locale().timer_ticks().map(|_| ()),
    );
    probe(
        CapabilityId::DynamicLinking,
        "dynlink.invoke_symbol",
        &["HAL-DYN-001", "HAL-GEN-001", "HAL-GEN-003", "HAL-GEN-004"],
        host.dynlink().invoke_symbol(1, 2).map(|_| ()),
    );
    probe(
        CapabilityId::DiagnosticsTelemetry,
        "diag.emit",
        &["HAL-DIAG-001", "HAL-GEN-001", "HAL-GEN-003"],
        host.diag().emit(1, 2).map(|_| ()),
    );

    ConformanceReport {
        descriptor,
        passed: failures.is_empty(),
        failures,
        clause_checks,
        probes,
    }
}

fn validate_descriptor_shape(
    descriptor: &HalDescriptor,
    checks: &mut Vec<ClauseCheck>,
    failures: &mut Vec<String>,
) {
    if descriptor.contract_version.is_empty() {
        let detail = "descriptor.contract_version must not be empty".to_string();
        failures.push(detail.clone());
        checks.push(ClauseCheck {
            clause_id: "HAL-DES-001",
            status: ClauseCheckStatus::Failed,
            detail: Some(detail),
        });
    } else {
        checks.push(ClauseCheck {
            clause_id: "HAL-DES-001",
            status: ClauseCheckStatus::Passed,
            detail: None,
        });
    }
    if descriptor.adapter_version.is_empty() {
        let detail = "descriptor.adapter_version must not be empty".to_string();
        failures.push(detail.clone());
        checks.push(ClauseCheck {
            clause_id: "HAL-DES-002",
            status: ClauseCheckStatus::Failed,
            detail: Some(detail),
        });
    } else {
        checks.push(ClauseCheck {
            clause_id: "HAL-DES-002",
            status: ClauseCheckStatus::Passed,
            detail: None,
        });
    }

    let seen: HashSet<_> = descriptor
        .capabilities
        .iter()
        .map(|entry| entry.id)
        .collect();
    let mut missing = Vec::new();
    for required in ALL_CAPABILITIES {
        if !seen.contains(&required) {
            let detail = format!(
                "descriptor missing required capability entry {:?}",
                required
            );
            failures.push(detail.clone());
            missing.push(detail);
        }
    }

    if missing.is_empty() {
        checks.push(ClauseCheck {
            clause_id: "HAL-GEN-002",
            status: ClauseCheckStatus::Passed,
            detail: None,
        });
    } else {
        checks.push(ClauseCheck {
            clause_id: "HAL-GEN-002",
            status: ClauseCheckStatus::Failed,
            detail: Some(missing.join("; ")),
        });
    }

    let has_duplicates = seen.len() != descriptor.capabilities.len();
    if has_duplicates {
        let detail = "descriptor contains duplicate capability entries".to_string();
        failures.push(detail.clone());
        checks.push(ClauseCheck {
            clause_id: "HAL-DES-003",
            status: ClauseCheckStatus::Failed,
            detail: Some(detail),
        });
    } else {
        checks.push(ClauseCheck {
            clause_id: "HAL-DES-003",
            status: ClauseCheckStatus::Passed,
            detail: None,
        });
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
    use std::collections::BTreeSet;

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

    #[test]
    fn conformance_l0_passes_for_all_profiles_in_compile_time_mode() {
        for profile in [
            HalProfileId::Windows,
            HalProfileId::Linux,
            HalProfileId::MacOs,
            HalProfileId::Wasm,
            HalProfileId::Null,
        ] {
            let host = for_profile(profile, HostPolicy::deterministic_compile_time());
            let report = run_conformance(host.as_ref());
            assert!(
                report.passed,
                "profile {:?} failed compile-time conformance: {:?}",
                profile, report.failures
            );
        }
    }

    #[test]
    fn conformance_report_exposes_clause_coverage_map() {
        let host = for_profile(HalProfileId::Windows, HostPolicy::deterministic_runtime());
        let report = run_conformance(host.as_ref());
        let coverage = report.clause_coverage();
        assert_eq!(coverage.get("HAL-DES-001"), Some(&true));
        assert_eq!(coverage.get("HAL-GEN-002"), Some(&true));
        assert_eq!(coverage.get("HAL-UI-001"), Some(&true));
        assert_eq!(coverage.get("HAL-COM-001"), Some(&true));
    }

    #[test]
    fn conformance_probe_records_clause_ids() {
        let host = for_profile(HalProfileId::Windows, HostPolicy::deterministic_runtime());
        let report = run_conformance(host.as_ref());
        let probe = report
            .probes
            .iter()
            .find(|probe| probe.operation == "com.create_object")
            .expect("com.create_object probe should exist");
        let expected: BTreeSet<&'static str> = ["HAL-COM-001", "HAL-COM-003", "HAL-GEN-001"]
            .into_iter()
            .collect();
        let actual: BTreeSet<&'static str> = probe.clause_ids.iter().copied().collect();
        assert!(
            expected.is_subset(&actual),
            "probe clause IDs should include required set"
        );
    }
}
