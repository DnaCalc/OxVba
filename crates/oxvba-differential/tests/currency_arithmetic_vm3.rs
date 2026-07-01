//! vm3 Currency arithmetic stays exact near the scaled-i64 boundary
//! (currency-mul-f64-lossy).
//!
//! Currency is a 64-bit integer scaled by 10,000. The VM must not route
//! Currency +/-/* through `f64`: near the top of the range, a double cannot carry
//! the low scaled units that decide the correct four-decimal result.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_main(public_ty: &str, body: &str) -> RunOutcome {
    let source = format!("Public result As {public_ty}\nSub Main()\n{body}End Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined as unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    outcome
}

fn assert_result(public_ty: &str, body: &str, expected: Canon) {
    let outcome = run_main(public_ty, body);
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(&expected),
            "snapshot {values:?} did not contain {expected:?}"
        ),
        Err(msg) => panic!("vm3 run failed: {msg}"),
    }
}

#[test]
fn currency_multiply_near_boundary_uses_scaled_integer_math() {
    assert_result(
        "Currency",
        "    Dim a As Currency\n    Dim b As Currency\n    a = CCur(\"30000000.0001\")\n    b = CCur(\"30000000.0001\")\n    result = a * b\n",
        canon(&Variant::from_currency_scaled_i64(
            9_000_000_000_060_000_000,
        )),
    );
}

#[test]
fn variant_currency_multiply_preserves_currency_subtype() {
    assert_result(
        "Variant",
        "    Dim a As Currency\n    Dim b As Currency\n    a = CCur(\"1.2345\")\n    b = CCur(\"6.7891\")\n    result = a * b\n",
        canon(&Variant::from_currency_scaled_i64(83_811)),
    );
}

#[test]
fn currency_multiply_rounds_half_scaled_units_to_even() {
    assert_result(
        "String",
        "    result = CStr(CCur(\"0.0001\") * CCur(\"0.5\")) & \"|\" & CStr(CCur(\"0.0003\") * CCur(\"0.5\"))\n",
        canon(&Variant::from_string("0|0.0002")),
    );
}
