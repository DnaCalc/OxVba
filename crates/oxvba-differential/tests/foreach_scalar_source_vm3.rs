//! vm3 `For Each` should reject scalar sources instead of iterating zero times.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn run_body(body: &str) -> oxvba_differential::RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn error_number(body: &str) -> i32 {
    let outcome = run_body(body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(outcome.raised, "expected error, got {:?}", outcome.result);
    outcome.err.number
}

fn value(body: &str) -> Canon {
    let outcome = run_body(body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    snap.first().cloned().expect("snapshot slot")
}

#[test]
fn foreach_over_statically_scalar_source_raises_type_mismatch() {
    assert_eq!(
        error_number(
            "    Dim item As Variant\n    Dim x As Long\n    x = 5\n    For Each item In x\n        r = \"iterated\"\n    Next item\n"
        ),
        13
    );
}

#[test]
fn foreach_over_variant_held_scalar_source_raises_type_mismatch() {
    assert_eq!(
        error_number(
            "    Dim item As Variant\n    Dim x As Variant\n    x = 5\n    For Each item In x\n        r = \"iterated\"\n    Next item\n"
        ),
        13
    );
}

#[test]
fn foreach_over_array_control_still_iterates() {
    assert_eq!(
        value(
            "    Dim item As Variant\n    Dim a As Variant\n    a = Array(1, 2, 3)\n    Dim total As Long\n    For Each item In a\n        total = total + item\n    Next item\n    r = total\n"
        ),
        canon(&Variant::from_i32(6))
    );
}

#[test]
fn foreach_over_collection_control_still_iterates() {
    assert_eq!(
        value(
            "    Dim item As Variant\n    Dim c As New Collection\n    c.Add 2\n    c.Add 3\n    Dim total As Long\n    For Each item In c\n        total = total + item\n    Next item\n    r = total\n"
        ),
        canon(&Variant::from_i32(5))
    );
}

#[test]
fn foreach_over_array_clears_variant_item_on_completion() {
    assert_eq!(
        value(
            "    Dim item As Variant\n    For Each item In Array(1, 2, 3)\n    Next item\n    r = item\n"
        ),
        canon(&Variant::empty())
    );
}

#[test]
fn foreach_over_collection_clears_variant_item_on_completion() {
    assert_eq!(
        value(
            "    Dim item As Variant\n    Dim c As New Collection\n    c.Add 2\n    c.Add 3\n    For Each item In c\n    Next item\n    r = item\n"
        ),
        canon(&Variant::empty())
    );
}
