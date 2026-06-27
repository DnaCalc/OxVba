//! Oracle-conformance gate: validate the OxVBA executors against the **live Excel/VBA
//! 7.1 oracle** captured in `oracle/results/*.json`.
//!
//! For every probe in the oracle corpus this runs the self-contained program under both
//! vm3 (the executor under construction; it MUST be 100% oracle-compliant) and vm2 (the
//! transitional golden oracle, which we expect to deviate in places and do NOT fix —
//! those deviations are reported, not failed). The ground truth is the oracle string.
//!
//! The single hard assertion: **vm3 matches the oracle on every case outside the
//! documented out-of-scope allowlist.** As vm3 grows (M2-c-2, then M3), entries leave the
//! allowlist; an allowlisted case that starts matching is flagged so the allowlist can
//! shrink.

use oxvba_differential::oracle::{
    OracleObservation, all_oracle_cases, run_oracle_case,
};
use oxvba_differential::Executor;

/// Probes whose vm3 result is a KNOWN, documented gap — NOT an error-model regression.
/// Each entry names the missing capability and the wave that closes it.
///
///  - `On n GoTo` / `On n GoSub` (computed branch): the binder rejects `On <expr> GoTo`
///    today (`Malformed("On Error form")`) — a front-end gap tracked for a later
///    control-flow wave, not the error model.
///  - `For Each` over `Array()`: typed arrays are M3; vm3 reports `Unsupported("array op")`.
const KNOWN_VM3_GAPS: &[&str] = &[
    "PROBE_cf_on_n_goto",
    "PROBE_cf_on_n_goto_zero",
    "PROBE_cf_on_n_gosub",
    "PROBE_cf_for_each_array",
];

fn render(obs: &OracleObservation) -> String {
    match obs {
        OracleObservation::Value(s) => format!("\"{s}\""),
        OracleObservation::Unsupported(w) => format!("UNSUPPORTED({w})"),
        OracleObservation::Failed(e) => format!("FAILED({e})"),
        OracleObservation::NotAString(c) => format!("NOT-A-STRING({c:?})"),
        OracleObservation::Empty => "EMPTY".to_string(),
        OracleObservation::Timeout => "TIMEOUT".to_string(),
    }
}

#[test]
fn vm3_is_oracle_compliant() {
    let cases = all_oracle_cases();
    assert!(!cases.is_empty(), "no oracle cases loaded — is oracle/ present?");

    let mut vm3_match = 0usize;
    let mut vm3_mismatch: Vec<String> = Vec::new();
    let mut vm2_divergence: Vec<String> = Vec::new();
    let mut allowlisted_now_passing: Vec<String> = Vec::new();
    let mut report: Vec<String> = Vec::new();

    for case in &cases {
        let vm3 = run_oracle_case(Executor::Vm3, case);
        let vm2 = run_oracle_case(Executor::Vm2, case);
        let allowlisted = KNOWN_VM3_GAPS.contains(&case.name.as_str());

        let vm3_ok = vm3.value() == Some(case.expected.as_str());
        let vm2_ok = vm2.value() == Some(case.expected.as_str());

        report.push(format!(
            "{:<36} oracle={:<40} vm3={:<44} vm2={}",
            case.name,
            format!("\"{}\"", case.expected),
            render(&vm3),
            render(&vm2),
        ));

        if allowlisted {
            if vm3_ok {
                allowlisted_now_passing.push(case.name.clone());
            }
        } else if vm3_ok {
            vm3_match += 1;
        } else {
            vm3_mismatch.push(format!(
                "  {} : oracle=\"{}\"  vm3={}",
                case.name,
                case.expected,
                render(&vm3)
            ));
        }

        if !vm2_ok {
            vm2_divergence.push(format!(
                "  {} : oracle=\"{}\"  vm2={}",
                case.name,
                case.expected,
                render(&vm2)
            ));
        }
    }

    eprintln!("\n=== oracle three-way report (oracle | vm3 | vm2) ===");
    for line in &report {
        eprintln!("{line}");
    }
    eprintln!(
        "\nvm3: {} matched / {} mismatched (outside allowlist), {} allowlisted gaps",
        vm3_match,
        vm3_mismatch.len(),
        KNOWN_VM3_GAPS.len()
    );
    if !vm2_divergence.is_empty() {
        eprintln!(
            "\nvm2-vs-oracle divergences ({} — documented vm2 non-compliances, NOT failed here):",
            vm2_divergence.len()
        );
        for line in &vm2_divergence {
            eprintln!("{line}");
        }
    }

    // If an allowlisted case now passes, the allowlist is stale — shrink it.
    assert!(
        allowlisted_now_passing.is_empty(),
        "these allowlisted gaps now pass on vm3 — remove them from KNOWN_VM3_GAPS: {allowlisted_now_passing:?}"
    );

    // The hard gate: vm3 must be oracle-compliant on every non-allowlisted case.
    assert!(
        vm3_mismatch.is_empty(),
        "vm3 diverges from the Excel/VBA oracle on {} case(s):\n{}",
        vm3_mismatch.len(),
        vm3_mismatch.join("\n")
    );
}
