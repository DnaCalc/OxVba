//! `Partition` range-label intrinsic on vm3.
//!
//! Expected spacing is cross-checked against the documented Microsoft.VisualBasic
//! algorithm: both bounds are right-justified to the width of `stop + 1`.

use oxvba_differential::{Canon, Executor, run};

fn long(n: i32) -> Canon {
    let b = n.to_le_bytes();
    Canon::Raw {
        tag: 3,
        bytes: [b[0], b[1], b[2], b[3], 0, 0, 0, 0],
        reserved: [0, 0, 0],
    }
}

fn snapshot(body: &str) -> Vec<Canon> {
    let source = format!("Sub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\n{body}"))
}

#[test]
fn partition_resolves_and_formats_ranges() {
    let snap = snapshot(
        "    Dim first As String, below As String, last As String, above As String\n\
             first = Partition(0, 0, 99, 5)\n\
             below = Partition(-1, 0, 99, 5)\n\
             last = Partition(1000, 100, 1010, 20)\n\
             above = Partition(1011, 100, 1010, 20)",
    );
    assert!(
        snap.contains(&Canon::Str("  0:  4".to_string())),
        "{snap:?}"
    );
    assert!(
        snap.contains(&Canon::Str("   : -1".to_string())),
        "{snap:?}"
    );
    assert!(
        snap.contains(&Canon::Str("1000:1010".to_string())),
        "{snap:?}"
    );
    assert!(
        snap.contains(&Canon::Str("1011:    ".to_string())),
        "{snap:?}"
    );
}

#[test]
fn partition_null_and_invalid_range_behaviour() {
    let null_snap = snapshot(
        "    Dim r As Variant, code As Long\n\
             r = Partition(Null, 0, 10, 2)\n\
             code = VarType(r)",
    );
    assert!(null_snap.contains(&long(1)), "{null_snap:?}");

    let outcome = run(
        Executor::Vm3,
        "Sub Main()\n    Dim r As String\n    r = Partition(1, 0, 10, 0)\nEnd Sub\n",
    );
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.result.is_err(),
        "invalid interval should raise: {outcome:?}"
    );
    assert_eq!(outcome.err.number, 5, "Partition invalid interval -> 5");
}
