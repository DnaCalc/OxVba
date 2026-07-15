//! vm3 compares wide numeric carriers (LongLong/Currency/Decimal) exactly, not
//! through f64. `9007199254740993^ = 9007199254740992^` (2^53+1 vs 2^53 as
//! LongLong) used to return True because both round to the same f64 —
//! inconsistent with the (exact i128) arithmetic. (An unsuffixed integer literal
//! beyond Long range is a Double in VBA, so the `^` LongLong suffix is required
//! to hold the exact value; the same-carrier exactness is unit-tested in
//! oxvba-eval `wide_integer_comparison_is_exact_not_via_f64`.)

use oxvba_differential::{Executor, canon, run};
use oxvba_runtime::Variant;

fn is_true(expr: &str) -> bool {
    let source = format!("Public result As Boolean\nSub Main()\n    result = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|e| panic!("run failed for {expr}: {e}"));
    values.contains(&canon(&Variant::from_bool(true)))
}

#[test]
fn longlong_equality_is_exact_past_2_pow_53() {
    // 2^53 = 9007199254740992, 2^53 + 1 = 9007199254740993 (LongLong literals).
    assert!(
        !is_true("9007199254740993^ = 9007199254740992^"),
        "2^53+1 must not equal 2^53"
    );
    assert!(is_true("9007199254740993^ > 9007199254740992^"));
    assert!(!is_true("9007199254740992^ > 9007199254740993^"));
}

#[test]
fn longlong_comparison_agrees_with_arithmetic() {
    // The difference is exactly 1, so equality/ordering must reflect that.
    assert!(is_true("9007199254740993^ - 9007199254740992^ = 1"));
    assert!(is_true("9007199254740993^ <> 9007199254740992^"));
}
