//! vm3 `Val` should parse the longest complete leading numeric token, matching
//! live Excel/VBA 7.1 for incomplete exponents, embedded signs/dots, and ignored
//! ASCII whitespace.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn value(expr: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n    r = {expr}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("Val probe failed: {err}\n{source}"));
    values
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("empty result for {expr}: {values:?}"))
}

fn f64v(value: f64) -> Canon {
    canon(&Variant::from_f64(value))
}

#[test]
fn val_stops_at_incomplete_or_invalid_decimal_continuation() {
    for (expr, expected) in [
        ("Val(\"123abc\")", 123.0),
        ("Val(\"12-3\")", 12.0),
        ("Val(\"1.2.3\")", 1.2),
        ("Val(\"1e\")", 1.0),
        ("Val(\"1e+\")", 1.0),
        ("Val(\"+\")", 0.0),
        ("Val(\"-\")", 0.0),
        ("Val(\".\")", 0.0),
        ("Val(\"$1\")", 0.0),
        ("Val(\"1,234\")", 1.0),
    ] {
        assert_eq!(value(expr), f64v(expected), "{expr}");
    }
}

#[test]
fn val_accepts_complete_decimal_and_d_exponents() {
    for (expr, expected) in [
        ("Val(\".5\")", 0.5),
        ("Val(\"-.5\")", -0.5),
        ("Val(\"1e2\")", 100.0),
        ("Val(\"1e+2\")", 100.0),
        ("Val(\"1e-2\")", 0.01),
        ("Val(\"1D2\")", 100.0),
    ] {
        assert_eq!(value(expr), f64v(expected), "{expr}");
    }
}

#[test]
fn val_ignores_ascii_whitespace_before_parsing() {
    assert_eq!(value("Val(\"1 2\")"), f64v(12.0));
    assert_eq!(value("Val(\"1\" & vbTab & \"2\")"), f64v(12.0));
    assert_eq!(value("Val(\"- .5\")"), f64v(-0.5));
}

#[test]
fn val_radix_prefix_behavior_stays_intact() {
    assert_eq!(value("Val(\"&H F F trailing\")"), f64v(255.0));
    assert_eq!(value("Val(\"+ &H7F\")"), f64v(127.0));
    assert_eq!(value("Val(\"- &O10\")"), f64v(-8.0));
}
