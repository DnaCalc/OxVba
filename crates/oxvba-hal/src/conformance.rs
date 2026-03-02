//! HAL conformance probes and report model.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{
    HalResult,
    error::HalErrorKind,
    model::{
        ALL_CAPABILITIES, CapabilityId, CapabilityMaturity, HalDescriptor, host_backed_mode_active,
        host_backed_profile_matches_host,
    },
    traits::HostServices,
};

const CLAUSE_CATALOG_CSV_V1: &str =
    include_str!("../../../docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.csv");
const CLAUSE_CATALOG_MD_V1: &str =
    include_str!("../../../docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md");

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
    pub host_backed_eligible: bool,
    pub host_backed_active: bool,
    pub passed: bool,
    pub failures: Vec<String>,
    pub governance_notices: Vec<String>,
    pub clause_checks: Vec<ClauseCheck>,
    pub probes: Vec<ProbeOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClauseVerificationScope {
    Conformance,
    HostTests,
    Documentation,
}

impl ClauseVerificationScope {
    fn from_csv(value: &str) -> Option<Self> {
        match value {
            "conformance" => Some(Self::Conformance),
            "host-tests" => Some(Self::HostTests),
            "documentation" => Some(Self::Documentation),
            _ => None,
        }
    }

    const fn is_conformance(self) -> bool {
        matches!(self, Self::Conformance)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClauseCatalogEntry {
    pub clause_id: &'static str,
    pub status: &'static str,
    pub verification_scope: ClauseVerificationScope,
}

pub fn clause_catalog_entries_v1() -> Vec<ClauseCatalogEntry> {
    let mut out = Vec::new();
    for (index, line) in CLAUSE_CATALOG_CSV_V1.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        assert!(
            parts.len() == 5,
            "HAL clause catalog CSV malformed at line {}: expected 5 columns",
            index + 1
        );
        let verification_scope = ClauseVerificationScope::from_csv(parts[3]).unwrap_or_else(|| {
            panic!(
                "HAL clause catalog CSV malformed at line {}: unknown verification scope '{}'",
                index + 1,
                parts[3]
            )
        });
        out.push(ClauseCatalogEntry {
            clause_id: parts[0],
            status: parts[2],
            verification_scope,
        });
    }
    out
}

pub fn markdown_clause_ids_v1() -> BTreeSet<&'static str> {
    let mut out = BTreeSet::new();
    let mut chunks = CLAUSE_CATALOG_MD_V1.split('`');
    while let Some(chunk) = chunks.next() {
        let _ = chunk;
        let Some(candidate) = chunks.next() else {
            break;
        };
        if is_clause_id_token(candidate) {
            out.insert(candidate);
        }
    }
    out
}

fn is_clause_id_token(candidate: &str) -> bool {
    let mut parts = candidate.split('-');
    let Some("HAL") = parts.next() else {
        return false;
    };
    let Some(domain) = parts.next() else {
        return false;
    };
    let Some(seq) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    !domain.is_empty()
        && domain.chars().all(|ch| ch.is_ascii_uppercase())
        && seq.len() == 3
        && seq.chars().all(|ch| ch.is_ascii_digit())
}

