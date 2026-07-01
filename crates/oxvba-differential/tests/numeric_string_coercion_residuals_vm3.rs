//! vm3 numeric/string coercion residuals from the Tier 4/5 inventory.

use oxvba_differential::{Canon, Executor, canon, run};
use oxvba_runtime::Variant;

fn value(body: &str) -> Canon {
    let source = format!("Public r As Variant\nSub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\n{source}"));
    values.first().cloned().expect("global r")
}

fn error_number(body: &str) -> i32 {
    let source = format!("Public r As Variant\nSub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    assert!(
        outcome.raised,
        "expected raised VBA error, got {:?}\n{source}",
        outcome.result
    );
    outcome.err.number
}

#[test]
fn cstr_of_null_raises_invalid_use_of_null() {
    assert_eq!(
        error_number("    Dim n As Variant\n    n = Null\n    r = CStr(n)"),
        94
    );
}

#[test]
fn empty_plus_numeric_promotes_to_double_but_empty_plus_string_is_mismatch() {
    assert_eq!(
        value(
            "    Dim e As Variant\n    r = CStr(VarType(e + 1)) & \":\" & TypeName(e + 1) & \":\" & CStr(e + 1) & \":\" & CStr(VarType(1 + e))"
        ),
        canon(&Variant::from_string("5:Double:1:5"))
    );
    assert_eq!(error_number("    Dim e As Variant\n    r = e + \"5\""), 13);
}

#[test]
fn arithmetic_numeric_string_parser_rejects_rust_only_float_spellings() {
    assert_eq!(
        value("    r = 1 + \"1e2\""),
        canon(&Variant::from_f64(101.0))
    );
    assert_eq!(error_number("    r = 1 + \"NaN\""), 13);
    assert_eq!(error_number("    r = 1 + \"inf\""), 13);
}

#[test]
fn negative_base_fractional_power_raises_error_5_instead_of_nan() {
    assert_eq!(error_number("    r = (-8) ^ 0.5"), 5);
    assert_eq!(value("    r = (-8) ^ 2"), canon(&Variant::from_f64(64.0)));
}

#[test]
fn left_right_mid_index_by_utf16_code_unit() {
    assert_eq!(
        value(
            "    Dim s As String\n    s = ChrW(&HD83D) & ChrW(&HDE00) & \"Z\"\n    r = CStr(Left(s, 1) = ChrW(&HD83D)) & \":\" & CStr(Right(s, 2) = ChrW(&HDE00) & \"Z\") & \":\" & CStr(Mid(s, 2, 1) = ChrW(&HDE00)) & \":\" & CStr(Len(Left(s, 1)))"
        ),
        canon(&Variant::from_string("True:True:True:1"))
    );
}

#[test]
fn strconv_byte_modes_convert_through_byte_arrays() {
    assert_eq!(
        value(
            "    Dim bytes() As Byte\n    bytes = StrConv(\"AZ\", 128)\n    r = CStr(VarType(bytes)) & \":\" & TypeName(bytes) & \":\" & CStr(LBound(bytes)) & \":\" & CStr(UBound(bytes)) & \":\" & CStr(bytes(0)) & \":\" & CStr(bytes(1))"
        ),
        canon(&Variant::from_string("8209:Byte():0:1:65:90"))
    );
    assert_eq!(
        value(
            "    Dim bytes(0 To 1) As Byte\n    bytes(0) = 65\n    bytes(1) = 90\n    r = StrConv(bytes, 64)"
        ),
        canon(&Variant::from_string("AZ"))
    );
    assert_eq!(
        value("    r = StrConv(\"AZ\", 64)"),
        canon(&Variant::from_string("AZ"))
    );
}
