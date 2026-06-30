//! vm3 floating type-suffix literals carry the right subtype
//! (currency-single-float-suffix-literals).
//!
//! VBA's trailing type-declaration character types a numeric literal: `@` = Currency,
//! `!` = Single, `#` = Double. vm3 previously folded every `FloatLiteral` to `Double`,
//! so `1.5@` lost Currency (and its exactness) and `1.5!` lost Single (its f32 width).
//!
//! Oracle (live Excel via probe.ps1):
//!   TypeName: 1.5@=Currency, 1.5!=Single, 1.5#=Double, 1.5=Double,
//!             100@=Currency, 100!=Single, 100#=Double
//!   CStr(1.5@)=1.5 ; CDbl(0.1!)=0.100000001490116 (genuine f32 rounding)

use oxvba_differential::{Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_result(public_ty: &str, expr: &str) -> RunOutcome {
    let source =
        format!("Public result As {public_ty}\nSub Main()\n    result = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined as unsupported: {:?}\nsource:\n{source}",
        outcome.unsupported
    );
    outcome
}

fn assert_typename(literal: &str, expected: &str) {
    let outcome = run_result("String", &format!("TypeName({literal})"));
    let want = canon(&Variant::from_string(expected));
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(&want),
            "TypeName({literal}) = {values:?}, expected to contain {expected:?}"
        ),
        Err(msg) => panic!("vm3 run failed for TypeName({literal}): {msg}"),
    }
}

fn assert_cdbl(literal: &str, expected: f64) {
    let outcome = run_result("Double", &format!("CDbl({literal})"));
    let want = canon(&Variant::from_f64(expected));
    match &outcome.result {
        Ok(values) => assert!(
            values.contains(&want),
            "CDbl({literal}) = {values:?}, expected to contain {expected}"
        ),
        Err(msg) => panic!("vm3 run failed for CDbl({literal}): {msg}"),
    }
}

#[test]
fn at_suffix_is_currency() {
    assert_typename("1.5@", "Currency");
    assert_typename("100@", "Currency");
    assert_typename("19.99@", "Currency");
}

#[test]
fn bang_suffix_is_single() {
    assert_typename("1.5!", "Single");
    assert_typename("100!", "Single");
}

#[test]
fn hash_or_no_suffix_is_double() {
    assert_typename("1.5#", "Double");
    assert_typename("1.5", "Double");
    assert_typename("100#", "Double");
}

#[test]
fn currency_literal_values_are_exact_to_four_decimals() {
    assert_cdbl("1.5@", 1.5);
    assert_cdbl("19.99@", 19.99);
}

#[test]
fn single_literal_keeps_f32_rounding() {
    // 1.5 is exact in f32, so it round-trips; 0.1 is not, so the Single literal carries
    // the genuine f32 value (proving it is really an f32, not a Double).
    assert_cdbl("1.5!", 1.5);
    assert_cdbl("0.1!", 0.1f32 as f64);
}
