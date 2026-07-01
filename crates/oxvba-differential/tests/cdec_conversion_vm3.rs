//! vm3 `CDec` should bind and return a Variant carrying the Decimal subtype,
//! rather than failing as an unresolved built-in.

use oxvba_differential::{Executor, canon, run};
use oxvba_runtime::Variant;

fn string_result(body: &str) -> String {
    let source = format!("Public result As String\nSub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 CDec probe failed: {err}\n{source}"));
    match values.first() {
        Some(oxvba_differential::Canon::Str(value)) => value.clone(),
        other => panic!("expected string result, got {other:?} from {values:?}"),
    }
}

fn error_number(expr: &str) -> i32 {
    let source = format!("Sub Main()\n    Dim v As Variant\n    v = {expr}\nEnd Sub\n");
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
fn cdec_returns_decimal_variant_subtype() {
    assert_eq!(
        string_result(
            "    result = CStr(VarType(CDec(10))) & \":\" & TypeName(CDec(10)) & \":\" & CStr(CDec(10))"
        ),
        "14:Decimal:10"
    );
}

#[test]
fn cdec_preserves_decimal_string_precision() {
    assert_eq!(
        string_result("    result = CStr(CDec(\"2.56989797976466769416958\"))"),
        "2.56989797976466769416958"
    );
}

#[test]
fn cdec_accepts_exponent_text_and_converts_back_to_double() {
    assert_eq!(
        string_result("    result = CStr(CDec(\"1.25E3\")) & \":\" & CStr(CDbl(CDec(\"123.45\")))"),
        "1250:123.45"
    );
}

#[test]
fn cdec_error_numbers_match_conversion_family() {
    assert_eq!(error_number("CDec(Null)"), 94);
    assert_eq!(error_number("CDec(\"not numeric\")"), 13);
    assert_eq!(error_number("CDec(\"79228162514264337593543950336\")"), 6);
}

#[test]
fn cdec_raw_decimal_payload_is_stable_for_integer_input() {
    let source = "Public result As Variant\nSub Main()\n    result = CDec(10)\nEnd Sub\n";
    let outcome = run(Executor::Vm3, source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 CDec integer payload failed: {err}"));
    assert!(
        values.contains(&canon(&Variant::from_decimal96(
            oxvba_runtime::Decimal96::from_parts(10, 0, 0, 0, false)
        ))),
        "expected raw Decimal(10), got {values:?}"
    );
}
