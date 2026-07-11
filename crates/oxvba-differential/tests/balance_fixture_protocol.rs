use std::collections::BTreeMap;
use std::process::Command;
use std::sync::{Arc, Barrier};

use oxvba_differential::balance_protocol::{
    ALL_BALANCE_FIXTURES, BalanceFixtureReport, CLEAN_BALANCE_FIXTURES, CanonObservation,
    FixtureCompletion, FullErrObservation, POLICY_ERROR_BALANCE_FIXTURE,
};

fn long(n: i32) -> CanonObservation {
    let bytes = n.to_le_bytes();
    CanonObservation::Raw {
        tag: 3,
        bytes: [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0, 0, 0],
        reserved: [0, 0, 0],
    }
}

fn run_fixture_child(fixture: &str) -> BalanceFixtureReport {
    let output = Command::new(env!("CARGO_BIN_EXE_oxvba_balance_fixture"))
        .args(["--fixture", fixture])
        .output()
        .unwrap_or_else(|err| panic!("fixture `{fixture}` failed to spawn: {err}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "fixture `{fixture}` child failed with {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );
    let report = BalanceFixtureReport::parse_protocol_output(&stdout).unwrap_or_else(|err| {
        panic!(
            "fixture `{fixture}` emitted an invalid report: {err}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        )
    });
    assert_eq!(
        report.fixture, fixture,
        "fixture `{fixture}` child reported the wrong identity"
    );
    report
}

fn assert_clean_fixture(report: &BalanceFixtureReport) {
    assert_eq!(
        report.result.completion,
        FixtureCompletion::Completed,
        "fixture `{}` completion mismatch: {report:?}",
        report.fixture
    );
    assert_eq!(
        report.full_err,
        FullErrObservation {
            number: 0,
            source: String::new(),
            description: String::new(),
            last_dll_error: 0,
        },
        "fixture `{}` full Err mismatch",
        report.fixture
    );
    assert!(
        report.carrier_deltas.is_zero(),
        "fixture `{}` carrier imbalance: {:?}",
        report.fixture,
        report.carrier_deltas
    );
}

#[test]
fn balance_fixture_subprocess_protocol() {
    let reports: BTreeMap<_, _> = ALL_BALANCE_FIXTURES
        .iter()
        .map(|fixture| ((*fixture).to_string(), run_fixture_child(fixture)))
        .collect();

    for fixture in CLEAN_BALANCE_FIXTURES {
        assert_clean_fixture(
            reports
                .get(*fixture)
                .unwrap_or_else(|| panic!("missing clean fixture `{fixture}` report")),
        );
    }

    let expected_results = [
        (
            "carrier-string",
            CanonObservation::String {
                value: "alpha-beta".to_string(),
            },
        ),
        ("carrier-array", long(60)),
        ("carrier-object", long(42)),
        ("carrier-record", long(41)),
    ];
    for (fixture, expected) in expected_results {
        let report = reports
            .get(fixture)
            .unwrap_or_else(|| panic!("missing fixture `{fixture}` report"));
        assert!(
            report.result.values.contains(&expected),
            "fixture `{fixture}` omitted expected result {expected:?}: {report:?}"
        );
    }

    let policy = reports
        .get(POLICY_ERROR_BALANCE_FIXTURE)
        .expect("host-policy fixture report");
    assert_eq!(
        policy.result.completion,
        FixtureCompletion::Raised,
        "{policy:?}"
    );
    assert_eq!(policy.full_err.number, 5, "{policy:?}");
    assert_eq!(
        policy.full_err,
        FullErrObservation {
            number: 5,
            source: "VBAProject".to_string(),
            description: "operation blocked by host policy".to_string(),
            last_dll_error: 0,
        },
        "host-policy fixture did not retain the full policy error"
    );
    assert_eq!(policy.result.message.as_deref(), Some("VBA error 5"));
    assert_eq!(
        policy.carrier_deltas.object_boxes, 0,
        "host-policy fixture object delta changed: {policy:?}"
    );
    assert_eq!(
        policy.carrier_deltas.safearrays, 0,
        "host-policy fixture SAFEARRAY delta changed: {policy:?}"
    );
    assert_eq!(
        policy.carrier_deltas.record_buffers, 0,
        "host-policy fixture record delta changed: {policy:?}"
    );
    // bd-59co.2.2.6 owns the known policy-error BSTR leak. This bead deliberately
    // characterizes it through the new isolated protocol instead of repairing it.
    assert_eq!(
        policy.carrier_deltas.bstrs, 1,
        "host-policy BSTR residual changed; bd-59co.2.2.6 must adjudicate it: {policy:?}"
    );

    let unknown = "missing-fixture";
    let output = Command::new(env!("CARGO_BIN_EXE_oxvba_balance_fixture"))
        .args(["--fixture", unknown])
        .output()
        .expect("spawn unknown fixture child");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "unknown fixture `{unknown}` unexpectedly succeeded"
    );
    assert!(
        stderr.contains(unknown),
        "unknown-fixture failure omitted its identity: {stderr}"
    );
}

#[test]
fn balance_fixture_parallel_isolation() {
    let serial: BTreeMap<_, _> = ALL_BALANCE_FIXTURES
        .iter()
        .map(|fixture| ((*fixture).to_string(), run_fixture_child(fixture)))
        .collect();

    let barrier = Arc::new(Barrier::new(ALL_BALANCE_FIXTURES.len()));
    let children: Vec<_> = ALL_BALANCE_FIXTURES
        .iter()
        .map(|fixture| {
            let fixture = (*fixture).to_string();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                let report = run_fixture_child(&fixture);
                (fixture, report)
            })
        })
        .collect();
    let parallel: BTreeMap<_, _> = children
        .into_iter()
        .map(|child| child.join().expect("balance fixture worker panicked"))
        .collect();

    assert_eq!(
        parallel, serial,
        "serial and concurrent named subprocess reports diverged"
    );
}
