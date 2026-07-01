//! vm3 hex (`&H…`) / octal (`&O…`) integer literals apply the width-based
//! two's-complement sign rule (MS-VBAL §3.3.2), verified against live Office.
//!
//! The digits are an unsigned magnitude reinterpreted as a signed value of the
//! literal's *type width*: a trailing `%`/`&`/`^` fixes the width, otherwise the
//! narrowest of Integer/Long/LongLong that holds the magnitude. So `&HFFFF` is
//! -1 (Integer width), `&HFFFF&` is 65535 (Long), `&HFFFFFFFF` is -1 (Long),
//! and `&O37777777777` is -1 (octal of 0xFFFFFFFF). Closes
//! `vba-hex-oct-literal-sign`. The same rule governs `Val`/`CLng` of
//! `&H…`/`&O…` *strings*.

use oxvba_differential::{canon, run, Canon, Executor, RunOutcome};
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

#[test]
fn hex_integer_width_two_s_complement() {
    assert_long("    r = &HFFFF\n", -1);
    assert_long("    r = &H8000\n", -32768);
    assert_long("    r = &H7FFF\n", 32767);
    assert_long("    r = &HFF\n", 255);
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
    assert_long("    r = &O177777\n", -1);
    assert_long("    r = &O37777777777\n", -1);
    assert_long("    r = &O17\n", 15);
}

#[test]
fn conversion_of_hex_string_applies_the_sign_rule() {
    // `CLng`/`CInt`/`CDbl` of a `&H…` string share the literal sign rule
    // (`parse_vba_numeric_string`): `CLng("&HFFFFFFFF")` is -1, `CInt("&HFFFF")`
    // is -1.
    assert_long("    r = CLng(\"&HFFFFFFFF\")\n", -1);
    assert_long("    r = CLng(\"&HFFFF\")\n", -1);
    assert_value("    r = CInt(\"&HFFFF\")\n", &canon(&Variant::from_i16(-1)));
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

// LongLong-width literals (`&H1FFFFFFFF`, `&HFFFFFFFFFFFFFFFF^`) carry the sign
// rule correctly in `parse_vba_radix`, but vm3 still truncates a 64-bit literal
// constant to Long when loading it — that is the separate LongLong-literal /
// integer-literal-surfaces-as-long carrier gap, not the sign rule.