impl ConformanceReport {
    pub fn summary_line(&self) -> String {
        format!(
            "HAL conformance: profile={:?} passed={} failures={} governance_notices={} probes={}",
            self.descriptor.profile,
            self.passed,
            self.failures.len(),
            self.governance_notices.len(),
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

    pub fn clause_coverage_against_catalog(&self) -> BTreeMap<&'static str, bool> {
        let raw = self.clause_coverage();
        let mut out = BTreeMap::new();
        for entry in clause_catalog_entries_v1() {
            if entry.verification_scope.is_conformance() {
                out.insert(
                    entry.clause_id,
                    raw.get(entry.clause_id).copied().unwrap_or(false),
                );
            }
        }
        out
    }
}

pub fn run_conformance(host: &dyn HostServices) -> ConformanceReport {
    let descriptor = host.descriptor();
    let host_backed_eligible = host_backed_profile_matches_host(descriptor.profile);
    let host_backed_active = host_backed_mode_active(descriptor.profile, host.policy());
    let mut clause_checks = Vec::new();
    let mut failures = Vec::new();
    let mut governance_notices = Vec::new();
    let mut probes = Vec::new();

    validate_descriptor_shape(&descriptor, &mut clause_checks, &mut failures);
    evaluate_maturity_governance(&descriptor, &mut clause_checks, &mut governance_notices);
    let mut err_stable_code_ok = true;
    let mut err_payload_ok = true;

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
                if err.stable_code != expected_stable_code_for_kind(err.kind) {
                    err_stable_code_ok = false;
                    failures.push(format!(
                        "{operation} returned unexpected stable code {} for {:?}",
                        err.stable_code, err.kind
                    ));
                }
                if err.operation.is_empty() || err.message.is_empty() {
                    err_payload_ok = false;
                    failures.push(format!(
                        "{operation} returned malformed HAL error payload (op='{}', message='{}')",
                        err.operation, err.message
                    ));
                }
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
        &[
            "HAL-UI-001",
            "HAL-DES-004",
            "HAL-GEN-001",
            "HAL-GEN-003",
            "HAL-GEN-004",
        ],
        host.ui().msg_box(7, 1).map(|_| ()),
    );
    probe(
        CapabilityId::EventPump,
        "events.do_events",
        &["HAL-EVT-001", "HAL-DES-004", "HAL-GEN-001", "HAL-GEN-003"],
        host.events().do_events().map(|_| ()),
    );
    probe(
        CapabilityId::FileSystemIo,
        "fs.free_file",
        &["HAL-FS-006", "HAL-DES-004", "HAL-GEN-001", "HAL-GEN-003"],
        host.fs().free_file(0).map(|_| ()),
    );
    probe(
        CapabilityId::ProcessEnv,
        "process.shell",
        &[
            "HAL-PROC-001",
            "HAL-DES-004",
            "HAL-GEN-001",
            "HAL-GEN-003",
            "HAL-GEN-004",
        ],
        host.process().shell(1, 0).map(|_| ()),
    );
    probe(
        CapabilityId::ComActivationDispatch,
        "com.create_object",
        &[
            "HAL-COM-001",
            "HAL-COM-003",
            "HAL-DES-004",
            "HAL-GEN-001",
            "HAL-GEN-003",
            "HAL-GEN-004",
        ],
        host.com().create_object(4).map(|_| ()),
    );
    probe(
        CapabilityId::TimeLocale,
        "time.timer_ticks",
        &["HAL-TIME-001", "HAL-DES-004", "HAL-GEN-001", "HAL-GEN-003"],
        host.time_locale().timer_ticks().map(|_| ()),
    );
    probe(
        CapabilityId::DynamicLinking,
        "dynlink.invoke_symbol",
        &[
            "HAL-DYN-001",
            "HAL-DES-004",
            "HAL-GEN-001",
            "HAL-GEN-003",
            "HAL-GEN-004",
        ],
        host.dynlink().invoke_symbol(1, 2).map(|_| ()),
    );
    probe(
        CapabilityId::DiagnosticsTelemetry,
        "diag.emit",
        &["HAL-DIAG-001", "HAL-DES-004", "HAL-GEN-001", "HAL-GEN-003"],
        host.diag().emit(1, 2).map(|_| ()),
    );
    evaluate_host_backed_contract_paths(
        host,
        &descriptor,
        host_backed_active,
        &mut clause_checks,
        &mut failures,
    );

    clause_checks.push(ClauseCheck {
        clause_id: "HAL-ERR-001",
        status: if err_stable_code_ok {
            ClauseCheckStatus::Passed
        } else {
            ClauseCheckStatus::Failed
        },
        detail: if err_stable_code_ok {
            None
        } else {
            Some("observed stable-code mismatch in HAL error responses".to_string())
        },
    });
    clause_checks.push(ClauseCheck {
        clause_id: "HAL-ERR-002",
        status: if err_payload_ok {
            ClauseCheckStatus::Passed
        } else {
            ClauseCheckStatus::Failed
        },
        detail: if err_payload_ok {
            None
        } else {
            Some("observed malformed HAL error payload fields".to_string())
        },
    });

    validate_clause_reference_integrity(&mut clause_checks, &mut failures, &probes);

    ConformanceReport {
        descriptor,
        host_backed_eligible,
        host_backed_active,
        passed: failures.is_empty(),
        failures,
        governance_notices,
        clause_checks,
        probes,
    }
}

