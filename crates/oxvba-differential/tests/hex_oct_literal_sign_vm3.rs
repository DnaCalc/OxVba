//! vm3 hex (`&H…`) / octal (`&O…`) integer literals apply the width-based
//! two's-complement sign rule (MS-VBAL §3.3.2), verified against live Office.
//!
//! The digits are an unsigned magnitude reinterpreted as a signed value of the
//! literal's *type width*: a trailing `%`/`&`/`^` fixes the width, otherwise
//! legal unsuffixed literals use the narrowest of Integer/Long that holds the
//! magnitude. So `&HFFFF` is -1 (Integer width), `&HFFFF&` is 65535 (Long),
//! `&HFFFFFFFF` is -1 (Long), and `&O37777777777` is -1 (octal of 0xFFFFFFFF).
//! Closes `vba-hex-oct-literal-sign`. `Val` keeps the literal sign rule for
//! `&H…`/`&O…` *strings*; explicit conversion functions use their target-width
//! conversion rules and reject trailing type suffixes.

use oxvba_differential::{Canon, Executor, RunOutcome, canon, run};
use oxvba_runtime::Variant;

fn run_main(body: &str) -> RunOutcome {
    let source = format!("Public r As Variant\nSub Main()\n{body}End Sub\n");
    run(Executor::Vm3, &source)
}

fn assert_value(body: &str, expected: &Canon) {
    let outcome = run_main(body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let snap = outcome.result.unwrap_or_else(|e| panic!("run failed: {e}"));
    assert_eq!(snap.first(), Some(expected), "{snap:?}");
}

fn assert_long(body: &str, expected: i32) {
    assert_value(body, &canon(&Variant::from_i32(expected)));
}

fn assert_integer(body: &str, expected: i16) {
    assert_value(body, &canon(&Variant::from_i16(expected)));
}

fn assert_error(body: &str, expected: &str) {
    let outcome = run_main(body);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    let err = outcome.result.expect_err("run should fail");
    assert!(err.contains(expected), "expected `{expected}` in `{err}`");
}

#[test]
fn hex_integer_width_two_s_complement() {
    assert_integer("    r = &HFFFF\n", -1);
    assert_integer("    r = &H8000\n", -32768);
    assert_integer("    r = &H7FFF\n", 32767);
    assert_integer("    r = &HFF\n", 255);
}

#[test]
fn hex_long_width_two_s_complement() {
    assert_long("    r = &H10000\n", 65536);
    assert_long("    r = &HFFFFFFFF\n", -1);
    assert_long("    r = &H80000000\n", i32::MIN);
}

#[test]
fn type_character_fixes_the_width() {
    // `&` forces Long even though 0xFFFF fits Integer → 65535, not -1.
    assert_long("    r = &HFFFF&\n", 65535);
    assert_long("    r = &HFFFFFFFF&\n", -1);
}

#[test]
fn octal_shares_the_rule() {
    // 0xFFFF == &O177777 (Integer width) and 0xFFFFFFFF == &O37777777777 (Long).
    assert_integer("    r = &O177777\n", -1);
    assert_long("    r = &O37777777777\n", -1);
    assert_integer("    r = &O17\n", 15);
}

#[test]
fn conversion_of_hex_string_applies_the_sign_rule() {
    // Excel/VBA oracle: `CLng`/`CDbl` apply Long-width conversion to radix
    // strings, while `CInt` applies Integer-width conversion.
    assert_long("    r = CLng(\"&HFFFFFFFF\")\n", -1);
    assert_long("    r = CLng(\"&HFFFF\")\n", 65_535);
    assert_value(
        "    r = CDbl(\"&HFFFFFFFF\")\n",
        &canon(&Variant::from_f64(-1.0)),
    );
    assert_value(
        "    r = CDbl(\"&HFFFF\")\n",
        &canon(&Variant::from_f64(65_535.0)),
    );
    assert_value("    r = CInt(\"&HFFFF\")\n", &canon(&Variant::from_i16(-1)));
    assert_error("    r = CInt(\"&H10000\")\n", "VBA error 6");
    assert_error("    r = CLng(\"&HFFFF&\")\n", "VBA error 13");
}

#[test]
fn val_of_radix_string_applies_the_sign_rule() {
    assert_value(
        "    r = Val(\"&HFFFFFFFF\")\n",
        &canon(&Variant::from_f64(-1.0)),
    );
    assert_value(
        "    r = Val(\"&HFFFF&\")\n",
        &canon(&Variant::from_f64(65_535.0)),
    );
    assert_value(
        "    r = Val(\"&O37777777777\")\n",
        &canon(&Variant::from_f64(-1.0)),
    );
    assert_value(
        "    r = Val(\"+&H7F\")\n",
        &canon(&Variant::from_f64(127.0)),
    );
}

// LongLong-width string conversions carry the sign rule in `parse_vba_radix`;
// literal LongLong carriers and unsuffixed beyond-Long syntax are covered by
// `integer_literal_carrier_vm3`.
