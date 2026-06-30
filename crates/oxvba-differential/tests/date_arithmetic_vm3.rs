//! vm3 Date arithmetic preserves the `Date` subtype (date-arith-loses-date-type).
//!
//! `Date + number`, `number + Date`, `Date + Date`, and a `-` with a *single* `Date`
//! operand all yield a `Date` in live VBA; `Date - Date` is the elapsed time as a plain
//! `Double`. vm3 previously coerced every `Date` arithmetic result to `Double` (the
//! `Date` subtype and `TypeName` were lost). `*`/`/`/`\`/`Mod` are never `Date`.
//!
//! Oracle (live Excel via probe.ps1, d = #1/1/2000#, serial 36526):
//!   TypeName: d+1=Date, 1+d=Date, d-1=Date, d-d=Double, d+d=Date, 5-d=Date,
//!             d*2=Double, d/2=Double
//!   CDbl:     d+1=36527, d-d=0, d+d=73052

use oxvba_differential::{Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_main(public_ty: &str, body: &str) -> RunOutcome {
    let source = format!(
        "Public result As {public_ty}\nSub Main()\n    Dim d As Date\n    d = #1/1/2000#\n    {body}\nEnd Sub\n"
    );
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined as unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    outcome
}

fn assert_typename(expr: &str, expected: &str) {
    let outcome = run_main("String", &format!("result = TypeName({expr})"));
    let want = canon(&Variant::from_string(expected));
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(&want),
            "TypeName({expr}) = {values:?}, expected to contain {expected:?}"
        ),
        Err(msg) => panic!("vm3 run failed for TypeName({expr}): {msg}"),
    }
}

fn assert_cdbl(expr: &str, expected: f64) {
    let outcome = run_main("Double", &format!("result = CDbl({expr})"));
    let want = canon(&Variant::from_f64(expected));
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(&want),
            "CDbl({expr}) = {values:?}, expected to contain {expected}"
        ),
        Err(msg) => panic!("vm3 run failed for CDbl({expr}): {msg}"),
    }
}

#[test]
fn addition_with_a_date_operand_is_a_date() {
    assert_typename("d + 1", "Date");
    assert_typename("1 + d", "Date");
    assert_typename("d + d", "Date");
    assert_typename("d + 100000", "Date");
    assert_typename("d - 1.5", "Date");
}

#[test]
fn subtraction_keeps_date_unless_both_are_dates() {
    assert_typename("d - 1", "Date");
    assert_typename("5 - d", "Date");
    // Two dates subtract to the elapsed Double, dropping the Date tag.
    assert_typename("d - d", "Double");
}

#[test]
fn multiplicative_operators_are_never_dates() {
    assert_typename("d * 2", "Double");
    assert_typename("d / 2", "Double");
}

#[test]
fn date_arithmetic_values_are_plain_serial_math() {
    assert_cdbl("d + 1", 36527.0);
    assert_cdbl("d - d", 0.0);
    assert_cdbl("d + d", 73052.0);
}