fn evaluate_host_backed_contract_paths(
    host: &dyn HostServices,
    descriptor: &HalDescriptor,
    host_backed_active: bool,
    checks: &mut Vec<ClauseCheck>,
    failures: &mut Vec<String>,
) {
    let fs_ok = verify_fs_host_backed_contract(host, descriptor, host_backed_active, failures);
    checks.push(ClauseCheck {
        clause_id: "HAL-FS-007",
        status: if fs_ok {
            ClauseCheckStatus::Passed
        } else {
            ClauseCheckStatus::Failed
        },
        detail: if fs_ok {
            None
        } else {
            Some("filesystem host-backed/deterministic floor checks failed".to_string())
        },
    });

    let proc_ok =
        verify_process_host_backed_contract(host, descriptor, host_backed_active, failures);
    checks.push(ClauseCheck {
        clause_id: "HAL-PROC-004",
        status: if proc_ok {
            ClauseCheckStatus::Passed
        } else {
            ClauseCheckStatus::Failed
        },
        detail: if proc_ok {
            None
        } else {
            Some("process/env host-backed contract checks failed".to_string())
        },
    });

    let time_ok = verify_time_host_backed_contract(host, descriptor, host_backed_active, failures);
    checks.push(ClauseCheck {
        clause_id: "HAL-TIME-002",
        status: if time_ok {
            ClauseCheckStatus::Passed
        } else {
            ClauseCheckStatus::Failed
        },
        detail: if time_ok {
            None
        } else {
            Some("time host-backed contract checks failed".to_string())
        },
    });
}

fn verify_fs_host_backed_contract(
    host: &dyn HostServices,
    descriptor: &HalDescriptor,
    host_backed_active: bool,
    failures: &mut Vec<String>,
) -> bool {
    if !descriptor.supports(CapabilityId::FileSystemIo) {
        return true;
    }

    let mut ok = true;
    let handle = match host.fs().open(901, 0) {
        Ok(handle) => handle,
        Err(err) => {
            failures.push(format!(
                "fs.open(mode=0) failed while FileSystemIo is supported: {} ({})",
                err.stable_code, err.message
            ));
            return false;
        }
    };
    let _ = host.fs().close(handle);

    if host.policy().allow_filesystem_mutation {
        match host.fs().open(902, 1) {
            Ok(mutable_handle) => {
                if let Err(err) = host.fs().seek(mutable_handle, 96) {
                    ok = false;
                    failures.push(format!(
                        "fs.seek mutation probe failed: {} ({})",
                        err.stable_code, err.message
                    ));
                } else {
                    match host.fs().lof(mutable_handle) {
                        Ok(len) => {
                            if len < 96 {
                                ok = false;
                                failures.push(format!(
                                    "fs.lof mutation probe expected >= 96, observed {}",
                                    len
                                ));
                            }
                        }
                        Err(err) => {
                            ok = false;
                            failures.push(format!(
                                "fs.lof mutation probe failed: {} ({})",
                                err.stable_code, err.message
                            ));
                        }
                    }
                }
                let _ = host.fs().close(mutable_handle);
            }
            Err(err) => {
                ok = false;
                failures.push(format!(
                    "fs.open(mode=1) denied/faulted while mutation is allowed: {} ({})",
                    err.stable_code, err.message
                ));
            }
        }
    } else if let Err(err) = host.fs().open(903, 1) {
        if err.kind != HalErrorKind::PolicyDenied {
            ok = false;
            failures.push(format!(
                "fs.open(mode=1) expected policy denial; got {} ({})",
                err.stable_code, err.message
            ));
        }
    } else {
        ok = false;
        failures.push(
            "fs.open(mode=1) unexpectedly succeeded when mutation policy is disabled".to_string(),
        );
    }

    if host_backed_active && descriptor.profile == crate::model::HalProfileId::Windows {
        // Windows-native realization check: mutation lane should not collapse to the deterministic
        // constant-size path.
        match host.fs().open(904, 1) {
            Ok(h) => {
                let _ = host.fs().seek(h, 128);
                match host.fs().lof(h) {
                    Ok(len) if len >= 128 => {}
                    Ok(len) => {
                        ok = false;
                        failures.push(format!(
                            "windows host-backed fs expected len >= 128, observed {}",
                            len
                        ));
                    }
                    Err(err) => {
                        ok = false;
                        failures.push(format!(
                            "windows host-backed fs lof probe failed: {} ({})",
                            err.stable_code, err.message
                        ));
                    }
                }
                let _ = host.fs().close(h);
            }
            Err(err) => {
                ok = false;
                failures.push(format!(
                    "windows host-backed fs open probe failed: {} ({})",
                    err.stable_code, err.message
                ));
            }
        }
    }

    ok
}

