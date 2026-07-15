//! `Round`/`Sgn` of `Null` raise "Invalid use of Null" (94); they are NOT
//! Null-propagating. This distinguishes them from the Variant-preserving
//! `Abs`/`Int`/`Fix` (which return Null for a Null argument — see
//! `abs_int_fix_sgn_vm3`): `Round` and `Sgn` coerce their argument to a numeric
//! type (Double / Integer), and coercing `Null` to a concrete numeric type is
//! error 94. `Sgn(Null)=94` is verified against live Office VBA 7.1; `Round`
//! shares the same coerce-to-numeric mechanism (`as_f64`). This test guards the
//! behavior so the (previously mis-reported "should propagate Null") finding is
//! not re-applied.

use oxvba_differential::{Executor, run};

fn error_number(expr: &str) -> i32 {
    let source = format!("Public r As Variant\nSub Main()\n    r = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected a VBA error for {expr}, got {:?}",
        outcome.result
    );
    outcome.err.number
}

#[test]
fn round_and_sgn_of_null_raise_invalid_use_of_null() {
    assert_eq!(error_number("Round(Null)"), 94);
    assert_eq!(error_number("Round(Null, 2)"), 94);
    assert_eq!(error_number("Sgn(Null)"), 94);
}