fn verify_process_host_backed_contract(
    host: &dyn HostServices,
    descriptor: &HalDescriptor,
    host_backed_active: bool,
    failures: &mut Vec<String>,
) -> bool {
    if !descriptor.supports(CapabilityId::ProcessEnv) {
        return true;
    }
    if !host.policy().allow_process_spawn {
        return true;
    }

    let mut ok = true;
    match host.process().shell(1, 0) {
        Ok(token) => {
            if host_backed_active && token <= 1 {
                ok = false;
                failures.push(format!(
                    "process.shell expected host-backed pid-like token > 1; observed {}",
                    token
                ));
            }
        }
        Err(err) => {
            ok = false;
            failures.push(format!(
                "process.shell probe failed despite supported capability/policy: {} ({})",
                err.stable_code, err.message
            ));
        }
    }

    if let Err(err) = host.process().environ(1) {
        ok = false;
        failures.push(format!(
            "process.environ host-backed contract probe failed: {} ({})",
            err.stable_code, err.message
        ));
    }
    if let Err(err) = host.process().dir(0, 0) {
        ok = false;
        failures.push(format!(
            "process.dir host-backed contract probe failed: {} ({})",
            err.stable_code, err.message
        ));
    }
    ok
}

fn verify_time_host_backed_contract(
    host: &dyn HostServices,
    descriptor: &HalDescriptor,
    host_backed_active: bool,
    failures: &mut Vec<String>,
) -> bool {
    if !descriptor.supports(CapabilityId::TimeLocale) {
        return true;
    }

    let mut ok = true;
    let date = match host.time_locale().date_serial_now() {
        Ok(value) => value,
        Err(err) => {
            failures.push(format!(
                "time.date_serial_now probe failed: {} ({})",
                err.stable_code, err.message
            ));
            return false;
        }
    };
    let time = match host.time_locale().time_serial_now() {
        Ok(value) => value,
        Err(err) => {
            failures.push(format!(
                "time.time_serial_now probe failed: {} ({})",
                err.stable_code, err.message
            ));
            return false;
        }
    };
    let ticks = match host.time_locale().timer_ticks() {
        Ok(value) => value,
        Err(err) => {
            failures.push(format!(
                "time.timer_ticks probe failed: {} ({})",
                err.stable_code, err.message
            ));
            return false;
        }
    };

    if host_backed_active {
        if date < 0 || time < 0 || ticks < 0 {
            ok = false;
            failures.push(format!(
                "host-backed time probe observed negative tokens: date={}, time={}, ticks={}",
                date, time, ticks
            ));
        }
        if date == 20_260_301 && time == 123_456 && ticks == 42 {
            ok = false;
            failures.push(
                "host-backed time probe observed deterministic constants; expected system-time-derived values".to_string(),
            );
        }
    } else if host.policy().deterministic_mode
        && (date != 20_260_301 || time != 123_456 || ticks != 42)
    {
        ok = false;
        failures.push(format!(
            "deterministic time lane expected constants; observed date={}, time={}, ticks={}",
            date, time, ticks
        ));
    }

    ok
}

const fn expected_stable_code_for_kind(kind: HalErrorKind) -> &'static str {
    match kind {
        HalErrorKind::CapabilityUnavailable => "HAL-E-CAP-UNAVAILABLE",
        HalErrorKind::PolicyDenied => "HAL-E-POLICY-DENIED",
        HalErrorKind::AdapterFault => "HAL-E-ADAPTER-FAULT",
        HalErrorKind::UnsupportedProfile => "HAL-E-UNSUPPORTED-PROFILE",
    }
}

fn evaluate_maturity_governance(
    descriptor: &HalDescriptor,
    checks: &mut Vec<ClauseCheck>,
    notices: &mut Vec<String>,
) {
    let mut supported_stub = Vec::new();
    let mut unsupported_non_stable = Vec::new();

    for capability in &descriptor.capabilities {
        if capability.supported && capability.maturity == CapabilityMaturity::Stub {
            supported_stub.push(format!("{:?}", capability.id));
        }
        if !capability.supported && capability.maturity != CapabilityMaturity::Stable {
            unsupported_non_stable.push(format!("{:?}", capability.id));
        }
    }

    if supported_stub.is_empty() {
        checks.push(ClauseCheck {
            clause_id: "HAL-GOV-001",
            status: ClauseCheckStatus::Passed,
            detail: None,
        });
    } else {
        let detail = format!(
            "supported capabilities with stub maturity: {}",
            supported_stub.join(", ")
        );
        notices.push(detail.clone());
        checks.push(ClauseCheck {
            clause_id: "HAL-GOV-001",
            status: ClauseCheckStatus::Failed,
            detail: Some(detail),
        });
    }

    if unsupported_non_stable.is_empty() {
        checks.push(ClauseCheck {
            clause_id: "HAL-GOV-002",
            status: ClauseCheckStatus::Passed,
            detail: None,
        });
    } else {
        let detail = format!(
            "unsupported capabilities with non-stable maturity: {}",
            unsupported_non_stable.join(", ")
        );
        notices.push(detail.clone());
        checks.push(ClauseCheck {
            clause_id: "HAL-GOV-002",
            status: ClauseCheckStatus::Failed,
            detail: Some(detail),
        });
    }
}

fn validate_clause_reference_integrity(
    checks: &mut Vec<ClauseCheck>,
    failures: &mut Vec<String>,
    probes: &[ProbeOutcome],
) {
    let known_ids: BTreeSet<&'static str> = clause_catalog_entries_v1()
        .into_iter()
        .map(|entry| entry.clause_id)
        .collect();
    let mut unknown = Vec::new();

    for check in checks.iter() {
        if !known_ids.contains(check.clause_id) {
            unknown.push(format!(
                "check references unknown clause {}",
                check.clause_id
            ));
        }
    }
    for probe in probes {
        for clause_id in &probe.clause_ids {
            if !known_ids.contains(clause_id) {
                unknown.push(format!(
                    "probe {} references unknown clause {}",
                    probe.operation, clause_id
                ));
            }
        }
    }

    if unknown.is_empty() {
        checks.push(ClauseCheck {
            clause_id: "HAL-GEN-008",
            status: ClauseCheckStatus::Passed,
            detail: None,
        });
    } else {
        let detail = unknown.join("; ");
        failures.push(format!("HAL clause drift detected: {detail}"));
        checks.push(ClauseCheck {
            clause_id: "HAL-GEN-008",
            status: ClauseCheckStatus::Failed,
            detail: Some(detail),
        });
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
        model::{
            CapabilityId, HalProfileId, HostPolicy, WasmRuntimeClass,
            host_backed_profile_matches_host,
        },
    };

    use super::{
        ClauseCheckStatus, clause_catalog_entries_v1, markdown_clause_ids_v1, run_conformance,
    };

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
        assert_eq!(coverage.get("HAL-GEN-008"), Some(&true));
    }

    #[test]
    fn conformance_catalog_and_markdown_clause_ids_match() {
        let csv_ids: BTreeSet<&'static str> = clause_catalog_entries_v1()
            .into_iter()
            .map(|entry| entry.clause_id)
            .collect();
        let markdown_ids = markdown_clause_ids_v1();
        assert_eq!(
            csv_ids, markdown_ids,
            "machine-readable and markdown clause catalogs must stay in lockstep"
        );
    }

    #[test]
    fn conformance_catalog_scoped_coverage_is_available() {
        let host = for_profile(HalProfileId::Windows, HostPolicy::deterministic_runtime());
        let report = run_conformance(host.as_ref());
        let coverage = report.clause_coverage_against_catalog();
        assert!(
            !coverage.is_empty(),
            "catalog-scoped coverage must not be empty"
        );
        assert_eq!(coverage.get("HAL-GEN-008"), Some(&true));
        assert_eq!(coverage.get("HAL-DES-001"), Some(&true));
    }

    #[test]
    fn governance_rules_are_executable_and_non_blocking() {
        let host = for_profile(HalProfileId::MacOs, HostPolicy::deterministic_runtime());
        let report = run_conformance(host.as_ref());
        assert!(
            report.passed,
            "governance notices should not hard-fail conformance report"
        );
        assert!(
            !report.governance_notices.is_empty(),
            "macOS profile should emit exploratory governance notices"
        );
        let gov_check = report
            .clause_checks
            .iter()
            .find(|check| check.clause_id == "HAL-GOV-001")
            .expect("governance check must be emitted");
        assert_eq!(gov_check.status, ClauseCheckStatus::Failed);
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

    #[test]
    fn conformance_report_exposes_host_backed_flags_for_interactive_lane() {
        let profile = if cfg!(target_os = "windows") {
            HalProfileId::Windows
        } else {
            HalProfileId::Linux
        };
        let host = for_profile(profile, HostPolicy::interactive_dev());
        let report = run_conformance(host.as_ref());
        let expected = host_backed_profile_matches_host(profile);
        assert_eq!(report.host_backed_eligible, expected);
        assert_eq!(report.host_backed_active, expected);
    }

    #[test]
    fn wasm_runtime_class_is_reported_in_descriptor() {
        let host = for_profile(
            HalProfileId::Wasm,
            HostPolicy::deterministic_runtime()
                .with_wasm_runtime_class(WasmRuntimeClass::BrowserSandbox),
        );
        let report = run_conformance(host.as_ref());
        assert_eq!(report.descriptor.runtime_class, "browser-sandbox");
    }
}
